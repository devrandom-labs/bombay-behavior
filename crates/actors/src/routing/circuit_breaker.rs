//! Single-flight circuit admission with explicit reset-timer evidence.

use std::{num::NonZeroU32, time::Duration};

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, EventLayer,
    InterpreterRequests, Never, NoBirths, Recipient, SendEffects, User,
};
use thiserror::Error;

use crate::{ScheduleAfter, TimedEvent, TimerGeneration, TimerId};

/// Identity of one admitted operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BreakerAttempt(pub u64);

/// Closed-state subphase; only one operation may own the breaker at a time.
pub enum ClosedPhase<Reply: behavior::Protocol> {
    /// The breaker may admit one operation.
    Idle { consecutive_failures: u32 },
    /// One admitted operation owns its terminal report capability.
    Awaiting {
        consecutive_failures: u32,
        attempt: BreakerAttempt,
        reply_to: Recipient<Reply>,
    },
}

/// Half-open probe subphase.
pub enum ProbePhase<Reply: behavior::Protocol> {
    /// Exactly one probe may be admitted.
    Available,
    /// The admitted probe owns its terminal report capability.
    Awaiting {
        attempt: BreakerAttempt,
        reply_to: Recipient<Reply>,
    },
}

/// Complete circuit-breaker phase sum.
pub enum BreakerPhase<Reply: behavior::Protocol> {
    /// Ordinary admission with a consecutive-failure count.
    Closed(ClosedPhase<Reply>),
    /// Admission is denied until matching timer evidence arrives.
    Open { generation: TimerGeneration },
    /// One recovery probe is available or in progress.
    Probing {
        generation: TimerGeneration,
        phase: ProbePhase<Reply>,
    },
    /// No fresh attempt or timer generation remains representable.
    Exhausted,
}

/// Typed construction failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BreakerConfigError {
    /// Reset delay must be non-zero.
    #[error("circuit-breaker reset delay must be non-zero")]
    ZeroResetDelay,
}

/// Why an admission request was declined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerRejection {
    /// A previously admitted operation has not reported a result.
    Busy,
    /// The breaker is open.
    Open { generation: TimerGeneration },
    /// Numeric freshness is exhausted.
    Exhausted,
}

/// Facts emitted to the recipient supplied with an admission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerOutcome {
    /// The caller may begin this uniquely identified operation while the
    /// circuit is closed.
    Admitted { attempt: BreakerAttempt },
    /// The caller's request is the single half-open recovery probe: a later
    /// [`BreakerOutcome::Succeeded`] closes the circuit, while a failed
    /// completion reopens it with a fresh timer generation.
    ProbeAdmitted { attempt: BreakerAttempt },
    /// A request was not admitted.
    Rejected(BreakerRejection),
    /// The admitted operation succeeded and closed the circuit.
    Succeeded { attempt: BreakerAttempt },
    /// Failure was recorded while the circuit remained closed.
    FailureRecorded {
        attempt: BreakerAttempt,
        consecutive_failures: u32,
    },
    /// Failure reached the threshold and opened the circuit.
    Opened {
        attempt: BreakerAttempt,
        generation: TimerGeneration,
    },
}

/// Closed user-command protocol.
pub enum BreakerMessage<Reply: behavior::Protocol> {
    /// Request one operation admission.
    Admit { reply_to: Recipient<Reply> },
    /// Report successful completion of an admitted operation.
    Succeeded { attempt: BreakerAttempt },
    /// Report failed completion of an admitted operation.
    Failed { attempt: BreakerAttempt },
}

/// Named output lanes for circuit facts and reset scheduling.
pub struct BreakerSends<Reply: behavior::Protocol> {
    /// Admission and completion facts.
    pub replies: Vec<Delivery<Reply>>,
    /// Relative reset requests interpreted by Bombay Timers.
    pub schedules: InterpreterRequests<ScheduleAfter>,
}

impl<Reply: behavior::Protocol> SendEffects for BreakerSends<Reply> {
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

impl<Event, Reply> behavior::SendsFor<Event> for BreakerSends<Reply>
where
    Reply: behavior::Protocol,
    InterpreterRequests<ScheduleAfter>: behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, Reply> behavior::InterpretSends<I, RootEvent, Path> for BreakerSends<Reply>
where
    I: behavior::SendInterpreter,
    Reply: behavior::Protocol,
    Vec<Delivery<Reply>>: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ScheduleAfter>: behavior::InterpretSends<I, RootEvent, Path>,
    BreakerSends<Reply>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.replies, interpreter).await?;
            behavior::InterpretSends::interpret(self.schedules, interpreter).await
        }
    }
}

