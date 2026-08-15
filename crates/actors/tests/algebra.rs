#![allow(
    clippy::needless_pass_by_value,
    clippy::no_effect_underscore_binding,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    reason = "fixture methods intentionally match the fallible behavior macro contract"
)]

use behavior_actors as behavior;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use behavior::{
    Acted, Actions, Activate, Become, Behavior, BehaviorBase, Births, ChildStopped, Compose, Crash,
    Create, CreationKind, CreationRejection, CreationResolved, DeadlineEvent, Delivery, Exit,
    Machine, MailAddr, Move, Never, NoBirths, ObserveChild, PeerStopped, Proxy, ProxyCommand,
    ProxyEvent, Recipient, RestartDenial, RestartPolicy, RouteInput, SendAlgebra, ServiceSends,
    ShutdownRequested, StashRoute, Step, Strategy, SupervisionEvent, SupervisionFailure,
    SupervisionFailureReason, Supervisor, TimerElapsed, TimerGeneration, TimerId, User, UserEvent,
    Watch, WatchEvent, WorkerStopped, stop_on_abnormal_death, stop_on_supervision_failure,
};
use proptest::prelude::*;
use std::time::Instant;
use tokio::runtime::Builder;

struct Quiet;

struct BehaviorSends {
    deliveries: Vec<Delivery<Quiet>>,
    child_observations: ServiceSends<ObserveChild<u64>>,
}

#[derive(Debug, PartialEq, Eq)]
struct Rejected(u64);

struct DelegatingCounter(usize);

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<Quiet>>, births = Births<Quiet>, error = Rejected)]
impl DelegatingCounter {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Quiet>>, Births<Quiet>, Rejected> {
        self.0 += 1;
        if message == 0 {
            return Err(Rejected(message));
        }
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), message)],
            vec![Create::replacement_incarnation(5, 3, Quiet)],
            Step::Stop(behavior::Stopped),
        ))
    }
}

struct ExplicitInitialization(Vec<u64>);

#[behavior::behavior(addr = MailAddr, message = u64, sends = BehaviorSends, births = Births<Quiet>, error = Never)]
impl ExplicitInitialization {
    fn init(&mut self) -> Acted<MailAddr, Never, BehaviorSends, Births<Quiet>, Never> {
        self.0.push(1);
        let mut sends = BehaviorSends::empty();
        sends.child_observations.extend([ObserveChild { nonce: 7 }]);
        Ok(Actions::new(
            sends,
            vec![Create::birth(7, Quiet)],
            Step::Continue,
        ))
    }

    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, BehaviorSends, Births<Quiet>, Never> {
        self.0.push(message);
        Ok(Actions::new(
            BehaviorSends {
                deliveries: vec![Delivery::new(Recipient::global(MailAddr(9)), message)],
                child_observations: ServiceSends::empty(),
            },
            Vec::new(),
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
            Vec::new(),
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

impl SendAlgebra for BehaviorSends {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            child_observations: ServiceSends::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.child_observations.append(other.child_observations);
    }
}

struct U8Sink;

impl Behavior for U8Sink {
    type Addr = MailAddr;
    type Msg = u8;
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
fn ordinary_and_service_send_algebras_have_disjoint_static_dispatch() {
    trait RouteSends<A: behavior::Address> {}

    impl<B: Behavior<Addr = MailAddr>> RouteSends<MailAddr> for Vec<Delivery<B>> {}
    impl<A: behavior::Address> RouteSends<A> for ServiceSends<ObserveChild<A::Nonce>> {}

    fn requires_route_sends<A: behavior::Address, S: RouteSends<A>>() {}

    requires_route_sends::<MailAddr, Vec<Delivery<Quiet>>>();
    requires_route_sends::<MailAddr, ServiceSends<ObserveChild<u64>>>();
}

fn requires_births<B, C>(_behavior: &B)
where
    B: Behavior<Birth = Births<C>>,
{
}

fn requires_worker_events<B>(_behavior: &B)
where
    B: Behavior,
    B::Event: RouteInput<WorkerStopped<MailAddr>>,
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

struct ShutdownParent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<Quiet>>, births = Births<Quiet>, error = Never)]
impl ShutdownParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Quiet>>, Births<Quiet>, Never> {
        Ok(Actions::cont())
    }
}

#[allow(
    clippy::type_complexity,
    clippy::unnecessary_wraps,
    reason = "the shutdown reaction must expose the complete typed Actions and error seats"
)]
fn finalize_parent(
    _behavior: &mut ShutdownParent,
    _request: ShutdownRequested,
) -> Acted<MailAddr, Never, Vec<Delivery<Quiet>>, Births<Quiet>, Never> {
    Ok(Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(9)), 42)],
        creates: vec![Create::birth(7, Quiet)],
        become_: Step::Continue,
    })
}

