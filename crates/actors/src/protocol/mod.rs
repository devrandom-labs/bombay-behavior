//! Neutral typed vocabulary for interpreter-originated event and service lanes.
//!
//! Concrete behavior transformations define the closed sum types that add
//! these lanes. Keeping their values and construction capabilities here avoids
//! dependencies between otherwise independent transformations.

mod established;
mod pool;

pub use established::{
    CancelObservation, EstablishedObservation, EstablishedShutdownResolved,
    InterpretEstablishedObservation, InterpretEstablishedShutdown, ObservationId,
    ObservationOperation, ObservationRejection, ObserveEstablished, ObserveEstablishedCreation,
    ShutdownEstablished, ShutdownId, ShutdownRejection,
};

pub use pool::{KeyedWorkerPoolProtocol, PoolAssignmentProtocol, WorkerPoolProtocol};

use std::time::Duration;

use std::time::Instant;

use crate::{Crash, CreationKind, Exit};
use behavior::Address;
pub use behavior::CreationRejection;

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

impl behavior::InterpreterRequest for ScheduleAt {
    type ReturnToEmitter = behavior::ReturnsToEmitter<TimerElapsed, behavior::Here>;
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

impl behavior::InterpreterRequest for ScheduleAfter {
    type ReturnToEmitter = behavior::ReturnsToEmitter<TimerElapsed, behavior::Here>;
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

/// Ask the local interpreter to observe the exact peer incarnation selected at
/// `peer` when this request is interpreted.
///
/// [`PeerStopped`] is the pure result protocol. It arrives eventually if a
/// selected live incarnation later terminates, or may arrive immediately when
/// the interpreter has authoritative retained termination for the requested
/// incarnation. Absence from a live-address table is not such authority: an
/// interpreter that can select neither a live incarnation nor retained
/// terminal history must return an interpreter error rather than fabricate a
/// stop result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservePeer<A: Address> {
    pub peer: A,
}

impl<A: Address> From<A> for ObservePeer<A> {
    fn from(peer: A) -> Self {
        Self::new(peer)
    }
}

impl<A: Address> ObservePeer<A> {
    #[must_use]
    pub const fn new(peer: A) -> Self {
        Self { peer }
    }
}

impl<A: Address> behavior::InterpreterRequest for ObservePeer<A> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<PeerStopped<A>, behavior::Here>;
}

/// Ask the local interpreter to cancel this actor's observation of `peer`.
///
/// Peer observation is a derived Bombay protocol, not an actor-model
/// primitive. The address selects every observer-local definition for that
/// peer created by [`ObservePeer`]. Exact-incarnation capture and cancellation
/// belong to the interpreter. Cancellation does not retract a [`PeerStopped`]
/// event already admitted to the actor's mailbox, and an interpreter treats a
/// request for relationships that are no longer present as inert. Distinct
/// structural observations remain independent until this explicit
/// address-wide cancellation policy is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnwatchPeer<A> {
    pub peer: A,
}

impl<A> UnwatchPeer<A> {
    #[must_use]
    pub const fn new(peer: A) -> Self {
        Self { peer }
    }
}

impl<A> From<A> for UnwatchPeer<A> {
    fn from(peer: A) -> Self {
        Self::new(peer)
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

impl<A: Address> From<(A, Result<Exit<A>, Crash>)> for PeerStopped<A> {
    fn from((peer, outcome): (A, Result<Exit<A>, Crash>)) -> Self {
        Self { peer, outcome }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl<A: Address> From<(A::Nonce, Result<Exit<A>, Crash>, Instant)> for ChildStopped<A> {
    fn from((nonce, outcome, at): (A::Nonce, Result<Exit<A>, Crash>, Instant)) -> Self {
        Self { nonce, outcome, at }
    }
}

/// Ask the local interpreter to observe the exact child generation bound at
/// `nonce`.
///
/// Creation is resolved before same-action service sends. If that creation was
/// rejected, no child exists to observe: the interpreter consumes this request
/// without installing an observation or emitting [`ChildStopped`]. The
/// rejection remains observable through [`ObserveCreation`], and a later
/// creation cannot inherit the consumed observation.
pub struct ObserveChild<A: Address, Occurrence> {
    pub nonce: A::Nonce,
    occurrence: core::marker::PhantomData<fn() -> Occurrence>,
}

impl<A: Address, Occurrence> Copy for ObserveChild<A, Occurrence> {}

impl<A: Address, Occurrence> Clone for ObserveChild<A, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Address, Occurrence> PartialEq for ObserveChild<A, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}

impl<A: Address, Occurrence> Eq for ObserveChild<A, Occurrence> {}

impl<A: Address, Occurrence> core::fmt::Debug for ObserveChild<A, Occurrence>
where
    A::Nonce: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ObserveChild")
            .field("nonce", &self.nonce)
            .finish()
    }
}

impl<A: Address, Occurrence> ObserveChild<A, Occurrence> {
    #[must_use]
    pub const fn new(nonce: A::Nonce) -> Self {
        Self {
            nonce,
            occurrence: core::marker::PhantomData,
        }
    }

