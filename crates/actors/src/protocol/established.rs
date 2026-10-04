//! Exact-incarnation observation and orderly-shutdown protocols.

use core::fmt;
use core::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use behavior::{
    ActionItem, Behavior, CreationId, EndpointAddress, EstablishedActor, EstablishedRecipient,
    Ingress, InjectEvent, InterpretEstablished, InterpretInstalledActor, InterpreterRequest,
    ItemSettlement, Never, Protocol, RecipientAddress, ReturnsToEmitter,
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
pub struct ObserveEstablishedCreation<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
    pub creation: CreationId,
    occurrence: PhantomData<fn() -> (C, Occurrence)>,
}

impl<C, Occurrence> ObserveEstablishedCreation<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
    #[must_use]
    pub const fn new(creation: CreationId) -> Self {
        Self {
            creation,
            occurrence: PhantomData,
        }
    }
}

impl<C, Occurrence> Copy for ObserveEstablishedCreation<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
}

impl<C, Occurrence> Clone for ObserveEstablishedCreation<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<C, Occurrence> InterpreterRequest for ObserveEstablishedCreation<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
{
    type ReturnToEmitter =
        ReturnsToEmitter<behavior::EstablishedCreation<C, Occurrence>, behavior::Here>;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<C, Occurrence> ActionItem for ObserveEstablishedCreation<C, Occurrence>
where
    C: Behavior,
    behavior::BehaviorAddr<C>: EndpointAddress,
    <behavior::BehaviorAddr<C> as behavior::Address>::Nonce: Send,
{
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = behavior::CreationCorrelation<C::Protocol, Occurrence>;
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
    report: behavior::EstablishedCreation<behavior::RoleChild<Parent, Role>, Role>,
) -> Result<EstablishedChild<behavior::RoleChild<Parent, Role>, Role>, behavior::CreationRejection>
where
    Parent: Behavior,
    Role: behavior::ChildRole<Parent>,
    behavior::BehaviorAddr<behavior::RoleChild<Parent, Role>>: EndpointAddress,
{
    let (creation, _, actor) = report.into_committed()?.into_parts();
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

/// Non-authorizing identity of one accepted protocol-indexed relationship.
///
/// The protocol brand cannot be changed even when the address is shared:
/// ```compile_fail,E0308
/// fn wrong_protocol<P, Q>(relationship: behavior_actors::ObservationRelationship<P>, outcome: Result<behavior_actors::Exit<Q::Addr>, behavior_actors::Crash>, at: std::time::Instant) -> behavior_actors::EstablishedObservation<Q>
/// where P: behavior::Protocol, Q: behavior::Protocol<Addr = P::Addr>, Q::Addr: behavior::RecipientAddress {
///     behavior_actors::EstablishedObservation::Stopped { relationship, outcome, at }
/// }
/// ```
/// A read-only relationship is not a cancellation permission:
/// ```compile_fail,E0308
/// fn permission<P: behavior::Protocol>(relationship: behavior_actors::ObservationRelationship<P>) -> behavior_actors::ObservationAuthority<P> {
///     relationship
/// }
/// ```
///
/// Same-protocol terminal reports and read-only identity transfer remain valid.
/// ```no_run
/// pub fn stopped<P, Q>(relationship: behavior_actors::ObservationRelationship<P>, outcome: Result<behavior_actors::Exit<P::Addr>, behavior_actors::Crash>, at: std::time::Instant) -> behavior_actors::EstablishedObservation<P>
/// where P: behavior::Protocol, Q: behavior::Protocol<Addr = P::Addr>, P::Addr: behavior::RecipientAddress {
///     behavior_actors::EstablishedObservation::Stopped { relationship, outcome, at }
/// }
/// ```
/// ```no_run
/// pub fn retained_relationship<P: behavior::Protocol>(relationship: behavior_actors::ObservationRelationship<P>) -> behavior_actors::ObservationRelationship<P> {
///     relationship
/// }
/// ```
pub struct ObservationRelationship<P: Protocol> {
    identity: Arc<ObservationId>,
    protocol: PhantomData<fn(P) -> P>,
}

impl<P: Protocol> fmt::Debug for ObservationRelationship<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObservationRelationship")
            .field("id", &self.id())
            .finish_non_exhaustive()
    }
}

