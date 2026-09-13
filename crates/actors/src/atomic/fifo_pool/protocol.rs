//! FIFO submission, assignment, completion, and customer outcomes.

use behavior::{Address, BehaviorAddr, EndpointAddress, MessageProtocol};

use crate::ReplyRoute;
use crate::atomic::RoleName;

use super::super::pool::worker::{CurrentWorker, WorkerPreparationError, WorkerReplacementError};
use super::super::pool::{Assignment, JobId, SubmissionId};
use super::super::worker::{ActivationAttempt, WorkerActivationOutcome};
use super::super::{
    ActivationPermit, ActivationPlan, WorkerAttempt, WorkerCreationRejection, WorkerSource,
    WorkerSubmission,
};
use super::{FifoEvent, PrepareWorkers};

/// Exact reason a submitted job never entered pool ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionRejection {
    /// No direct worker can serve now or recover later.
    NoRecoverableWorkers,
    /// Waiting capacity is full and no worker is ready for immediate assignment.
    BacklogFull,
    /// The accepted-job sequence cannot issue another non-reused correlation.
    JobCorrelationUnavailable,
    /// Immediate assignment cannot issue another non-reused correlation.
    AssignmentCorrelationUnavailable,
    /// The pool has closed admission.
    ShuttingDown,
}

/// Exact reason a queued job returned without assignment completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueuedReturnReason {
    /// Every direct worker retired without a recoverable successor.
    NoRecoverableWorkers,
    /// Assignment correlation could not be issued while a worker was ready.
    AssignmentCorrelationUnavailable,
    /// Pool shutdown extracted the waiting job.
    PoolShutdown,
}

/// Exact reason an assigned job returned without a completed result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssignedReturnReason {
    /// The exact assigned worker stopped and interruption policy selected failure.
    WorkerStopped,
    /// A retried job could not reserve fresh assignment authority.
    RetryPreparationRejected,
    /// Pool shutdown extracted the assigned customer obligation.
    PoolShutdown,
    /// Delivery returned authority that contradicted an already received completion.
    ContradictoryAssignmentSettlement,
}

enum CustomerOutcome<Role, Job, WorkerResult> {
    Accepted {
        submission: SubmissionId,
        job: JobId,
    },
    Rejected {
        submission: SubmissionId,
        payload: Job,
        reason: AdmissionRejection,
    },
    Completed {
        job: JobId,
        role: RoleName<Role>,
        worker_result: WorkerResult,
    },
    ReturnedQueued {
        job: JobId,
        payload: Job,
        reason: QueuedReturnReason,
    },
    ReturnedAssigned {
        job: JobId,
        role: RoleName<Role>,
        payload: Job,
        reason: AssignedReturnReason,
    },
}

/// Customer-visible alternative selected by one FIFO outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FifoOutcomeKind {
    Accepted,
    Rejected,
    Completed,
    ReturnedQueued,
    ReturnedAssigned,
}

/// Complete customer-visible progress or terminal outcome for FIFO work.
///
/// The value is opaque so a pool can retain a non-`Clone` application role
/// while an outcome borrows the same immutable role name. Exact consuming
/// projections return owned payloads and results without exposing shared
/// storage.
pub struct FifoOutcome<Role, Job, WorkerResult> {
    outcome: CustomerOutcome<Role, Job, WorkerResult>,
}

impl<Role, Job, WorkerResult> FifoOutcome<Role, Job, WorkerResult> {
    pub(super) const fn accepted(submission: SubmissionId, job: JobId) -> Self {
        Self {
            outcome: CustomerOutcome::Accepted { submission, job },
        }
    }

    pub(super) const fn rejected(
        submission: SubmissionId,
        payload: Job,
        reason: AdmissionRejection,
    ) -> Self {
        Self {
            outcome: CustomerOutcome::Rejected {
                submission,
                payload,
                reason,
            },
        }
    }

    pub(super) const fn completed(
        job: JobId,
        role: RoleName<Role>,
        worker_result: WorkerResult,
    ) -> Self {
        Self {
            outcome: CustomerOutcome::Completed {
                job,
                role,
                worker_result,
            },
        }
    }

    pub(super) const fn returned_queued(
        job: JobId,
        payload: Job,
        reason: QueuedReturnReason,
    ) -> Self {
        Self {
            outcome: CustomerOutcome::ReturnedQueued {
                job,
                payload,
                reason,
            },
        }
    }

    pub(super) const fn returned_assigned(
        job: JobId,
        role: RoleName<Role>,
        payload: Job,
        reason: AssignedReturnReason,
    ) -> Self {
        Self {
            outcome: CustomerOutcome::ReturnedAssigned {
                job,
                role,
                payload,
                reason,
            },
        }
    }

