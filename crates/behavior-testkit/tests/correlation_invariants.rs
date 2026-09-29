//! Independent retained-lifecycle models for correlation templates.

use behavior_actors::{
    AcknowledgementError, AcknowledgementInput, AcknowledgementMessage, AcknowledgementOutcome,
    AcknowledgementState, Acknowledgements, Activate as _, CorrelationResult, CorrelationState,
    Correlator, CorrelatorError, CorrelatorMessage,
};

use behavior_core::{MailAddr, MessageProtocol, Recipient, Step};
use proptest::collection::vec;
use proptest::prelude::*;
use proptest::test_runner::TestCaseResult;

type Acks = Acknowledgements<
    MailAddr,
    u8,
    u8,
    Recipient<MessageProtocol<MailAddr, AcknowledgementOutcome<u8, u8>>>,
>;
type Correlations =
    Correlator<MailAddr, u8, u8, Recipient<MessageProtocol<MailAddr, CorrelationResult<u8, u8>>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CorrelationPhase {
    Pending,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug)]
enum AckTurn {
    Begin {
        key: u8,
        participants: Vec<u8>,
        reply: u8,
    },
    Acknowledge {
        key: u8,
        participant: u8,
        reply: u8,
    },
    Cancel {
        key: u8,
        reply: u8,
    },
}

impl AckTurn {
    fn reply(&self) -> u8 {
        match self {
            Self::Begin { reply, .. }
            | Self::Acknowledge { reply, .. }
            | Self::Cancel { reply, .. } => *reply,
        }
    }
}

#[derive(Debug)]
enum AckPhase {
    Pending {
        declared: Vec<u8>,
        accepted: Vec<u8>,
    },
    Completed,
    Cancelled,
}

struct ModeledAck {
    key: u8,
    phase: AckPhase,
}

fn expected_ack(records: &mut Vec<ModeledAck>, turn: &AckTurn) -> AcknowledgementOutcome<u8, u8> {
    match turn {
        AckTurn::Begin {
            key, participants, ..
        } => {
            if records.iter().any(|record| record.key == *key) {
                return AcknowledgementOutcome::Rejected(AcknowledgementError::Existing {
                    key: *key,
                    participants: participants.clone(),
                });
            }
            let mut declared = Vec::new();
            for participant in participants {
                if !declared.contains(participant) {
                    declared.push(*participant);
                }
            }
            let remaining = declared.len();
            records.push(ModeledAck {
                key: *key,
                phase: if remaining == 0 {
                    AckPhase::Completed
                } else {
                    AckPhase::Pending {
                        declared,
                        accepted: Vec::new(),
                    }
                },
            });
            if remaining == 0 {
                AcknowledgementOutcome::Completed { key: *key }
            } else {
                AcknowledgementOutcome::Started {
                    key: *key,
                    remaining,
                }
            }
        }
        AckTurn::Acknowledge {
            key, participant, ..
        } => {
            let input = AcknowledgementInput::Acknowledge {
                key: *key,
                participant: *participant,
            };
            let Some(record) = records.iter_mut().find(|record| record.key == *key) else {
                return AcknowledgementOutcome::Rejected(AcknowledgementError::Unknown(input));
            };
            match &mut record.phase {
                AckPhase::Completed => {
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Completed(input))
                }
                AckPhase::Cancelled => {
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Cancelled(input))
                }
                AckPhase::Pending { declared, accepted } => {
                    if accepted.contains(participant) {
                        return AcknowledgementOutcome::Rejected(
                            AcknowledgementError::DuplicateParticipant {
                                key: *key,
                                participant: *participant,
                            },
                        );
                    }
                    if !declared.contains(participant) {
                        return AcknowledgementOutcome::Rejected(
                            AcknowledgementError::UnexpectedParticipant {
                                key: *key,
                                participant: *participant,
                            },
                        );
                    }
                    accepted.push(*participant);
                    let remaining = declared.len() - accepted.len();
                    if remaining == 0 {
                        record.phase = AckPhase::Completed;
                        AcknowledgementOutcome::Completed { key: *key }
                    } else {
                        AcknowledgementOutcome::Acknowledged {
                            key: *key,
                            participant: *participant,
                            remaining,
                        }
                    }
                }
            }
        }
        AckTurn::Cancel { key, .. } => {
            let input = AcknowledgementInput::Cancel { key: *key };
            let Some(record) = records.iter_mut().find(|record| record.key == *key) else {
                return AcknowledgementOutcome::Rejected(AcknowledgementError::Unknown(input));
            };
            match &record.phase {
                AckPhase::Pending { .. } => {
                    record.phase = AckPhase::Cancelled;
                    AcknowledgementOutcome::Cancelled { key: *key }
                }
                AckPhase::Completed => {
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Completed(input))
                }
                AckPhase::Cancelled => {
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Cancelled(input))
                }
            }
        }
    }
}