impl<P: Protocol> ObservationRelationship<P> {
    #[must_use]
    pub fn id(&self) -> ObservationId {
        *self.identity
    }

    /// Borrow the strong accepted identity for exact runtime membership checks.
    ///
    /// Reading or cloning this Arc confers no cancellation permission and
    /// cannot reconstruct or rebrand a relationship.
    #[must_use]
    pub fn identity(&self) -> &Arc<ObservationId> {
        &self.identity
    }
}

impl<P: Protocol> Clone for ObservationRelationship<P> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            protocol: PhantomData,
        }
    }
}

impl<P: Protocol> PartialEq for ObservationRelationship<P> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

impl<P: Protocol> Eq for ObservationRelationship<P> {}

/// Affine permission to attempt cancellation of one exact relationship.
///
/// A permission does not assert that its registration remains live.
/// It transfers once and cannot be reused after constructing cancellation:
/// ```compile_fail,E0382
/// fn duplicate<P: behavior::Protocol>(authority: behavior_actors::ObservationAuthority<P>) {
///     let first = behavior_actors::CancelObservation::new(authority);
///     let second = behavior_actors::CancelObservation::new(authority);
///     drop((first, second));
/// }
/// ```
/// A different protocol cannot consume the grant:
/// ```compile_fail,E0308
/// fn wrong_protocol<P: behavior::Protocol, Q: behavior::Protocol<Addr = P::Addr>>(authority: behavior_actors::ObservationAuthority<P>) -> behavior_actors::CancelObservation<Q> {
///     behavior_actors::CancelObservation::new(authority)
/// }
/// ```
///
/// A same-protocol grant transfers into exactly one cancellation request.
/// ```no_run
/// pub fn cancellation<P: behavior::Protocol>(authority: behavior_actors::ObservationAuthority<P>) {
///     let first = behavior_actors::CancelObservation::new(authority);
///     drop(first);
/// }
/// ```
/// ```no_run
/// pub fn cancellation<P: behavior::Protocol, Q: behavior::Protocol<Addr = P::Addr>>(authority: behavior_actors::ObservationAuthority<P>) -> behavior_actors::CancelObservation<P> {
///     behavior_actors::CancelObservation::new(authority)
/// }
/// ```
pub struct ObservationAuthority<P: Protocol> {
    relationship: ObservationRelationship<P>,
}

impl<P: Protocol> fmt::Debug for ObservationAuthority<P> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ObservationAuthority")
            .field("relationship", &self.relationship)
            .finish_non_exhaustive()
    }
}

impl<P: Protocol> ObservationAuthority<P> {
    /// Transfer the original private request identity into its affine authority.
    ///
    /// This advanced-host operation consumes the original request. The host
    /// must commit the same identity to its exact membership owner before
    /// publishing Started. Perform issuance outside all Behavior folds.
    /// Never issue for a rejected request or return an admitted original.
    #[must_use]
    pub fn issued(request: ObserveEstablished<P>) -> Self
    where
        P::Addr: RecipientAddress,
    {
        let ObserveEstablished {
            correlation,
            recipient,
        } = request;
        drop(recipient);
        Self {
            relationship: ObservationRelationship {
                identity: correlation,
                protocol: PhantomData,
            },
        }
    }

    #[must_use]
    pub fn relationship(&self) -> &ObservationRelationship<P> {
        &self.relationship
    }

    pub(crate) fn matches_request(&self, correlation: &Arc<ObservationId>) -> bool {
        Arc::ptr_eq(self.relationship.identity(), correlation)
    }

    /// Discharge permission and retain the exact non-authorizing relationship.
    #[must_use]
    pub fn into_relationship(self) -> ObservationRelationship<P> {
        self.relationship
    }
}