    /// Identify the customer-visible alternative without consuming it.
    #[must_use]
    pub const fn kind(&self) -> FifoOutcomeKind {
        match &self.outcome {
            CustomerOutcome::Accepted { .. } => FifoOutcomeKind::Accepted,
            CustomerOutcome::Rejected { .. } => FifoOutcomeKind::Rejected,
            CustomerOutcome::Completed { .. } => FifoOutcomeKind::Completed,
            CustomerOutcome::ReturnedQueued { .. } => FifoOutcomeKind::ReturnedQueued,
            CustomerOutcome::ReturnedAssigned { .. } => FifoOutcomeKind::ReturnedAssigned,
        }
    }

    /// Borrow the semantic worker role carried by a terminal assigned outcome.
    #[must_use]
    pub fn role(&self) -> Option<&Role> {
        match &self.outcome {
            CustomerOutcome::Completed { role, .. }
            | CustomerOutcome::ReturnedAssigned { role, .. } => Some(role.role()),
            CustomerOutcome::Accepted { .. }
            | CustomerOutcome::Rejected { .. }
            | CustomerOutcome::ReturnedQueued { .. } => None,
        }
    }

    /// Borrow a returned customer payload, when present.
    #[must_use]
    pub const fn payload(&self) -> Option<&Job> {
        match &self.outcome {
            CustomerOutcome::Rejected { payload, .. }
            | CustomerOutcome::ReturnedQueued { payload, .. }
            | CustomerOutcome::ReturnedAssigned { payload, .. } => Some(payload),
            CustomerOutcome::Accepted { .. } | CustomerOutcome::Completed { .. } => None,
        }
    }

    /// Borrow a completed worker result, when present.
    #[must_use]
    pub const fn worker_result(&self) -> Option<&WorkerResult> {
        match &self.outcome {
            CustomerOutcome::Completed { worker_result, .. } => Some(worker_result),
            CustomerOutcome::Accepted { .. }
            | CustomerOutcome::Rejected { .. }
            | CustomerOutcome::ReturnedQueued { .. }
            | CustomerOutcome::ReturnedAssigned { .. } => None,
        }
    }

    /// Consume an accepted-admission outcome.
    pub fn into_accepted(self) -> core::result::Result<(SubmissionId, JobId), Self> {
        match self.outcome {
            CustomerOutcome::Accepted { submission, job } => Ok((submission, job)),
            outcome => Err(Self { outcome }),
        }
    }

    /// Consume a rejected-admission outcome and recover its complete payload.
    pub fn into_rejected(
        self,
    ) -> core::result::Result<(SubmissionId, Job, AdmissionRejection), Self> {
        match self.outcome {
            CustomerOutcome::Rejected {
                submission,
                payload,
                reason,
            } => Ok((submission, payload, reason)),
            outcome => Err(Self { outcome }),
        }
    }

    /// Consume a completed outcome after borrowing its role if needed.
    pub fn into_completed(self) -> core::result::Result<(JobId, WorkerResult), Self> {
        match self.outcome {
            CustomerOutcome::Completed {
                job, worker_result, ..
            } => Ok((job, worker_result)),
            outcome => Err(Self { outcome }),
        }
    }

    /// Consume a returned queued outcome and recover its complete payload.
    pub fn into_returned_queued(
        self,
    ) -> core::result::Result<(JobId, Job, QueuedReturnReason), Self> {
        match self.outcome {
            CustomerOutcome::ReturnedQueued {
                job,
                payload,
                reason,
            } => Ok((job, payload, reason)),
            outcome => Err(Self { outcome }),
        }
    }

    /// Consume a returned assigned outcome after borrowing its role if needed.
    pub fn into_returned_assigned(
        self,
    ) -> core::result::Result<(JobId, Job, AssignedReturnReason), Self> {
        match self.outcome {
            CustomerOutcome::ReturnedAssigned {
                job,
                payload,
                reason,
                ..
            } => Ok((job, payload, reason)),
            outcome => Err(Self { outcome }),
        }
    }
}

/// Application commands accepted by one FIFO pool.
pub enum FifoCommand<A, Role, Job, WorkerResult>
where
    A: Address + EndpointAddress,
{
    /// Submit one payload and its customer correlation and reply capability.
    Submit {
        /// Customer-authored admission correlation.
        submission: SubmissionId,
        /// Payload retained by the pool while accepted.
        payload: Job,
        /// Temporary typed customer capability.
        customer: ReplyRoute<MessageProtocol<A, FifoOutcome<Role, Job, WorkerResult>>>,
    },
    /// Close admission and retire the complete owned actor graph.
    Shutdown,
}

