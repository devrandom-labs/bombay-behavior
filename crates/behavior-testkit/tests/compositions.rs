//! Composition routing attacks: every wrapper ordering preserves its own
//! initialization protocol at the exact nesting depth; non-user lanes
//! (time, peer observation) bypass a stash buffer while user messages are
//! intercepted; watch reactions are re-invocable and deterministic.

use std::time::{Duration, Instant};

use behavior::EventLayer;
use behavior::{
    Acted, Actions, Activate, Behavior, Crash, CreateChild, CreationSequence, Creations, Delivery,
    Exit, MailAddr, Never, PeerStopped, Recipient, SendEffects, StashRoute, Step, TimerElapsed,
    TimerGeneration, TimerId, User, UserEvent, stop_on_abnormal_death,
};

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
            creates: Creations::empty(),
            become_: Step::Continue,
        })
    }
}

const PEER: MailAddr = MailAddr(44);

fn at<T: Behavior>(behavior: T, when: Instant) -> behavior::Deadline<T> {
    behavior::Deadline::new(behavior, behavior::TimerId(0), Some(when), |_| {
        Step::Continue
    })
}

struct GeneratedBase {
    creations: CreationSequence,
}

#[behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = {
        replies: Vec<Delivery<Sink>>,
    },
    births = {
        recorder: Recorder,
    },
)]
impl GeneratedBase {
    fn new() -> Self {
        Self {
            creations: CreationSequence::new(),
        }
    }

    fn init(&mut self) -> behavior::BehaviorActed<Self> {
        let mut sends = GeneratedBaseSends::empty();
        sends
            .send::<_, GeneratedBaseSendsReplies>(Delivery::new(Recipient::global(MailAddr(9)), 7));
        let id = self
            .creations
            .issue()
            .expect("the generated child ID exists");
        let creates = Creations::one(CreateChild::birth(id, Recorder::default()));
        Ok(Actions::new(sends, creates, Step::Continue))
    }

    fn receive(&mut self, _: MailAddr, _: u8) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn assert_generated_base_effects(sends: &GeneratedBaseSends, creates: usize) {
    assert_eq!(sends.replies.len(), 1);
    assert_eq!(sends.replies[0].message, 7);
    assert_eq!(creates, 1);
}

/// Every sound ordering of the three wrapper families accepts the generated
/// nominal send and birth products. `Stash` must remain inside every fallible
/// wrapper because replay cannot roll back actions from earlier messages if a
/// later replayed transition is rejected. Its compile-fail contract covers
/// the other three permutations.
#[tokio::test]
async fn generated_products_compose_through_every_sound_three_wrapper_order() {
    let due = Instant::now() + Duration::from_secs(1);

    let first = at(
        behavior::Watch::new(
            behavior::Stash::new(GeneratedBase::new(), |_| StashRoute::Deliver),
            PEER,
            stop_on_abnormal_death,
        ),
        due,
    )
    .initialize()
    .unwrap()
    .actions;
    assert_generated_base_effects(&first.sends.inner.inner, first.creates.len());

    let second = behavior::Watch::new(
        at(
            behavior::Stash::new(GeneratedBase::new(), |_| StashRoute::Deliver),
            due,
        ),
        PEER,
        stop_on_abnormal_death,
    )
    .initialize()
    .unwrap()
    .actions;
    assert_generated_base_effects(&second.sends.inner.inner, second.creates.len());

    let third = behavior::Watch::new(
        behavior::Stash::new(at(GeneratedBase::new(), due), |_| StashRoute::Deliver),
        PEER,
        stop_on_abnormal_death,
    )
    .initialize()
    .unwrap()
    .actions;
    assert_generated_base_effects(&third.sends.inner.inner, third.creates.len());
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
        |_| Step::Continue,
    );
    let initialized = c1.initialize().unwrap();
    let i1 = initialized.actions;
    let _c1 = initialized.behavior;
    assert_eq!(i1.sends.owned[0].at, second);
    assert_eq!(i1.sends.inner.owned[0].peer, PEER);
    assert_eq!(i1.sends.inner.inner.owned[0].at, first);

    // Watch::new(Deadline::new(Deadline::new(inner, T1), T2), p): Watch owns the outer product.
    let c2 = behavior::Watch::new(
        behavior::Deadline::new(
            at(Recorder::default(), first),
            behavior::TimerId(0),
            Some(second),
            |_| Step::Continue,
        ),
        PEER,
        stop_on_abnormal_death,
    );
    let initialized = c2.initialize().unwrap();
    let i2 = initialized.actions;
    let _c2 = initialized.behavior;
    assert_eq!(i2.sends.owned[0].peer, PEER);
    assert_eq!(i2.sends.inner.owned[0].at, second);
    assert_eq!(i2.sends.inner.inner.owned[0].at, first);

