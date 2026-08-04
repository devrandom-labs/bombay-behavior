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

mod behavior; // the core: Address, Behavior, Envelope, Become, Target, run, lift, Base
mod verdict; // Never, Step — owned verdict vocabulary (no upward deps)
mod deadlined; // Deadlined
mod stashing; // Stashing — the buffer primitive (STAYS in core)
mod supervising; // Supervising — decides the FULL strategy space as pure fold decisions
mod watching; // Watching

// `Fsm` is NOT a core capability — it's a thin helper (a state machine built
// from core, using the Stash buffer). `Phased`/`Admit` left with it: phases are
// the aggregate's (nexus's) concern, not core's.
mod fsm;

pub use behavior::{
    Actions, Address, Base, Become, Behavior, Create, Envelope, MailAddr, Target, Transcript, run,
};
pub use deadlined::{DeadlineReaction, Deadlined};
pub use fsm::{Fsm, Move};
pub use stashing::{StashRoute, Stashing};
pub use supervising::{RestartPolicy, Strategy, Supervising};
pub use verdict::{Never, Step};
pub use watching::{LinkReaction, Watching, stop_on_abnormal_death};

mod exit; // Exit, Crash — the trace-exit vocabulary (moved in-crate; reference crate retired)

pub use exit::{Crash, Exit};
