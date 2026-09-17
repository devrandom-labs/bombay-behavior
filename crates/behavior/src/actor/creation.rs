//! Staged fresh-actor creation capabilities.

use core::future::Future;
use core::marker::PhantomData;
use core::num::NonZeroU64;

use super::addressing::{Address, EndpointAddress, EstablishedActor, EstablishedRecipient};
use crate::next::Never;
use crate::{
    ActionItem, Actions, Behavior, BehaviorAddr, BehaviorBase, ItemSettlement, Protocol,
    SettledItem,
};

/// Correlation for one staged creation within a static child occurrence.
///
/// This value is neither an actor address nor a runtime route. Its constructor
/// is private so only a [`CreationSequence`] can issue it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreationId(NonZeroU64);

impl CreationId {
    /// Return the occurrence-local numeric value for protocols that derive
    /// another correlation from this creation.
    #[doc(hidden)]
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Checked source of IDs that are not reused within one child occurrence.
#[derive(Debug, PartialEq, Eq)]
pub struct CreationSequence {
    next: Option<NonZeroU64>,
}

impl CreationSequence {
    /// Begin a fresh sequence for one child occurrence.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: Some(NonZeroU64::MIN),
        }
    }

    /// Issue the next ID, or return absence after the finite sequence is
    /// exhausted. Exhaustion leaves the sequence unchanged.
    pub fn issue(&mut self) -> Option<CreationId> {
        let value = self.next?;
        self.next = value.checked_add(1);
        Some(CreationId(value))
    }
}

impl Default for CreationSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// Behavior-owned provenance for a staged fresh actor creation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationKind {
    /// An initial or ordinary later birth.
    Birth,
    /// A fresh successor requested by a replacement protocol.
    Replacement {
        /// Creator-local ID of the exact child this request supersedes.
        previous: CreationId,
    },
}

impl CreationKind {
    #[must_use]
    pub const fn replacement(previous: CreationId) -> Self {
        Self::Replacement { previous }
    }
}

/// A staged request to establish one fresh child.
///
/// The ID is correlation within the request's static child occurrence, not an
/// address, route, actor identity, or proof of freshness. The kind is
/// Behavior-owned intent. Replacement at an existing address is deliberately
/// absent; stable identity is derived with a proxy actor.
///
/// ```compile_fail
/// let mut creations = behavior::CreationSequence::new();
/// let id = creations.issue().expect("the first child ID exists");
/// let _ = behavior::CreateChild::<behavior::MailAddr, ()>::new(
///     id,
///     (),
///     behavior::CreationKind::Birth,
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateChild<A: Address, New> {
    id: CreationId,
    child: New,
    kind: CreationKind,
    address: PhantomData<fn() -> A>,
}

impl<A: Address, New> CreateChild<A, New> {
    #[must_use]
    pub(crate) const fn from_parts(id: CreationId, child: New, kind: CreationKind) -> Self {
        Self {
            id,
            child,
            kind,
            address: PhantomData,
        }
    }

    #[must_use]
    pub const fn birth(id: CreationId, child: New) -> Self {
        Self::from_parts(id, child, CreationKind::Birth)
    }

    #[must_use]
    pub const fn replacement(id: CreationId, previous: CreationId, child: New) -> Self {
        Self::from_parts(id, child, CreationKind::replacement(previous))
    }

    #[must_use]
    pub const fn id(&self) -> CreationId {
        self.id
    }

    #[must_use]
    pub const fn kind(&self) -> CreationKind {
        self.kind
    }

    #[must_use]
    pub const fn child(&self) -> &New {
        &self.child
    }

    #[doc(hidden)]
    #[must_use]
    pub fn into_parts(self) -> (CreationId, New, CreationKind) {
        (self.id, self.child, self.kind)
    }
}

/// One ordered creation batch.
///
/// The batch is the all-or-none unit for runtime route preparation. After
/// routing succeeds, each child still settles independently in this declared
/// order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Creations<Item> {
    items: Vec<Item>,
}

impl<Item> Creations<Item> {
    /// Construct an empty creation batch.
    #[must_use]
    pub const fn empty() -> Self {
        Self { items: Vec::new() }
    }

    #[must_use]
    pub fn one(item: Item) -> Self {
        Self { items: vec![item] }
    }

    #[must_use]
    pub fn and(mut self, item: Item) -> Self {
        self.items.push(item);
        self
    }

    /// Transform every item while preserving declared order and cardinality.
    #[must_use]
    pub fn map<Mapped>(self, mut map: impl FnMut(Item) -> Mapped) -> Creations<Mapped> {
        Creations::from_items(self.items.into_iter().map(&mut map).collect())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub(crate) fn from_items(items: Vec<Item>) -> Self {
        Self { items }
    }

    pub fn iter(&self) -> core::slice::Iter<'_, Item> {
        self.items.iter()
    }
}

impl<Item> Default for Creations<Item> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<Item> Extend<Item> for Creations<Item> {
    fn extend<Items: IntoIterator<Item = Item>>(&mut self, items: Items) {
        self.items.extend(items);
    }
}

impl<Item> FromIterator<Item> for Creations<Item> {
    fn from_iter<Items: IntoIterator<Item = Item>>(items: Items) -> Self {
        Self::from_items(items.into_iter().collect())
    }
}

impl<'a, Item> IntoIterator for &'a Creations<Item> {
    type Item = &'a Item;
    type IntoIter = core::slice::Iter<'a, Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<Item> IntoIterator for Creations<Item> {
    type Item = Item;
    type IntoIter = std::vec::IntoIter<Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

/// Static proof that `Role` names one exact direct child of `Parent`.
///
/// Behavior authoring owns this relationship. A runtime may use the proof to
/// build application topology, but the role itself allocates nothing and is
/// not evidence that the child was created or installed.
pub trait ChildRole<Parent: Behavior> {
    /// The only child behavior accepted at this role.
    type Child: Behavior;

    /// Structural position of this role in `Parent`'s closed child sum.
    type Position: ChildPosition<<Parent::Birth as BirthMode>::Child, Self::Child>;
}

/// Declares how one effect occurrence is resolved from an authored parent.
///
/// This is topology metadata, not actor identity or a runtime capability.
/// Generated nominal roles implement it with their declared parent, child,
/// and structural position. [`ChildHead`] and [`ChildTail`] implement it as
/// raw structural positions. Consumers normally use
/// [`ResolveChildOccurrence`] rather than inspecting `Resolution`.
///
/// Manually authored roles may implement this trait as the power-user path by
/// selecting [`DeclaredChildOccurrence`] with the same relationship expressed
/// by their [`ChildRole`] implementation. The actual resolution contract is
/// sealed, so downstream code cannot redefine wrapper transparency or replace
/// structural resolution with a runtime lookup.
pub trait ChildOccurrence<Parent: Behavior>: Sized {
    /// Sealed descriptor interpreted by [`ResolveChildOccurrence`].
    #[doc(hidden)]
    type Resolution: ChildOccurrenceResolution<Parent, Self>;
}

/// Sealed descriptor for one nominal child occurrence declared by `Parent`.
///
/// This type exists so generated and manually authored roles can carry their
/// static declaration into the sealed resolver. It has no values or runtime
/// behavior.
#[doc(hidden)]
pub struct DeclaredChildOccurrence;

/// Sealed descriptor for a raw structural child position.
///
/// This type has no values or runtime behavior.
#[doc(hidden)]
pub struct StructuralChildOccurrence<Position>(PhantomData<fn() -> Position>);

/// Resolve an effect's nominal or structural occurrence against the concrete
/// behavior currently being interpreted.
///
/// This is a sealed, type-level derived construction. It performs no lookup,
/// allocates no actor, and introduces no second identity: the resolved child's
/// [`Behavior::Protocol`] remains canonical identity, while `Position` is only
/// navigation evidence into the emitter's direct birth algebra.
///
/// A nominal role follows [`BehaviorBase`] through a wrapper only when the
/// wrapper preserves the exact protocol and the role's declared child at its
/// exact structural position. A wrapper may append births after that position,
/// but it cannot replace, reorder, or insert births before it. Raw
/// [`ChildHead`] and [`ChildTail`] positions instead resolve directly against
/// the running emitter's own birth algebra.
///
/// A wrapper that replaces its base role's child cannot silently reuse that
/// role:
///
/// ```compile_fail
/// use behavior::{
///     Actions, Behavior, BehaviorActed, BehaviorBase, Births, ChildHead,
///     ChildOccurrence, ChildRole, DeclaredChildOccurrence, MailAddr, Never,
///     NoBirths, NoSends, Protocol, ResolveChildOccurrence,
/// };
///
/// struct ActorProtocol;
/// impl Protocol for ActorProtocol {
///     type Addr = MailAddr;
///     type Msg = Never;
/// }
///
/// macro_rules! inert {
///     ($actor:ident, $birth:ty) => {
///         struct $actor;
///         impl Behavior for $actor {
///             type Protocol = ActorProtocol;
///             type Event = Never;
///             type Sends = NoSends;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = $birth;
///             fn transition(
///                 &mut self,
///                 _: behavior::ActiveTurn,
///                 event: Never,
///             ) -> BehaviorActed<Self> {
///                 match event {}
///             }
///         }
///     };
/// }
/// inert!(Child, NoBirths);
/// inert!(Proxy, NoBirths);
/// inert!(Parent, Births<Child>);
/// inert!(ChangedTopology, Births<Proxy>);
///
/// impl BehaviorBase for Parent {
///     type Base = Self;
///     fn base(&self) -> &Self { self }
/// }
/// impl BehaviorBase for ChangedTopology {
///     type Base = Parent;
///     fn base(&self) -> &Parent { unreachable!() }
/// }
///
/// struct WorkerRole;
/// impl ChildRole<Parent> for WorkerRole {
///     type Child = Child;
///     type Position = ChildHead;
/// }
/// impl ChildOccurrence<Parent> for WorkerRole {
///     type Resolution = DeclaredChildOccurrence;
/// }
///
/// fn require<T: ResolveChildOccurrence<WorkerRole>>() {}
/// require::<ChangedTopology>();
/// ```
pub trait ResolveChildOccurrence<Occurrence>:
    Behavior + sealed::ResolveChildOccurrence<Occurrence>
{
    /// Exact concrete child behavior at this occurrence.
    type Child: Behavior;

    /// Exact structural position in this emitter's direct birth algebra.
    type Position: ChildPosition<<Self::Birth as BirthMode>::Child, Self::Child>;
}

impl<Emitter, Occurrence> ResolveChildOccurrence<Occurrence> for Emitter
where
    Emitter: Behavior + BehaviorBase + sealed::ResolveChildOccurrence<Occurrence>,
    Emitter::Base: Behavior,
    Occurrence: ChildOccurrence<Emitter::Base>,
    Occurrence::Resolution: ResolveChildOccurrenceDescriptor<Emitter, Occurrence>,
{
    type Child =
        <Occurrence::Resolution as ResolveChildOccurrenceDescriptor<Emitter, Occurrence>>::Child;
    type Position =
        <Occurrence::Resolution as ResolveChildOccurrenceDescriptor<Emitter, Occurrence>>::Position;
}

/// Child behavior resolved from `Occurrence` for the running `Emitter`.
pub type ResolvedChild<Emitter, Occurrence> =
    <Emitter as ResolveChildOccurrence<Occurrence>>::Child;

/// Structural birth position resolved from `Occurrence` for the running
/// `Emitter`.
pub type ResolvedChildPosition<Emitter, Occurrence> =
    <Emitter as ResolveChildOccurrence<Occurrence>>::Position;

/// Child behavior selected by one named role.
pub type RoleChild<Parent, Role> = <Role as ChildRole<Parent>>::Child;

/// Canonical protocol selected by one named role.
pub type RoleProtocol<Parent, Role> = <RoleChild<Parent, Role> as Behavior>::Protocol;

/// One complete creation paired with a route selected by the interpreter.
///
/// Behavior never constructs or stores this value. It exists only between the
/// generic batch-routing step and the concrete child host, and it returns
/// complete on rejection or corruption.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedCreation<A: Address, New> {
    creation: CreateChild<A, New>,
    route: A::Nonce,
}

