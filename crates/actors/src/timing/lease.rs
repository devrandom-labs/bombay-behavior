//! Expiring exclusive ownership over explicit timer generations.

use std::time::Duration;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    SendAlgebra, ServiceSends, User,
};
use thiserror::Error;

use crate::{ScheduleAfter, TimedEvent, TimerGeneration, TimerId};

/// Complete exclusive lease phase.
pub enum LeaseState<K, Reply: Behavior> {
    /// No holder owns the lease.
    Vacant,
    /// One holder owns exactly this timer generation.
    Held {
        /// Application-defined holder identity.
        holder: K,
        /// Current expiry generation.
        generation: TimerGeneration,
        /// Recipient notified of expiry.
        notify: Recipient<Reply>,
    },
    /// No fresh expiry generation is representable and no holder exists.
    Exhausted,
}

/// Typed lease-operation rejection.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LeaseRejection<K> {
    /// Another holder currently owns the lease.
    #[error("lease is occupied")]
    Occupied {
        /// Rejected requested holder.
        requested: K,
    },
    /// Operation named a holder other than the current holder.
    #[error("lease holder does not match")]
    WrongHolder {
        /// Rejected holder.
        requested: K,
    },
    /// Operation names an old or future generation.
    #[error("lease generation is stale")]
    StaleGeneration {
        /// Rejected generation.
        observed: TimerGeneration,
        /// Current generation.
        current: TimerGeneration,
    },
    /// No holder currently exists.
    #[error("lease is vacant")]
    Vacant,
    /// The finite generation domain has been consumed.
    #[error("lease generation domain is exhausted")]
    GenerationExhausted,
}

/// Factual lease operation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseOutcome<K> {
    /// Fresh ownership was committed and expiry requested.
    Acquired {
        /// Holder.
        holder: K,
        /// Committed generation.
        generation: TimerGeneration,
    },
    /// Existing ownership was renewed into a fresh generation.
    Renewed {
        /// Holder.
        holder: K,
        /// Fresh committed generation.
        generation: TimerGeneration,
    },
    /// Ownership was explicitly relinquished.
    Released {
        /// Former holder.
        holder: K,
        /// Released generation.
        generation: TimerGeneration,
    },
    /// Matching timer evidence expired ownership.
    Expired {
        /// Former holder.
        holder: K,
        /// Expired generation.
        generation: TimerGeneration,
    },
    /// Operation was rejected without changing state.
    Rejected(LeaseRejection<K>),
}

/// User commands accepted by [`Lease`].
pub enum LeaseMessage<K, Reply: Behavior> {
    /// Acquire a vacant lease for a relative duration.
    Acquire {
        /// Requested holder.
        holder: K,
        /// Relative duration interpreted by Timers.
        duration: Duration,
        /// Outcome and expiry recipient.
        reply_to: Recipient<Reply>,
    },
    /// Renew the exact current incarnation.
    Renew {
        /// Claimed holder.
        holder: K,
        /// Claimed current generation.
        generation: TimerGeneration,
        /// New relative duration.
        duration: Duration,
        /// Outcome and expiry recipient.
        reply_to: Recipient<Reply>,
    },
    /// Explicitly release the exact current incarnation.
    Release {
        /// Claimed holder.
        holder: K,
        /// Claimed current generation.
        generation: TimerGeneration,
        /// Outcome recipient.
        reply_to: Recipient<Reply>,
    },
}

/// Named effect lanes emitted by [`Lease`].
pub struct LeaseSends<Reply: Behavior> {
    /// Lease facts.
    pub outcomes: Vec<Delivery<Reply>>,
    /// Relative expiry requests.
    pub schedules: ServiceSends<ScheduleAfter>,
}
impl<Reply: Behavior> SendAlgebra for LeaseSends<Reply> {
    fn empty() -> Self {
        Self {
            outcomes: Vec::new(),
            schedules: ServiceSends::empty(),
        }
    }
    fn append(&mut self, mut other: Self) {
        self.outcomes.append(&mut other.outcomes);
        self.schedules.append(other.schedules);
    }
}

