//! Driver-level lossless accumulation: `drive` appends every transition's
//! effect into the trace via `SendEffects::append`. For composed behaviors
//! the sends use named products; this is where "send order and wrapper
//! preservation are lossless under composition" (contract #3) is enforced at
//! the trace level. It also checks the `SendEffects` monoid law itself.

use core::future::Future;
use std::time::{Duration, Instant};

use behavior::{
    Acted, ActionItem, Actions, Crash, Creations, Delivery, EventLayer, Here, Inside,
    InterpretItem, InterpretSends, Interpretation, ItemSettlement, MailAddr, Never, ObservePeer,
    PeerObservationRejection, PeerStopped, Recipient, ScheduleAt, SendEffects, SettledItem,
    StashRoute, Step, TimerElapsed, TimerGeneration, TimerId, TimerScheduled, User,
    stop_on_abnormal_death,
};
use behavior_testkit::{DriveDisposition, Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::{ProptestConfig, any};
use proptest::{prop_assert_eq, proptest};

// The `SendEffects` monoid law that the driver's accumulation depends on:
// `empty` is a two-sided identity and `append` is associative, at both the
// `Vec` level. Named wrapper products have composition-specific tests below.
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })]

    #[test]
    fn send_effect_accumulation_has_identity_and_associativity(
        a in vec(any::<u8>(), 0..32),
        b in vec(any::<u8>(), 0..32),
        c in vec(any::<u8>(), 0..32),
    ) {
        // Vec: empty is a two-sided identity.
        let mut left_id = Vec::empty();
        SendEffects::append(&mut left_id, a.clone());
        prop_assert_eq!(&left_id, &a);
        let mut right_id = a.clone();
        SendEffects::append(&mut right_id, Vec::empty());
        prop_assert_eq!(&right_id, &a);

        // Vec: append is associative.
        let mut left_assoc = a.clone();
        SendEffects::append(&mut left_assoc, b.clone());
        SendEffects::append(&mut left_assoc, c.clone());
        let mut mid = b.clone();
        SendEffects::append(&mut mid, c.clone());
        let mut right_assoc = a.clone();
        SendEffects::append(&mut right_assoc, mid);
        prop_assert_eq!(&left_assoc, &right_assoc);

    }
}

struct EchoingApplication {
    seen: Vec<u64>,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>, births = behavior::NoBirths, error = Never)]
impl EchoingApplication {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
        behavior::NoBirths,
        Never,
    > {
        self.seen.push(message);
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(MailAddr(0)),
            message,
        )]))
    }
}

type FullStackEvent = behavior::DeadlineEvent<behavior::WatchEvent<User<MailAddr, u64>>>;

#[derive(Debug, PartialEq, Eq)]
enum FullStackRuntimeEffect {
    EchoDeliveryAccepted(u64),
    PeerObservationAccepted(MailAddr),
    TimerScheduleAccepted(TimerId),
}

struct FullStackRuntime {
    effects: Vec<FullStackRuntimeEffect>,
}

impl
    InterpretItem<
        Delivery<behavior_testkit::TestRecipient<u64>>,
        FullStackEvent,
        Inside<Inside<Here>>,
    > for FullStackRuntime
{
    fn interpret_item(
        &mut self,
        delivery: Delivery<behavior_testkit::TestRecipient<u64>>,
    ) -> impl Future<
        Output = ItemSettlement<
            Delivery<behavior_testkit::TestRecipient<u64>>,
            <Delivery<behavior_testkit::TestRecipient<u64>> as ActionItem>::Accepted,
            <Delivery<behavior_testkit::TestRecipient<u64>> as ActionItem>::Rejection,
            <Delivery<behavior_testkit::TestRecipient<u64>> as ActionItem>::Prerequisite,
        >,
    > + Send {
        async move {
            self.effects
                .push(FullStackRuntimeEffect::EchoDeliveryAccepted(
                    delivery.message,
                ));
            ItemSettlement::Accepted(())
        }
    }
}