    // Deadline::new(Deadline::new(Watch::new(inner, p), T1), T2).
    let c3 = behavior::Deadline::new(
        behavior::Deadline::new(
            behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death),
            behavior::TimerId(0),
            Some(first),
            |_| Step::Continue,
        ),
        behavior::TimerId(0),
        Some(second),
        |_| Step::Continue,
    );
    let initialized = c3.initialize().unwrap();
    let i3 = initialized.actions;
    let _c3 = initialized.behavior;
    assert_eq!(i3.sends.owned[0].at, second);
    assert_eq!(i3.sends.inner.owned[0].at, first);
    assert_eq!(i3.sends.inner.inner.owned[0].peer, PEER);

    // Watch::new(Deadline::new(Deadline::new(inner, T2), T1), p).
    let c4 = behavior::Watch::new(
        behavior::Deadline::new(
            at(Recorder::default(), second),
            behavior::TimerId(0),
            Some(first),
            |_| Step::Continue,
        ),
        PEER,
        stop_on_abnormal_death,
    );
    let initialized = c4.initialize().unwrap();
    let i4 = initialized.actions;
    let _c4 = initialized.behavior;
    assert_eq!(i4.sends.owned[0].peer, PEER);
    assert_eq!(i4.sends.inner.owned[0].at, first);
    assert_eq!(i4.sends.inner.inner.owned[0].at, second);

    // Deadline::new(Watch::new(Deadline::new(inner, T2), p), T1).
    let c5 = behavior::Deadline::new(
        behavior::Watch::new(
            at(Recorder::default(), second),
            PEER,
            stop_on_abnormal_death,
        ),
        behavior::TimerId(0),
        Some(first),
        |_| Step::Continue,
    );
    let initialized = c5.initialize().unwrap();
    let i5 = initialized.actions;
    let _c5 = initialized.behavior;
    assert_eq!(i5.sends.owned[0].at, first);
    assert_eq!(i5.sends.inner.owned[0].peer, PEER);
    assert_eq!(i5.sends.inner.inner.owned[0].at, second);

    // Deadline::new(Deadline::new(Watch::new(inner, p), T2), T1).
    let c6 = behavior::Deadline::new(
        behavior::Deadline::new(
            behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death),
            behavior::TimerId(0),
            Some(second),
            |_| Step::Continue,
        ),
        behavior::TimerId(0),
        Some(first),
        |_| Step::Continue,
    );
    let initialized = c6.initialize().unwrap();
    let i6 = initialized.actions;
    let _c6 = initialized.behavior;
    assert_eq!(i6.sends.owned[0].at, first);
    assert_eq!(i6.sends.inner.owned[0].at, second);
    assert_eq!(i6.sends.inner.inner.owned[0].peer, PEER);
}

#[test]
fn nested_deadlines_at_the_same_instant_route_to_the_selected_occurrence() {
    let due = Instant::now() + Duration::from_secs(1);
    let deadline = behavior::Deadline::new(
        behavior::Deadline::new(Recorder::default(), TimerId(0), Some(due), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(1),
        Some(due),
        |_| Step::Continue,
    );
    let mut deadline = deadline.initialize().unwrap().behavior;

    let actions = deadline
        .transition(EventLayer::Inner(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })))
        .unwrap();

    assert!(matches!(actions.become_, Step::Stop(_)));
}