#[test]
fn actions_expose_the_typed_actor_transition_effects() {
    let mut actions: Actions<MailAddr, Never, Vec<Delivery<Quiet>>, NoBirths> = Actions::cont();
    actions
        .sends
        .push(Delivery::new(Recipient::global(MailAddr(9)), 42));

    assert_eq!(actions.sends[0].to.resolve(MailAddr(0)), MailAddr(9));
    assert_eq!(actions.sends[0].message, 42);
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
}

#[test]
fn active_behavior_invokes_one_fold_and_preserves_actions() {
    let initialized = DelegatingCounter(0).initialize().unwrap();
    let mut behavior = initialized.behavior;
    let actions = behavior.transition(User::new(MailAddr(7), 11)).unwrap();

    assert_eq!(behavior.base().0, 1);
    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].to.resolve(MailAddr(0)), MailAddr(7));
    assert_eq!(actions.sends[0].message, 11);
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.creates[0].nonce, 5);
    assert_eq!(
        actions.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 3 }
    );
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));

    assert!(matches!(
        behavior.transition(User::new(MailAddr(7), 0)),
        Err(Rejected(0))
    ));
    assert_eq!(behavior.base().0, 2);
}

#[test]
fn direct_behavior_preserves_explicit_initialization_and_transition_actions() {
    let behavior = ExplicitInitialization(Vec::new());

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(
        initial.sends.child_observations.as_slice(),
        &[ObserveChild { nonce: 7 }]
    );
    assert_eq!(initial.creates.len(), 1);
    assert!(matches!(initial.become_, Step::Continue));

    let transitioned = behavior.receive(MailAddr(3), 5).unwrap();
    assert_eq!(transitioned.sends.deliveries[0].message, 5);
    assert!(transitioned.creates.is_empty());
    assert!(matches!(
        transitioned.become_,
        Step::Stop(behavior::Stopped)
    ));
    assert_eq!(behavior.base().0, [1, 5]);
}

#[test]
fn direct_behavior_composes_with_existing_wrappers_and_init_order() {
    let due = Instant::now();
    let behavior = (InitializationCounter(0))
        .deadline(TimerId(0), Some(due), |_| Ok(Step::Continue))
        .stop_on_shutdown();

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.behavior[0].message, 1);
    assert_eq!(initial.sends.schedules.len(), 1);

    behavior.receive(MailAddr(2), 4).unwrap();
    assert_eq!(behavior.base().0, 5);
}

