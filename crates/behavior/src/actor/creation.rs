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
/// protocol it can install. Heterogeneous birth sums require all applicable
/// implementations through generated bounds, so unsupported variants fail to
/// compile instead of falling through to a registry or erased path.
pub trait InstallBirth<A: Address, C: Behavior<Addr = A>, Output, Error> {
    /// Install and commit exactly the supplied concrete creation.
    ///
    /// # Errors
    /// Returns the interpreter's typed allocation, initialization, or commit
    /// failure without binding the requested nonce.
    fn install_birth(
        &mut self,
        creation: Create<A, C>,
    ) -> impl Future<Output = Result<Output, Error>>;
}

/// Exhaustive static dispatch of one creation-only child sum.
///
/// This is an interpreter-facing derived construction, not another actor-model
/// operation. Implementations must preserve the nonce and provenance and call
/// exactly one concrete [`InstallBirth`] implementation. The `#[births]`
/// attribute generates this implementation for a closed enum.
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
    ) -> impl Future<Output = Result<Output, Error>>;
}

impl<A, Installer, Output, Error> DispatchBirth<A, Installer, Output, Error> for Never
where
    A: Address,
{
    async fn dispatch_birth(
        self,
        _nonce: A::Nonce,
        _kind: CreationKind<A::Nonce>,
        _installer: &mut Installer,
    ) -> Result<Output, Error> {
        match self {}
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
