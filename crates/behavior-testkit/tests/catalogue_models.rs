use std::collections::{BTreeMap, VecDeque};

use behavior::{
    Actions, Behavior, BehaviorActed, Compose, Deduplicator, DeduplicatorMessage,
    DeduplicatorOutcome, MailAddr, Never, NoBirths, OrderGate, OrderGateMessage, Recipient,
    Sequence, Sequencer, SequencerMessage, SequencerState, User,
};
use proptest::collection::vec;
use proptest::prelude::*;

struct ByteTarget;
struct SequenceReply;
struct DedupReply;
struct GateReply;

macro_rules! leaf {
    ($name:ident, $message:ty) => {
        impl Behavior for $name {
            type Addr = MailAddr;
            type Msg = $message;
            type Event = User<MailAddr, Self::Msg>;
            type Sends = Vec<Never>;
            type Ph = Never;
            type Error = Never;
            type Birth = NoBirths;

            fn transition(
                &mut self,
                _: behavior::ActiveTurn,
                _: Self::Event,
            ) -> BehaviorActed<Self> {
                Ok(Actions::cont())
            }
        }
    };
}

leaf!(ByteTarget, u8);
leaf!(SequenceReply, behavior::SequencerOutcome<u8>);
leaf!(DedupReply, DeduplicatorOutcome<u8, u8>);
leaf!(GateReply, behavior::OrderGateOutcome<u8, u8>);

#[derive(Default)]
struct SequenceOracle {
    expected: u64,
    waiting: BTreeMap<u64, u8>,
}

impl SequenceOracle {
    fn offer(&mut self, sequence: u64, value: u8) -> Vec<u8> {
        if sequence < self.expected || self.waiting.contains_key(&sequence) {
            return Vec::new();
        }
        self.waiting.insert(sequence, value);
        let mut released = Vec::new();
        while let Some(value) = self.waiting.remove(&self.expected) {
            released.push(value);
            self.expected += 1;
        }
        released
    }
}

#[derive(Debug, Clone)]
enum GateOperation {
    Hold(u8, u8),
    Open(u8),
}

fn gate_operations() -> impl Strategy<Value = Vec<GateOperation>> {
    vec(
        prop_oneof![
            (0_u8..12, any::<u8>()).prop_map(|(key, value)| GateOperation::Hold(key, value)),
            (0_u8..12).prop_map(GateOperation::Open),
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
        offers in vec((0_u64..24, any::<u8>()), 0..128)
    ) {
        type Subject = Sequencer<MailAddr, u8, ByteTarget, SequenceReply>;
        let mut actual = Compose::new(Subject::new(Sequence(0))).initialize().unwrap().behavior;
        let mut oracle = SequenceOracle::default();

        for (sequence, value) in offers {
            let expected_deliveries = oracle.offer(sequence, value);
            let actions = actual.receive(
                MailAddr(9),
                SequencerMessage::Offer {
                    sequence: Sequence(sequence),
                    value,
                    to: Recipient::global(MailAddr(1)),
                    reply_to: Recipient::global(MailAddr(2)),
                },
            ).unwrap();
            let actual_deliveries = actions.sends.deliveries.iter().map(|delivery| delivery.message).collect::<Vec<_>>();
            prop_assert_eq!(actual_deliveries, expected_deliveries);
            prop_assert_eq!(actual.state(), SequencerState::Active {
                expected: Sequence(oracle.expected),
                buffered: oracle.waiting.len(),
            });
        }
    }

    #[test]
    fn deduplicator_matches_an_independent_fifo_window_after_every_delivery(
        capacity in 1_usize..8,
        attempts in vec((0_u8..16, any::<u8>()), 0..128)
    ) {
        type Subject = Deduplicator<MailAddr, u8, u8, ByteTarget, DedupReply>;
        let mut actual = Compose::new(Subject::new(capacity).unwrap()).initialize().unwrap().behavior;
        let mut retained = VecDeque::new();

        for (key, value) in attempts {
            let duplicate = retained.contains(&key);
            let actions = actual.receive(
                MailAddr(9),
                DeduplicatorMessage::Deliver {
                    key,
                    value,
                    to: Recipient::global(MailAddr(1)),
                    reply_to: Recipient::global(MailAddr(2)),
                },
            ).unwrap();
            if duplicate {
                prop_assert!(actions.sends.deliveries.is_empty());
                let ownership_returned = matches!(
                    actions.sends.outcomes[0].message,
                    DeduplicatorOutcome::Duplicate {
                        key: returned_key,
                        value: returned_value
                    } if returned_key == key && returned_value == value
                );
                prop_assert!(ownership_returned);
            } else {
                if retained.len() == capacity { retained.pop_front(); }
                retained.push_back(key);
                prop_assert_eq!(actions.sends.deliveries[0].message, value);
            }
            prop_assert_eq!(actual.state().retained, retained.iter().copied().collect::<Vec<_>>());
        }
    }

    #[test]
    fn order_gate_matches_an_independent_watermark_map_after_every_operation(
        operations in gate_operations()
    ) {
        type Subject = OrderGate<MailAddr, u8, u8, ByteTarget, GateReply>;
        let mut actual = Compose::new(Subject::new()).initialize().unwrap().behavior;
        let mut watermark = None;
        let mut held = BTreeMap::new();

        for operation in operations {
            match operation {
                GateOperation::Hold(key, value) => {
                    let expected = if watermark.is_some_and(|open| key <= open) {
                        vec![value]
                    } else {
                        held.entry(key).or_insert(value);
                        Vec::new()
                    };
                    let actions = actual.receive(MailAddr(9), OrderGateMessage::Hold {
                        key,
                        value,
                        to: Recipient::global(MailAddr(1)),
                        reply_to: Recipient::global(MailAddr(2)),
                    }).unwrap();
                    prop_assert_eq!(actions.sends.deliveries.iter().map(|delivery| delivery.message).collect::<Vec<_>>(), expected);
                }
                GateOperation::Open(through) => {
                    let expected = if watermark.is_some_and(|open| through <= open) {
                        Vec::new()
                    } else {
                        let released = held.range(..=through).map(|(_, value)| *value).collect::<Vec<_>>();
                        held.retain(|key, _| *key > through);
                        watermark = Some(through);
                        released
                    };
                    let actions = actual.receive(MailAddr(9), OrderGateMessage::OpenThrough {
                        through,
                        reply_to: Recipient::global(MailAddr(2)),
                    }).unwrap();
                    prop_assert_eq!(actions.sends.deliveries.iter().map(|delivery| delivery.message).collect::<Vec<_>>(), expected);
                }
            }
            let state = actual.state();
            prop_assert_eq!(state.watermark, watermark);
            prop_assert_eq!(state.held, held.len());
        }
    }
}
