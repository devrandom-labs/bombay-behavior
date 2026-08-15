//! Multi-participant acknowledgement lifecycle correlation.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// Exhaustive lifecycle phase for one acknowledgement key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcknowledgementState<P> {
    /// Some required participants have not acknowledged.
    Pending {
        /// Participants still required, in declaration order.
        remaining: Vec<P>,
        /// Participants already accepted, in acknowledgement order.
        acknowledged: Vec<P>,
    },
    /// Every declared participant acknowledged.
    Completed,
    /// The pending lifecycle was explicitly cancelled.
    Cancelled,
}

/// One retained acknowledgement lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcknowledgementRecord<K, P> {
    /// Application-defined correlation key.
    pub key: K,
    /// Complete current phase.
    pub state: AcknowledgementState<P>,
}

/// Typed rejection of an acknowledgement operation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AcknowledgementError<K, P> {
    /// A lifecycle already exists for this key.
    #[error("acknowledgement key already exists")]
    Existing {
        /// Rejected key.
        key: K,
    },
    /// No lifecycle exists for this key.
    #[error("acknowledgement key is unknown")]
    Unknown {
        /// Rejected key.
        key: K,
    },
    /// The participant was not declared for the pending lifecycle.
    #[error("participant is not required by this acknowledgement")]
    UnexpectedParticipant {
        /// Correlation key.
        key: K,
        /// Rejected participant.
        participant: P,
    },
    /// The participant already acknowledged.
    #[error("participant acknowledgement is a duplicate")]
    DuplicateParticipant {
        /// Correlation key.
        key: K,
        /// Rejected participant.
        participant: P,
    },
    /// An operation targeted a completed lifecycle.
    #[error("acknowledgement lifecycle is already complete")]
    Completed {
        /// Correlation key.
        key: K,
    },
    /// An operation targeted a cancelled lifecycle.
    #[error("acknowledgement lifecycle is cancelled")]
    Cancelled {
        /// Correlation key.
        key: K,
    },
}

/// Complete result of one acknowledgement operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcknowledgementOutcome<K, P> {
    /// A new lifecycle was accepted.
    Started {
        /// Correlation key.
        key: K,
        /// Number of distinct participants still required.
        remaining: usize,
    },
    /// One participant was accepted and requirements remain.
    Acknowledged {
        /// Correlation key.
        key: K,
        /// Accepted participant.
        participant: P,
        /// Number still required.
        remaining: usize,
    },
    /// The lifecycle reached its terminal successful phase.
    Completed {
        /// Correlation key.
        key: K,
    },
    /// The lifecycle reached its terminal cancelled phase.
    Cancelled {
        /// Correlation key.
        key: K,
    },
    /// The operation was rejected without changing state.
    Rejected(AcknowledgementError<K, P>),
}

/// Operations accepted by [`Acknowledgements`].
pub enum AcknowledgementMessage<K, P, Reply: Behavior> {
    /// Establish a fresh acknowledgement lifecycle.
    Begin {
        /// Correlation key.
        key: K,
        /// Required participants; duplicates are normalized by first occurrence.
        participants: Vec<P>,
        /// Typed outcome recipient.
        reply_to: Recipient<Reply>,
    },
    /// Record one participant acknowledgement.
    Acknowledge {
        /// Correlation key.
        key: K,
        /// Participant making the acknowledgement.
        participant: P,
        /// Typed outcome recipient.
        reply_to: Recipient<Reply>,
    },
    /// Cancel a pending lifecycle.
    Cancel {
        /// Correlation key.
        key: K,
        /// Typed outcome recipient.
        reply_to: Recipient<Reply>,
    },
}

