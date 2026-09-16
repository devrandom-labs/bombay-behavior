//! Exact-incarnation observation and orderly-shutdown protocols.

use core::marker::PhantomData;
use std::time::Instant;

use behavior::{
    ActionItem, Behavior, CreationId, EndpointAddress, EstablishedActor, EstablishedRecipient,
    Ingress, InjectEvent, InterpretEstablished, InterpreterRequest, ItemSettlement, Never,
    Protocol, ReturnsToEmitter,
};

use super::ShutdownRequested;
use crate::{Crash, Exit};

/// Request the exact committed result of a same-action creation at one named
/// creator-local role.
///
/// The interpreter commits creation before interpreting this request. A
/// successful report carries an [`EstablishedRecipient`]; a rejected report
/// carries no capability. `Occurrence` keeps duplicate declarations of the
/// same child protocol distinct without becoming protocol identity or a
/// runtime key.
pub struct ObserveEstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub creation: CreationId,
    occurrence: PhantomData<fn() -> (P, Occurrence)>,
}

impl<P, Occurrence> ObserveEstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn new(creation: CreationId) -> Self {
        Self {
            creation,
            occurrence: PhantomData,
        }
    }
}

impl<P, Occurrence> Copy for ObserveEstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}

impl<P, Occurrence> Clone for ObserveEstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<P, Occurrence> InterpreterRequest for ObserveEstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type ReturnToEmitter =
        ReturnsToEmitter<behavior::EstablishedCreation<P, Occurrence>, behavior::Here>;
}

impl<P, Occurrence> ActionItem for ObserveEstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as behavior::Address>::Nonce: Send,
{
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = behavior::CreationCorrelation<P, Occurrence>;
}

/// Both capabilities established by one committed named-child creation.
///
/// `creation` remains relative to the creating actor and selects its private
/// child binding together with the static occurrence. `actor` is the stronger
/// exact installed capability used by exact delivery, observation, or
/// [`ShutdownEstablished`].
pub struct EstablishedChild<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
    creation: CreationId,
    actor: EstablishedActor<C>,
    occurrence: PhantomData<fn() -> Occurrence>,
}

impl<C, Occurrence> EstablishedChild<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
    /// Return the creator-local ID of this child.
    #[must_use]
    pub const fn creation(&self) -> CreationId {
        self.creation
    }

    /// Clone the exact installed-actor capability.
    #[must_use]
    pub fn actor(&self) -> EstablishedActor<C> {
        self.actor.clone()
    }

    /// Select this child in an existing heterogeneous shutdown target sum.
    ///
    /// `Parent` restores the namespace in which `Occurrence` was declared;
    /// the compiler then selects the role's exact structural position. No
    /// role value, address reconstruction, or runtime protocol choice is
    /// required after creation has committed.
    #[must_use]
    pub fn shutdown_target<Parent, Targets>(&self) -> Targets
    where
        Parent: Behavior,
        Occurrence: behavior::ChildRole<Parent, Child = C>,
        Targets: crate::ShutdownTargetAt<C, <Occurrence as behavior::ChildRole<Parent>>::Position>,
    {
        Targets::shutdown_target_at(self.creation)
    }

    /// Consume the product into its local and exact capabilities.
    #[must_use]
    pub fn into_parts(self) -> (CreationId, EstablishedActor<C>) {
        (self.creation, self.actor)
    }
}

/// Strengthen one successful named-child creation report without losing either
/// of its routing capabilities.
///
/// `Role` must be the occurrence declared by `Parent`, so the returned exact
/// actor proves the concrete child behavior while the returned ID retains
/// creator-local correlation. This is an Actors-level
/// construction over existing Behavior capabilities, not another creation
/// operation or an allocation shortcut.
///
/// # Errors
///
/// Returns the creation's typed [`behavior::CreationRejection`] and produces
/// no actor capability when installation did not commit.
pub fn established_child<Parent, Role>(
    report: behavior::EstablishedCreation<behavior::RoleProtocol<Parent, Role>, Role>,
) -> Result<EstablishedChild<behavior::RoleChild<Parent, Role>, Role>, behavior::CreationRejection>
where
    Parent: Behavior,
    Role: behavior::ChildRole<Parent>,
    behavior::BehaviorAddr<behavior::RoleChild<Parent, Role>>: EndpointAddress,
{
    let creation = report.id();
    let actor = report.into_actor::<Parent>()?;
    Ok(EstablishedChild {
        creation,
        actor,
        occurrence: PhantomData,
    })
}