    /// Observe the child selected by one typed creator-local route.
    #[must_use]
    pub const fn at<C>(route: behavior::ChildRoute<C, Occurrence>) -> Self
    where
        C: behavior::Behavior,
        C::Protocol: behavior::Protocol<Addr = A>,
    {
        Self::new(route.nonce())
    }
}

impl<A: Address, Occurrence> behavior::InterpreterRequest for ObserveChild<A, Occurrence> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<ChildStopped<A>, behavior::Here>;
}

/// A proxy's request for its interpreter to report a worker termination to
/// the proxy's parent. The interpreter supplies the emitting proxy's child
/// nonce when constructing [`WorkerStopped`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportWorkerStopped<A: Address, Path> {
    /// Exact worker-stop owner in the proxy's parent event algebra.
    pub ingress: behavior::Ingress<WorkerStopped<A>, Path>,
    pub worker: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

impl<A: Address, Path> ReportWorkerStopped<A, Path> {
    #[must_use]
    pub fn new(
        ingress: behavior::Ingress<WorkerStopped<A>, Path>,
        worker: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
        at: Instant,
    ) -> Self {
        Self {
            ingress,
            worker,
            outcome,
            at,
        }
    }
}

impl<A: Address, Path> behavior::InterpreterRequest for ReportWorkerStopped<A, Path> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
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

impl<A: Address, Path> From<(A::Nonce, ReportWorkerStopped<A, Path>)> for WorkerStopped<A> {
    fn from((proxy, stopped): (A::Nonce, ReportWorkerStopped<A, Path>)) -> Self {
        Self::new(proxy, stopped.worker, stopped.outcome, stopped.at)
    }
}

/// The committed result of one staged [`crate::Create`] request.
///
/// `Installed` is emitted only after fresh allocation, successful
/// initialization, and binding at `nonce`. The replacement provenance is the
/// provenance supplied by Behavior; an interpreter must never infer it from
/// address reuse or creation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreationResolved<A: behavior::Address> {
    pub nonce: A::Nonce,
    pub kind: CreationKind<A::Nonce>,
    pub result: Result<A, CreationRejection>,
}

impl<A: behavior::Address> CreationResolved<A> {
    #[must_use]
    pub const fn new(
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        result: Result<A, CreationRejection>,
    ) -> Self {
        Self {
            nonce,
            kind,
            result,
        }
    }

    #[must_use]
    pub const fn installed(nonce: A::Nonce, kind: CreationKind<A::Nonce>, address: A) -> Self {
        Self::new(nonce, kind, Ok(address))
    }

    /// A successfully committed ordinary birth.
    #[must_use]
    pub const fn birth(nonce: A::Nonce, address: A) -> Self {
        Self::installed(nonce, CreationKind::Birth, address)
    }