/// One affine request for exact protocol-indexed termination observation.
///
/// The admitted original transfers once into its authority. Nested request
/// lanes require separate report ingress at each structural path.
/// ```no_run
/// pub fn accepted_request<P>(request: behavior_actors::ObserveEstablished<P>) where P: behavior::Protocol, P::Addr: behavior::RecipientAddress {
///     let first = behavior_actors::ObservationAuthority::issued(request);
///     drop(first);
/// }
/// ```
/// ```compile_fail,E0382
/// pub fn accepted_request<P>(request: behavior_actors::ObserveEstablished<P>) where P: behavior::Protocol, P::Addr: behavior::RecipientAddress {
///     let first = behavior_actors::ObservationAuthority::issued(request);
///     let second = behavior_actors::ObservationAuthority::issued(request);
///     drop((first, second));
/// }
/// ```
/// ```no_run
/// fn accepts<Event, Sends: behavior::SendsFor<Event>>() {}
///
/// pub fn independent_acknowledgements<P>() where P: behavior::Protocol, P::Addr: behavior::RecipientAddress {
///     accepts::<behavior::EventLayer<behavior_actors::EstablishedObservation<P>, behavior::EventLayer<behavior_actors::EstablishedObservation<P>, behavior::User<P::Addr, P::Msg>>>, behavior::SendLayer<behavior::InterpreterRequests<behavior_actors::CancelObservation<P>>, behavior::SendLayer<behavior::InterpreterRequests<behavior_actors::ObserveEstablished<P>>, Vec<behavior::Never>>>>();
/// }
/// ```
/// ```compile_fail,E0277
/// fn accepts<Event, Sends: behavior::SendsFor<Event>>() {}
///
/// pub fn independent_acknowledgements<P>() where P: behavior::Protocol, P::Addr: behavior::RecipientAddress {
///     accepts::<behavior::EventLayer<behavior_actors::EstablishedObservation<P>, behavior::User<P::Addr, P::Msg>>, behavior::SendLayer<behavior::InterpreterRequests<behavior_actors::CancelObservation<P>>, behavior::SendLayer<behavior::InterpreterRequests<behavior_actors::ObserveEstablished<P>>, Vec<behavior::Never>>>>();
/// }
/// ```
/// A request remains affine and cannot be cloned:
/// ```no_run
/// pub fn transfer<P>(request: behavior_actors::ObserveEstablished<P>) -> behavior_actors::ObserveEstablished<P> where P: behavior::Protocol, P::Addr: behavior::RecipientAddress { request }
/// ```
/// ```compile_fail,E0599
/// pub fn transfer<P>(request: behavior_actors::ObserveEstablished<P>) -> behavior_actors::ObserveEstablished<P> where P: behavior::Protocol, P::Addr: behavior::RecipientAddress { request.clone() }
/// ```
/// Reading a relationship cannot reconstruct its old request identity:
/// ```no_run
/// pub fn new_request<P>(relationship: behavior_actors::ObservationRelationship<P>, recipient: behavior::EstablishedRecipient<P>) -> behavior_actors::ObserveEstablished<P> where P: behavior::Protocol, P::Addr: behavior::RecipientAddress { behavior_actors::ObserveEstablished::new(relationship.id(), recipient) }
/// ```
/// ```compile_fail,E0451
/// pub fn old_request<P>(relationship: behavior_actors::ObservationRelationship<P>, recipient: behavior::EstablishedRecipient<P>) -> behavior_actors::ObserveEstablished<P> where P: behavior::Protocol, P::Addr: behavior::RecipientAddress { behavior_actors::ObserveEstablished { correlation: relationship.identity().clone(), recipient } }
/// ```
pub struct ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    correlation: Arc<ObservationId>,
    recipient: EstablishedRecipient<P>,
}