/// Typed multi-participant acknowledgement behavior.
///
/// `Begin` normalizes participant membership by first occurrence. An empty set
/// completes immediately. Each declared participant can advance a pending
/// lifecycle exactly once; the final acknowledgement atomically commits
/// [`AcknowledgementState::Completed`] and reports completion. Cancellation is
/// accepted only while pending. Completed and cancelled records are retained,
/// making stale terminal input distinguishable from an unknown key. Every
/// rejection is emitted as a concrete [`AcknowledgementError`] and leaves the
/// state unchanged. Initialization is empty, the template creates no actors,
/// and it does not terminate its host. Membership normalization, terminal
/// retention, and acknowledgement ordering are Bombay policy. Delivery remains
/// a runtime capability, and transitions have no panic path.
pub struct Acknowledgements<
    A: Address,
    K,
    P,
    Reply: Behavior<Addr = A, Msg = AcknowledgementOutcome<K, P>>,
> {
    records: Vec<AcknowledgementRecord<K, P>>,
    marker: core::marker::PhantomData<fn() -> (A, Reply)>,
}

impl<A, K, P, Reply> Acknowledgements<A, K, P, Reply>
where
    A: Address,
    K: Clone + Eq,
    P: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = AcknowledgementOutcome<K, P>>,
{
    /// Construct an empty acknowledgement table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow every pending or retained terminal lifecycle.
    #[must_use]
    pub fn records(&self) -> &[AcknowledgementRecord<K, P>] {
        &self.records
    }

    fn result(
        reply_to: Recipient<Reply>,
        outcome: AcknowledgementOutcome<K, P>,
    ) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        Actions::send(vec![Delivery::new(reply_to, outcome)])
    }

    fn begin(
        &mut self,
        key: K,
        participants: Vec<P>,
        reply_to: Recipient<Reply>,
    ) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        if self.records.iter().any(|record| record.key == key) {
            return Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::Existing { key }),
            );
        }
        let mut distinct = Vec::new();
        for participant in participants {
            if !distinct.contains(&participant) {
                distinct.push(participant);
            }
        }
        let remaining = distinct.len();
        let state = if distinct.is_empty() {
            AcknowledgementState::Completed
        } else {
            AcknowledgementState::Pending {
                remaining: distinct,
                acknowledged: Vec::new(),
            }
        };
        self.records.push(AcknowledgementRecord {
            key: key.clone(),
            state,
        });
        let outcome = if remaining == 0 {
            AcknowledgementOutcome::Completed { key }
        } else {
            AcknowledgementOutcome::Started { key, remaining }
        };
        Self::result(reply_to, outcome)
    }

    fn acknowledge(
        &mut self,
        key: K,
        participant: P,
        reply_to: Recipient<Reply>,
    ) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        let Some(record) = self.records.iter_mut().find(|record| record.key == key) else {
            return Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::Unknown { key }),
            );
        };
        let (remaining, acknowledged) = match &mut record.state {
            AcknowledgementState::Pending {
                remaining,
                acknowledged,
            } => (remaining, acknowledged),
            AcknowledgementState::Completed => {
                return Self::result(
                    reply_to,
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Completed { key }),
                );
            }
            AcknowledgementState::Cancelled => {
                return Self::result(
                    reply_to,
                    AcknowledgementOutcome::Rejected(AcknowledgementError::Cancelled { key }),
                );
            }
        };
        if acknowledged.contains(&participant) {
            return Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::DuplicateParticipant {
                    key,
                    participant,
                }),
            );
        }
        let Some(index) = remaining
            .iter()
            .position(|required| required == &participant)
        else {
            return Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::UnexpectedParticipant {
                    key,
                    participant,
                }),
            );
        };
        remaining.remove(index);
        acknowledged.push(participant.clone());
        if remaining.is_empty() {
            record.state = AcknowledgementState::Completed;
            Self::result(reply_to, AcknowledgementOutcome::Completed { key })
        } else {
            Self::result(
                reply_to,
                AcknowledgementOutcome::Acknowledged {
                    key,
                    participant,
                    remaining: remaining.len(),
                },
            )
        }
    }

    fn cancel(
        &mut self,
        key: K,
        reply_to: Recipient<Reply>,
    ) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        let Some(record) = self.records.iter_mut().find(|record| record.key == key) else {
            return Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::Unknown { key }),
            );
        };
        match record.state {
            AcknowledgementState::Pending { .. } => {
                record.state = AcknowledgementState::Cancelled;
                Self::result(reply_to, AcknowledgementOutcome::Cancelled { key })
            }
            AcknowledgementState::Completed => Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::Completed { key }),
            ),
            AcknowledgementState::Cancelled => Self::result(
                reply_to,
                AcknowledgementOutcome::Rejected(AcknowledgementError::Cancelled { key }),
            ),
        }
    }
}