    /// A successfully committed replacement incarnation.
    #[must_use]
    pub const fn replacement_incarnation(nonce: A::Nonce, replaces: A::Nonce, address: A) -> Self {
        Self::installed(
            nonce,
            CreationKind::ReplacementIncarnation { replaces },
            address,
        )
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

impl<A: behavior::Address>
    From<(
        A::Nonce,
        CreationKind<A::Nonce>,
        Result<A, CreationRejection>,
    )> for CreationResolved<A>
{
    fn from(
        (nonce, kind, result): (
            A::Nonce,
            CreationKind<A::Nonce>,
            Result<A, CreationRejection>,
        ),
    ) -> Self {
        Self {
            nonce,
            kind,
            result,
        }
    }
}

/// Ask the local interpreter to return the committed result of the same-action
/// creation at `nonce` through the behavior's typed creation-result lane.
pub struct ObserveCreation<A: Address, Occurrence> {
    pub nonce: A::Nonce,
    occurrence: core::marker::PhantomData<fn() -> Occurrence>,
}

impl<A: Address, Occurrence> Copy for ObserveCreation<A, Occurrence> {}

impl<A: Address, Occurrence> Clone for ObserveCreation<A, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Address, Occurrence> PartialEq for ObserveCreation<A, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}

impl<A: Address, Occurrence> Eq for ObserveCreation<A, Occurrence> {}

impl<A: Address, Occurrence> core::fmt::Debug for ObserveCreation<A, Occurrence>
where
    A::Nonce: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ObserveCreation")
            .field("nonce", &self.nonce)
            .finish()
    }
}

impl<A: Address, Occurrence> ObserveCreation<A, Occurrence> {
    #[must_use]
    pub const fn new(nonce: A::Nonce) -> Self {
        Self {
            nonce,
            occurrence: core::marker::PhantomData,
        }
    }

    /// Observe the creation staged through one typed creator-local route.
    #[must_use]
    pub const fn at<C>(route: behavior::ChildRoute<C, Occurrence>) -> Self
    where
        C: behavior::Behavior,
        C::Protocol: behavior::Protocol<Addr = A>,
    {
        Self::new(route.nonce())
    }
}

impl<A: Address, Occurrence> behavior::InterpreterRequest for ObserveCreation<A, Occurrence> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<CreationResolved<A>, behavior::Here>;
}

/// Ask a proxy's interpreter to report a worker creation result to its parent.
/// The interpreter supplies the emitting proxy's nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportWorkerCreationResolved<N, Path> {
    /// Exact creation-result owner in the proxy's parent event algebra.
    pub ingress: behavior::Ingress<WorkerCreationResolved<N>, Path>,
    pub worker: N,
    pub kind: CreationKind<N>,
    pub result: Result<(), CreationRejection>,
}

impl<N, Path> ReportWorkerCreationResolved<N, Path> {
    #[must_use]
    pub const fn new(
        ingress: behavior::Ingress<WorkerCreationResolved<N>, Path>,
        worker: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self {
            ingress,
            worker,
            kind,
            result,
        }
    }
}

impl<N, Path> behavior::InterpreterRequest for ReportWorkerCreationResolved<N, Path> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
}

/// A worker creation result reported by a still-live supervised proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerCreationResolved<N> {
    pub proxy: N,
    pub worker: N,
    pub kind: CreationKind<N>,
    pub result: Result<(), CreationRejection>,
}

/// The complete, statically selected return capability given to a stable
/// proxy when its parent stages the proxy's fresh creation.
///
/// Actor acquaintance locality requires the proxy to receive this capability;
/// the `Path` is Bombay's derived proof of the exact owners in the parent's
/// composed event algebra. Neither the proxy nor its interpreter may discover
/// a parent lane from runtime topology or payload type.
pub struct ProxyParentIngress<A: Address, Path> {
    pub stopped: behavior::Ingress<WorkerStopped<A>, Path>,
    pub creation: behavior::Ingress<WorkerCreationResolved<A::Nonce>, Path>,
}

impl<A: Address, Path> Copy for ProxyParentIngress<A, Path> {}

impl<A: Address, Path> Clone for ProxyParentIngress<A, Path> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Address, Path> core::fmt::Debug for ProxyParentIngress<A, Path> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ProxyParentIngress")
    }
}

impl<A: Address, Path> PartialEq for ProxyParentIngress<A, Path> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<A: Address, Path> Eq for ProxyParentIngress<A, Path> {}

