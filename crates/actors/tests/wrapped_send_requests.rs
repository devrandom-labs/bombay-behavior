use behavior::MailAddr;
use behavior::{
    Actions, Behavior, BehaviorBase, EventLayer, InterpreterRequests, Never, NoBirths,
    ReportToParent, Step, delegate_transition, initialize,
};
use behavior_actors::{
    OneShot, OwnedScheduleAfter, OwnedTimerElapsed, Periodic, StopOnShutdown, TimerGeneration,
};
use core::time::Duration;

struct Worker {
    notice: u32,
}

#[behavior::behavior(addr = MailAddr, message = Never, sends = { elapsed: InterpreterRequests<ReportToParent<u32>> })]
impl Worker {
    fn init(&mut self) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont().with_send(ReportToParent::new(self.notice)))
    }

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
        Duration::ZERO,
        report_elapsed_with_shutdown,
    );
    let initialized = initialize(&mut timer).expect("timer initializes");
    assert_eq!(
        initialized.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(0), Duration::ZERO)]
    );
    assert_eq!(
        initialized.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let schedule = initialized.sends.owned.as_slice()[0];
    let elapsed = OwnedTimerElapsed::new(schedule.generation);
    let first =
        delegate_transition(&mut timer, EventLayer::Owned(elapsed)).expect("elapsed is accepted");
    assert_eq!(
        first.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
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
        Duration::ZERO,
        report_elapsed,
    ));
    let initialized = initialize(&mut shutdown).expect("shutdown outside initializes timer");
    assert_eq!(
        initialized.sends.inner.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(0), Duration::ZERO)]
    );
    assert_eq!(
        initialized.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let schedule = initialized.sends.inner.owned.as_slice()[0];
    let elapsed = OwnedTimerElapsed::new(schedule.generation);
    let first = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("inner timer is accepted");
    assert_eq!(
        first.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
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
        Duration::ZERO,
        report_elapsed_with_shutdown,
    );
    let initialized = initialize(&mut timer).expect("periodic timer initializes");
    assert_eq!(
        initialized.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(0), Duration::ZERO)]
    );
    assert_eq!(
        initialized.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let schedule = initialized.sends.owned.as_slice()[0];
    let elapsed = OwnedTimerElapsed::new(schedule.generation);
    let first = delegate_transition(&mut timer, EventLayer::Owned(elapsed))
        .expect("first period is accepted");
    assert_eq!(
        first.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert_eq!(
        first.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(1), Duration::ZERO)]
    );
    assert!(first.creates.is_empty());
    assert!(matches!(first.become_, Step::Continue));
    let replay = delegate_transition(&mut timer, EventLayer::Owned(elapsed))
        .expect("old period is discharged");
    assert!(replay.sends.inner.inner.elapsed.is_empty());
    assert!(replay.sends.owned.is_empty());
    assert!(replay.creates.is_empty());
    assert!(matches!(replay.become_, Step::Continue));

    let second = delegate_transition(
        &mut timer,
        EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(1))),
    )
    .expect("the rearmed period is actually accepted");
    assert_eq!(
        second.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert_eq!(
        second.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(2), Duration::ZERO)]
    );
    assert!(second.creates.is_empty());
    assert!(matches!(second.become_, Step::Continue));

    let mut shutdown = StopOnShutdown::new(Periodic::new(
        Worker { notice: 1 },
        Duration::ZERO,
        report_elapsed,
    ));
    let initialized =
        initialize(&mut shutdown).expect("shutdown outside initializes periodic timer");
    assert_eq!(
        initialized.sends.inner.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(0), Duration::ZERO)]
    );
    assert_eq!(
        initialized.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let schedule = initialized.sends.inner.owned.as_slice()[0];
    let elapsed = OwnedTimerElapsed::new(schedule.generation);
    let first = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("inner period is accepted");
    assert_eq!(
        first.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert_eq!(
        first.sends.inner.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(1), Duration::ZERO)]
    );
    assert!(first.creates.is_empty());
    assert!(matches!(first.become_, Step::Continue));
    let replay = delegate_transition(&mut shutdown, EventLayer::Inner(EventLayer::Owned(elapsed)))
        .expect("old inner period is discharged");
    assert!(replay.sends.inner.inner.elapsed.is_empty());
    assert!(replay.sends.inner.owned.is_empty());
    assert_eq!(shutdown.base().notice, 1);
    assert!(replay.creates.is_empty());
    assert!(matches!(replay.become_, Step::Continue));
    let second = delegate_transition(
        &mut shutdown,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            1,
        )))),
    )
    .expect("the inner rearmed period is actually accepted");
    assert_eq!(
        second.sends.inner.inner.elapsed.as_slice(),
        [ReportToParent::new(1)]
    );
    assert_eq!(
        second.sends.inner.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(2), Duration::ZERO)]
    );
    assert!(second.creates.is_empty());
    assert!(matches!(second.become_, Step::Continue));
}