impl<A: Address, New> RoutedCreation<A, New> {
    /// Pair one creation with the route selected by the current runtime.
    #[must_use]
    pub const fn new(creation: CreateChild<A, New>, route: A::Nonce) -> Self {
        Self { creation, route }
    }

    #[must_use]
    pub const fn id(&self) -> CreationId {
        self.creation.id()
    }

    #[must_use]
    pub const fn kind(&self) -> CreationKind {
        self.creation.kind()
    }

    #[must_use]
    pub const fn route(&self) -> A::Nonce {
        self.route
    }

    #[must_use]
    pub fn into_parts(self) -> (CreateChild<A, New>, A::Nonce) {
        (self.creation, self.route)
    }
}

/// Failure to claim an address fresh with respect to the current actor
/// configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AllocationRejection {
    /// The allocator has no address it can presently claim.
    #[error("fresh actor-address allocation is exhausted")]
    Exhausted,
    /// The proposed address was already claimed; accepting it would violate
    /// actor-name freshness.
    #[error("the proposed actor address is already claimed")]
    AddressAlreadyClaimed,
}

/// Complete semantic rejection of one staged fresh creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CreationRejection {
    /// Fresh address allocation failed.
    #[error("fresh allocation failed: {0}")]
    Allocation(AllocationRejection),
    /// The child's initialization fold did not complete successfully.
    #[error("child initialization failed")]
    InitializationFailed,
    /// Installation or commit failed after allocation.
    #[error("the interpreter could not install and commit the child")]
    EnvironmentFailed,
}

/// The creator namespace cannot route an entire declared creation batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the creator child namespace is exhausted")]
pub struct ChildNamespaceExhausted;

impl<A, New> ActionItem for Creations<CreateChild<A, New>>
where
    A: Address,
    A::Nonce: Send,
    New: Send,
{
    type Accepted = Creations<RoutedCreation<A, New>>;
    type Rejection = ChildNamespaceExhausted;
    type Prerequisite = Never;
}

/// Committed or rejected result for one exact child-protocol occurrence.
///
/// `Installed` is constructed only after fresh allocation, successful
/// initialization, endpoint establishment, and creator-local binding. It returns the
/// exact protocol capability. `Rejected` carries no capability, so a
/// failed request cannot be used as an established destination. Both variants
/// preserve Behavior-authored creation provenance.
///
/// `Occurrence` is topology navigation evidence authored by the parent. It
/// distinguishes duplicate occurrences without becoming another protocol
/// identity or runtime key. `P` remains canonical identity. The concrete child
/// behavior is deliberately absent: consumers that only retain or communicate
/// with the installed protocol do not have to pretend to be parent actors.
///
/// Duplicate occurrences remain incompatible even when their protocols and
/// endpoint representations match:
///
/// ```compile_fail
/// use behavior::{
///     Address, CreationKind, CreationSequence, EndpointAddress, EstablishedCreation,
///     EstablishedRecipient, Protocol,
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
/// struct Primary;
/// struct Backup;
/// fn accepts_primary(_: EstablishedCreation<Worker, Primary>) {}
/// let mut sequence = CreationSequence::new();
/// let id = sequence.issue().expect("fixture creation ID");
/// let backup: EstablishedCreation<Worker, Backup> = EstablishedCreation::installed(
///     id,
///     CreationKind::Birth,
///     EstablishedRecipient::issued(Endpoint),
/// );
/// accepts_primary(backup);
/// ```
pub enum EstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    Installed {
        id: CreationId,
        kind: CreationKind,
        recipient: EstablishedRecipient<P>,
        occurrence: PhantomData<fn() -> Occurrence>,
    },
    Rejected {
        id: CreationId,
        kind: CreationKind,
        reason: CreationRejection,
        occurrence: PhantomData<fn() -> Occurrence>,
    },
}