impl<A: Address, Path> ProxyParentIngress<A, Path> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stopped: behavior::Ingress::new(),
            creation: behavior::Ingress::new(),
        }
    }

    /// Lift both correlated report lanes through one structural parent layer.
    #[must_use]
    pub const fn inside(self) -> ProxyParentIngress<A, behavior::Inside<Path>> {
        ProxyParentIngress {
            stopped: self.stopped.inside(),
            creation: self.creation.inside(),
        }
    }
}

impl<A: Address, Path> Default for ProxyParentIngress<A, Path> {
    fn default() -> Self {
        Self::new()
    }
}

/// Consumer-facing resolution of one explicitly designated replacement.
///
/// This is a derived view of [`WorkerCreationResolved`], not another runtime
/// fact or observation request. `replaced` is the exact prior incarnation
/// carried by Behavior in [`CreationKind::ReplacementIncarnation`];
/// `replacement`/`attempt` is the fresh creation nonce. The prior worker's
/// terminal outcome remains the separate [`WorkerStopped`] fact so creation
/// resolution cannot duplicate, erase, or reinterpret it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementResolution<N> {
    Installed {
        proxy: N,
        replaced: N,
        replacement: N,
    },
    Rejected {
        proxy: N,
        replaced: N,
        attempt: N,
        rejection: CreationRejection,
    },
}

impl<N> WorkerCreationResolved<N> {
    #[must_use]
    pub const fn new(
        proxy: N,
        worker: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self {
            proxy,
            worker,
            kind,
            result,
        }
    }

    /// Project a replacement result without conflating ordinary birth with
    /// replacement or inferring provenance from nonce arithmetic.
    #[must_use]
    pub fn into_replacement(self) -> Option<ReplacementResolution<N>> {
        let CreationKind::ReplacementIncarnation { replaces } = self.kind else {
            return None;
        };
        Some(match self.result {
            Ok(()) => ReplacementResolution::Installed {
                proxy: self.proxy,
                replaced: replaces,
                replacement: self.worker,
            },
            Err(rejection) => ReplacementResolution::Rejected {
                proxy: self.proxy,
                replaced: replaces,
                attempt: self.worker,
                rejection,
            },
        })
    }
}

impl<N> From<(N, N, CreationKind<N>, Result<(), CreationRejection>)> for WorkerCreationResolved<N> {
    fn from(
        (proxy, worker, kind, result): (N, N, CreationKind<N>, Result<(), CreationRejection>),
    ) -> Self {
        Self {
            proxy,
            worker,
            kind,
            result,
        }
    }
}

impl<N, Path> From<(N, ReportWorkerCreationResolved<N, Path>)> for WorkerCreationResolved<N> {
    fn from((proxy, resolved): (N, ReportWorkerCreationResolved<N, Path>)) -> Self {
        Self::new(proxy, resolved.worker, resolved.kind, resolved.result)
    }
}

/// A request to finish through one serialized behavior transition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ShutdownRequested;