impl<P> ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    /// Construct one affine request with a fresh private correlation.
    ///
    /// Issue the request outside Behavior folds, then transfer the whole
    /// original into its owning Actions lane. A never-accepted rejected
    /// original may be transferred again; new construction has fresh identity.
    #[must_use]
    pub fn new(id: ObservationId, recipient: EstablishedRecipient<P>) -> Self {
        Self {
            correlation: Arc::new(id),
            recipient,
        }
    }

    #[must_use]
    pub fn id(&self) -> ObservationId {
        *self.correlation
    }

    pub(crate) fn correlation(&self) -> &Arc<ObservationId> {
        &self.correlation
    }

    /// Discharge this request correlation and recover the original inputs.
    #[must_use]
    pub fn into_inputs(self) -> (ObservationId, EstablishedRecipient<P>) {
        (*self.correlation, self.recipient)
    }

    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretEstablishedObservation<P>,
    {
        let recipient = self.recipient.clone();
        recipient.interpret(&mut ObservationTransfer {
            request: Some(self),
            interpreter,
        })
    }

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
    P::Addr: RecipientAddress,
{
    type ReturnToEmitter = ReturnsToEmitter<EstablishedObservation<P>, behavior::Here>;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<P> ActionItem for ObserveEstablished<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: Send,
{
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

/// One affine attempt to cancel the exact accepted relationship.
///
/// Cancellation acknowledgements require their own typed report ingress.
/// ```no_run
/// fn accepts<Event, Sends: behavior::SendsFor<Event>>() {}
///
/// pub fn cancellation_acknowledgement<P>() where P: behavior::Protocol, P::Addr: behavior::RecipientAddress {
///     accepts::<behavior::EventLayer<behavior_actors::EstablishedObservation<P>, behavior::User<P::Addr, P::Msg>>, behavior::InterpreterRequests<behavior_actors::CancelObservation<P>>>();
/// }
/// ```
/// ```compile_fail,E0277
/// fn accepts<Event, Sends: behavior::SendsFor<Event>>() {}
///
/// pub fn cancellation_acknowledgement<P>() where P: behavior::Protocol, P::Addr: behavior::RecipientAddress {
///     accepts::<behavior::User<P::Addr, P::Msg>, behavior::InterpreterRequests<behavior_actors::CancelObservation<P>>>();
/// }
/// ```
pub struct CancelObservation<P: Protocol> {
    authority: ObservationAuthority<P>,
}

impl<P: Protocol> CancelObservation<P> {
    #[must_use]
    pub fn new(authority: ObservationAuthority<P>) -> Self {
        Self { authority }
    }

    #[must_use]
    pub fn id(&self) -> ObservationId {
        self.authority.relationship().id()
    }

    #[must_use]
    pub fn relationship(&self) -> &ObservationRelationship<P> {
        self.authority.relationship()
    }

    /// Recover the same permission from an unconsumed cancellation request.
    #[must_use]
    pub fn into_authority(self) -> ObservationAuthority<P> {
        self.authority
    }

    /// Consume permission and retain the exact non-authorizing receipt.
    #[must_use]
    pub fn into_relationship(self) -> ObservationRelationship<P> {
        self.authority.into_relationship()
    }

    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        P::Addr: RecipientAddress,
        I: InterpretEstablishedObservation<P>,
    {
        interpreter.cancel(self)
    }

    pub fn settle<I>(self, interpreter: &mut I) -> ItemSettlement<Self, (), Never, Never>
    where
        P::Addr: RecipientAddress,
        I: InterpretEstablishedObservation<P, Output = ()>,
    {
        self.interpret(interpreter);
        ItemSettlement::Accepted(())
    }
}

impl<P: Protocol> InterpreterRequest for CancelObservation<P>
where
    P::Addr: RecipientAddress,
{
    type ReturnToEmitter = ReturnsToEmitter<EstablishedObservation<P>, behavior::Here>;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<P: Protocol> ActionItem for CancelObservation<P>
where
    P::Addr: RecipientAddress,
{
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
    /// Cancellation named no exact registered relationship.
    #[error("the exact observation relationship is not registered")]
    NotObserved,
}

/// Whole protocol-indexed observation and cancellation receipts.
pub enum EstablishedObservation<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    Started {
        authority: ObservationAuthority<P>,
    },
    Stopped {
        relationship: ObservationRelationship<P>,
        outcome: Result<Exit<P::Addr>, Crash>,
        at: Instant,
    },
    Cancelled {
        relationship: ObservationRelationship<P>,
    },
    ObserveRejected {
        request: ObserveEstablished<P>,
        reason: ObservationRejection,
    },
    CancelRejected {
        request: CancelObservation<P>,
        reason: ObservationRejection,
    },
}

impl<P> EstablishedObservation<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    #[must_use]
    pub fn started(authority: ObservationAuthority<P>) -> Self {
        Self::Started { authority }
    }

    #[must_use]
    pub fn stopped(
        relationship: ObservationRelationship<P>,
        outcome: Result<Exit<P::Addr>, Crash>,
        at: Instant,
    ) -> Self {
        Self::Stopped {
            relationship,
            outcome,
            at,
        }
    }

    #[must_use]
    pub fn cancelled(request: CancelObservation<P>) -> Self {
        Self::Cancelled {
            relationship: request.into_relationship(),
        }
    }

    #[must_use]
    pub fn observe_rejected(request: ObserveEstablished<P>, reason: ObservationRejection) -> Self {
        Self::ObserveRejected { request, reason }
    }

    #[must_use]
    pub fn cancel_rejected(request: CancelObservation<P>, reason: ObservationRejection) -> Self {
        Self::CancelRejected { request, reason }
    }

    #[must_use]
    pub fn id(&self) -> ObservationId {
        match self {
            Self::Started { authority } => authority.relationship().id(),
            Self::Stopped { relationship, .. } | Self::Cancelled { relationship } => {
                relationship.id()
            }
            Self::ObserveRejected { request, .. } => request.id(),
            Self::CancelRejected { request, .. } => request.id(),
        }
    }
}

/// Advanced-host transfer of the whole original requests and exact endpoint.
pub trait InterpretEstablishedObservation<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    type Output;

    fn observe(
        &mut self,
        request: ObserveEstablished<P>,
        endpoint: <P::Addr as RecipientAddress>::Established<P>,
    ) -> Self::Output;
    fn cancel(&mut self, request: CancelObservation<P>) -> Self::Output;
}