impl<A, Role, Job, WorkerResult> FifoCommand<A, Role, Job, WorkerResult>
where
    A: Address + EndpointAddress,
{
    /// Construct one complete submission without exposing pool internals.
    #[must_use]
    pub fn submit<Route>(submission: SubmissionId, payload: Job, customer: Route) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, FifoOutcome<Role, Job, WorkerResult>>>>,
    {
        Self::Submit {
            submission,
            payload,
            customer: customer.into(),
        }
    }

    /// Construct shutdown without a reply placeholder.
    #[must_use]
    pub const fn shutdown() -> Self {
        Self::Shutdown
    }
}

#[expect(
    dead_code,
    reason = "diagnostic disposition transfers complete affine values to their next owner"
)]
pub(super) enum FifoDiagnosticCause<Role, W, P, Source, Job, WorkerResult>
where
    Role: Send + Sync,
    W: behavior::Behavior + Send,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    Unexpected(
        FifoEvent<
            Role,
            W,
            P,
            Job,
            WorkerResult,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        >,
    ),
    WorkerReturned {
        role: RoleName<Role>,
        rejection: WorkerCreationRejection<W>,
        activation: P,
        stopped: Option<crate::ChildStopped<BehaviorAddr<W>>>,
    },
    WorkersReturned(behavior::CreationsSettled<BehaviorAddr<W>, crate::StopOnShutdown<W>>),
    UnusedActivation {
        role: RoleName<Role>,
        permit: Option<ActivationPermit<W>>,
        activation: P,
    },
    StoppedWorkerEstablished {
        role: RoleName<Role>,
        worker: CurrentWorker<W>,
        activation: P,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
    },
    UnmatchedWorkerReturn {
        id: behavior::CreationId,
        kind: behavior::CreationKind,
        rejection: WorkerCreationRejection<W>,
    },
    WorkerActivationReturned {
        role: RoleName<Role>,
        worker: WorkerAttempt,
        activation: ActivationAttempt,
        outcome: WorkerActivationOutcome<W, P>,
    },
    WorkerShutdownRejected {
        role: RoleName<Role>,
        shutdown: crate::EstablishedShutdownResolved<W::Protocol>,
    },
    WorkerPreparationFailed {
        role: RoleName<Role>,
        previous: super::super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        returned_source: Option<Source>,
        error: WorkerPreparationError<Source::WorkerRejection, Source::SourceRejection>,
    },
    WorkerReplacementFailed {
        role: RoleName<Role>,
        previous: super::super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        submission: WorkerSubmission<W, P>,
        returned_source: Option<Source>,
        error: WorkerReplacementError,
    },
}

/// Complete FIFO input or precise aggregate failure selected for diagnostics.
///
/// The value is opaque so runtime correlation and private role-sharing evidence
/// cannot become application construction syntax. The selected diagnostic route
/// or Bombay's terminal custodian receives the complete owned value; applications
/// can borrow the associated semantic role when one exists.
pub struct FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>
where
    Role: Send + Sync,
    W: behavior::Behavior + Send,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    cause: FifoDiagnosticCause<Role, W, P, Source, Job, WorkerResult>,
}

impl<Role, W, P, Source, Job, WorkerResult> FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>
where
    Role: Send + Sync,
    W: behavior::Behavior + Send,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    pub(super) const fn new(
        cause: FifoDiagnosticCause<Role, W, P, Source, Job, WorkerResult>,
    ) -> Self {
        Self { cause }
    }

    /// Borrow the semantic role associated with a role-specific failure.
    #[must_use]
    pub fn role(&self) -> Option<&Role> {
        match &self.cause {
            FifoDiagnosticCause::WorkerReturned { role, .. }
            | FifoDiagnosticCause::UnusedActivation { role, .. }
            | FifoDiagnosticCause::StoppedWorkerEstablished { role, .. }
            | FifoDiagnosticCause::WorkerActivationReturned { role, .. }
            | FifoDiagnosticCause::WorkerShutdownRejected { role, .. }
            | FifoDiagnosticCause::WorkerPreparationFailed { role, .. }
            | FifoDiagnosticCause::WorkerReplacementFailed { role, .. } => Some(role.role()),
            FifoDiagnosticCause::Unexpected(_)
            | FifoDiagnosticCause::WorkersReturned(_)
            | FifoDiagnosticCause::UnmatchedWorkerReturn { .. } => None,
        }
    }
}

impl<Role, W, P, Source, Job, WorkerResult> core::fmt::Debug
    for FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>
where
    Role: Send + Sync,
    W: behavior::Behavior + Send,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FifoDiagnostic")
            .finish_non_exhaustive()
    }
}
