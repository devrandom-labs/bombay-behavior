//! Composition routing attacks: every wrapper ordering preserves its own
//! initialization protocol at the exact nesting depth; non-user lanes
//! (time, peer observation) bypass a stash buffer while user messages are
//! intercepted; watch reactions are re-invocable and deterministic.

use std::time::Duration;

use behavior::{
    Acted, Actions, Activate, Behavior, Crash, DeadlineEvent, Delivery, Exit, MailAddr, Never,
    PeerStopped, Recipient, StashRoute, Step, SupervisionEvent, TimerElapsed, TimerGeneration,
    TimerId, User, UserEvent, WatchEvent, WorkerStopped, stop_on_abnormal_death,
    stop_on_supervision_failure,
};
use std::time::Instant;

struct Sink;

impl behavior::Protocol for Sink {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Sink {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = behavior::NoBirths;

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

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<Sink>>, births = behavior::NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Sink>>, behavior::NoBirths, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

type Child = Recorder;

fn child(_index: usize) -> Child {
    Recorder::default()
}

const PEER: MailAddr = MailAddr(44);

fn at<T: Behavior>(behavior: T, when: Instant) -> behavior::Deadline<T> {
    behavior::Deadline::new(behavior, behavior::TimerId(0), Some(when), |_| {
        Ok(Step::Continue)
    })
}

/// Every ordering of {at, watch, at} preserves each layer's own initial
/// protocol at exactly its nesting depth: outermost send product carries the
/// outermost schedule, and so on inward.
#[tokio::test]
async fn all_wrapper_permutations_preserve_init_protocol_nesting() {
    let first = Instant::now() + Duration::from_secs(1);
    let second = first + Duration::from_secs(1);

    // Deadline::new(Watch::new(Deadline::new(inner, T1), p), T2): outermost Deadline owns T2, Watch owns p.
    // innermost Deadline owns T1.
    let c1 = behavior::Deadline::new(
        behavior::Watch::new(at(Recorder::default(), first), PEER, stop_on_abnormal_death),
        behavior::TimerId(0),
        Some(second),
        |_| Ok(Step::Continue),
    );
    let initialized = c1.initialize().unwrap();
    let i1 = initialized.actions;
    let _c1 = initialized.behavior;
    assert_eq!(i1.sends.schedules[0].at, second);
    assert_eq!(i1.sends.behavior.observations[0].peer, PEER);
    assert_eq!(i1.sends.behavior.behavior.schedules[0].at, first);

    // Watch::new(Deadline::new(Deadline::new(inner, T1), T2), p): Watch owns the outer product.
    let c2 = behavior::Watch::new(
        behavior::Deadline::new(
            at(Recorder::default(), first),
            behavior::TimerId(0),
            Some(second),
            |_| Ok(Step::Continue),
        ),
        PEER,
        stop_on_abnormal_death,
    );
    let initialized = c2.initialize().unwrap();
    let i2 = initialized.actions;
    let _c2 = initialized.behavior;
    assert_eq!(i2.sends.observations[0].peer, PEER);
    assert_eq!(i2.sends.behavior.schedules[0].at, second);
    assert_eq!(i2.sends.behavior.behavior.schedules[0].at, first);

    // Deadline::new(Deadline::new(Watch::new(inner, p), T1), T2).
    let c3 = behavior::Deadline::new(
        behavior::Deadline::new(
            behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death),
            behavior::TimerId(0),
            Some(first),
            |_| Ok(Step::Continue),
        ),
        behavior::TimerId(0),
        Some(second),
        |_| Ok(Step::Continue),
    );
    let initialized = c3.initialize().unwrap();
    let i3 = initialized.actions;
    let _c3 = initialized.behavior;
    assert_eq!(i3.sends.schedules[0].at, second);
    assert_eq!(i3.sends.behavior.schedules[0].at, first);
    assert_eq!(i3.sends.behavior.behavior.observations[0].peer, PEER);

