//! Neutral typed vocabulary for interpreter-originated event and service lanes.
//!
//! Concrete behavior transformations define the closed sum types that add
//! these lanes. Keeping their values and construction capabilities here avoids
//! dependencies between otherwise independent transformations.

mod established;

pub use established::{
    CancelObservation, EstablishedChild, EstablishedObservation, EstablishedShutdownResolved,
    InterpretEstablishedObservation, InterpretEstablishedShutdown, ObservationId,
    ObservationOperation, ObservationRejection, ObserveEstablished, ObserveEstablishedCreation,
    ShutdownEstablished, ShutdownId, ShutdownRejection, established_child,
};

use std::time::Duration;

use std::time::Instant;

use crate::{Crash, CreationId, CreationKind, Exit};
pub use behavior::CreationRejection;
use behavior::{ActionItem, Address, Protocol, SourceAction};

/// Exact timer correlation accepted by the local scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerScheduled {
    /// Actor-local timer identity supplied by the behavior.
    pub id: TimerId,
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

impl ActionItem for ScheduleAt {
    type Accepted = TimerScheduled;
    type Rejection = ScheduleAtRejection;
    type Prerequisite = behavior::Never;
}

impl SourceAction for ScheduleAt {
    type Source = Self;
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

impl ActionItem for ScheduleAfter {
    type Accepted = TimerScheduled;
    type Rejection = ScheduleAfterRejection;
    type Prerequisite = behavior::Never;
}

impl SourceAction for ScheduleAfter {
    type Source = Self;
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
/// interpreter that can select neither a live incarnation nor retained terminal
/// history must return the complete request with
/// [`PeerObservationRejection::UnknownAddress`] rather than fabricate a stop
/// result.
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

/// Exact reason a logical peer observation was not established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PeerObservationRejection {
    /// No live incarnation or authoritative retained termination exists at the
    /// requested logical address.
    #[error("no actor incarnation exists at the requested logical address")]
    UnknownAddress,
}

impl<A: Address> behavior::InterpreterRequest for ObservePeer<A> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<PeerStopped<A>, behavior::Here>;
}

/// Acceptance establishes the relationship and leaves unit; its later result
/// is [`PeerStopped`]. Name resolution is the only expected rejection, and the
/// request has no prerequisite.
impl<A> ActionItem for ObservePeer<A>
where
    A: Address + Send,
{
    type Accepted = ();
    type Rejection = PeerObservationRejection;
    type Prerequisite = behavior::Never;
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
    pub child: CreationId,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

impl<A: Address> ChildStopped<A> {
    #[must_use]
    pub fn new(child: CreationId, outcome: Result<Exit<A>, Crash>, at: Instant) -> Self {
        Self { child, outcome, at }
    }
}

impl<A: Address> From<(CreationId, Result<Exit<A>, Crash>, Instant)> for ChildStopped<A> {
    fn from((child, outcome, at): (CreationId, Result<Exit<A>, Crash>, Instant)) -> Self {
        Self { child, outcome, at }
    }
}

/// Ask the local interpreter to observe the exact child generation of protocol
/// `P` bound at one creator-local creation ID.
///
/// Creation is resolved before same-action service sends. If that creation was
/// rejected, the request is blocked by its exact
/// [`behavior::CreationCorrelation`]. An established child is observed through
/// its concrete protocol and declared occurrence; equal address types do not
/// make different child protocols interchangeable.
///
/// ```compile_fail,E0308
/// struct Worker;
/// impl behavior::Protocol for Worker {
///     type Addr = behavior::MailAddr;
///     type Msg = ();
/// }
/// struct Account;
/// impl behavior::Protocol for Account {
///     type Addr = behavior::MailAddr;
///     type Msg = ();
/// }
/// let child = behavior::CreationSequence::new()
///     .issue()
///     .expect("the first creation ID exists");
/// let account = behavior_actors::ObserveChild::<Account, behavior::ChildHead>::new(child);
/// let _: behavior_actors::ObserveChild<Worker, behavior::ChildHead> = account;
/// ```
pub struct ObserveChild<P: Protocol, Occurrence> {
    pub child: CreationId,
    protocol: core::marker::PhantomData<fn() -> P>,
    occurrence: core::marker::PhantomData<fn() -> Occurrence>,
}

impl<P: Protocol, Occurrence> Copy for ObserveChild<P, Occurrence> {}

impl<P: Protocol, Occurrence> Clone for ObserveChild<P, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol, Occurrence> PartialEq for ObserveChild<P, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.child == other.child
    }
}