#[tokio::test]
async fn typed_shutdown_stops_normally_without_running_the_inner_fold() {
    let behavior = (Quiet).stop_on_shutdown();
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event = <_ as RouteInput<ShutdownRequested>>::route(ShutdownRequested).unwrap();
    let actions = behavior.transition(event).unwrap();

    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn final_shutdown_fold_preserves_effects_and_forces_normal_stop() {
    let behavior = (ShutdownParent).finalize_on_shutdown(finalize_parent);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event = <_ as RouteInput<ShutdownRequested>>::route(ShutdownRequested).unwrap();
    let actions = behavior.transition(event).unwrap();

    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].message, 42);
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.creates[0].nonce, 7);
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn outer_combinators_preserve_the_shutdown_lane() {
    let behavior = (Quiet)
        .stop_on_shutdown()
        .deadline(TimerId(0), None, |_| Ok(Step::Continue))
        .watch(MailAddr(8), stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event = <_ as RouteInput<ShutdownRequested>>::route(ShutdownRequested).unwrap();
    let actions = behavior.transition(event).unwrap();

    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn shutdown_composition_preserves_inner_initialization_effects() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = (Quiet)
        .deadline(TimerId(0), Some(due), |_| Ok(Step::Continue))
        .stop_on_shutdown();
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let _behavior = initialized.behavior;

    assert_eq!(initial.sends.schedules.len(), 1);
    assert_eq!(initial.sends.schedules[0].at, due);
    assert!(matches!(initial.become_, Step::Continue));
}

#[tokio::test]
async fn at_is_a_typed_clock_actor_protocol() {
    let now = Instant::now();
    let behavior = (Quiet).deadline(TimerId(0), Some(now), |_| Ok(Step::Continue));

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert!(initial.sends.behavior.is_empty());
    assert_eq!(initial.sends.schedules.len(), 1);
    assert_eq!(initial.sends.schedules[0].at, now);

    let fired = behavior
        .on(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })
        .unwrap();
    assert!(fired.sends.schedules.is_empty());
}

#[tokio::test]
async fn nested_at_composition_routes_stale_and_matching_events() {
    let early = Instant::now() + Duration::from_secs(1);
    let late = early + Duration::from_secs(1);
    let outer = (Quiet)
        .deadline(TimerId(0), Some(early), |_| Ok(Step::Continue))
        .deadline(TimerId(1), Some(late), |_| Ok(Step::Continue));

    let initialized = outer.initialize().unwrap();
    let initial = initialized.actions;
    let mut outer = initialized.behavior;
    assert_eq!(initial.sends.behavior.schedules[0].id, TimerId(0));
    assert_eq!(initial.sends.schedules[0].id, TimerId(1));
    assert_eq!(initial.sends.behavior.schedules[0].at, early);
    assert_eq!(initial.sends.schedules[0].at, late);

    let early_event = DeadlineEvent::Elapsed(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    });
    let actions = outer.transition(early_event).unwrap();
    assert!(actions.sends.behavior.schedules.is_empty());
}

#[tokio::test]
async fn spec_hides_composed_protocols_without_losing_their_effects() {
    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(8);
    let behavior = (Quiet)
        .deadline(TimerId(0), Some(due), |_| Ok(Step::Continue))
        .watch(peer, stop_on_abnormal_death)
        .stash(|_| StashRoute::Deliver);

    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.behavior.schedules[0].at, due);
    assert_eq!(initial.sends.observations[0].peer, peer);

    let time = WatchEvent::Behavior(DeadlineEvent::Elapsed(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    }));
    let actions = behavior.transition(time).unwrap();
    assert!(matches!(actions.become_, Step::Continue));
}

