//! The golf target (bombay card #298): the capability machinery whose
//! code-only LOC the concision loop minimizes, plus the 24-point lattice
//! generator that emits one minimal actor per legal configuration.
//!
//! # What lives here (EDIT-freely surface)
//!
//! This crate is the concision loop's target. Phase-1 build (next focused
//! pass, tracked on #298) ports bombay's capability layer in and adds the
//! lattice generator:
//!
//! - **Ported capability machinery** — the `Behavior`-algebra realization of
//!   `Stashing`/`Deadlined`/`Phased`/`Watching`/`Supervising` (ADR-0030), the
//!   surface the loop golfs. It composes onto bombay's runtime (`bombay` is a
//!   dependency for `spawn`/mailbox/etc.); only the capability LAYER is copied
//!   here to be golfable.
//! - **The lattice generator** — one minimal actor per legal point:
//!   cap-set subsets of {Stashing, Deadlined, Phased, Watching, Supervising}
//!   under the composition laws (Supervising ⇒ Watching; Phased ⊥
//!   Stashing/Deadlined) = 15 valid stacks × Phased's inner seats where
//!   present = **24 legal machines**; the **17 illegal** points are trybuild
//!   `compile_fail` cases (laws enforced, not documented).
//!
//! # What defines "done"
//!
//! Trace equality to [`behaviorpass_reference`] at every lattice point
//! (driven by the frozen `behaviorpass-testkit`), the 17 illegal points still
//! failing to compile, and the god-level clippy bar inside the gate (so a line
//! cannot be bought with unreadability). See `.auto/prompt.md`.

// Phase-1 content is intentionally absent — this scaffold ships the frozen
// reference + the .auto contract; the loop grows this crate. The empty lib
// compiles so the workspace + gate are wired end to end from commit one.
