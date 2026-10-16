use core::future::{Future, poll_fn};
use core::pin::pin;
use core::task::Poll;
use std::time::Duration;

use behavior_actors::{
    Activate, OwnedScheduleAfter, OwnedScheduleAt, OwnedTimerElapsed, OwnedTimerScheduled,
    TimerGeneration,
};
use behavior_core::EventLayer;
use behavior_core::{
    Acted, ActionItem, Actions, Behavior, Births, CreateChild, CreationKind, CreationSequence,
    Creations, Delivery, Here, Inside, InterpretItem, InterpretSends, Interpretation,
    InterpretationProgress, ItemSettlement, MailAddr, Never, NoBirths, Recipient, RetirementBirths,
    Step, User, UserEvent,
};
use behavior_testkit::model::InactivityModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Failed;

struct Child;

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Never>, error = Never)]
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

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = Births<ChildBehavior>, creation_settlements = retain_for_retirement, error = Failed)]
impl Subject {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        RetirementBirths<ChildBehavior>,
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
    let first_child_kind = children.next().map(CreateChild::kind);
    assert_eq!(first_child_kind, Some(CreationKind::Birth));
    let remaining_children = children.next();
    assert!(remaining_children.is_none());
}

fn on_timeout(
    inner: &mut SubjectBehavior,
) -> Actions<
    MailAddr,
    Never,
    Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
    RetirementBirths<ChildBehavior>,
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
    let behavior = behavior_actors::ReceiveTimeout::new(Subject::default(), after, on_timeout);

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert!(initial.sends.inner.is_empty());
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    assert_eq!(
        initial.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(0), after)]
    );

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
        Duration::from_secs(1),
        on_timeout,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let stale = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            1,
        ))))
        .unwrap();
    assert!(stale.sends.inner.is_empty());
    assert!(stale.sends.owned.is_empty());

    let fired = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert_eq!(fired.sends.inner[0].message, 99);
    assert_one_child_birth(&fired.creates);
    assert_eq!(fired.become_, Step::Continue);
    assert!(fired.sends.owned.is_empty());

    let duplicate = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
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
        Duration::from_secs(1),
        on_timeout,
    );
    let initialized = failing.initialize().unwrap();
    let mut failing = initialized.behavior;
    let failed = failing.transition(EventLayer::Inner(User::user(MailAddr(1), 7)));
    assert!(matches!(failed, Err(Failed)));
    let still_live = failing
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert_eq!(still_live.sends.inner[0].message, 99);

    let terminal = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
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
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert!(formerly_live.sends.inner.is_empty());
    assert!(formerly_live.sends.owned.is_empty());
}

fn inner_at(inner: &mut SubjectBehavior) -> behavior_core::Become {
    inner.accepted.push(88);
    Step::Continue
}

type TimedInner = behavior_actors::Deadline<SubjectBehavior>;

fn outer_timeout(
    _inner: &mut TimedInner,
) -> Actions<
    MailAddr,
    Never,
    behavior_core::SendLayer<
        behavior_core::InterpreterRequests<behavior_actors::OwnedScheduleAt>,
        <SubjectBehavior as Behavior>::Sends,
    >,
    RetirementBirths<ChildBehavior>,
> {
    Actions::cont().with_send(Delivery::new(Recipient::global(MailAddr(91)), 98))
}

type TimerCompositionEvent =
    EventLayer<OwnedTimerElapsed, EventLayer<OwnedTimerElapsed, User<MailAddr, u8>>>;

