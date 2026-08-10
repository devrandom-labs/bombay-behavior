use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use behavior::{
    Acted, Actions, Become, Behavior, Births, ChildStopped, Compose, Crash, Create, CreationKind,
    CreationRejection, CreationResolved, Deadline, DeadlineEvent, Delivery, Exit, Handler, Machine,
    MailAddr, Move, Never, NoBirths, ObserveChild, PeerStopped, Proxy, ProxyCommand, ProxyEvent,
    Pure, Recipient, RestartDenial, RestartPolicy, Route, ServiceSends, ShutdownEvent,
    ShutdownRequested, StashRoute, Step, Strategy, SupervisionEvent, SupervisionFailure,
    SupervisionFailureReason, Supervisor, TimeEvent, TimerElapsed, TimerGeneration, TimerId, User,
    UserEvent, Watch, WatchEvent, WorkerEvent, WorkerStopped, run, stop_on_abnormal_death,
    stop_on_supervision_failure, workers,
};
use communication::{Config, channel};
use proptest::prelude::*;
use tokio::runtime::Builder;
use tokio::time::Instant;

struct Quiet;

fn requires_no_births<B: Behavior<Birth = NoBirths>>(_behavior: &B) {}

#[test]
fn ordinary_and_service_send_algebras_have_disjoint_static_dispatch() {
    trait RouteSends<A: behavior::Address> {}

    impl<A: behavior::Address, M> RouteSends<A> for Vec<Delivery<A, M>> {}
    impl<A: behavior::Address> RouteSends<A> for ServiceSends<ObserveChild<A::Nonce>> {}

    fn requires_route_sends<A: behavior::Address, S: RouteSends<A>>() {}

    requires_route_sends::<MailAddr, Vec<Delivery<MailAddr, ObserveChild<u64>>>>();
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
    B::Event: WorkerEvent,
{
}

impl Handler for Quiet {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

struct ShutdownParent;

impl Handler<u64, Births<Pure<Quiet>>> for ShutdownParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Births<Pure<Quiet>>, Never> {
        Ok(Actions::cont())
    }
}

#[allow(
    clippy::type_complexity,
    clippy::unnecessary_wraps,
    reason = "the shutdown reaction must expose the complete typed Actions and error seats"
)]
fn finalize_parent(
    _behavior: &mut Pure<ShutdownParent, u64, Births<Pure<Quiet>>>,
    _request: ShutdownRequested,
) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Births<Pure<Quiet>>, Never> {
    Ok(Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(9)), 42)],
        creates: vec![Create::birth(7, Pure::new(Quiet))],
        become_: Step::Continue,
    })
}

#[test]
fn actions_expose_the_typed_actor_transition_effects() {
    let mut actions: Actions<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, NoBirths> =
        Actions::cont();
    actions
        .sends
        .push(Delivery::new(Recipient::global(MailAddr(9)), 42));

    assert_eq!(actions.sends[0].to.route(), Route::Global(MailAddr(9)));
    assert_eq!(actions.sends[0].message, 42);
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
}

