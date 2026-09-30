use std::hint::black_box;
use std::time::{Duration, Instant};

use behavior_actors::{Activate, Machine, Move, StashRoute, stop_on_abnormal_death};
use behavior_core::{Acted, Actions, MailAddr, Never, Step};

const ITERATIONS: usize = 250_000;
const SHORT_ITERATIONS: usize = 100_000;
const SAMPLES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    A,
    B,
}

fn alternate(phase: Phase, state: &mut u64, message: &u64) -> Result<Move<Phase>, Never> {
    *state = state.wrapping_add(*message);
    Ok(Move::Goto(match phase {
        Phase::A => Phase::B,
        Phase::B => Phase::A,
    }))
}

fn iterations(variable: &str, default: usize) -> usize {
    std::env::var(variable)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

struct Sink(u64);

#[behavior_core::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior_core::NoBirths, error = Never)]
impl Sink {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior_core::NoBirths, Never> {
        self.0 = self.0.wrapping_add(message);
        Ok(Actions::cont())
    }
}

fn main() {
    preflight_fsm();
    samples("base_transitions_per_s", measure_base);
    samples("fsm_tps", measure_fsm);
    samples("stash_tps", measure_stash);
    samples("nested_tps", measure_nested);
    samples("machine_clone_ops_per_s", measure_machine_clone);

    let end_delay_ms = iterations("BOMBAY_BENCH_END_DELAY_MS", 0);
    if end_delay_ms != 0 {
        std::thread::sleep(Duration::from_millis(u64::try_from(end_delay_ms).unwrap()));
    }
}

fn samples(name: &str, measure: fn() -> f64) {
    let count = iterations("BOMBAY_BENCH_SAMPLES", SAMPLES);
    assert!(count > 0, "a benchmark needs at least one sample");
    let mut rates: Vec<_> = (0..count).map(|_| measure()).collect();
    rates.sort_by(f64::total_cmp);
    println!("METRIC {name}_min={:.0}", rates[0]);
    println!("METRIC {name}_median={:.0}", rates[rates.len() / 2]);
    println!("METRIC {name}_max={:.0}", rates[rates.len() - 1]);
    println!("METRIC {name}_samples={count}");
}

fn preflight_fsm() {
    let mut machine = Machine::new(0_u64, Phase::A, alternate)
        .initialize()
        .unwrap()
        .behavior;
    for (message, expected) in [(1, Phase::B), (2, Phase::A), (3, Phase::B)] {
        let actions = machine.receive(MailAddr(0), message).unwrap();
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(machine.phase(), expected);
    }
    assert_eq!(*machine.state(), 6);
    assert_eq!(machine.held(), 0);
}

fn measure_base() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_ITERATIONS", ITERATIONS);
    let mut behavior = Sink(0).initialize().unwrap().behavior;
    let started = Instant::now();
    for index in 0..iterations {
        let message = u64::try_from(index).unwrap();
        let actions = behavior.receive(MailAddr(0), black_box(message)).unwrap();
        black_box((
            actions.sends.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    black_box(behavior.0);
    rate(iterations, started.elapsed())
}

/// FSM with a phase change on every event and an empty held queue.
fn measure_fsm() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_SHORT_ITERATIONS", SHORT_ITERATIONS);
    let mut machine = Machine::new(0_u64, Phase::A, alternate)
        .initialize()
        .unwrap()
        .behavior;
    let started = Instant::now();
    for index in 0..iterations {
        let actions = machine
            .receive(MailAddr(0), u64::try_from(index).unwrap())
            .unwrap();
        black_box((
            actions.sends.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    assert_eq!(machine.held(), 0);
    assert_eq!(
        machine.phase(),
        if iterations % 2 == 0 {
            Phase::A
        } else {
            Phase::B
        }
    );
    black_box(*machine.state());
    rate(iterations, started.elapsed())
}

/// Stash passthrough (every message routes Deliver): buffer machinery on
/// the hot path without holding.
fn measure_stash() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_SHORT_ITERATIONS", SHORT_ITERATIONS);
    let behavior = behavior_actors::Stash::new(Sink(0), |_| StashRoute::Deliver);
    let mut behavior = behavior.initialize().unwrap().behavior;
    let started = Instant::now();
    for index in 0..iterations {
        let actions = behavior
            .receive(MailAddr(0), u64::try_from(index).unwrap())
            .unwrap();
        black_box((
            actions.sends.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    black_box(behavior.base().0);
    rate(iterations, started.elapsed())
}

/// Three-layer wrapper (Deadline over Watch over Stash) folding user messages:
/// probes event-routing and send-product wrap cost of the deepest common
/// stack.
fn measure_nested() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_SHORT_ITERATIONS", SHORT_ITERATIONS);
    let due = Instant::now() + Duration::from_mins(1);
    let behavior = behavior_actors::Deadline::new(
        behavior_actors::Watch::new(
            behavior_actors::Stash::new(Sink(0), |_| StashRoute::Deliver),
            MailAddr(7),
            stop_on_abnormal_death,
        ),
        behavior_actors::TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let started = Instant::now();
    for index in 0..iterations {
        let actions = behavior
            .receive(MailAddr(0), u64::try_from(index).unwrap())
            .unwrap();
        black_box((
            actions.sends,
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    black_box(behavior.base().0);
    rate(iterations, started.elapsed())
}

/// Clone a nonempty machine state and held queue to measure rollback cost.
fn measure_machine_clone() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_SHORT_ITERATIONS", SHORT_ITERATIONS);
    let mut machine = Machine::new(vec![7_u64; 64], (), |_, _: &mut Vec<u64>, _: &u64| {
        Ok::<Move<()>, Never>(Move::Defer)
    })
    .initialize()
    .unwrap()
    .behavior;
    for message in 0..128 {
        let actions = machine.receive(MailAddr(0), message).unwrap();
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }
    assert_eq!(machine.state().len(), 64);
    assert_eq!(machine.held(), 128);
    let started = Instant::now();
    for _ in 0..iterations {
        let cloned = (*machine).clone();
        black_box((&cloned.state()[0], cloned.held()));
    }
    rate(iterations, started.elapsed())
}

fn rate(iterations: usize, elapsed: Duration) -> f64 {
    assert!(iterations > 0, "a benchmark needs at least one iteration");
    f64::from(u32::try_from(iterations).unwrap()) / elapsed.as_secs_f64()
}
