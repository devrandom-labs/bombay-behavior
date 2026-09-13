#![allow(
    clippy::needless_pass_by_value,
    clippy::no_effect_underscore_binding,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    reason = "fixture methods intentionally match the fallible behavior macro contract"
)]

use behavior_actors as behavior;
use core::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use behavior::{
    Acted, ActionItem, Actions, Activate, Become, Behavior, BehaviorBase, Births, Crash,
    CreateChild, CreationKind, CreationSequence, Creations, Delivery, EventLayer, Exit, Here,
    InjectEvent, Inside, InterpretItem, InterpretSends, Interpretation, InterpreterRequests,
    ItemSettlement, LogicalDeliveryProtocols, Machine, MailAddr, Move, Never, NoBirthProtocols,
    NoBirths, ObserveChild, PeerStopped, Recipient, ScheduleAt, SendEffects, ShutdownRequested,
    StashRoute, Step, TimerElapsed, TimerGeneration, TimerId, TimerScheduled, User, UserEvent,
    Watch, stop_on_abnormal_death,
};
use std::time::Instant;

macro_rules! assert_no_vec_effects {
    ($actions:expr, $become:pat_param) => {{
        let actions = &$actions;
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(&actions.become_, $become));
    }};
}

struct Quiet;

struct BehaviorSends {
    deliveries: Vec<Delivery<Quiet>>,
    markers: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
struct Rejected(u64);

struct DelegatingCounter {
    transitions: usize,
    creations: CreationSequence,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<Quiet>>, births = Births<Quiet>, error = Rejected)]
impl DelegatingCounter {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Quiet>>, Births<Quiet>, Rejected> {
        self.transitions += 1;
        if message == 0 {
            return Err(Rejected(message));
        }
        let creation = self.creations.issue().ok_or(Rejected(message))?;
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), message)],
            Creations::one(CreateChild::birth(creation, Quiet)),
            Step::Stop(behavior::Stopped),
        ))
    }
}

struct ExplicitInitialization {
    received: Vec<u64>,
    creations: CreationSequence,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = BehaviorSends, births = Births<Quiet>, error = Never)]
impl ExplicitInitialization {
    fn init(&mut self) -> Acted<MailAddr, Never, BehaviorSends, Births<Quiet>, Never> {
        self.received.push(1);
        let mut sends = BehaviorSends::empty();
        sends.markers.push(7);
        let creation = self.creations.issue().expect("fixture has one creation ID");
        Ok(Actions::new(
            sends,
            Creations::one(CreateChild::birth(creation, Quiet)),
            Step::Continue,
        ))
    }

    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, BehaviorSends, Births<Quiet>, Never> {
        self.received.push(message);
        Ok(Actions::new(
            BehaviorSends {
                deliveries: vec![Delivery::new(Recipient::global(MailAddr(9)), message)],
                markers: Vec::new(),
            },
            Creations::empty(),
            Step::Stop(behavior::Stopped),
        ))
    }
}

struct InitializationCounter(u8);

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<U8Sink>>, births = NoBirths, error = Never)]
impl InitializationCounter {
    fn init(&mut self) -> Acted<MailAddr, Never, Vec<Delivery<U8Sink>>, NoBirths, Never> {
        self.0 += 1;
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(MailAddr(4)), self.0)],
            Creations::empty(),
            Step::Continue,
        ))
    }

    fn receive(
        &mut self,
        _from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<U8Sink>>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

impl SendEffects for BehaviorSends {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            markers: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.markers.append(&mut other.markers);
    }
}

impl LogicalDeliveryProtocols for BehaviorSends {
    type Protocols = behavior::BirthProtocol<Quiet, NoBirthProtocols>;
}

impl<Event> behavior::SendsFor<Event> for BehaviorSends {}

struct U8Sink;