type BreakerEvent<A, Reply> = TimedEvent<User<A, BreakerMessage<Reply>>>;
type BreakerActions<A, Reply> = Actions<A, Never, BreakerSends<Reply>, NoBirths>;

/// Pure single-flight closed/open/probing circuit-breaker fold.
///
/// Admission and completion are distinct typed events. Closed and probing
/// phases admit at most one operation, so a completion cannot be attributed by
/// timing or adjacency. Consecutive closed failures open the circuit at the
/// configured threshold; matching reset evidence makes exactly one probe
/// available. Probe success closes the breaker and probe failure reopens it
/// with a fresh generation. Stale attempts and timer evidence are inert. Empty
/// initialization, single-flight policy, failure counting, and reset ordering
/// are Bombay policy. Timer scheduling is interpreted by Bombay Timers; the
/// protected operation remains ordinary domain behavior. No transition panics.
pub struct CircuitBreaker<A: Address, Reply: behavior::Protocol<Addr = A, Msg = BreakerOutcome>> {
    threshold: NonZeroU32,
    reset_after: Duration,
    timer_id: TimerId,
    next_attempt: u64,
    phase: BreakerPhase<Reply>,
    marker: core::marker::PhantomData<fn() -> A>,
}

impl<A: Address, Reply: behavior::Protocol<Addr = A, Msg = BreakerOutcome>>
    CircuitBreaker<A, Reply>
{
    /// Construct a closed breaker.
    ///
    /// # Errors
    ///
    /// Returns [`BreakerConfigError::ZeroResetDelay`] when no positive reset
    /// interval was supplied.
    pub fn new(
        threshold: NonZeroU32,
        reset_after: Duration,
        timer_id: TimerId,
    ) -> Result<Self, BreakerConfigError> {
        if reset_after.is_zero() {
            return Err(BreakerConfigError::ZeroResetDelay);
        }
        Ok(Self {
            threshold,
            reset_after,
            timer_id,
            next_attempt: 0,
            phase: BreakerPhase::Closed(ClosedPhase::Idle {
                consecutive_failures: 0,
            }),
            marker: core::marker::PhantomData,
        })
    }

    /// Borrow the complete current phase.
    #[must_use]
    pub const fn phase(&self) -> &BreakerPhase<Reply> {
        &self.phase
    }

    fn reply(reply_to: Recipient<Reply>, outcome: BreakerOutcome) -> BreakerActions<A, Reply> {
        Actions::send(BreakerSends {
            replies: vec![Delivery::new(reply_to, outcome)],
            schedules: InterpreterRequests::empty(),
        })
    }

    fn admit(&mut self, reply_to: Recipient<Reply>) -> BreakerActions<A, Reply> {
        let attempt = BreakerAttempt(self.next_attempt);
        let next = self.next_attempt.checked_add(1);
        match &self.phase {
            BreakerPhase::Closed(ClosedPhase::Idle {
                consecutive_failures,
            }) if next.is_some() => {
                let failures = *consecutive_failures;
                self.next_attempt = next.expect("the is_some guard proves the counter advanced");
                self.phase = BreakerPhase::Closed(ClosedPhase::Awaiting {
                    consecutive_failures: failures,
                    attempt,
                    reply_to,
                });
                Self::reply(reply_to, BreakerOutcome::Admitted { attempt })
            }
            BreakerPhase::Probing {
                generation,
                phase: ProbePhase::Available,
            } if next.is_some() => {
                let generation = *generation;
                self.next_attempt = next.expect("the is_some guard proves the counter advanced");
                self.phase = BreakerPhase::Probing {
                    generation,
                    phase: ProbePhase::Awaiting { attempt, reply_to },
                };
                Self::reply(reply_to, BreakerOutcome::ProbeAdmitted { attempt })
            }
            BreakerPhase::Open { generation } => Self::reply(
                reply_to,
                BreakerOutcome::Rejected(BreakerRejection::Open {
                    generation: *generation,
                }),
            ),
            BreakerPhase::Exhausted => Self::reply(
                reply_to,
                BreakerOutcome::Rejected(BreakerRejection::Exhausted),
            ),
            _ if next.is_none() => {
                self.phase = BreakerPhase::Exhausted;
                Self::reply(
                    reply_to,
                    BreakerOutcome::Rejected(BreakerRejection::Exhausted),
                )
            }
            _ => Self::reply(reply_to, BreakerOutcome::Rejected(BreakerRejection::Busy)),
        }
    }

    fn complete(&mut self, attempt: BreakerAttempt, succeeded: bool) -> BreakerActions<A, Reply> {
        let ownership = match &self.phase {
            BreakerPhase::Closed(ClosedPhase::Awaiting {
                consecutive_failures,
                attempt: current,
                reply_to,
            }) if *current == attempt => Some((*reply_to, *consecutive_failures, None)),
            BreakerPhase::Probing {
                generation,
                phase:
                    ProbePhase::Awaiting {
                        attempt: current,
                        reply_to,
                    },
            } if *current == attempt => Some((*reply_to, 0, Some(*generation))),
            _ => None,
        };
        let Some((reply_to, failures, probe_generation)) = ownership else {
            return Actions::cont();
        };
        if succeeded {
            self.phase = BreakerPhase::Closed(ClosedPhase::Idle {
                consecutive_failures: 0,
            });
            return Self::reply(reply_to, BreakerOutcome::Succeeded { attempt });
        }
        let failures = failures.saturating_add(1);
        if probe_generation.is_none() && failures < self.threshold.get() {
            self.phase = BreakerPhase::Closed(ClosedPhase::Idle {
                consecutive_failures: failures,
            });
            return Self::reply(
                reply_to,
                BreakerOutcome::FailureRecorded {
                    attempt,
                    consecutive_failures: failures,
                },
            );
        }
        let next_generation = match probe_generation {
            Some(generation) => generation.0.checked_add(1).map(TimerGeneration),
            None => Some(TimerGeneration(0)),
        };
        let Some(generation) = next_generation else {
            self.phase = BreakerPhase::Exhausted;
            return Self::reply(
                reply_to,
                BreakerOutcome::Rejected(BreakerRejection::Exhausted),
            );
        };
        self.phase = BreakerPhase::Open { generation };
        Actions::send(BreakerSends {
            replies: vec![Delivery::new(
                reply_to,
                BreakerOutcome::Opened {
                    attempt,
                    generation,
                },
            )],
            schedules: InterpreterRequests::one(ScheduleAfter::new(
                self.timer_id,
                generation,
                self.reset_after,
            )),
        })
    }
}

