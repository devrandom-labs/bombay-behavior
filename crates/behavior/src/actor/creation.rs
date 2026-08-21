//! Staged fresh-actor creation capabilities.

use core::future::Future;
use core::marker::PhantomData;

use super::addressing::{Address, EndpointAddress, EstablishedActor, EstablishedRecipient};
use crate::next::Never;
use crate::{Behavior, BehaviorAddr, Protocol};

/// Behavior-owned provenance for a staged fresh actor creation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationKind<N> {
    /// An initial or ordinary later birth.
    Birth,
    /// A fresh successor incarnation requested by a replacement protocol.
    ReplacementIncarnation {
        /// The exact child incarnation this fresh actor is intended to replace.
        replaces: N,
    },
}

impl<N> CreationKind<N> {
    #[must_use]
    pub const fn replacement_of(replaces: N) -> Self {
        Self::ReplacementIncarnation { replaces }
    }
}

/// A staged request to establish a fresh child at a creator-local nonce.
///
/// The nonce is a routing and correlation key, not an actor identity or proof
/// of freshness. The kind is Behavior-owned intent; a runtime's typed creation
/// resolution is the corresponding committed fact. Replacement at an existing
/// address is deliberately absent; stable identity is derived with a proxy
/// actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Create<A: Address, New> {
    pub nonce: A::Nonce,
    pub child: New,
    pub kind: CreationKind<A::Nonce>,
}

impl<A: Address, New> Create<A, New> {
    #[must_use]
    pub const fn new(nonce: A::Nonce, child: New, kind: CreationKind<A::Nonce>) -> Self {
        Self { nonce, child, kind }
    }

    #[must_use]
    pub const fn birth(nonce: A::Nonce, child: New) -> Self {
        Self::new(nonce, child, CreationKind::Birth)
    }

    #[must_use]
    pub const fn replacement_incarnation(nonce: A::Nonce, replaces: A::Nonce, child: New) -> Self {
        Self::new(nonce, child, CreationKind::replacement_of(replaces))
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

/// Child behavior selected by one named role.
pub type RoleChild<Parent, Role> = <Role as ChildRole<Parent>>::Child;

/// Canonical protocol selected by one named role.
pub type RoleProtocol<Parent, Role> = <RoleChild<Parent, Role> as Behavior>::Protocol;

/// Creator-local route for one child behavior at one nominal role.
///
/// `Role` is authored or generated topology evidence. Its [`ChildRole`]
/// implementation selects a structural position only when an operation is
/// contextualized by the parent behavior. The role is not protocol identity,
/// runtime identity, or a lookup key. Duplicate occurrences receive distinct
/// nominal role types and therefore cannot exchange routes.
pub struct ChildRoute<Child, Role>
where
    Child: Behavior,
{
    nonce: <BehaviorAddr<Child> as Address>::Nonce,
    role: PhantomData<fn() -> Role>,
}

impl<Child: Behavior, Position> Copy for ChildRoute<Child, Position> {}

impl<Child: Behavior, Position> Clone for ChildRoute<Child, Position> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Child: Behavior, Position> PartialEq for ChildRoute<Child, Position> {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}

impl<Child: Behavior, Position> Eq for ChildRoute<Child, Position> {}

impl<Child: Behavior, Position> core::fmt::Debug for ChildRoute<Child, Position>
where
    <BehaviorAddr<Child> as Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ChildRoute")
            .field("nonce", &self.nonce)
            .finish()
    }
}

impl<Child: Behavior, Position> ChildRoute<Child, Position> {
    /// Name this declared role at a creator-local route.
    #[must_use]
    pub const fn new(nonce: <BehaviorAddr<Child> as Address>::Nonce) -> Self {
        Self {
            nonce,
            role: PhantomData,
        }
    }

    /// Return the creator-local nonce used by creation and lifecycle facts.
    #[must_use]
    pub const fn nonce(self) -> <BehaviorAddr<Child> as Address>::Nonce {
        self.nonce
    }