struct ObservationTransfer<'a, P, I>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    request: Option<ObserveEstablished<P>>,
    interpreter: &'a mut I,
}

impl<P, I> InterpretEstablished<P> for ObservationTransfer<'_, P, I>
where
    P: Protocol,
    P::Addr: RecipientAddress,
    I: InterpretEstablishedObservation<P>,
{
    type Output = I::Output;

    fn interpret_established(
        &mut self,
        endpoint: <P::Addr as RecipientAddress>::Established<P>,
    ) -> Self::Output {
        let Some(request) = self.request.take() else {
            unreachable!("Core transfers one owned established endpoint once");
        };
        self.interpreter.observe(request, endpoint)
    }
}

/// Behavior-owned correlation for one orderly-shutdown request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShutdownId(pub u64);

/// Request orderly shutdown of one exact installed concrete behavior.
///
/// `TargetPath` proves where [`ShutdownRequested`] enters the installed
/// behavior's closed event algebra. The interpreter receives that typed
/// ingress together with the exact installed actor; shutdown is therefore still an
/// explicit event/effect transformation, not a privileged runtime side
/// channel.
///
/// A concrete actor whose event algebra has no shutdown ingress cannot be
/// strengthened into an orderly-shutdown request:
///
/// ```compile_fail,E0277
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct RuntimeAddr(u64);
/// impl behavior::Address for RuntimeAddr { type Nonce = u64; }
/// struct Endpoint;
/// impl Clone for Endpoint { fn clone(&self) -> Self { Self } }
/// struct Installed<B: behavior::Behavior>(Endpoint, std::sync::mpsc::Sender<B::Event>);
/// impl<B: behavior::Behavior> Clone for Installed<B> {
///     fn clone(&self) -> Self { Self(self.0.clone(), self.1.clone()) }
/// }
/// impl behavior::EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
///     type Installed<B> = Installed<B>
///         where B: behavior::Behavior<Protocol: behavior::Protocol<Addr = Self>>;
///     fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
///     where B: behavior::Behavior<Protocol: behavior::Protocol<Addr = Self>> {
///         installed.0.clone()
///     }
/// }
/// struct Worker;
/// impl behavior::Protocol for Worker { type Addr = RuntimeAddr; type Msg = (); }
/// impl behavior::Behavior for Worker {
///     type Protocol = Self;
///     type Event = behavior::User<RuntimeAddr, ()>;
///     type Sends = behavior::NoSends;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(
///         &mut self,
///         _: behavior::ActiveTurn,
///         _: Self::Event,
///     ) -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// }
/// let (control, _inbox) = std::sync::mpsc::channel::<<Worker as behavior::Behavior>::Event>();
/// let actor = behavior::EstablishedActor::<Worker>::issued(Installed(Endpoint, control));
/// let _ = behavior_actors::ShutdownEstablished::<Worker, behavior::Here>::new(
///     behavior_actors::ShutdownId(1),
///     actor,
///     behavior::Ingress::<behavior_actors::ShutdownRequested, behavior::Here>::new(),
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
        let admission = self.actor.clone().interpret_actor(&mut ShutdownTransfer {
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
    type LogicalProtocols = behavior::NoBirthProtocols;
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
        installed: <behavior::BehaviorAddr<B> as EndpointAddress>::Installed<B>,
        ingress: Ingress<ShutdownRequested, TargetPath>,
    ) -> Result<(), ShutdownRejection>;
}

