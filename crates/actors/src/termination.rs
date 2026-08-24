//! Typed lifecycle outcomes used by reusable actor compositions.

use crate::Address;

/// The authoritative terminal fact for one exact actor incarnation.
///
/// Successful lifecycle classification and execution failure are disjoint
/// outcomes.  Keeping the complete sum intact prevents compositions from
/// reconstructing provenance from a stop verdict, address reuse, or an
/// adjacent diagnostic.
pub type TerminalOutcome<A> = Result<Exit<A>, Crash>;

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
    StableChildNotAccepted(StableSlotRejection),
    StableChildCreationRejected(crate::CreationRejection),
    WorkerFactoryRejected,
    WorkerCreationRejected(crate::CreationRejection),
}

/// Why a proposed stable child slot was not added to an owned topology.
///
/// This is a composition-time admission result. It is distinct from
/// [`crate::CreationRejection`], which is the interpreter's result after a
/// fresh creation request has actually been staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableSlotRejection {
    /// The creator-local nonce already names a stable slot in this topology.
    DuplicateNonce,
    /// The topology can no longer assign an ordering sequence to another slot.
    SequenceExhausted,
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

/// Ask the interpreter to publish one exact terminal outcome for the
/// emitting incarnation before interpreting the same action's terminal
/// verdict.
///
/// This is a Bombay lifecycle-publication policy, not an actor-model
/// primitive.  It is an explicit effect so a pure composition can propagate
/// an authoritative child or peer fact without placing lifecycle provenance
/// in [`behavior::Step`] or using an ambient runtime side channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportTerminalOutcome<A: Address> {
    pub outcome: TerminalOutcome<A>,
}

impl<A: Address> ReportTerminalOutcome<A> {
    #[must_use]
    pub const fn new(outcome: TerminalOutcome<A>) -> Self {
        Self { outcome }
    }
}

impl<A: Address> behavior::InterpreterRequest for ReportTerminalOutcome<A> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
}