impl behavior::Protocol for U8Sink {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for U8Sink {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn requires_no_births<B: Behavior<Birth = NoBirths>>(_behavior: &B) {}

#[test]
fn deliveries_and_interpreter_requests_have_disjoint_static_dispatch() {
    trait RouteSends<A: behavior::Address> {}

    impl<P> RouteSends<MailAddr> for Vec<Delivery<P>> where P: behavior::Protocol<Addr = MailAddr> {}
    impl<P> RouteSends<MailAddr> for InterpreterRequests<ObserveChild<P, behavior::ChildHead>> where
        P: behavior::Protocol<Addr = MailAddr>
    {
    }

    fn requires_route_sends<A: behavior::Address, S: RouteSends<A>>() {}

    requires_route_sends::<MailAddr, Vec<Delivery<Quiet>>>();
    requires_route_sends::<MailAddr, InterpreterRequests<ObserveChild<U8Sink, behavior::ChildHead>>>(
    );
}

fn requires_births<B, C>(_behavior: &B)
where
    B: Behavior<Birth = Births<C>>,
{
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = NoBirths, error = Never)]
impl Quiet {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

struct ShutdownParent(CreationSequence);

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<Quiet>>, births = Births<Quiet>, error = Never)]
impl ShutdownParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Quiet>>, Births<Quiet>, Never> {
        let creation = self.0.issue().expect("fixture has one creation ID");
        Ok(Actions::create(Creations::one(CreateChild::birth(
            creation, Quiet,
        ))))
    }
}

fn finalize_parent(
    behavior: &mut ShutdownParent,
    _request: ShutdownRequested,
) -> Actions<MailAddr, Never, Vec<Delivery<Quiet>>, Births<Quiet>> {
    let creation = behavior
        .0
        .issue()
        .expect("fixture has one final creation ID");
    Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(9)), 42)],
        creates: Creations::one(CreateChild::birth(creation, Quiet)),
        become_: Step::Continue,
    }
}

#[test]
fn actions_expose_the_typed_actor_transition_effects() {
    let mut actions: Actions<MailAddr, Never, Vec<Delivery<Quiet>>, NoBirths> = Actions::cont();
    actions
        .sends
        .push(Delivery::new(Recipient::global(MailAddr(9)), 42));

    assert_eq!(actions.sends[0].to.address(), MailAddr(9));
    assert_eq!(actions.sends[0].message, 42);
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
}

#[test]
fn active_behavior_runs_one_transition_and_preserves_actions() {
    let initialized = DelegatingCounter {
        transitions: 0,
        creations: CreationSequence::new(),
    }
    .initialize()
    .unwrap();
    let mut behavior = initialized.behavior;
    let actions = behavior.transition(User::new(MailAddr(7), 11)).unwrap();

    assert_eq!(behavior.base().transitions, 1);
    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].to.address(), MailAddr(7));
    assert_eq!(actions.sends[0].message, 11);
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(
        actions.creates.iter().next().unwrap().kind(),
        CreationKind::Birth
    );
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));

    assert!(matches!(
        behavior.transition(User::new(MailAddr(7), 0)),
        Err(Rejected(0))
    ));
    assert_eq!(behavior.base().transitions, 2);
}

#[test]
fn direct_behavior_preserves_explicit_initialization_and_transition_actions() {
    let behavior = ExplicitInitialization {
        received: Vec::new(),
        creations: CreationSequence::new(),
    };

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.markers, [7]);
    assert_eq!(initial.creates.len(), 1);
    assert!(matches!(initial.become_, Step::Continue));

    let transitioned = behavior.receive(MailAddr(3), 5).unwrap();
    assert_eq!(transitioned.sends.deliveries[0].message, 5);
    assert!(transitioned.creates.is_empty());
    assert!(matches!(
        transitioned.become_,
        Step::Stop(behavior::Stopped)
    ));
    assert_eq!(behavior.base().received, [1, 5]);
}

