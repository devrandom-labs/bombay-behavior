//! Composition routing attacks: every wrapper ordering preserves its own
//! initialization protocol at the exact nesting depth; non-user lanes
//! (time, peer observation) bypass a stash buffer while user messages are
//! intercepted; watch reactions are re-invocable and deterministic.

use std::time::Duration;

use behavior::{
    Acted, Actions, AtEvent, AtGeneration, AtId, Base, Behavior, ChildStopped, Crash, Delivery,
    Exit, MailAddr, Never, PeerStopped, Recipient, Route, Spec, StashRoute, State, Step,
    SupervisionEvent, TimeReached, User, UserEvent, WatchEvent, stop_on_abnormal_death,
};
use tokio::time::Instant;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

impl State<u8, behavior::NoBirths, Never> for Recorder {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

type Child = Base<Recorder, u8>;

fn child(_index: usize) -> Child {
    Base::new(Recorder::default())
}

const PEER: MailAddr = MailAddr(44);

fn at<T: Behavior>(behavior: Spec<T>, when: Instant) -> Spec<behavior::At<T>> {
    behavior.at(Some(when), |_| Ok(Step::Continue))
}

/// Every ordering of {at, watch, at} preserves each layer's own initial
/// protocol at exactly its nesting depth: outermost send product carries the
/// outermost schedule, and so on inward.
#[tokio::test]
async fn all_wrapper_permutations_preserve_init_protocol_nesting() {
    let first = Instant::now() + Duration::from_secs(1);
    let second = first + Duration::from_secs(1);

    // Chain .at(T1).watch(p).at(T2): outermost At owns T2, Watch owns p,
    // innermost At owns T1.
    let mut c1 = at(Spec::new(Recorder::default()), first)
        .watch(PEER, stop_on_abnormal_death)
        .at(Some(second), |_| Ok(Step::Continue));
    let i1 = c1.init().await.unwrap();
    assert_eq!(i1.sends.own[0].at, second);
    assert_eq!(i1.sends.inner.own[0].peer, PEER);
    assert_eq!(i1.sends.inner.inner.own[0].at, first);

    // Chain .at(T1).at(T2).watch(p): Watch owns the outer product.
    let mut c2 = at(Spec::new(Recorder::default()), first)
        .at(Some(second), |_| Ok(Step::Continue))
        .watch(PEER, stop_on_abnormal_death);
    let i2 = c2.init().await.unwrap();
    assert_eq!(i2.sends.own[0].peer, PEER);
    assert_eq!(i2.sends.inner.own[0].at, second);
    assert_eq!(i2.sends.inner.inner.own[0].at, first);

    // Chain .watch(p).at(T1).at(T2).
    let mut c3 = Spec::new(Recorder::default())
        .watch(PEER, stop_on_abnormal_death)
        .at(Some(first), |_| Ok(Step::Continue))
        .at(Some(second), |_| Ok(Step::Continue));
    let i3 = c3.init().await.unwrap();
    assert_eq!(i3.sends.own[0].at, second);
    assert_eq!(i3.sends.inner.own[0].at, first);
    assert_eq!(i3.sends.inner.inner.own[0].peer, PEER);

    // Chain .at(T2).at(T1).watch(p).
    let mut c4 = at(Spec::new(Recorder::default()), second)
        .at(Some(first), |_| Ok(Step::Continue))
        .watch(PEER, stop_on_abnormal_death);
    let i4 = c4.init().await.unwrap();
    assert_eq!(i4.sends.own[0].peer, PEER);
    assert_eq!(i4.sends.inner.own[0].at, first);
    assert_eq!(i4.sends.inner.inner.own[0].at, second);

    // Chain .at(T2).watch(p).at(T1).
    let mut c5 = at(Spec::new(Recorder::default()), second)
        .watch(PEER, stop_on_abnormal_death)
        .at(Some(first), |_| Ok(Step::Continue));
    let i5 = c5.init().await.unwrap();
    assert_eq!(i5.sends.own[0].at, first);
    assert_eq!(i5.sends.inner.own[0].peer, PEER);
    assert_eq!(i5.sends.inner.inner.own[0].at, second);

    // Chain .watch(p).at(T2).at(T1).
    let mut c6 = Spec::new(Recorder::default())
        .watch(PEER, stop_on_abnormal_death)
        .at(Some(second), |_| Ok(Step::Continue))
        .at(Some(first), |_| Ok(Step::Continue));
    let i6 = c6.init().await.unwrap();
    assert_eq!(i6.sends.own[0].at, first);
    assert_eq!(i6.sends.inner.own[0].at, second);
    assert_eq!(i6.sends.inner.inner.own[0].peer, PEER);
}

/// A stash layer contributes no initialization sends and shifts nothing:
/// the at/watch markers keep their nesting depths relative to each other.
#[tokio::test]
async fn stash_layer_contributes_no_init_sends() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior = Spec::new(Recorder::default())
        .stash(|_| StashRoute::Deliver)
        .watch(PEER, stop_on_abnormal_death)
        .at(Some(due), |_| Ok(Step::Continue));
    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.sends.own[0].at, due);
    assert_eq!(initial.sends.inner.own[0].peer, PEER);
    assert!(initial.sends.inner.inner.is_empty());
}

