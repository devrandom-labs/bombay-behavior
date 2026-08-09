//! Staged fresh-actor creation capabilities.

use core::marker::PhantomData;

use super::addressing::Address;
use crate::verdict::Never;

/// Behavior-owned provenance for a staged fresh actor creation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationKind {
    /// An initial or ordinary later birth.
    Birth,
    /// A fresh successor incarnation requested by a replacement protocol.
    ReplacementIncarnation,
}

/// A staged request to establish a fresh child at a creator-local nonce.
///
/// The nonce is a routing and correlation key, not an actor identity or proof
/// of freshness. Creation and its [`CreationKind`] become runtime facts only
/// after an interpreter successfully installs the fresh actor and commits the
/// child binding. Replacement at an existing address is deliberately absent;
/// stable identity is derived with a proxy actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Create<A: Address, New> {
    pub nonce: A::Nonce,
    pub child: New,
    pub kind: CreationKind,
}

impl<A: Address, New> Create<A, New> {
    #[must_use]
    pub const fn birth(nonce: A::Nonce, child: New) -> Self {
        Self {
            nonce,
            child,
            kind: CreationKind::Birth,
        }
    }

    #[must_use]
    pub const fn replacement_incarnation(nonce: A::Nonce, child: New) -> Self {
        Self {
            nonce,
            child,
            kind: CreationKind::ReplacementIncarnation,
        }
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
