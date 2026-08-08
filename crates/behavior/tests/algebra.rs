use std::time::Duration;

use behavior::{
    Acted, Actions, At, AtEvent, Base, Become, Behavior, Births, ChildStopped, Crash, Create,
    CreationKind, Delivery, Exit, MailAddr, Move, Never, NoBirths, ObserveChild, PeerStopped,
    Proxy, ProxyCommand, Recipient, RestartDenial, RestartPolicy, Route, ServiceSends,
    ShutdownEvent, ShutdownRequested, Spec, StashRoute, State, Step, Strategy, Supervising,
    SupervisionEvent, SupervisionFailure, SupervisionFailureReason, TimerElapsed, TimerGeneration,
    TimerId, User, UserEvent, WatchEvent, Watching, WorkerEvent, WorkerStopped, run,
    stop_on_abnormal_death, stop_on_supervision_failure, workers,
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
    impl<A: behavior::Address> RouteSends<A> for ServiceSends<ObserveChild<A>> {}

    fn requires_route_sends<A: behavior::Address, S: RouteSends<A>>() {}

    requires_route_sends::<MailAddr, Vec<Delivery<MailAddr, ObserveChild<MailAddr>>>>();
    requires_route_sends::<MailAddr, ServiceSends<ObserveChild<MailAddr>>>();
}

fn requires_births<B, C>(_behavior: &B)
where
    B: Behavior<Birth = Births<C>>,
{
}

fn requires_worker_events<B>(_behavior: &B)
where
    B: Behavior,
    B::Event: WorkerEvent<B::Addr>,
{
}

impl State for Quiet {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

struct ShutdownParent;

impl State<u64, Births<Base<Quiet>>> for ShutdownParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Births<Base<Quiet>>, Never> {
        Ok(Actions::cont())
    }
}

#[allow(
    clippy::type_complexity,
    clippy::unnecessary_wraps,
    reason = "the shutdown reaction must expose the complete typed Actions and error seats"
)]
fn finalize_parent(
    _behavior: &mut Base<ShutdownParent, u64, Births<Base<Quiet>>>,
    _request: ShutdownRequested,
) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Births<Base<Quiet>>, Never> {
    Ok(Actions {
        sends: vec![Delivery::new(Recipient::global(MailAddr(9)), 42)],
        creates: vec![Create::birth(7, Base::new(Quiet))],
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
    let mut behavior = Spec::new(Quiet).stop_on_shutdown();
    let event = <_ as ShutdownEvent>::shutdown_requested(ShutdownRequested).unwrap();
    let actions = behavior.step(event).await.unwrap();

    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn final_shutdown_fold_preserves_effects_and_forces_normal_stop() {
    let mut behavior = Spec::new(ShutdownParent).finalize_on_shutdown(finalize_parent);
    let event = <_ as ShutdownEvent>::shutdown_requested(ShutdownRequested).unwrap();
    let actions = behavior.step(event).await.unwrap();

    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].message, 42);
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.creates[0].nonce, 7);
    assert!(matches!(actions.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn outer_combinators_preserve_the_shutdown_lane() {
    let mut behavior = Spec::new(Quiet)
        .stop_on_shutdown()
        .at(None, |_| Ok(Step::Continue))
        .watch(MailAddr(8), stop_on_abnormal_death);
    let event = <_ as ShutdownEvent>::shutdown_requested(ShutdownRequested).unwrap();
    let actions = behavior.step(event).await.unwrap();

    assert!(matches!(actions.become_, Step::Stop(Exit::Normal)));
}

#[tokio::test]
async fn shutdown_composition_preserves_inner_initialization_effects() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior = Spec::new(Quiet)
        .at(Some(due), |_| Ok(Step::Continue))
        .stop_on_shutdown();
    let initial = behavior.init().await.unwrap();

    assert_eq!(initial.sends.own.len(), 1);
    assert_eq!(initial.sends.own[0].at, due);
    assert!(matches!(initial.become_, Step::Continue));
}

#[tokio::test]
async fn at_is_a_typed_clock_actor_protocol() {
    let now = Instant::now();
    let mut behavior = Spec::new(Quiet).at(Some(now), |_| Ok(Step::Continue));

    let initial = behavior.init().await.unwrap();
    assert!(initial.sends.inner.is_empty());
    assert_eq!(initial.sends.own.len(), 1);
    assert_eq!(initial.sends.own[0].at, now);

    let fired = behavior
        .step(AtEvent::Reached(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(fired.sends.own.is_empty());
}

#[tokio::test]
async fn driver_interprets_initial_effect_before_receiving() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = Spec::new(Quiet).at(Some(due), |_| Ok(Step::Continue));
    let (control, user, mailbox) = channel::<Never, u64>(Config::new(1));
    drop(user);
    drop(control);

    let transcript = run(behavior, mailbox, MailAddr(0)).await.unwrap();
    assert_eq!(transcript.sends.own.len(), 1);
    assert_eq!(transcript.sends.own[0].at, due);
    assert_eq!(transcript.exit, Exit::Collected);
}

#[tokio::test]
async fn nested_at_composition_routes_stale_and_matching_events() {
    let early = Instant::now() + Duration::from_secs(1);
    let late = early + Duration::from_secs(1);
    let inner = At::new(Base::new(Quiet), TimerId(0), Some(early), |_| {
        Ok(Step::Continue)
    });
    let mut outer = At::new(inner, TimerId(1), Some(late), |_| Ok(Step::Continue));

    let initial = outer.init().await.unwrap();
    assert_eq!(initial.sends.inner.own[0].id, TimerId(0));
    assert_eq!(initial.sends.own[0].id, TimerId(1));
    assert_eq!(initial.sends.inner.own[0].at, early);
    assert_eq!(initial.sends.own[0].at, late);

    let early_event = AtEvent::Reached(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    });
    let actions = outer.step(early_event).await.unwrap();
    assert!(actions.sends.inner.own.is_empty());
}

#[tokio::test]
async fn spec_hides_composed_protocols_without_losing_their_effects() {
    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(8);
    let mut behavior = Spec::new(Quiet)
        .at(Some(due), |_| Ok(Step::Continue))
        .watch(peer, stop_on_abnormal_death)
        .stash(|_| StashRoute::Deliver);

    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.sends.inner.own[0].at, due);
    assert_eq!(initial.sends.own[0].peer, peer);

    let time = WatchEvent::Inner(AtEvent::Reached(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    }));
    let actions = behavior.step(time).await.unwrap();
    assert!(matches!(actions.become_, Step::Continue));
}