#[derive(Debug, PartialEq, Eq)]
enum TimerScheduleAcceptance {
    Absolute(OwnedScheduleAt),
    Relative(OwnedScheduleAfter),
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
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Delivery<behavior_testkit::TestRecipient<u8>>>,
        received: &'a mut Option<
            <Delivery<behavior_testkit::TestRecipient<u8>> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Delivery<behavior_testkit::TestRecipient<u8>>: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(delivery) = input.take() else {
                return;
            };
            let producer = {
                async move {
                    self.accepted_deliveries.push(delivery);
                    ItemSettlement::Accepted(())
                }
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

impl InterpretItem<OwnedScheduleAt, TimerCompositionEvent, Inside<Here>>
    for TimerCompositionRuntime
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<OwnedScheduleAt>,
        received: &'a mut Option<<OwnedScheduleAt as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        OwnedScheduleAt: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(schedule) = input.take() else {
                return;
            };
            let producer = {
                async move {
                    self.accepted_schedules
                        .push(TimerScheduleAcceptance::Absolute(schedule));
                    ItemSettlement::Accepted(OwnedTimerScheduled {
                        id: schedule.id,
                        generation: schedule.generation,
                    })
                }
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

impl InterpretItem<OwnedScheduleAfter, TimerCompositionEvent, Here> for TimerCompositionRuntime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<OwnedScheduleAfter>,
        received: &'a mut Option<<OwnedScheduleAfter as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        OwnedScheduleAfter: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(schedule) = input.take() else {
                return;
            };
            let producer = {
                async move {
                    self.accepted_schedules
                        .push(TimerScheduleAcceptance::Relative(schedule));
                    ItemSettlement::Accepted(OwnedTimerScheduled {
                        id: schedule.id,
                        generation: schedule.generation,
                    })
                }
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

#[tokio::test]
async fn nested_timer_service_events_never_reset_receive_inactivity() {
    let due = std::time::Instant::now() + Duration::from_secs(2);
    let behavior = behavior_actors::ReceiveTimeout::new(
        behavior_actors::Deadline::new(Subject::default(), Some(due), inner_at),
        Duration::from_secs(1),
        outer_timeout,
    );
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    let mut behavior = initialized.behavior;
    let mut runtime = TimerCompositionRuntime {
        accepted_deliveries: Vec::new(),
        accepted_schedules: Vec::new(),
    };
    let interpreted = {
        let mut progress = Some(InterpretationProgress::Original(initial.sends));
        <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the recorded effect product must return its exact complete settlement");
        };
        settlement
    };
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(runtime.accepted_deliveries.is_empty());
    assert_eq!(
        runtime.accepted_schedules,
        [
            TimerScheduleAcceptance::Absolute(behavior_actors::OwnedScheduleAt::new(
                TimerGeneration(0),
                due,
            )),
            TimerScheduleAcceptance::Relative(behavior_actors::OwnedScheduleAfter::new(
                TimerGeneration(0),
                Duration::from_secs(1),
            )),
        ]
    );

    let accepted = behavior
        .transition(EventLayer::Inner(EventLayer::Owned(
            OwnedTimerElapsed::new(TimerGeneration(0)),
        )))
        .unwrap();
    assert_eq!(behavior.base().accepted, [88]);
    assert!(accepted.sends.owned.is_empty());
    assert!(accepted.sends.inner.owned.is_empty());
    assert!(accepted.sends.inner.inner.is_empty());
    assert!(accepted.creates.is_empty());
    assert_eq!(accepted.become_, Step::Continue);
    let interpreted = {
        let mut progress = Some(InterpretationProgress::Original(accepted.sends));
        <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the recorded effect product must return its exact complete settlement");
        };
        settlement
    };
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(runtime.accepted_deliveries.is_empty());
    assert_eq!(runtime.accepted_schedules.len(), 2);

