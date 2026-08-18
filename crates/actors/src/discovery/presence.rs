//! Versioned presence evidence with generation-safe expiry.

use std::time::Duration;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, EventLayer,
    InterpreterRequests, Never, NoBirths, Recipient, SendEffects, User,
};
use thiserror::Error;

use crate::{ScheduleAfter, TimedEvent, TimerGeneration, TimerId};

/// Version within one participant's presence-evidence stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PresenceVersion(pub u64);

/// Complete retained phase for one known participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresencePhase {
    /// Participant is present until matching timer evidence arrives.
    Present {
        /// Latest evidence version.
        version: PresenceVersion,
        /// Current expiry generation.
        generation: TimerGeneration,
        /// Declared relative lifetime.
        lifetime: Duration,
    },
    /// Matching expiry was committed and retained as a tombstone.
    Expired {
        /// Evidence version that established the expired incarnation.
        version: PresenceVersion,
        /// Expired timer generation.
        generation: TimerGeneration,
    },
}

/// One known participant and its timer namespace data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceEntry<K> {
    /// Application-defined participant identity.
    pub participant: K,
    /// Actor-local timer key chosen by the supplied pure mapping.
    pub timer_id: TimerId,
    /// Complete presence phase.
    pub phase: PresencePhase,
}

/// Typed rejection of presence evidence.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PresenceError<K> {
    /// Evidence predates the retained participant version.
    #[error("presence evidence is stale")]
    Stale {
        /// Participant.
        participant: K,
        /// Rejected version.
        observed: PresenceVersion,
        /// Current version.
        current: PresenceVersion,
    },
    /// Evidence reuses a version with a different lifetime or phase.
    #[error("presence evidence conflicts at the committed version")]
    ConflictingVersion {
        /// Participant.
        participant: K,
        /// Reused version.
        version: PresenceVersion,
    },
    /// A currently present different participant owns the mapped timer key.
    #[error("presence timer key collides with a live participant")]
    TimerCollision {
        /// Rejected participant.
        participant: K,
        /// Existing participant owning the timer key.
        existing: K,
        /// Colliding timer key.
        timer_id: TimerId,
    },
    /// No fresh timer generation is representable.
    #[error("presence timer generation is exhausted")]
    GenerationExhausted {
        /// Participant whose refresh was rejected.
        participant: K,
    },
}

/// Factual presence transition result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceOutcome<K> {
    /// A previously unknown participant became present.
    Announced {
        /// Participant.
        participant: K,
        /// Committed evidence version.
        version: PresenceVersion,
        /// Committed timer generation.
        generation: TimerGeneration,
    },
    /// A known participant committed newer evidence.
    Refreshed {
        /// Participant.
        participant: K,
        /// Committed evidence version.
        version: PresenceVersion,
        /// Fresh timer generation.
        generation: TimerGeneration,
    },
    /// Identical evidence was accepted idempotently without rescheduling.
    Unchanged {
        /// Participant.
        participant: K,
        /// Existing evidence version.
        version: PresenceVersion,
        /// Existing timer generation.
        generation: TimerGeneration,
    },
    /// Matching timer evidence committed expiry.
    Expired {
        /// Participant.
        participant: K,
        /// Evidence version of the expired incarnation.
        version: PresenceVersion,
        /// Expired generation.
        generation: TimerGeneration,
    },
    /// Evidence was rejected without mutation.
    Rejected(PresenceError<K>),
}

/// Point-in-time retained presence report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceReport<K> {
    /// Participants in first-observation order, including tombstones.
    pub entries: Vec<PresenceEntry<K>>,
}

/// Closed reply protocol emitted by [`Presence`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresenceReply<K> {
    /// One transition result.
    Outcome(PresenceOutcome<K>),
    /// One query result.
    Report(PresenceReport<K>),
}

/// User commands accepted by [`Presence`].
pub enum PresenceMessage<K, Reply: behavior::Protocol> {
    /// Announce versioned presence for a relative lifetime.
    Announce {
        /// Participant.
        participant: K,
        /// Evidence version.
        version: PresenceVersion,
        /// Relative lifetime.
        lifetime: Duration,
        /// Outcome and later-expiry recipient.
        reply_to: Recipient<Reply>,
    },
    /// Return every retained present or expired state.
    Query {
        /// Typed report recipient.
        reply_to: Recipient<Reply>,
    },
}