#[test]
fn custom_send_product_owner_projects_delivery_without_classifying_custom_markers() {
    type Actual = <ExplicitInitialization as behavior::LogicalHostRequirements>::LogicalHosts;
    type Expected = behavior::BirthProtocol<Quiet, NoBirthProtocols>;

    trait Same<T> {}
    impl<T> Same<T> for T {}
    fn exact<T: Same<Expected>, Expected>() {}

    exact::<Actual, Expected>();
}

#[tokio::test]
async fn direct_behavior_composes_with_existing_wrappers_and_init_order() {
    type InitializationEvent = behavior::ShutdownEvent<behavior::DeadlineEvent<User<MailAddr, u8>>>;

    #[derive(PartialEq, Eq)]
    enum AcceptedInitializationRequest {
        Message(Delivery<U8Sink>),
        AbsoluteTimer(ScheduleAt),
    }

    struct InitializationInterpreter {
        accepted: Vec<AcceptedInitializationRequest>,
    }

    impl InterpretItem<Delivery<U8Sink>, InitializationEvent, Inside<Inside<Here>>>
        for InitializationInterpreter
    {
        fn interpret_item(
            &mut self,
            request: Delivery<U8Sink>,
        ) -> impl Future<
            Output = ItemSettlement<
                Delivery<U8Sink>,
                <Delivery<U8Sink> as ActionItem>::Accepted,
                <Delivery<U8Sink> as ActionItem>::Rejection,
                <Delivery<U8Sink> as ActionItem>::Prerequisite,
            >,
        > + Send {
            async move {
                self.accepted
                    .push(AcceptedInitializationRequest::Message(request));
                ItemSettlement::Accepted(())
            }
        }
    }

    impl InterpretItem<ScheduleAt, InitializationEvent, Inside<Here>> for InitializationInterpreter {
        fn interpret_item(
            &mut self,
            request: ScheduleAt,
        ) -> impl Future<
            Output = ItemSettlement<
                ScheduleAt,
                <ScheduleAt as ActionItem>::Accepted,
                <ScheduleAt as ActionItem>::Rejection,
                <ScheduleAt as ActionItem>::Prerequisite,
            >,
        > + Send {
            let scheduled = TimerScheduled {
                id: request.id,
                generation: request.generation,
            };
            async move {
                self.accepted
                    .push(AcceptedInitializationRequest::AbsoluteTimer(request));
                ItemSettlement::Accepted(scheduled)
            }
        }
    }

    let due = Instant::now();
    let behavior = behavior_actors::StopOnShutdown::new(behavior_actors::Deadline::new(
        InitializationCounter(0),
        TimerId(0),
        Some(due),
        |_| Step::Continue,
    ));

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    let mut runtime = InitializationInterpreter {
        accepted: Vec::new(),
    };
    let interpreted =
        <_ as InterpretSends<_, InitializationEvent, Here>>::interpret(initial.sends, &mut runtime)
            .await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert!(
        runtime.accepted
            == [
                AcceptedInitializationRequest::Message(Delivery::new(
                    Recipient::global(MailAddr(4)),
                    1,
                )),
                AcceptedInitializationRequest::AbsoluteTimer(ScheduleAt::new(
                    TimerId(0),
                    TimerGeneration(0),
                    due,
                )),
            ]
    );

    let received = behavior.receive(MailAddr(2), 4).unwrap();
    let accepted_after_initialization = runtime.accepted.len();
    let interpreted = <_ as InterpretSends<_, InitializationEvent, Here>>::interpret(
        received.sends,
        &mut runtime,
    )
    .await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert_eq!(runtime.accepted.len(), accepted_after_initialization);
    assert!(received.creates.is_empty());
    assert!(matches!(received.become_, Step::Continue));
    assert_eq!(behavior.base().0, 5);
}

