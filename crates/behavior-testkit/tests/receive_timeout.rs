use std::time::Duration;

use behavior::{
    Acted, Actions, At, AtEvent, Base, Behavior, Births, Create, Delivery, Exit, MailAddr, Never,
    NoBirths, ReceiveTimeoutError, ReceiveTimeoutEvent, Recipient, Spec, State, Step, TimerElapsed,
    TimerGeneration, TimerId, User, UserEvent,
};
use behavior_testkit::model::InactivityModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failed;

struct Child;

impl State for Child {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

type ChildBehavior = Base<Child>;

#[derive(Default)]
struct Subject {
    accepted: Vec<u8>,
}

impl State<u8, Births<ChildBehavior>, Failed> for Subject {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Births<ChildBehavior>, Failed> {
        if message == 7 {
            return Err(Failed);
        }
        self.accepted.push(message);
        let mut actions: Actions<
            MailAddr,
            Never,
            Vec<Delivery<MailAddr, u8>>,
            Births<ChildBehavior>,
        > = Actions::cont();
        actions
            .sends
            .push(Delivery::new(Recipient::global(MailAddr(90)), message));
        actions
            .creates
            .push(Create::birth(u64::from(message), Base::new(Child)));
        if message == 0 {
            actions.become_ = Step::Stop(Exit::Normal);
        }
        Ok(actions)
    }
}

type Inner = Base<Subject, u8, Births<ChildBehavior>, Failed>;

fn on_timeout(
    _inner: &mut Inner,
) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Births<ChildBehavior>, Failed> {
    Ok(Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(91)), 99)],
        creates: vec![Create::birth(99, Base::new(Child))],
        become_: Step::Continue,
    })
}

#[tokio::test]
async fn initialization_and_successful_user_folds_arm_after_preserving_actions() {
    let after = Duration::from_secs(5);
    let mut behavior =
        Spec::from_behavior(Base::new(Subject::default())).receive_timeout(after, on_timeout);

    let initial = behavior.init().await.unwrap();
    assert!(initial.sends.inner.is_empty());
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    assert_eq!(initial.sends.own.len(), 1);
    assert_eq!(initial.sends.own[0].id, TimerId(0));
    assert_eq!(initial.sends.own[0].generation, TimerGeneration(0));
    assert_eq!(initial.sends.own[0].after, after);

    let first = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 1)))
        .await
        .unwrap();
    assert_eq!(first.sends.inner.len(), 1);
    assert_eq!(first.sends.inner[0].message, 1);
    assert_eq!(first.creates.len(), 1);
    assert_eq!(first.creates[0].nonce, 1);
    assert_eq!(first.become_, Step::Continue);
    assert_eq!(first.sends.own[0].generation, TimerGeneration(1));

    let second = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 2)))
        .await
        .unwrap();
    assert_eq!(second.sends.own[0].generation, TimerGeneration(2));
}

#[tokio::test]
async fn matching_delivery_consumes_once_and_reaction_preserves_full_actions() {
    let mut behavior = Spec::from_behavior(Base::new(Subject::default()))
        .receive_timeout(Duration::from_secs(1), on_timeout);
    behavior.init().await.unwrap();

    let stale = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }))
        .await
        .unwrap();
    assert!(stale.sends.inner.is_empty());
    assert!(stale.sends.own.is_empty());

    let fired = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert_eq!(fired.sends.inner[0].message, 99);
    assert_eq!(fired.creates[0].nonce, 99);
    assert_eq!(fired.become_, Step::Continue);
    assert!(fired.sends.own.is_empty());

    let duplicate = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(duplicate.sends.inner.is_empty());
    assert!(duplicate.sends.own.is_empty());

    let rearmed = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 3)))
        .await
        .unwrap();
    assert_eq!(rearmed.sends.own[0].generation, TimerGeneration(1));
}

#[tokio::test]
async fn errors_and_terminal_user_folds_do_not_rearm() {
    let mut failing = Spec::from_behavior(Base::new(Subject::default()))
        .receive_timeout(Duration::from_secs(1), on_timeout);
    failing.init().await.unwrap();
    let failed = failing
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 7)))
        .await;
    assert!(matches!(failed, Err(ReceiveTimeoutError::Inner(Failed))));
    let still_live = failing
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert_eq!(still_live.sends.inner[0].message, 99);

    let mut terminal = Spec::from_behavior(Base::new(Subject::default()))
        .receive_timeout(Duration::from_secs(1), on_timeout);
    terminal.init().await.unwrap();
    let stopped = terminal
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 0)))
        .await
        .unwrap();
    assert_eq!(stopped.become_, Step::Stop(Exit::Normal));
    assert!(stopped.sends.own.is_empty());
    assert_eq!(stopped.sends.inner[0].message, 0);
    assert_eq!(stopped.creates[0].nonce, 0);

    let formerly_live = terminal
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(formerly_live.sends.inner.is_empty());
    assert!(formerly_live.sends.own.is_empty());
}