    // Watch::new(Deadline::new(Deadline::new(inner, T2), T1), p).
    let c4 = behavior::Watch::new(
        behavior::Deadline::new(
            at(Recorder::default(), second),
            behavior::TimerId(0),
            Some(first),
            |_| Ok(Step::Continue),
        ),
        PEER,
        stop_on_abnormal_death,
    );
    let initialized = c4.initialize().unwrap();
    let i4 = initialized.actions;
    let _c4 = initialized.behavior;
    assert_eq!(i4.sends.observations[0].peer, PEER);
    assert_eq!(i4.sends.behavior.schedules[0].at, first);
    assert_eq!(i4.sends.behavior.behavior.schedules[0].at, second);

    // Deadline::new(Watch::new(Deadline::new(inner, T2), p), T1).
    let c5 = behavior::Deadline::new(
        behavior::Watch::new(
            at(Recorder::default(), second),
            PEER,
            stop_on_abnormal_death,
        ),
        behavior::TimerId(0),
        Some(first),
        |_| Ok(Step::Continue),
    );
    let initialized = c5.initialize().unwrap();
    let i5 = initialized.actions;
    let _c5 = initialized.behavior;
    assert_eq!(i5.sends.schedules[0].at, first);
    assert_eq!(i5.sends.behavior.observations[0].peer, PEER);
    assert_eq!(i5.sends.behavior.behavior.schedules[0].at, second);

    // Deadline::new(Deadline::new(Watch::new(inner, p), T2), T1).
    let c6 = behavior::Deadline::new(
        behavior::Deadline::new(
            behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death),
            behavior::TimerId(0),
            Some(second),
            |_| Ok(Step::Continue),
        ),
        behavior::TimerId(0),
        Some(first),
        |_| Ok(Step::Continue),
    );
    let initialized = c6.initialize().unwrap();
    let i6 = initialized.actions;
    let _c6 = initialized.behavior;
    assert_eq!(i6.sends.schedules[0].at, first);
    assert_eq!(i6.sends.behavior.schedules[0].at, second);
    assert_eq!(i6.sends.behavior.behavior.observations[0].peer, PEER);
}

/// A stash layer contributes no initialization sends and shifts nothing:
/// the at/watch markers keep their nesting depths relative to each other.
#[tokio::test]
async fn stash_layer_contributes_no_init_sends() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = behavior::Deadline::new(
        behavior::Watch::new(
            behavior::Stash::new(Recorder::default(), |_| StashRoute::Deliver),
            PEER,
            stop_on_abnormal_death,
        ),
        behavior::TimerId(0),
        Some(due),
        |_| Ok(Step::Continue),
    );
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let _behavior = initialized.behavior;
    assert_eq!(initial.sends.schedules[0].at, due);
    assert_eq!(initial.sends.behavior.observations[0].peer, PEER);
    assert!(initial.sends.behavior.behavior.is_empty());
}

/// In an Deadline∘Watch∘Stash stack, only the user lane enters the stash buffer:
/// Reached and `PeerStopped` events pass through to their layer untouched.
#[tokio::test]
async fn environment_lanes_bypass_stash_while_user_lane_is_intercepted() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = behavior::Deadline::new(
        behavior::Watch::new(
            behavior::Stash::new(Recorder::default(), |_| StashRoute::Stash),
            PEER,
            stop_on_abnormal_death,
        ),
        behavior::TimerId(0),
        Some(due),
        |_| Ok(Step::Continue),
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Time lane: fires through the stash layer, nothing stashed.
    let reached = DeadlineEvent::Elapsed(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    });
    let fired = behavior.transition(reached).unwrap();
    assert!(matches!(fired.become_, Step::Continue));
    assert_eq!(behavior.stashed(), 0);

    // Peer lane: matching peer death stops the fold through the stash layer.
    let peer = WatchEvent::PeerStopped(PeerStopped {
        peer: PEER,
        outcome: Err(Crash::Failed),
    });
    let died = behavior.transition(DeadlineEvent::Behavior(peer)).unwrap();
    assert!(matches!(died.become_, Step::Stop(behavior::Stopped)));
    assert_eq!(behavior.stashed(), 0);

    // User lane: intercepted by the stash buffer.
    let user = User::user(MailAddr(7), 3);
    behavior
        .transition(DeadlineEvent::Behavior(WatchEvent::Behavior(user)))
        .unwrap();
    assert_eq!(behavior.stashed(), 1);
    assert!(behavior.base().seen.is_empty());
}