impl InterpretItem<ObservePeer<MailAddr>, FullStackEvent, Inside<Here>> for FullStackRuntime {
    fn interpret_item(
        &mut self,
        observation: ObservePeer<MailAddr>,
    ) -> impl Future<
        Output = ItemSettlement<
            ObservePeer<MailAddr>,
            <ObservePeer<MailAddr> as ActionItem>::Accepted,
            <ObservePeer<MailAddr> as ActionItem>::Rejection,
            <ObservePeer<MailAddr> as ActionItem>::Prerequisite,
        >,
    > + Send {
        async move {
            self.effects
                .push(FullStackRuntimeEffect::PeerObservationAccepted(
                    observation.peer,
                ));
            ItemSettlement::Accepted(())
        }
    }
}

impl InterpretItem<ScheduleAt, FullStackEvent, Here> for FullStackRuntime {
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
            self.effects
                .push(FullStackRuntimeEffect::TimerScheduleAccepted(schedule.id));
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })
        }
    }
}

struct UnknownPeerRuntime;

impl InterpretItem<ObservePeer<MailAddr>, FullStackEvent, Inside<Here>> for UnknownPeerRuntime {
    fn interpret_item(
        &mut self,
        observation: ObservePeer<MailAddr>,
    ) -> impl Future<
        Output = ItemSettlement<
            ObservePeer<MailAddr>,
            <ObservePeer<MailAddr> as ActionItem>::Accepted,
            <ObservePeer<MailAddr> as ActionItem>::Rejection,
            <ObservePeer<MailAddr> as ActionItem>::Prerequisite,
        >,
    > + Send {
        async move {
            ItemSettlement::Rejected {
                item: observation,
                reason: PeerObservationRejection::UnknownAddress,
            }
        }
    }
}

/// The full three-layer stack driven through the mailbox: user echoes
/// accumulate in the innermost lane, the time lane fires once, a watched
/// peer's death stops the behavior with `LinkDied` and leaves the remaining
/// mailbox unconsumed.
#[tokio::test]
async fn driver_full_stack_mixed_lanes_stop_on_peer_death() {
    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let behavior = behavior::Deadline::new(
        behavior::Watch::new(
            behavior::Stash::new(EchoingApplication { seen: Vec::new() }, |message| {
                match message % 3 {
                    2 => StashRoute::Stash,
                    _ => StashRoute::Deliver,
                }
            }),
            peer,
            stop_on_abnormal_death,
        ),
        TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let mut mailbox = Mailbox::new([
        EventLayer::Inner(EventLayer::Inner(User::new(MailAddr(9), 1))),
        EventLayer::Inner(EventLayer::Inner(User::new(MailAddr(9), 5))),
        EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }),
        EventLayer::Inner(EventLayer::Owned(PeerStopped {
            peer,
            outcome: Err(Crash::Failed),
        })),
        EventLayer::Inner(EventLayer::Inner(User::new(MailAddr(9), 7))),
    ]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    // Stopped at the peer death: init + 4 processed events, tail left.
    assert_eq!(trace.transitions, 5);
    assert_eq!(trace.pending, 1);
    assert!(matches!(
        trace.disposition,
        DriveDisposition::BehaviorStopped(behavior::Stopped)
    ));

    let mut runtime = FullStackRuntime {
        effects: Vec::new(),
    };
    let interpreted =
        <_ as InterpretSends<_, FullStackEvent, Here>>::interpret(trace.sends, &mut runtime).await;
    assert!(matches!(interpreted, Interpretation::Complete(_)));
    assert_eq!(
        runtime.effects,
        [
            FullStackRuntimeEffect::EchoDeliveryAccepted(1),
            FullStackRuntimeEffect::PeerObservationAccepted(peer),
            FullStackRuntimeEffect::TimerScheduleAccepted(TimerId(0)),
        ]
    );
}

