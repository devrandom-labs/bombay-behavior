//! Semantic values shared only by direct-worker pools.

use behavior::{InterpreterRequests, ReportToParent, SendEffects, SendLayer};

pub(in crate::atomic) mod assignment;
mod customer;
mod policy;
mod shutdown;
pub(in crate::atomic) mod worker;

#[doc(hidden)]
pub use assignment::{AssignWorker, AssignmentReceipt};
pub use assignment::{Assignment, Completion, JobId, SubmissionId};
#[doc(hidden)]
pub use customer::CustomerDelivery;
pub use policy::{BacklogCapacity, Interruption, PoolFailureReaction, PoolRecovery};
pub(in crate::atomic) use policy::{PoolRecoveryState, WorkerRecoveryDecision};
pub(in crate::atomic) use shutdown::ShutdownSequence;

mod completion_capability {
    pub trait Sealed {}
}

/// Static proof that a worker send product owns one assignment completion result.
#[doc(hidden)]
pub trait CompletesAssignments: completion_capability::Sealed + SendEffects {
    /// Worker result transported by the opaque completion.
    type WorkerResult: Send;
}

impl<WorkerResult> completion_capability::Sealed
    for InterpreterRequests<ReportToParent<Completion<WorkerResult>>>
where
    WorkerResult: Send,
{
}

impl<WorkerResult> CompletesAssignments
    for InterpreterRequests<ReportToParent<Completion<WorkerResult>>>
where
    WorkerResult: Send,
{
    type WorkerResult = WorkerResult;
}

impl<Owned, Inner> completion_capability::Sealed for SendLayer<Owned, Inner>
where
    Owned: SendEffects,
    Inner: CompletesAssignments,
{
}

impl<Owned, Inner> CompletesAssignments for SendLayer<Owned, Inner>
where
    Owned: SendEffects,
    Inner: CompletesAssignments,
{
    type WorkerResult = Inner::WorkerResult;
}