#[tokio::test]
async fn typed_shutdown_stops_normally_without_running_the_inner_fold() {
    let mut behavior = Compose::new(Quiet).stop_on_shutdown();
    let event = <_ as ShutdownEvent>::shutdown_requested(ShutdownRequested).unwrap();
    let actions = behavior.transition(event).unwrap();

    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn final_shutdown_fold_preserves_effects_and_forces_normal_stop() {
    let mut behavior = Compose::new(ShutdownParent).finalize_on_shutdown(finalize_parent);
    let event = <_ as ShutdownEvent>::shutdown_requested(ShutdownRequested).unwrap();
    let actions = behavior.transition(event).unwrap();

    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].message, 42);
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.creates[0].nonce, 7);
    assert!(matches!(actions.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn outer_combinators_preserve_the_shutdown_lane() {
    let mut behavior = Compose::new(Quiet)
        .stop_on_shutdown()
        .deadline(None, |_| Ok(Step::Continue))
        .watch(MailAddr(8), stop_on_abnormal_death);
    let event = <_ as ShutdownEvent>::shutdown_requested(ShutdownRequested).unwrap();
    let actions = behavior.transition(event).unwrap();

    assert!(matches!(actions.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn shutdown_composition_preserves_inner_initialization_effects() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior = Compose::new(Quiet)
        .deadline(Some(due), |_| Ok(Step::Continue))
        .stop_on_shutdown();
    let initial = behavior.init().unwrap();

    assert_eq!(initial.sends.schedules.len(), 1);
    assert_eq!(initial.sends.schedules[0].at, due);
    assert!(matches!(initial.become_, Step::Continue));
}

#[tokio::test]
async fn at_is_a_typed_clock_actor_protocol() {
    let now = Instant::now();
    let mut behavior = Compose::new(Quiet).deadline(Some(now), |_| Ok(Step::Continue));

    let initial = behavior.init().unwrap();
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
async fn driver_interprets_initial_effect_before_receiving() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = Compose::new(Quiet).deadline(Some(due), |_| Ok(Step::Continue));
    let (control, user, mailbox) = channel::<Never, u64>(Config::new(1));
    drop(user);
    drop(control);

    let transcript = run(behavior, mailbox, MailAddr(0)).await.unwrap();
    assert_eq!(transcript.sends.schedules.len(), 1);
    assert_eq!(transcript.sends.schedules[0].at, due);
    assert_eq!(transcript.exit, Exit::Collected);
}

#[tokio::test]
async fn nested_at_composition_routes_stale_and_matching_events() {
    let early = Instant::now() + Duration::from_secs(1);
    let late = early + Duration::from_secs(1);
    let inner = Deadline::new(Pure::new(Quiet), TimerId(0), Some(early), |_| {
        Ok(Step::Continue)
    });
    let mut outer = Deadline::new(inner, TimerId(1), Some(late), |_| Ok(Step::Continue));

    let initial = outer.init().unwrap();
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
    let mut behavior = Compose::new(Quiet)
        .deadline(Some(due), |_| Ok(Step::Continue))
        .watch(peer, stop_on_abnormal_death)
        .stash(|_| StashRoute::Deliver);

    let initial = behavior.init().unwrap();
    assert_eq!(initial.sends.behavior.schedules[0].at, due);
    assert_eq!(initial.sends.observations[0].peer, peer);

    let time = WatchEvent::Inner(DeadlineEvent::Elapsed(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    }));
    let actions = behavior.transition(time).unwrap();
    assert!(matches!(actions.become_, Step::Continue));
}

#[tokio::test]
async fn watching_registers_and_reacts_through_messages() {
    let peer = MailAddr(7);
    let mut behavior = Watch::new(Pure::new(Quiet), peer, stop_on_abnormal_death);
    let initial = behavior.init().unwrap();
    assert_eq!(initial.sends.observations[0].peer, peer);

    let stopped = WatchEvent::PeerStopped(PeerStopped {
        peer,
        outcome: Err(Crash::Failed),
    });
    let actions = behavior.transition(stopped).unwrap();
    assert!(matches!(actions.become_, Step::Stop(Exit::LinkDied(p)) if p == peer));
}

#[tokio::test]
async fn stashing_is_local_state_and_replay() {
    struct Seen(Vec<u64>);
    impl Handler for Seen {
        type Addr = MailAddr;
        type Msg = u64;
        fn receive(
            &mut self,
            _from: MailAddr,
            message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
            self.0.push(message);
            Ok(Actions::cont())
        }
    }
    let mut behavior = Compose::new(Seen(Vec::new())).stash(|message| match message {
        0 => StashRoute::Release,
        1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    behavior.transition(User::user(MailAddr(1), 1)).unwrap();
    behavior.transition(User::user(MailAddr(1), 0)).unwrap();
    assert_eq!(behavior.behavior().inner().state().0, vec![0]);
    assert_eq!(behavior.behavior().held(), 1);
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
    let mut machine = Compose::machine(
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
    machine
        .transition(User::user(MailAddr(0), Message::Work(3)))
        .unwrap();
    machine
        .transition(User::user(MailAddr(0), Message::Ready))
        .unwrap();
    assert_eq!(machine.behavior().state(), &[3]);
}

type Child = Pure<Quiet>;

struct Parent;

impl Handler<Never, Births<Child>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Births<Child>, Never> {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Pure::new(Quiet)
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

impl Handler for StashRecording {
    type Addr = MailAddr;
    type Msg = StashMessage;

    fn receive(
        &mut self,
        _from: MailAddr,
        message: StashMessage,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        self.0.push(message.id);
        Ok(Actions::cont())
    }
}

#[tokio::test]
async fn stash_release_delivers_the_trigger_then_drains_the_held_fifo() {
    let release = Arc::new(AtomicBool::new(false));
    let mut behavior = Compose::new(StashRecording(Vec::new())).stash(mutation_stash_route);
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
    assert_eq!(behavior.behavior().inner().state().0, [2, 1]);
    assert_eq!(behavior.behavior().held(), 0);
}

fn continue_on_death(
    _behavior: &mut Watch<Pure<Quiet>>,
    _peer: MailAddr,
    _outcome: &Result<Exit<MailAddr>, Crash>,
) -> Result<Become<MailAddr>, Never> {
    Ok(Step::Continue)
}

#[tokio::test]
async fn an_outer_watcher_forwards_a_different_peer_to_the_inner_watcher() {
    let mut behavior = Watch::new(
        Watch::new(Pure::new(Quiet), MailAddr(1), stop_on_abnormal_death),
        MailAddr(2),
        continue_on_death,
    );
    let actions = behavior
        .transition(WatchEvent::PeerStopped(PeerStopped {
            peer: MailAddr(1),
            outcome: Err(Crash::Failed),
        }))
        .unwrap();
    assert!(matches!(
        actions.become_,
        Step::Stop(Exit::LinkDied(MailAddr(1)))
    ));
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
    let mut machine = Machine::new(
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
    assert!(matches!(stopped.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn receive_timeout_reacts_only_to_its_own_live_timer_id() {
    let mut behavior = Compose::new(Quiet)
        .deadline(Some(Instant::now()), |_| {
            Ok(Step::Stop(Exit::LinkDied(MailAddr(7))))
        })
        .receive_timeout(Duration::from_secs(1), |_| Ok(Actions::stop(Exit::Normal)));
    behavior.init().unwrap();
    let wrong = behavior
        .transition(behavior::ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(
        wrong.become_,
        Step::Stop(Exit::LinkDied(MailAddr(7)))
    ));
    let mut matching_behavior = Compose::new(Quiet)
        .deadline(Some(Instant::now()), |_| {
            Ok(Step::Stop(Exit::LinkDied(MailAddr(7))))
        })
        .receive_timeout(Duration::from_secs(1), |_| Ok(Actions::stop(Exit::Normal)));
    let matching_generation = matching_behavior.init().unwrap().sends.schedules[0].generation;
    let matching = matching_behavior
        .transition(behavior::ReceiveTimeoutEvent::Elapsed(TimerElapsed {
            id: TimerId(1),
            generation: matching_generation,
        }))
        .unwrap();
    assert!(matches!(matching.become_, Step::Stop(Exit::Normal)));
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

impl TimeEvent for TimerAwareEvent {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        Some(Self::Time(event))
    }
}

struct TimerAware;

impl Behavior for TimerAware {
    type Addr = MailAddr;
    type Msg = u64;
    type Event = TimerAwareEvent;
    type Sends = Vec<Delivery<MailAddr, Never>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self) -> Result<Actions<MailAddr, Never, Self::Sends, NoBirths>, Never> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        event: Self::Event,
    ) -> Result<Actions<MailAddr, Never, Self::Sends, NoBirths>, Never> {
        match event {
            TimerAwareEvent::Time(elapsed) => {
                let _ = elapsed;
                Ok(Actions::stop(Exit::LinkDied(MailAddr(8))))
            }
            TimerAwareEvent::User(_) => Ok(Actions::cont()),
        }
    }
}

#[tokio::test]
async fn a_stale_local_receive_timeout_is_consumed_not_forwarded() {
    let mut behavior = Compose::from_behavior(TimerAware)
        .receive_timeout(Duration::from_secs(1), |_| Ok(Actions::stop(Exit::Normal)));
    behavior.init().unwrap();
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
    let mut proxy = Proxy::new(child(0));
    proxy.init().unwrap();
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
        .transition(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(7),
        )))
        .unwrap();
    assert_eq!(forwarded.sends.deliveries[0].to.route(), Route::Child(0));
}

#[test]
fn birth_modes_are_disjoint_and_wrappers_forward_them() {
    requires_no_births(&Compose::new(Quiet));

    let creator = Compose::new(Parent)
        .deadline(None, |_| Ok(Step::Continue))
        .watch(MailAddr(4), stop_on_abnormal_death)
        .stash(|_| StashRoute::Deliver);
    requires_births::<_, Child>(&creator);

    let supervisor = Compose::new(Parent).children((1, child));
    requires_births::<_, Proxy<Child>>(&supervisor);
    requires_worker_events(&supervisor);
    requires_worker_events(&supervisor.deadline(None, |_| Ok(Step::Continue)));
    requires_births::<_, Child>(&Proxy::new(child(0)));
}

fn supervisor(
    strategy: Strategy,
    policy: RestartPolicy,
    budget: u32,
) -> Supervisor<Pure<Parent, Never, Births<Child>, Never>, Child> {
    Supervisor::new(
        Pure::new(Parent),
        |index| u64::try_from(index).unwrap(),
        3,
        child,
        strategy,
        policy,
        budget,
        Duration::MAX,
    )
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the supervision reaction shares its wrapped behavior's controlled-error seat"
)]
fn verify_budget_failure_and_stop(
    _parent: &mut Pure<Parent, Never, Births<Child>, Never>,
    failure: &SupervisionFailure<MailAddr>,
) -> Result<Become<MailAddr>, Never> {
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
    Ok(Step::Stop(failure.into_exit()))
}

#[tokio::test]
async fn supervisor_creates_proxies_and_replacement_is_a_send() {
    let mut supervisor = Compose::new(Parent)
        .children((2, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Transient)
        .within(2, Duration::MAX);
    let initial = supervisor.init().unwrap();
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
        actions.sends.replacement_commands[0].to.route(),
        Route::Child(0)
    );
}

#[tokio::test]
async fn proxy_replacement_creates_a_fresh_incarnation() {
    let mut proxy = Proxy::new(child(0));
    let first = proxy.init().unwrap();
    assert_eq!(first.creates[0].nonce, 0);
    assert_eq!(first.creates[0].kind, CreationKind::Birth);
    proxy.on(CreationResolved::birth(0)).unwrap();
    let second = proxy
        .transition(ProxyEvent::Inner(User::user(
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
        .transition(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(7),
        )))
        .unwrap();
    assert_eq!(forwarded.sends.deliveries[0].to.route(), Route::Child(1));
    assert_eq!(forwarded.sends.deliveries[0].message, 7);
}

#[tokio::test]
async fn proxy_routes_only_after_matching_installation_and_rejection_stays_vacant() {
    let mut proxy = Proxy::new(child(0));
    proxy.init().unwrap();

    let pending = proxy
        .transition(ProxyEvent::Inner(User::user(
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
        .transition(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(2),
        )))
        .unwrap();
    assert!(vacant.sends.deliveries.is_empty());
}

#[tokio::test]
async fn proxy_serializes_attempts_and_rejection_preserves_last_installed_incarnation() {
    let mut proxy = Proxy::new(child(0));
    proxy.init().unwrap();
    proxy.on(CreationResolved::birth(0)).unwrap();
    proxy
        .transition(ProxyEvent::Inner(User::user(
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
        .transition(ProxyEvent::Inner(User::user(
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
        .transition(ProxyEvent::Inner(User::user(
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
    let mut proxy = Proxy::new(child(0));
    let initial = proxy.init().unwrap();
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
        .transition(ProxyEvent::Inner(User::user(
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
    let mut supervisor = Compose::new(Parent)
        .children((1, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Transient)
        .within(1, Duration::MAX);

    let mut initial = supervisor.init().unwrap();
    assert_eq!(initial.creates[0].nonce, 0);
    assert_eq!(initial.sends.child_observations[0].nonce, 0);
    let mut proxy = initial.creates.remove(0).child;

    let worker_birth = proxy.init().unwrap();
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
        restart.sends.replacement_commands[0].to.route(),
        Route::Child(0)
    );

    let command = restart
        .sends
        .replacement_commands
        .into_iter()
        .next()
        .unwrap();
    let replacement = proxy
        .transition(ProxyEvent::Inner(User::user(MailAddr(0), command.message)))
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
    let mut supervisor = supervisor(Strategy::OneForAll, RestartPolicy::Permanent, 3);
    let stopped = supervisor
        .transition(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 1,
            outcome: Err(Crash::Panicked),
            at: Instant::now(),
        }))
        .unwrap();

    assert!(stopped.creates.is_empty());
    assert!(stopped.sends.replacement_commands.is_empty());
    assert!(!supervisor.is_alive(1));
    assert_eq!(stopped.become_, Step::Continue);
}

#[tokio::test]
async fn configured_supervision_failure_reaction_stops_on_budget_denial() {
    let mut supervisor = Compose::new(Parent)
        .children((3, child))
        .restart(Strategy::OneForAll)
        .when(RestartPolicy::Permanent)
        .within(2, Duration::MAX)
        .on_supervision_failure(verify_budget_failure_and_stop);

    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            worker: 1,
            outcome: Err(Crash::Panicked),
            at: Instant::now(),
        }))
        .unwrap();

    assert!(actions.sends.replacement_commands.is_empty());
    assert!(actions.creates.is_empty());
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::SupervisionFailed(
            SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                restarts_in_window: 0,
                replacements_requested: 3,
                maximum_restarts: 2,
            })
        ))
    );
    assert!(!supervisor.behavior().is_alive(1));
}

#[tokio::test]
async fn configured_supervision_failure_reaction_stops_when_stable_proxy_stops() {
    let mut supervisor = Compose::new(Parent)
        .children((1, child))
        .on_supervision_failure(stop_on_supervision_failure);
    let actions = supervisor
        .transition(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Ok(Exit::Normal),
            at: Instant::now(),
        }))
        .unwrap();

    assert_eq!(
        actions.become_,
        Step::Stop(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildStopped
        ))
    );
    assert!(!supervisor.behavior().is_alive(0));
}

#[tokio::test]
async fn restart_policy_ineligibility_is_not_a_supervision_failure() {
    let mut supervisor = Compose::new(Parent)
        .children((1, child))
        .when(RestartPolicy::Temporary)
        .on_supervision_failure(stop_on_supervision_failure);
    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();

    assert_eq!(actions.become_, Step::Continue);
    assert!(!supervisor.behavior().is_alive(0));
}

#[tokio::test]
async fn supervision_failure_exit_is_an_abnormal_transient_worker_outcome() {
    let mut supervisor = supervisor(Strategy::OneForOne, RestartPolicy::Transient, 1);
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
        actions.sends.replacement_commands[0].to.route(),
        Route::Child(0)
    );
}

struct BirthingParent(bool);

impl Handler<Never, Births<Child>, Never> for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Births<Child>, Never> {
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

impl Handler<Never, Births<Child>, Never> for ReplacingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Births<Child>, Never> {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::replacement_incarnation(nonce, nonce - 1, child(0))],
            become_: Step::Continue,
        })
    }
}

#[tokio::test]
async fn supervisor_preserves_and_observes_dynamic_births_once() {
    let mut supervisor = Compose::new(BirthingParent(false))
        .children((0, child))
        .within(1, Duration::MAX);
    let initial = supervisor.init().unwrap();
    assert!(initial.creates.is_empty());

    let born = supervisor
        .transition(UserEvent::user(MailAddr(0), 9))
        .unwrap();
    assert_eq!(born.creates.len(), 1);
    assert_eq!(born.creates[0].nonce, 9);
    assert_eq!(born.creates[0].kind, CreationKind::Birth);
    assert_eq!(born.sends.child_observations.len(), 1);
    assert_eq!(born.sends.child_observations[0].nonce, 9);
    assert_eq!(supervisor.behavior().child_count(), 1);

    let stopped = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 9,
        worker: 9,
        outcome: Err(Crash::Failed),
        at: Instant::now(),
    });
    let replacement = supervisor.transition(stopped).unwrap();
    assert_eq!(replacement.sends.replacement_commands.len(), 1);
    assert_eq!(
        replacement.sends.replacement_commands[0].to.route(),
        Route::Child(9)
    );
}

#[tokio::test]
async fn supervisor_preserves_dynamic_replacement_provenance_when_wrapping_the_child() {
    let mut supervisor = Compose::new(ReplacingParent)
        .children((0, child))
        .within(1, Duration::MAX);
    supervisor.init().unwrap();

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

    let mut one = supervisor(Strategy::OneForOne, RestartPolicy::Transient, 3);
    assert_eq!(
        one.transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        1
    );

    let mut all = supervisor(Strategy::OneForAll, RestartPolicy::Transient, 3);
    assert_eq!(
        all.transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        3
    );

    let mut rest = supervisor(Strategy::RestForOne, RestartPolicy::Transient, 3);
    assert_eq!(
        rest.transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        2
    );

    let mut temporary = supervisor(Strategy::OneForOne, RestartPolicy::Temporary, 3);
    assert!(
        temporary
            .transition(stopped(1))
            .unwrap()
            .sends
            .replacement_commands
            .is_empty()
    );
    assert!(!temporary.is_alive(1));

    let mut denied = supervisor(Strategy::OneForOne, RestartPolicy::Permanent, 0);
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
    let mut behavior = Deadline::new(Pure::new(Quiet), TimerId(0), Some(due), |_| {
        Ok(Step::Stop(Exit::Normal))
    });
    behavior.init().unwrap();
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
    assert!(matches!(fired.become_, Step::Stop(Exit::Normal)));

    let duplicate = behavior
        .transition(DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(duplicate.become_, Step::Continue));
}

#[tokio::test]
async fn workers_macro_hides_a_heterogeneous_child_sum() {
    struct Other;
    impl Handler for Other {
        type Addr = MailAddr;
        type Msg = u64;
        fn receive(
            &mut self,
            _from: MailAddr,
            _message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
            Ok(Actions::cont())
        }
    }
    fn other(_index: usize) -> Pure<Other> {
        Pure::new(Other)
    }

    let (count, build) = workers![(2, Child, child), (1, Pure<Other>, other)];
    assert_eq!(count, 3);
    let mut worker = build(2);
    worker.transition(User::user(MailAddr(0), 7)).unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn nested_time_protocol_preserves_every_schedule(first in 0_u64..10_000, second in 0_u64..10_000) {
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(first);
        let second = origin + Duration::from_nanos(second);
        let inner = Deadline::new(Pure::new(Quiet), TimerId(0), Some(first), |_| Ok(Step::Continue));
        let mut outer = Deadline::new(inner, TimerId(1), Some(second), |_| Ok(Step::Continue));
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let actions = outer.init().unwrap();
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
        let mut behavior = supervisor(strategy, RestartPolicy::Transient, 3);
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
