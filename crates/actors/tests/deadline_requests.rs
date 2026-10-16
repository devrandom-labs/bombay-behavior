use behavior::{
    Actions, Become, BehaviorBase, EventLayer, InterpreterRequests, MailAddr, ReportToParent, Step,
    Stopped, User, UserEvent, delegate_transition, initialize,
};
use behavior_actors::{
    Deadline, OwnedScheduleAfter, OwnedScheduleAt, OwnedTimerElapsed, ReceiveTimeout,
    StopOnShutdown, TimerGeneration,
};
use std::time::{Duration, Instant};

struct Journal {
    entries: u32,
}

#[behavior::behavior(addr = MailAddr, message = u32, sends = { reports: InterpreterRequests<ReportToParent<u32>> })]
impl Journal {
    fn init(&mut self) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont().with_send(ReportToParent::new(self.entries)))
    }

    fn receive(&mut self, _: MailAddr, entries: u32) -> behavior::BehaviorActed<Self> {
        self.entries += entries;
        Ok(Actions::cont().with_send(ReportToParent::new(self.entries)))
    }
}

fn deadline(journal: &mut Journal) -> Become {
    journal.entries += 10;
    Step::Continue
}

fn deadline_with_shutdown(journal: &mut StopOnShutdown<Journal>) -> Become {
    assert_eq!(journal.base().entries, 1);
    Step::Stop(Stopped)
}

#[test]
fn deadline_orders_preserve_initialization_and_consume_only_the_current_generation() {
    let at = Instant::now();
    let mut journal =
        StopOnShutdown::new(Deadline::new(Journal { entries: 1 }, Some(at), deadline));
    let initialized = initialize(&mut journal).expect("deadline initializes");
    assert_eq!(
        initialized.sends.inner.owned.as_slice(),
        [OwnedScheduleAt::new(TimerGeneration(0), at)]
    );
    assert_eq!(
        initialized.sends.inner.inner.reports.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let stale = delegate_transition(
        &mut journal,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            1,
        )))),
    )
    .expect("a different generation is discharged");
    assert!(stale.sends.inner.owned.is_empty());
    assert!(stale.sends.inner.inner.reports.is_empty());
    assert!(stale.creates.is_empty());
    assert!(matches!(stale.become_, Step::Continue));
    assert_eq!(journal.base().entries, 1);
    let accepted = delegate_transition(
        &mut journal,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        )))),
    )
    .expect("the requested deadline is accepted");
    assert!(accepted.sends.inner.owned.is_empty());
    assert!(accepted.sends.inner.inner.reports.is_empty());
    assert!(accepted.creates.is_empty());
    assert!(matches!(accepted.become_, Step::Continue));
    assert_eq!(journal.base().entries, 11);
    let duplicate = delegate_transition(
        &mut journal,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        )))),
    )
    .expect("the consumed deadline cannot run twice");
    assert!(duplicate.sends.inner.owned.is_empty());
    assert!(duplicate.sends.inner.inner.reports.is_empty());
    assert!(duplicate.creates.is_empty());
    assert!(matches!(duplicate.become_, Step::Continue));
    assert_eq!(journal.base().entries, 11);

    let mut journal = Deadline::new(
        StopOnShutdown::new(Journal { entries: 1 }),
        Some(at),
        deadline_with_shutdown,
    );
    let initialized = initialize(&mut journal).expect("deadline outside shutdown initializes");
    assert_eq!(
        initialized.sends.owned.as_slice(),
        [OwnedScheduleAt::new(TimerGeneration(0), at)]
    );
    assert_eq!(
        initialized.sends.inner.inner.reports.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let accepted = delegate_transition(
        &mut journal,
        EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(0))),
    )
    .expect("the outer deadline retains its stop verdict");
    assert!(accepted.sends.owned.is_empty());
    assert!(accepted.sends.inner.inner.reports.is_empty());
    assert!(accepted.creates.is_empty());
    assert!(matches!(accepted.become_, Step::Stop(Stopped)));
}

#[test]
fn receive_timeout_rearms_only_for_user_activity_and_keeps_both_timer_lanes() {
    let at = Instant::now();
    let mut journal = ReceiveTimeout::new(
        Deadline::new(Journal { entries: 1 }, Some(at), deadline),
        Duration::from_secs(2),
        |journal| Actions::cont().with_send(ReportToParent::new(journal.base().entries)),
    );
    let initialized = initialize(&mut journal).expect("both independent timers initialize");
    assert_eq!(
        initialized.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(
            TimerGeneration(0),
            Duration::from_secs(2)
        )]
    );
    assert_eq!(
        initialized.sends.inner.owned.as_slice(),
        [OwnedScheduleAt::new(TimerGeneration(0), at)]
    );
    assert_eq!(
        initialized.sends.inner.inner.reports.as_slice(),
        [ReportToParent::new(1)]
    );
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let service = delegate_transition(
        &mut journal,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        )))),
    )
    .expect("the inner deadline is a service event, not user activity");
    assert!(service.sends.owned.is_empty());
    assert!(service.sends.inner.owned.is_empty());
    assert!(service.sends.inner.inner.reports.is_empty());
    assert!(service.creates.is_empty());
    assert!(matches!(service.become_, Step::Continue));
    assert_eq!(journal.base().entries, 11);
    let accepted = delegate_transition(
        &mut journal,
        EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(0))),
    )
    .expect("service traffic did not replace the original inactivity generation");
    assert!(accepted.sends.owned.is_empty());
    assert!(accepted.sends.inner.owned.is_empty());
    assert_eq!(
        accepted.sends.inner.inner.reports.as_slice(),
        [ReportToParent::new(11)]
    );
    assert!(accepted.creates.is_empty());
    assert!(matches!(accepted.become_, Step::Continue));
    let duplicate = delegate_transition(
        &mut journal,
        EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(0))),
    )
    .expect("one idle period produces one report");
    assert!(duplicate.sends.owned.is_empty());
    assert!(duplicate.sends.inner.owned.is_empty());
    assert!(duplicate.sends.inner.inner.reports.is_empty());
    assert!(duplicate.creates.is_empty());
    assert!(matches!(duplicate.become_, Step::Continue));
    let activity = delegate_transition(
        &mut journal,
        EventLayer::Inner(EventLayer::Inner(User::user(MailAddr(17), 2))),
    )
    .expect("a genuine user communication rearms inactivity");
    assert_eq!(
        activity.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(
            TimerGeneration(1),
            Duration::from_secs(2)
        )]
    );
    assert!(activity.sends.inner.owned.is_empty());
    assert_eq!(
        activity.sends.inner.inner.reports.as_slice(),
        [ReportToParent::new(13)]
    );
    assert!(activity.creates.is_empty());
    assert!(matches!(activity.become_, Step::Continue));
    let stale = delegate_transition(
        &mut journal,
        EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(0))),
    )
    .expect("the prior idle generation stays consumed after activity");
    assert!(stale.sends.owned.is_empty());
    assert!(stale.sends.inner.owned.is_empty());
    assert!(stale.sends.inner.inner.reports.is_empty());
    assert!(stale.creates.is_empty());
    assert!(matches!(stale.become_, Step::Continue));
    let next = delegate_transition(
        &mut journal,
        EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(1))),
    )
    .expect("the actual rearmed generation produces its own report");
    assert!(next.sends.owned.is_empty());
    assert!(next.sends.inner.owned.is_empty());
    assert_eq!(
        next.sends.inner.inner.reports.as_slice(),
        [ReportToParent::new(13)]
    );
    assert!(next.creates.is_empty());
    assert!(matches!(next.become_, Step::Continue));
}
