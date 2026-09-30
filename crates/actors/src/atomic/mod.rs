//! Atomic actor families with one independent behavior per aggregate.

mod activation;
mod capacity;
mod diagnostic;
mod drain;
mod dynamic_supervisor;
mod fifo_pool;
mod fixed_supervisor;
mod keyed_pool;
mod pool;
mod proxy_creation;
mod restart;
mod roster;
mod schedule;
mod stable_proxy;
mod worker;

pub use activation::{ActivationPolicy, EntryCapacity};
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
/// #[behavior_actors::atomic::pool_worker(addr = behavior::MailAddr)]
/// impl Worker {
///     fn transition(&mut self, assignment: behavior_actors::atomic::Assignment<u8>) -> WorkerActed<Self> {
///         Ok(behavior::Actions::cont().with_send(assignment.complete(10_u16)))
///     }
/// }
/// ```
///
/// The transition must consume one assignment rather than an unrelated input:
///
/// ```compile_fail
/// struct Worker;
/// #[behavior_actors::atomic::pool_worker(addr = behavior::MailAddr, result = u16)]
/// impl Worker {
///     fn transition(&mut self, job: u8) -> WorkerActed<Self> {
///         loop {}
///     }
/// }
/// ```
pub use behavior_macros::pool_worker;
pub use capacity::ZeroCapacity;
pub use diagnostic::DiagnosticDisposition;
pub use diagnostic::{DiagnosticAccepted, DiagnosticAction, DiagnosticRoute};
pub use drain::ActorDrainPolicy;
pub use dynamic_supervisor::{
    CancelAuthority, CancellationOutcome, CancellationReceipt, DynamicCommand, DynamicDiagnostic,
    DynamicLifecycle, DynamicStatus, DynamicSupervisor, EntryRetirement, EntryStopFailure,
    EntryStopFailureReason, InterruptedWorker, QueryReply, ReplaceRejection, ReplacementFailure,
    StartRejection, StopRejection, UnexpectedExit, WorkerChange, WorkerChangeInterruption,
    WorkerChangeReceipt, WorkerChangeRejection, dynamic,
};
#[doc(hidden)]
pub use dynamic_supervisor::{DynamicSupervisorEvent, DynamicSupervisorRequests};
pub use fifo_pool::{
    AdmissionRejection, AssignedReturnReason, FifoCommand, FifoConstructionRejected, FifoError,
    FifoEvent, FifoOutcome, FifoOutcomeKind, FifoPool, FifoRequests, QueuedReturnReason, fifo,
};
pub use fixed_supervisor::{
    CapabilityResult, FailureReaction, FixedBuilder, FixedCommand, FixedConstructionRejected,
    FixedDiagnostic, FixedLifecycle, FixedLifecycleEvent, FixedSnapshot, FixedSupervisor,
    FixedSupervisorError, MemberStatus, ProxyInputFailure, ProxyOutcomeFailure, Recovery,
    RecoveryDenialReason, RecoveryDenied, RestartScheduleFailure, Strategy, UnavailablePhase,
    WorkerPreparationFailure, WorkerPreparationFailureReason, WorkerSource, WorkerUnavailable,
    fixed,
};
#[doc(hidden)]
pub use fixed_supervisor::{FixedLifecycleRoute, FixedSupervisorEvent, FixedSupervisorRequests};
pub use fixed_supervisor::{PendingWorkerPreparation, PrepareWorkers, WorkerPreparation};
pub use keyed_pool::{
    BindingCapacity, BindingCommand, BindingEvidence, BindingExpectation, BindingGeneration,
    BindingRejection, BindingReply, BindingRequestId, KeyedAdmissionRejection,
    KeyedAssignedReturnReason, KeyedCommand, KeyedConstructionRejected, KeyedDiagnostic,
    KeyedError, KeyedOutcome, KeyedPool, KeyedQueuedReturnReason, keyed,
};
#[doc(hidden)]
pub use keyed_pool::{KeyedEvent, KeyedRequests};
#[doc(hidden)]
pub use pool::CompletesAssignments;
pub use pool::CustomerDelivery;
pub use pool::{AssignWorker, AssignmentReceipt};
pub use pool::{
    Assignment, BacklogCapacity, Completion, Interruption, JobId, PoolFailureReaction,
    PoolRecovery, SubmissionId,
};
pub use restart::{RestartLimit, RestartRelease, RestartReleaseError, RestartReleaseFailure};
pub(crate) use roster::RoleName;
pub use roster::{DuplicateRole, OrderedRoles};
pub(crate) use stable_proxy::ProxyOperationId;
pub use stable_proxy::{
    InitialWorkerOutcome, ProxyControlAdmission, ProxyDiagnostic, ProxyOutcome, ProxyPhase,
    ReplacementOutcome, StableProxy,
};
pub use stable_proxy::{ProxyControl, ProxyInputReceipt, ProxyInputResult, ProxyOperation};
#[doc(hidden)]
pub use stable_proxy::{ProxyDrain, ProxyEffects, WorkerStartResult};
#[doc(hidden)]
pub use worker::{
    ActivationPermit, ActivationStartRejection, BeginActivation, InitializationAttempt,
    InitializeWorker, WorkerActivation, WorkerAttempt, WorkerInitializationOutcome,
    WorkerInitializationReport,
};
pub use worker::{
    ActivationPlan, ImmediateActivation, InitialWorkerRejection, PreparedWorker,
    WorkerCreationRejection, WorkerInitializationFailure, WorkerRecovery, WorkerSubmission,
};
