//! Neutral typed vocabulary for interpreter-originated event and service lanes.
//!
//! Concrete behavior transformations define the closed sum types that add
//! these lanes. Keeping their values and construction capabilities here avoids
//! dependencies between otherwise independent transformations.

pub(crate) mod forward;
mod pool;

pub use pool::{KeyedWorkerPoolProtocol, PoolAssignmentProtocol, WorkerPoolProtocol};

use std::time::Duration;

use std::time::Instant;

use crate::{Crash, CreationKind, Exit};
use behavior::Address;

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

/// Ask the local interpreter to cancel this actor's observation of `peer`.
///
/// Peer observation is a derived Bombay protocol, not an actor-model
/// primitive. The address names the same observer-local relationship created
/// by [`ObservePeer`]; exact-incarnation capture and cancellation belong to the
/// interpreter. Cancellation does not retract a [`PeerStopped`] event already
/// admitted to the actor's mailbox, and an interpreter treats a request for a
/// relationship that is no longer present as inert.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveChild<N> {
    pub nonce: N,
}

impl<N> ObserveChild<N> {
    #[must_use]
    pub const fn new(nonce: N) -> Self {
        Self { nonce }
    }
}

impl<N> From<N> for ObserveChild<N> {
    fn from(nonce: N) -> Self {
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

impl<A: Address> From<(A::Nonce, Result<Exit<A>, Crash>, Instant)> for ReportWorkerStopped<A> {
    fn from((worker, outcome, at): (A::Nonce, Result<Exit<A>, Crash>, Instant)) -> Self {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveCreation<N> {
    pub nonce: N,
}

impl<N> ObserveCreation<N> {
    #[must_use]
    pub const fn new(nonce: N) -> Self {
        Self { nonce }
    }
}

impl<N> From<N> for ObserveCreation<N> {
    fn from(nonce: N) -> Self {
        Self { nonce }
    }
}

/// Ask a proxy's interpreter to report a worker creation result to its parent.
/// The interpreter supplies the emitting proxy's nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportWorkerCreationResolved<N> {
    pub worker: N,
    pub kind: CreationKind<N>,
    pub result: Result<(), CreationRejection>,
}

impl<N> ReportWorkerCreationResolved<N> {
    #[must_use]
    pub const fn new(
        worker: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self {
            worker,
            kind,
            result,
        }
    }
}

impl<N> From<(N, CreationKind<N>, Result<(), CreationRejection>)>
    for ReportWorkerCreationResolved<N>
{
    fn from((worker, kind, result): (N, CreationKind<N>, Result<(), CreationRejection>)) -> Self {
        Self {
            worker,
            kind,
            result,
        }
    }
}

impl<A: behavior::Address> From<CreationResolved<A>> for ReportWorkerCreationResolved<A::Nonce> {
    fn from(resolved: CreationResolved<A>) -> Self {
        Self::new(resolved.nonce, resolved.kind, resolved.result.map(|_| ()))
    }
}

/// A worker creation result reported by a still-live supervised proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerCreationResolved<N> {
    pub proxy: N,
    pub worker: N,
    pub kind: CreationKind<N>,
    pub result: Result<(), CreationRejection>,
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

impl<N> From<(N, ReportWorkerCreationResolved<N>)> for WorkerCreationResolved<N> {
    fn from((proxy, resolved): (N, ReportWorkerCreationResolved<N>)) -> Self {
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
/// use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, Protocol, User};
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
/// let queue = ShutdownChild::<Queue>::new(1);
/// let _: ShutdownChild<Worker> = queue;
/// ```
pub struct ShutdownChild<C: behavior::Behavior> {
    pub nonce: <crate::BehaviorAddr<C> as behavior::Address>::Nonce,
    protocol: core::marker::PhantomData<fn() -> C>,
}

impl<C: behavior::Behavior> ShutdownChild<C> {
    #[must_use]
    pub const fn new(nonce: <crate::BehaviorAddr<C> as behavior::Address>::Nonce) -> Self {
        Self {
            nonce,
            protocol: core::marker::PhantomData,
        }
    }
}

impl<C: behavior::Behavior> Copy for ShutdownChild<C> {}

impl<C: behavior::Behavior> Clone for ShutdownChild<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: behavior::Behavior> PartialEq for ShutdownChild<C> {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}

impl<C: behavior::Behavior> Eq for ShutdownChild<C> {}

impl<C: behavior::Behavior> core::fmt::Debug for ShutdownChild<C>
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
        let report = ReportWorkerStopped::from(child);
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
        let report = ReportWorkerCreationResolved::from(creation);
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
    fn expected_lane_types_infer_lossless_protocol_products() {
        let peer: ObservePeer<MailAddr> = MailAddr(7).into();
        let child: ObserveChild<u64> = 9.into();
        let creation: ObserveCreation<u64> = 11.into();
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
