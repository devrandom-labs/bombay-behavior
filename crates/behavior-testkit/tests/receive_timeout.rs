use std::time::Duration;

use behavior::EventLayer;
use behavior::{
    Acted, Actions, Activate, Behavior, Births, Create, Delivery, MailAddr, Never, NoBirths,
    Recipient, Step, TimerElapsed, TimerGeneration, TimerId, User, UserEvent,
};
use behavior_testkit::model::InactivityModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failed;

struct Child;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Never>, births = behavior::NoBirths, error = Never)]
impl Child {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

type ChildBehavior = Child;

#[derive(Default)]
struct Subject {
    accepted: Vec<u8>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = Births<ChildBehavior>, error = Failed)]
impl Subject {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        Births<ChildBehavior>,
        Failed,
    > {
        if message == 7 {
            return Err(Failed);
        }
        self.accepted.push(message);
        let mut actions: Actions<
            MailAddr,
            Never,
            Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
            Births<ChildBehavior>,
        > = Actions::cont();
        actions
            .sends
            .push(Delivery::new(Recipient::global(MailAddr(90)), message));
        actions
            .creates
            .push(Create::birth(u64::from(message), Child));
        if message == 0 {
            actions.become_ = Step::Stop(behavior::Stopped);
        }
        Ok(actions)
    }
}

type SubjectBehavior = Subject;

fn on_timeout(
    _inner: &mut SubjectBehavior,
) -> Acted<
    MailAddr,
    Never,
    Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
    Births<ChildBehavior>,
    Failed,
> {
    Ok(Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(91)), 99)],
        creates: vec![Create::birth(99, Child)],
        become_: Step::Continue,
    })
}

#[tokio::test]
async fn initialization_and_successful_user_folds_arm_after_preserving_actions() {
    let after = Duration::from_secs(5);
    let behavior =
        behavior::ReceiveTimeout::new(Subject::default(), behavior::TimerId(0), after, on_timeout);

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert!(initial.sends.inner.is_empty());
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    assert_eq!(initial.sends.owned.len(), 1);
    assert_eq!(initial.sends.owned[0].id, TimerId(0));
    assert_eq!(initial.sends.owned[0].generation, TimerGeneration(0));
    assert_eq!(initial.sends.owned[0].after, after);

    let first = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 1)))
        .unwrap();
    assert_eq!(first.sends.inner.len(), 1);
    assert_eq!(first.sends.inner[0].message, 1);
    assert_eq!(first.creates.len(), 1);
    assert_eq!(first.creates[0].nonce, 1);
    assert_eq!(first.become_, Step::Continue);
    assert_eq!(first.sends.owned[0].generation, TimerGeneration(1));

    let second = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 2)))
        .unwrap();
    assert_eq!(second.sends.owned[0].generation, TimerGeneration(2));
}

#[tokio::test]
async fn matching_delivery_consumes_once_and_reaction_preserves_full_actions() {
    let behavior = behavior::ReceiveTimeout::new(
        Subject::default(),
        behavior::TimerId(0),
        Duration::from_secs(1),
        on_timeout,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let stale = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }))
        .unwrap();
    assert!(stale.sends.inner.is_empty());
    assert!(stale.sends.owned.is_empty());

    let fired = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(fired.sends.inner[0].message, 99);
    assert_eq!(fired.creates[0].nonce, 99);
    assert_eq!(fired.become_, Step::Continue);
    assert!(fired.sends.owned.is_empty());

    let duplicate = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(duplicate.sends.inner.is_empty());
    assert!(duplicate.sends.owned.is_empty());

    let rearmed = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 3)))
        .unwrap();
    assert_eq!(rearmed.sends.owned[0].generation, TimerGeneration(1));
}

#[tokio::test]
async fn errors_and_terminal_user_folds_do_not_rearm() {
    let failing = behavior::ReceiveTimeout::new(
        Subject::default(),
        behavior::TimerId(0),
        Duration::from_secs(1),
        on_timeout,
    );
    let initialized = failing.initialize().unwrap();
    let mut failing = initialized.behavior;
    let failed = failing.transition(EventLayer::Inner(User::user(MailAddr(1), 7)));
    assert!(matches!(failed, Err(Failed)));
    let still_live = failing
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(still_live.sends.inner[0].message, 99);

    let terminal = behavior::ReceiveTimeout::new(
        Subject::default(),
        behavior::TimerId(0),
        Duration::from_secs(1),
        on_timeout,
    );
    let initialized = terminal.initialize().unwrap();
    let mut terminal = initialized.behavior;
    let stopped = terminal
        .transition(EventLayer::Inner(User::user(MailAddr(1), 0)))
        .unwrap();
    assert_eq!(stopped.become_, Step::Stop(behavior::Stopped));
    assert!(stopped.sends.owned.is_empty());
    assert_eq!(stopped.sends.inner[0].message, 0);
    assert_eq!(stopped.creates[0].nonce, 0);

    let formerly_live = terminal
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(formerly_live.sends.inner.is_empty());
    assert!(formerly_live.sends.owned.is_empty());
}