#[tokio::test]
async fn unknown_peer_observation_returns_the_complete_request() {
    let peer = MailAddr(404);
    let observations = behavior::InterpreterRequests::one(ObservePeer::new(peer));
    let mut runtime = UnknownPeerRuntime;

    let interpreted = <_ as InterpretSends<_, FullStackEvent, Inside<Here>>>::interpret(
        observations,
        &mut runtime,
    )
    .await;
    let Interpretation::Complete(settlements) = interpreted else {
        panic!("expected a complete logical-peer observation rejection");
    };
    let [SettledItem::Attempted(ItemSettlement::Rejected { item, reason })] =
        settlements.as_slice()
    else {
        panic!("expected exactly one attempted logical-peer observation rejection");
    };

    assert_eq!(item.peer, peer);
    assert_eq!(*reason, PeerObservationRejection::UnknownAddress);
}

/// A macro-defined behavior runs through the same Driver contract as a
/// hand-written `Behavior`: same effect algebra and accumulation.
#[tokio::test]
#[allow(
    clippy::type_complexity,
    reason = "the fixture exposes the complete typed effect surface"
)]
async fn macro_defined_behavior_drives_like_a_base() {
    struct FnRecorder {
        seen: Vec<u64>,
    }

    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>, births = behavior::NoBirths, error = Never)]
    impl FnRecorder {
        fn receive(
            &mut self,
            _from: MailAddr,
            message: u64,
        ) -> Acted<
            MailAddr,
            Never,
            Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
            behavior::NoBirths,
            Never,
        > {
            self.seen.push(message);
            Ok(Actions {
                sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
                creates: Creations::empty(),
                become_: if message == 9 {
                    Step::Stop(behavior::Stopped)
                } else {
                    Step::Continue
                },
            })
        }
    }

    let behavior = FnRecorder { seen: Vec::new() };
    let mut mailbox = Mailbox::new([
        User::new(MailAddr(1), 3),
        User::new(MailAddr(2), 9),
        User::new(MailAddr(3), 5),
    ]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 1);
    assert!(matches!(
        trace.disposition,
        DriveDisposition::BehaviorStopped(behavior::Stopped)
    ));
    let echoes: Vec<u64> = trace.sends.iter().map(|d| d.message).collect();
    assert_eq!(echoes, [3, 9]);
    assert_eq!(trace.behavior.seen, [3, 9]);
}

/// A stash release whose trigger stops the inner behavior: the Driver stops,
/// the stash buffer keeps the held messages, and nothing is lost.
#[tokio::test]
async fn driver_stash_stop_preserves_held_and_stops() {
    struct StopOnZero {
        seen: Vec<(MailAddr, u64)>,
    }
    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>, births = behavior::NoBirths, error = Never)]
    impl StopOnZero {
        fn receive(
            &mut self,
            from: MailAddr,
            message: u64,
        ) -> Acted<
            MailAddr,
            Never,
            Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
            behavior::NoBirths,
            Never,
        > {
            self.seen.push((from, message));
            Ok(Actions {
                sends: Vec::new(),
                creates: Creations::empty(),
                become_: if message == 0 {
                    Step::Stop(behavior::Stopped)
                } else {
                    Step::Continue
                },
            })
        }
    }

    let behavior =
        behavior::Stash::new(
            StopOnZero { seen: Vec::new() },
            |message: &u64| match message {
                0 => StashRoute::Release,
                _ => StashRoute::Stash,
            },
        );
    let mut mailbox = Mailbox::new([User::new(MailAddr(1), 5), User::new(MailAddr(9), 0)]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 0);
    assert!(matches!(
        trace.disposition,
        DriveDisposition::BehaviorStopped(behavior::Stopped)
    ));
    assert_eq!(trace.behavior.held(), 1); // the stashed message survives the stop
    assert_eq!(trace.behavior.base().seen, [(MailAddr(9), 0)]);
}
