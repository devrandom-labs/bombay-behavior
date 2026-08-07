use std::time::Duration;

use behavior::{
    Acted, Actions, At, AtEvent, AtId, Base, Behavior, Births, ChildStopped, Crash, Create,
    Delivery, Exit, MailAddr, Move, Never, NoBirths, PeerStopped, Proxy, ProxyCommand, Recipient,
    RestartPolicy, Route, Spec, StashRoute, State, Step, Strategy, Supervising, SupervisionEvent,
    TimeReached, User, UserEvent, WatchEvent, Watching, run, stop_on_abnormal_death, workers,
};
use communication::{Config, channel};
use proptest::prelude::*;
use tokio::runtime::Builder;
use tokio::time::Instant;

struct Quiet;

fn requires_no_births<B: Behavior<Birth = NoBirths>>(_behavior: &B) {}

fn requires_births<B, C>(_behavior: &B)
where
    B: Behavior<Birth = Births<C>>,
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

#[test]
fn actions_are_exactly_the_agha_triple() {
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
async fn at_is_a_typed_clock_actor_protocol() {
    let now = Instant::now();
    let mut behavior = Spec::new(Quiet).at(Some(now), |_| Ok(Step::Continue));

    let initial = behavior.init().await.unwrap();
    assert!(initial.sends.inner.is_empty());
    assert_eq!(initial.sends.own.len(), 1);
    assert_eq!(initial.sends.own[0].at, now);

    let fired = behavior
        .step(AtEvent::Reached(TimeReached {
            id: AtId(0),
            at: now,
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
    let inner = At::new(Base::new(Quiet), Some(early), |_| Ok(Step::Continue));
    let mut outer = At::new(inner, Some(late), |_| Ok(Step::Continue));

    let initial = outer.init().await.unwrap();
    assert_eq!(initial.sends.inner.own[0].at, early);
    assert_eq!(initial.sends.own[0].at, late);

    let early_event = AtEvent::Reached(TimeReached {
        id: AtId(0),
        at: early,
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
    assert_eq!(initial.sends.own[0].message.peer, peer);

    let time = WatchEvent::Inner(AtEvent::Reached(TimeReached {
        id: AtId(0),
        at: due,
    }));
    let actions = behavior.step(time).await.unwrap();
    assert!(matches!(actions.become_, Step::Continue));
}

#[tokio::test]
async fn watching_registers_and_reacts_through_messages() {
    let peer = MailAddr(7);
    let mut behavior = Watching::new(Base::new(Quiet), peer, stop_on_abnormal_death);
    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.sends.own[0].message.peer, peer);

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

#[tokio::test]
async fn supervisor_creates_proxies_and_replacement_is_a_send() {
    let mut supervisor = Spec::new(Parent)
        .children((2, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Transient)
        .within(2, Duration::MAX);
    let initial = supervisor.init().await.unwrap();
    assert_eq!(initial.creates.len(), 2);
    assert_eq!(initial.sends.own.inner.len(), 2);
    assert!(
        initial
            .sends
            .own
            .inner
            .iter()
            .all(|send| send.to.route() == Route::Service)
    );

    let event = SupervisionEvent::ChildStopped(ChildStopped {
        nonce: 0,
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
    let second = proxy
        .step(User::user(MailAddr(0), ProxyCommand::Replace(child(0))))
        .await
        .unwrap();
    assert_eq!(second.creates[0].nonce, 1);

    let forwarded = proxy
        .step(User::user(MailAddr(0), ProxyCommand::Forward(7)))
        .await
        .unwrap();
    assert_eq!(forwarded.sends[0].to.route(), Route::Child(1));
    assert_eq!(forwarded.sends[0].message, 7);
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
            creates: vec![Create {
                nonce,
                child: child(0),
            }],
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
    assert_eq!(born.sends.own.inner.len(), 1);
    assert_eq!(born.sends.own.inner[0].message.nonce, 9);
    assert_eq!(supervisor.behavior().child_count(), 1);

    let stopped = SupervisionEvent::ChildStopped(ChildStopped {
        nonce: 9,
        outcome: Err(Crash::Failed),
        at: Instant::now(),
    });
    let replacement = supervisor.step(stopped).await.unwrap();
    assert_eq!(replacement.sends.own.own.len(), 1);
    assert_eq!(replacement.sends.own.own[0].to.route(), Route::Child(9));
}

#[tokio::test]
async fn supervision_strategy_policy_and_budget_are_pure_send_decisions() {
    let at = Instant::now();
    let stopped = |nonce| {
        SupervisionEvent::ChildStopped(ChildStopped {
            nonce,
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
    let mut behavior = At::new(Base::new(Quiet), Some(due), |_| {
        Ok(Step::Stop(Exit::Normal))
    });
    behavior.init().await.unwrap();
    let stale = AtEvent::Reached(TimeReached {
        id: AtId(0),
        at: due - Duration::from_secs(1),
    });
    let ignored = behavior.step(stale).await.unwrap();
    assert!(matches!(ignored.become_, Step::Continue));

    let fired = behavior
        .step(AtEvent::Reached(TimeReached {
            id: AtId(0),
            at: due,
        }))
        .await
        .unwrap();
    assert!(matches!(fired.become_, Step::Stop(Exit::Normal)));

    let duplicate = behavior
        .step(AtEvent::Reached(TimeReached {
            id: AtId(0),
            at: due,
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
        let inner = At::new(Base::new(Quiet), Some(first), |_| Ok(Step::Continue));
        let mut outer = At::new(inner, Some(second), |_| Ok(Step::Continue));
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
        let event = SupervisionEvent::ChildStopped(ChildStopped {
            nonce: u64::try_from(dead).unwrap(),
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        });
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let actions = runtime.block_on(behavior.step(event)).unwrap();
        prop_assert_eq!(actions.sends.own.own.len(), expected);
        prop_assert!(actions.creates.is_empty());
    }
}
