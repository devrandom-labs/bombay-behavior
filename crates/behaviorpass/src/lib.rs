//! The golf target (bombay card #298): a fresh ASYNC realization of the
//! ADR-0030 Behavior algebra, spawnable on plain tokio primitives, whose
//! code-only LOC the concision loop minimizes while staying trace-equal to
//! the frozen sync reference at every lattice point.
//!
//! bombay is depended on ONLY for the verdict vocabulary
//! (`Step`/`Never`/`Disposition`/`Deferred`); the driver and channels are
//! plain tokio (the ceremony a concision harness must not carry). The shared
//! [`Exit`] trace vocabulary comes from the frozen reference so a SUT trace
//! compares type-exact with a model trace.
//!
//! Phase-1 build order (see `docs/phase-1-plan.md`): the async driver (here) →
//! the five async layers → the lattice generator. The loop takes over once the
//! frozen oracle is green at all 24 points.

mod behavior;

pub use behavior::{Behavior, run};

/// The shared trace-exit vocabulary, re-exported from the frozen reference.
pub use behaviorpass_reference::Exit;