/// Ask the local interpreter to begin orderly shutdown of one established
/// child of protocol `C` in the emitting actor's namespace.
///
/// Acceptance is not completion. A successfully accepted request is completed
/// only by the corresponding [`ChildStopped`] fact. If the interpreter cannot
/// select an established `C` child, it must return [`ChildShutdownRejected`]
/// rather than fabricate termination or fail the whole action application.
/// Protocol identity is retained in the type even when two child protocols use
/// the same address and nonce types:
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, ChildHead, MailAddr, Never, NoBirths, Protocol, User};
/// use behavior_actors::ShutdownChild;
///
/// struct Queue;
/// struct Worker;
/// macro_rules! inert {
///     ($actor:ty) => {
///         impl Protocol for $actor {
///             type Addr = MailAddr;
///             type Msg = u8;
///         }
///         impl Behavior for $actor {
///             type Event = User<MailAddr, u8>;
///             type Sends = Vec<Never>;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = NoBirths;
///             fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> {
///                 Ok(Actions::cont())
///             }
///             fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///                 Ok(Actions::cont())
///             }
///         }
///     };
/// }
/// inert!(Queue);
/// inert!(Worker);
///
/// let queue = ShutdownChild::<Queue, ChildHead>::new(1);
/// let _: ShutdownChild<Worker, ChildHead> = queue;
/// ```
///
/// Repeated occurrences of the same behavior are also incompatible:
///
/// ```compile_fail
/// use behavior::{
///     Actions, Behavior, ChildHead, ChildTail, MailAddr, Never, NoBirths, Protocol, User,
/// };
/// use behavior_actors::ShutdownChild;
/// struct Worker;
/// impl Protocol for Worker {
///     type Addr = MailAddr;
///     type Msg = ();
/// }
/// impl Behavior for Worker {
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(
///         &mut self,
///         _: behavior::ActiveTurn,
///         _: Self::Event,
///     ) -> behavior::BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
/// let first = ShutdownChild::<Worker, ChildHead>::new(1);
/// let _: ShutdownChild<Worker, ChildTail<ChildHead>> = first;
/// ```
pub struct ShutdownChild<C: behavior::Behavior, Occurrence> {
    pub nonce: <crate::BehaviorAddr<C> as behavior::Address>::Nonce,
    /// Exact shutdown owner in the selected child behavior.
    pub ingress: behavior::Ingress<ShutdownRequested, behavior::Here>,
    protocol: core::marker::PhantomData<fn() -> (C, Occurrence)>,
}

impl<C: behavior::Behavior, Occurrence> ShutdownChild<C, Occurrence> {
    #[must_use]
    pub const fn new(nonce: <crate::BehaviorAddr<C> as behavior::Address>::Nonce) -> Self {
        Self {
            nonce,
            ingress: behavior::Ingress::new(),
            protocol: core::marker::PhantomData,
        }
    }

    /// Request shutdown through one typed route to this exact child behavior.
    #[must_use]
    pub const fn at(route: behavior::ChildRoute<C, Occurrence>) -> Self {
        Self::new(route.nonce())
    }
}

impl<C: behavior::Behavior, Occurrence> behavior::InterpreterRequest
    for ShutdownChild<C, Occurrence>
{
    type ReturnToEmitter = behavior::ReturnsToEmitter<
        ChildShutdownRejected<<crate::BehaviorAddr<C> as behavior::Address>::Nonce>,
        behavior::Here,
    >;
}

impl<C: behavior::Behavior, Occurrence> Copy for ShutdownChild<C, Occurrence> {}

impl<C: behavior::Behavior, Occurrence> Clone for ShutdownChild<C, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: behavior::Behavior, Occurrence> PartialEq for ShutdownChild<C, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}

impl<C: behavior::Behavior, Occurrence> Eq for ShutdownChild<C, Occurrence> {}

impl<C: behavior::Behavior, Occurrence> core::fmt::Debug for ShutdownChild<C, Occurrence>
where
    <crate::BehaviorAddr<C> as behavior::Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShutdownChild")
            .field("nonce", &self.nonce)
            .finish()
    }
}

/// Why a local child-shutdown request was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChildShutdownRejection {
    /// No established child is bound at the requested creator-local nonce.
    #[error("no established child exists at the requested nonce")]
    NotEstablished,
    /// Shutdown was already accepted for the selected child.
    #[error("child shutdown is already in progress")]
    AlreadyStopping,
}

/// Explicit failed resolution of one [`ShutdownChild`] request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildShutdownRejected<N> {
    pub nonce: N,
    pub reason: ChildShutdownRejection,
}

impl<N> ChildShutdownRejected<N> {
    #[must_use]
    pub const fn new(nonce: N, reason: ChildShutdownRejection) -> Self {
        Self { nonce, reason }
    }
}