impl<P, Occurrence> EstablishedCreation<P, Occurrence>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn installed(
        id: CreationId,
        kind: CreationKind,
        recipient: EstablishedRecipient<P>,
    ) -> Self {
        Self::Installed {
            id,
            kind,
            recipient,
            occurrence: PhantomData,
        }
    }

    #[must_use]
    pub const fn rejected(id: CreationId, kind: CreationKind, reason: CreationRejection) -> Self {
        Self::Rejected {
            id,
            kind,
            reason,
            occurrence: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(&self) -> CreationId {
        match self {
            Self::Installed { id, .. } | Self::Rejected { id, .. } => *id,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> CreationKind {
        match self {
            Self::Installed { kind, .. } | Self::Rejected { kind, .. } => *kind,
        }
    }

    /// Consume the report and return its exact endpoint capability.
    ///
    /// # Errors
    /// Returns the original [`CreationRejection`] when child creation did not
    /// commit.
    pub fn into_recipient(self) -> Result<EstablishedRecipient<P>, CreationRejection> {
        match self {
            Self::Installed { recipient, .. } => Ok(recipient),
            Self::Rejected { reason, .. } => Err(reason),
        }
    }

    /// Recover the concrete installed-actor proof when this occurrence is a
    /// declared role of `Parent`.
    ///
    /// `Parent` is used only as compile-time topology evidence at this
    /// capability-strengthening boundary; it is not part of the creation-report
    /// identity. Ordinary consumers can retain [`EstablishedRecipient<P>`]
    /// without carrying the parent behavior.
    ///
    /// # Errors
    /// Returns the original [`CreationRejection`] when child creation did not
    /// commit.
    pub fn into_actor<Parent>(
        self,
    ) -> Result<EstablishedActor<RoleChild<Parent, Occurrence>>, CreationRejection>
    where
        Parent: Behavior,
        Occurrence: ChildRole<Parent>,
        RoleChild<Parent, Occurrence>: Behavior<Protocol = P>,
    {
        self.into_recipient().map(EstablishedActor::from_recipient)
    }
}

/// Complete result after a runtime accepts one child definition.
///
/// A created child leaves only its exact established capability. Rejection
/// during the pure initialization transition returns the current child and its
/// exact error. Rejection while establishing the host returns the current child
/// and the still-uninterpreted initialization actions. No variant reconstructs
/// a pre-initialization value or silently discards an affine action.
pub enum ChildCreationOutcome<C, Occurrence>
where
    C: Behavior,
    BehaviorAddr<C>: EndpointAddress,
{
    /// Fresh child creation committed successfully.
    Established {
        /// Exact capability and creation provenance for this occurrence.
        established: EstablishedCreation<C::Protocol, Occurrence>,
    },
    /// The child's pure initialization transition rejected before host commit.
    InitializationRejected {
        /// Current child value with its creator correlation and private route.
        creation: RoutedCreation<BehaviorAddr<C>, C>,
        /// Exact error returned by the child's initialization transition.
        error: C::Error,
    },
    /// Host establishment rejected after initialization produced actions.
    HostRejected {
        /// Current child value with its creator correlation and private route.
        creation: RoutedCreation<BehaviorAddr<C>, C>,
        /// Complete initialization actions that were never interpreted.
        initialization: Actions<BehaviorAddr<C>, C::Ph, C::Sends, C::Birth>,
        /// Exact reason the host could not commit this child.
        reason: CreationRejection,
    },
}

impl<C, Occurrence> ChildCreationOutcome<C, Occurrence>
where
    C: Behavior,
    BehaviorAddr<C>: EndpointAddress,
{
    /// Consume a committed child result into its exact concrete actor
    /// capability.
    ///
    /// The enclosing `ChildCreationOutcome<C, Occurrence>` already proves which
    /// concrete behavior was created. A protocol recipient is therefore
    /// strengthened only to `EstablishedActor<C>`; callers cannot select a
    /// different behavior sharing the same protocol.
    ///
    /// # Errors
    ///
    /// Returns the complete original result when creation did not commit.
    pub fn into_actor(self) -> Result<EstablishedActor<C>, Self> {
        match self {
            Self::Established {
                established: EstablishedCreation::Installed { recipient, .. },
            } => Ok(EstablishedActor::from_recipient(recipient)),
            other => Err(other),
        }
    }
}

/// One complete child-creation settlement returned to its creator.
///
/// This product moves the existing settlement without reclassifying it. In
/// particular, initialization and host rejection retain the current child and
/// exact initialization value. A runtime that cannot admit this value to the
/// live creator must recover it from the returned event and keep it in host
/// custody.
#[must_use = "a child-creation settlement must be admitted or retained"]
pub struct ChildCreationSettled<C, Occurrence>
where
    C: Behavior,
    BehaviorAddr<C>: EndpointAddress,
{
    settlement: SettledItem<
        RoutedCreation<BehaviorAddr<C>, C>,
        ItemSettlement<
            RoutedCreation<BehaviorAddr<C>, C>,
            ChildCreationOutcome<C, Occurrence>,
            CreationRejection,
            Never,
        >,
    >,
}

impl<C, Occurrence> ChildCreationSettled<C, Occurrence>
where
    C: Behavior,
    BehaviorAddr<C>: EndpointAddress,
{
    #[must_use]
    pub const fn new(
        settlement: SettledItem<
            RoutedCreation<BehaviorAddr<C>, C>,
            ItemSettlement<
                RoutedCreation<BehaviorAddr<C>, C>,
                ChildCreationOutcome<C, Occurrence>,
                CreationRejection,
                Never,
            >,
        >,
    ) -> Self {
        Self { settlement }
    }

    /// Recover the exact generic settlement without cloning any owned value.
    #[must_use]
    pub fn into_settlement(
        self,
    ) -> SettledItem<
        RoutedCreation<BehaviorAddr<C>, C>,
        ItemSettlement<
            RoutedCreation<BehaviorAddr<C>, C>,
            ChildCreationOutcome<C, Occurrence>,
            CreationRejection,
            Never,
        >,
    > {
        self.settlement
    }
}

impl<C, Occurrence> core::fmt::Debug for ChildCreationSettled<C, Occurrence>
where
    C: Behavior,
    BehaviorAddr<C>: EndpointAddress,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ChildCreationSettled")
            .finish_non_exhaustive()
    }
}

/// Non-authoritative correlation to one creation result in the current action.
///
/// `P` and `Occurrence` select the exact creator-local entry statically. The
/// value carries no recipient, actor identity, or child-hosting authority; the
/// corresponding creation settlement remains its sole authoritative owner.
///
/// Equal ID representations at different occurrences cannot be
/// substituted:
///
/// ```compile_fail,E0308
/// struct Worker;
/// impl behavior::Protocol for Worker {
///     type Addr = behavior::MailAddr;
///     type Msg = ();
/// }
/// struct Primary;
/// struct Backup;
/// let mut sequence = behavior::CreationSequence::new();
/// let id = sequence.issue().expect("fixture creation ID");
/// let backup = behavior::CreationCorrelation::<Worker, Backup>::new(id);
/// let _: behavior::CreationCorrelation<Worker, Primary> = backup;
/// ```
#[doc(hidden)]
pub struct CreationCorrelation<P, Occurrence>
where
    P: Protocol,
{
    id: CreationId,
    occurrence: PhantomData<fn() -> (P, Occurrence)>,
}

impl<P, Occurrence> CreationCorrelation<P, Occurrence>
where
    P: Protocol,
{
    #[must_use]
    pub const fn new(id: CreationId) -> Self {
        Self {
            id,
            occurrence: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(self) -> CreationId {
        self.id
    }
}

impl<P, Occurrence> Copy for CreationCorrelation<P, Occurrence> where P: Protocol {}

impl<P, Occurrence> Clone for CreationCorrelation<P, Occurrence>
where
    P: Protocol,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<P, Occurrence> core::fmt::Debug for CreationCorrelation<P, Occurrence>
where
    P: Protocol,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("CreationCorrelation")
            .field(&self.id)
            .finish()
    }
}

impl<P, Occurrence> PartialEq for CreationCorrelation<P, Occurrence>
where
    P: Protocol,
{
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<P, Occurrence> Eq for CreationCorrelation<P, Occurrence> where P: Protocol {}

/// Same-action communication to one declared creator-local role.
///
/// The interpreter resolves this route only after all creations in the same
/// [`crate::Actions`] have committed. A rejected or absent binding must produce
/// a typed interpreter outcome; it can never be converted into a logical
/// address by nonce arithmetic.
pub struct ChildDelivery<P, Occurrence>
where
    P: Protocol,
{
    pub creation: CreationId,
    pub message: P::Msg,
    occurrence: PhantomData<fn() -> Occurrence>,
}

/// Private typed communication to one declared creator-local child role.
///
/// `ChildDelivery` addresses the child's public protocol. `ChildInput`
/// instead selects one owner-defined member of the concrete child's event
/// algebra through `Source`. This is the static boundary used for lifecycle
/// coordination between an owner and a composed child: it retains the exact
/// child behavior, occurrence, input, and ingress owner without exposing the
/// input through the child's public protocol or performing a runtime lookup.
///
/// This is a derived Bombay communication form. Like `ChildDelivery`, its
/// creator-local route is interpreted only after same-action creations have
/// committed; constructing it performs no delivery.
pub struct ChildInput<Child, Source, Input, Occurrence>
where
    Child: Behavior,
{
    /// Creator-local creation ID of the concrete child receiving the input.
    pub creation: CreationId,
    /// Complete private input transferred to the child.
    pub input: Input,
    marker: PhantomData<fn() -> (Child, Source, Occurrence)>,
}

/// Exact reason one public child delivery was not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildDeliveryReason {
    MissingBinding,
    ClosedRecipient,
}

/// Exact reason one private child input was not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChildInputReason {
    MissingBinding,
    ClosedControlLane,
}

/// One report emitted through an established creator/child relationship.
///
/// The interpreter attaches `child` from its exact local binding; the
/// emitting behavior supplies only `report`. `EventIngress` separately keeps
/// the concrete child behavior and occurrence in the parent's event type, so
/// equal nonce representations cannot confuse different child roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildReport<R> {
    /// Creator-local ID of the child that emitted the report.
    pub child: CreationId,
    /// Complete report value transferred by that child.
    pub report: R,
}

impl<R> ChildReport<R> {
    /// Attach an established creator-local creation ID to one report.
    #[must_use]
    pub const fn new(child: CreationId, report: R) -> Self {
        Self { child, report }
    }
}

impl<R> From<(CreationId, crate::ReportToParent<R>)> for ChildReport<R> {
    fn from((child, request): (CreationId, crate::ReportToParent<R>)) -> Self {
        Self::new(child, request.into_inner())
    }
}

impl<Child, Source, Input, Occurrence> ChildInput<Child, Source, Input, Occurrence>
where
    Child: Behavior,
    Child::Event: crate::ChildInputIngress<Source, Input>,
{
    /// Construct a private input for one exact creator-local child creation.
    #[must_use]
    pub const fn after(creation: CreationId, input: Input) -> Self {
        Self {
            creation,
            input,
            marker: PhantomData,
        }
    }
}

impl<Child, Source, Input, Occurrence> Clone for ChildInput<Child, Source, Input, Occurrence>
where
    Child: Behavior,
    Input: Clone,
{
    fn clone(&self) -> Self {
        Self {
            creation: self.creation,
            input: self.input.clone(),
            marker: PhantomData,
        }
    }
}

impl<P, Occurrence> ChildDelivery<P, Occurrence>
where
    P: Protocol,
{
    #[must_use]
    pub const fn after(creation: CreationId, message: P::Msg) -> Self {
        Self {
            creation,
            message,
            occurrence: PhantomData,
        }
    }
}

impl<P, Occurrence> Clone for ChildDelivery<P, Occurrence>
where
    P: Protocol,
    P::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self {
            creation: self.creation,
            message: self.message.clone(),
            occurrence: PhantomData,
        }
    }
}

impl<P, Occurrence> ActionItem for ChildDelivery<P, Occurrence>
where
    P: Protocol,
    P::Msg: Send,
{
    type Accepted = ();
    type Rejection = ChildDeliveryReason;
    type Prerequisite = CreationCorrelation<P, Occurrence>;
}

impl<Child, Source, Input, Occurrence> ActionItem for ChildInput<Child, Source, Input, Occurrence>
where
    Child: Behavior,
    Input: Send,
{
    type Accepted = ();
    type Rejection = ChildInputReason;
    type Prerequisite = CreationCorrelation<Child::Protocol, Occurrence>;
}

