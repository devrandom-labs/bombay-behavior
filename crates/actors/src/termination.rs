//! Typed lifecycle outcomes used by reusable actor compositions.

use crate::Address;

/// A successfully observed actor termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit<A: Address> {
    /// The behavior explicitly designated termination.
    Normal,
    /// The Bombay runtime collected an actor after its sources were exhausted.
    Collected,
    /// A watch composition propagated a linked peer's death.
    LinkDied(A),
    /// A supervisor could no longer preserve its child topology.
    SupervisionFailed(SupervisionFailureReason),
}

/// Why a supervisor could no longer preserve its child topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionFailureReason {
    RestartDenied(RestartDenial),
    StableChildStopped,
}

/// Why an otherwise eligible replacement set was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDenial {
    BudgetExceeded {
        restarts_in_window: usize,
        replacements_requested: usize,
        maximum_restarts: u32,
    },
}

/// Why execution terminated without a behavior-requested stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crash {
    Failed,
    EnvironmentFailed,
    Panicked,
    Cancelled,
}