    /// Stage one creation request with explicit Behavior-owned provenance.
    #[must_use]
    pub const fn stage(
        self,
        child: Child,
        kind: CreationKind<<BehaviorAddr<Child> as Address>::Nonce>,
    ) -> Create<BehaviorAddr<Child>, Child> {
        Create::new(self.nonce(), child, kind)
    }

    /// Stage an ordinary fresh-birth request for this declared child role.
    #[must_use]
    pub const fn birth(self, child: Child) -> Create<BehaviorAddr<Child>, Child> {
        self.stage(child, CreationKind::Birth)
    }

    /// Stage a replacement incarnation for this declared child role.
    #[must_use]
    pub const fn replacement_incarnation(
        self,
        replaces: <BehaviorAddr<Child> as Address>::Nonce,
        child: Child,
    ) -> Create<BehaviorAddr<Child>, Child> {
        self.stage(child, CreationKind::replacement_of(replaces))
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
    /// The creator-local nonce is already bound.
    #[error("the creator-local nonce is already bound")]
    NonceAlreadyBound,
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

/// Committed or rejected result for one exact child-protocol occurrence.
///
/// `Installed` is constructed only after fresh allocation, successful
/// initialization, installation, and creator-local binding. It returns the
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
///     Address, CreationKind, EndpointAddress, EstablishedCreation,
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
/// let backup: EstablishedCreation<Worker, Backup> = EstablishedCreation::installed(
///     1,
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
        nonce: <P::Addr as Address>::Nonce,
        kind: CreationKind<<P::Addr as Address>::Nonce>,
        recipient: EstablishedRecipient<P>,
        occurrence: PhantomData<fn() -> Occurrence>,
    },
    Rejected {
        nonce: <P::Addr as Address>::Nonce,
        kind: CreationKind<<P::Addr as Address>::Nonce>,
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
        nonce: <P::Addr as Address>::Nonce,
        kind: CreationKind<<P::Addr as Address>::Nonce>,
        recipient: EstablishedRecipient<P>,
    ) -> Self {
        Self::Installed {
            nonce,
            kind,
            recipient,
            occurrence: PhantomData,
        }
    }

    #[must_use]
    pub const fn rejected(
        nonce: <P::Addr as Address>::Nonce,
        kind: CreationKind<<P::Addr as Address>::Nonce>,
        reason: CreationRejection,
    ) -> Self {
        Self::Rejected {
            nonce,
            kind,
            reason,
            occurrence: PhantomData,
        }
    }