#[tokio::test]
async fn typed_shutdown_stops_normally_without_running_the_inner_behavior() {
    let behavior = behavior_actors::StopOnShutdown::new(Quiet);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event = <_ as InjectEvent<ShutdownRequested, Here>>::inject_at(ShutdownRequested);
    let actions = behavior.transition(event).unwrap();

    assert!(actions.sends.inner.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn final_shutdown_transition_preserves_effects_and_forces_normal_stop() {
    let behavior = behavior_actors::FinalizeOnShutdown::new(
        ShutdownParent(CreationSequence::new()),
        finalize_parent,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event = <_ as InjectEvent<ShutdownRequested, Here>>::inject_at(ShutdownRequested);
    let actions = behavior.transition(event).unwrap();

    assert_eq!(actions.sends.inner.len(), 1);
    assert_eq!(actions.sends.inner[0].message, 42);
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(
        actions.creates.iter().next().unwrap().kind(),
        CreationKind::Birth
    );
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn outer_combinators_preserve_the_shutdown_lane() {
    let behavior = behavior_actors::Watch::new(
        behavior_actors::Deadline::new(
            behavior_actors::StopOnShutdown::new(Quiet),
            TimerId(0),
            None,
            |_| Step::Continue,
        ),
        MailAddr(8),
        stop_on_abnormal_death,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event =
        <_ as InjectEvent<ShutdownRequested, Inside<Inside<Here>>>>::inject_at(ShutdownRequested);
    let actions = behavior.transition(event).unwrap();

    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn shutdown_over_two_deadlines_preserves_both_exact_local_continuations() {
    type RootEvent = behavior::ShutdownEvent<
        behavior::DeadlineEvent<behavior::DeadlineEvent<User<MailAddr, u64>>>,
    >;

    struct TimerInterpreter {
        schedules: Vec<ScheduleAt>,
        pending: Vec<RootEvent>,
    }

    impl InterpretItem<ScheduleAt, RootEvent, Inside<Here>> for TimerInterpreter {
        fn interpret_item(
            &mut self,
            request: ScheduleAt,
        ) -> impl Future<
            Output = ItemSettlement<
                ScheduleAt,
                <ScheduleAt as ActionItem>::Accepted,
                <ScheduleAt as ActionItem>::Rejection,
                <ScheduleAt as ActionItem>::Prerequisite,
            >,
        > + Send {
            let scheduled = TimerScheduled {
                id: request.id,
                generation: request.generation,
            };
            async move {
                self.schedules.push(request);
                self.pending.push(
                    <RootEvent as InjectEvent<TimerElapsed, Inside<Here>>>::inject_at(
                        TimerElapsed {
                            id: request.id,
                            generation: request.generation,
                        },
                    ),
                );
                ItemSettlement::Accepted(scheduled)
            }
        }
    }

    impl InterpretItem<ScheduleAt, RootEvent, Inside<Inside<Here>>> for TimerInterpreter {
        fn interpret_item(
            &mut self,
            request: ScheduleAt,
        ) -> impl Future<
            Output = ItemSettlement<
                ScheduleAt,
                <ScheduleAt as ActionItem>::Accepted,
                <ScheduleAt as ActionItem>::Rejection,
                <ScheduleAt as ActionItem>::Prerequisite,
            >,
        > + Send {
            let scheduled = TimerScheduled {
                id: request.id,
                generation: request.generation,
            };
            async move {
                self.schedules.push(request);
                self.pending.push(<RootEvent as InjectEvent<
                    TimerElapsed,
                    Inside<Inside<Here>>,
                >>::inject_at(TimerElapsed {
                    id: request.id,
                    generation: request.generation,
                }));
                ItemSettlement::Accepted(scheduled)
            }
        }
    }

    let inner_due = Instant::now() + Duration::from_secs(1);
    let outer_due = inner_due + Duration::from_secs(1);
    let initialized = behavior_actors::StopOnShutdown::new(behavior_actors::Deadline::new(
        behavior_actors::Deadline::new(Quiet, TimerId(0), Some(inner_due), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(0),
        Some(outer_due),
        |_| Step::Continue,
    ))
    .initialize()
    .unwrap();

    let mut timers = TimerInterpreter {
        schedules: Vec::new(),
        pending: Vec::new(),
    };
    let interpreted = <_ as InterpretSends<_, RootEvent, Here>>::interpret(
        initialized.actions.sends,
        &mut timers,
    )
    .await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert_eq!(
        timers.schedules,
        [
            ScheduleAt::new(TimerId(0), TimerGeneration(0), inner_due),
            ScheduleAt::new(TimerId(0), TimerGeneration(0), outer_due),
        ]
    );
    assert_eq!(timers.pending.len(), 2);

    let mut active = initialized.behavior;
    // Traversal records the inner deadline first. Popping completes the two
    // identical requests in reverse order: outer, then inner.
    let outer = timers.pending.pop().unwrap();
    assert_eq!(active.transition(outer).unwrap().become_, Step::Continue);

    let inner = timers.pending.pop().unwrap();
    assert_eq!(
        active.transition(inner).unwrap().become_,
        Step::Stop(behavior::Stopped)
    );
}

#[tokio::test]
async fn at_is_a_typed_clock_actor_protocol() {
    let now = Instant::now();
    let behavior = behavior_actors::Deadline::new(Quiet, TimerId(0), Some(now), |_| Step::Continue);

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert!(initial.sends.inner.is_empty());
    assert_eq!(initial.sends.owned.len(), 1);
    assert_eq!(initial.sends.owned[0].at, now);

    let fired = behavior
        .on_path(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })
        .unwrap();
    assert!(fired.sends.owned.is_empty());
}

#[tokio::test]
async fn watching_registers_and_reacts_through_messages() {
    let peer = MailAddr(7);
    let behavior = behavior_actors::Watch::new(Quiet, peer, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.owned[0].peer, peer);

    let stopped = EventLayer::Owned(PeerStopped {
        peer,
        outcome: Err(Crash::Failed),
    });
    let actions = behavior.transition(stopped).unwrap();
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn stashing_is_local_state_and_replay() {
    struct Seen(Vec<u64>);
    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = NoBirths, error = Never)]
    impl Seen {
        fn receive(
            &mut self,
            _from: MailAddr,
            message: u64,
        ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
            self.0.push(message);
            Ok(Actions::cont())
        }
    }
    let behavior = behavior_actors::Stash::new(Seen(Vec::new()), |message| match message {
        0 => StashRoute::Release,
        1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stashed = behavior.transition(User::user(MailAddr(1), 1)).unwrap();
    assert_no_vec_effects!(stashed, Step::Continue);
    let released = behavior.transition(User::user(MailAddr(1), 0)).unwrap();
    assert_no_vec_effects!(released, Step::Continue);
    assert_eq!(behavior.base().0, vec![0]);
    assert_eq!(behavior.held(), 1);
}

#[tokio::test]
async fn fsm_is_receive_plus_become_policy() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        Loading,
        Ready,
    }
    #[derive(Clone)]
    enum Message {
        Work(u64),
        Ready,
    }
    let machine = Machine::new(
        Vec::new(),
        Phase::Loading,
        |phase, seen: &mut Vec<u64>, message| {
            Ok::<Move<Phase>, Never>(match (phase, message) {
                (Phase::Loading, Message::Work(_)) => Move::Defer,
                (_, Message::Work(value)) => {
                    seen.push(*value);
                    Move::Stay
                }
                (_, Message::Ready) => Move::Goto(Phase::Ready),
            })
        },
    );
    let initialized = machine.initialize().unwrap();
    let mut machine = initialized.behavior;
    let deferred = machine
        .transition(User::user(MailAddr(0), Message::Work(3)))
        .unwrap();
    assert_no_vec_effects!(deferred, Step::Continue);
    let ready = machine
        .transition(User::user(MailAddr(0), Message::Ready))
        .unwrap();
    assert_no_vec_effects!(ready, Step::Continue);
    assert_eq!(machine.state(), &[3]);
}

#[derive(Clone)]
struct StashMessage {
    id: u64,
    release: Arc<AtomicBool>,
}

fn mutation_stash_route(message: &StashMessage) -> StashRoute {
    if message.id == 2 {
        message.release.store(true, Ordering::SeqCst);
        StashRoute::Release
    } else if message.release.load(Ordering::SeqCst) {
        StashRoute::Deliver
    } else {
        StashRoute::Stash
    }
}

struct StashRecording(Vec<u64>);

#[behavior::behavior(addr = MailAddr, message = StashMessage, sends = Vec<Never>, births = NoBirths, error = Never)]
impl StashRecording {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: StashMessage,
    ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        self.0.push(message.id);
        Ok(Actions::cont())
    }
}

#[tokio::test]
async fn stash_release_delivers_the_trigger_then_drains_the_held_fifo() {
    let release = Arc::new(AtomicBool::new(false));
    let behavior = behavior_actors::Stash::new(StashRecording(Vec::new()), mutation_stash_route);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stashed = behavior
        .transition(User::user(
            MailAddr(0),
            StashMessage {
                id: 1,
                release: Arc::clone(&release),
            },
        ))
        .unwrap();
    assert_no_vec_effects!(stashed, Step::Continue);
    let released = behavior
        .transition(User::user(MailAddr(0), StashMessage { id: 2, release }))
        .unwrap();
    assert_no_vec_effects!(released, Step::Continue);
    assert_eq!(behavior.base().0, [2, 1]);
    assert_eq!(behavior.held(), 0);
}

fn continue_on_death(
    _behavior: &mut Watch<Quiet>,
    _peer: MailAddr,
    _outcome: &Result<Exit<MailAddr>, Crash>,
) -> Become {
    Step::Continue
}

#[tokio::test]
async fn nested_watch_receives_the_selected_peer_stop() {
    let behavior = behavior_actors::Watch::new(
        behavior_actors::Watch::new(Quiet, MailAddr(1), stop_on_abnormal_death),
        MailAddr(2),
        continue_on_death,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let actions = behavior
        .transition(EventLayer::Inner(EventLayer::Owned(PeerStopped {
            peer: MailAddr(1),
            outcome: Err(Crash::Failed),
        })))
        .unwrap();
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MutationPhase {
    Initial,
}

#[derive(Clone, Copy)]
enum MutationMessage {
    Deferred,
    Stay,
    Stop,
}

#[tokio::test]
async fn fsm_preserves_direct_stop_and_does_not_drain_on_stay() {
    let machine = Machine::new(
        0_u8,
        MutationPhase::Initial,
        |_, visits, message| -> Result<Move<MutationPhase>, Never> {
            match message {
                MutationMessage::Deferred => {
                    *visits += 1;
                    Ok(Move::Defer)
                }
                MutationMessage::Stay => Ok(Move::Stay),
                MutationMessage::Stop => Ok(Move::Stop),
            }
        },
    );
    let initialized = machine.initialize().unwrap();
    let mut machine = initialized.behavior;
    let deferred = machine
        .transition(User::user(MailAddr(0), MutationMessage::Deferred))
        .unwrap();
    assert_no_vec_effects!(deferred, Step::Continue);
    let stayed = machine
        .transition(User::user(MailAddr(0), MutationMessage::Stay))
        .unwrap();
    assert_no_vec_effects!(stayed, Step::Continue);
    assert_eq!(*machine.state(), 1);
    assert_eq!(machine.held(), 1);
    let stopped = machine
        .transition(User::user(MailAddr(0), MutationMessage::Stop))
        .unwrap();
    assert!(matches!(stopped.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn receive_timeout_reacts_only_to_its_own_live_timer_id() {
    let behavior = behavior_actors::ReceiveTimeout::new(
        behavior_actors::Deadline::new(Quiet, TimerId(0), Some(Instant::now()), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(1),
        Duration::from_secs(1),
        |_| Actions::stop(),
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let inner_deadline = behavior
        .transition(behavior::EventLayer::Inner(behavior::EventLayer::Owned(
            TimerElapsed {
                id: TimerId(0),
                generation: TimerGeneration(0),
            },
        )))
        .unwrap();
    assert!(matches!(
        inner_deadline.become_,
        Step::Stop(behavior::Stopped)
    ));
    let matching_behavior = behavior_actors::ReceiveTimeout::new(
        behavior_actors::Deadline::new(Quiet, TimerId(0), Some(Instant::now()), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(1),
        Duration::from_secs(1),
        |_| Actions::stop(),
    );
    let initialized = matching_behavior.initialize().unwrap();
    let matching_generation = initialized.actions.sends.owned[0].generation;
    let mut matching_behavior = initialized.behavior;
    let matching = matching_behavior
        .transition(behavior::EventLayer::Owned(TimerElapsed {
            id: TimerId(1),
            generation: matching_generation,
        }))
        .unwrap();
    assert!(matches!(matching.become_, Step::Stop(behavior::Stopped)));
}

#[derive(Clone)]
enum TimerAwareEvent {
    User(User<MailAddr, u64>),
    Time(TimerElapsed),
}

impl UserEvent for TimerAwareEvent {
    type Addr = MailAddr;
    type Message = u64;

    fn user(from: MailAddr, message: u64) -> Self {
        Self::User(User { from, message })
    }

    fn into_user(self) -> Result<User<MailAddr, u64>, Self> {
        match self {
            Self::User(user) => Ok(user),
            event @ Self::Time(_) => Err(event),
        }
    }
}

impl InjectEvent<TimerElapsed, Here> for TimerAwareEvent {
    fn inject_at(event: TimerElapsed) -> Self {
        Self::Time(event)
    }
}

struct TimerAware;

impl behavior::Protocol for TimerAware {
    type Addr = MailAddr;
    type Msg = u64;
}

impl Behavior for TimerAware {
    type Protocol = Self;
    type Event = TimerAwareEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(
        &mut self,
        _: behavior::InitializationTurn,
    ) -> Result<Actions<MailAddr, Never, Self::Sends, NoBirths>, Never> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        event: Self::Event,
    ) -> Result<Actions<MailAddr, Never, Self::Sends, NoBirths>, Never> {
        match event {
            TimerAwareEvent::Time(_elapsed) => Ok(Actions::stop()),
            TimerAwareEvent::User(_) => Ok(Actions::cont()),
        }
    }
}

#[tokio::test]
async fn a_stale_local_receive_timeout_is_consumed_not_forwarded() {
    let behavior = behavior_actors::ReceiveTimeout::new(
        TimerAware,
        TimerId(0),
        Duration::from_secs(1),
        |_| Actions::stop(),
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stale = behavior
        .transition(behavior::EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(99),
        }))
        .unwrap();
    assert!(matches!(stale.become_, Step::Continue));
}

#[test]
fn birth_modes_are_disjoint_and_wrappers_forward_them() {
    requires_no_births((Quiet).base());

    let creator = behavior_actors::Watch::new(
        behavior_actors::Stash::new(
            behavior_actors::Deadline::new(
                ExplicitInitialization {
                    received: Vec::new(),
                    creations: CreationSequence::new(),
                },
                TimerId(0),
                None,
                |_| Step::Continue,
            ),
            |_| StashRoute::Deliver,
        ),
        MailAddr(4),
        stop_on_abnormal_death,
    );
    requires_births::<_, Quiet>(&creator);
}

#[tokio::test]
async fn stale_time_events_do_not_fire_or_reschedule() {
    let due = Instant::now() + Duration::from_secs(2);
    let behavior = behavior_actors::Deadline::new(Quiet, TimerId(0), Some(due), |_| {
        Step::Stop(behavior::Stopped)
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stale = EventLayer::Owned(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(1),
    });
    let ignored = behavior.transition(stale).unwrap();
    assert!(matches!(ignored.become_, Step::Continue));

    let fired = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(fired.become_, Step::Stop(behavior::Stopped)));

    let duplicate = behavior
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(duplicate.become_, Step::Continue));
}