/// Runtime ownership port for establishing one concrete child behavior.
///
/// The result vocabulary belongs to the creation request, never the runtime.
/// Accepting ownership consumes the child definition and returns its one
/// authoritative created-or-rejected result. Refusal before ownership transfer
/// returns the complete request. Child creation has no prerequisite, while
/// interpreter corruption uses the shared [`crate::InterpreterFault`] sum.
///
/// A runtime cannot substitute a private receipt or rejection type:
///
/// ```compile_fail,E0053
/// #[derive(Clone, Copy, Eq, PartialEq)]
/// struct RuntimeAddr;
/// impl behavior::Address for RuntimeAddr { type Nonce = u8; }
/// #[derive(Clone)]
/// struct Endpoint;
/// impl behavior::EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
/// }
/// struct Child;
/// impl behavior::Protocol for Child {
///     type Addr = RuntimeAddr;
///     type Msg = behavior::Never;
/// }
/// impl behavior::Behavior for Child {
///     type Protocol = Self;
///     type Event = behavior::User<RuntimeAddr, behavior::Never>;
///     type Sends = behavior::NoSends;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(
///         &mut self,
///         _: behavior::ActiveTurn,
///         event: Self::Event,
///     ) -> behavior::BehaviorActed<Self> {
///         match event.message {}
///     }
/// }
/// struct Runtime;
///
/// impl behavior::EstablishChild<behavior::ChildHead, Child> for Runtime {
///     fn establish_child(
///         &mut self,
///         creation: behavior::RoutedCreation<behavior::BehaviorAddr<Child>, Child>,
///     ) -> impl core::future::Future<Output = behavior::ItemSettlement<
///         behavior::RoutedCreation<behavior::BehaviorAddr<Child>, Child>,
///         (),
///         &'static str,
///         behavior::Never,
///     >> + Send {
///         async move { behavior::ItemSettlement::Rejected { item: creation, reason: "no" } }
///     }
/// }
/// ```
pub trait EstablishChild<Occurrence, C>
where
    C: Behavior,
    BehaviorAddr<C>: EndpointAddress,
{
    /// Establish exactly the supplied child or return its complete settlement.
    fn establish_child(
        &mut self,
        creation: RoutedCreation<BehaviorAddr<C>, C>,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<BehaviorAddr<C>, C>,
            ChildCreationOutcome<C, Occurrence>,
            CreationRejection,
            Never,
        >,
    > + Send;
}

/// Authoritative result shape for one closed child choice.
///
/// This type-level product depends only on the declared child protocols and
/// their occurrences. It contains no runtime-selected type.
#[doc(hidden)]
pub trait ChildCreationProduct<A: Address, Occurrence>: Sized {
    type Result;
}

/// Exhaustive static dispatch of one creation-only child sum.
///
/// This is an interpreter-facing derived construction, not another actor-model
/// operation. Implementations must preserve the ID, route, and creation kind and
/// call exactly one concrete [`EstablishChild`] implementation. [`ChildChoice`]
/// provides the closed recursive heterogeneous sum. Dispatch futures are
/// sendable; heterogeneous sums therefore require sendable alternatives,
/// runtime routes, and child hosts.
pub trait DispatchBirth<A: Address, Host>: ChildCreationProduct<A, ChildHead> + Sized {
    /// Select exactly one concrete child host while preserving creation data
    /// in every non-accepted settlement.
    fn dispatch_birth(
        self,
        id: CreationId,
        route: A::Nonce,
        kind: CreationKind,
        host: &mut Host,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<A, Self>,
            <Self as ChildCreationProduct<A, ChildHead>>::Result,
            CreationRejection,
            Never,
        >,
    > + Send;
}

trait CreateSelectedChild<A: Address, Occurrence, Host>:
    ChildCreationProduct<A, Occurrence> + Sized
{
    fn dispatch_birth_at(
        self,
        id: CreationId,
        route: A::Nonce,
        kind: CreationKind,
        host: &mut Host,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<A, Self>,
            <Self as ChildCreationProduct<A, Occurrence>>::Result,
            CreationRejection,
            Never,
        >,
    > + Send;
}

impl<A, Child, Host> DispatchBirth<A, Host> for Child
where
    A: Address,
    Child: CreateSelectedChild<A, ChildHead, Host>,
{
    fn dispatch_birth(
        self,
        id: CreationId,
        route: A::Nonce,
        kind: CreationKind,
        host: &mut Host,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<A, Self>,
            <Self as ChildCreationProduct<A, ChildHead>>::Result,
            CreationRejection,
            Never,
        >,
    > + Send {
        self.dispatch_birth_at(id, route, kind, host)
    }
}

/// One alternative in a closed, recursively composed child-creation sum.
///
/// `Head` is one concrete child behavior and `Tail` is the remaining closed
/// sum. This is a creation choice only: it is not a behavior, message
/// envelope, registry, or runtime dispatch mechanism.
///
/// Every alternative requires a concrete child host; incomplete interpreter
/// support is rejected statically:
///
/// ```compile_fail
/// #[derive(Clone, Copy, Eq, PartialEq)]
/// struct RuntimeAddr;
/// impl behavior::Address for RuntimeAddr { type Nonce = u8; }
/// #[derive(Clone)]
/// struct Endpoint;
/// impl behavior::EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
/// }
/// struct CacheWorker;
/// struct QueueWorker;
///
/// macro_rules! inert {
///     ($child:ty) => {
///         impl behavior::Protocol for $child {
///             type Addr = RuntimeAddr;
///             type Msg = behavior::Never;
///         }
///         impl behavior::Behavior for $child {
///             type Protocol = Self;
///             type Event = behavior::User<RuntimeAddr, behavior::Never>;
///             type Sends = behavior::NoSends;
///             type Ph = behavior::Never;
///             type Error = behavior::Never;
///             type Birth = behavior::NoBirths;
///
///             fn transition(
///                 &mut self,
///                 _: behavior::ActiveTurn,
///                 event: Self::Event,
///             ) -> behavior::BehaviorActed<Self> {
///                 match event.message {}
///             }
///         }
///     };
/// }
/// inert!(CacheWorker);
/// inert!(QueueWorker);
///
/// struct Incomplete;
/// impl behavior::EstablishChild<behavior::ChildHead, CacheWorker> for Incomplete {
///     async fn establish_child(
///         &mut self,
///         creation: behavior::RoutedCreation<RuntimeAddr, CacheWorker>,
///     ) -> behavior::ItemSettlement<
///         behavior::RoutedCreation<RuntimeAddr, CacheWorker>,
///         behavior::ChildCreationOutcome<CacheWorker, behavior::ChildHead>,
///         behavior::CreationRejection,
///         behavior::Never,
///     > {
///         behavior::ItemSettlement::Rejected {
///             item: creation,
///             reason: behavior::CreationRejection::EnvironmentFailed,
///         }
///     }
/// }
///
/// type WorkerChoices = behavior::ChildChoice<
///     CacheWorker,
///     behavior::ChildChoice<QueueWorker, behavior::Never>,
/// >;
/// fn require_complete<T: behavior::DispatchBirth<RuntimeAddr, Incomplete>>() {}
/// require_complete::<WorkerChoices>();
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildChoice<Head, Tail> {
    /// Select the concrete child at this position.
    Head(Head),
    /// Select one concrete child from the remaining alternatives.
    Tail(Tail),
}

/// Position selecting the head of a closed [`ChildChoice`] sum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChildHead;

/// Position selecting inside the tail of a closed [`ChildChoice`] sum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChildTail<Position>(PhantomData<fn() -> Position>);

/// Static proof that `Child` occupies `Position` in a closed child sum.
///
/// This trait provides structural evidence only. It does not construct a
/// choice, perform creation, or select a child through runtime inspection.
/// A role cannot claim a position occupied by a different child:
///
/// ```compile_fail
/// use behavior::{
///     Actions, Behavior, BehaviorActed, Births, ChildChoice, ChildHead,
///     ChildRole, MailAddr, Never, NoBirths, User,
/// };
/// struct CacheWorker;
/// struct QueueWorker;
/// struct Parent;
/// macro_rules! inert {
///     ($actor:ty) => {
///         impl behavior::Protocol for $actor {
///             type Addr = MailAddr;
///             type Msg = Never;
///         }
///         impl Behavior for $actor {
///             type Protocol = Self;
///             type Event = User<MailAddr, Never>;
///             type Sends = Vec<Never>;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = NoBirths;
///             fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
///                 match event.message {}
///             }
///         }
///     };
/// }
/// inert!(CacheWorker);
/// inert!(QueueWorker);
/// impl behavior::Protocol for Parent {
///     type Addr = MailAddr;
///     type Msg = Never;
/// }
/// impl Behavior for Parent {
///     type Protocol = Self;
///     type Event = User<MailAddr, Never>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = Births<ChildChoice<QueueWorker, ChildChoice<CacheWorker, Never>>>;
///     fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
///         match event.message {}
///     }
/// }
/// struct ForgedCacheRole;
/// impl ChildRole<Parent> for ForgedCacheRole {
///     type Child = CacheWorker;
///     type Position = ChildHead;
/// }
/// ```
pub trait ChildPosition<Children, Child: Behavior>: sealed::ChildPosition {}

impl sealed::ChildPosition for ChildHead {}

impl<Child: Behavior> ChildPosition<Child, Child> for ChildHead {}

impl<Head: Behavior, Tail> ChildPosition<ChildChoice<Head, Tail>, Head> for ChildHead {}

impl<Position> sealed::ChildPosition for ChildTail<Position> {}

impl<Head, Tail, Position, Child> ChildPosition<ChildChoice<Head, Tail>, Child>
    for ChildTail<Position>
where
    Child: Behavior,
    Position: ChildPosition<Tail, Child>,
{
}

/// Append one closed direct-child algebra after another.
///
/// `Self` remains the structural prefix, so every child occurrence already
/// valid in that prefix retains both its child type and its position. `Tail`
/// begins only after the prefix's final position. The two injection functions
/// change only the closed sum containing a child; [`append_creations`](Self::append_creations)
/// additionally preserves every creation's ID, kind, and batch order while
/// placing all prefix creations before all appended creations.
///
/// This is a static composition of Bombay's existing staged-creation
/// capability, not another actor effect and not an allocation operation. The
/// interpreter remains solely responsible for establishing and binding the child.
///
/// A generic topology owner can therefore retain an inner behavior's exact
/// creation effects and append children whose concrete types were inferred
/// from value construction:
///
/// ```
/// use behavior::{BirthNodeAppend, Never};
/// use core::marker::PhantomData;
///
/// type Combined = <Never as BirthNodeAppend<Never>>::Output;
/// let _: PhantomData<Combined> = PhantomData;
/// ```
pub trait BirthNodeAppend<Tail>: sealed::BirthNode + Sized
where
    Tail: sealed::BirthNode,
{
    /// Closed child algebra containing the complete prefix followed by the
    /// complete appended tail.
    type Output: sealed::BirthNode;

    /// Inject one child from the existing prefix without changing its
    /// structural occurrence.
    fn append_prefix(self) -> Self::Output;

    /// Inject one child from the appended tail after every prefix occurrence.
    fn append_tail(tail: Tail) -> Self::Output;

    /// Preserve and concatenate two ordered creation batches.
    #[must_use]
    fn append_creations<A: Address>(
        prefix: Creations<CreateChild<A, Self>>,
        tail: Creations<CreateChild<A, Tail>>,
    ) -> Creations<CreateChild<A, Self::Output>> {
        let mut combined = Vec::with_capacity(prefix.len() + tail.len());
        combined.extend(prefix.into_iter().map(|creation| {
            let (id, child, kind) = creation.into_parts();
            CreateChild::from_parts(id, Self::append_prefix(child), kind)
        }));
        combined.extend(tail.into_iter().map(|creation| {
            let (id, child, kind) = creation.into_parts();
            CreateChild::from_parts(id, Self::append_tail(child), kind)
        }));
        Creations::from_items(combined)
    }
}

impl<Tail> BirthNodeAppend<Tail> for Never
where
    Tail: sealed::BirthNode,
{
    type Output = Tail;

    fn append_prefix(self) -> Self::Output {
        match self {}
    }

    fn append_tail(tail: Tail) -> Self::Output {
        tail
    }
}

impl<Node> BirthNodeAppend<Never> for Node
where
    Node: sealed::NonEmptyBirthNode,
{
    type Output = Node;

    fn append_prefix(self) -> Self::Output {
        self
    }

    fn append_tail(tail: Never) -> Self::Output {
        match tail {}
    }
}

impl<Child, Tail> BirthNodeAppend<Tail> for Child
where
    Child: Behavior,
    Tail: sealed::NonEmptyBirthNode,
{
    type Output = ChildChoice<Child, Tail>;

    fn append_prefix(self) -> Self::Output {
        ChildChoice::Head(self)
    }

    fn append_tail(tail: Tail) -> Self::Output {
        ChildChoice::Tail(tail)
    }
}

impl<Head, Rest, Tail> BirthNodeAppend<Tail> for ChildChoice<Head, Rest>
where
    Head: Behavior,
    Rest: sealed::BirthNode + BirthNodeAppend<Tail>,
    Tail: sealed::NonEmptyBirthNode,
    Rest::Output: sealed::BirthNode,
{
    type Output = ChildChoice<Head, Rest::Output>;

    fn append_prefix(self) -> Self::Output {
        match self {
            ChildChoice::Head(head) => ChildChoice::Head(head),
            ChildChoice::Tail(rest) => ChildChoice::Tail(rest.append_prefix()),
        }
    }

    fn append_tail(tail: Tail) -> Self::Output {
        ChildChoice::Tail(Rest::append_tail(tail))
    }
}

impl<Parent: Behavior> ChildOccurrence<Parent> for ChildHead {
    type Resolution = StructuralChildOccurrence<Self>;
}

impl<Parent: Behavior, Position> ChildOccurrence<Parent> for ChildTail<Position> {
    type Resolution = StructuralChildOccurrence<Self>;
}

/// Sealed proof that an occurrence may select one resolver descriptor.
///
/// Nominal occurrences can select [`DeclaredChildOccurrence`] only when their
/// existing [`ChildRole`] implementation supplies the child and position. Raw
/// structural descriptors are available only to the identical
/// [`ChildHead`] or [`ChildTail`] occurrence.
#[doc(hidden)]
pub trait ChildOccurrenceResolution<Parent: Behavior, Occurrence>:
    sealed::ChildOccurrenceDescriptor + sealed::OccurrenceResolution<Parent, Occurrence>
{
}

impl<Parent, Occurrence> ChildOccurrenceResolution<Parent, Occurrence> for DeclaredChildOccurrence
where
    Parent: Behavior,
    Occurrence: ChildRole<Parent>,
{
}

impl<Parent: Behavior> ChildOccurrenceResolution<Parent, ChildHead>
    for StructuralChildOccurrence<ChildHead>
{
}

impl<Parent: Behavior, Position> ChildOccurrenceResolution<Parent, ChildTail<Position>>
    for StructuralChildOccurrence<ChildTail<Position>>
{
}

/// Sealed implementation detail for resolving occurrence descriptors.
#[doc(hidden)]
pub trait ResolveChildOccurrenceDescriptor<Emitter: Behavior, Occurrence>:
    sealed::ChildOccurrenceDescriptor + sealed::ResolveDescriptor<Emitter, Occurrence>
{
    type Child: Behavior;
    type Position: ChildPosition<<Emitter::Birth as BirthMode>::Child, Self::Child>;
}

impl<Emitter, Occurrence, Position> ResolveChildOccurrenceDescriptor<Emitter, Occurrence>
    for StructuralChildOccurrence<Position>
where
    Emitter: Behavior,
    <Emitter::Birth as BirthMode>::Child: BirthNodeAt<Position>,
    Position: ChildPosition<
            <Emitter::Birth as BirthMode>::Child,
            <<Emitter::Birth as BirthMode>::Child as BirthNodeAt<Position>>::Child,
        >,
{
    type Child = <<Emitter::Birth as BirthMode>::Child as BirthNodeAt<Position>>::Child;
    type Position = Position;
}

impl<Emitter, Occurrence> ResolveChildOccurrenceDescriptor<Emitter, Occurrence>
    for DeclaredChildOccurrence
where
    Emitter:
        Behavior<Protocol = <<Emitter as BehaviorBase>::Base as Behavior>::Protocol> + BehaviorBase,
    Emitter::Base: Behavior,
    Occurrence: ChildRole<Emitter::Base>,
    <Emitter::Birth as BirthMode>::Child:
        BirthNodeAt<Occurrence::Position, Child = Occurrence::Child>,
    Occurrence::Position: ChildPosition<<Emitter::Birth as BirthMode>::Child, Occurrence::Child>,
{
    type Child = Occurrence::Child;
    type Position = Occurrence::Position;
}

/// Sealed inverse projection from one structural position to its child.
#[doc(hidden)]
pub trait BirthNodeAt<Position>: sealed::BirthNode {
    type Child: Behavior;
}

impl<Child: Behavior> BirthNodeAt<ChildHead> for Child {
    type Child = Child;
}

impl<Head: Behavior, Tail> BirthNodeAt<ChildHead> for ChildChoice<Head, Tail>
where
    Tail: sealed::BirthNode,
{
    type Child = Head;
}

impl<Head, Tail, Position> BirthNodeAt<ChildTail<Position>> for ChildChoice<Head, Tail>
where
    Head: Behavior,
    Tail: BirthNodeAt<Position>,
{
    type Child = <Tail as BirthNodeAt<Position>>::Child;
}

/// Downstream shape of one direct-child occurrence product.
///
/// Behavior owns the closed node algebra: a concrete [`Behavior`] leaf,
/// [`ChildChoice`], or [`Never`]. A runtime owns the representation associated
/// with each leaf. `Empty` supplies its terminal representation and `Member`
/// receives one concrete child, its structural occurrence, and the remaining
/// child product.
///
/// This is a type-level derived construction. It creates no value, allocates
/// no actor, interprets no effect, and introduces no protocol identity or
/// runtime key. A shape that builds a heterogeneous product should retain
/// `Tail`; the product includes every declared child exactly once.
///
/// ```
/// use core::marker::PhantomData;
/// use behavior::{Behavior, ChildOccurrenceShape, ChildOccurrences};
///
/// struct NoChildBindings;
/// struct ChildBinding<Position, Child, Tail>(PhantomData<fn() -> (Position, Child, Tail)>);
/// struct RuntimeStorage;
///
/// impl ChildOccurrenceShape for RuntimeStorage {
///     type Empty = NoChildBindings;
///     type Member<Occurrence, Child: Behavior, Tail> =
///         ChildBinding<Occurrence, Child, Tail>;
/// }
///
/// type ChildBindings<Node> = ChildOccurrences<Node, RuntimeStorage>;
/// ```
pub trait ChildOccurrenceShape {
    /// Representation of the empty [`Never`] node.
    type Empty;

    /// Representation of one concrete child followed by the remaining product.
    type Member<Occurrence, Child: Behavior, Tail>;
}

/// Sealed occurrence product of a closed direct-child birth node.
///
/// The product starts at [`ChildHead`] and advances through
/// [`ChildTail<Position>`] in exactly the same way as [`DispatchBirth`] and
/// [`ChildPosition`]. A downstream runtime selects only the result shape
/// through [`ChildOccurrenceShape`]; it cannot reclassify a foreign type as a
/// birth node or replace the recursion.
///
/// This product is intentionally direct rather than transitive. Each installed
/// actor owns the bindings for its own `Behavior::Birth`; when a concrete child
/// is installed, the same product law applies to that child's birth algebra.
///
/// Foreign types cannot extend the closed node algebra:
///
/// ```compile_fail
/// use behavior::{ChildOccurrenceProduct, ChildOccurrenceShape};
///
/// struct RuntimeShape;
/// impl ChildOccurrenceShape for RuntimeShape {
///     type Empty = ();
///     type Member<Occurrence, Child: behavior::Behavior, Tail> = ();
/// }
///
/// struct ForeignNode;
/// impl ChildOccurrenceProduct<RuntimeShape> for ForeignNode {
///     type Product = ();
/// }
/// ```
pub trait ChildOccurrenceProduct<Shape>: sealed::BirthNode
where
    Shape: ChildOccurrenceShape,
{
    /// Complete shape-owned representation of this closed birth node.
    type Product;
}

/// Occurrence-carrying recursion for [`ChildOccurrenceProduct`]. Consumers
/// should name [`ChildOccurrenceProduct`] or [`ChildOccurrences`] instead.
#[doc(hidden)]
pub trait ChildOccurrenceProductAt<Occurrence, Shape>: sealed::BirthNode
where
    Shape: ChildOccurrenceShape,
{
    type Product;
}

impl<Node, Shape> ChildOccurrenceProduct<Shape> for Node
where
    Node: ChildOccurrenceProductAt<ChildHead, Shape>,
    Shape: ChildOccurrenceShape,
{
    type Product = <Node as ChildOccurrenceProductAt<ChildHead, Shape>>::Product;
}

impl<Occurrence, Shape, Child> ChildOccurrenceProductAt<Occurrence, Shape> for Child
where
    Shape: ChildOccurrenceShape,
    Child: Behavior,
{
    type Product = Shape::Member<Occurrence, Child, Shape::Empty>;
}

impl<Occurrence, Shape, Head, Tail> ChildOccurrenceProductAt<Occurrence, Shape>
    for ChildChoice<Head, Tail>
where
    Shape: ChildOccurrenceShape,
    Head: Behavior,
    Tail: ChildOccurrenceProductAt<ChildTail<Occurrence>, Shape>,
{
    type Product = Shape::Member<
        Occurrence,
        Head,
        <Tail as ChildOccurrenceProductAt<ChildTail<Occurrence>, Shape>>::Product,
    >;
}

impl<Occurrence, Shape> ChildOccurrenceProductAt<Occurrence, Shape> for Never
where
    Shape: ChildOccurrenceShape,
{
    type Product = Shape::Empty;
}

/// Occurrence-preserving representation selected by one downstream shape.
pub type ChildOccurrences<Node, Shape> = <Node as ChildOccurrenceProduct<Shape>>::Product;

mod sealed {
    use super::{
        Behavior, BehaviorBase, BirthMode, BirthNodeAt, ChildChoice, ChildHead, ChildOccurrence,
        ChildRole, ChildTail, DeclaredChildOccurrence, Never, ResolveChildOccurrenceDescriptor,
        StructuralChildOccurrence,
    };

    pub trait BirthNode {}

    pub trait NonEmptyBirthNode: BirthNode {}

    impl<Child: Behavior> BirthNode for Child {}
    impl<Child: Behavior> NonEmptyBirthNode for Child {}

    impl<Head, Tail> BirthNode for ChildChoice<Head, Tail>
    where
        Head: Behavior,
        Tail: BirthNode,
    {
    }

    impl<Head, Tail> NonEmptyBirthNode for ChildChoice<Head, Tail>
    where
        Head: Behavior,
        Tail: BirthNode,
    {
    }

    impl BirthNode for Never {}

    pub trait ChildPosition {}
    pub trait ChildProduct {}

    pub trait ChildOccurrenceDescriptor {}

    impl ChildOccurrenceDescriptor for DeclaredChildOccurrence {}

    impl<Position> ChildOccurrenceDescriptor for StructuralChildOccurrence<Position> {}

    pub trait OccurrenceResolution<Parent: Behavior, Occurrence> {}

    impl<Parent, Occurrence> OccurrenceResolution<Parent, Occurrence> for DeclaredChildOccurrence
    where
        Parent: Behavior,
        Occurrence: ChildRole<Parent>,
    {
    }

    impl<Parent: Behavior> OccurrenceResolution<Parent, ChildHead>
        for StructuralChildOccurrence<ChildHead>
    {
    }

    impl<Parent: Behavior, Position> OccurrenceResolution<Parent, ChildTail<Position>>
        for StructuralChildOccurrence<ChildTail<Position>>
    {
    }

    pub trait ResolveDescriptor<Emitter: Behavior, Occurrence> {}

    impl<Emitter, Occurrence, Position> ResolveDescriptor<Emitter, Occurrence>
        for StructuralChildOccurrence<Position>
    where
        Emitter: Behavior,
        <Emitter::Birth as BirthMode>::Child: BirthNodeAt<Position>,
    {
    }

    impl<Emitter, Occurrence> ResolveDescriptor<Emitter, Occurrence> for DeclaredChildOccurrence
    where
        Emitter: Behavior<Protocol = <<Emitter as BehaviorBase>::Base as Behavior>::Protocol>
            + BehaviorBase,
        Emitter::Base: Behavior,
        Occurrence: ChildRole<Emitter::Base>,
        <Emitter::Birth as BirthMode>::Child:
            BirthNodeAt<Occurrence::Position, Child = Occurrence::Child>,
        Occurrence::Position:
            super::ChildPosition<<Emitter::Birth as BirthMode>::Child, Occurrence::Child>,
    {
    }

    pub trait ResolveChildOccurrence<Occurrence> {}

    impl<Emitter, Occurrence> ResolveChildOccurrence<Occurrence> for Emitter
    where
        Emitter: Behavior + BehaviorBase,
        Emitter::Base: Behavior,
        Occurrence: ChildOccurrence<Emitter::Base>,
        Occurrence::Resolution: ResolveChildOccurrenceDescriptor<Emitter, Occurrence>,
    {
    }
}

/// The empty heterogeneous creation product.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoChildren;

impl sealed::ChildProduct for NoChildren {}

/// One creation appended to an ordered heterogeneous child product.
pub struct ChildCons<A: Address, C, Earlier> {
    creation: CreateChild<A, C>,
    earlier: Earlier,
}

impl<A: Address, C, Earlier> sealed::ChildProduct for ChildCons<A, C, Earlier> {}

/// A pure, ordered heterogeneous product of staged direct-child creations.
///
/// This value owns no mailbox, address, runtime actor, or lifecycle. Every
/// product position is a distinct static child occurrence, so equal numeric IDs
/// in different positions remain distinct correlations.
pub struct Children<A: Address, Product = NoChildren> {
    product: Product,
    address: PhantomData<fn() -> A>,
}

/// Closed recursive conversion implemented only by Bombay child products.
pub trait ChildProduct<A: Address>: sealed::ChildProduct + Sized {
    /// Closed sum containing exactly the concrete child behavior types.
    type Choice: BirthNodeAppend<Never>;

    #[doc(hidden)]
    fn stage(self) -> Vec<CreateChild<A, Self::Choice>>;
}

impl<A: Address> ChildProduct<A> for NoChildren {
    type Choice = Never;

    fn stage(self) -> Vec<CreateChild<A, Self::Choice>> {
        Vec::new()
    }
}

impl<A, C, Earlier> ChildProduct<A> for ChildCons<A, C, Earlier>
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Earlier: ChildProduct<A>,
{
    type Choice = ChildChoice<C, Earlier::Choice>;

    fn stage(self) -> Vec<CreateChild<A, Self::Choice>> {
        let earlier = self.earlier.stage();
        let mut creates = earlier
            .into_iter()
            .map(|creation| {
                let (id, child, kind) = creation.into_parts();
                CreateChild::from_parts(id, ChildChoice::Tail(child), kind)
            })
            .collect::<Vec<_>>();
        let (id, child, kind) = self.creation.into_parts();
        creates.push(CreateChild::from_parts(id, ChildChoice::Head(child), kind));
        creates
    }
}

impl<A: Address> Children<A, NoChildren> {
    /// Start an empty heterogeneous creation product.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            product: NoChildren,
            address: PhantomData,
        }
    }
}

impl<A: Address> Default for Children<A, NoChildren> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address, Product> Children<A, Product> {
    /// Append one complete staged creation, preserving its provenance.
    #[must_use]
    pub fn create<C>(self, creation: CreateChild<A, C>) -> Children<A, ChildCons<A, C, Product>>
    where
        C: Behavior,
        C::Protocol: Protocol<Addr = A>,
    {
        Children {
            product: ChildCons {
                creation,
                earlier: self.product,
            },
            address: PhantomData,
        }
    }

    /// Append one ordinary fresh-birth request.
    #[must_use]
    pub fn child<C>(self, id: CreationId, child: C) -> Children<A, ChildCons<A, C, Product>>
    where
        C: Behavior,
        C::Protocol: Protocol<Addr = A>,
    {
        self.create(CreateChild::birth(id, child))
    }
}

impl<A, Product> Children<A, Product>
where
    A: Address,
    Product: ChildProduct<A>,
{
    /// Produce one ordered creation batch.
    #[must_use]
    pub fn into_creates(self) -> Creations<CreateChild<A, Product::Choice>> {
        Creations::from_items(self.product.stage())
    }
}

impl<A, Occurrence, Head, Tail> ChildCreationProduct<A, Occurrence> for ChildChoice<Head, Tail>
where
    A: EndpointAddress,
    Head: Behavior,
    Head::Protocol: Protocol<Addr = A>,
    Tail: ChildCreationProduct<A, ChildTail<Occurrence>>,
{
    type Result = ChildChoice<
        ChildCreationOutcome<Head, Occurrence>,
        <Tail as ChildCreationProduct<A, ChildTail<Occurrence>>>::Result,
    >;
}

impl<A, Occurrence, Head, Tail, Host> CreateSelectedChild<A, Occurrence, Host>
    for ChildChoice<Head, Tail>
where
    A: EndpointAddress,
    A::Nonce: Send,
    Head: Behavior + Send,
    Head::Protocol: Protocol<Addr = A>,
    Tail: CreateSelectedChild<A, ChildTail<Occurrence>, Host> + Send,
    Host: EstablishChild<Occurrence, Head> + Send,
{
    async fn dispatch_birth_at(
        self,
        id: CreationId,
        route: A::Nonce,
        kind: CreationKind,
        host: &mut Host,
    ) -> ItemSettlement<
        RoutedCreation<A, Self>,
        <Self as ChildCreationProduct<A, Occurrence>>::Result,
        CreationRejection,
        Never,
    > {
        match self {
            Self::Head(child) => {
                let creation = RoutedCreation::new(CreateChild::from_parts(id, child, kind), route);
                match host.establish_child(creation).await {
                    ItemSettlement::Accepted(receipt) => {
                        ItemSettlement::Accepted(ChildChoice::Head(receipt))
                    }
                    ItemSettlement::Rejected { item, reason } => {
                        let (creation, route) = item.into_parts();
                        let (id, child, kind) = creation.into_parts();
                        ItemSettlement::Rejected {
                            item: RoutedCreation::new(
                                CreateChild::from_parts(id, Self::Head(child), kind),
                                route,
                            ),
                            reason,
                        }
                    }
                    ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
                    ItemSettlement::Corrupt { item, fault } => {
                        let (creation, route) = item.into_parts();
                        let (id, child, kind) = creation.into_parts();
                        ItemSettlement::Corrupt {
                            item: RoutedCreation::new(
                                CreateChild::from_parts(id, Self::Head(child), kind),
                                route,
                            ),
                            fault,
                        }
                    }
                }
            }
            Self::Tail(tail) => match tail.dispatch_birth_at(id, route, kind, host).await {
                ItemSettlement::Accepted(receipt) => {
                    ItemSettlement::Accepted(ChildChoice::Tail(receipt))
                }
                ItemSettlement::Rejected { item, reason } => {
                    let (creation, route) = item.into_parts();
                    let (id, child, kind) = creation.into_parts();
                    ItemSettlement::Rejected {
                        item: RoutedCreation::new(
                            CreateChild::from_parts(id, Self::Tail(child), kind),
                            route,
                        ),
                        reason,
                    }
                }
                ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
                ItemSettlement::Corrupt { item, fault } => {
                    let (creation, route) = item.into_parts();
                    let (id, child, kind) = creation.into_parts();
                    ItemSettlement::Corrupt {
                        item: RoutedCreation::new(
                            CreateChild::from_parts(id, Self::Tail(child), kind),
                            route,
                        ),
                        fault,
                    }
                }
            },
        }
    }
}

impl<A, Occurrence> ChildCreationProduct<A, Occurrence> for Never
where
    A: Address,
{
    type Result = Never;
}

impl<A, Occurrence, Host> CreateSelectedChild<A, Occurrence, Host> for Never
where
    A: Address,
{
    fn dispatch_birth_at(
        self,
        _id: CreationId,
        _route: A::Nonce,
        _kind: CreationKind,
        _host: &mut Host,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<A, Self>,
            <Self as ChildCreationProduct<A, Occurrence>>::Result,
            CreationRejection,
            Never,
        >,
    > + Send {
        async move { match self {} }
    }
}

impl<A, Occurrence, C> ChildCreationProduct<A, Occurrence> for C
where
    A: EndpointAddress,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
{
    type Result = ChildCreationOutcome<C, Occurrence>;
}

impl<A, Occurrence, C, Host> CreateSelectedChild<A, Occurrence, Host> for C
where
    A: EndpointAddress,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Host: EstablishChild<Occurrence, C>,
{
    fn dispatch_birth_at(
        self,
        id: CreationId,
        route: A::Nonce,
        kind: CreationKind,
        host: &mut Host,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<A, Self>,
            <Self as ChildCreationProduct<A, Occurrence>>::Result,
            CreationRejection,
            Never,
        >,
    > + Send {
        host.establish_child(RoutedCreation::new(
            CreateChild::from_parts(id, self, kind),
            route,
        ))
    }
}

/// A type-level description of a behavior's creation capability.
pub trait BirthMode {
    type Child;
}

/// Empty protocol projection of a closed behavior-birth algebra.
pub struct NoBirthProtocols;

/// One behavior protocol followed by the remaining closed birth projection.
pub struct BirthProtocol<P: Protocol, Tail> {
    protocol: PhantomData<fn() -> P>,
    tail: PhantomData<fn() -> Tail>,
}

/// Structural position selecting the current projected birth protocol.
pub struct BirthProtocolHead;

/// Structural position selecting inside the remaining protocol projection.
pub struct BirthProtocolTail<Position>(PhantomData<fn() -> Position>);

/// Static membership evidence for one occurrence in a birth-protocol product.
pub trait BirthProtocolAt<P: Protocol, Position> {}

impl<P: Protocol, Tail> BirthProtocolAt<P, BirthProtocolHead> for BirthProtocol<P, Tail> {}

impl<Head, Tail, P, Position> BirthProtocolAt<P, BirthProtocolTail<Position>>
    for BirthProtocol<Head, Tail>
where
    Head: Protocol,
    P: Protocol,
    Tail: BirthProtocolAt<P, Position>,
{
}

/// Closed product operation used by the structural birth projection.
#[doc(hidden)]
pub trait BirthProtocolProduct {
    type Append<Tail: BirthProtocolProduct>: BirthProtocolProduct;
}

impl BirthProtocolProduct for NoBirthProtocols {
    type Append<Tail: BirthProtocolProduct> = Tail;
}

impl<P, Rest> BirthProtocolProduct for BirthProtocol<P, Rest>
where
    P: Protocol,
    Rest: BirthProtocolProduct,
{
    type Append<Tail: BirthProtocolProduct> = BirthProtocol<P, Rest::Append<Tail>>;
}

/// Closed static projection of a behavior's own protocol and every protocol
/// reachable through its transitive staged-birth algebra.
///
/// This is structural information derived from [`Behavior::Birth`]. It makes
/// no hosting or allocation decision and does not inspect send destinations.
pub trait BirthProtocols: Behavior {
    type Protocols: BirthProtocolProduct;
}

impl<B> BirthProtocols for B
where
    B: Behavior,
    B::Birth: BirthModeProtocols,
{
    type Protocols = BirthProtocol<B::Protocol, <B::Birth as BirthModeProtocols>::Protocols>;
}

#[doc(hidden)]
pub trait BirthModeProtocols {
    type Protocols: BirthProtocolProduct;
}

impl<M> BirthModeProtocols for M
where
    M: BirthMode,
    M::Child: BirthNodeProtocols,
{
    type Protocols = <M::Child as BirthNodeProtocols>::Protocols;
}

#[doc(hidden)]
pub trait BirthNodeProtocols {
    type Protocols: BirthProtocolProduct;
}

impl<B> BirthNodeProtocols for B
where
    B: BirthProtocols,
{
    type Protocols = B::Protocols;
}

impl<Head, Tail> BirthNodeProtocols for ChildChoice<Head, Tail>
where
    Head: BirthNodeProtocols,
    Tail: BirthNodeProtocols,
{
    type Protocols = <Head::Protocols as BirthProtocolProduct>::Append<Tail::Protocols>;
}

impl BirthNodeProtocols for Never {
    type Protocols = NoBirthProtocols;
}

/// Structural logical-destination projection for one closed birth node.
///
/// This implementation detail is public only because its associated product
/// participates in the blanket [`crate::LogicalHostRequirements`] interface.
#[doc(hidden)]
pub trait BirthNodeLogicalHosts {
    type LogicalHosts: BirthProtocolProduct;
}

impl<B> BirthNodeLogicalHosts for B
where
    B: crate::LogicalHostRequirements,
{
    type LogicalHosts = B::LogicalHosts;
}

impl<Head, Tail> BirthNodeLogicalHosts for ChildChoice<Head, Tail>
where
    Head: BirthNodeLogicalHosts,
    Tail: BirthNodeLogicalHosts,
{
    type LogicalHosts = <Head::LogicalHosts as BirthProtocolProduct>::Append<Tail::LogicalHosts>;
}

impl BirthNodeLogicalHosts for Never {
    type LogicalHosts = NoBirthProtocols;
}

/// This behavior cannot emit child births.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoBirths;

impl BirthMode for NoBirths {
    type Child = Never;
}

/// This behavior may emit births of `C`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Births<C>(PhantomData<fn() -> C>);

impl<C> BirthMode for Births<C> {
    type Child = C;
}

/// This behavior may emit births of `C` whose exact settlements remain in
/// runtime custody until actor retirement.
///
/// Unlike [`Births`], this mode does not return a creation settlement as a
/// later input to the live creator. It is the deliberate policy for actors
/// whose terminal owner, rather than another behavior transition, must retain
/// every authoritative creation result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RetirementBirths<C>(PhantomData<fn() -> C>);

impl<C> BirthMode for RetirementBirths<C> {
    type Child = C;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actions, BehaviorActed, NoBirths, User};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestAddr(u64);

    impl Address for TestAddr {
        type Nonce = u64;
    }

    struct TestEndpoint<P>(PhantomData<fn() -> P>);

    impl<P> Clone for TestEndpoint<P> {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl<P> Copy for TestEndpoint<P> {}

    impl EndpointAddress for TestAddr {
        type Established<P>
            = TestEndpoint<P>
        where
            P: Protocol<Addr = Self>;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Child;

    struct SharedProtocol;

    impl behavior::Protocol for SharedProtocol {
        type Addr = TestAddr;
        type Msg = u8;
    }

    struct Primary;
    struct Fallback;

    macro_rules! shared_protocol_behavior {
        ($behavior:ty, $birth:ty) => {
            impl Behavior for $behavior {
                type Protocol = SharedProtocol;
                type Event = User<TestAddr, u8>;
                type Sends = Vec<Never>;
                type Ph = Never;
                type Error = Never;
                type Birth = $birth;

                fn transition(
                    &mut self,
                    _: crate::ActiveTurn,
                    _: Self::Event,
                ) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }
        };
    }

    shared_protocol_behavior!(Primary, Births<Child>);
    shared_protocol_behavior!(Fallback, NoBirths);

    impl behavior::Protocol for Child {
        type Addr = TestAddr;
        type Msg = u8;
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<TestAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum ChildPlan {
        CreateChild,
        RejectBeforeTransfer,
    }

    struct RecordingHost {
        calls: usize,
        observed: Vec<(CreationId, u64, CreationKind)>,
        plan: ChildPlan,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SharedCreation {
        Primary(CreationId, u64),
        Fallback(CreationId, u64),
    }

    #[derive(Default)]
    struct SharedProtocolHost(Vec<SharedCreation>);

    impl EstablishChild<ChildHead, Primary> for SharedProtocolHost {
        async fn establish_child(
            &mut self,
            creation: RoutedCreation<TestAddr, Primary>,
        ) -> ItemSettlement<
            RoutedCreation<TestAddr, Primary>,
            ChildCreationOutcome<Primary, ChildHead>,
            CreationRejection,
            Never,
        > {
            let id = creation.id();
            let kind = creation.kind();
            let route = creation.route();
            let (creation, _) = creation.into_parts();
            let (_, _child, _) = creation.into_parts();
            self.0.push(SharedCreation::Primary(id, route));
            ItemSettlement::Accepted(ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    id,
                    kind,
                    EstablishedRecipient::issued(TestEndpoint(PhantomData)),
                ),
            })
        }
    }

    impl EstablishChild<ChildTail<ChildHead>, Fallback> for SharedProtocolHost {
        async fn establish_child(
            &mut self,
            creation: RoutedCreation<TestAddr, Fallback>,
        ) -> ItemSettlement<
            RoutedCreation<TestAddr, Fallback>,
            ChildCreationOutcome<Fallback, ChildTail<ChildHead>>,
            CreationRejection,
            Never,
        > {
            let id = creation.id();
            let kind = creation.kind();
            let route = creation.route();
            let (creation, _) = creation.into_parts();
            let (_, _child, _) = creation.into_parts();
            self.0.push(SharedCreation::Fallback(id, route));
            ItemSettlement::Accepted(ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    id,
                    kind,
                    EstablishedRecipient::issued(TestEndpoint(PhantomData)),
                ),
            })
        }
    }

    impl EstablishChild<ChildHead, Child> for RecordingHost {
        async fn establish_child(
            &mut self,
            creation: RoutedCreation<TestAddr, Child>,
        ) -> ItemSettlement<
            RoutedCreation<TestAddr, Child>,
            ChildCreationOutcome<Child, ChildHead>,
            CreationRejection,
            Never,
        > {
            self.calls += 1;
            self.observed
                .push((creation.id(), creation.route(), creation.kind()));
            match self.plan {
                ChildPlan::CreateChild => {
                    let id = creation.id();
                    let kind = creation.kind();
                    let (creation, _) = creation.into_parts();
                    let (_, _child, _) = creation.into_parts();
                    ItemSettlement::Accepted(ChildCreationOutcome::Established {
                        established: EstablishedCreation::installed(
                            id,
                            kind,
                            EstablishedRecipient::issued(TestEndpoint(PhantomData)),
                        ),
                    })
                }
                ChildPlan::RejectBeforeTransfer => ItemSettlement::Rejected {
                    item: creation,
                    reason: CreationRejection::EnvironmentFailed,
                },
            }
        }
    }

    fn host(plan: ChildPlan) -> RecordingHost {
        RecordingHost {
            calls: 0,
            observed: Vec::new(),
            plan,
        }
    }

    fn assert_send<T: Send>(_: &T) {}

    #[test]
    fn empty_child_product_stages_no_creations() {
        let creations = <NoChildren as ChildProduct<TestAddr>>::stage(NoChildren);
        assert!(creations.is_empty());
    }

    #[test]
    fn creation_values_preserve_ids_order_and_debug_identity() {
        let mut sequence = CreationSequence::new();
        let first = sequence.issue().expect("the first creation ID exists");
        let second = sequence.issue().expect("the second creation ID exists");
        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);

        let first_correlation = CreationCorrelation::<SharedProtocol, ChildHead>::new(first);
        let same_correlation = CreationCorrelation::<SharedProtocol, ChildHead>::new(first);
        let second_correlation = CreationCorrelation::<SharedProtocol, ChildHead>::new(second);
        assert_eq!(first_correlation, same_correlation);
        assert_ne!(first_correlation, second_correlation);
        assert_eq!(
            format!("{first_correlation:?}"),
            "CreationCorrelation(CreationId(1))"
        );

        let mut creations = Creations::one(1_u8);
        creations.extend([2, 3]);
        assert_eq!(creations.len(), 3);
        assert!(!creations.is_empty());
        assert_eq!(creations.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(
            (&creations).into_iter().copied().collect::<Vec<_>>(),
            [1, 2, 3]
        );

        let collected: Creations<_> = [4_u8, 5].into_iter().collect();
        assert_eq!(collected.len(), 2);
        assert_eq!(collected.into_iter().collect::<Vec<_>>(), [4, 5]);

        let returned = ChildCreationSettled::<Child, ChildHead>::new(SettledItem::Unattempted(
            RoutedCreation::new(CreateChild::birth(first, Child), 17),
        ));
        assert_eq!(format!("{returned:?}"), "ChildCreationSettled { .. }");
    }

    #[tokio::test]
    async fn concrete_child_dispatches_once_with_exact_creation_and_route() {
        let mut sequence = CreationSequence::new();
        let previous = sequence.issue().expect("the previous ID exists");
        let id = sequence.issue().expect("the replacement ID exists");
        let mut host = host(ChildPlan::CreateChild);
        let kind = CreationKind::replacement(previous);
        let future = Child.dispatch_birth(id, 17, kind, &mut host);
        assert_send(&future);
        let result = future.await;

        let ItemSettlement::Accepted(ChildCreationOutcome::Established { established }) = result
        else {
            panic!("expected the child to be created");
        };
        assert_eq!(established.id(), id);
        assert_eq!(host.calls, 1);
        assert_eq!(host.observed, [(id, 17, kind)]);
    }

    #[tokio::test]
    async fn concrete_child_returns_the_exact_rejected_creation_without_retry() {
        let mut sequence = CreationSequence::new();
        let previous = sequence.issue().expect("the previous ID exists");
        let id = sequence.issue().expect("the replacement ID exists");
        let mut host = host(ChildPlan::RejectBeforeTransfer);
        let kind = CreationKind::replacement(previous);
        let future = Child.dispatch_birth(id, 23, kind, &mut host);
        assert_send(&future);
        let result = future.await;

        let ItemSettlement::Rejected { item, reason } = result else {
            panic!("expected refusal before child ownership transfer");
        };
        assert_eq!(
            item,
            RoutedCreation::new(CreateChild::replacement(id, previous, Child), 23)
        );
        assert_eq!(reason, CreationRejection::EnvironmentFailed);
        assert_eq!(host.calls, 1);
        assert_eq!(host.observed, [(id, 23, kind)]);
    }

    #[tokio::test]
    async fn distinct_child_alternatives_preserve_one_canonical_protocol_identity() {
        type Alternatives = ChildChoice<Primary, ChildChoice<Fallback, Never>>;

        fn requires_shared_protocol<C: Behavior<Protocol = SharedProtocol>>() {}
        requires_shared_protocol::<Primary>();
        requires_shared_protocol::<Fallback>();

        let mut sequence = CreationSequence::new();
        let primary_id = sequence.issue().expect("the primary ID exists");
        let fallback_id = sequence.issue().expect("the fallback ID exists");
        let mut host = SharedProtocolHost::default();
        let primary = Alternatives::Head(Primary)
            .dispatch_birth(primary_id, 11, CreationKind::Birth, &mut host)
            .await;
        let fallback = Alternatives::Tail(ChildChoice::Head(Fallback))
            .dispatch_birth(fallback_id, 17, CreationKind::Birth, &mut host)
            .await;

        let ItemSettlement::Accepted(ChildChoice::Head(ChildCreationOutcome::Established {
            established: primary,
        })) = primary
        else {
            panic!("expected the primary child result");
        };
        assert_eq!(primary.id(), primary_id);
        let ItemSettlement::Accepted(ChildChoice::Tail(ChildChoice::Head(
            ChildCreationOutcome::Established {
                established: fallback,
            },
        ))) = fallback
        else {
            panic!("expected the fallback child result");
        };
        assert_eq!(fallback.id(), fallback_id);

        assert_eq!(
            host.0,
            [
                SharedCreation::Primary(primary_id, 11),
                SharedCreation::Fallback(fallback_id, 17),
            ]
        );
    }

    #[test]
    fn birth_protocol_projection_recurses_without_inspecting_send_lanes() {
        type Protocols = <Primary as BirthProtocols>::Protocols;
        type Expected = BirthProtocol<SharedProtocol, BirthProtocol<Child, NoBirthProtocols>>;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}
        exact::<Protocols>();
    }
}