/// Watch does not latch: stepping after a Stop is allowed and re-invokes
/// the reaction on each matching death; ordinary user messages still fold.
#[tokio::test]
async fn watch_reaction_reinvokes_on_each_death_and_fold_continues() {
    let behavior = behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let death = WatchEvent::PeerStopped(PeerStopped {
        peer: PEER,
        outcome: Err(Crash::Failed),
    });
    let first = behavior.transition(death.clone()).unwrap();
    assert!(matches!(first.become_, Step::Stop(behavior::Stopped)));

    let second = behavior.transition(death).unwrap();
    assert!(matches!(second.become_, Step::Stop(behavior::Stopped)));

    // The fold is still usable: a user message after the stop is processed.
    let actions = behavior
        .transition(UserEvent::user(MailAddr(2), 9))
        .unwrap();
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(behavior.base().seen, [(MailAddr(2), 9)]);
}

/// An unrelated peer's death routed through a watch-of-watch reaches the
/// inner watcher, not the outer reaction; both layers keep their own peer.
#[tokio::test]
async fn watch_of_watch_routes_each_peer_to_its_own_layer() {
    let inner_peer = MailAddr(1);
    let outer_peer = MailAddr(2);
    let behavior = behavior::Watch::new(
        behavior::Watch::new(Recorder::default(), inner_peer, stop_on_abnormal_death),
        outer_peer,
        stop_on_abnormal_death,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Outer peer death: outer layer stops.
    let outer_death = WatchEvent::PeerStopped(PeerStopped {
        peer: outer_peer,
        outcome: Err(Crash::Failed),
    });
    let outer = behavior.transition(outer_death).unwrap();
    assert!(matches!(outer.become_, Step::Stop(behavior::Stopped)));

    // Inner peer death: forwarded to the inner watcher.
    let inner_death = WatchEvent::PeerStopped(PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    });
    let inner = behavior.transition(inner_death).unwrap();
    assert!(matches!(inner.become_, Step::Stop(behavior::Stopped)));
}

fn continue_after_death<B: Behavior>(
    _: &mut B,
    _: MailAddr,
    _: &Result<Exit<MailAddr>, Crash>,
) -> Result<Step<Never>, B::Error> {
    Ok(Step::Continue)
}

#[tokio::test]
async fn duplicate_nested_watch_peer_has_one_outermost_owner() {
    let behavior = behavior::Watch::new(
        behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death),
        PEER,
        continue_after_death,
    );
    let mut behavior = behavior.initialize().unwrap().behavior;

    let actions = behavior
        .transition(WatchEvent::PeerStopped(PeerStopped {
            peer: PEER,
            outcome: Err(Crash::Failed),
        }))
        .unwrap();

    assert_eq!(actions.become_, Step::Continue);
}

/// An `Deadline` constructed with `None` schedules nothing and is inert to every
/// Reached event: the reaction never fires.
#[tokio::test]
async fn unscheduled_at_is_inert_to_reached_events() {
    let behavior = behavior::Deadline::new(Recorder::default(), behavior::TimerId(0), None, |_| {
        Ok(Step::Stop(behavior::Stopped))
    });
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert!(initial.sends.schedules.is_empty());

    for id in [TimerId(0), TimerId(1)] {
        let actions = behavior
            .transition(DeadlineEvent::Elapsed(TimerElapsed {
                id,
                generation: TimerGeneration(0),
            }))
            .unwrap();
        assert_eq!(actions.become_, Step::Continue);
    }
}

