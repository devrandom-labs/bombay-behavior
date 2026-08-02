//! The golf target (bombay card #298): a fresh ASYNC realization of the
//! ADR-0030 Behavior algebra, spawnable on plain tokio primitives, whose
//! code-only LOC the concision loop minimizes while staying trace-equal to
//! the frozen sync reference at every lattice point.
//!
//! bombay is depended on ONLY for the shared verdict words (`Step`/`Never` —
//! this crate's own grammar otherwise); the driver and channels are
//! plain tokio (the ceremony a concision harness must not carry). The shared
//! [`Exit`] trace vocabulary comes from the frozen reference so a SUT trace
//! compares type-exact with a model trace.
//!
//! Everything here is ONE kind of thing — a [`Behavior`]. `Base` is the
//! innermost; every capability is a `Behavior` that wraps a `Behavior`. One
//! file per capability so the module name says exactly what it holds.

mod behavior; // the core: Behavior, Envelope, Become, run, lift, Base
mod deadlined; // Deadlined
mod phased; // Phased
mod stashing; // Stashing
mod supervising; // Supervising
mod watching; // Watching

pub use behavior::{Base, Become, Behavior, Envelope, run};
pub use deadlined::{DeadlineReaction, Deadlined};
pub use phased::{Admit, Phased};
pub use stashing::{StashRoute, Stashing};
pub use supervising::{Child, Supervising};
pub use watching::{LinkReaction, Watching, otp_propagation};

/// The shared trace-exit vocabulary, re-exported from the frozen reference.
pub use behaviorpass_reference::Exit;