#[tokio::test]
async fn watching_registers_and_reacts_through_messages() {
    let peer = MailAddr(7);
    let behavior = (Quiet).watch(peer, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.observations[0].peer, peer);

    let stopped = WatchEvent::PeerStopped(PeerStopped {
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
    let behavior = (Seen(Vec::new())).stash(|message| match message {
        0 => StashRoute::Release,
        1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    behavior.transition(User::user(MailAddr(1), 1)).unwrap();
    behavior.transition(User::user(MailAddr(1), 0)).unwrap();
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
    machine
        .transition(User::user(MailAddr(0), Message::Work(3)))
        .unwrap();
    machine
        .transition(User::user(MailAddr(0), Message::Ready))
        .unwrap();
    assert_eq!(machine.state(), &[3]);
}

type Child = Quiet;

struct Parent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = Births<Child>, error = Never)]
impl Parent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, Births<Child>, Never> {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Quiet
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
    let behavior = (StashRecording(Vec::new())).stash(mutation_stash_route);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    behavior
        .transition(User::user(
            MailAddr(0),
            StashMessage {
                id: 1,
                release: Arc::clone(&release),
            },
        ))
        .unwrap();
    behavior
        .transition(User::user(MailAddr(0), StashMessage { id: 2, release }))
        .unwrap();
    assert_eq!(behavior.base().0, [2, 1]);
    assert_eq!(behavior.held(), 0);
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the fixture implements the fallible watch-reaction signature"
)]
fn continue_on_death(
    _behavior: &mut Watch<Quiet>,
    _peer: MailAddr,
    _outcome: &Result<Exit<MailAddr>, Crash>,
) -> Result<Become, Never> {
    Ok(Step::Continue)
}

#[tokio::test]
async fn an_outer_watcher_forwards_a_different_peer_to_the_inner_watcher() {
    let behavior = (Quiet)
        .watch(MailAddr(1), stop_on_abnormal_death)
        .watch(MailAddr(2), continue_on_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let actions = behavior
        .transition(WatchEvent::PeerStopped(PeerStopped {
            peer: MailAddr(1),
            outcome: Err(Crash::Failed),
        }))
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
    machine
        .transition(User::user(MailAddr(0), MutationMessage::Deferred))
        .unwrap();
    machine
        .transition(User::user(MailAddr(0), MutationMessage::Stay))
        .unwrap();
    assert_eq!(*machine.state(), 1);
    assert_eq!(machine.held(), 1);
    let stopped = machine
        .transition(User::user(MailAddr(0), MutationMessage::Stop))
        .unwrap();
    assert!(matches!(stopped.become_, Step::Stop(behavior::Stopped)));
}

#[tokio::test]
async fn receive_timeout_reacts_only_to_its_own_live_timer_id() {
    let behavior = (Quiet)
        .deadline(TimerId(0), Some(Instant::now()), |_| {
            Ok(Step::Stop(behavior::Stopped))
        })
        .receive_timeout(TimerId(1), Duration::from_secs(1), |_| Ok(Actions::stop()));
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let wrong = behavior
        .transition(behavior::ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(wrong.become_, Step::Stop(behavior::Stopped)));
    let matching_behavior = (Quiet)
        .deadline(TimerId(0), Some(Instant::now()), |_| {
            Ok(Step::Stop(behavior::Stopped))
        })
        .receive_timeout(TimerId(1), Duration::from_secs(1), |_| Ok(Actions::stop()));
    let initialized = matching_behavior.initialize().unwrap();
    let matching_generation = initialized.actions.sends.schedules[0].generation;
    let mut matching_behavior = initialized.behavior;
    let matching = matching_behavior
        .transition(behavior::ReceiveTimeoutEvent::Elapsed(TimerElapsed {
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

impl RouteInput<TimerElapsed> for TimerAwareEvent {
    fn route(event: TimerElapsed) -> Result<Self, TimerElapsed> {
        Ok(Self::Time(event))
    }
}

struct TimerAware;

impl Behavior for TimerAware {
    type Addr = MailAddr;
    type Msg = u64;
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
            TimerAwareEvent::Time(elapsed) => {
                let _ = elapsed;
                Ok(Actions::stop())
            }
            TimerAwareEvent::User(_) => Ok(Actions::cont()),
        }
    }
}

#[tokio::test]
async fn a_stale_local_receive_timeout_is_consumed_not_forwarded() {
    let behavior =
        (TimerAware).receive_timeout(TimerId(0), Duration::from_secs(1), |_| Ok(Actions::stop()));
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stale = behavior
        .transition(behavior::ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(99),
        }))
        .unwrap();
    assert!(matches!(stale.become_, Step::Continue));
}

#[tokio::test]
async fn a_proxy_ignores_a_stale_child_stop_nonce() {
    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let mut proxy = initialized.behavior;
    proxy.on(CreationResolved::birth(0)).unwrap();
    let stale = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 99,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert!(stale.sends.stopped_reports.is_empty());
    let forwarded = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Forward(7),
        )))
        .unwrap();
    assert_eq!(
        forwarded.sends.deliveries[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
}

#[test]
fn birth_modes_are_disjoint_and_wrappers_forward_them() {
    requires_no_births((Quiet).base());

    let creator = (Parent)
        .deadline(TimerId(0), None, |_| Ok(Step::Continue))
        .watch(MailAddr(4), stop_on_abnormal_death)
        .stash(|_| StashRoute::Deliver);
    requires_births::<_, Child>(&creator);

    let supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap();
    requires_births::<_, Proxy<Child>>(&supervisor);
    requires_worker_events(&supervisor);
    let timed_supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap()
        .deadline(TimerId(0), None, |_| Ok(Step::Continue));
    requires_worker_events(&timed_supervisor);
    requires_births::<_, Child>(&Proxy::new(child(0)));
}

fn supervisor(strategy: Strategy, policy: RestartPolicy, budget: u32) -> Supervisor<Parent, Child> {
    Supervisor::new(
        Parent,
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            3,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(strategy, policy, budget, Duration::MAX),
    )
    .unwrap()
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the supervision reaction shares its wrapped behavior's controlled-error seat"
)]
fn verify_budget_failure_and_stop(
    _parent: &mut Parent,
    failure: &SupervisionFailure<MailAddr>,
) -> Result<Become, Never> {
    assert_eq!(failure.child, 1);
    assert_eq!(failure.outcome, Err(Crash::Panicked));
    assert_eq!(
        failure.reason,
        SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
            restarts_in_window: 0,
            replacements_requested: 3,
            maximum_restarts: 2,
        })
    );
    let _ = failure;
    Ok(Step::Stop(behavior::Stopped))
}