/// In an At∘Watch∘Stash stack, only the user lane enters the stash buffer:
/// Reached and `PeerStopped` events pass through to their layer untouched.
#[tokio::test]
async fn environment_lanes_bypass_stash_while_user_lane_is_intercepted() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior = Spec::new(Recorder::default())
        .stash(|_| StashRoute::Stash)
        .watch(PEER, stop_on_abnormal_death)
        .at(Some(due), |_| Ok(Step::Continue));
    behavior.init().await.unwrap();

    // Time lane: fires through the stash layer, nothing stashed.
    let reached = AtEvent::Reached(TimeReached {
        id: AtId(0),
        generation: AtGeneration(0),
        at: due,
    });
    let fired = behavior.step(reached).await.unwrap();
    assert!(matches!(fired.become_, Step::Continue));
    assert_eq!(behavior.behavior().inner().inner().held(), 0);

    // Peer lane: matching peer death stops the fold through the stash layer.
    let peer = WatchEvent::PeerStopped(PeerStopped {
        peer: PEER,
        outcome: Err(Crash::Failed),
    });
    let died = behavior.step(AtEvent::Inner(peer)).await.unwrap();
    assert!(matches!(
        died.become_,
        Step::Stop(Exit::LinkDied(p)) if p == PEER
    ));
    assert_eq!(behavior.behavior().inner().inner().held(), 0);

    // User lane: intercepted by the stash buffer.
    let user = User::user(MailAddr(7), 3);
    behavior
        .step(AtEvent::Inner(WatchEvent::Inner(user)))
        .await
        .unwrap();
    assert_eq!(behavior.behavior().inner().inner().held(), 1);
    assert!(
        behavior
            .behavior()
            .inner()
            .inner()
            .inner()
            .state()
            .seen
            .is_empty()
    );
}

/// Watching does not latch: stepping after a Stop is allowed and re-invokes
/// the reaction on each matching death; ordinary user messages still fold.
#[tokio::test]
async fn watch_reaction_reinvokes_on_each_death_and_fold_continues() {
    let mut behavior = Spec::new(Recorder::default()).watch(PEER, stop_on_abnormal_death);
    behavior.init().await.unwrap();

    let death = WatchEvent::PeerStopped(PeerStopped {
        peer: PEER,
        outcome: Err(Crash::Failed),
    });
    let first = behavior.step(death.clone()).await.unwrap();
    assert!(matches!(
        first.become_,
        Step::Stop(Exit::LinkDied(p)) if p == PEER
    ));

    let second = behavior.step(death).await.unwrap();
    assert!(matches!(
        second.become_,
        Step::Stop(Exit::LinkDied(p)) if p == PEER
    ));

    // The fold is still usable: a user message after the stop is processed.
    let actions = behavior
        .step(UserEvent::user(MailAddr(2), 9))
        .await
        .unwrap();
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(behavior.behavior().inner().state().seen, [(MailAddr(2), 9)]);
}

/// An unrelated peer's death routed through a watch-of-watch reaches the
/// inner watcher, not the outer reaction; both layers keep their own peer.
#[tokio::test]
async fn watch_of_watch_routes_each_peer_to_its_own_layer() {
    let inner_peer = MailAddr(1);
    let outer_peer = MailAddr(2);
    let mut behavior = Spec::new(Recorder::default())
        .watch(inner_peer, stop_on_abnormal_death)
        .watch(outer_peer, stop_on_abnormal_death);
    behavior.init().await.unwrap();

    // Outer peer death: outer layer stops.
    let outer_death = WatchEvent::PeerStopped(PeerStopped {
        peer: outer_peer,
        outcome: Err(Crash::Failed),
    });
    let outer = behavior.step(outer_death).await.unwrap();
    assert!(matches!(
        outer.become_,
        Step::Stop(Exit::LinkDied(p)) if p == outer_peer
    ));

    // Inner peer death: forwarded to the inner watcher.
    let inner_death = WatchEvent::PeerStopped(PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    });
    let inner = behavior.step(inner_death).await.unwrap();
    assert!(matches!(
        inner.become_,
        Step::Stop(Exit::LinkDied(p)) if p == inner_peer
    ));
}

