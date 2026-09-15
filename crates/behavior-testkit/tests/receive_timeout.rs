use core::future::Future;
use std::time::Duration;

use behavior_actors::{
    Activate, ScheduleAfter, ScheduleAt, TimerElapsed, TimerGeneration, TimerId, TimerScheduled,
};
use behavior_core::EventLayer;
use behavior_core::{
    Acted, ActionItem, Actions, Behavior, Births, CreateChild, CreationKind, CreationSequence,
    Creations, Delivery, Here, Inside, InterpretItem, InterpretSends, Interpretation,
    ItemSettlement, MailAddr, Never, NoBirths, Recipient, Step, User, UserEvent,
};
use behavior_testkit::model::InactivityModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failed;

struct Child;

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Never>, births = behavior_core::NoBirths, error = Never)]
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
    child_ids: CreationSequence,
}

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = Births<ChildBehavior>, error = Failed)]
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
        let child_id = self
            .child_ids
            .issue()
            .expect("the test creator has another child ID");
        let next = match message {
            0 => Step::Stop(behavior_core::Stopped),
            _ => Step::Continue,
        };
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(MailAddr(90)), message)],
            Creations::one(CreateChild::birth(child_id, Child)),
            next,
        ))
    }
}

type SubjectBehavior = Subject;

fn assert_one_child_birth(creations: &Creations<CreateChild<MailAddr, ChildBehavior>>) {
    let mut children = creations.iter();
    assert_eq!(
        children.next().map(CreateChild::kind),
        Some(CreationKind::Birth)
    );
    assert!(children.next().is_none());
}

fn on_timeout(
    inner: &mut SubjectBehavior,
) -> Actions<
    MailAddr,
    Never,
    Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
    Births<ChildBehavior>,
> {
    let child_id = inner
        .child_ids
        .issue()
        .expect("the test creator has another child ID");
    Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(91)), 99)],
        creates: Creations::one(CreateChild::birth(child_id, Child)),
        become_: Step::Continue,
    }
}

#[tokio::test]
async fn initialization_and_successful_user_turns_arm_after_preserving_actions() {
    let after = Duration::from_secs(5);
    let behavior = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
        behavior_actors::TimerId(0),
        after,
        on_timeout,
    );

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
    assert_one_child_birth(&first.creates);
    assert_eq!(first.become_, Step::Continue);
    assert_eq!(first.sends.owned[0].generation, TimerGeneration(1));

    let second = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 2)))
        .unwrap();
    assert_eq!(second.sends.owned[0].generation, TimerGeneration(2));
}

#[tokio::test]
async fn matching_delivery_consumes_once_and_reaction_preserves_full_actions() {
    let behavior = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
        behavior_actors::TimerId(0),
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
    assert_one_child_birth(&fired.creates);
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
async fn errors_and_terminal_user_turns_do_not_rearm() {
    let failing = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
        behavior_actors::TimerId(0),
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

    let terminal = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
        behavior_actors::TimerId(0),
        Duration::from_secs(1),
        on_timeout,
    );
    let initialized = terminal.initialize().unwrap();
    let mut terminal = initialized.behavior;
    let stopped = terminal
        .transition(EventLayer::Inner(User::user(MailAddr(1), 0)))
        .unwrap();
    assert_eq!(stopped.become_, Step::Stop(behavior_core::Stopped));
    assert!(stopped.sends.owned.is_empty());
    assert_eq!(stopped.sends.inner[0].message, 0);
    assert_one_child_birth(&stopped.creates);

    let formerly_live = terminal
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(formerly_live.sends.inner.is_empty());
    assert!(formerly_live.sends.owned.is_empty());
}

fn inner_at(_inner: &mut SubjectBehavior) -> behavior_core::Become {
    Step::Continue
}

type TimedInner = behavior_actors::Deadline<SubjectBehavior>;

fn outer_timeout(
    _inner: &mut TimedInner,
) -> Actions<
    MailAddr,
    Never,
    behavior_core::SendLayer<
        behavior_core::InterpreterRequests<behavior_actors::ScheduleAt>,
        <SubjectBehavior as Behavior>::Sends,
    >,
    Births<ChildBehavior>,
> {
    Actions::cont()
}

type TimerCompositionEvent =
    behavior_actors::TimedEvent<behavior_actors::TimedEvent<User<MailAddr, u8>>>;

#[derive(Debug, PartialEq, Eq)]
enum TimerScheduleAcceptance {
    Absolute(ScheduleAt),
    Relative(ScheduleAfter),
}

struct TimerCompositionRuntime {
    accepted_deliveries: Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
    accepted_schedules: Vec<TimerScheduleAcceptance>,
}

