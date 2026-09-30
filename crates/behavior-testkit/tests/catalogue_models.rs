use std::collections::{BTreeMap, VecDeque};

use behavior_actors::{
    Activate, Deduplicator, DeduplicatorMessage, DeduplicatorOutcome, OrderGate, OrderGateMessage,
    OrderGateOutcome, Sequence, Sequencer, SequencerMessage, SequencerOutcome, SequencerState,
};

use behavior_core::{MailAddr, Recipient, Step};
use proptest::collection::vec;
use proptest::prelude::*;

struct ByteTarget;
struct SequenceReply;
struct OwnedTarget;
struct OwnedDedupReply;
struct GateReply;

#[derive(Debug, Eq, PartialEq)]
struct OwnedValue(u8);

macro_rules! leaf {
    ($name:ident, $message:ty) => {
        impl behavior_core::Protocol for $name {
            type Addr = MailAddr;
            type Msg = $message;
        }
    };
}

leaf!(ByteTarget, u8);
leaf!(SequenceReply, behavior_actors::SequencerOutcome<u8>);
leaf!(OwnedTarget, Box<OwnedValue>);
leaf!(OwnedDedupReply, DeduplicatorOutcome<u8, Box<OwnedValue>>);
leaf!(GateReply, behavior_actors::OrderGateOutcome<u8, u8>);

#[derive(Default)]
struct SequenceOracle {
    expected: u64,
    waiting: BTreeMap<u64, (MailAddr, u8)>,
}

impl SequenceOracle {
    fn offer(
        &mut self,
        sequence: u64,
        value: u8,
        to: MailAddr,
    ) -> (Vec<(MailAddr, u8)>, SequencerOutcome<u8>) {
        if sequence < self.expected {
            return (
                Vec::new(),
                SequencerOutcome::Stale {
                    sequence: Sequence(sequence),
                    value,
                    expected: Sequence(self.expected),
                },
            );
        }
        if self.waiting.contains_key(&sequence) {
            return (
                Vec::new(),
                SequencerOutcome::Duplicate {
                    sequence: Sequence(sequence),
                    value,
                },
            );
        }
        self.waiting.insert(sequence, (to, value));
        let mut released = Vec::new();
        while let Some(delivery) = self.waiting.remove(&self.expected) {
            released.push(delivery);
            self.expected += 1;
        }
        let outcome = SequencerOutcome::Accepted {
            released: released.len(),
            buffered: self.waiting.len(),
        };
        (released, outcome)
    }
}

#[derive(Debug, Clone)]
enum GateOperation {
    Hold(u8, u8, u64, u64),
    Open(u8, u64),
}

