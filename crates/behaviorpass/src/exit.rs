//! `Exit` — the trace-exit vocabulary: the `R` corner of the verdict family
//! (`Step<Ph, Exit>`, ADR-0029) that a stopped fold rides out on. Moved
//! in-crate when the `behaviorpass-reference` crate was retired; it is the
//! core's own vocabulary now, not a shared one.

use crate::behavior::Address;

/// How a fold ends. The `R` parameter of the become verdict (`Step<Ph, Exit>`):
/// a `Stop` carries one of these; the driver also mints `Collected` when the
/// mailbox drains with no self-stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit<A: Address> {
    /// Clean self-stop (`Flow::Stop(Normal)`'s image).
    Normal,
    /// Sources exhausted — the mailbox-closed / ref-count-collection image.
    Collected,
    /// A watch layer propagated a linked peer's death, carrying its address.
    LinkDied(A),
}

/// How the fold crashed. The reason VALUE stays with the driver in both
/// cases (heterogeneous error types and panic payloads are runtime
/// plumbing — typed reasons are the homogeneous-fleet door, deferred);
/// the fold receives the DOMAIN. Both variants classify as abnormal; the
/// distinction is preserved for future policy (e.g. poison-message
/// handling) and for trace truth — the death site knows which one
/// happened, and the report must not collapse it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crash {
    /// `step` returned `Err` — the behavior's declared controlled crash.
    Failed,
    /// The fold panicked — an undeclared programmer bug.
    Panicked,
}
