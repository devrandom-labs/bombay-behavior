//! The FROZEN mode-blind oracle (bombay card #298), built on the #266 /
//! `phase_equivalence` pattern: one abstract script drives BOTH a generated
//! SUT actor and the [`behaviorpass_reference`] fold, and their observable
//! probe sequences plus stop kinds must be identical. Divergence is a bug by
//! definition.
//!
//! # Phase-1 content (next focused pass, tracked on #298)
//!
//! - **Probe vocabulary** — the mode-blind observable alphabet (`Applied`,
//!   `Processed`, `Refused`, `ShedFull`, `TimedOut`, `StaleTimeoutLeaked`, …):
//!   only what USER code can observe on both the model and the real actor.
//! - **Per-axis suites**, run at every lattice point that HAS the axis:
//!   FIFO + exactly-once at all 24; defer/replay laws at `Bounded` points;
//!   fires-once/anchor/left-phase at armed points; death/restart choreography
//!   at Watching/Supervising points; proptest boundaries on every knob
//!   (capacity 1/max, deadline 0/beyond-representable).
//! - **The equivalence macro** — `behavior_suite!($sut)` instantiating the
//!   same test bodies against the reference fold and a generated actor
//!   (mirrors fastpass's `property_suite!`), each awaited `recv` under a
//!   timeout so a hung SUT FAILS rather than stalling the measure loop.
//!
//! Frozen from commit one so the loop optimizes the SUT, never the oracle
//! (`.auto/checks.sh` enforces the freeze via a BASELINE diff).

/// Re-export the reference model so a subject crate imports the whole oracle
/// surface (fold + layers + exit vocabulary) from one path.
pub use behaviorpass_reference as reference;