/// Named effect lanes emitted by [`Presence`].
pub struct PresenceSends<Reply: behavior::Protocol> {
    /// Transition and query facts.
    pub replies: Vec<Delivery<Reply>>,
    /// Relative expiry requests.
    pub schedules: InterpreterRequests<ScheduleAfter>,
}
impl<Reply> SendEffects for PresenceSends<Reply>
where
    Reply: behavior::Protocol,
{
    fn empty() -> Self {
        Self {
            replies: Vec::new(),
            schedules: InterpreterRequests::empty(),
        }
    }
    fn append(&mut self, mut other: Self) {
        self.replies.append(&mut other.replies);
        self.schedules.append(other.schedules);
    }
}

impl<Event, Reply> behavior::SendsFor<Event> for PresenceSends<Reply>
where
    Reply: behavior::Protocol,
    InterpreterRequests<ScheduleAfter>: behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, Reply> behavior::InterpretSends<I, RootEvent, Path>
    for PresenceSends<Reply>
where
    I: behavior::SendInterpreter,
    Reply: behavior::Protocol,
    Vec<Delivery<Reply>>: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ScheduleAfter>: behavior::InterpretSends<I, RootEvent, Path>,
{
    fn interpret(self, interpreter: &mut I) -> Result<(), I::Error> {
        behavior::InterpretSends::interpret(self.replies, interpreter)?;
        behavior::InterpretSends::interpret(self.schedules, interpreter)
    }
}

struct Record<K, Reply: behavior::Protocol> {
    entry: PresenceEntry<K>,
    notify: Recipient<Reply>,
}

/// Versioned, generation-safe membership-presence behavior.
///
/// The first announcement commits generation zero before requesting expiry.
/// Greater evidence versions refresh with a checked fresh generation; identical
/// evidence is idempotent and does not reschedule. Lower or contradictory
/// evidence is rejected atomically. A timer-key collision with another live
/// participant is rejection, never replacement. Matching elapsed evidence
/// commits an explicit tombstone and reports expiry; stale and unknown timer
/// evidence is inert. Initialization is empty, no actors are created, and the
/// host never terminates by policy. Versioning, first-observation ordering,
/// timer mapping, and tombstone retention are Bombay policy. Scheduling belongs
/// to Timers; release/cancellation is not part of this announcement protocol.
/// No transition has a semantic panic condition.
pub struct Presence<
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = PresenceReply<K>>,
> {
    timer_id: fn(&K) -> TimerId,
    records: Vec<Record<K, Reply>>,
    marker: core::marker::PhantomData<fn() -> A>,
}
type PresenceActions<A, Reply> = Actions<A, Never, PresenceSends<Reply>, NoBirths>;
impl<A, K, Reply> Presence<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = PresenceReply<K>>,
{
    /// Construct an empty presence table with a pure actor-local timer mapping.
    #[must_use]
    pub const fn new(timer_id: fn(&K) -> TimerId) -> Self {
        Self {
            timer_id,
            records: Vec::new(),
            marker: core::marker::PhantomData,
        }
    }
    /// Snapshot complete present and tombstoned state.
    #[must_use]
    pub fn report(&self) -> PresenceReport<K> {
        PresenceReport {
            entries: self
                .records
                .iter()
                .map(|record| record.entry.clone())
                .collect(),
        }
    }
    fn reply(reply_to: Recipient<Reply>, reply: PresenceReply<K>) -> PresenceActions<A, Reply> {
        Actions::send(PresenceSends {
            replies: vec![Delivery::new(reply_to, reply)],
            schedules: InterpreterRequests::empty(),
        })
    }
    fn announce(
        &mut self,
        participant: K,
        version: PresenceVersion,
        lifetime: Duration,
        reply_to: Recipient<Reply>,
    ) -> PresenceActions<A, Reply> {
        let timer_id = (self.timer_id)(&participant);
        if let Some(existing) = self.records.iter().find(|record| {
            record.entry.participant != participant
                && record.entry.timer_id == timer_id
                && matches!(record.entry.phase, PresencePhase::Present { .. })
        }) {
            return Self::reply(
                reply_to,
                PresenceReply::Outcome(PresenceOutcome::Rejected(PresenceError::TimerCollision {
                    participant,
                    existing: existing.entry.participant.clone(),
                    timer_id,
                })),
            );
        }
        let existing = self
            .records
            .iter()
            .position(|record| record.entry.participant == participant);
        match existing {
            Some(index) => self.refresh(index, participant, version, lifetime, reply_to, timer_id),
            None => self.introduce(participant, version, lifetime, reply_to, timer_id),
        }
    }

