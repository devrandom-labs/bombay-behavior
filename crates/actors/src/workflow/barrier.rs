//! Fixed-membership cyclic barrier coordination.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol,
    SendEffects, User,
};
#[cfg(test)]
use behavior::{Delivery, Recipient};
use thiserror::Error;

use crate::DeliveryRoute;

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
pub struct BarrierMessage<K, Route> {
    /// Generation the participant intends to join.
    pub generation: BarrierGeneration,
    /// Participant key from the barrier's fixed membership.
    pub participant: K,
    /// Typed recipient for this generation's release.
    pub reply_to: Route,
}

/// One accepted arrival retained until its generation releases.
pub struct BarrierArrival<K, Route> {
    /// Fixed-membership key.
    pub participant: K,
    /// Typed release recipient.
    pub reply_to: Route,
}

/// Complete semantic state of a cyclic [`Barrier`].
pub enum BarrierState<K, Route> {
    /// The current generation is accepting its fixed membership once each.
    Gathering {
        /// Exact accepted generation.
        generation: BarrierGeneration,
        /// Accepted arrivals in arrival order.
        arrivals: Vec<BarrierArrival<K, Route>>,
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

/// Validated fixed barrier membership, independent of an address namespace.
pub struct BarrierMembership<K> {
    members: Vec<K>,
}

impl<K: Clone + Eq> BarrierMembership<K> {
    /// Validate fixed membership before binding it to an actor protocol.
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
        Ok(Self { members })
    }
}

/// Rejected barrier arrival.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BarrierError<K, Route> {
    /// The participant is not in fixed membership.
    #[error("barrier participant is unknown")]
    UnknownParticipant { participant: K, reply_to: Route },
    /// The participant already arrived in this generation.
    #[error("barrier participant already arrived in this generation")]
    DuplicateArrival {
        /// Duplicate participant key.
        participant: K,
        /// Current barrier generation.
        generation: BarrierGeneration,
        /// Exact release recipient from the rejected arrival.
        reply_to: Route,
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
        /// Exact release recipient from the rejected arrival.
        reply_to: Route,
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
        /// Exact release recipient from the rejected arrival.
        reply_to: Route,
    },
    /// No representable successor generation remains.
    #[error("barrier generations are exhausted")]
    Exhausted {
        /// Participant whose arrival was rejected.
        participant: K,
        /// Final released generation.
        generation: BarrierGeneration,
        /// Exact release recipient from the rejected arrival.
        reply_to: Route,
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
pub struct Barrier<A, K, Route>
where
    A: Address,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = BarrierReleased>,
{
    members: Vec<K>,
    state: BarrierState<K, Route>,
    marker: core::marker::PhantomData<fn() -> A>,
}

impl<A, K, Route> Barrier<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = BarrierReleased>,
{
    /// Bind validated membership to generation zero of a barrier actor.
    #[must_use]
    pub fn new(membership: BarrierMembership<K>) -> Self {
        Self {
            members: membership.members,
            state: BarrierState::Gathering {
                generation: BarrierGeneration(0),
                arrivals: Vec::new(),
            },
            marker: core::marker::PhantomData,
        }
    }

    /// Fixed participant keys in definition order.
    #[must_use]
    pub fn members(&self) -> &[K] {
        &self.members
    }

    /// Borrow the complete current generation state.
    #[must_use]
    pub const fn state(&self) -> &BarrierState<K, Route> {
        &self.state
    }
}

impl<A, K, Route> BehaviorBase for Barrier<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = BarrierReleased>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, Route> behavior::Protocol for Barrier<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = BarrierReleased>,
{
    type Addr = A;
    type Msg = BarrierMessage<K, Route>;
}

impl<A, K, Route> Behavior for Barrier<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = BarrierReleased>,
    Route::Sends: behavior::SendsFor<User<A, BarrierMessage<K, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Route::Sends;
    type Ph = Never;
    type Error = BarrierError<K, Route>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let BarrierMessage {
            generation: observed,
            participant,
            reply_to,
        } = event.message;
        if !self.members.contains(&participant) {
            return Err(BarrierError::UnknownParticipant {
                participant,
                reply_to,
            });
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
                    reply_to,
                });
            }
        };
        if observed < *current {
            return Err(BarrierError::StaleGeneration {
                participant,
                observed,
                current: *current,
                reply_to,
            });
        }
        if observed > *current {
            return Err(BarrierError::FutureGeneration {
                participant,
                observed,
                current: *current,
                reply_to,
            });
        }
        if arrivals
            .iter()
            .any(|arrival| arrival.participant == participant)
        {
            return Err(BarrierError::DuplicateArrival {
                participant,
                generation: *current,
                reply_to,
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
        let mut sends = Route::Sends::empty();
        for arrival in completed {
            sends.append(arrival.reply_to.deliver(BarrierReleased { generation }));
        }
        Ok(Actions::send(sends))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    #[test]
    fn configuration_rejects_empty_and_duplicate_membership() {
        assert!(matches!(
            BarrierMembership::new(Vec::<u8>::new()),
            Err(BarrierConfigError::EmptyMembership)
        ));
        assert!(matches!(
            BarrierMembership::new(vec![1, 2, 1]),
            Err(BarrierConfigError::DuplicateParticipant(1))
        ));
    }

    #[test]
    fn generation_releases_exact_membership_in_arrival_order() {
        let one = Recipient::from(MailAddr(1));
        let two = Recipient::from(MailAddr(2));
        let mut barrier = Barrier::new(BarrierMembership::new(vec![1_u8, 2]).unwrap())
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
                reply_to,
            }) if reply_to == two
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
                reply_to,
                ..
            }) if reply_to == one
        ));
    }

    #[test]
    fn final_generation_releases_then_exhausts_without_wraparound() {
        let participant = Recipient::from(MailAddr(1));
        let mut definition = Barrier::new(BarrierMembership::new(vec![1_u8]).unwrap());
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
