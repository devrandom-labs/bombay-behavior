use std::hint::black_box;
use std::time::Duration;

use behavior_actors::{Machine, Move, StashRoute, stop_on_abnormal_death};

use behavior_core::{Acted, Actions, MailAddr, Never, Step};
use behavior_testkit::InitializeTest;
use std::time::Instant;

const ITERATIONS: usize = 250_000;
const SHORT_ITERATIONS: usize = 100_000;

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
    let base_rate = measure_base();
    println!("METRIC base_transitions_per_s={base_rate:.0}");

    let fsm_rate = measure_fsm();
    let stash_rate = measure_stash();
    let nested_rate = measure_nested();
    println!("METRIC fsm_tps={fsm_rate:.0}");
    println!("METRIC stash_tps={stash_rate:.0}");
    println!("METRIC nested_tps={nested_rate:.0}");

    let end_delay_ms = iterations("BOMBAY_BENCH_END_DELAY_MS", 0);
    if end_delay_ms != 0 {
        std::thread::sleep(Duration::from_millis(u64::try_from(end_delay_ms).unwrap()));
    }
}

fn measure_base() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_ITERATIONS", ITERATIONS);
    let mut behavior = Sink(0);
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
    rate(iterations, started.elapsed())
}

/// FSM with alternating phase changes: every other event drains (empty) held
/// queue. Probes deferral machinery overhead on the hot path.
fn measure_fsm() -> f64 {
    let iterations = iterations("BOMBAY_BENCH_SHORT_ITERATIONS", SHORT_ITERATIONS);
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        A,
        B,
    }
    let machine = Machine::new((), Phase::A, |phase, (): &mut (), _: &u64| {
        Ok::<Move<Phase>, Never>(match phase {
            Phase::A => Move::Goto(Phase::B),
            Phase::B => Move::Stay,
        })
    });
    let mut machine = machine.initialize().unwrap().behavior;
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
    rate(iterations, started.elapsed())
}

fn rate(iterations: usize, elapsed: Duration) -> f64 {
    f64::from(u32::try_from(iterations).unwrap()) / elapsed.as_secs_f64()
}
