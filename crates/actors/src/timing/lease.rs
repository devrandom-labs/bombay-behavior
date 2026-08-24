//! Expiring exclusive ownership over explicit timer generations.

use std::time::Duration;

#[cfg(test)]
use behavior::Recipient;
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, EventLayer, InterpreterRequests,
    Never, NoBirths, SendEffects, User,
};
use thiserror::Error;

use crate::{DeliveryRoute, ScheduleAfter, TimedEvent, TimerGeneration, TimerId};

/// Complete exclusive lease phase.
pub enum LeaseState<K, Route> {
    /// No holder owns the lease.
    Vacant {
        /// Generation committed by the next successful acquisition.
        next: TimerGeneration,
    },
    /// One holder owns exactly this timer generation.
    Held {
        /// Application-defined holder identity.
        holder: K,
        /// Current expiry generation.
        generation: TimerGeneration,
        /// Recipient notified of expiry.
        notify: Route,
        /// Generation available to a renewal or later acquisition. `None`
        /// means this held generation is the final representable one.
        next: Option<TimerGeneration>,
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
        /// Holder that currently owns the lease.
        current: K,
    },
    /// Operation named a holder other than the current holder.
    #[error("lease holder does not match")]
    WrongHolder {
        /// Holder that currently owns the lease.
        current: K,
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

/// Complete lease operation retained when admission is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseRequest<K> {
    /// Attempt to acquire a vacant lease.
    Acquire {
        /// Requested holder.
        holder: K,
        /// Requested lease lifetime.
        duration: Duration,
    },
    /// Attempt to renew one exact held generation.
    Renew {
        /// Claimed holder.
        holder: K,
        /// Claimed current generation.
        generation: TimerGeneration,
        /// Requested renewed lifetime.
        duration: Duration,
    },
    /// Attempt to release one exact held generation.
    Release {
        /// Claimed holder.
        holder: K,
        /// Claimed current generation.
        generation: TimerGeneration,
    },
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
    Rejected {
        /// Complete operation that was not admitted.
        request: LeaseRequest<K>,
        /// Exhaustive reason derived from current lease state.
        reason: LeaseRejection<K>,
    },
}

/// User commands accepted by [`Lease`].
pub enum LeaseMessage<K, Route> {
    /// Acquire a vacant lease for a relative duration.
    Acquire {
        /// Requested holder.
        holder: K,
        /// Relative duration interpreted by Timers.
        duration: Duration,
        /// Outcome and expiry recipient.
        reply_to: Route,
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
        reply_to: Route,
    },
    /// Explicitly release the exact current incarnation.
    Release {
        /// Claimed holder.
        holder: K,
        /// Claimed current generation.
        generation: TimerGeneration,
        /// Outcome recipient.
        reply_to: Route,
    },
}

/// Named effect lanes emitted by [`Lease`].
pub struct LeaseSends<OutcomeSends: SendEffects> {
    /// Lease facts.
    pub outcomes: OutcomeSends,
    /// Relative expiry requests.
    pub schedules: InterpreterRequests<ScheduleAfter>,
}
impl<OutcomeSends: SendEffects> SendEffects for LeaseSends<OutcomeSends> {
    fn empty() -> Self {
        Self {
            outcomes: OutcomeSends::empty(),
            schedules: InterpreterRequests::empty(),
        }
    }
    fn append(&mut self, other: Self) {
        self.outcomes.append(other.outcomes);
        self.schedules.append(other.schedules);
    }
}

impl<Event, OutcomeSends> behavior::SendsFor<Event> for LeaseSends<OutcomeSends>
where
    OutcomeSends: SendEffects + behavior::SendsFor<Event>,
    InterpreterRequests<ScheduleAfter>: behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, OutcomeSends> behavior::InterpretSends<I, RootEvent, Path>
    for LeaseSends<OutcomeSends>
where
    I: behavior::SendInterpreter,
    OutcomeSends: SendEffects + behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ScheduleAfter>: behavior::InterpretSends<I, RootEvent, Path>,
    LeaseSends<OutcomeSends>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.outcomes, interpreter).await?;
            behavior::InterpretSends::interpret(self.schedules, interpreter).await
        }
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
pub struct Lease<
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute<Protocol: behavior::Protocol<Addr = A, Msg = LeaseOutcome<K>>>,
> {
    id: TimerId,
    state: LeaseState<K, Route>,
    marker: core::marker::PhantomData<fn() -> A>,
}
type LeaseActions<A, OutcomeSends> = Actions<A, Never, LeaseSends<OutcomeSends>, NoBirths>;
impl<A, K, Route> Lease<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute<Protocol: behavior::Protocol<Addr = A, Msg = LeaseOutcome<K>>> + Clone,
{
    /// Construct a vacant lease using one actor-local timer key.
    #[must_use]
    pub const fn new(id: TimerId) -> Self {
        Self {
            id,
            state: LeaseState::Vacant {
                next: TimerGeneration(0),
            },
            marker: core::marker::PhantomData,
        }
    }
    /// Borrow the complete ownership phase.
    #[must_use]
    pub const fn state(&self) -> &LeaseState<K, Route> {
        &self.state
    }
    fn result(reply_to: Route, outcome: LeaseOutcome<K>) -> LeaseActions<A, Route::Sends> {
        Actions::send(LeaseSends {
            outcomes: reply_to.deliver(outcome),
            schedules: InterpreterRequests::empty(),
        })
    }
    fn acquire(
        &mut self,
        holder: K,
        duration: Duration,
        reply_to: Route,
    ) -> LeaseActions<A, Route::Sends> {
        let next = match &self.state {
            LeaseState::Held {
                holder: current, ..
            } => {
                return Self::result(
                    reply_to,
                    LeaseOutcome::Rejected {
                        request: LeaseRequest::Acquire { holder, duration },
                        reason: LeaseRejection::Occupied {
                            current: current.clone(),
                        },
                    },
                );
            }
            LeaseState::Exhausted => {
                return Self::result(
                    reply_to,
                    LeaseOutcome::Rejected {
                        request: LeaseRequest::Acquire { holder, duration },
                        reason: LeaseRejection::GenerationExhausted,
                    },
                );
            }
            LeaseState::Vacant { next } => *next,
        };
        let successor = next.0.checked_add(1).map(TimerGeneration);
        self.state = LeaseState::Held {
            holder: holder.clone(),
            generation: next,
            notify: reply_to.clone(),
            next: successor,
        };
        Actions::send(LeaseSends {
            outcomes: reply_to.deliver(LeaseOutcome::Acquired {
                holder,
                generation: next,
            }),
            schedules: InterpreterRequests::one(ScheduleAfter::new(self.id, next, duration)),
        })
    }
    fn successor(
        &self,
        holder: &K,
        generation: TimerGeneration,
    ) -> Result<Option<TimerGeneration>, LeaseRejection<K>> {
        match &self.state {
            LeaseState::Vacant { .. } => Err(LeaseRejection::Vacant),
            LeaseState::Exhausted => Err(LeaseRejection::GenerationExhausted),
            LeaseState::Held {
                holder: current, ..
            } if current != holder => Err(LeaseRejection::WrongHolder {
                current: current.clone(),
            }),
            LeaseState::Held {
                generation: active, ..
            } if *active != generation => Err(LeaseRejection::StaleGeneration {
                observed: generation,
                current: *active,
            }),
            LeaseState::Held { next, .. } => Ok(*next),
        }
    }
}
impl<A, K, Route> BehaviorBase for Lease<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute<Protocol: behavior::Protocol<Addr = A, Msg = LeaseOutcome<K>>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, K, Route> behavior::Protocol for Lease<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute<Protocol: behavior::Protocol<Addr = A, Msg = LeaseOutcome<K>>>,
{
    type Addr = A;
    type Msg = LeaseMessage<K, Route>;
}