#[tokio::test]
async fn supervisor_creates_proxies_and_replacement_is_a_send() {
    let supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            2,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_strategy(Strategy::OneForOne)
        .with_policy(RestartPolicy::Transient)
        .with_budget(2, Duration::MAX);
    let initialized = supervisor.initialize().unwrap();
    let initial = initialized.actions;
    let mut supervisor = initialized.behavior;
    assert_eq!(initial.creates.len(), 2);
    assert!(
        initial
            .creates
            .iter()
            .all(|create| create.kind == CreationKind::Birth)
    );
    assert_eq!(initial.sends.child_observations.len(), 2);
    assert_eq!(initial.sends.child_observations[0].nonce, 0);
    assert_eq!(initial.sends.child_observations[1].nonce, 1);

    let event = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 0,
        worker: 0,
        outcome: Err(Crash::Failed),
        at: Instant::now(),
    });
    let actions = supervisor.transition(event).unwrap();
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.replacement_commands.len(), 1);
    assert_eq!(
        actions.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
}

#[tokio::test]
async fn proxy_replacement_creates_a_fresh_incarnation() {
    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let first = initialized.actions;
    let mut proxy = initialized.behavior;
    assert_eq!(first.creates[0].nonce, 0);
    assert_eq!(first.creates[0].kind, CreationKind::Birth);
    proxy.on(CreationResolved::birth(0)).unwrap();
    let second = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .unwrap();
    assert!(second.creates.is_empty());

    let second = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(second.creates[0].nonce, 1);
    assert_eq!(
        second.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 0 }
    );
    assert_eq!(second.sends.child_observations[0].nonce, 1);
    assert_eq!(second.sends.stopped_reports[0].outcome, Err(Crash::Failed));
    proxy
        .on(CreationResolved::replacement_incarnation(1, 0))
        .unwrap();

    let forwarded = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Forward(7),
        )))
        .unwrap();
    assert_eq!(
        forwarded.sends.deliveries[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 1)
    );
    assert_eq!(forwarded.sends.deliveries[0].message, 7);
}

#[tokio::test]
async fn proxy_routes_only_after_matching_installation_and_rejection_stays_vacant() {
    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let mut proxy = initialized.behavior;

    let pending = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Forward(1),
        )))
        .unwrap();
    assert!(pending.sends.deliveries.is_empty());

    let stale = proxy.on(CreationResolved::birth(1)).unwrap();
    assert!(stale.sends.creation_reports.is_empty());

    let rejected = proxy
        .on(CreationResolved::rejected(
            0,
            CreationKind::Birth,
            CreationRejection::InitializationFailed,
        ))
        .unwrap();
    assert_eq!(
        rejected.sends.creation_reports[0].result,
        Err(CreationRejection::InitializationFailed)
    );

    let vacant = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Forward(2),
        )))
        .unwrap();
    assert!(vacant.sends.deliveries.is_empty());
}