#[tokio::test]
async fn watching_registers_and_reacts_through_messages() {
    let peer = MailAddr(7);
    let mut behavior = Watching::new(Base::new(Quiet), peer, stop_on_abnormal_death);
    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.sends.own[0].peer, peer);

    let stopped = WatchEvent::PeerStopped(PeerStopped {
        peer,
        outcome: Err(Crash::Failed),
    });
    let actions = behavior.step(stopped).await.unwrap();
    assert!(matches!(actions.become_, Step::Stop(Exit::LinkDied(p)) if p == peer));
}

#[tokio::test]
async fn stashing_is_local_state_and_replay() {
    struct Seen(Vec<u64>);
    impl State for Seen {
        type Addr = MailAddr;
        type Msg = u64;
        fn handle(
            &mut self,
            _from: MailAddr,
            message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
            self.0.push(message);
            Ok(Actions::cont())
        }
    }
    let mut behavior = Spec::new(Seen(Vec::new())).stash(|message| match message {
        0 => StashRoute::Release,
        1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    behavior.step(User::user(MailAddr(1), 1)).await.unwrap();
    behavior.step(User::user(MailAddr(1), 0)).await.unwrap();
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
    let mut machine = Spec::machine(
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
        .step(User::user(MailAddr(0), Message::Work(3)))
        .await
        .unwrap();
    machine
        .step(User::user(MailAddr(0), Message::Ready))
        .await
        .unwrap();
    assert_eq!(machine.behavior().state(), &[3]);
}

type Child = Base<Quiet>;

struct Parent;

impl State<Never, Births<Child>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Births<Child>, Never> {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Base::new(Quiet)
}

#[test]
fn birth_modes_are_disjoint_and_wrappers_forward_them() {
    requires_no_births(&Spec::new(Quiet));

    let creator = Spec::new(Parent)
        .at(None, |_| Ok(Step::Continue))
        .watch(MailAddr(4), stop_on_abnormal_death)
        .stash(|_| StashRoute::Deliver);
    requires_births::<_, Child>(&creator);

    let supervisor = Spec::new(Parent).children((1, child));
    requires_births::<_, Proxy<Child>>(&supervisor);
    requires_worker_events(&supervisor);
    requires_worker_events(&supervisor.at(None, |_| Ok(Step::Continue)));
    requires_births::<_, Child>(&Proxy::new(child(0)));
}

fn supervisor(
    strategy: Strategy,
    policy: RestartPolicy,
    budget: u32,
) -> Supervising<Base<Parent, Never, Births<Child>, Never>, Child> {
    Supervising::new(
        Base::new(Parent),
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
    _parent: &mut Base<Parent, Never, Births<Child>, Never>,
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
    let mut supervisor = Spec::new(Parent)
        .children((2, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Transient)
        .within(2, Duration::MAX);
    let initial = supervisor.init().await.unwrap();
    assert_eq!(initial.creates.len(), 2);
    assert!(
        initial
            .creates
            .iter()
            .all(|create| create.kind == CreationKind::Birth)
    );
    assert_eq!(initial.sends.own.inner.len(), 2);
    assert_eq!(initial.sends.own.inner[0].nonce, 0);
    assert_eq!(initial.sends.own.inner[1].nonce, 1);

    let event = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 0,
        outcome: Err(Crash::Failed),
        at: Instant::now(),
    });
    let actions = supervisor.step(event).await.unwrap();
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends.own.own.len(), 1);
    assert_eq!(actions.sends.own.own[0].to.route(), Route::Child(0));
}

#[tokio::test]
async fn proxy_replacement_creates_a_fresh_incarnation() {
    let mut proxy = Proxy::new(child(0));
    let first = proxy.init().await.unwrap();
    assert_eq!(first.creates[0].nonce, 0);
    assert_eq!(first.creates[0].kind, CreationKind::Birth);
    let second = proxy
        .step(SupervisionEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .await
        .unwrap();
    assert!(second.creates.is_empty());

    let second = proxy
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .await
        .unwrap();
    assert_eq!(second.creates[0].nonce, 1);
    assert_eq!(second.creates[0].kind, CreationKind::ReplacementIncarnation);
    assert_eq!(second.sends.own.inner[0].nonce, 1);
    assert_eq!(second.sends.own.own[0].outcome, Err(Crash::Failed));

    let forwarded = proxy
        .step(SupervisionEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(7),
        )))
        .await
        .unwrap();
    assert_eq!(forwarded.sends.inner[0].to.route(), Route::Child(1));
    assert_eq!(forwarded.sends.inner[0].message, 7);
}

#[tokio::test]
async fn idle_proxy_marks_an_immediate_successor_as_a_replacement_incarnation() {
    let mut proxy = Proxy::new(child(0));
    let initial = proxy.init().await.unwrap();
    assert_eq!(initial.creates[0].kind, CreationKind::Birth);

    let stopped = proxy
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .await
        .unwrap();
    assert!(stopped.creates.is_empty());

    let replacement = proxy
        .step(SupervisionEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .await
        .unwrap();
    assert_eq!(replacement.creates[0].nonce, 1);
    assert_eq!(
        replacement.creates[0].kind,
        CreationKind::ReplacementIncarnation
    );
}

#[tokio::test]
async fn stable_proxy_reports_worker_stop_and_creates_fresh_replacement() {
    let at = Instant::now();
    let mut supervisor = Spec::new(Parent)
        .children((1, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Transient)
        .within(1, Duration::MAX);

    let mut initial = supervisor.init().await.unwrap();
    assert_eq!(initial.creates[0].nonce, 0);
    assert_eq!(initial.sends.own.inner[0].nonce, 0);
    let mut proxy = initial.creates.remove(0).child;

    let worker_birth = proxy.init().await.unwrap();
    assert_eq!(worker_birth.creates[0].nonce, 0);
    assert_eq!(worker_birth.creates[0].kind, CreationKind::Birth);
    assert_eq!(worker_birth.sends.own.inner[0].nonce, 0);

    let worker_stop = proxy
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Panicked),
            at,
        }))
        .await
        .unwrap();
    assert!(worker_stop.creates.is_empty());
    assert!(matches!(worker_stop.become_, Step::Continue));
    assert_eq!(worker_stop.sends.own.own[0].outcome, Err(Crash::Panicked));

    let restart = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            outcome: worker_stop.sends.own.own[0].outcome,
            at: worker_stop.sends.own.own[0].at,
        }))
        .await
        .unwrap();
    assert!(restart.creates.is_empty());
    assert_eq!(restart.sends.own.own.len(), 1);
    assert_eq!(restart.sends.own.own[0].to.route(), Route::Child(0));

    let command = restart.sends.own.own.into_iter().next().unwrap();
    let replacement = proxy
        .step(SupervisionEvent::Inner(User::user(
            MailAddr(0),
            command.message,
        )))
        .await
        .unwrap();
    assert_eq!(replacement.creates[0].nonce, 1);
    assert_eq!(
        replacement.creates[0].kind,
        CreationKind::ReplacementIncarnation
    );
    assert_eq!(replacement.sends.own.inner[0].nonce, 1);
    assert!(matches!(replacement.become_, Step::Continue));
}