/// Behavior-owned correlation for one observation relationship.
///
/// This value is local relationship evidence, not actor identity, endpoint
/// identity, or proof that an observation was accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObservationId(pub u64);

/// Request observation of one exact installed protocol incarnation.
pub struct ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub id: ObservationId,
    recipient: EstablishedRecipient<P>,
}

impl<P> ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn new(id: ObservationId, recipient: EstablishedRecipient<P>) -> Self {
        Self { id, recipient }
    }

    /// Transfer the endpoint through the explicit power-user interpretation
    /// boundary.
    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretEstablishedObservation<P>,
    {
        self.recipient.interpret(&mut ObservationTransfer {
            id: self.id,
            interpreter,
        })
    }

    /// Transfer this request and return its unconditional action settlement.
    pub fn settle<I>(self, interpreter: &mut I) -> ItemSettlement<Self, (), Never, Never>
    where
        I: InterpretEstablishedObservation<P, Output = ()>,
    {
        self.interpret(interpreter);
        ItemSettlement::Accepted(())
    }
}

impl<P> InterpreterRequest for ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type ReturnToEmitter = ReturnsToEmitter<EstablishedObservation<P>, behavior::Here>;
}

impl<P> ActionItem for ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
{
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

/// Cancel one exact observer-local relationship.
pub struct CancelObservation<P: Protocol> {
    pub id: ObservationId,
    protocol: PhantomData<fn() -> P>,
}

impl<P: Protocol> CancelObservation<P> {
    #[must_use]
    pub const fn new(id: ObservationId) -> Self {
        Self {
            id,
            protocol: PhantomData,
        }
    }

    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        P::Addr: EndpointAddress,
        I: InterpretEstablishedObservation<P>,
    {
        interpreter.cancel(self.id)
    }

    /// Transfer this request and return its unconditional action settlement.
    pub fn settle<I>(self, interpreter: &mut I) -> ItemSettlement<Self, (), Never, Never>
    where
        P::Addr: EndpointAddress,
        I: InterpretEstablishedObservation<P, Output = ()>,
    {
        self.interpret(interpreter);
        ItemSettlement::Accepted(())
    }
}

impl<P: Protocol> Copy for CancelObservation<P> {}

impl<P: Protocol> Clone for CancelObservation<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol> InterpreterRequest for CancelObservation<P> {
    type ReturnToEmitter = ReturnsToEmitter<EstablishedObservation<P>, behavior::Here>;
}

impl<P: Protocol> ActionItem for CancelObservation<P> {
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

/// Operation correlated by an [`ObservationId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationOperation {
    Start,
    Cancel,
}

/// Semantic rejection of an exact observation operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ObservationRejection {
    /// This observation ID already names a live relationship.
    #[error("the observation ID is already bound")]
    IdAlreadyBound,
    /// Cancellation named no live relationship.
    #[error("the observation ID is not bound")]
    NotObserved,
}

/// Complete report algebra for one exact observation relationship.
pub enum EstablishedObservation<P: Protocol> {
    /// The observation relationship was installed.
    Started {
        id: ObservationId,
        protocol: PhantomData<fn() -> P>,
    },
    /// Cancellation consumed the live relationship.
    Cancelled {
        id: ObservationId,
        protocol: PhantomData<fn() -> P>,
    },
    /// Starting or cancelling the relationship was rejected.
    Rejected {
        id: ObservationId,
        operation: ObservationOperation,
        reason: ObservationRejection,
        protocol: PhantomData<fn() -> P>,
    },
    /// The exact observed incarnation terminated, consuming the relationship.
    Stopped {
        id: ObservationId,
        outcome: Result<Exit<P::Addr>, Crash>,
        at: Instant,
        protocol: PhantomData<fn() -> P>,
    },
}

