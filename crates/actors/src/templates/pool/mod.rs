//! Semantic values shared only by direct-worker pools.

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
pub use policy::{
    BacklogCapacity, Interruption, PoolFailureReaction, PoolRecovery, RecoveryPolicy,
};
pub(in crate::templates) use policy::{PoolRecoveryState, WorkerRecoveryDecision};
pub(in crate::templates) use shutdown::ShutdownSequence;

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
