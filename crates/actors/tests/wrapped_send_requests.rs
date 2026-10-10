use behavior::MailAddr;
use behavior::{
    Actions, Behavior, BehaviorBase, EventLayer, InterpreterRequests, Never, NoBirths,
    ReportToParent, Step, delegate_transition, initialize,
};
use behavior_actors::{OneShot, Periodic, StopOnShutdown, TimerElapsed, TimerId};
use core::time::Duration;

struct Worker {
    notice: u32,
}

#[behavior::behavior(addr = MailAddr, message = Never, sends = { elapsed: InterpreterRequests<ReportToParent<u32>> })]
impl Worker {
    fn receive(&mut self, _: MailAddr, message: Never) -> behavior::BehaviorActed<Self> {
        match message {}
    }
}

fn report_elapsed(worker: &mut Worker) -> Actions<MailAddr, Never, WorkerSends, NoBirths> {
    Actions::cont().with_send(ReportToParent::new(worker.notice))
}

fn report_elapsed_with_shutdown(
    worker: &mut StopOnShutdown<Worker>,
) -> Actions<MailAddr, Never, <StopOnShutdown<Worker> as Behavior>::Sends, NoBirths> {
    Actions::cont().with_send(ReportToParent::new(worker.base().notice))
}

#[test]
fn one_shot_orders_keep_the_report_and_discharge_duplicate_elapsed() {
    let mut timer = OneShot::new(
        StopOnShutdown::new(Worker { notice: 1 }),
        TimerId(1),
        Duration::ZERO,
        report_elapsed_with_shutdown,
    );
    let initialized = initialize(&mut timer).expect("timer initializes");
    let schedule = initialized.sends.owned.as_slice()[0];
    let elapsed = TimerElapsed::new(schedule.id, schedule.generation);
    let first =
        delegate_transition(&mut timer, EventLayer::Owned(elapsed)).expect("elapsed is accepted");
    assert_eq!(first.sends.inner.inner.elapsed.as_slice()[0].report, 1);
    assert!(first.sends.owned.is_empty());
    assert!(first.creates.is_empty());
    assert!(matches!(first.become_, Step::Continue));
    let replay = delegate_transition(&mut timer, EventLayer::Owned(elapsed))
        .expect("duplicate is discharged");
    assert!(replay.sends.inner.inner.elapsed.is_empty());
    assert!(replay.sends.owned.is_empty());
    assert!(replay.creates.is_empty());
    assert!(matches!(replay.become_, Step::Continue));

    let mut shutdown = StopOnShutdown::new(OneShot::new(
        Worker { notice: 1 },
        TimerId(2),
        Duration::ZERO,
        report_elapsed,
    ));
    let initialized = initialize(&mut shutdown).expect("shutdown outside initializes timer");
    let schedule = initialized.sends.inner.owned.as_slice()[0];
    let elapsed = TimerElapsed::new(schedule.id, schedule.generation);
    let first = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("inner timer is accepted");
    assert_eq!(first.sends.inner.inner.elapsed.as_slice()[0].report, 1);
    assert_eq!(shutdown.base().notice, 1);
    assert!(first.sends.inner.owned.is_empty());
    assert!(first.creates.is_empty());
    assert!(matches!(first.become_, Step::Continue));
    let replay = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("duplicate is discharged");
    assert!(replay.sends.inner.inner.elapsed.is_empty());
    assert_eq!(shutdown.base().notice, 1);
    assert!(replay.sends.inner.owned.is_empty());
    assert!(replay.creates.is_empty());
    assert!(matches!(replay.become_, Step::Continue));
}

#[test]
fn periodic_orders_preserve_report_and_rearm_generation() {
    let mut timer = Periodic::new(
        StopOnShutdown::new(Worker { notice: 1 }),
        TimerId(3),
        Duration::ZERO,
        report_elapsed_with_shutdown,
    );
    let initialized = initialize(&mut timer).expect("periodic timer initializes");
    let schedule = initialized.sends.owned.as_slice()[0];
    let elapsed = TimerElapsed::new(schedule.id, schedule.generation);
    let first = delegate_transition(&mut timer, EventLayer::Owned(elapsed))
        .expect("first period is accepted");
    assert_eq!(first.sends.inner.inner.elapsed.as_slice()[0].report, 1);
    assert_eq!(first.sends.owned.len(), 1);
    assert_ne!(
        first.sends.owned.as_slice()[0].generation,
        schedule.generation
    );
    let replay = delegate_transition(&mut timer, EventLayer::Owned(elapsed))
        .expect("old period is discharged");
    assert!(replay.sends.inner.inner.elapsed.is_empty());
    assert!(replay.sends.owned.is_empty());

    let mut shutdown = StopOnShutdown::new(Periodic::new(
        Worker { notice: 1 },
        TimerId(4),
        Duration::ZERO,
        report_elapsed,
    ));
    let initialized =
        initialize(&mut shutdown).expect("shutdown outside initializes periodic timer");
    let schedule = initialized.sends.inner.owned.as_slice()[0];
    let elapsed = TimerElapsed::new(schedule.id, schedule.generation);
    let first = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("inner period is accepted");
    assert_eq!(first.sends.inner.inner.elapsed.as_slice()[0].report, 1);
    assert_eq!(first.sends.inner.owned.len(), 1);
    assert_ne!(
        first.sends.inner.owned.as_slice()[0].generation,
        schedule.generation
    );
    let replay = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("old inner period is discharged");
    assert!(replay.sends.inner.inner.elapsed.is_empty());
    assert!(replay.sends.inner.owned.is_empty());
    assert_eq!(shutdown.base().notice, 1);
}