impl<A: Address, Reply: behavior::Protocol<Addr = A, Msg = BreakerOutcome>> BehaviorBase
    for CircuitBreaker<A, Reply>
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A: Address, Reply: behavior::Protocol<Addr = A, Msg = BreakerOutcome>> behavior::Protocol
    for CircuitBreaker<A, Reply>
{
    type Addr = A;
    type Msg = BreakerMessage<Reply>;
}

impl<A: Address, Reply: behavior::Protocol<Addr = A, Msg = BreakerOutcome>> Behavior
    for CircuitBreaker<A, Reply>
{
    type Protocol = Self;
    type Event = BreakerEvent<A, Reply>;
    type Sends = BreakerSends<Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event {
            EventLayer::Inner(event) => match event.message {
                BreakerMessage::Admit { reply_to } => self.admit(reply_to),
                BreakerMessage::Succeeded { attempt } => self.complete(attempt, true),
                BreakerMessage::Failed { attempt } => self.complete(attempt, false),
            },
            EventLayer::Owned(elapsed) => {
                if let BreakerPhase::Open { generation } = self.phase
                    && elapsed.id == self.timer_id
                    && elapsed.generation == generation
                {
                    self.phase = BreakerPhase::Probing {
                        generation,
                        phase: ProbePhase::Available,
                    };
                }
                Actions::cont()
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
        type Msg = BreakerOutcome;
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

    fn subject() -> crate::Active<CircuitBreaker<MailAddr, Reply>> {
        (CircuitBreaker::new(
            NonZeroU32::new(2).unwrap(),
            Duration::from_secs(1),
            TimerId(8),
        )
        .unwrap())
        .initialize()
        .unwrap()
        .behavior
    }
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(9))
    }
    fn admit(subject: &mut crate::Active<CircuitBreaker<MailAddr, Reply>>) -> BreakerAttempt {
        let actions = subject
            .receive(MailAddr(0), BreakerMessage::Admit { reply_to: reply() })
            .unwrap();
        match actions.sends.replies[0].message {
            BreakerOutcome::Admitted { attempt } | BreakerOutcome::ProbeAdmitted { attempt } => {
                attempt
            }
            _ => unreachable!("test expected admission"),
        }
    }

    #[test]
    fn threshold_opens_matching_elapsed_allows_one_probe() {
        let mut subject = subject();
        let first = admit(&mut subject);
        subject
            .receive(MailAddr(0), BreakerMessage::Failed { attempt: first })
            .unwrap();
        let second = admit(&mut subject);
        let opened = subject
            .receive(MailAddr(0), BreakerMessage::Failed { attempt: second })
            .unwrap();
        assert_eq!(
            opened.sends.schedules.as_slice()[0].generation,
            TimerGeneration(0)
        );
        let denied = subject
            .receive(MailAddr(0), BreakerMessage::Admit { reply_to: reply() })
            .unwrap();
        assert!(matches!(
            denied.sends.replies[0].message,
            BreakerOutcome::Rejected(BreakerRejection::Open { .. })
        ));
        subject
            .on_path(TimerElapsed::new(TimerId(8), TimerGeneration(0)))
            .unwrap();
        let probe = admit(&mut subject);
        assert_eq!(probe, BreakerAttempt(2));
        let busy = subject
            .receive(MailAddr(0), BreakerMessage::Admit { reply_to: reply() })
            .unwrap();
        assert!(matches!(
            busy.sends.replies[0].message,
            BreakerOutcome::Rejected(BreakerRejection::Busy)
        ));
    }

    #[test]
    fn stale_attempt_and_timer_evidence_are_inert() {
        let mut subject = subject();
        let attempt = admit(&mut subject);
        assert!(
            subject
                .receive(
                    MailAddr(0),
                    BreakerMessage::Succeeded {
                        attempt: BreakerAttempt(99)
                    }
                )
                .unwrap()
                .sends
                .replies
                .is_empty()
        );
        assert!(
            subject
                .on_path(TimerElapsed::new(TimerId(8), TimerGeneration(7)))
                .unwrap()
                .sends
                .replies
                .is_empty()
        );
        let success = subject
            .receive(MailAddr(0), BreakerMessage::Succeeded { attempt })
            .unwrap();
        assert!(matches!(
            success.sends.replies[0].message,
            BreakerOutcome::Succeeded { .. }
        ));
    }

    fn breaker() -> CircuitBreaker<MailAddr, Reply> {
        CircuitBreaker::new(
            NonZeroU32::new(2).unwrap(),
            Duration::from_secs(1),
            TimerId(8),
        )
        .unwrap()
    }

    #[test]
    fn attempt_counter_exhaustion_is_typed_not_wrapped() {
        let mut breaker = breaker();
        breaker.next_attempt = u64::MAX;
        let mut subject = (breaker).initialize().unwrap().behavior;
        let actions = subject
            .receive(MailAddr(0), BreakerMessage::Admit { reply_to: reply() })
            .unwrap();
        assert!(matches!(*subject.phase(), BreakerPhase::Exhausted));
        assert!(matches!(
            actions.sends.replies[0].message,
            BreakerOutcome::Rejected(BreakerRejection::Exhausted)
        ));
    }

    #[test]
    fn timer_generation_exhaustion_is_typed_not_wrapped() {
        let mut breaker = breaker();
        breaker.phase = BreakerPhase::Probing {
            generation: TimerGeneration(u64::MAX),
            phase: ProbePhase::Awaiting {
                attempt: BreakerAttempt(0),
                reply_to: reply(),
            },
        };
        let mut subject = (breaker).initialize().unwrap().behavior;
        let actions = subject
            .receive(
                MailAddr(0),
                BreakerMessage::Failed {
                    attempt: BreakerAttempt(0),
                },
            )
            .unwrap();
        assert!(matches!(*subject.phase(), BreakerPhase::Exhausted));
        assert!(matches!(
            actions.sends.replies[0].message,
            BreakerOutcome::Rejected(BreakerRejection::Exhausted)
        ));
    }
}