#[tokio::test]
async fn proxy_serializes_attempts_and_rejection_preserves_last_installed_incarnation() {
    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let mut proxy = initialized.behavior;
    proxy.on(CreationResolved::birth(0)).unwrap();
    proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(1)),
        )))
        .unwrap();
    let first_attempt = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(
        first_attempt.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 0 }
    );

    let overlapping = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(2)),
        )))
        .unwrap();
    assert!(overlapping.creates.is_empty());

    proxy
        .on(CreationResolved::rejected(
            1,
            CreationKind::ReplacementIncarnation { replaces: 0 },
            CreationRejection::EnvironmentFailed,
        ))
        .unwrap();
    let retry = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(3)),
        )))
        .unwrap();
    assert_eq!(retry.creates[0].nonce, 2);
    assert_eq!(
        retry.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 0 }
    );
}

#[tokio::test]
async fn idle_proxy_marks_an_immediate_successor_as_a_replacement_incarnation() {
    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let initial = initialized.actions;
    let mut proxy = initialized.behavior;
    assert_eq!(initial.creates[0].kind, CreationKind::Birth);
    proxy.on(CreationResolved::birth(0)).unwrap();

    let stopped = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert!(stopped.creates.is_empty());

    let replacement = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .unwrap();
    assert_eq!(replacement.creates[0].nonce, 1);
    assert_eq!(
        replacement.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 0 }
    );
}

#[tokio::test]
async fn stable_proxy_reports_worker_stop_and_creates_fresh_replacement() {
    let at = Instant::now();
    let supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_strategy(Strategy::OneForOne)
        .with_policy(RestartPolicy::Transient)
        .with_budget(1, Duration::MAX);

    let initialized = supervisor.initialize().unwrap();
    let mut initial = initialized.actions;
    let mut supervisor = initialized.behavior;
    assert_eq!(initial.creates[0].nonce, 0);
    assert_eq!(initial.sends.child_observations[0].nonce, 0);
    let proxy = initial.creates.remove(0).child;

    let initialized = proxy.initialize().unwrap();
    let worker_birth = initialized.actions;
    let mut proxy = initialized.behavior;
    assert_eq!(worker_birth.creates[0].nonce, 0);
    assert_eq!(worker_birth.creates[0].kind, CreationKind::Birth);
    assert_eq!(worker_birth.sends.child_observations[0].nonce, 0);
    proxy.on(CreationResolved::birth(0)).unwrap();

    let worker_stop = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Panicked),
            at,
        }))
        .unwrap();
    assert!(worker_stop.creates.is_empty());
    assert!(matches!(worker_stop.become_, Step::Continue));
    assert_eq!(
        worker_stop.sends.stopped_reports[0].outcome,
        Err(Crash::Panicked)
    );

    let restart = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: worker_stop.sends.stopped_reports[0].outcome,
            at: worker_stop.sends.stopped_reports[0].at,
        }))
        .unwrap();
    assert!(restart.creates.is_empty());
    assert_eq!(restart.sends.replacement_commands.len(), 1);
    assert_eq!(
        restart.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );

    let command = restart
        .sends
        .replacement_commands
        .into_iter()
        .next()
        .unwrap();
    let replacement = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            command.message,
        )))
        .unwrap();
    assert_eq!(replacement.creates[0].nonce, 1);
    assert_eq!(
        replacement.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 0 }
    );
    assert_eq!(replacement.sends.child_observations[0].nonce, 1);
    assert!(matches!(replacement.become_, Step::Continue));
}

#[tokio::test]
async fn stopped_proxy_is_retired_without_sending_to_its_dead_address() {
    let initialized = supervisor(Strategy::OneForAll, RestartPolicy::Permanent, 3)
        .initialize()
        .unwrap();
    let mut supervisor = initialized.behavior;
    let stopped = supervisor
        .transition(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 1,
            outcome: Err(Crash::Panicked),
            at: Instant::now(),
        }))
        .unwrap();

    assert!(stopped.creates.is_empty());
    assert!(stopped.sends.replacement_commands.is_empty());
    assert!(!supervisor.is_alive(1).unwrap());
    assert_eq!(stopped.become_, Step::Continue);
}