#[tokio::test]
async fn stopped_proxy_is_retired_without_sending_to_its_dead_address() {
    let mut supervisor = supervisor(Strategy::OneForAll, RestartPolicy::Permanent, 3);
    let stopped = supervisor
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 1,
            outcome: Err(Crash::Panicked),
            at: Instant::now(),
        }))
        .await
        .unwrap();

    assert!(stopped.creates.is_empty());
    assert!(stopped.sends.own.own.is_empty());
    assert!(!supervisor.is_alive(1));
    assert_eq!(stopped.become_, Step::Continue);
}

#[tokio::test]
async fn configured_supervision_failure_reaction_stops_on_budget_denial() {
    let mut supervisor = Spec::new(Parent)
        .children((3, child))
        .restart(Strategy::OneForAll)
        .when(RestartPolicy::Permanent)
        .within(2, Duration::MAX)
        .on_supervision_failure(verify_budget_failure_and_stop);

    let actions = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            outcome: Err(Crash::Panicked),
            at: Instant::now(),
        }))
        .await
        .unwrap();

    assert!(actions.sends.own.own.is_empty());
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
    let mut supervisor = Spec::new(Parent)
        .children((1, child))
        .on_supervision_failure(stop_on_supervision_failure);
    let actions = supervisor
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Ok(Exit::Normal),
            at: Instant::now(),
        }))
        .await
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
    let mut supervisor = Spec::new(Parent)
        .children((1, child))
        .when(RestartPolicy::Temporary)
        .on_supervision_failure(stop_on_supervision_failure);
    let actions = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .await
        .unwrap();

    assert_eq!(actions.become_, Step::Continue);
    assert!(!supervisor.behavior().is_alive(0));
}