fn inner_at(_inner: &mut SubjectBehavior) -> Result<behavior::Become, Failed> {
    Ok(Step::Continue)
}

type TimedInner = behavior::Deadline<SubjectBehavior>;

fn outer_timeout(
    _inner: &mut TimedInner,
) -> Acted<
    MailAddr,
    Never,
    behavior::SendLayer<
        behavior::InterpreterRequests<behavior::ScheduleAt>,
        <SubjectBehavior as Behavior>::Sends,
    >,
    Births<ChildBehavior>,
    Failed,
> {
    Ok(Actions::cont())
}

#[tokio::test]
async fn nested_timer_service_events_never_reset_receive_inactivity() {
    let due = std::time::Instant::now() + Duration::from_secs(2);
    let behavior = behavior::ReceiveTimeout::new(
        behavior::Deadline::new(
            Subject::default(),
            behavior::TimerId(0),
            Some(due),
            inner_at,
        ),
        behavior::TimerId(1),
        Duration::from_secs(1),
        outer_timeout,
    );
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.inner.owned[0].id, TimerId(0));
    assert_eq!(initial.sends.owned[0].id, TimerId(1));

    let accepted = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(accepted.sends.owned.is_empty());

    let stale = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(stale.sends.owned.is_empty());

    let outer = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(1),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(outer.become_, Step::Continue);
}

#[tokio::test]
async fn accepted_stale_timeout_error_and_terminal_turns_match_independent_model() {
    let mut model = InactivityModel::new();
    let behavior = behavior::ReceiveTimeout::new(
        Subject::default(),
        behavior::TimerId(0),
        Duration::from_secs(1),
        on_timeout,
    );

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(
        initial.sends.owned[0].generation,
        TimerGeneration(model.initialize())
    );

    let accepted = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 1)))
        .unwrap();
    assert_eq!(
        accepted.sends.owned[0].generation,
        TimerGeneration(model.activity().unwrap())
    );

    assert!(!model.notification(0));
    let stale = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(stale.sends.inner.is_empty());

    assert!(model.notification(1));
    let timeout = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }))
        .unwrap();
    assert_eq!(timeout.sends.inner[0].message, 99);
    assert!(timeout.sends.owned.is_empty());

    assert!(!model.notification(1));
    let duplicate = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }))
        .unwrap();
    assert!(duplicate.sends.inner.is_empty());

    assert_eq!(model.no_activity(), None);
    let failed = behavior.transition(EventLayer::Inner(User::user(MailAddr(1), 7)));
    assert!(matches!(failed, Err(Failed)));

    let accepted = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 2)))
        .unwrap();
    assert_eq!(
        accepted.sends.owned[0].generation,
        TimerGeneration(model.activity().unwrap())
    );

    let terminal = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 0)))
        .unwrap();
    assert_eq!(terminal.become_, Step::Stop(behavior::Stopped));
    assert!(terminal.sends.owned.is_empty());
    assert_eq!(model.no_activity(), Some(2));
}

struct StopsAtInitialization;

impl behavior::Protocol for StopsAtInitialization {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for StopsAtInitialization {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(
        &mut self,
        _: behavior::InitializationTurn,
    ) -> Acted<MailAddr, Never, Self::Sends, NoBirths, Never> {
        Ok(Actions::stop())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _event: Self::Event,
    ) -> Acted<MailAddr, Never, Self::Sends, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

fn stopped_at_reaction(_inner: &mut StopsAtInitialization) -> Result<behavior::Become, Never> {
    Ok(Step::Continue)
}

#[tokio::test]
async fn terminal_initialization_consumes_absolute_timer_state() {
    let due = std::time::Instant::now() + Duration::from_secs(1);
    let behavior = behavior::Deadline::new(
        StopsAtInitialization,
        TimerId(0),
        Some(due),
        stopped_at_reaction,
    );
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.become_, Step::Stop(behavior::Stopped));
    assert!(initial.sends.owned.is_empty());

    let after_stop = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(after_stop.become_, Step::Continue);
}
