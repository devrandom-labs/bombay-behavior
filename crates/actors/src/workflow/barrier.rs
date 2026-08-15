//! Fixed-membership cyclic barrier coordination.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// Explicit barrier generation carried by every arrival and release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BarrierGeneration(pub u64);

/// Release fact delivered to every participant in a completed generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BarrierReleased {
    /// Exact generation whose complete membership arrived.
    pub generation: BarrierGeneration,
}

/// One keyed arrival at a [`Barrier`].
pub struct BarrierMessage<K, Participant: Behavior> {
    /// Generation the participant intends to join.
    pub generation: BarrierGeneration,
    /// Participant key from the barrier's fixed membership.
    pub participant: K,
    /// Typed recipient for this generation's release.
    pub reply_to: Recipient<Participant>,
}

/// One accepted arrival retained until its generation releases.
pub struct BarrierArrival<K, Participant: Behavior> {
    /// Fixed-membership key.
    pub participant: K,
    /// Typed release recipient.
    pub reply_to: Recipient<Participant>,
}

/// Complete semantic state of a cyclic [`Barrier`].
pub enum BarrierState<K, Participant: Behavior> {
    /// The current generation is accepting its fixed membership once each.
    Gathering {
        /// Exact accepted generation.
        generation: BarrierGeneration,
        /// Accepted arrivals in arrival order.
        arrivals: Vec<BarrierArrival<K, Participant>>,
    },
    /// The final representable generation released; no later generation exists.
    Exhausted {
        /// Final released generation.
        generation: BarrierGeneration,
    },
}

/// Invalid fixed barrier definition.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BarrierConfigError<K> {
    /// A barrier without participants has no meaningful release transition.
    #[error("barrier membership must not be empty")]
    EmptyMembership,
    /// One participant key appeared more than once.
    #[error("barrier membership contains a duplicate participant")]
    DuplicateParticipant(K),
}

/// Rejected barrier arrival.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BarrierError<K> {
    /// The participant is not in fixed membership.
    #[error("barrier participant is unknown")]
    UnknownParticipant(K),
    /// The participant already arrived in this generation.
    #[error("barrier participant already arrived in this generation")]
    DuplicateArrival {
        /// Duplicate participant key.
        participant: K,
        /// Current barrier generation.
        generation: BarrierGeneration,
    },
    /// Arrival belongs to a generation already released.
    #[error("barrier arrival is stale")]
    StaleGeneration {
        /// Participant whose arrival was rejected.
        participant: K,
        /// Generation carried by the arrival.
        observed: BarrierGeneration,
        /// Current accepted generation.
        current: BarrierGeneration,
    },
    /// Arrival attempts to skip the current generation.
    #[error("barrier arrival is for a future generation")]
    FutureGeneration {
        /// Participant whose arrival was rejected.
        participant: K,
        /// Generation carried by the arrival.
        observed: BarrierGeneration,
        /// Current accepted generation.
        current: BarrierGeneration,
    },
    /// No representable successor generation remains.
    #[error("barrier generations are exhausted")]
    Exhausted {
        /// Participant whose arrival was rejected.
        participant: K,
        /// Final released generation.
        generation: BarrierGeneration,
    },
}

/// Fixed-membership cyclic barrier behavior.
///
/// Each generation accepts every configured key exactly once. The final
/// arrival commits the successor generation before emitting one
/// [`BarrierReleased`] delivery to every arrival recipient in arrival order.
/// Stale, future, duplicate, unknown, and exhausted arrivals are distinct
/// typed errors and preserve state. At `u64::MAX`, the final release commits
/// [`BarrierState::Exhausted`] instead of inferring wraparound. Initialization
/// is empty and the behavior never terminates itself. Membership, generation,
/// and release ordering are Bombay workflow policy; delivery is interpreted
/// by Address and Communication. Construction rejects empty or duplicate
/// membership. No method has a semantic panic condition.
pub struct Barrier<A: Address, K, Participant: Behavior<Addr = A, Msg = BarrierReleased>> {
    members: Vec<K>,
    state: BarrierState<K, Participant>,
    address: core::marker::PhantomData<A>,
}

impl<A, K, Participant> Barrier<A, K, Participant>
where
    A: Address,
    K: Clone + Eq,
    Participant: Behavior<Addr = A, Msg = BarrierReleased>,
{
    /// Construct generation zero with fixed membership order.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty or duplicate membership.
    pub fn new(members: Vec<K>) -> Result<Self, BarrierConfigError<K>> {
        if members.is_empty() {
            return Err(BarrierConfigError::EmptyMembership);
        }
        for (index, participant) in members.iter().enumerate() {
            if members[..index].contains(participant) {
                return Err(BarrierConfigError::DuplicateParticipant(
                    participant.clone(),
                ));
            }
        }
        Ok(Self {
            members,
            state: BarrierState::Gathering {
                generation: BarrierGeneration(0),
                arrivals: Vec::new(),
            },
            address: core::marker::PhantomData,
        })
    }

    /// Fixed participant keys in definition order.
    #[must_use]
    pub fn members(&self) -> &[K] {
        &self.members
    }

    /// Borrow the complete current generation state.
    #[must_use]
    pub const fn state(&self) -> &BarrierState<K, Participant> {
        &self.state
    }
}

impl<A, K, Participant> BehaviorBase for Barrier<A, K, Participant>
where
    A: Address,
    K: Clone + Eq,
    Participant: Behavior<Addr = A, Msg = BarrierReleased>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, Participant> Behavior for Barrier<A, K, Participant>
