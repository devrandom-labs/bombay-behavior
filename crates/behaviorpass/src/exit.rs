//! `Exit` — the trace-exit vocabulary: the `R` corner of the verdict family
//! (`Step<Ph, Exit>`, ADR-0029) that a stopped fold rides out on. Moved
//! in-crate when the `behaviorpass-reference` crate was retired; it is the
//! core's own vocabulary now, not a shared one.

/// How a fold ends. The `R` parameter of the become verdict (`Step<Ph, Exit>`):
/// a `Stop` carries one of these; the driver also mints `Collected` when the
/// mailbox drains with no self-stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Clean self-stop (`Flow::Stop(Normal)`'s image).
    Normal,
    /// Sources exhausted — the mailbox-closed / ref-count-collection image.
    Collected,
    /// A watch layer propagated a linked peer's death, carrying its id.
    LinkDied(u64),
}