fn check_ack_trace(turns: impl IntoIterator<Item = AckTurn>) -> TestCaseResult {
    let mut actual = Acks::new().initialize().unwrap().behavior;
    let mut expected = Vec::new();
    for turn in turns {
        let reply_to = Recipient::global(MailAddr(u64::from(turn.reply())));
        let outcome = expected_ack(&mut expected, &turn);
        let actions = match turn {
            AckTurn::Begin {
                key, participants, ..
            } => actual.receive(
                MailAddr(9),
                AcknowledgementMessage::Begin {
                    key,
                    participants,
                    reply_to,
                },
            ),
            AckTurn::Acknowledge {
                key, participant, ..
            } => actual.receive(
                MailAddr(9),
                AcknowledgementMessage::Acknowledge {
                    key,
                    participant,
                    reply_to,
                },
            ),
            AckTurn::Cancel { key, .. } => actual.receive(
                MailAddr(9),
                AcknowledgementMessage::Cancel { key, reply_to },
            ),
        }
        .unwrap();
        prop_assert_eq!(actions.sends.len(), 1);
        prop_assert_eq!(actions.sends[0].to, reply_to);
        prop_assert_eq!(&actions.sends[0].message, &outcome);
        prop_assert!(actions.creates.is_empty());
        prop_assert_eq!(actions.become_, Step::Continue);
        prop_assert_eq!(actual.records().len(), expected.len());
        for (record, modeled) in actual.records().iter().zip(&expected) {
            prop_assert_eq!(record.key, modeled.key);
            let modeled_state = match &modeled.phase {
                AckPhase::Pending { declared, accepted } => AcknowledgementState::Pending {
                    remaining: declared
                        .iter()
                        .copied()
                        .filter(|participant| !accepted.contains(participant))
                        .collect(),
                    acknowledged: accepted.clone(),
                },
                AckPhase::Completed => AcknowledgementState::Completed,
                AckPhase::Cancelled => AcknowledgementState::Cancelled,
            };
            prop_assert_eq!(&record.state, &modeled_state);
        }
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 384, max_shrink_iters: 100_000, ..ProptestConfig::default() })]

    #[test]
    fn correlator_permits_one_terminal_transition_and_never_reopens(
        operations in vec((0_u8..3, 0_u8..8, any::<u8>()), 0..200),
    ) {
        let mut actual = Correlations::new().initialize().unwrap().behavior;
        let mut expected: Vec<(u8, CorrelationPhase)> = Vec::new();
        let reply = Recipient::global(MailAddr(1));

        for (operation, key, value) in operations {
            let existing = expected.iter().position(|(candidate, _)| *candidate == key);
            let result = match operation {
                0 => actual.receive(MailAddr(9), CorrelatorMessage::Begin { key, reply_to: reply }),
                1 => actual.receive(MailAddr(9), CorrelatorMessage::Resolve { key, value }),
                _ => actual.receive(MailAddr(9), CorrelatorMessage::Cancel { key }),
            };
            match (operation, existing.map(|index| expected[index].1)) {
                (0, None) => { prop_assert!(result.is_ok()); expected.push((key, CorrelationPhase::Pending)); }
                (0, Some(CorrelationPhase::Pending)) => {
                    let exact = matches!(result, Err(CorrelatorError::AlreadyPending { key: returned, reply_to }) if returned == key && reply_to == reply);
                    prop_assert!(exact);
                }
                (0, Some(CorrelationPhase::Completed)) => {
                    let exact = matches!(result, Err(CorrelatorError::ReopenCompleted { key: returned, reply_to }) if returned == key && reply_to == reply);
                    prop_assert!(exact);
                }
                (0, Some(CorrelationPhase::Cancelled)) => {
                    let exact = matches!(result, Err(CorrelatorError::ReopenCancelled { key: returned, reply_to }) if returned == key && reply_to == reply);
                    prop_assert!(exact);
                }
                (1, None) => {
                    let matched = matches!(result, Err(CorrelatorError::UnknownReply { key: returned, value: returned_value }) if returned == key && returned_value == value);
                    prop_assert!(matched);
                }
                (1, Some(CorrelationPhase::Pending)) => {
                    let actions = result.unwrap();
                    expected[existing.unwrap()].1 = CorrelationPhase::Completed;
                    prop_assert_eq!(&actions.sends[0].message, &CorrelationResult::Resolved { key, value });
                }
                (1, Some(CorrelationPhase::Completed)) => {
                    let matched = matches!(result, Err(CorrelatorError::StaleCompleted { key: returned, value: returned_value }) if returned == key && returned_value == value);
                    prop_assert!(matched);
                }
                (1, Some(CorrelationPhase::Cancelled)) => {
                    let matched = matches!(result, Err(CorrelatorError::StaleCancelled { key: returned, value: returned_value }) if returned == key && returned_value == value);
                    prop_assert!(matched);
                }
                (2, None) => prop_assert!(matches!(result, Err(CorrelatorError::Unknown(returned)) if returned == key)),
                (2, Some(CorrelationPhase::Pending)) => {
                    let actions = result.unwrap();
                    expected[existing.unwrap()].1 = CorrelationPhase::Cancelled;
                    prop_assert_eq!(&actions.sends[0].message, &CorrelationResult::Cancelled { key });
                }
                (2, Some(CorrelationPhase::Completed)) => prop_assert!(matches!(result, Err(CorrelatorError::AlreadyCompleted(returned)) if returned == key)),
                (2, Some(CorrelationPhase::Cancelled)) => prop_assert!(matches!(result, Err(CorrelatorError::AlreadyCancelled(returned)) if returned == key)),
                _ => unreachable!(),
            }
            prop_assert_eq!(actual.states().len(), expected.len());
            for (state, (modeled_key, phase)) in actual.states().iter().zip(&expected) {
                let same = match (state, phase) {
                    (CorrelationState::Pending { key, .. }, CorrelationPhase::Pending)
                    | (CorrelationState::Completed { key }, CorrelationPhase::Completed)
                    | (CorrelationState::Cancelled { key }, CorrelationPhase::Cancelled) => key == modeled_key,
                    _ => false,
                };
                prop_assert!(same);
            }
        }
    }

    #[test]
    fn acknowledgements_match_an_independent_interleaved_lifecycle_model(
        turns in vec(prop_oneof![
            (0_u8..6, vec(0_u8..6, 0..8), 1_u8..5).prop_map(|(key, participants, reply)| AckTurn::Begin { key, participants, reply }),
            (0_u8..6, 0_u8..6, 1_u8..5).prop_map(|(key, participant, reply)| AckTurn::Acknowledge { key, participant, reply }),
            (0_u8..6, 1_u8..5).prop_map(|(key, reply)| AckTurn::Cancel { key, reply }),
        ], 0..160),
    ) {
        check_ack_trace(turns)?;
    }
}

