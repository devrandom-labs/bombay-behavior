//! `Exit` — the trace-exit vocabulary: the `R` corner of the verdict family
//! (`Step<Ph, Exit>`, ADR-0029) that a stopped fold rides out on. Moved
//! in-crate when the `behavior-reference` crate was retired; it is the
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
    /// A supervision layer stopped because it could no longer preserve its
    /// child topology.
    SupervisionFailed(SupervisionFailureReason),
}

/// Why a supervisor could no longer preserve its child topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionFailureReason {
    /// A restart policy admitted the worker termination, but a later restart
    /// constraint denied the requested replacement set.
    RestartDenied(RestartDenial),
    /// The stable proxy itself stopped and therefore cannot accept a fresh
    /// worker incarnation at its existing address.
    StableChildStopped,
}

/// Why an otherwise eligible replacement set was denied.
///
/// The vocabulary is deliberately exhaustive. Additional restart gates must
/// expose their concrete denial here (or in a future statically composed sum)
/// rather than hiding it behind an open or erased reason type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDenial {
    /// The replacement set would exceed the configured restart budget.
    BudgetExceeded {
        /// Replacement attempts currently retained in the restart window.
        restarts_in_window: usize,
        /// Number of workers the selected strategy would replace.
        replacements_requested: usize,
        /// Configured maximum replacements in one window.
        maximum_restarts: u32,
    },
}

/// Why actor execution terminated abnormally.
///
/// The reason value stays with the interpreter: heterogeneous behavior and
/// environment errors, panic payloads, and executor cancellation details are
/// runtime plumbing. Observation carries only the statically known terminal
/// domain. Every variant is abnormal; the distinction is preserved for
/// supervision policy and truthful traces.
///
/// This classification is Bombay policy layered over the actor algebra. It
/// does not add an effect to a behavior transition: interpreters mint a
/// `Crash` only when execution terminates without an [`Exit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crash {
    /// `Behavior::init` or `Behavior::step` returned its declared error.
    Failed,
    /// The interpreter could not execute an emitted effect and terminated the
    /// actor.
    EnvironmentFailed,
    /// Actor execution unwound through a panic.
    Panicked,
    /// The executor cancelled actor execution before normal completion.
    Cancelled,
}