where
    A: Address,
    K: Clone + Eq,
    Participant: Behavior<Addr = A, Msg = BarrierReleased>,
{
    type Addr = A;
    type Msg = BarrierMessage<K, Participant>;
    type Event = User<A, Self::Msg>;
    type Sends = Vec<Delivery<Participant>>;
    type Ph = Never;
    type Error = BarrierError<K>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let BarrierMessage {
            generation: observed,
            participant,
            reply_to,
        } = event.message;
        if !self.members.contains(&participant) {
            return Err(BarrierError::UnknownParticipant(participant));
        }
        let (current, arrivals) = match &mut self.state {
            BarrierState::Gathering {
                generation,
                arrivals,
            } => (generation, arrivals),
            BarrierState::Exhausted { generation } => {
                return Err(BarrierError::Exhausted {
                    participant,
                    generation: *generation,
                });
            }
        };
        if observed < *current {
            return Err(BarrierError::StaleGeneration {
                participant,
                observed,
                current: *current,
            });
        }
        if observed > *current {
            return Err(BarrierError::FutureGeneration {
                participant,
                observed,
                current: *current,
            });
        }
        if arrivals
            .iter()
            .any(|arrival| arrival.participant == participant)
        {
            return Err(BarrierError::DuplicateArrival {
                participant,
                generation: *current,
            });
        }
        arrivals.push(BarrierArrival {
            participant,
            reply_to,
        });
        if arrivals.len() < self.members.len() {
            return Ok(Actions::cont());
        }
        let generation = *current;
        let completed = core::mem::take(arrivals);
        self.state =
            generation
                .0
                .checked_add(1)
                .map_or(BarrierState::Exhausted { generation }, |next| {
                    BarrierState::Gathering {
                        generation: BarrierGeneration(next),
                        arrivals: Vec::with_capacity(self.members.len()),
                    }
                });
        Ok(Actions::send(
            completed
                .into_iter()
                .map(|arrival| Delivery::new(arrival.reply_to, BarrierReleased { generation }))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    struct Participant;

    impl Behavior for Participant {
        type Addr = MailAddr;
        type Msg = BarrierReleased;
        type Event = User<MailAddr, BarrierReleased>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    type TestBarrier = Barrier<MailAddr, u8, Participant>;

    #[test]
    fn configuration_rejects_empty_and_duplicate_membership() {
        assert!(matches!(
            TestBarrier::new(Vec::new()),
            Err(BarrierConfigError::EmptyMembership)
        ));
        assert!(matches!(
            TestBarrier::new(vec![1, 2, 1]),
            Err(BarrierConfigError::DuplicateParticipant(1))
        ));
    }

    #[test]
    fn generation_releases_exact_membership_in_arrival_order() {
        let one = Recipient::<Participant>::global(MailAddr(1));
        let two = Recipient::<Participant>::global(MailAddr(2));
        let mut barrier = (TestBarrier::new(vec![1, 2]).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        let first = barrier
            .receive(
                MailAddr(9),
                BarrierMessage {
                    generation: BarrierGeneration(0),
                    participant: 2,
                    reply_to: two,
                },
            )
            .unwrap();
        assert!(first.sends.is_empty());
        assert!(matches!(
            barrier.receive(
                MailAddr(9),
                BarrierMessage {
                    generation: BarrierGeneration(0),
                    participant: 2,
                    reply_to: two,
                },
            ),
            Err(BarrierError::DuplicateArrival {
                participant: 2,
                generation: BarrierGeneration(0),
            })
        ));
        let released = barrier
            .receive(
                MailAddr(9),
                BarrierMessage {
                    generation: BarrierGeneration(0),
                    participant: 1,
                    reply_to: one,
                },
            )
            .unwrap();
        assert!(
            released.sends
                == vec![
                    Delivery::new(
                        two,
                        BarrierReleased {
                            generation: BarrierGeneration(0),
                        },
                    ),
                    Delivery::new(
                        one,
                        BarrierReleased {
                            generation: BarrierGeneration(0),
                        },
                    ),
                ]
        );
        assert!(matches!(
            barrier.state(),
            BarrierState::Gathering {
                generation: BarrierGeneration(1),
                arrivals,
            } if arrivals.is_empty()
        ));
        assert!(matches!(
            barrier.receive(
                MailAddr(9),
                BarrierMessage {
                    generation: BarrierGeneration(0),
                    participant: 1,
                    reply_to: one,
                },
            ),
            Err(BarrierError::StaleGeneration {
                current: BarrierGeneration(1),
                ..
            })
        ));
    }

    #[test]
    fn final_generation_releases_then_exhausts_without_wraparound() {
        let participant = Recipient::<Participant>::global(MailAddr(1));
        let mut definition = TestBarrier::new(vec![1]).unwrap();
        definition.state = BarrierState::Gathering {
            generation: BarrierGeneration(u64::MAX),
            arrivals: Vec::new(),
        };
        let mut barrier = (definition).initialize().unwrap().behavior;
        let released = barrier
            .receive(
                MailAddr(9),
                BarrierMessage {
                    generation: BarrierGeneration(u64::MAX),
                    participant: 1,
                    reply_to: participant,
                },
            )
            .unwrap();
        assert_eq!(released.sends.len(), 1);
        assert!(matches!(
            barrier.state(),
            BarrierState::Exhausted {
                generation: BarrierGeneration(u64::MAX),
            }
        ));
    }
}