impl<P: Protocol, Occurrence> Eq for ObserveChild<P, Occurrence> {}

impl<P: Protocol, Occurrence> core::fmt::Debug for ObserveChild<P, Occurrence> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ObserveChild")
            .field("child", &self.child)
            .finish()
    }
}

impl<P: Protocol, Occurrence> ObserveChild<P, Occurrence> {
    #[must_use]
    pub const fn new(child: CreationId) -> Self {
        Self {
            child,
            protocol: core::marker::PhantomData,
            occurrence: core::marker::PhantomData,
        }
    }
}

impl<P: Protocol, Occurrence> behavior::InterpreterRequest for ObserveChild<P, Occurrence> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<ChildStopped<P::Addr>, behavior::Here>;
}

impl<P, Occurrence> behavior::ActionItem for ObserveChild<P, Occurrence>
where
    P: Protocol,
    <P::Addr as Address>::Nonce: Send,
{
    type Accepted = ();
    type Rejection = behavior::Never;
    type Prerequisite = behavior::CreationCorrelation<P, Occurrence>;
}

/// The committed result of one staged [`crate::CreateChild`] request.
///
/// `Installed` is emitted only after fresh allocation, successful
/// initialization, and binding at `nonce`. The replacement provenance is the
/// provenance supplied by Behavior; an interpreter must never infer it from
/// address reuse or creation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreationResolved<A: behavior::Address> {
    pub creation: CreationId,
    pub kind: CreationKind,
    pub result: Result<A, CreationRejection>,
}

impl<A: behavior::Address> CreationResolved<A> {
    #[must_use]
    pub const fn new(
        creation: CreationId,
        kind: CreationKind,
        result: Result<A, CreationRejection>,
    ) -> Self {
        Self {
            creation,
            kind,
            result,
        }
    }

    #[must_use]
    pub const fn installed(creation: CreationId, kind: CreationKind, address: A) -> Self {
        Self::new(creation, kind, Ok(address))
    }

    /// A successfully committed ordinary birth.
    #[must_use]
    pub const fn birth(creation: CreationId, address: A) -> Self {
        Self::installed(creation, CreationKind::Birth, address)
    }

    /// A successfully committed replacement incarnation.
    #[must_use]
    pub const fn replacement(creation: CreationId, previous: CreationId, address: A) -> Self {
        Self::installed(creation, CreationKind::replacement(previous), address)
    }

    #[must_use]
    pub const fn rejected(
        creation: CreationId,
        kind: CreationKind,
        rejection: CreationRejection,
    ) -> Self {
        Self::new(creation, kind, Err(rejection))
    }
}

impl<A: behavior::Address> From<(CreationId, CreationKind, Result<A, CreationRejection>)>
    for CreationResolved<A>
{
    fn from(
        (creation, kind, result): (CreationId, CreationKind, Result<A, CreationRejection>),
    ) -> Self {
        Self {
            creation,
            kind,
            result,
        }
    }
}