/// `stop_on_abnormal_death` classifies outcomes: Normal and Collected keep
/// the fold alive; `LinkDied` and crashes stop it carrying the peer address.
#[tokio::test]
async fn abnormal_death_reaction_outcome_classes() {
    let behavior = behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let outcome = |outcome| {
        WatchEvent::PeerStopped(PeerStopped {
            peer: PEER,
            outcome,
        })
    };
    let normal = behavior.transition(outcome(Ok(Exit::Normal))).unwrap();
    assert_eq!(normal.become_, Step::Continue);
    let collected = behavior.transition(outcome(Ok(Exit::Collected))).unwrap();
    assert_eq!(collected.become_, Step::Continue);
    let linked = behavior
        .transition(outcome(Ok(Exit::LinkDied(MailAddr(3)))))
        .unwrap();
    assert!(matches!(linked.become_, Step::Stop(behavior::Stopped)));
    for crash in [
        Crash::Failed,
        Crash::EnvironmentFailed,
        Crash::Panicked,
        Crash::Cancelled,
    ] {
        let crashed = behavior.transition(outcome(Err(crash))).unwrap();
        assert!(matches!(crashed.become_, Step::Stop(behavior::Stopped)));
    }
}

/// Supervision over a watched parent: the fleet's observe sends and the
/// watch's observe send live in their own product lanes, both event lanes
/// (peer death, child death) route to their own layer, and a peer death can
/// stop the whole supervised fold.
#[tokio::test]
async fn supervision_preserves_inner_watch_routing() {
    struct Parent;
    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
    impl Parent {
        fn receive(
            &mut self,
            _from: MailAddr,
            _message: u64,
        ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
            Ok(Actions::cont())
        }
    }

    let behavior = behavior::Supervisor::new(
        behavior::Watch::new(Parent, PEER, stop_on_abnormal_death),
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
        ),
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.creates.len(), 2);
    assert_eq!(initial.sends.child_observations.len(), 2); // observe-child x2
    assert_eq!(initial.sends.child_observations[0].nonce, 0);
    assert_eq!(initial.sends.creation_observations.len(), 2);
    assert_eq!(initial.sends.creation_observations[0].nonce, 0);
    assert_eq!(initial.sends.behavior.observations.len(), 1); // observe-peer
    assert_eq!(initial.sends.behavior.observations[0].peer, PEER);

    // Peer lane: the watch reaction stops the supervised fold.
    let died = behavior
        .on(PeerStopped {
            peer: PEER,
            outcome: Err(Crash::Failed),
        })
        .unwrap();
    assert!(matches!(died.become_, Step::Stop(behavior::Stopped)));

    // Child lane: a death still yields a replacement send on a fresh stack.
    let replacement = behavior::Supervisor::new(
        behavior::Watch::new(Parent, PEER, stop_on_abnormal_death),
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
        ),
    )
    .unwrap();
    let initialized = replacement.initialize().unwrap();
    let mut replacement = initialized.behavior;
    let actions = replacement
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
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

/// A supervision failure reaction composes above an inner watch without
/// emitting into either layer's send lane. Its stop verdict remains an
/// ordinary become result visible through the complete stack.
#[tokio::test]
async fn supervision_failure_reaction_preserves_composed_send_lanes() {
    struct Parent;
    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
    impl Parent {
        fn receive(
            &mut self,
            _from: MailAddr,
            _message: u64,
        ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
            Ok(Actions::cont())
        }
    }

    let behavior = behavior::Supervisor::new(
        behavior::Watch::new(Parent, PEER, stop_on_abnormal_death),
        behavior::ChildTopology::new((0..1).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            0,
            Duration::MAX,
        ),
    )
    .unwrap()
    .with_failure_reaction(stop_on_supervision_failure);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let actions = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();

    assert!(actions.sends.behavior.behavior.is_empty());
    assert!(actions.sends.behavior.observations.is_empty());
    assert!(actions.sends.child_observations.is_empty());
    assert!(actions.sends.replacement_commands.is_empty());
    assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
}