impl<P: Protocol> EstablishedObservation<P> {
    #[must_use]
    pub const fn started(id: ObservationId) -> Self {
        Self::Started {
            id,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn cancelled(id: ObservationId) -> Self {
        Self::Cancelled {
            id,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn rejected(
        id: ObservationId,
        operation: ObservationOperation,
        reason: ObservationRejection,
    ) -> Self {
        Self::Rejected {
            id,
            operation,
            reason,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn stopped(
        id: ObservationId,
        outcome: Result<Exit<P::Addr>, Crash>,
        at: Instant,
    ) -> Self {
        Self::Stopped {
            id,
            outcome,
            at,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(&self) -> ObservationId {
        match self {
            Self::Started { id, .. }
            | Self::Cancelled { id, .. }
            | Self::Rejected { id, .. }
            | Self::Stopped { id, .. } => *id,
        }
    }
}

/// Public power-user boundary for exact observation transfer.
pub trait InterpretEstablishedObservation<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Output;

    fn observe(
        &mut self,
        id: ObservationId,
        endpoint: <P::Addr as EndpointAddress>::Established<P>,
    ) -> Self::Output;

    fn cancel(&mut self, id: ObservationId) -> Self::Output;
}

struct ObservationTransfer<'a, I> {
    id: ObservationId,
    interpreter: &'a mut I,
}

impl<P, I> InterpretEstablished<P> for ObservationTransfer<'_, I>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    I: InterpretEstablishedObservation<P>,
{
    type Output = I::Output;

    fn interpret_established(
        &mut self,
        endpoint: <P::Addr as EndpointAddress>::Established<P>,
    ) -> Self::Output {
        self.interpreter.observe(self.id, endpoint)
    }
}

/// Behavior-owned correlation for one orderly-shutdown request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShutdownId(pub u64);

/// Request orderly shutdown of one exact installed concrete behavior.
///
/// `TargetPath` proves where [`ShutdownRequested`] enters the installed
/// behavior's closed event algebra. The interpreter receives that typed
/// ingress together with the exact endpoint; shutdown is therefore still an
/// explicit event/effect transformation, not a privileged runtime side
/// channel.
///
/// A concrete actor whose event algebra has no shutdown ingress cannot be
/// strengthened into an orderly-shutdown request:
///
/// ```compile_fail
/// use behavior::{
///     Actions, Address, Behavior, BehaviorActed, EndpointAddress,
///     EstablishedActor, Here, Ingress, Never, NoBirths, NoSends, Protocol, User,
/// };
/// use behavior_actors::{
///     ShutdownEstablished, ShutdownId, ShutdownRequested,
/// };
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct RuntimeAddr(u64);
/// impl Address for RuntimeAddr { type Nonce = u64; }
/// struct Endpoint;
/// impl Clone for Endpoint { fn clone(&self) -> Self { Self } }
/// impl EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: Protocol<Addr = Self>;
/// }
/// struct Worker;
/// impl Protocol for Worker { type Addr = RuntimeAddr; type Msg = (); }
/// impl Behavior for Worker {
///     type Protocol = Self;
///     type Event = User<RuntimeAddr, ()>;
///     type Sends = NoSends;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(
///         &mut self,
///         _: behavior::ActiveTurn,
///         _: Self::Event,
///     ) -> BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// let actor = EstablishedActor::<Worker>::issued(Endpoint);
/// let _ = ShutdownEstablished::<Worker, Here>::new(
///     ShutdownId(1),
///     actor,
///     Ingress::<ShutdownRequested, Here>::new(),
/// );
/// ```
pub struct ShutdownEstablished<B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
{
    pub id: ShutdownId,
    actor: EstablishedActor<B>,
    ingress: Ingress<ShutdownRequested, TargetPath>,
}

impl<B, TargetPath> ShutdownEstablished<B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
{
    #[must_use]
    pub const fn new(
        id: ShutdownId,
        actor: EstablishedActor<B>,
        ingress: Ingress<ShutdownRequested, TargetPath>,
    ) -> Self {
        Self { id, actor, ingress }
    }

    /// Clone the exact actor capability without changing request ownership.
    #[must_use]
    pub fn actor(&self) -> EstablishedActor<B> {
        self.actor.clone()
    }

    /// Attempt this request and return its one statically selected settlement.
    pub fn settle<I>(
        self,
        interpreter: &mut I,
    ) -> ItemSettlement<Self, ShutdownId, ShutdownRejection, Never>
    where
        I: InterpretEstablishedShutdown<B, TargetPath>,
    {
        let admission = self.actor.clone().interpret(&mut ShutdownTransfer {
            id: self.id,
            ingress: self.ingress,
            interpreter,
            behavior: PhantomData,
        });
        match admission {
            Ok(()) => ItemSettlement::Accepted(self.id),
            Err(reason) => ItemSettlement::Rejected { item: self, reason },
        }
    }
}

impl<B, TargetPath> InterpreterRequest for ShutdownEstablished<B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
{
    type ReturnToEmitter =
        ReturnsToEmitter<EstablishedShutdownResolved<B::Protocol>, behavior::Here>;
}

impl<B, TargetPath> ActionItem for ShutdownEstablished<B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
    EstablishedActor<B>: Send,
{
    type Accepted = ShutdownId;
    type Rejection = ShutdownRejection;
    type Prerequisite = Never;
}

/// Semantic rejection of exact orderly shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShutdownRejection {
    #[error("shutdown is already in progress for the exact incarnation")]
    AlreadyStopping,
    #[error("the exact incarnation is already stopped")]
    AlreadyStopped,
}

/// Complete immediate resolution of one exact orderly-shutdown request.
pub enum EstablishedShutdownResolved<P: Protocol> {
    Accepted {
        id: ShutdownId,
        protocol: PhantomData<fn() -> P>,
    },
    Rejected {
        id: ShutdownId,
        reason: ShutdownRejection,
        protocol: PhantomData<fn() -> P>,
    },
}

impl<P: Protocol> EstablishedShutdownResolved<P> {
    #[must_use]
    pub const fn accepted(id: ShutdownId) -> Self {
        Self::Accepted {
            id,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn rejected(id: ShutdownId, reason: ShutdownRejection) -> Self {
        Self::Rejected {
            id,
            reason,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(&self) -> ShutdownId {
        match self {
            Self::Accepted { id, .. } | Self::Rejected { id, .. } => *id,
        }
    }
}

/// Public power-user boundary for exact orderly-shutdown transfer.
pub trait InterpretEstablishedShutdown<B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
{
    fn shutdown(
        &mut self,
        id: ShutdownId,
        endpoint: <behavior::BehaviorAddr<B> as EndpointAddress>::Established<B::Protocol>,
        ingress: Ingress<ShutdownRequested, TargetPath>,
    ) -> Result<(), ShutdownRejection>;
}

struct ShutdownTransfer<'a, I, B, TargetPath> {
    id: ShutdownId,
    ingress: Ingress<ShutdownRequested, TargetPath>,
    interpreter: &'a mut I,
    behavior: PhantomData<fn() -> B>,
}

impl<B, TargetPath, I> InterpretEstablished<B::Protocol> for ShutdownTransfer<'_, I, B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
    I: InterpretEstablishedShutdown<B, TargetPath>,
{
    type Output = Result<(), ShutdownRejection>;

    fn interpret_established(
        &mut self,
        endpoint: <behavior::BehaviorAddr<B> as EndpointAddress>::Established<B::Protocol>,
    ) -> Self::Output {
        self.interpreter.shutdown(self.id, endpoint, self.ingress)
    }
}