#[tokio::test]
async fn configured_supervision_failure_reaction_stops_on_budget_denial() {
    let supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            3,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_strategy(Strategy::OneForAll)
        .with_policy(RestartPolicy::Permanent)
        .with_budget(2, Duration::MAX)
        .with_failure_reaction(verify_budget_failure_and_stop);
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            worker: 1,
            outcome: Err(Crash::Panicked),
            at: Instant::now(),
        }))
        .unwrap();

    assert!(actions.sends.replacement_commands.is_empty());
    assert_eq!(actions.sends.failure_reports.len(), 1);
    assert_eq!(
        actions.sends.failure_reports[0].failure.reason,
        SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
            restarts_in_window: 0,
            replacements_requested: 3,
            maximum_restarts: 2,
        })
    );
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
    assert!(!supervisor.is_alive(1).unwrap());
}

#[tokio::test]
async fn configured_supervision_failure_reaction_stops_when_stable_proxy_stops() {
    let supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_failure_reaction(stop_on_supervision_failure);
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let actions = supervisor
        .transition(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Ok(Exit::Normal),
            at: Instant::now(),
        }))
        .unwrap();

    assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
    assert_eq!(actions.sends.failure_reports.len(), 1);
    assert_eq!(
        actions.sends.failure_reports[0].failure.reason,
        SupervisionFailureReason::StableChildStopped
    );
    assert!(!supervisor.is_alive(0).unwrap());
}

#[tokio::test]
async fn restart_policy_ineligibility_is_not_a_supervision_failure() {
    let supervisor = (Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_policy(RestartPolicy::Temporary)
        .with_failure_reaction(stop_on_supervision_failure);
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();

    assert_eq!(actions.become_, Step::Continue);
    assert!(!supervisor.is_alive(0).unwrap());
}

#[tokio::test]
async fn supervision_failure_exit_is_an_abnormal_transient_worker_outcome() {
    let initialized = supervisor(Strategy::OneForOne, RestartPolicy::Transient, 1)
        .initialize()
        .unwrap();
    let mut supervisor = initialized.behavior;
    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildStopped,
            )),
            at: Instant::now(),
        }))
        .unwrap();

    assert_eq!(actions.sends.replacement_commands.len(), 1);
    assert_eq!(
        actions.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
}

struct BirthingParent(bool);

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = Births<Child>, error = Never)]
impl BirthingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, Births<Child>, Never> {
        if self.0 {
            return Ok(Actions::cont());
        }
        self.0 = true;
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::birth(nonce, child(0))],
            become_: Step::Continue,
        })
    }
}

struct ReplacingParent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = Births<Child>, error = Never)]
impl ReplacingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, Births<Child>, Never> {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::replacement_incarnation(nonce, nonce - 1, child(0))],
            become_: Step::Continue,
        })
    }
}

#[tokio::test]
async fn supervisor_preserves_and_observes_dynamic_births_once() {
    let supervisor = (BirthingParent(false))
        .children(
            |index| u64::try_from(index).unwrap(),
            0,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_budget(1, Duration::MAX);
    let initialized = supervisor.initialize().unwrap();
    let initial = initialized.actions;
    let mut supervisor = initialized.behavior;
    assert!(initial.creates.is_empty());

    let born = supervisor
        .transition(UserEvent::user(MailAddr(0), 9))
        .unwrap();
    assert_eq!(born.creates.len(), 1);
    assert_eq!(born.creates[0].nonce, 9);
    assert_eq!(born.creates[0].kind, CreationKind::Birth);
    assert_eq!(born.sends.child_observations.len(), 1);
    assert_eq!(born.sends.child_observations[0].nonce, 9);
    assert_eq!(supervisor.child_count(), 1);

    let stopped = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 9,
        worker: 9,
        outcome: Err(Crash::Failed),
        at: Instant::now(),
    });
    let replacement = supervisor.transition(stopped).unwrap();
    assert_eq!(replacement.sends.replacement_commands.len(), 1);
    assert_eq!(
        replacement.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 9)
    );
}

