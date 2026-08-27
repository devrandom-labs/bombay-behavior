use std::hint::black_box;
use std::time::Duration;

use behavior::{
    Acted, Actions, Crash, CreationResolved, Machine, MailAddr, Move, Never, Proxy, RestartPolicy,
    StashRoute, Step, Strategy, Supervisor, WorkerStopped, stop_on_abnormal_death,
};
use behavior_testkit::InitializeTest;
use std::time::Instant;

const ITERATIONS: usize = 250_000;
const SHORT_ITERATIONS: usize = 100_000;

struct Sink(u64);

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::NoBirths, error = Never)]
impl Sink {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::NoBirths, Never> {
        self.0 = self.0.wrapping_add(message);
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Sink {
    Sink(0)
}

fn main() {
    let base_rate = measure_base();
    let proxy_rate = measure_proxy();
    let score = base_rate.min(proxy_rate);

    println!("METRIC score={score:.0}");
    println!("METRIC base_transitions_per_s={base_rate:.0}");
    println!("METRIC proxy_transitions_per_s={proxy_rate:.0}");

    let supervise_8 = measure_supervise(8);
    let supervise_256 = measure_supervise(256);
    let fsm_rate = measure_fsm();
    let stash_rate = measure_stash();
    let nested_rate = measure_nested();
    println!("METRIC supervise_8_tps={supervise_8:.0}");
    println!("METRIC supervise_256_tps={supervise_256:.0}");
    println!("METRIC fsm_tps={fsm_rate:.0}");
    println!("METRIC stash_tps={stash_rate:.0}");
    println!("METRIC nested_tps={nested_rate:.0}");
}

fn measure_base() -> f64 {
    let mut behavior = Sink(0);
    let started = Instant::now();
    for index in 0..ITERATIONS {
        let message = u64::try_from(index).unwrap();
        let actions = behavior.receive(MailAddr(0), black_box(message)).unwrap();
        black_box((
            actions.sends.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    rate(ITERATIONS, started.elapsed())
}

fn measure_proxy() -> f64 {
    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let mut proxy = initialized.behavior;
    let installed = proxy
        .on_path(CreationResolved::birth(0, MailAddr(1)))
        .unwrap();
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable_reports.is_empty());
    assert!(installed.sends.child_observations.is_empty());
    assert!(installed.sends.creation_observations.is_empty());
    assert!(installed.sends.stopped_reports.is_empty());
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(installed.sends.shutdowns.is_empty());
    assert!(installed.creates.is_empty());
    assert!(matches!(installed.become_, Step::Continue));
    let started = Instant::now();
    for index in 0..ITERATIONS {
        let command = u64::try_from(index).unwrap();
        let actions = proxy.receive(MailAddr(0), black_box(command)).unwrap();
        black_box((
            actions.sends.deliveries.len(),
            actions.sends.unavailable_reports.len(),
            actions.sends.child_observations.len(),
            actions.sends.creation_observations.len(),
            actions.sends.stopped_reports.len(),
            actions.sends.creation_reports.len(),
            actions.sends.shutdowns.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    rate(ITERATIONS, started.elapsed())
}

/// Child-stopped throughput for a fleet of `fleet` children under
/// OneForOne/Permanent with an unbounded budget: every event scans the
/// slot list (linear in fleet size) and appends one restart stamp
/// (linear memory growth in emitted events).
fn measure_supervise(fleet: usize) -> f64 {
    let behavior = Supervisor::new(
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            fleet,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let at = Instant::now();
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        let nonce = u64::try_from(index % fleet).unwrap();
        let actions = behavior
            .on_path(WorkerStopped {
                proxy: nonce,
                worker: nonce,
                outcome: Err(Crash::Failed),
                at,
            })
            .unwrap();
        // Asserting stress workload: every death yields exactly one
        // replacement routed to the dead slot (OneForOne, Permanent,
        // unbounded budget) — correctness checked while measuring.
        assert_eq!(actions.sends.replacement_inputs.len(), 1);
        assert_eq!(actions.sends.replacement_inputs[0].nonce, nonce);
        black_box((
            actions.sends.child_observations.len(),
            actions.sends.creation_observations.len(),
            actions.sends.schedules.len(),
            actions.sends.replacement_inputs.len(),
            actions.sends.failure_reports.len(),
            actions.sends.shutdowns.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    println!(
        "info supervise_{fleet}_restarts_after={}",
        behavior.restarts_in_window()
    );
    rate(SHORT_ITERATIONS, started.elapsed())
}

/// FSM with alternating phase changes: every other event drains (empty) held
/// queue. Probes deferral machinery overhead on the hot path.
fn measure_fsm() -> f64 {
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
    for index in 0..SHORT_ITERATIONS {
        let actions = machine
            .receive(MailAddr(0), u64::try_from(index).unwrap())
            .unwrap();
        black_box((
            actions.sends.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    rate(SHORT_ITERATIONS, started.elapsed())
}

/// Stash passthrough (every message routes Deliver): buffer machinery on
/// the hot path without holding.
fn measure_stash() -> f64 {
    let behavior = behavior::Stash::new(Sink(0), |_| StashRoute::Deliver);
    let mut behavior = behavior.initialize().unwrap().behavior;
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        let actions = behavior
            .receive(MailAddr(0), u64::try_from(index).unwrap())
            .unwrap();
        black_box((
            actions.sends.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    rate(SHORT_ITERATIONS, started.elapsed())
}

/// Three-layer wrapper (Deadline over Watch over Stash) folding user messages:
/// probes event-routing and send-product wrap cost of the deepest common
/// stack.
fn measure_nested() -> f64 {
    let due = Instant::now() + Duration::from_mins(1);
    let behavior = behavior::Deadline::new(
        behavior::Watch::new(
            behavior::Stash::new(Sink(0), |_| StashRoute::Deliver),
            MailAddr(7),
            stop_on_abnormal_death,
        ),
        behavior::TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        let actions = behavior
            .receive(MailAddr(0), u64::try_from(index).unwrap())
            .unwrap();
        black_box((
            actions.sends.owned.len(),
            actions.sends.inner.owned.len(),
            actions.sends.inner.inner.len(),
            actions.creates.len(),
            matches!(actions.become_, Step::Continue),
        ));
    }
    rate(SHORT_ITERATIONS, started.elapsed())
}

fn rate(iterations: usize, elapsed: Duration) -> f64 {
    f64::from(u32::try_from(iterations).unwrap()) / elapsed.as_secs_f64()
}