impl<A, K, Route> Behavior for Lease<A, K, Route>
where
    A: Address,
    K: Clone + Eq,
    Route: DeliveryRoute<Protocol: behavior::Protocol<Addr = A, Msg = LeaseOutcome<K>>> + Clone,
    Route::Sends: behavior::SendsFor<TimedEvent<User<A, LeaseMessage<K, Route>>>>,
{
    type Protocol = Self;
    type Event = TimedEvent<User<A, crate::BehaviorMessage<Self>>>;
    type Sends = LeaseSends<Route::Sends>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event {
            EventLayer::Inner(event) => match event.message {
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
                    let request = LeaseRequest::Renew {
                        holder: holder.clone(),
                        generation,
                        duration,
                    };
                    let successor = match self.successor(&holder, generation) {
                        Ok(successor) => successor,
                        Err(rejection) => {
                            return Ok(Self::result(
                                reply_to,
                                LeaseOutcome::Rejected {
                                    request,
                                    reason: rejection,
                                },
                            ));
                        }
                    };
                    let Some(fresh) = successor else {
                        return Ok(Self::result(
                            reply_to,
                            LeaseOutcome::Rejected {
                                request,
                                reason: LeaseRejection::GenerationExhausted,
                            },
                        ));
                    };
                    let next = fresh.0.checked_add(1).map(TimerGeneration);
                    self.state = LeaseState::Held {
                        holder: holder.clone(),
                        generation: fresh,
                        notify: reply_to.clone(),
                        next,
                    };
                    Actions::send(LeaseSends {
                        outcomes: reply_to.deliver(LeaseOutcome::Renewed {
                            holder,
                            generation: fresh,
                        }),
                        schedules: InterpreterRequests::one(ScheduleAfter::new(
                            self.id, fresh, duration,
                        )),
                    })
                }
                LeaseMessage::Release {
                    holder,
                    generation,
                    reply_to,
                } => {
                    let request = LeaseRequest::Release {
                        holder: holder.clone(),
                        generation,
                    };
                    let next = match self.successor(&holder, generation) {
                        Ok(next) => next,
                        Err(rejection) => {
                            return Ok(Self::result(
                                reply_to,
                                LeaseOutcome::Rejected {
                                    request,
                                    reason: rejection,
                                },
                            ));
                        }
                    };
                    self.state =
                        next.map_or(LeaseState::Exhausted, |next| LeaseState::Vacant { next });
                    Self::result(reply_to, LeaseOutcome::Released { holder, generation })
                }
            },
            EventLayer::Owned(elapsed) => {
                if elapsed.id != self.id {
                    return Ok(Actions::cont());
                }
                let LeaseState::Held {
                    holder,
                    generation,
                    notify,
                    next,
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
                let recipient = notify.clone();
                let next = *next;
                self.state = next.map_or(LeaseState::Exhausted, |next| LeaseState::Vacant { next });
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
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = LeaseOutcome<u8>;
    }

    impl Behavior for Reply {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = Lease<MailAddr, u8, Recipient<Reply>>;
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
            s.on_path(TimerElapsed::new(TimerId(7), TimerGeneration(0)))
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
        assert!(matches!(s.state(), LeaseState::Vacant { .. }));
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
            LeaseOutcome::Rejected {
                request: LeaseRequest::Release {
                    holder: 2,
                    generation: TimerGeneration(0),
                },
                reason: LeaseRejection::WrongHolder { current: 1 },
            }
        ));
        let expired = s
            .on_path(TimerElapsed::new(TimerId(7), TimerGeneration(0)))
            .unwrap();
        assert!(matches!(
            expired.sends.outcomes[0].message,
            LeaseOutcome::Expired { holder: 1, .. }
        ));
        assert!(matches!(s.state(), LeaseState::Vacant { .. }));
    }

    #[test]
    fn generation_exhaustion_is_terminal_and_never_wraps() {
        let mut definition = Subject::new(TimerId(7));
        definition.state = LeaseState::Vacant {
            next: TimerGeneration(u64::MAX),
        };
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
            .on_path(TimerElapsed::new(TimerId(7), TimerGeneration(u64::MAX)))
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
            LeaseOutcome::Rejected {
                request: LeaseRequest::Acquire {
                    holder: 2,
                    duration: requested_duration,
                },
                reason: LeaseRejection::GenerationExhausted,
            } if requested_duration == duration()
        ));
    }
}