    #[must_use]
    pub const fn nonce(&self) -> <P::Addr as Address>::Nonce {
        match self {
            Self::Installed { nonce, .. } | Self::Rejected { nonce, .. } => *nonce,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> CreationKind<<P::Addr as Address>::Nonce> {
        match self {
            Self::Installed { kind, .. } | Self::Rejected { kind, .. } => *kind,
        }
    }

    /// Consume the fact and return its exact endpoint capability.
    ///
    /// # Errors
    /// Returns the original [`CreationRejection`] when installation did not
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
    /// capability-strengthening boundary; it is not part of the creation-fact
    /// identity. Ordinary consumers can retain [`EstablishedRecipient<P>`]
    /// without carrying the parent behavior.
    ///
    /// # Errors
    /// Returns the original [`CreationRejection`] when installation did not
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
    pub nonce: <P::Addr as Address>::Nonce,
    pub message: P::Msg,
    occurrence: PhantomData<fn() -> Occurrence>,
}

impl<P, Occurrence> ChildDelivery<P, Occurrence>
where
    P: Protocol,
{
    #[must_use]
    pub const fn at<Child>(route: ChildRoute<Child, Occurrence>, message: P::Msg) -> Self
    where
        Child: Behavior<Protocol = P>,
    {
        Self {
            nonce: route.nonce(),
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
            nonce: self.nonce,
            message: self.message.clone(),
            occurrence: PhantomData,
        }
    }
}

/// Static interpreter capability for installing one concrete child behavior.
///
/// An interpreter implements this trait separately for every concrete child
/// behavior it can install. `C::Protocol` remains the canonical hosting
/// identity at this boundary: distinct child behaviors may select the same
/// protocol and therefore share one runtime-owned protocol space while still
/// receiving distinct installation calls. Semantic child roles and routes are
/// deliberately not storage identities. Heterogeneous child sums require all
/// applicable implementations through recursive static bounds, so unsupported
/// alternatives fail to compile instead of falling through to a registry or
/// erased path. The returned future is sendable so an interpreter may remain
/// inside a thread-safe recursive driver future.
pub trait InstallBirth<Position, C, Output, Error>
where
    C: Behavior,
{
    /// Install and commit exactly the supplied concrete creation.
    ///
    /// # Errors
    /// Returns the interpreter's typed allocation, initialization, or commit
    /// failure without binding the requested nonce.
    fn install_birth(
        &mut self,
        creation: Create<BehaviorAddr<C>, C>,
    ) -> impl Future<Output = Result<Output, Error>> + Send;
}

/// Exhaustive static dispatch of one creation-only child sum.
///
/// This is an interpreter-facing derived construction, not another actor-model
/// operation. Implementations must preserve the nonce and provenance and call
/// exactly one concrete [`InstallBirth`] implementation. [`ChildChoice`]
/// provides the closed recursive heterogeneous sum. Dispatch futures are
/// sendable; heterogeneous sums therefore require sendable alternatives,
/// creator-local nonces, and installers.
pub trait DispatchBirth<A: Address, Installer, Output, Error>: Sized {
    /// Select exactly one concrete installer while preserving creation data.
    ///
    /// # Errors
    /// Returns the selected concrete installer's typed failure unchanged.
    fn dispatch_birth(
        self,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        installer: &mut Installer,
    ) -> impl Future<Output = Result<Output, Error>> + Send;
}

#[doc(hidden)]
pub trait DispatchBirthAt<A: Address, Position, Installer, Output, Error>: Sized {
    fn dispatch_birth_at(
        self,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        installer: &mut Installer,
    ) -> impl Future<Output = Result<Output, Error>> + Send;
}

impl<A, Child, Installer, Output, Error> DispatchBirth<A, Installer, Output, Error> for Child
where
    A: Address,
    Child: DispatchBirthAt<A, ChildHead, Installer, Output, Error>,
{
    fn dispatch_birth(
        self,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        installer: &mut Installer,
    ) -> impl Future<Output = Result<Output, Error>> + Send {
        self.dispatch_birth_at(nonce, kind, installer)
    }
}

/// One alternative in a closed, recursively composed child-creation sum.
///
/// `Head` is one concrete child behavior and `Tail` is the remaining closed
/// sum. This is a creation choice only: it is not a behavior, message
/// envelope, registry, or runtime dispatch mechanism.
///
/// Every alternative requires a concrete installer; incomplete interpreter
/// support is rejected statically:
///
/// ```compile_fail
/// use behavior::{
///     Actions, Behavior, BehaviorActed, ChildChoice, Create, DispatchBirth,
///     InstallBirth, MailAddr, Never, NoBirths, Protocol, User,
/// };
///
/// struct First;
/// struct Second;
///
/// macro_rules! inert {
///     ($child:ty) => {
///         impl Protocol for $child {
///             type Addr = MailAddr;
///             type Msg = Never;
///         }
///         impl Behavior for $child {
///             type Event = User<MailAddr, Never>;
///             type Sends = Vec<Never>;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = NoBirths;
///
///             fn transition(
///                 &mut self,
///                 _: behavior::ActiveTurn,
///                 event: Self::Event,
///             ) -> BehaviorActed<Self> {
///                 match event.message {}
///             }
///         }
///     };
/// }
/// inert!(First);
/// inert!(Second);
///
/// struct Incomplete;
/// impl InstallBirth<ChildHead, First, (), Never> for Incomplete {
///     async fn install_birth(
///         &mut self,
///         _: Create<MailAddr, First>,
///     ) -> Result<(), Never> {
///         Ok(())
///     }
/// }
///
/// type Both = ChildChoice<First, ChildChoice<Second, Never>>;
/// fn require_complete<T: DispatchBirth<MailAddr, Incomplete, (), Never>>() {}
/// require_complete::<Both>();
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
/// struct First;
/// struct Second;
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
/// inert!(First);
/// inert!(Second);
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
///     type Birth = Births<ChildChoice<Second, ChildChoice<First, Never>>>;
///     fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
///         match event.message {}
///     }
/// }
/// struct ForgedFirst;
/// impl ChildRole<Parent> for ForgedFirst {
///     type Child = First;
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

mod sealed {
    pub trait ChildPosition {}
    pub trait ChildProduct {}
}

/// The empty heterogeneous creation product.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoChildren;

impl sealed::ChildProduct for NoChildren {}

/// One creation appended to an ordered heterogeneous child product.
pub struct ChildCons<A: Address, C, Earlier> {
    creation: Create<A, C>,
    earlier: Earlier,
}

impl<A: Address, C, Earlier> sealed::ChildProduct for ChildCons<A, C, Earlier> {}

/// A pure, ordered heterogeneous product of staged direct-child creations.
///
/// This value owns no mailbox, address, runtime actor, or lifecycle. Explicit
/// creator-local nonces remain caller policy. Conversion rejects duplicates
/// within this product before producing any `Actions` value; the interpreter
/// must still reject collisions with already established children.
pub struct Children<A: Address, Product = NoChildren> {
    product: Product,
    address: PhantomData<fn() -> A>,
}

/// Rejection while converting a heterogeneous child product into creations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChildrenError<N> {
    /// Two declarations selected the same creator-local routing nonce.
    #[error("duplicate creator-local child nonce")]
    DuplicateNonce {
        /// The colliding nonce; no creation product was returned.
        nonce: N,
    },
}

/// Closed recursive conversion implemented only by Bombay child products.
pub trait ChildProduct<A: Address>: sealed::ChildProduct + Sized {
    /// Closed sum containing exactly the concrete child behavior types.
    type Choice;

