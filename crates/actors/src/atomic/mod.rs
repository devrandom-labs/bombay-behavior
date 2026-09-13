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
mod requests;
mod restart;
mod roster;
mod schedule;
mod stable_proxy;
mod worker;

pub use activation::{ActivationPolicy, EntryCapacity};
/// Derive the one ordinary [`Behavior`](crate::Behavior) implementation for a
/// pool worker from its assignment transition.
///
/// The authored transition receives [`Assignment`] and completes it directly;
/// the generated protocol and parent-completion request remain interpreter
/// machinery rather than application syntax.
///
/// Omitting a required domain declaration is rejected at the attribute:
///
/// ```compile_fail
/// use behavior_actors::atomic::{Assignment, pool_worker};
/// use behavior_actors::{Actions, MailAddr};
/// struct Worker;
/// #[pool_worker(addr = MailAddr)]
/// impl Worker {
///     fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
///         Ok(Actions::cont().with_send(assignment.complete(10_u16)))
///     }
/// }
/// ```
///
/// The transition must consume one assignment rather than an unrelated input:
///
/// ```compile_fail
/// use behavior_actors::atomic::pool_worker;
/// use behavior_actors::MailAddr;
/// struct Worker;
/// #[pool_worker(addr = MailAddr, result = u16)]
/// impl Worker {
///     fn transition(&mut self, job: u8) -> WorkerActed<Self> {
///         loop {}
///     }
/// }
/// ```
pub use behavior_macros::pool_worker;
pub use capacity::ZeroCapacity;
pub use diagnostic::DiagnosticDisposition;
#[doc(hidden)]
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
    AdmissionRejection, AssignedReturnReason, FifoCommand, FifoConstructionRejected, FifoEvent,
    FifoOutcome, FifoOutcomeKind, FifoPool, FifoRequests, QueuedReturnReason, fifo,
};
pub use fixed_supervisor::{
    CapabilityResult, FailureReaction, FixedCommand, FixedConstructionRejected, FixedDiagnostic,
    FixedLifecycle, FixedLifecycleEvent, FixedSnapshot, FixedSupervisor, FixedSupervisorError,
    MemberStatus, ProxyInputFailure, ProxyOutcomeFailure, Recovery, RecoveryDenialReason,
    RecoveryDenied, RestartScheduleFailure, Strategy, UnavailablePhase, WorkerPreparationFailure,
    WorkerPreparationFailureReason, WorkerSource, WorkerUnavailable, fixed,
};
#[doc(hidden)]
pub use fixed_supervisor::{
    FixedBuilder, FixedLifecycleRoute, FixedSupervisorEvent, FixedSupervisorRequests,
    PendingWorkerPreparation, PrepareWorkers, WorkerPreparation,
};
pub use keyed_pool::{
    BindingCapacity, BindingCommand, BindingEvidence, BindingExpectation, BindingGeneration,
    BindingRejection, BindingReply, BindingRequestId, KeyedAdmissionRejection,
    KeyedAssignedReturnReason, KeyedCommand, KeyedConstructionRejected, KeyedDiagnostic,
    KeyedOutcome, KeyedPool, KeyedQueuedReturnReason, keyed,
};
#[doc(hidden)]
pub use keyed_pool::{KeyedError, KeyedEvent, KeyedRequests};
#[doc(hidden)]
pub use pool::{AssignWorker, AssignmentReceipt, CompletesAssignments, CustomerDelivery};
pub use pool::{
    Assignment, BacklogCapacity, Completion, Interruption, JobId, PoolFailureReaction,
    PoolRecovery, SubmissionId,
};
pub use restart::{RestartLimit, RestartRelease, RestartReleaseError, RestartReleaseFailure};
pub(crate) use roster::RoleName;
pub use roster::{DuplicateRole, OrderedRoles};
pub use stable_proxy::{
    InitialWorkerOutcome, ProxyDiagnostic, ProxyOutcome, ProxyPhase, ReplacementOutcome,
    StableProxy,
};
#[doc(hidden)]
pub use stable_proxy::{
    ProxyControl, ProxyDrain, ProxyEffects, ProxyInputReceipt, ProxyInputResult, ProxyOperation,
    ProxyOperationId, WorkerStartResult,
};
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
