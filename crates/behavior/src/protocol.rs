//! Neutral typed vocabulary for interpreter-originated event and service lanes.
//!
//! Concrete behavior transformations define the closed sum types that add
//! these lanes. Keeping their values and construction capabilities here avoids
//! dependencies between otherwise independent transformations.

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::Address;
use crate::{Crash, CreationKind, Exit};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleAt {
    pub id: TimerId,
    pub generation: TimerGeneration,
    pub at: Instant,
}

impl ScheduleAt {
    #[must_use]
    pub const fn new(id: TimerId, generation: TimerGeneration, at: Instant) -> Self {
        Self { id, generation, at }
    }
}

impl From<(TimerId, TimerGeneration, Instant)> for ScheduleAt {
    fn from((id, generation, at): (TimerId, TimerGeneration, Instant)) -> Self {
        Self::new(id, generation, at)
    }
}

/// Request scheduling relative to the interpreter's clock.
///
/// Constructing this value does not observe a clock. The interpreter resolves
/// `after` only when it interprets the successful transition that emitted the
/// request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleAfter {
    pub id: TimerId,
    pub generation: TimerGeneration,
    pub after: Duration,
}

impl ScheduleAfter {
    #[must_use]
    pub const fn new(id: TimerId, generation: TimerGeneration, after: Duration) -> Self {
        Self {
            id,
            generation,
            after,
        }
    }
}