fn inner_at(_inner: &mut Inner) -> Result<behavior::Become<MailAddr>, Failed> {
    Ok(Step::Continue)
}

type TimedInner = behavior::At<Inner>;

fn outer_timeout(
    _inner: &mut TimedInner,
) -> Acted<MailAddr, Never, behavior::AtSends<Inner>, Births<ChildBehavior>, Failed> {
    Ok(Actions::cont())
}

#[tokio::test]
async fn nested_timer_service_events_never_reset_receive_inactivity() {
    let due = tokio::time::Instant::now() + Duration::from_secs(2);
    let mut behavior = Spec::from_behavior(Base::new(Subject::default()))
        .at(Some(due), inner_at)
        .receive_timeout(Duration::from_secs(1), outer_timeout);
    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.sends.inner.own[0].id, TimerId(0));
    assert_eq!(initial.sends.own[0].id, TimerId(1));

    let accepted = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(accepted.sends.own.is_empty());

    let stale = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(stale.sends.own.is_empty());

    let outer = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(1),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert_eq!(outer.become_, Step::Continue);
}

#[tokio::test]
async fn accepted_stale_timeout_error_and_terminal_turns_match_independent_model() {
    let mut model = InactivityModel::new();
    let mut behavior = Spec::from_behavior(Base::new(Subject::default()))
        .receive_timeout(Duration::from_secs(1), on_timeout);

    let initial = behavior.init().await.unwrap();
    assert_eq!(
        initial.sends.own[0].generation,
        TimerGeneration(model.initialize())
    );

    let accepted = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 1)))
        .await
        .unwrap();
    assert_eq!(
        accepted.sends.own[0].generation,
        TimerGeneration(model.activity().unwrap())
    );

    assert!(!model.notification(0));
    let stale = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(stale.sends.inner.is_empty());

    assert!(model.notification(1));
    let timeout = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }))
        .await
        .unwrap();
    assert_eq!(timeout.sends.inner[0].message, 99);
    assert!(timeout.sends.own.is_empty());

    assert!(!model.notification(1));
    let duplicate = behavior
        .step(ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }))
        .await
        .unwrap();
    assert!(duplicate.sends.inner.is_empty());

    assert_eq!(model.no_activity(), None);
    let failed = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 7)))
        .await;
    assert!(matches!(failed, Err(ReceiveTimeoutError::Inner(Failed))));

    let accepted = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 2)))
        .await
        .unwrap();
    assert_eq!(
        accepted.sends.own[0].generation,
        TimerGeneration(model.activity().unwrap())
    );

    let terminal = behavior
        .step(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), 0)))
        .await
        .unwrap();
    assert_eq!(terminal.become_, Step::Stop(Exit::Normal));
    assert!(terminal.sends.own.is_empty());
    assert_eq!(model.no_activity(), Some(2));
}

struct StopsAtInitialization;

impl Behavior for StopsAtInitialization {
    type Addr = MailAddr;
    type Msg = ();
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Delivery<MailAddr, Never>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    async fn init(&mut self) -> Acted<MailAddr, Never, Self::Sends, NoBirths, Never> {
        Ok(Actions::stop(Exit::Normal))
    }

    async fn step(
        &mut self,
        _event: Self::Event,
    ) -> Acted<MailAddr, Never, Self::Sends, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

fn stopped_at_reaction(
    _inner: &mut StopsAtInitialization,
) -> Result<behavior::Become<MailAddr>, Never> {
    Ok(Step::Continue)
}

#[tokio::test]
async fn terminal_initialization_consumes_absolute_timer_state() {
    let due = tokio::time::Instant::now() + Duration::from_secs(1);
    let mut behavior = At::new(
        StopsAtInitialization,
        TimerId(0),
        Some(due),
        stopped_at_reaction,
    );
    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.become_, Step::Stop(Exit::Normal));
    assert!(initial.sends.own.is_empty());

    let after_stop = behavior
        .step(AtEvent::Reached(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert_eq!(after_stop.become_, Step::Continue);
}
