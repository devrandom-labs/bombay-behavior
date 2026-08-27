//! Independent retained-lifecycle models for correlation templates.

use behavior::{
    AcknowledgementError, AcknowledgementInput, AcknowledgementMessage, AcknowledgementOutcome,
    AcknowledgementState, Acknowledgements, Actions, Activate as _, Behavior, BehaviorActed,
    CorrelationResult, CorrelationState, Correlator, CorrelatorError, CorrelatorMessage, MailAddr,
    Never, NoBirths, Recipient, User,
};
use proptest::collection::vec;
use proptest::prelude::*;

macro_rules! reply {
    ($name:ident, $message:ty) => {
        struct $name;
        impl behavior::Protocol for $name {
            type Addr = MailAddr;
            type Msg = $message;
        }
        impl Behavior for $name {
            type Protocol = Self;
            type Event = User<MailAddr, $message>;
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

reply!(AckReply, AcknowledgementOutcome<u8, u8>);
reply!(CorrelationReply, CorrelationResult<u8, u8>);

type Acks = Acknowledgements<MailAddr, u8, u8, Recipient<AckReply>>;
type Correlations = Correlator<MailAddr, u8, u8, Recipient<CorrelationReply>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CorrelationPhase {
    Pending,
    Completed,
    Cancelled,
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
    fn acknowledgements_never_lose_or_double_count_declared_participants(
        keys_and_participants in vec((0_u8..8, vec(0_u8..8, 0..10)), 0..80),
        acknowledgements in vec((0_u8..8, 0_u8..8), 0..160),
    ) {
        let mut actual = Acks::new().initialize().unwrap().behavior;
        let reply = Recipient::global(MailAddr(1));
        let mut modeled: Vec<(u8, Vec<u8>, Vec<u8>)> = Vec::new();

        for (key, participants) in keys_and_participants {
            let actions = actual.receive(MailAddr(9), AcknowledgementMessage::Begin {
                key, participants: participants.clone(), reply_to: reply,
            }).unwrap();
            if modeled.iter().any(|(candidate, _, _)| *candidate == key) {
                let exact = matches!(
                    &actions.sends[0].message,
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Existing {
                        key: returned_key,
                        participants: returned,
                    }) if *returned_key == key && returned == &participants
                );
                prop_assert!(exact);
                continue;
            }
            let mut distinct = Vec::new();
            for participant in participants { if !distinct.contains(&participant) { distinct.push(participant); } }
            modeled.push((key, distinct.clone(), Vec::new()));
            let expected = if distinct.is_empty() {
                AcknowledgementOutcome::Completed { key }
            } else {
                AcknowledgementOutcome::Started { key, remaining: distinct.len() }
            };
            prop_assert_eq!(&actions.sends[0].message, &expected);
        }

        for (key, participant) in acknowledgements {
            let before = modeled.clone();
            let actions = actual.receive(MailAddr(9), AcknowledgementMessage::Acknowledge {
                key, participant, reply_to: reply,
            }).unwrap();
            let rejected = matches!(
                &actions.sends[0].message,
                AcknowledgementOutcome::Rejected(_)
            );
            if let Some(index) = modeled.iter().position(|(candidate, _, _)| *candidate == key) {
                let (remaining, accepted) = (&modeled[index].1, &modeled[index].2);
                if remaining.is_empty() {
                    prop_assert_eq!(&actions.sends[0].message,
                        &AcknowledgementOutcome::Rejected(AcknowledgementError::Completed(
                            AcknowledgementInput::Acknowledge { key, participant }
                        )));
                } else if accepted.contains(&participant) {
                    prop_assert_eq!(&actions.sends[0].message,
                        &AcknowledgementOutcome::Rejected(AcknowledgementError::DuplicateParticipant { key, participant }));
                } else if let Some(position) = remaining.iter().position(|candidate| *candidate == participant) {
                    modeled[index].1.remove(position);
                    modeled[index].2.push(participant);
                    let expected = if modeled[index].1.is_empty() {
                        AcknowledgementOutcome::Completed { key }
                    } else {
                        AcknowledgementOutcome::Acknowledged { key, participant, remaining: modeled[index].1.len() }
                    };
                    prop_assert_eq!(&actions.sends[0].message, &expected);
                } else {
                    prop_assert_eq!(&actions.sends[0].message,
                        &AcknowledgementOutcome::Rejected(AcknowledgementError::UnexpectedParticipant { key, participant }));
                }
            } else {
                prop_assert_eq!(&actions.sends[0].message,
                    &AcknowledgementOutcome::Rejected(AcknowledgementError::Unknown(
                        AcknowledgementInput::Acknowledge { key, participant }
                    )));
            }

            for (record, (modeled_key, remaining, accepted)) in actual.records().iter().zip(&modeled) {
                prop_assert_eq!(&record.key, modeled_key);
                match &record.state {
                    AcknowledgementState::Pending { remaining: actual_remaining, acknowledged } => {
                        prop_assert_eq!(actual_remaining, remaining);
                        prop_assert_eq!(acknowledged, accepted);
                        prop_assert!(actual_remaining.iter().all(|participant| !acknowledged.contains(participant)));
                    }
                    AcknowledgementState::Completed => prop_assert!(remaining.is_empty()),
                    AcknowledgementState::Cancelled => prop_assert!(false, "this model emits no cancellation"),
                }
            }
            if rejected {
                prop_assert_eq!(&modeled, &before, "rejection mutated acknowledgement state");
            }
        }
    }
}
