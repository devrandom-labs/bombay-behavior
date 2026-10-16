//! Typed scheduling products shared by named timers and individual timer owners.
//!
//! Named targets share the actor-local replacement namespace. Here targets the
//! emitting wrapper's exact ingress; outer composition carries that destination.
//! Both retain the same original request, receipt, rejection and generation law.
//! Clock observation and queue operations belong exclusively to the interpreter.

use behavior::{Here, OwnedItem, SourceAction};
use std::time::{Duration, Instant};

mod targets {
    pub trait Sealed {}
    impl Sealed for super::TimerId {}
    impl Sealed for behavior::Here {}
}

/// The closed choice of a domain timer name or the current timer owner's ingress.
///
/// Ordinary timer constructors supply their ingress; callers provide no marker.
/// The two target types retain distinct concrete request and event protocols.
///
/// Applications cannot add another targeting or replacement rule.
///
/// ```compile_fail,E0277
/// #[derive(Clone, Copy)]
/// struct Target;
/// impl behavior_actors::timer::TimerTarget for Target {}
/// ```
pub trait TimerTarget: targets::Sealed + Copy + Send {}
impl TimerTarget for TimerId {}
impl TimerTarget for Here {}

/// Exact timer correlation accepted by the local scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerScheduled<Target: TimerTarget> {
    /// The exact named timer or current-layer ingress capability.
    pub id: Target,
    /// Behavior-owned generation returned without reinterpretation.
    pub generation: TimerGeneration,
}

/// Exact reason an absolute timer request was rejected before scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleAtRejection {
    /// The timer queue cannot issue another internal generation.
    #[error("timer queue generation exhausted")]
    QueueGenerationExhausted,
    /// The timer queue cannot issue another stable insertion sequence.
    #[error("timer queue insertion sequence exhausted")]
    QueueSequenceExhausted,
}

/// Exact reason a relative timer request was rejected before scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleAfterRejection {
    /// Adding the delay to the interpreter's current instant overflowed.
    #[error("timer deadline overflowed")]
    DeadlineOverflow,
    /// The timer queue cannot issue another internal generation.
    #[error("timer queue generation exhausted")]
    QueueGenerationExhausted,
    /// The timer queue cannot issue another stable insertion sequence.
    #[error("timer queue insertion sequence exhausted")]
    QueueSequenceExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerGeneration(pub u64);

impl From<u64> for TimerId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<TimerId> for u64 {
    fn from(value: TimerId) -> Self {
        value.0
    }
}

impl From<u64> for TimerGeneration {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<TimerGeneration> for u64 {
    fn from(value: TimerGeneration) -> Self {
        value.0
    }
}

/// Request an absolute deadline from the emitting actor's interpreter.
///
/// The target selects either shared named replacement or this wrapper's ingress.
/// Rejection returns this complete request and its exact scheduling reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleAt<Target: TimerTarget> {
    pub id: Target,
    pub generation: TimerGeneration,
    pub at: Instant,
}

impl ScheduleAt<TimerId> {
    /// Request an absolute deadline using an actor-local timer name.
    #[must_use]
    pub const fn new(id: TimerId, generation: TimerGeneration, at: Instant) -> Self {
        Self { id, generation, at }
    }
}

impl ScheduleAt<Here> {
    /// Select this timer owner's exact event occurrence without a caller name.
    #[must_use]
    pub const fn new(generation: TimerGeneration, at: Instant) -> Self {
        Self {
            id: Here,
            generation,
            at,
        }
    }
}

impl<Target: TimerTarget> From<(Target, TimerGeneration, Instant)> for ScheduleAt<Target> {
    fn from((id, generation, at): (Target, TimerGeneration, Instant)) -> Self {
        Self { id, generation, at }
    }
}

impl<Target: TimerTarget> behavior::InterpreterRequest for ScheduleAt<Target> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<TimerElapsed<Target>, Here>;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<Target: TimerTarget> OwnedItem for ScheduleAt<Target> {
    type Accepted = TimerScheduled<Target>;
    type Rejection = ScheduleAtRejection;
    type Prerequisite = behavior::Never;
}

impl<Target: TimerTarget> SourceAction for ScheduleAt<Target> {
    type Source = Self;
}

/// Request scheduling relative to the interpreter's clock.
///
/// Constructing this value does not observe a clock. The interpreter resolves
/// `after` only when it interprets the successful transition that emitted the
/// request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleAfter<Target: TimerTarget> {
    pub id: Target,
    pub generation: TimerGeneration,
    pub after: Duration,
}

impl ScheduleAfter<TimerId> {
    /// Request a delay using an actor-local timer name.
    #[must_use]
    pub const fn new(id: TimerId, generation: TimerGeneration, after: Duration) -> Self {
        Self {
            id,
            generation,
            after,
        }
    }
}

impl ScheduleAfter<Here> {
    /// Select this timer owner's exact event occurrence without a caller name.
    #[must_use]
    pub const fn new(generation: TimerGeneration, after: Duration) -> Self {
        Self {
            id: Here,
            generation,
            after,
        }
    }
}

impl<Target: TimerTarget> From<(Target, TimerGeneration, Duration)> for ScheduleAfter<Target> {
    fn from((id, generation, after): (Target, TimerGeneration, Duration)) -> Self {
        Self {
            id,
            generation,
            after,
        }
    }
}

impl<Target: TimerTarget> behavior::InterpreterRequest for ScheduleAfter<Target> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<TimerElapsed<Target>, Here>;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<Target: TimerTarget> OwnedItem for ScheduleAfter<Target> {
    type Accepted = TimerScheduled<Target>;
    type Rejection = ScheduleAfterRejection;
    type Prerequisite = behavior::Never;
}

impl<Target: TimerTarget> SourceAction for ScheduleAfter<Target> {
    type Source = Self;
}

/// An elapsed generation returned to the request's exact typed ingress.
///
/// This notification is distinct from scheduling acceptance. Timer wrappers
/// accept their current generation once and ignore stale or duplicate delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerElapsed<Target: TimerTarget> {
    pub id: Target,
    pub generation: TimerGeneration,
}

impl TimerElapsed<TimerId> {
    /// Report an elapsed generation of the named timer.
    #[must_use]
    pub const fn new(id: TimerId, generation: TimerGeneration) -> Self {
        Self { id, generation }
    }
}

impl TimerElapsed<Here> {
    /// Select this timer owner's exact event occurrence without a caller name.
    #[must_use]
    pub const fn new(generation: TimerGeneration) -> Self {
        Self {
            id: Here,
            generation,
        }
    }
}

impl<Target: TimerTarget> From<(Target, TimerGeneration)> for TimerElapsed<Target> {
    fn from((id, generation): (Target, TimerGeneration)) -> Self {
        Self { id, generation }
    }
}