#[tokio::test]
async fn supervision_failure_exit_is_an_abnormal_transient_worker_outcome() {
    let mut supervisor = supervisor(Strategy::OneForOne, RestartPolicy::Transient, 1);
    let actions = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            outcome: Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildStopped,
            )),
            at: Instant::now(),
        }))
        .await
        .unwrap();

    assert_eq!(actions.sends.own.own.len(), 1);
    assert_eq!(actions.sends.own.own[0].to.route(), Route::Child(0));
}

struct BirthingParent(bool);

impl State<Never, Births<Child>, Never> for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
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

impl State<Never, Births<Child>, Never> for ReplacingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Births<Child>, Never> {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::replacement_incarnation(nonce, child(0))],
            become_: Step::Continue,
        })
    }
}

#[tokio::test]
async fn supervisor_preserves_and_observes_dynamic_births_once() {
    let mut supervisor = Spec::new(BirthingParent(false))
        .children((0, child))
        .within(1, Duration::MAX);
    let initial = supervisor.init().await.unwrap();
    assert!(initial.creates.is_empty());

    let born = supervisor
        .step(UserEvent::user(MailAddr(0), 9))
        .await
        .unwrap();
    assert_eq!(born.creates.len(), 1);
    assert_eq!(born.creates[0].nonce, 9);
    assert_eq!(born.creates[0].kind, CreationKind::Birth);
    assert_eq!(born.sends.own.inner.len(), 1);
    assert_eq!(born.sends.own.inner[0].nonce, 9);
    assert_eq!(supervisor.behavior().child_count(), 1);

    let stopped = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 9,
        outcome: Err(Crash::Failed),
        at: Instant::now(),
    });
    let replacement = supervisor.step(stopped).await.unwrap();
    assert_eq!(replacement.sends.own.own.len(), 1);
    assert_eq!(replacement.sends.own.own[0].to.route(), Route::Child(9));
}