    #[doc(hidden)]
    fn stage(
        self,
    ) -> Result<(Vec<Create<A, Self::Choice>>, Vec<A::Nonce>), ChildrenError<A::Nonce>>;
}

impl<A: Address> ChildProduct<A> for NoChildren {
    type Choice = Never;

    fn stage(
        self,
    ) -> Result<(Vec<Create<A, Self::Choice>>, Vec<A::Nonce>), ChildrenError<A::Nonce>> {
        Ok((Vec::new(), Vec::new()))
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

    fn stage(
        self,
    ) -> Result<(Vec<Create<A, Self::Choice>>, Vec<A::Nonce>), ChildrenError<A::Nonce>> {
        let (earlier, mut nonces) = self.earlier.stage()?;
        if nonces.contains(&self.creation.nonce) {
            return Err(ChildrenError::DuplicateNonce {
                nonce: self.creation.nonce,
            });
        }
        let mut creates = earlier
            .into_iter()
            .map(|creation| {
                Create::new(
                    creation.nonce,
                    ChildChoice::Tail(creation.child),
                    creation.kind,
                )
            })
            .collect::<Vec<_>>();
        nonces.push(self.creation.nonce);
        creates.push(Create::new(
            self.creation.nonce,
            ChildChoice::Head(self.creation.child),
            self.creation.kind,
        ));
        Ok((creates, nonces))
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
    pub fn create<C>(self, creation: Create<A, C>) -> Children<A, ChildCons<A, C, Product>>
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
    pub fn child<C>(self, nonce: A::Nonce, child: C) -> Children<A, ChildCons<A, C, Product>>
    where
        C: Behavior,
        C::Protocol: Protocol<Addr = A>,
    {
        self.create(Create::birth(nonce, child))
    }

    /// Append an ordinary birth through one named child-role binding.
    #[must_use]
    pub fn child_at<C, Position>(
        self,
        route: ChildRoute<C, Position>,
        child: C,
    ) -> Children<A, ChildCons<A, C, Product>>
    where
        C: Behavior,
        C::Protocol: Protocol<Addr = A>,
    {
        self.create(route.birth(child))
    }
}

impl<A, Product> Children<A, Product>
where
    A: Address,
    Product: ChildProduct<A>,
{
    /// Validate the shared nonce domain and produce ordered creation requests.
    ///
    /// # Errors
    /// Returns [`ChildrenError::DuplicateNonce`] without returning a partial
    /// creation vector when any two declarations in this product share a
    /// nonce. Existing child-namespace collisions remain interpreter-owned
    /// creation rejections.
    pub fn into_creates(self) -> Result<Vec<Create<A, Product::Choice>>, ChildrenError<A::Nonce>> {
        self.product.stage().map(|(creates, _)| creates)
    }
}

impl<A, Position, Head, Tail, Installer, Output, Error>
    DispatchBirthAt<A, Position, Installer, Output, Error> for ChildChoice<Head, Tail>
where
    A: Address,
    A::Nonce: Send,
    Head: Behavior + Send,
    Head::Protocol: Protocol<Addr = A>,
    Tail: DispatchBirthAt<A, ChildTail<Position>, Installer, Output, Error> + Send,
    Installer: InstallBirth<Position, Head, Output, Error> + Send,
{
    async fn dispatch_birth_at(
        self,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        installer: &mut Installer,
    ) -> Result<Output, Error> {
        match self {
            Self::Head(child) => {
                installer
                    .install_birth(Create::new(nonce, child, kind))
                    .await
            }
            Self::Tail(tail) => tail.dispatch_birth_at(nonce, kind, installer).await,
        }
    }
}

impl<A, Position, Installer, Output, Error> DispatchBirthAt<A, Position, Installer, Output, Error>
    for Never
where
    A: Address,
{
    fn dispatch_birth_at(
        self,
        _nonce: A::Nonce,
        _kind: CreationKind<A::Nonce>,
        _installer: &mut Installer,
    ) -> impl Future<Output = Result<Output, Error>> + Send {
        async move { match self {} }
    }
}

impl<A, Position, C, Installer, Output, Error>
    DispatchBirthAt<A, Position, Installer, Output, Error> for C
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Installer: InstallBirth<Position, C, Output, Error>,
{
    fn dispatch_birth_at(
        self,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        installer: &mut Installer,
    ) -> impl Future<Output = Result<Output, Error>> + Send {
        installer.install_birth(Create::new(nonce, self, kind))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actions, BehaviorActed, MailAddr, NoBirths, User};

    struct Child;

    struct SharedProtocol;

    impl behavior::Protocol for SharedProtocol {
        type Addr = MailAddr;
        type Msg = u8;
    }

    struct Primary;
    struct Fallback;

    macro_rules! shared_protocol_behavior {
        ($behavior:ty, $birth:ty) => {
            impl Behavior for $behavior {
                type Protocol = SharedProtocol;
                type Event = User<MailAddr, u8>;
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
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum InstallError {
        Refused,
    }

    struct RecordingInstaller {
        calls: usize,
        observed: Vec<(u64, CreationKind<u64>)>,
        result: Result<u32, InstallError>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SharedInstallation {
        Primary(u64),
        Fallback(u64),
    }

    #[derive(Default)]
    struct SharedProtocolInstaller(Vec<SharedInstallation>);

    impl InstallBirth<ChildHead, Primary, (), Never> for SharedProtocolInstaller {
        async fn install_birth(
            &mut self,
            creation: Create<MailAddr, Primary>,
        ) -> Result<(), Never> {
            self.0.push(SharedInstallation::Primary(creation.nonce));
            Ok(())
        }
    }

    impl InstallBirth<ChildTail<ChildHead>, Fallback, (), Never> for SharedProtocolInstaller {
        async fn install_birth(
            &mut self,
            creation: Create<MailAddr, Fallback>,
        ) -> Result<(), Never> {
            self.0.push(SharedInstallation::Fallback(creation.nonce));
            Ok(())
        }
    }

    impl InstallBirth<ChildHead, Child, u32, InstallError> for RecordingInstaller {
        async fn install_birth(
            &mut self,
            creation: Create<MailAddr, Child>,
        ) -> Result<u32, InstallError> {
            self.calls += 1;
            self.observed.push((creation.nonce, creation.kind));
            self.result
        }
    }

    fn installer(result: Result<u32, InstallError>) -> RecordingInstaller {
        RecordingInstaller {
            calls: 0,
            observed: Vec::new(),
            result,
        }
    }

    fn assert_send<T: Send>(_: &T) {}

    enum WorkerRole {}

    impl ChildRole<Primary> for WorkerRole {
        type Child = Child;
        type Position = ChildHead;
    }

    #[test]
    fn child_route_preserves_one_nonce_across_routing_and_creation_provenance() {
        let route = ChildRoute::<Child, WorkerRole>::new(17);

        assert_eq!(route.nonce(), 17);
        assert_eq!(route, ChildRoute::new(17));
        assert_ne!(route, ChildRoute::new(19));
        assert_eq!(format!("{route:?}"), "ChildRoute { nonce: 17 }");

        let birth = route.birth(Child);
        assert_eq!(birth.nonce, 17);
        assert_eq!(birth.kind, CreationKind::Birth);

        let replacement = route.replacement_incarnation(11, Child);
        assert_eq!(replacement.nonce, 17);
        assert_eq!(replacement.kind, CreationKind::replacement_of(11));
    }

    #[test]
    fn empty_child_product_stages_no_creations_or_nonces() {
        let (creations, nonces) = <NoChildren as ChildProduct<MailAddr>>::stage(NoChildren)
            .expect("an empty child product is valid");
        assert!(creations.is_empty());
        assert!(nonces.is_empty());
    }

    #[tokio::test]
    async fn concrete_child_dispatches_once_with_exact_nonce_provenance_and_output() {
        let mut installer = installer(Ok(91));
        let future = Child.dispatch_birth(
            17,
            CreationKind::ReplacementIncarnation { replaces: 8 },
            &mut installer,
        );
        assert_send(&future);
        let result = future.await;

        assert_eq!(result, Ok(91));
        assert_eq!(installer.calls, 1);
        assert_eq!(
            installer.observed,
            [(17, CreationKind::ReplacementIncarnation { replaces: 8 })]
        );
    }

    #[tokio::test]
    async fn concrete_child_returns_the_exact_installer_error_without_retry() {
        let mut installer = installer(Err(InstallError::Refused));
        let future = Child.dispatch_birth(23, CreationKind::replacement_of(19), &mut installer);
        assert_send(&future);
        let result = future.await;

        assert_eq!(result, Err(InstallError::Refused));
        assert_eq!(installer.calls, 1);
        assert_eq!(installer.observed, [(23, CreationKind::replacement_of(19))]);
    }

    #[tokio::test]
    async fn distinct_child_alternatives_preserve_one_canonical_protocol_identity() {
        type Alternatives = ChildChoice<Primary, ChildChoice<Fallback, Never>>;

        fn requires_shared_protocol<C: Behavior<Protocol = SharedProtocol>>() {}
        requires_shared_protocol::<Primary>();
        requires_shared_protocol::<Fallback>();

        let mut installer = SharedProtocolInstaller::default();
        Alternatives::Head(Primary)
            .dispatch_birth(11, CreationKind::Birth, &mut installer)
            .await
            .unwrap();
        Alternatives::Tail(ChildChoice::Head(Fallback))
            .dispatch_birth(17, CreationKind::Birth, &mut installer)
            .await
            .unwrap();

        assert_eq!(
            installer.0,
            [
                SharedInstallation::Primary(11),
                SharedInstallation::Fallback(17),
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
