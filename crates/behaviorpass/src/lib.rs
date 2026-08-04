//! The behavior algebra: the pure fold at the heart of the pass family.
//! A [`Behavior`] folds an [`Envelope`] into [`Actions`] — sends, creates,
//! and the replacement verdict, all DATA, no I/O — and every capability is
//! a `Behavior` that wraps a `Behavior`. The floor is a [`State`]: a state
//! type with its transition, bound in one type (the coalgebra); `Base`
//! lifts it into the fold. One file per capability so the module name says
//! exactly what it holds. Trace-equality to the frozen reference at every
//! lattice point is the correctness gate (tests/oracle.rs).

// The `workers!` macro emits `::behaviorpass::…` paths; this alias lets
// those expansions resolve inside this crate too (macro hygiene).
extern crate self as behaviorpass;

mod behavior; // the core: Address, Behavior, State, Base, FnState, Envelope, Actions, run
mod verdict; // Never, Step — owned verdict vocabulary (no upward deps)
mod deadlined; // Deadlined
mod stashing; // Stashing — the buffer primitive (STAYS in core)
mod supervising; // Supervising — decides the FULL strategy space as pure fold decisions
mod watching; // Watching

// `Fsm` is NOT a core capability — it's a thin helper (a state machine built
// from core, using the Stash buffer). `Phased`/`Admit` left with it: phases are
// the aggregate's (nexus's) concern, not core's.
mod fsm;
mod spec; // Spec — the intent builder over the capability stack

pub use behavior::{
    Actions, Acted, Address, Base, Become, Behavior, Create, Envelope, Fleet, FnState, MailAddr,
    State, Target, Transcript, run,
};
pub use deadlined::{DeadlineReaction, Deadlined};
pub use fsm::{Fsm, Move};
pub use spec::Spec;
pub use stashing::{StashRoute, Stashing};
pub use supervising::{RestartPolicy, Strategy, Supervising, restart_all, restart_one, restart_rest};
pub use verdict::{Never, Step};
pub use watching::{LinkReaction, Watching, stop_on_abnormal_death};

mod exit; // Exit, Crash — the trace-exit vocabulary (moved in-crate; reference crate retired)

pub use exit::{Crash, Exit};

pub use behaviorpass_macros::workers;