#[tokio::test]
async fn supervisor_preserves_dynamic_replacement_provenance_when_wrapping_the_child() {
    let mut supervisor = Spec::new(ReplacingParent)
        .children((0, child))
        .within(1, Duration::MAX);
    supervisor.init().await.unwrap();

    let replacement = supervisor
        .step(UserEvent::user(MailAddr(0), 9))
        .await
        .unwrap();
    assert_eq!(replacement.creates.len(), 1);
    assert_eq!(replacement.creates[0].nonce, 9);
    assert_eq!(
        replacement.creates[0].kind,
        CreationKind::ReplacementIncarnation
    );
}

#[tokio::test]
async fn supervision_strategy_policy_and_budget_are_pure_send_decisions() {
    let at = Instant::now();
    let stopped = |nonce| {
        SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: nonce,
            outcome: Err(Crash::Failed),
            at,
        })
    };

    let mut one = supervisor(Strategy::OneForOne, RestartPolicy::Transient, 3);
    assert_eq!(one.step(stopped(1)).await.unwrap().sends.own.own.len(), 1);

    let mut all = supervisor(Strategy::OneForAll, RestartPolicy::Transient, 3);
    assert_eq!(all.step(stopped(1)).await.unwrap().sends.own.own.len(), 3);

    let mut rest = supervisor(Strategy::RestForOne, RestartPolicy::Transient, 3);
    assert_eq!(rest.step(stopped(1)).await.unwrap().sends.own.own.len(), 2);

    let mut temporary = supervisor(Strategy::OneForOne, RestartPolicy::Temporary, 3);
    assert!(
        temporary
            .step(stopped(1))
            .await
            .unwrap()
            .sends
            .own
            .own
            .is_empty()
    );
    assert!(!temporary.is_alive(1));

    let mut denied = supervisor(Strategy::OneForOne, RestartPolicy::Permanent, 0);
    assert!(
        denied
            .step(stopped(1))
            .await
            .unwrap()
            .sends
            .own
            .own
            .is_empty()
    );
}

#[tokio::test]
async fn stale_time_events_do_not_fire_or_reschedule() {
    let due = Instant::now() + Duration::from_secs(2);
    let mut behavior = At::new(Base::new(Quiet), TimerId(0), Some(due), |_| {
        Ok(Step::Stop(Exit::Normal))
    });
    behavior.init().await.unwrap();
    let stale = AtEvent::Reached(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(1),
    });
    let ignored = behavior.step(stale).await.unwrap();
    assert!(matches!(ignored.become_, Step::Continue));

    let fired = behavior
        .step(AtEvent::Reached(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(matches!(fired.become_, Step::Stop(Exit::Normal)));

    let duplicate = behavior
        .step(AtEvent::Reached(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .await
        .unwrap();
    assert!(matches!(duplicate.become_, Step::Continue));
}

#[tokio::test]
async fn workers_macro_hides_a_heterogeneous_child_sum() {
    struct Other;
    impl State for Other {
        type Addr = MailAddr;
        type Msg = u64;
        fn handle(
            &mut self,
            _from: MailAddr,
            _message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
            Ok(Actions::cont())
        }
    }
    fn other(_index: usize) -> Base<Other> {
        Base::new(Other)
    }

    let (count, build) = workers![(2, Child, child), (1, Base<Other>, other)];
    assert_eq!(count, 3);
    let mut worker = build(2);
    worker.step(User::user(MailAddr(0), 7)).await.unwrap();
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn nested_time_protocol_preserves_every_schedule(first in 0_u64..10_000, second in 0_u64..10_000) {
        let origin = Instant::now();
        let first = origin + Duration::from_nanos(first);
        let second = origin + Duration::from_nanos(second);
        let inner = At::new(Base::new(Quiet), TimerId(0), Some(first), |_| Ok(Step::Continue));
        let mut outer = At::new(inner, TimerId(1), Some(second), |_| Ok(Step::Continue));
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let actions = runtime.block_on(outer.init()).unwrap();
        prop_assert_eq!(actions.sends.inner.own[0].at, first);
        prop_assert_eq!(actions.sends.own[0].at, second);
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
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        });
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let actions = runtime.block_on(behavior.step(event)).unwrap();
        prop_assert_eq!(actions.sends.own.own.len(), expected);
        prop_assert!(actions.creates.is_empty());
    }
}
