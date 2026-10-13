//! Semantic values shared only by direct-worker pools.

use behavior::{InterpreterRequests, ReportToParent, SendEffects, SendLayer};

pub(in crate::templates) mod assignment;
mod customer;
mod policy;
pub mod queue;
pub mod router;
mod shutdown;
pub(in crate::templates) mod worker;

pub use assignment::{AssignWorker, AssignmentReceipt};
pub use assignment::{Assignment, Completion, JobId, SubmissionId};
#[doc(hidden)]
pub use customer::CustomerDelivery;
pub use policy::{BacklogCapacity, Interruption, PoolFailureReaction, PoolRecovery};
pub(in crate::templates) use policy::{PoolRecoveryState, WorkerRecoveryDecision};
pub(in crate::templates) use shutdown::ShutdownSequence;

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

/// Derive the one ordinary [`Behavior`](behavior::Behavior) implementation for a
/// pool worker from its assignment transition.
///
/// The authored transition receives [`Assignment`] and completes it directly;
/// the generated protocol and parent-completion request remain interpreter
/// machinery rather than application syntax.
///
/// Omitting a required domain declaration is rejected at the attribute:
///
/// ```compile_fail
/// struct Worker;
/// #[behavior_actors::pool::pool_worker(addr = behavior::MailAddr)]
/// impl Worker {
///     fn transition(&mut self, assignment: behavior_actors::pool::Assignment<u8>) -> WorkerActed<Self> {
///         Ok(behavior::Actions::cont().with_send(assignment.complete(10_u16)))
///     }
/// }
/// ```
///
/// The transition must consume one assignment rather than an unrelated input:
///
/// ```compile_fail
/// struct Worker;
/// #[behavior_actors::pool::pool_worker(addr = behavior::MailAddr, result = u16)]
/// impl Worker {
///     fn transition(&mut self, job: u8) -> WorkerActed<Self> {
///         loop {}
///     }
/// }
/// ```
pub use behavior_macros::pool_worker;