struct ShutdownTransfer<'a, I, B, TargetPath> {
    id: ShutdownId,
    ingress: Ingress<ShutdownRequested, TargetPath>,
    interpreter: &'a mut I,
    behavior: PhantomData<fn() -> B>,
}

impl<B, TargetPath, I> InterpretInstalledActor<B> for ShutdownTransfer<'_, I, B, TargetPath>
where
    B: Behavior,
    behavior::BehaviorAddr<B>: EndpointAddress,
    B::Event: InjectEvent<ShutdownRequested, TargetPath>,
    I: InterpretEstablishedShutdown<B, TargetPath>,
{
    type Output = Result<(), ShutdownRejection>;

    fn interpret_actor(
        &mut self,
        installed: <behavior::BehaviorAddr<B> as EndpointAddress>::Installed<B>,
    ) -> Self::Output {
        self.interpreter.shutdown(self.id, installed, self.ingress)
    }
}

#[cfg(test)]
mod observation_request_ownership {
    use std::sync::Arc;

    use behavior::{
        Address, EstablishedRecipient, InterpretEstablished, Protocol, RecipientAddress,
    };

    use super::{ObservationAuthority, ObservationId, ObserveEstablished};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct ObservationAddr(u64);

    impl Address for ObservationAddr {
        type Nonce = u64;
    }
    impl RecipientAddress for ObservationAddr {
        type Established<P>
            = Arc<Vec<u64>>
        where
            P: Protocol<Addr = Self>;
    }

    struct ObservedProtocol;
    impl Protocol for ObservedProtocol {
        type Addr = ObservationAddr;
        type Msg = ();
    }

    struct ObservationEndpoint;
    impl InterpretEstablished<ObservedProtocol> for ObservationEndpoint {
        type Output = Arc<Vec<u64>>;
        fn interpret_established(&mut self, endpoint: Arc<Vec<u64>>) -> Self::Output {
            endpoint
        }
    }

    #[test]
    fn original_request_inputs_return_without_reconstruction() {
        let values = Arc::new(vec![17, 43, 31]);
        let original = values.as_ptr();
        let id = ObservationId(91);
        let recipient = EstablishedRecipient::<ObservedProtocol>::issued(values);
        let request = ObserveEstablished::new(id, recipient);
        let (returned_id, recipient) = request.into_inputs();
        let returned = recipient.interpret(&mut ObservationEndpoint);
        assert_eq!(returned_id, id);
        assert_eq!(returned.as_ptr(), original);
        assert_eq!(returned.as_slice(), [17, 43, 31]);
    }

    #[test]
    fn distinct_request_construction_cannot_alias_preacceptance() {
        let id = ObservationId(92);
        let recipient = EstablishedRecipient::<ObservedProtocol>::issued(Arc::new(vec![17]));
        let first = ObserveEstablished::new(id, recipient.clone());
        let second = ObserveEstablished::new(id, recipient.clone());
        let original_correlation = first.correlation().clone();
        let foreign_correlation = second.correlation().clone();
        let original_authority = ObservationAuthority::issued(first);
        let foreign_authority = ObservationAuthority::issued(second);
        assert!(original_authority.matches_request(&original_correlation));
        assert!(!original_authority.matches_request(&foreign_correlation));
        assert_ne!(
            original_authority.relationship(),
            foreign_authority.relationship()
        );
        assert!(Arc::ptr_eq(
            original_authority.relationship().identity(),
            &original_correlation
        ));
        assert!(Arc::ptr_eq(
            foreign_authority.relationship().identity(),
            &foreign_correlation
        ));
    }
}