#[test]
fn acknowledgement_cancel_and_completion_keep_distinct_terminal_answers() {
    check_ack_trace([
        AckTurn::Begin {
            key: 1,
            participants: vec![3, 2, 3],
            reply: 1,
        },
        AckTurn::Acknowledge {
            key: 1,
            participant: 3,
            reply: 2,
        },
        AckTurn::Cancel { key: 1, reply: 3 },
        AckTurn::Acknowledge {
            key: 1,
            participant: 2,
            reply: 4,
        },
        AckTurn::Cancel { key: 1, reply: 1 },
        AckTurn::Begin {
            key: 1,
            participants: vec![2],
            reply: 2,
        },
        AckTurn::Begin {
            key: 3,
            participants: vec![1, 2],
            reply: 1,
        },
        AckTurn::Acknowledge {
            key: 3,
            participant: 4,
            reply: 2,
        },
        AckTurn::Acknowledge {
            key: 3,
            participant: 1,
            reply: 3,
        },
        AckTurn::Acknowledge {
            key: 3,
            participant: 1,
            reply: 4,
        },
        AckTurn::Acknowledge {
            key: 3,
            participant: 2,
            reply: 1,
        },
        AckTurn::Acknowledge {
            key: 3,
            participant: 1,
            reply: 2,
        },
        AckTurn::Begin {
            key: 2,
            participants: vec![],
            reply: 3,
        },
        AckTurn::Cancel { key: 2, reply: 4 },
        AckTurn::Acknowledge {
            key: 5,
            participant: 1,
            reply: 1,
        },
        AckTurn::Cancel { key: 5, reply: 2 },
    ])
    .unwrap();
}