    let stale = behavior
        .transition(EventLayer::Inner(EventLayer::Owned(
            OwnedTimerElapsed::new(TimerGeneration(0)),
        )))
        .unwrap();
    assert_eq!(behavior.base().accepted, [88]);
    assert!(stale.sends.owned.is_empty());
    assert!(stale.sends.inner.owned.is_empty());
    assert!(stale.sends.inner.inner.is_empty());
    assert!(stale.creates.is_empty());
    assert_eq!(stale.become_, Step::Continue);
    let interpreted = {
        let mut progress = Some(InterpretationProgress::Original(stale.sends));
        <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the recorded effect product must return its exact complete settlement");
        };
        settlement
    };
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(runtime.accepted_deliveries.is_empty());
    assert_eq!(runtime.accepted_schedules.len(), 2);

    let outer = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert_eq!(outer.become_, Step::Continue);
    assert!(outer.sends.owned.is_empty());
    assert!(outer.sends.inner.owned.is_empty());
    assert_eq!(outer.sends.inner.inner.len(), 1);
    assert_eq!(outer.sends.inner.inner[0].message, 98);
    assert_eq!(outer.sends.inner.inner[0].to.address(), MailAddr(91));
    assert!(outer.creates.is_empty());
    assert_eq!(behavior.base().accepted, [88]);
    let mut progress = Some(InterpretationProgress::Original(outer.sends));
    <_ as InterpretSends<_, TimerCompositionEvent, Here>>::interpret(&mut progress, &mut runtime)
        .await;
    assert!(matches!(
        progress,
        Some(InterpretationProgress::Completed(Interpretation::Complete(
            _
        )))
    ));
    assert_eq!(runtime.accepted_schedules.len(), 2);
    assert_eq!(runtime.accepted_deliveries.len(), 1);
    assert_eq!(runtime.accepted_deliveries[0].message, 98);
    assert_eq!(runtime.accepted_deliveries[0].to.address(), MailAddr(91));

    let duplicate = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert!(duplicate.sends.owned.is_empty());
    assert!(duplicate.sends.inner.owned.is_empty());
    assert!(duplicate.sends.inner.inner.is_empty());
    assert!(duplicate.creates.is_empty());
    assert_eq!(duplicate.become_, Step::Continue);
    assert_eq!(behavior.base().accepted, [88]);
}

#[tokio::test]
async fn accepted_stale_timeout_error_and_terminal_turns_match_independent_model() {
    let mut model = InactivityModel::new();
    let behavior = behavior_actors::ReceiveTimeout::new(
        Subject::default(),
        Duration::from_secs(1),
        on_timeout,
    );

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    let initial_token = model.initialize();
    assert_eq!(
        initial.sends.owned[0].generation,
        TimerGeneration(initial_token)
    );

    let accepted = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 1)))
        .unwrap();
    let active_token = model.activity().unwrap();
    assert_eq!(
        accepted.sends.owned[0].generation,
        TimerGeneration(active_token)
    );

    let stale_admission = model.notification(0);
    assert_eq!(stale_admission, None);
    assert_eq!(model.no_activity(), Some(1));
    let stale = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert!(stale.sends.inner.is_empty());

    let current_admission = model.notification(1);
    assert_eq!(current_admission, Some(1));
    let timeout = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            1,
        ))))
        .unwrap();
    assert_eq!(timeout.sends.inner[0].message, 99);
    assert!(timeout.sends.owned.is_empty());

    let duplicate_admission = model.notification(1);
    assert_eq!(duplicate_admission, None);
    let duplicate = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            1,
        ))))
        .unwrap();
    assert!(duplicate.sends.inner.is_empty());

    assert_eq!(model.no_activity(), None);
    let failed = behavior.transition(EventLayer::Inner(User::user(MailAddr(1), 7)));
    assert!(matches!(failed, Err(Failed)));

    let accepted = behavior
        .transition(EventLayer::Inner(User::user(MailAddr(1), 2)))
        .unwrap();
    let next_token = model.activity().unwrap();
    assert_eq!(
        accepted.sends.owned[0].generation,
        TimerGeneration(next_token)
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
    let behavior =
        behavior_actors::Deadline::new(StopsAtInitialization, Some(due), stopped_at_reaction);
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.become_, Step::Stop(behavior_core::Stopped));
    assert!(initial.sends.owned.is_empty());

    let after_stop = behavior
        .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
            0,
        ))))
        .unwrap();
    assert_eq!(after_stop.become_, Step::Continue);
}