impl<A, K, P, Reply> Default for Acknowledgements<A, K, P, Reply>
where
    A: Address,
    K: Clone + Eq,
    P: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = AcknowledgementOutcome<K, P>>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, K, P, Reply> BehaviorBase for Acknowledgements<A, K, P, Reply>
where
    A: Address,
    K: Clone + Eq,
    P: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = AcknowledgementOutcome<K, P>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, P, Reply> Behavior for Acknowledgements<A, K, P, Reply>
where
    A: Address,
    K: Clone + Eq,
    P: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = AcknowledgementOutcome<K, P>>,
{
    type Addr = A;
    type Msg = AcknowledgementMessage<K, P, Reply>;
    type Event = User<A, Self::Msg>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event.message {
            AcknowledgementMessage::Begin {
                key,
                participants,
                reply_to,
            } => self.begin(key, participants, reply_to),
            AcknowledgementMessage::Acknowledge {
                key,
                participant,
                reply_to,
            } => self.acknowledge(key, participant, reply_to),
            AcknowledgementMessage::Cancel { key, reply_to } => self.cancel(key, reply_to),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use behavior::MailAddr;

    struct Reply;
    impl Behavior for Reply {
        type Addr = MailAddr;
        type Msg = AcknowledgementOutcome<u8, u8>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    type Subject = Acknowledgements<MailAddr, u8, u8, Reply>;
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(1))
    }

    #[test]
    fn membership_is_normalized_and_completion_is_terminal() {
        let mut subject = crate::Compose::new(Subject::new())
            .initialize()
            .unwrap()
            .behavior;
        let started = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Begin {
                    key: 7,
                    participants: vec![1, 1, 2],
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            started.sends[0].message,
            AcknowledgementOutcome::Started {
                key: 7,
                remaining: 2
            }
        ));
        let first = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Acknowledge {
                    key: 7,
                    participant: 1,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            first.sends[0].message,
            AcknowledgementOutcome::Acknowledged { remaining: 1, .. }
        ));
        let completed = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Acknowledge {
                    key: 7,
                    participant: 2,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            completed.sends[0].message,
            AcknowledgementOutcome::Completed { key: 7 }
        ));
        let stale = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Acknowledge {
                    key: 7,
                    participant: 2,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            stale.sends[0].message,
            AcknowledgementOutcome::Rejected(AcknowledgementError::Completed { key: 7 })
        ));
    }

    #[test]
    fn rejection_and_cancellation_do_not_conflate_phases() {
        let mut subject = crate::Compose::new(Subject::new())
            .initialize()
            .unwrap()
            .behavior;
        let _ = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Begin {
                    key: 1,
                    participants: vec![3],
                    reply_to: reply(),
                },
            )
            .unwrap();
        let unexpected = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Acknowledge {
                    key: 1,
                    participant: 4,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            unexpected.sends[0].message,
            AcknowledgementOutcome::Rejected(AcknowledgementError::UnexpectedParticipant { .. })
        ));
        let cancelled = subject
            .receive(
                MailAddr(9),
                AcknowledgementMessage::Cancel {
                    key: 1,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            cancelled.sends[0].message,
            AcknowledgementOutcome::Cancelled { key: 1 }
        ));
        assert!(matches!(
            subject.records()[0].state,
            AcknowledgementState::Cancelled
        ));
    }
}