impl
    InterpretItem<
        Delivery<behavior_testkit::TestRecipient<u8>>,
        TimerCompositionEvent,
        Inside<Inside<Here>>,
    > for TimerCompositionRuntime
{
    fn interpret_item(
        &mut self,
        delivery: Delivery<behavior_testkit::TestRecipient<u8>>,
    ) -> impl Future<
        Output = ItemSettlement<
            Delivery<behavior_testkit::TestRecipient<u8>>,
            <Delivery<behavior_testkit::TestRecipient<u8>> as ActionItem>::Accepted,
            <Delivery<behavior_testkit::TestRecipient<u8>> as ActionItem>::Rejection,
            <Delivery<behavior_testkit::TestRecipient<u8>> as ActionItem>::Prerequisite,
        >,
    > + Send {
        async move {
            self.accepted_deliveries.push(delivery);
            ItemSettlement::Accepted(())
        }
    }
}

impl InterpretItem<ScheduleAt, TimerCompositionEvent, Inside<Here>> for TimerCompositionRuntime {
    fn interpret_item(
        &mut self,
        schedule: ScheduleAt,
    ) -> impl Future<
        Output = ItemSettlement<
            ScheduleAt,
            <ScheduleAt as ActionItem>::Accepted,
            <ScheduleAt as ActionItem>::Rejection,
            <ScheduleAt as ActionItem>::Prerequisite,
        >,
    > + Send {
        async move {
            self.accepted_schedules
                .push(TimerScheduleAcceptance::Absolute(schedule));
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })
        }
    }
}

impl InterpretItem<ScheduleAfter, TimerCompositionEvent, Here> for TimerCompositionRuntime {
    fn interpret_item(
        &mut self,
        schedule: ScheduleAfter,
    ) -> impl Future<
        Output = ItemSettlement<
            ScheduleAfter,
            <ScheduleAfter as ActionItem>::Accepted,
            <ScheduleAfter as ActionItem>::Rejection,
            <ScheduleAfter as ActionItem>::Prerequisite,
        >,
    > + Send {
        async move {
            self.accepted_schedules
                .push(TimerScheduleAcceptance::Relative(schedule));
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })
        }
    }
}

#[tokio::test]
async fn nested_timer_service_events_never_reset_receive_inactivity() {
    let due = std::time::Instant::now() + Duration::from_secs(2);
    let behavior = behavior_actors::ReceiveTimeout::new(
        behavior_actors::Deadline::new(
            Subject::default(),
            behavior_actors::TimerId(0),
            Some(due),
            inner_at,
        ),
        behavior_actors::TimerId(1),
        Duration::from_secs(1),
        outer_timeout,
    );
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    let mut runtime = TimerCompositionRuntime {
        accepted_deliveries: Vec::new(),
        accepted_schedules: Vec::new(),
    };
    let interpreted = <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(
        initial.sends,
        &mut runtime,
    )
    .await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(runtime.accepted_deliveries.is_empty());
    assert_eq!(
        runtime.accepted_schedules,
        [
            TimerScheduleAcceptance::Absolute(behavior_actors::ScheduleAt::new(
                TimerId(0),
                TimerGeneration(0),
                due,
            )),
            TimerScheduleAcceptance::Relative(behavior_actors::ScheduleAfter::new(
                TimerId(1),
                TimerGeneration(0),
                Duration::from_secs(1),
            )),
        ]
    );

    let accepted = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    let interpreted = <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(
        accepted.sends,
        &mut runtime,
    )
    .await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(runtime.accepted_deliveries.is_empty());
    assert_eq!(runtime.accepted_schedules.len(), 2);

    let stale = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    let interpreted =
        <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(stale.sends, &mut runtime)
            .await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(runtime.accepted_deliveries.is_empty());
    assert_eq!(runtime.accepted_schedules.len(), 2);

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
    let behavior = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
        behavior_actors::TimerId(0),
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
    assert_eq!(terminal.become_, Step::Stop(behavior_core::Stopped));
    assert!(terminal.sends.owned.is_empty());
    assert_eq!(model.no_activity(), Some(2));
}

struct StopsAtInitialization;

impl behavior_core::Protocol for StopsAtInitialization {
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
        _: behavior_core::InitializationTurn,
    ) -> Acted<MailAddr, Never, Self::Sends, NoBirths, Never> {
        Ok(Actions::stop())
    }

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        _event: Self::Event,
    ) -> Acted<MailAddr, Never, Self::Sends, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

fn stopped_at_reaction(_inner: &mut StopsAtInitialization) -> behavior_core::Become {
    Step::Continue
}

#[tokio::test]
async fn terminal_initialization_consumes_absolute_timer_state() {
    let due = std::time::Instant::now() + Duration::from_secs(1);
    let behavior = behavior_actors::Deadline::new(
        StopsAtInitialization,
        TimerId(0),
        Some(due),
        stopped_at_reaction,
    );
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.become_, Step::Stop(behavior_core::Stopped));
    assert!(initial.sends.owned.is_empty());

    let after_stop = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(after_stop.become_, Step::Continue);
}