    fn introduce(
        &mut self,
        participant: K,
        version: PresenceVersion,
        lifetime: Duration,
        reply_to: Recipient<Reply>,
        timer_id: TimerId,
    ) -> PresenceActions<A, Reply> {
        let generation = TimerGeneration(0);
        self.records.push(Record {
            entry: PresenceEntry {
                participant: participant.clone(),
                timer_id,
                phase: PresencePhase::Present {
                    version,
                    generation,
                    lifetime,
                },
            },
            notify: reply_to,
        });
        Actions::send(PresenceSends {
            replies: vec![Delivery::new(
                reply_to,
                PresenceReply::Outcome(PresenceOutcome::Announced {
                    participant,
                    version,
                    generation,
                }),
            )],
            schedules: InterpreterRequests::one(ScheduleAfter::new(timer_id, generation, lifetime)),
        })
    }

    fn refresh(
        &mut self,
        index: usize,
        participant: K,
        version: PresenceVersion,
        lifetime: Duration,
        reply_to: Recipient<Reply>,
        timer_id: TimerId,
    ) -> PresenceActions<A, Reply> {
        let current_version = match self.records[index].entry.phase {
            PresencePhase::Present { version, .. } | PresencePhase::Expired { version, .. } => {
                version
            }
        };
        if version < current_version {
            return Self::reply(
                reply_to,
                PresenceReply::Outcome(PresenceOutcome::Rejected(PresenceError::Stale {
                    participant,
                    observed: version,
                    current: current_version,
                })),
            );
        }
        if version == current_version {
            if let PresencePhase::Present {
                generation,
                lifetime: current,
                ..
            } = self.records[index].entry.phase
                && current == lifetime
            {
                return Self::reply(
                    reply_to,
                    PresenceReply::Outcome(PresenceOutcome::Unchanged {
                        participant,
                        version,
                        generation,
                    }),
                );
            }
            return Self::reply(
                reply_to,
                PresenceReply::Outcome(PresenceOutcome::Rejected(
                    PresenceError::ConflictingVersion {
                        participant,
                        version,
                    },
                )),
            );
        }
        let generation = match self.records[index].entry.phase {
            PresencePhase::Present { generation, .. }
            | PresencePhase::Expired { generation, .. } => match generation.0.checked_add(1) {
                Some(next) => TimerGeneration(next),
                None => {
                    return Self::reply(
                        reply_to,
                        PresenceReply::Outcome(PresenceOutcome::Rejected(
                            PresenceError::GenerationExhausted { participant },
                        )),
                    );
                }
            },
        };
        self.records[index].entry.phase = PresencePhase::Present {
            version,
            generation,
            lifetime,
        };
        self.records[index].notify = reply_to;
        Actions::send(PresenceSends {
            replies: vec![Delivery::new(
                reply_to,
                PresenceReply::Outcome(PresenceOutcome::Refreshed {
                    participant,
                    version,
                    generation,
                }),
            )],
            schedules: InterpreterRequests::one(ScheduleAfter::new(timer_id, generation, lifetime)),
        })
    }
}
impl<A, K, Reply> BehaviorBase for Presence<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = PresenceReply<K>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, K, Reply> behavior::Protocol for Presence<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = PresenceReply<K>>,
{
    type Addr = A;
    type Msg = PresenceMessage<K, Reply>;
}