/// Exclusive expiring ownership behavior.
///
/// Acquisition is accepted only while vacant and commits a fresh generation
/// before emitting its schedule. Renewal and release require the exact holder
/// and generation. Matching elapsed evidence expires ownership; wrong timer IDs
/// and stale generations are inert. Release cannot retract an already queued
/// elapsed observation, so later evidence is explicitly stale. Once the finite
/// generation domain is consumed, the vacant state becomes `Exhausted` and no
/// successful ownership can be created. Initialization is empty, no actors are
/// created, and the host never terminates by policy. Exclusivity, checked
/// generation progression, and commit-before-schedule ordering are Bombay
/// policy. Scheduling and sleeping belong to Timers. The current Bombay timer
/// adapter has no cancellation effect lane; release is semantically immediate
/// while the stale queue entry may remain until due. No transition panics.
pub struct Lease<A: Address, K: Clone + Eq, Reply: Behavior<Addr = A, Msg = LeaseOutcome<K>>> {
    id: TimerId,
    state: LeaseState<K, Reply>,
    next: Option<u64>,
    marker: core::marker::PhantomData<fn() -> A>,
}
type LeaseActions<A, Reply> = Actions<A, Never, LeaseSends<Reply>, NoBirths>;
impl<A, K, Reply> Lease<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = LeaseOutcome<K>>,
{
    /// Construct a vacant lease using one actor-local timer key.
    #[must_use]
    pub const fn new(id: TimerId) -> Self {
        Self {
            id,
            state: LeaseState::Vacant,
            next: Some(0),
            marker: core::marker::PhantomData,
        }
    }
    /// Borrow the complete ownership phase.
    #[must_use]
    pub const fn state(&self) -> &LeaseState<K, Reply> {
        &self.state
    }
    fn result(reply_to: Recipient<Reply>, outcome: LeaseOutcome<K>) -> LeaseActions<A, Reply> {
        Actions::send(LeaseSends {
            outcomes: vec![Delivery::new(reply_to, outcome)],
            schedules: ServiceSends::empty(),
        })
    }
    fn acquire(
        &mut self,
        holder: K,
        duration: Duration,
        reply_to: Recipient<Reply>,
    ) -> LeaseActions<A, Reply> {
        match &self.state {
            LeaseState::Held { .. } => {
                return Self::result(
                    reply_to,
                    LeaseOutcome::Rejected(LeaseRejection::Occupied { requested: holder }),
                );
            }
            LeaseState::Exhausted => {
                return Self::result(
                    reply_to,
                    LeaseOutcome::Rejected(LeaseRejection::GenerationExhausted),
                );
            }
            LeaseState::Vacant => {}
        }
        let Some(raw) = self.next else {
            self.state = LeaseState::Exhausted;
            return Self::result(
                reply_to,
                LeaseOutcome::Rejected(LeaseRejection::GenerationExhausted),
            );
        };
        let generation = TimerGeneration(raw);
        self.next = raw.checked_add(1);
        self.state = LeaseState::Held {
            holder: holder.clone(),
            generation,
            notify: reply_to,
        };
        Actions::send(LeaseSends {
            outcomes: vec![Delivery::new(
                reply_to,
                LeaseOutcome::Acquired { holder, generation },
            )],
            schedules: ServiceSends::one(ScheduleAfter::new(self.id, generation, duration)),
        })
    }
    fn validate(&self, holder: &K, generation: TimerGeneration) -> Result<(), LeaseRejection<K>> {
        match &self.state {
            LeaseState::Vacant => Err(LeaseRejection::Vacant),
            LeaseState::Exhausted => Err(LeaseRejection::GenerationExhausted),
            LeaseState::Held {
                holder: current, ..
            } if current != holder => Err(LeaseRejection::WrongHolder {
                requested: holder.clone(),
            }),
            LeaseState::Held {
                generation: active, ..
            } if *active != generation => Err(LeaseRejection::StaleGeneration {
                observed: generation,
                current: *active,
            }),
            LeaseState::Held { .. } => Ok(()),
        }
    }
}
impl<A, K, Reply> BehaviorBase for Lease<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = LeaseOutcome<K>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, K, Reply> Behavior for Lease<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: Behavior<Addr = A, Msg = LeaseOutcome<K>>,
{
    type Addr = A;
    type Msg = LeaseMessage<K, Reply>;
    type Event = TimedEvent<User<A, Self::Msg>>;
    type Sends = LeaseSends<Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event {
            TimedEvent::Behavior(event) => match event.message {
                LeaseMessage::Acquire {
                    holder,
                    duration,
                    reply_to,
                } => self.acquire(holder, duration, reply_to),
                LeaseMessage::Renew {
                    holder,
                    generation,
                    duration,
                    reply_to,
                } => {
                    if let Err(rejection) = self.validate(&holder, generation) {
                        return Ok(Self::result(reply_to, LeaseOutcome::Rejected(rejection)));
                    }
                    let Some(raw) = self.next else {
                        return Ok(Self::result(
                            reply_to,
                            LeaseOutcome::Rejected(LeaseRejection::GenerationExhausted),
                        ));
                    };
                    let fresh = TimerGeneration(raw);
                    self.next = raw.checked_add(1);
                    self.state = LeaseState::Held {
                        holder: holder.clone(),
                        generation: fresh,
                        notify: reply_to,
                    };
                    Actions::send(LeaseSends {
                        outcomes: vec![Delivery::new(
                            reply_to,
                            LeaseOutcome::Renewed {
                                holder,
                                generation: fresh,
                            },
                        )],
                        schedules: ServiceSends::one(ScheduleAfter::new(self.id, fresh, duration)),
                    })
                }
                LeaseMessage::Release {
                    holder,
                    generation,
                    reply_to,
                } => {
                    if let Err(rejection) = self.validate(&holder, generation) {
                        return Ok(Self::result(reply_to, LeaseOutcome::Rejected(rejection)));
                    }
                    self.state = if self.next.is_some() {
                        LeaseState::Vacant
                    } else {
                        LeaseState::Exhausted
                    };
                    Self::result(reply_to, LeaseOutcome::Released { holder, generation })
                }
            },
            TimedEvent::Elapsed(elapsed) => {
                if elapsed.id != self.id {
                    return Ok(Actions::cont());
                }
                let LeaseState::Held {
                    holder,
                    generation,
                    notify,
                } = &self.state
                else {
                    return Ok(Actions::cont());
                };
                if elapsed.generation != *generation {
                    return Ok(Actions::cont());
                }
                let outcome = LeaseOutcome::Expired {
                    holder: holder.clone(),
                    generation: *generation,
                };
                let recipient = *notify;
                self.state = if self.next.is_some() {
                    LeaseState::Vacant
                } else {
                    LeaseState::Exhausted
                };
                Self::result(recipient, outcome)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Activate as _, TimerElapsed};
    use behavior::MailAddr;
    struct Reply;
    impl Behavior for Reply {
        type Addr = MailAddr;
        type Msg = LeaseOutcome<u8>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = Lease<MailAddr, u8, Reply>;
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(1))
    }
    fn duration() -> Duration {
        Duration::from_secs(1)
    }
    #[test]
    fn acquire_renew_release_and_stale_elapsed_are_generation_safe() {
        let mut s = (Subject::new(TimerId(7))).initialize().unwrap().behavior;
        let acquired = s
            .receive(
                MailAddr(0),
                LeaseMessage::Acquire {
                    holder: 1,
                    duration: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(
            acquired.sends.schedules.as_slice()[0].generation,
            TimerGeneration(0)
        );
        let renewed = s
            .receive(
                MailAddr(0),
                LeaseMessage::Renew {
                    holder: 1,
                    generation: TimerGeneration(0),
                    duration: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(
            renewed.sends.schedules.as_slice()[0].generation,
            TimerGeneration(1)
        );
        assert!(
            s.on(TimerElapsed::new(TimerId(7), TimerGeneration(0)))
                .unwrap()
                .sends
                .outcomes
                .is_empty()
        );
        s.receive(
            MailAddr(0),
            LeaseMessage::Release {
                holder: 1,
                generation: TimerGeneration(1),
                reply_to: reply(),
            },
        )
        .unwrap();
        assert!(matches!(s.state(), LeaseState::Vacant));
    }
    #[test]
    fn wrong_holder_and_matching_expiry_are_distinct() {
        let mut s = (Subject::new(TimerId(7))).initialize().unwrap().behavior;
        s.receive(
            MailAddr(0),
            LeaseMessage::Acquire {
                holder: 1,
                duration: duration(),
                reply_to: reply(),
            },
        )
        .unwrap();
        let wrong = s
            .receive(
                MailAddr(0),
                LeaseMessage::Release {
                    holder: 2,
                    generation: TimerGeneration(0),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            wrong.sends.outcomes[0].message,
            LeaseOutcome::Rejected(LeaseRejection::WrongHolder { requested: 2 })
        ));
        let expired = s
            .on(TimerElapsed::new(TimerId(7), TimerGeneration(0)))
            .unwrap();
        assert!(matches!(
            expired.sends.outcomes[0].message,
            LeaseOutcome::Expired { holder: 1, .. }
        ));
        assert!(matches!(s.state(), LeaseState::Vacant));
    }

    #[test]
    fn generation_exhaustion_is_terminal_and_never_wraps() {
        let mut definition = Subject::new(TimerId(7));
        definition.next = Some(u64::MAX);
        let mut subject = (definition).initialize().unwrap().behavior;
        subject
            .receive(
                MailAddr(0),
                LeaseMessage::Acquire {
                    holder: 1,
                    duration: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        subject
            .on(TimerElapsed::new(TimerId(7), TimerGeneration(u64::MAX)))
            .unwrap();
        assert!(matches!(subject.state(), LeaseState::Exhausted));
        let rejected = subject
            .receive(
                MailAddr(0),
                LeaseMessage::Acquire {
                    holder: 2,
                    duration: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            LeaseOutcome::Rejected(LeaseRejection::GenerationExhausted)
        ));
    }
}
