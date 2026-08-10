//! Staged fresh-actor creation capabilities.

use core::marker::PhantomData;

use super::addressing::Address;
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