#[test]
fn equal_timer_ids_in_nested_deadlines_remain_separately_addressable() {
    let due = Instant::now() + Duration::from_secs(1);
    let deadline = behavior::Deadline::new(
        behavior::Deadline::new(Recorder::default(), TimerId(0), Some(due), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let mut deadline = deadline.initialize().unwrap().behavior;

    let outer = deadline
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert!(matches!(outer.become_, Step::Continue));

    let inner = deadline
        .transition(EventLayer::Inner(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })))
        .unwrap();
    assert!(matches!(inner.become_, Step::Stop(_)));
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
        |_| Step::Continue,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Time lane: fires through the stash layer, nothing stashed.
    let reached = EventLayer::Owned(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    });
    let fired = behavior.transition(reached).unwrap();
    assert!(matches!(fired.become_, Step::Continue));
    assert_eq!(behavior.stashed(), 0);

    // Peer lane: matching peer death stops the behavior through the stash layer.
    let peer = EventLayer::Owned(PeerStopped {
        peer: PEER,
        outcome: Err(Crash::Failed),
    });
    let died = behavior.transition(EventLayer::Inner(peer)).unwrap();
    assert!(matches!(died.become_, Step::Stop(behavior::Stopped)));
    assert_eq!(behavior.stashed(), 0);

    // User lane: intercepted by the stash buffer.
    let user = User::user(MailAddr(7), 3);
    let stashed = behavior
        .transition(EventLayer::Inner(EventLayer::Inner(user)))
        .unwrap();
    assert!(stashed.creates.is_empty());
    assert!(matches!(stashed.become_, Step::Continue));
    assert_eq!(behavior.stashed(), 1);
    assert!(behavior.base().seen.is_empty());
}

/// Watch does not latch: stepping after a Stop is allowed and re-invokes
/// the reaction on each matching death; ordinary user messages still run.
#[tokio::test]
async fn watch_reaction_reinvokes_on_each_death_and_transition_continues() {
    let behavior = behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let death = EventLayer::Owned(PeerStopped {
        peer: PEER,
        outcome: Err(Crash::Failed),
    });
    let first = behavior.transition(death.clone()).unwrap();
    assert!(matches!(first.become_, Step::Stop(behavior::Stopped)));

    let second = behavior.transition(death).unwrap();
    assert!(matches!(second.become_, Step::Stop(behavior::Stopped)));

    // The behavior remains usable: a user message after the stop is processed.
    let actions = behavior
        .transition(UserEvent::user(MailAddr(2), 9))
        .unwrap();
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(behavior.base().seen, [(MailAddr(2), 9)]);
}

/// A watch-of-watch exposes distinct structural destinations for both owners.
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
    let outer_death = EventLayer::Owned(PeerStopped {
        peer: outer_peer,
        outcome: Err(Crash::Failed),
    });
    let outer = behavior.transition(outer_death).unwrap();
    assert!(matches!(outer.become_, Step::Stop(behavior::Stopped)));

    // The inner observation request selects the inner watcher directly.
    let inner_death = EventLayer::Inner(EventLayer::Owned(PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    }));
    let inner = behavior.transition(inner_death).unwrap();
    assert!(matches!(inner.become_, Step::Stop(behavior::Stopped)));
}

fn continue_after_death<B: Behavior>(
    _: &mut B,
    _: MailAddr,
    _: &Result<Exit<MailAddr>, Crash>,
) -> behavior::Become {
    Step::Continue
}

#[tokio::test]
async fn duplicate_nested_watch_peer_remains_addressable_at_both_paths() {
    let behavior = behavior::Watch::new(
        behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death),
        PEER,
        continue_after_death,
    );
    let mut behavior = behavior.initialize().unwrap().behavior;

    let outer = behavior
        .transition(EventLayer::Owned(PeerStopped {
            peer: PEER,
            outcome: Err(Crash::Failed),
        }))
        .unwrap();
    assert_eq!(outer.become_, Step::Continue);

    let inner = behavior
        .transition(EventLayer::Inner(EventLayer::Owned(PeerStopped {
            peer: PEER,
            outcome: Err(Crash::Failed),
        })))
        .unwrap();
    assert!(matches!(inner.become_, Step::Stop(behavior::Stopped)));
}

/// An `Deadline` constructed with `None` schedules nothing and is inert to every
/// Reached event: the reaction never fires.
#[tokio::test]
async fn unscheduled_at_is_inert_to_reached_events() {
    let behavior = behavior::Deadline::new(Recorder::default(), behavior::TimerId(0), None, |_| {
        Step::Stop(behavior::Stopped)
    });
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert!(initial.sends.owned.is_empty());

    for id in [TimerId(0), TimerId(1)] {
        let actions = behavior
            .transition(EventLayer::Owned(TimerElapsed {
                id,
                generation: TimerGeneration(0),
            }))
            .unwrap();
        assert_eq!(actions.become_, Step::Continue);
    }
}

/// `stop_on_abnormal_death` classifies outcomes: Normal and Collected keep
/// the behavior active; `LinkDied` and crashes stop it carrying the peer address.
#[tokio::test]
async fn abnormal_death_reaction_outcome_classes() {
    let behavior = behavior::Watch::new(Recorder::default(), PEER, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let outcome = |outcome| {
        EventLayer::Owned(PeerStopped {
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
