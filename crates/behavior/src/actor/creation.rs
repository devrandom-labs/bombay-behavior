//! Staged fresh-actor creation capabilities.

use core::future::Future;
use core::marker::PhantomData;

use super::addressing::Address;
use crate::Behavior;
use crate::next::Never;

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
/// of freshness. The kind is Behavior-owned intent; [`crate::CreationResolved`]
/// is the corresponding committed runtime fact. Replacement at an existing
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

/// Static interpreter capability for installing one concrete child behavior.
///
/// An interpreter implements this trait separately for every concrete child
/// protocol it can install. Heterogeneous child sums require all applicable
/// implementations through recursive static bounds, so unsupported alternatives
/// fail to compile instead of falling through to a registry or erased path. The
/// returned future is sendable so an interpreter may remain inside a
/// thread-safe recursive driver future.
pub trait InstallBirth<A: Address, C: Behavior<Addr = A>, Output, Error> {
    /// Install and commit exactly the supplied concrete creation.
    ///
    /// # Errors
    /// Returns the interpreter's typed allocation, initialization, or commit
    /// failure without binding the requested nonce.
    fn install_birth(
        &mut self,
        creation: Create<A, C>,
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
///     InstallBirth, MailAddr, Never, NoBirths, User,
/// };
///
/// struct First;
/// struct Second;
///
/// macro_rules! inert {
///     ($child:ty) => {
///         impl Behavior for $child {
///             type Addr = MailAddr;
///             type Msg = Never;
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
/// impl InstallBirth<MailAddr, First, (), Never> for Incomplete {
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

mod sealed {
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
    C: Behavior<Addr = A>,
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
        C: Behavior<Addr = A>,
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
        C: Behavior<Addr = A>,
    {
        self.create(Create::birth(nonce, child))
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

impl<A, Head, Tail, Installer, Output, Error> DispatchBirth<A, Installer, Output, Error>
    for ChildChoice<Head, Tail>
where
    A: Address,
    A::Nonce: Send,
    Head: Behavior<Addr = A> + Send,
    Tail: DispatchBirth<A, Installer, Output, Error> + Send,
    Installer: InstallBirth<A, Head, Output, Error> + Send,
{
    async fn dispatch_birth(
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
            Self::Tail(tail) => tail.dispatch_birth(nonce, kind, installer).await,
        }
    }
}

impl<A, Installer, Output, Error> DispatchBirth<A, Installer, Output, Error> for Never
where
    A: Address,
{
    fn dispatch_birth(
        self,
        _nonce: A::Nonce,
        _kind: CreationKind<A::Nonce>,
        _installer: &mut Installer,
    ) -> impl Future<Output = Result<Output, Error>> + Send {
        async move { match self {} }
    }
}

impl<A, C, Installer, Output, Error> DispatchBirth<A, Installer, Output, Error> for C
where
    A: Address,
    C: Behavior<Addr = A>,
    Installer: InstallBirth<A, C, Output, Error>,
{
    fn dispatch_birth(
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

    impl Behavior for Child {
        type Addr = MailAddr;
        type Msg = u8;
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

    impl InstallBirth<MailAddr, Child, u32, InstallError> for RecordingInstaller {
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
}