/// Ask the local interpreter to return the committed result of the same-action
/// creation ID through the behavior's typed creation-result lane.
///
/// Equal address and message types do not make child protocols substitutable:
///
/// ```compile_fail,E0308
/// struct Store;
/// struct Gateway;
/// impl behavior::Protocol for Store {
///     type Addr = behavior::MailAddr;
///     type Msg = ();
/// }
/// impl behavior::Protocol for Gateway {
///     type Addr = behavior::MailAddr;
///     type Msg = ();
/// }
/// let creation = behavior::CreationSequence::new()
///     .issue()
///     .expect("the child creation ID exists");
/// let gateway = behavior_actors::ObserveCreation::<Gateway, behavior::ChildHead>::new(creation);
/// let _: behavior_actors::ObserveCreation<Store, behavior::ChildHead> = gateway;
/// ```
#[doc(hidden)]
pub struct ObserveCreation<P: Protocol, Occurrence> {
    pub creation: CreationId,
    occurrence: core::marker::PhantomData<fn() -> (P, Occurrence)>,
}

impl<P: Protocol, Occurrence> Copy for ObserveCreation<P, Occurrence> {}

impl<P: Protocol, Occurrence> Clone for ObserveCreation<P, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol, Occurrence> PartialEq for ObserveCreation<P, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.creation == other.creation
    }
}

impl<P: Protocol, Occurrence> Eq for ObserveCreation<P, Occurrence> {}

impl<P: Protocol, Occurrence> core::fmt::Debug for ObserveCreation<P, Occurrence> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ObserveCreation")
            .field("creation", &self.creation)
            .finish()
    }
}

impl<P: Protocol, Occurrence> ObserveCreation<P, Occurrence> {
    #[must_use]
    pub const fn new(creation: CreationId) -> Self {
        Self {
            creation,
            occurrence: core::marker::PhantomData,
        }
    }
}

impl<P: Protocol, Occurrence> behavior::InterpreterRequest for ObserveCreation<P, Occurrence> {
    type ReturnToEmitter = behavior::ReturnsToEmitter<CreationResolved<P::Addr>, behavior::Here>;
}

impl<P, Occurrence> behavior::ActionItem for ObserveCreation<P, Occurrence>
where
    P: Protocol,
    <P::Addr as Address>::Nonce: Send,
{
    type Accepted = ();
    type Rejection = behavior::Never;
    type Prerequisite = behavior::CreationCorrelation<P, Occurrence>;
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
/// use behavior::{
///     Actions, Behavior, ChildHead, CreationSequence, MailAddr, Never, NoBirths, Protocol, User,
/// };
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
///             type Protocol = Self;
///             type Event = User<MailAddr, u8>;
///             type Sends = Vec<Never>;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = NoBirths;
///             fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
///                 Ok(Actions::cont())
///             }
///             fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///                 Ok(Actions::cont())
///             }
///         }
///     };
/// }
/// inert!(Queue);
/// inert!(Worker);
///
/// let child = CreationSequence::new()
///     .issue()
///     .expect("the first creation ID exists");
/// let queue = ShutdownChild::<Queue, ChildHead>::new(child);
/// let _: ShutdownChild<Worker, ChildHead> = queue;
/// ```
///
/// Repeated occurrences of the same behavior are also incompatible:
///
/// ```compile_fail
/// use behavior::{
///     Actions, Behavior, ChildHead, ChildTail, CreationSequence, MailAddr, Never, NoBirths,
///     Protocol, User,
/// };
/// use behavior_actors::ShutdownChild;
/// struct Worker;
/// impl Protocol for Worker {
///     type Addr = MailAddr;
///     type Msg = ();
/// }
/// impl Behavior for Worker {
///     type Protocol = Self;
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
/// let child = CreationSequence::new()
///     .issue()
///     .expect("the first creation ID exists");
/// let first = ShutdownChild::<Worker, ChildHead>::new(child);
/// let _: ShutdownChild<Worker, ChildTail<ChildHead>> = first;
/// ```
pub struct ShutdownChild<C: behavior::Behavior, Occurrence> {
    pub child: CreationId,
    /// Exact shutdown owner in the selected child behavior.
    pub ingress: behavior::Ingress<ShutdownRequested, behavior::Here>,
    protocol: core::marker::PhantomData<fn() -> (C, Occurrence)>,
}

impl<C: behavior::Behavior, Occurrence> ShutdownChild<C, Occurrence> {
    #[must_use]
    pub const fn new(child: CreationId) -> Self {
        Self {
            child,
            ingress: behavior::Ingress::new(),
            protocol: core::marker::PhantomData,
        }
    }
}

impl<C: behavior::Behavior, Occurrence> behavior::InterpreterRequest
    for ShutdownChild<C, Occurrence>
{
    type ReturnToEmitter = behavior::ReturnsToEmitter<ChildShutdownRejected, behavior::Here>;
}

impl<C, Occurrence> behavior::ActionItem for ShutdownChild<C, Occurrence>
where
    C: behavior::Behavior,
    <crate::BehaviorAddr<C> as behavior::Address>::Nonce: Send,
{
    type Accepted = ();
    type Rejection = ChildShutdownRejection;
    type Prerequisite = behavior::CreationCorrelation<C::Protocol, Occurrence>;
}

impl<C: behavior::Behavior, Occurrence> Copy for ShutdownChild<C, Occurrence> {}

impl<C: behavior::Behavior, Occurrence> Clone for ShutdownChild<C, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: behavior::Behavior, Occurrence> PartialEq for ShutdownChild<C, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.child == other.child
    }
}