fn gate_operations() -> impl Strategy<Value = Vec<GateOperation>> {
    vec(
        prop_oneof![
            (0_u8..12, any::<u8>(), 1_u64..4, 5_u64..8).prop_map(|(key, value, target, reply)| {
                GateOperation::Hold(key, value, target, reply)
            }),
            (0_u8..12, 5_u64..8).prop_map(|(through, reply)| GateOperation::Open(through, reply)),
        ],
        0..128,
    )
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn sequencer_matches_an_independent_gap_map_after_every_offer(
        offers in vec((0_u64..24, any::<u8>(), 1_u64..4, 5_u64..8), 0..128)
    ) {
        type Subject =
            Sequencer<MailAddr, u8, Recipient<ByteTarget>, Recipient<SequenceReply>>;
        let initialized = (Subject::new(Sequence(0))).initialize().unwrap();
        prop_assert!(initialized.actions.sends.deliveries.is_empty());
        prop_assert!(initialized.actions.sends.outcomes.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut oracle = SequenceOracle::default();

        for (sequence, value, target, reply) in offers {
            let destination = MailAddr(target);
            let reply_destination = MailAddr(reply);
            let (expected_deliveries, expected_outcome) =
                oracle.offer(sequence, value, destination);
            let actions = actual.receive(
                MailAddr(9),
                SequencerMessage::Offer {
                    sequence: Sequence(sequence),
                    value,
                    to: Recipient::global(destination),
                    reply_to: Recipient::global(reply_destination),
                },
            ).unwrap();
            prop_assert!(actions.creates.is_empty());
            prop_assert!(matches!(actions.become_, Step::Continue));
            let actual_deliveries = actions.sends.deliveries.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message)).collect::<Vec<_>>();
            let actual_outcomes = actions.sends.outcomes.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message)).collect::<Vec<_>>();
            prop_assert_eq!(actual_deliveries, expected_deliveries);
            prop_assert_eq!(actual_outcomes, [(reply_destination, expected_outcome)]);
            prop_assert_eq!(actual.state(), SequencerState::Active {
                expected: Sequence(oracle.expected),
                buffered: oracle.waiting.len(),
            });
        }
    }

    #[test]
    fn deduplicator_matches_an_independent_fifo_window_after_every_delivery(
        capacity in 1_usize..8,
        attempts in vec((0_u8..16, any::<u8>(), 1_u64..4, 5_u64..8), 0..128)
    ) {
        type Subject = Deduplicator<
            MailAddr,
            u8,
            Box<OwnedValue>,
            Recipient<OwnedTarget>,
            Recipient<OwnedDedupReply>,
        >;
        let initialized = (Subject::new(capacity).unwrap()).initialize().unwrap();
        prop_assert!(initialized.actions.sends.deliveries.is_empty());
        prop_assert!(initialized.actions.sends.outcomes.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut retained = VecDeque::new();

        for (key, value, target, reply) in attempts {
            let duplicate = retained.contains(&key);
            let destination = MailAddr(target);
            let reply_destination = MailAddr(reply);
            let payload = Box::new(OwnedValue(value));
            let original = &*payload as *const OwnedValue;
            let actions = actual.receive(
                MailAddr(9),
                DeduplicatorMessage::Deliver {
                    key,
                    value: payload,
                    to: Recipient::global(destination),
                    reply_to: Recipient::global(reply_destination),
                },
            ).unwrap();
            let (expected_deliveries, expected_outcome) = if duplicate {
                (
                    Vec::new(),
                    DeduplicatorOutcome::Duplicate { key, value: Box::new(OwnedValue(value)) },
                )
            } else {
                let evicted = if retained.len() == capacity {
                    retained.pop_front()
                } else {
                    None
                };
                retained.push_back(key);
                (
                    vec![(destination, value)],
                    DeduplicatorOutcome::Delivered { key, evicted },
                )
            };
            prop_assert!(actions.creates.is_empty());
            prop_assert!(matches!(actions.become_, Step::Continue));
            let returned = if duplicate {
                match actions.sends.outcomes.as_slice() {
                    [delivery] => match &delivery.message {
                        DeduplicatorOutcome::Duplicate { value, .. } =>
                            Some(&**value as *const OwnedValue),
                        DeduplicatorOutcome::Delivered { .. } => None,
                    },
                    _ => None,
                }
            } else {
                actions.sends.deliveries.first()
                    .map(|delivery| &*delivery.message as *const OwnedValue)
            };
            prop_assert_eq!(returned, Some(original));
            let actual_deliveries = actions.sends.deliveries.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message.0)).collect::<Vec<_>>();
            let actual_outcomes = actions.sends.outcomes.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message)).collect::<Vec<_>>();
            prop_assert_eq!(actual_deliveries, expected_deliveries);
            prop_assert_eq!(actual_outcomes, [(reply_destination, expected_outcome)]);
            prop_assert_eq!(
                actual.state().retained().to_vec(),
                retained.iter().copied().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn order_gate_matches_an_independent_watermark_map_after_every_operation(
        operations in gate_operations()
    ) {
        type Subject =
            OrderGate<MailAddr, u8, u8, Recipient<ByteTarget>, Recipient<GateReply>>;
        let initialized = (Subject::new()).initialize().unwrap();
        prop_assert!(initialized.actions.sends.deliveries.is_empty());
        prop_assert!(initialized.actions.sends.outcomes.is_empty());
        prop_assert!(initialized.actions.creates.is_empty());
        prop_assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut actual = initialized.behavior;
        let mut watermark = None;
        let mut held = BTreeMap::new();

        for operation in operations {
            let (actions, expected_deliveries, reply_destination, expected_outcome) =
                match operation {
                GateOperation::Hold(key, value, target, reply) => {
                    let destination = MailAddr(target);
                    let expected = if watermark.is_some_and(|open| key <= open) {
                        (vec![(destination, value)], OrderGateOutcome::Delivered { key })
                    } else if held.contains_key(&key) {
                        (Vec::new(), OrderGateOutcome::Duplicate { key, value })
                    } else {
                        held.insert(key, (destination, value));
                        (Vec::new(), OrderGateOutcome::Held { key, held: held.len() })
                    };
                    let actions = actual.receive(MailAddr(9), OrderGateMessage::Hold {
                        key,
                        value,
                        to: Recipient::global(destination),
                        reply_to: Recipient::global(MailAddr(reply)),
                    }).unwrap();
                    (actions, expected.0, MailAddr(reply), expected.1)
                }
                GateOperation::Open(through, reply) => {
                    let expected = if let Some(current) = watermark.filter(|open| through <= *open) {
                        (Vec::new(), OrderGateOutcome::StaleOpening { requested: through, current })
                    } else {
                        let released = held.range(..=through).map(|(_, delivery)| *delivery).collect::<Vec<_>>();
                        held.retain(|key, _| *key > through);
                        watermark = Some(through);
                        let count = released.len();
                        (released, OrderGateOutcome::Opened { through, released: count, held: held.len() })
                    };
                    let actions = actual.receive(MailAddr(9), OrderGateMessage::OpenThrough {
                        through,
                        reply_to: Recipient::global(MailAddr(reply)),
                    }).unwrap();
                    (actions, expected.0, MailAddr(reply), expected.1)
                }
            };
            prop_assert!(actions.creates.is_empty());
            prop_assert!(matches!(actions.become_, Step::Continue));
            let actual_deliveries = actions.sends.deliveries.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message)).collect::<Vec<_>>();
            let actual_outcomes = actions.sends.outcomes.into_iter()
                .map(|delivery| (delivery.to.address(), delivery.message)).collect::<Vec<_>>();
            prop_assert_eq!(actual_deliveries, expected_deliveries);
            prop_assert_eq!(actual_outcomes, [(reply_destination, expected_outcome)]);
            let state = actual.state();
            prop_assert_eq!(state.watermark, watermark);
            prop_assert_eq!(state.held(), held.len());
        }
    }
}