impl From<(TimerId, TimerGeneration, Duration)> for ScheduleAfter {
    fn from((id, generation, after): (TimerId, TimerGeneration, Duration)) -> Self {
        Self::new(id, generation, after)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerElapsed {
    pub id: TimerId,
    pub generation: TimerGeneration,
}

impl TimerElapsed {
    #[must_use]
    pub const fn new(id: TimerId, generation: TimerGeneration) -> Self {
        Self { id, generation }
    }
}

impl From<(TimerId, TimerGeneration)> for TimerElapsed {
    fn from((id, generation): (TimerId, TimerGeneration)) -> Self {
        Self::new(id, generation)
    }
}

pub trait TimeEvent: Sized {
    fn time_reached(event: TimerElapsed) -> Option<Self>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservePeer<A> {
    pub peer: A,
}

impl<A> From<A> for ObservePeer<A> {
    fn from(peer: A) -> Self {
        Self { peer }
    }
}

impl<A> ObservePeer<A> {
    #[must_use]
    pub const fn new(peer: A) -> Self {
        Self { peer }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerStopped<A: Address> {
    pub peer: A,
    pub outcome: Result<Exit<A>, Crash>,
}

impl<A: Address> PeerStopped<A> {
    #[must_use]
    pub fn new(peer: A, outcome: Result<Exit<A>, Crash>) -> Self {
        Self { peer, outcome }
    }
}

pub trait PeerEvent<A: Address>: Sized {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildStopped<A: Address> {
    pub nonce: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

impl<A: Address> ChildStopped<A> {
    #[must_use]
    pub fn new(nonce: A::Nonce, outcome: Result<Exit<A>, Crash>, at: Instant) -> Self {
        Self { nonce, outcome, at }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveChild<A: Address> {
    pub nonce: A::Nonce,
}

impl<A: Address> ObserveChild<A> {
    #[must_use]
    pub const fn new(nonce: A::Nonce) -> Self {
        Self { nonce }
    }
}

/// A proxy's request for its interpreter to report a worker termination to
/// the proxy's parent. The interpreter supplies the emitting proxy's child
/// nonce when constructing [`WorkerStopped`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportWorkerStopped<A: Address> {
    pub worker: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

impl<A: Address> ReportWorkerStopped<A> {
    #[must_use]
    pub fn new(worker: A::Nonce, outcome: Result<Exit<A>, Crash>, at: Instant) -> Self {
        Self {
            worker,
            outcome,
            at,
        }
    }
}

impl<A: Address> From<ChildStopped<A>> for ReportWorkerStopped<A> {
    fn from(stopped: ChildStopped<A>) -> Self {
        Self::new(stopped.nonce, stopped.outcome, stopped.at)
    }
}

/// A worker termination reported by a still-live supervised proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerStopped<A: Address> {
    pub proxy: A::Nonce,
    pub worker: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

impl<A: Address> WorkerStopped<A> {
    #[must_use]
    pub fn new(
        proxy: A::Nonce,
        worker: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
        at: Instant,
    ) -> Self {
        Self {
            proxy,
            worker,
            outcome,
            at,
        }
    }
}

impl<A: Address> From<(A::Nonce, ReportWorkerStopped<A>)> for WorkerStopped<A> {
    fn from((proxy, stopped): (A::Nonce, ReportWorkerStopped<A>)) -> Self {
        Self::new(proxy, stopped.worker, stopped.outcome, stopped.at)
    }
}

pub trait ChildEvent<A: Address>: Sized {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self>;
}

pub trait WorkerEvent<A: Address>: Sized {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self>;
}

/// Why a staged fresh creation was not committed by an interpreter.
///
/// This is a closed semantic classification; interpreter-specific error
/// values remain at the runtime boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationRejection {
    /// The creator-local nonce was already bound, so accepting the request
    /// would overwrite rather than establish a fresh child.
    NonceAlreadyBound,
    /// The fresh child's initialization did not complete successfully.
    InitializationFailed,
    /// The interpreter could not allocate, install, or commit the fresh child.
    EnvironmentFailed,
}

/// The committed result of one staged [`crate::Create`] request.
///
/// `Installed` is emitted only after fresh allocation, successful
/// initialization, and binding at `nonce`. The replacement provenance is the
/// provenance supplied by Behavior; an interpreter must never infer it from
/// address reuse or creation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreationResolved<A: Address> {
    pub nonce: A::Nonce,
    pub kind: CreationKind<A::Nonce>,
    pub result: Result<(), CreationRejection>,
}

impl<A: Address> CreationResolved<A> {
    #[must_use]
    pub const fn new(
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self {
            nonce,
            kind,
            result,
        }
    }

    #[must_use]
    pub const fn installed(nonce: A::Nonce, kind: CreationKind<A::Nonce>) -> Self {
        Self::new(nonce, kind, Ok(()))
    }

    #[must_use]
    pub const fn rejected(
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        rejection: CreationRejection,
    ) -> Self {
        Self::new(nonce, kind, Err(rejection))
    }
}

/// Ask the local interpreter to return the committed result of the same-action
/// creation at `nonce` through the behavior's [`CreationEvent`] lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveCreation<A: Address> {
    pub nonce: A::Nonce,
}

impl<A: Address> ObserveCreation<A> {
    #[must_use]
    pub const fn new(nonce: A::Nonce) -> Self {
        Self { nonce }
    }
}

pub trait CreationEvent<A: Address>: Sized {
    fn creation_resolved(event: CreationResolved<A>) -> Option<Self>;
}

/// Ask a proxy's interpreter to report a worker creation result to its parent.
/// The interpreter supplies the emitting proxy's nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportWorkerCreationResolved<A: Address> {
    pub worker: A::Nonce,
    pub kind: CreationKind<A::Nonce>,
    pub result: Result<(), CreationRejection>,
}

impl<A: Address> ReportWorkerCreationResolved<A> {
    #[must_use]
    pub const fn new(
        worker: A::Nonce,
        kind: CreationKind<A::Nonce>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self {
            worker,
            kind,
            result,
        }
    }
}

impl<A: Address> From<CreationResolved<A>> for ReportWorkerCreationResolved<A> {
    fn from(resolved: CreationResolved<A>) -> Self {
        Self::new(resolved.nonce, resolved.kind, resolved.result)
    }
}

/// A worker creation result reported by a still-live supervised proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerCreationResolved<A: Address> {
    pub proxy: A::Nonce,
    pub worker: A::Nonce,
    pub kind: CreationKind<A::Nonce>,
    pub result: Result<(), CreationRejection>,
}

impl<A: Address> WorkerCreationResolved<A> {
    #[must_use]
    pub const fn new(
        proxy: A::Nonce,
        worker: A::Nonce,
        kind: CreationKind<A::Nonce>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self {
            proxy,
            worker,
            kind,
            result,
        }
    }
}

impl<A: Address> From<(A::Nonce, ReportWorkerCreationResolved<A>)> for WorkerCreationResolved<A> {
    fn from((proxy, resolved): (A::Nonce, ReportWorkerCreationResolved<A>)) -> Self {
        Self::new(proxy, resolved.worker, resolved.kind, resolved.result)
    }
}

pub trait WorkerCreationEvent<A: Address>: Sized {
    fn worker_creation_resolved(event: WorkerCreationResolved<A>) -> Option<Self>;
}

/// A request to finish through one serialized behavior transition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ShutdownRequested;

pub trait ShutdownEvent: Sized {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MailAddr;

    #[test]
    fn lifecycle_conversions_preserve_every_semantic_field() {
        let at = Instant::now();
        let child = ChildStopped::<MailAddr>::new(3, Err(Crash::Failed), at);
        let report = ReportWorkerStopped::from(child);
        let worker = WorkerStopped::from((7, report));
        assert_eq!(worker.proxy, 7);
        assert_eq!(worker.worker, 3);
        assert_eq!(worker.outcome, Err(Crash::Failed));
        assert_eq!(worker.at, at);

        let creation = CreationResolved::<MailAddr>::rejected(
            4,
            CreationKind::replacement_of(3),
            CreationRejection::EnvironmentFailed,
        );
        let report = ReportWorkerCreationResolved::from(creation);
        let worker = WorkerCreationResolved::from((7, report));
        assert_eq!(worker.proxy, 7);
        assert_eq!(worker.worker, 4);
        assert_eq!(worker.kind, CreationKind::replacement_of(3));
        assert_eq!(worker.result, Err(CreationRejection::EnvironmentFailed));
    }

    #[test]
    fn timer_newtypes_and_requests_have_lossless_construction() {
        let id = TimerId::from(2);
        let generation = TimerGeneration::from(5);
        assert_eq!(u64::from(id), 2);
        assert_eq!(u64::from(generation), 5);
        assert_eq!(
            TimerElapsed::from((id, generation)),
            TimerElapsed::new(id, generation)
        );
    }
}