#[tokio::test]
async fn supervisor_preserves_dynamic_replacement_provenance_when_wrapping_the_child() {
    let supervisor = (ReplacingParent)
        .children(
            |index| u64::try_from(index).unwrap(),
            0,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_budget(1, Duration::MAX);
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let replacement = supervisor
        .transition(UserEvent::user(MailAddr(0), 9))
        .unwrap();
    assert_eq!(replacement.creates.len(), 1);
    assert_eq!(replacement.creates[0].nonce, 9);
    assert_eq!(
        replacement.creates[0].kind,
        CreationKind::ReplacementIncarnation { replaces: 8 }
    );
}

#[tokio::test]
async fn supervision_strategy_policy_and_budget_are_pure_send_decisions() {
    let at = Instant::now();
    let stopped = |nonce| {
        SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: nonce,
            worker: nonce,
            outcome: Err(Crash::Failed),
            at,
        })
    };

    let mut one = supervisor(Strategy::OneForOne, RestartPolicy::Transient, 3)
        .initialize()
        .unwrap()
        .behavior;
    assert_eq!(
        one.transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        1
    );

    let mut all = supervisor(Strategy::OneForAll, RestartPolicy::Transient, 3)
        .initialize()
        .unwrap()
        .behavior;
    assert_eq!(
        all.transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        3
    );

    let mut rest = supervisor(Strategy::RestForOne, RestartPolicy::Transient, 3)
        .initialize()
        .unwrap()
        .behavior;
    assert_eq!(
        rest.transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        2
    );

    let mut temporary = supervisor(Strategy::OneForOne, RestartPolicy::Temporary, 3)
        .initialize()
        .unwrap()
        .behavior;
    assert!(
        temporary
            .transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .is_empty()
    );
    assert!(!temporary.is_alive(1).unwrap());

    let mut denied = supervisor(Strategy::OneForOne, RestartPolicy::Permanent, 0)
        .initialize()
        .unwrap()
        .behavior;
    assert!(
        denied
            .transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .is_empty()
    );
}

#[tokio::test]
async fn stale_time_events_do_not_fire_or_reschedule() {
    let due = Instant::now() + Duration::from_secs(2);
    let behavior = (Quiet).deadline(TimerId(0), Some(due), |_| Ok(Step::Stop(behavior::Stopped)));
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stale = DeadlineEvent::Elapsed(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(1),
    });
    let ignored = behavior.transition(stale).unwrap();
    assert!(matches!(ignored.become_, Step::Continue));

    let fired = behavior
        .transition(DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(fired.become_, Step::Stop(behavior::Stopped)));

    let duplicate = behavior
        .transition(DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(duplicate.become_, Step::Continue));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn nested_time_protocol_preserves_every_schedule(first in 0_u64..10_000, second in 0_u64..10_000) {
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(first);
        let second = origin + Duration::from_nanos(second);
        let outer = (Quiet)
            .deadline(TimerId(0), Some(first), |_| Ok(Step::Continue))
            .deadline(TimerId(1), Some(second), |_| Ok(Step::Continue));
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = outer.initialize().unwrap();
    let actions = initialized.actions;
    let _outer = initialized.behavior;
        prop_assert_eq!(actions.sends.behavior.schedules[0].at, first);
        prop_assert_eq!(actions.sends.schedules[0].at, second);
    }

    #[test]
    fn supervision_strategy_matches_its_candidate_set(dead in 0_usize..3, strategy in 0_u8..3) {
        let strategy = match strategy {
            0 => Strategy::OneForOne,
            1 => Strategy::OneForAll,
            _ => Strategy::RestForOne,
        };
        let expected = match strategy {
            Strategy::OneForOne => 1,
            Strategy::OneForAll => 3,
            Strategy::RestForOne => 3 - dead,
        };
        let mut behavior = supervisor(strategy, RestartPolicy::Transient, 3)
            .initialize()
            .unwrap()
            .behavior;
        let event = SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: u64::try_from(dead).unwrap(),
            worker: u64::try_from(dead).unwrap(),
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        });
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let actions = behavior.transition(event).unwrap();
        prop_assert_eq!(actions.sends.replacement_commands.len(), expected);
        prop_assert!(actions.creates.is_empty());
    }
}