impl<N> From<(N, ChildShutdownRejection)> for ChildShutdownRejected<N> {
    fn from((nonce, reason): (N, ChildShutdownRejection)) -> Self {
        Self { nonce, reason }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MailAddr;

    #[test]
    fn lifecycle_conversions_preserve_every_semantic_field() {
        let at = Instant::now();
        let child: ChildStopped<MailAddr> = (3, Err(Crash::Failed), at).into();
        let report = ReportWorkerStopped::new(
            behavior::Ingress::<WorkerStopped<MailAddr>, behavior::Here>::new(),
            child.nonce,
            child.outcome,
            child.at,
        );
        let worker = WorkerStopped::from((7, report));
        assert_eq!(worker.proxy, 7);
        assert_eq!(worker.worker, 3);
        assert_eq!(worker.outcome, Err(Crash::Failed));
        assert_eq!(worker.at, at);

        let creation = CreationResolved::<behavior::MailAddr>::rejected(
            4,
            CreationKind::replacement_of(3),
            CreationRejection::EnvironmentFailed,
        );
        let report = ReportWorkerCreationResolved::new(
            behavior::Ingress::<WorkerCreationResolved<u64>, behavior::Here>::new(),
            creation.nonce,
            creation.kind,
            creation.result.map(|_| ()),
        );
        let worker = WorkerCreationResolved::from((7, report));
        assert_eq!(worker.proxy, 7);
        assert_eq!(worker.worker, 4);
        assert_eq!(worker.kind, CreationKind::replacement_of(3));
        assert_eq!(worker.result, Err(CreationRejection::EnvironmentFailed));
        assert_eq!(
            worker.into_replacement(),
            Some(ReplacementResolution::Rejected {
                proxy: 7,
                replaced: 3,
                attempt: 4,
                rejection: CreationRejection::EnvironmentFailed,
            })
        );

        let installed = WorkerCreationResolved::new(7, 5, CreationKind::replacement_of(4), Ok(()));
        assert_eq!(
            installed.into_replacement(),
            Some(ReplacementResolution::Installed {
                proxy: 7,
                replaced: 4,
                replacement: 5,
            })
        );
        assert_eq!(
            WorkerCreationResolved::new(7, 0, CreationKind::Birth, Ok(())).into_replacement(),
            None
        );
        assert_eq!(
            WorkerCreationResolved::new(
                7,
                0,
                CreationKind::Birth,
                Err(CreationRejection::NonceAlreadyBound),
            )
            .into_replacement(),
            None
        );
    }

    #[test]
    fn one_child_route_drives_observation_creation_and_shutdown_correlation() {
        enum WorkerRole {}

        struct Worker;

        impl behavior::Protocol for Worker {
            type Addr = MailAddr;
            type Msg = behavior::Never;
        }

        impl behavior::Behavior for Worker {
            type Protocol = Self;
            type Event = behavior::User<MailAddr, behavior::Never>;
            type Sends = Vec<behavior::Never>;
            type Ph = behavior::Never;
            type Error = behavior::Never;
            type Birth = behavior::NoBirths;

            fn transition(
                &mut self,
                _: behavior::ActiveTurn,
                event: Self::Event,
            ) -> behavior::BehaviorActed<Self> {
                match event.message {}
            }
        }

        let route = behavior::ChildRoute::<Worker, WorkerRole>::new(13);

        fn copy_without_occurrence_bounds<T: Copy>() {}
        copy_without_occurrence_bounds::<ObserveChild<MailAddr, WorkerRole>>();
        copy_without_occurrence_bounds::<ObserveCreation<MailAddr, WorkerRole>>();
        copy_without_occurrence_bounds::<ShutdownChild<Worker, WorkerRole>>();

        assert_eq!(ObserveChild::at(route).nonce, 13);
        assert_eq!(ObserveCreation::at(route).nonce, 13);
        assert_eq!(ShutdownChild::at(route).nonce, 13);
    }

    #[test]
    fn expected_lane_types_infer_lossless_protocol_products() {
        let peer: ObservePeer<MailAddr> = MailAddr(7).into();
        let child: ObserveChild<MailAddr, behavior::ChildHead> = ObserveChild::new(9);
        let creation: ObserveCreation<MailAddr, behavior::ChildHead> = ObserveCreation::new(11);
        let rejected: ChildShutdownRejected<u64> =
            (13, ChildShutdownRejection::NotEstablished).into();

        assert_eq!(peer.peer, MailAddr(7));
        assert_eq!(child.nonce, 9);
        assert_eq!(creation.nonce, 11);
        assert_eq!(rejected.nonce, 13);
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