impl<C: behavior::Behavior, Occurrence> Eq for ShutdownChild<C, Occurrence> {}

impl<C: behavior::Behavior, Occurrence> core::fmt::Debug for ShutdownChild<C, Occurrence> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShutdownChild")
            .field("child", &self.child)
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
pub struct ChildShutdownRejected {
    pub child: CreationId,
    pub reason: ChildShutdownRejection,
}

impl ChildShutdownRejected {
    #[must_use]
    pub const fn new(child: CreationId, reason: ChildShutdownRejection) -> Self {
        Self { child, reason }
    }
}

impl From<(CreationId, ChildShutdownRejection)> for ChildShutdownRejected {
    fn from((child, reason): (CreationId, ChildShutdownRejection)) -> Self {
        Self { child, reason }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MailAddr;

    fn creation(number: u64) -> CreationId {
        let mut sequence = behavior::CreationSequence::new();
        (0..number)
            .filter_map(|_| sequence.issue())
            .last()
            .unwrap_or_else(|| panic!("test creation ID is issued"))
    }

    #[test]
    fn one_creation_correlates_observation_creation_and_shutdown() {
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

        let child = creation(13);

        fn copy_without_occurrence_bounds<T: Copy>() {}
        copy_without_occurrence_bounds::<ObserveChild<Worker, WorkerRole>>();
        copy_without_occurrence_bounds::<ObserveCreation<Worker, WorkerRole>>();
        copy_without_occurrence_bounds::<ShutdownChild<Worker, WorkerRole>>();

        assert_eq!(ObserveChild::<Worker, WorkerRole>::new(child).child, child);
        assert_eq!(
            ObserveCreation::<Worker, WorkerRole>::new(child).creation,
            child
        );
        assert_eq!(ShutdownChild::<Worker, WorkerRole>::new(child).child, child);
    }

    #[test]
    fn expected_lane_types_infer_lossless_protocol_products() {
        let peer: ObservePeer<MailAddr> = MailAddr(7).into();
        let child: ObserveChild<
            behavior::MessageProtocol<MailAddr, behavior::Never>,
            behavior::ChildHead,
        > = ObserveChild::new(creation(9));
        let observed_creation: ObserveCreation<
            behavior::MessageProtocol<MailAddr, behavior::Never>,
            behavior::ChildHead,
        > = ObserveCreation::new(creation(11));
        let rejected: ChildShutdownRejected =
            (creation(13), ChildShutdownRejection::NotEstablished).into();

        assert_eq!(peer.peer, MailAddr(7));
        assert_eq!(child.child.get(), 9);
        assert_eq!(observed_creation.creation.get(), 11);
        assert_eq!(rejected.child.get(), 13);
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