impl<A, K, Reply> Behavior for Presence<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = PresenceReply<K>>,
{
    type Protocol = Self;
    type Event = TimedEvent<User<A, crate::BehaviorMessage<Self>>>;
    type Sends = PresenceSends<Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event {
            EventLayer::Inner(event) => match event.message {
                PresenceMessage::Announce {
                    participant,
                    version,
                    lifetime,
                    reply_to,
                } => self.announce(participant, version, lifetime, reply_to),
                PresenceMessage::Query { reply_to } => {
                    Self::reply(reply_to, PresenceReply::Report(self.report()))
                }
            },
            EventLayer::Owned(elapsed) => {
                let Some(index)=self.records.iter().position(|record|record.entry.timer_id==elapsed.id&&matches!(record.entry.phase,PresencePhase::Present{generation,..}if generation==elapsed.generation))else{return Ok(Actions::cont());};
                let (version, generation) = match self.records[index].entry.phase {
                    PresencePhase::Present {
                        version,
                        generation,
                        ..
                    } => (version, generation),
                    PresencePhase::Expired { .. } => return Ok(Actions::cont()),
                };
                let participant = self.records[index].entry.participant.clone();
                let notify = self.records[index].notify;
                self.records[index].entry.phase = PresencePhase::Expired {
                    version,
                    generation,
                };
                Self::reply(
                    notify,
                    PresenceReply::Outcome(PresenceOutcome::Expired {
                        participant,
                        version,
                        generation,
                    }),
                )
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Activate as _, TimerElapsed};
    use behavior::MailAddr;
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Participant(u8);
    struct Reply;
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = PresenceReply<Participant>;
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
    type Subject = Presence<MailAddr, Participant, Reply>;
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(9))
    }
    fn duration() -> Duration {
        Duration::from_secs(1)
    }
    fn timer(key: &Participant) -> TimerId {
        TimerId(u64::from(key.0))
    }
    #[test]
    fn refresh_and_expiry_are_version_and_generation_safe() {
        let mut s = (Subject::new(timer)).initialize().unwrap().behavior;
        s.receive(
            MailAddr(0),
            PresenceMessage::Announce {
                participant: Participant(1),
                version: PresenceVersion(1),
                lifetime: duration(),
                reply_to: reply(),
            },
        )
        .unwrap();
        let refreshed = s
            .receive(
                MailAddr(0),
                PresenceMessage::Announce {
                    participant: Participant(1),
                    version: PresenceVersion(2),
                    lifetime: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(
            refreshed.sends.schedules.as_slice()[0].generation,
            TimerGeneration(1)
        );
        assert!(
            s.on_path(TimerElapsed::new(TimerId(1), TimerGeneration(0)))
                .unwrap()
                .sends
                .replies
                .is_empty()
        );
        let expired = s
            .on_path(TimerElapsed::new(TimerId(1), TimerGeneration(1)))
            .unwrap();
        assert!(matches!(
            expired.sends.replies[0].message,
            PresenceReply::Outcome(PresenceOutcome::Expired {
                participant: Participant(1),
                ..
            })
        ));
    }
    #[test]
    fn collision_and_stale_evidence_are_atomic() {
        fn collision(_: &Participant) -> TimerId {
            TimerId(1)
        }
        let mut s = (Subject::new(collision)).initialize().unwrap().behavior;
        s.receive(
            MailAddr(0),
            PresenceMessage::Announce {
                participant: Participant(1),
                version: PresenceVersion(2),
                lifetime: duration(),
                reply_to: reply(),
            },
        )
        .unwrap();
        let collision = s
            .receive(
                MailAddr(0),
                PresenceMessage::Announce {
                    participant: Participant(2),
                    version: PresenceVersion(1),
                    lifetime: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            collision.sends.replies[0].message,
            PresenceReply::Outcome(PresenceOutcome::Rejected(PresenceError::TimerCollision {
                participant: Participant(2),
                existing: Participant(1),
                ..
            }))
        ));
        let stale = s
            .receive(
                MailAddr(0),
                PresenceMessage::Announce {
                    participant: Participant(1),
                    version: PresenceVersion(1),
                    lifetime: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            stale.sends.replies[0].message,
            PresenceReply::Outcome(PresenceOutcome::Rejected(PresenceError::Stale { .. }))
        ));
        assert_eq!(s.report().entries.len(), 1);
    }

    #[test]
    fn identical_evidence_is_idempotent_without_rescheduling() {
        let mut s = (Subject::new(timer)).initialize().unwrap().behavior;
        let message = || PresenceMessage::Announce {
            participant: Participant(1),
            version: PresenceVersion(1),
            lifetime: duration(),
            reply_to: reply(),
        };
        s.receive(MailAddr(0), message()).unwrap();
        let unchanged = s.receive(MailAddr(0), message()).unwrap();
        assert!(unchanged.sends.schedules.as_slice().is_empty());
        assert!(matches!(
            unchanged.sends.replies[0].message,
            PresenceReply::Outcome(PresenceOutcome::Unchanged {
                generation: TimerGeneration(0),
                ..
            })
        ));
    }

    #[test]
    fn exhausted_generation_rejects_without_mutation() {
        let mut definition = Subject::new(timer);
        definition.records.push(Record {
            entry: PresenceEntry {
                participant: Participant(1),
                timer_id: TimerId(1),
                phase: PresencePhase::Present {
                    version: PresenceVersion(1),
                    generation: TimerGeneration(u64::MAX),
                    lifetime: duration(),
                },
            },
            notify: reply(),
        });
        let mut s = (definition).initialize().unwrap().behavior;
        let rejected = s
            .receive(
                MailAddr(0),
                PresenceMessage::Announce {
                    participant: Participant(1),
                    version: PresenceVersion(2),
                    lifetime: duration(),
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            rejected.sends.replies[0].message,
            PresenceReply::Outcome(PresenceOutcome::Rejected(
                PresenceError::GenerationExhausted { .. }
            ))
        ));
        assert!(matches!(
            s.report().entries[0].phase,
            PresencePhase::Present {
                version: PresenceVersion(1),
                generation: TimerGeneration(u64::MAX),
                ..
            }
        ));
    }
}
