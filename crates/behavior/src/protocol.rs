//! Neutral typed vocabulary for interpreter-originated event and service lanes.
//!
//! Concrete behavior transformations define the closed sum types that add
//! these lanes. Keeping their values and construction capabilities here avoids
//! dependencies between otherwise independent transformations.

use tokio::time::Instant;

use crate::behavior::Address;
use crate::{Crash, Exit};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtGeneration(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleAt {
    pub id: AtId,
    pub generation: AtGeneration,
    pub at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeReached {
    pub id: AtId,
    pub generation: AtGeneration,
    pub at: Instant,
}

pub trait TimeEvent: Sized {
    fn time_reached(event: TimeReached) -> Option<Self>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservePeer<A> {
    pub peer: A,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerStopped<A: Address> {
    pub peer: A,
    pub outcome: Result<Exit<A>, Crash>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveChild<A: Address> {
    pub nonce: A::Nonce,
}

/// A proxy's request for its interpreter to report a worker termination to
/// the proxy's parent. The interpreter supplies the emitting proxy's child
/// nonce when constructing [`WorkerStopped`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportWorkerStopped<A: Address> {
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

/// A worker termination reported by a still-live supervised proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerStopped<A: Address> {
    pub proxy: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

pub trait ChildEvent<A: Address>: Sized {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self>;
}

pub trait WorkerEvent<A: Address>: Sized {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self>;
}

/// A request to finish through one serialized behavior transition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ShutdownRequested;

pub trait ShutdownEvent: Sized {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self>;
}