/// An `At` constructed with `None` schedules nothing and is inert to every
/// Reached event: the reaction never fires.
#[tokio::test]
async fn unscheduled_at_is_inert_to_reached_events() {
    let mut behavior = Spec::new(Recorder::default()).at(None, |_| Ok(Step::Stop(Exit::Normal)));
    let initial = behavior.init().await.unwrap();
    assert!(initial.sends.own.is_empty());

    for (id, at) in [
        (AtId(0), Instant::now()),
        (AtId(1), Instant::now() + Duration::from_secs(9)),
    ] {
        let actions = behavior
            .step(AtEvent::Reached(TimeReached {
                id,
                generation: AtGeneration(0),
                at,
            }))
            .await
            .unwrap();
        assert_eq!(actions.become_, Step::Continue);
    }
}

/// `stop_on_abnormal_death` classifies outcomes: Normal and Collected keep
/// the fold alive; `LinkDied` and crashes stop it carrying the peer address.
#[tokio::test]
async fn abnormal_death_reaction_outcome_classes() {
    let mut behavior = Spec::new(Recorder::default()).watch(PEER, stop_on_abnormal_death);
    behavior.init().await.unwrap();

    let outcome = |outcome| {
        WatchEvent::PeerStopped(PeerStopped {
            peer: PEER,
            outcome,
        })
    };
    let normal = behavior.step(outcome(Ok(Exit::Normal))).await.unwrap();
    assert_eq!(normal.become_, Step::Continue);
    let collected = behavior.step(outcome(Ok(Exit::Collected))).await.unwrap();
    assert_eq!(collected.become_, Step::Continue);
    let linked = behavior
        .step(outcome(Ok(Exit::LinkDied(MailAddr(3)))))
        .await
        .unwrap();
    assert!(matches!(
        linked.become_,
        Step::Stop(Exit::LinkDied(p)) if p == PEER
    ));
    for crash in [
        Crash::Failed,
        Crash::EnvironmentFailed,
        Crash::Panicked,
        Crash::Cancelled,
    ] {
        let crashed = behavior.step(outcome(Err(crash))).await.unwrap();
        assert!(matches!(
            crashed.become_,
            Step::Stop(Exit::LinkDied(p)) if p == PEER
        ));
    }
}

/// Supervision over a watched parent: the fleet's observe sends and the
/// watch's observe send live in their own product lanes, both event lanes
/// (peer death, child death) route to their own layer, and a peer death can
/// stop the whole supervised fold.
#[tokio::test]
async fn supervision_preserves_inner_watch_routing() {
    struct Parent;
    impl State<Never, behavior::Births<Child>, Never> for Parent {
        type Addr = MailAddr;
        type Msg = u64;

        fn handle(
            &mut self,
            _from: MailAddr,
            _message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Never>
        {
            Ok(Actions::cont())
        }
    }

    let mut behavior = Spec::new(Parent)
        .watch(PEER, stop_on_abnormal_death)
        .children((2, child));
    let initial = behavior.init().await.unwrap();
    assert_eq!(initial.creates.len(), 2);
    assert_eq!(initial.sends.own.inner.len(), 2); // observe-child x2
    assert_eq!(initial.sends.own.inner[0].nonce, 0);
    assert_eq!(initial.sends.inner.own.len(), 1); // observe-peer
    assert_eq!(initial.sends.inner.own[0].peer, PEER);

    // Peer lane: the watch reaction stops the supervised fold.
    let died = behavior
        .step(SupervisionEvent::Inner(WatchEvent::PeerStopped(
            PeerStopped {
                peer: PEER,
                outcome: Err(Crash::Failed),
            },
        )))
        .await
        .unwrap();
    assert!(matches!(
        died.become_,
        Step::Stop(Exit::LinkDied(p)) if p == PEER
    ));

    // Child lane: a death still yields a replacement send on a fresh stack.
    let mut replacement = Spec::new(Parent)
        .watch(PEER, stop_on_abnormal_death)
        .children((2, child));
    replacement.init().await.unwrap();
    let actions = replacement
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .await
        .unwrap();
    assert_eq!(actions.sends.own.own.len(), 1);
    assert_eq!(actions.sends.own.own[0].to.route(), Route::Child(0));
}
