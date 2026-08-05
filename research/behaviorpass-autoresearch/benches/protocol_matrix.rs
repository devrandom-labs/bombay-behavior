use std::hint::black_box;
use std::time::Duration;

use behaviorpass::{
    Acted, Actions, AtEvent, Base, Behavior, ChildStopped, Crash, Delivery, Fsm, MailAddr, Move,
    Never, Proxy, ProxyCommand, RestartPolicy, Spec, StashRoute, State, Step, Strategy,
    Supervising, SupervisionEvent, User, UserEvent, WatchEvent, stop_on_abnormal_death,
};
use tokio::runtime::Builder;
use tokio::time::Instant;

const ITERATIONS: usize = 250_000;
const SHORT_ITERATIONS: usize = 100_000;

struct Sink(u64);

impl State for Sink {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Never, Never> {
        self.0 = self.0.wrapping_add(message);
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Base<Sink> {
    Base::new(Sink(0))
}

/// The supervising parent: quiet, with the supervised child as its offspring
/// type (a fleet parent must produce the fleet, not `Never`).
struct FleetParent;

impl State<Never, Base<Sink>, Never> for FleetParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Base<Sink>, Never> {
        Ok(Actions::cont())
    }
}

fn main() {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let base_rate = runtime.block_on(measure_base());
    let proxy_rate = runtime.block_on(measure_proxy());
    let score = base_rate.min(proxy_rate);

    println!("METRIC score={score:.0}");
    println!("METRIC base_transitions_per_s={base_rate:.0}");
    println!("METRIC proxy_transitions_per_s={proxy_rate:.0}");

    let supervise_8 = runtime.block_on(measure_supervise(8));
    let supervise_256 = runtime.block_on(measure_supervise(256));
    let fsm_rate = runtime.block_on(measure_fsm());
    let stash_rate = runtime.block_on(measure_stash());
    let nested_rate = runtime.block_on(measure_nested());
    println!("METRIC supervise_8_tps={supervise_8:.0}");
    println!("METRIC supervise_256_tps={supervise_256:.0}");
    println!("METRIC fsm_tps={fsm_rate:.0}");
    println!("METRIC stash_tps={stash_rate:.0}");
    println!("METRIC nested_tps={nested_rate:.0}");
}

async fn measure_base() -> f64 {
    let mut behavior = Base::new(Sink(0));
    let started = Instant::now();
    for index in 0..ITERATIONS {
        let message = u64::try_from(index).unwrap();
        black_box(
            behavior
                .step(User::user(MailAddr(0), black_box(message)))
                .await
                .unwrap(),
        );
    }
    rate(ITERATIONS, started.elapsed())
}

async fn measure_proxy() -> f64 {
    let mut proxy = Proxy::new(child(0));
    proxy.init().await.unwrap();
    let started = Instant::now();
    for index in 0..ITERATIONS {
        let command = if index % 64 == 0 {
            ProxyCommand::Replace(child(index))
        } else {
            ProxyCommand::Forward(u64::try_from(index).unwrap())
        };
        black_box(
            proxy
                .step(User::user(MailAddr(0), black_box(command)))
                .await
                .unwrap(),
        );
    }
    rate(ITERATIONS, started.elapsed())
}

/// Child-stopped throughput for a fleet of `fleet` children under
/// OneForOne/Permanent with an unbounded budget: every event scans the
/// slot list (linear in fleet size) and appends one restart stamp
/// (linear memory growth in emitted events).
async fn measure_supervise(fleet: usize) -> f64 {
    let mut behavior = Supervising::new(
        Base::new(FleetParent),
        |index| u64::try_from(index).unwrap(),
        fleet,
        child,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    behavior.init().await.unwrap();
    let at = Instant::now();
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        let nonce = u64::try_from(index % fleet).unwrap();
        black_box(
            behavior
                .step(SupervisionEvent::ChildStopped(ChildStopped {
                    nonce,
                    outcome: Err(Crash::Failed),
                    at,
                }))
                .await
                .unwrap(),
        );
    }
    println!("info supervise_{fleet}_restarts_after={}", behavior.restarts_in_window());
    rate(SHORT_ITERATIONS, started.elapsed())
}

/// FSM with alternating phase changes: every other event drains (empty) held
/// queue. Probes deferral machinery overhead on the hot path.
async fn measure_fsm() -> f64 {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        A,
        B,
    }
    let mut machine = Fsm::new((), Phase::A, |phase, (): &mut (), _: &u64| {
        Ok::<Move<Phase>, Never>(match phase {
            Phase::A => Move::Goto(Phase::B),
            Phase::B => Move::Stay,
        })
    });
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        black_box(
            machine
                .step(User::user(MailAddr(0), u64::try_from(index).unwrap()))
                .await
                .unwrap(),
        );
    }
    rate(SHORT_ITERATIONS, started.elapsed())
}

/// Stash passthrough (every message routes Deliver): buffer machinery on
/// the hot path without holding.
async fn measure_stash() -> f64 {
    let mut behavior = Spec::new(Sink(0)).stash(|_| StashRoute::Deliver);
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        black_box(
            behavior
                .step(UserEvent::user(MailAddr(0), u64::try_from(index).unwrap()))
                .await
                .unwrap(),
        );
    }
    rate(SHORT_ITERATIONS, started.elapsed())
}

/// Three-layer wrapper (At over Watch over Stash) folding user messages:
/// probes event-routing and send-product wrap cost of the deepest common
/// stack.
async fn measure_nested() -> f64 {
    let due = Instant::now() + Duration::from_mins(1);
    let mut behavior = Spec::new(Sink(0))
        .stash(|_| StashRoute::Deliver)
        .watch(MailAddr(7), stop_on_abnormal_death)
        .at(Some(due), |_| Ok(Step::Continue));
    behavior.init().await.unwrap();
    let started = Instant::now();
    for index in 0..SHORT_ITERATIONS {
        let event = AtEvent::Inner(WatchEvent::Inner(User::user(
            MailAddr(0),
            u64::try_from(index).unwrap(),
        )));
        black_box(behavior.step(event).await.unwrap());
    }
    rate(SHORT_ITERATIONS, started.elapsed())
}

fn rate(iterations: usize, elapsed: Duration) -> f64 {
    f64::from(u32::try_from(iterations).unwrap()) / elapsed.as_secs_f64()
}
