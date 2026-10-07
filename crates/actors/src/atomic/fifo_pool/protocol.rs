//! FIFO submission, assignment, completion, and customer outcomes.

use std::sync::Arc;

use behavior::{Address, BehaviorAddr, EndpointAddress, MessageProtocol};

use crate::atomic::RoleName;
use crate::{ChildStopped, ReplyRoute};

use super::super::pool::worker::{CurrentWorker, WorkerPreparationError, WorkerReplacementError};
use super::super::pool::{Assignment, JobId, SubmissionId};
use super::super::worker::{ActivationAttempt, WorkerActivationOutcome};
use super::super::{
    ActivationPermit, ActivationPlan, WorkerAttempt, WorkerCreationRejection, WorkerPreparation,
    WorkerSource, WorkerSubmission,
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
            WorkerPreparation<Source, Role, W, P>,
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
/// or Bombay's terminal custodian receives the complete owned value. Applications
/// can borrow the associated semantic role when one exists, or consume an original
/// source rejection with [`Self::into_source_rejection`]. That projection retains
/// shared role ownership and returns every other complete diagnostic unchanged.
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

    /// Consume the complete source-rejection diagnostic, returning every other
    /// diagnostic unchanged. The role retains its original shared allocation;
    /// this operation does not clone a role or alter the pool's recovery policy.
    /// The returned source is present only when the existing diagnostic owned it.
    pub fn into_source_rejection(
        self,
    ) -> Result<
        (
            Arc<Role>,
            WorkerAttempt,
            ChildStopped<BehaviorAddr<W>>,
            Option<Source>,
            Source::SourceRejection,
        ),
        Self,
    > {
        match self.cause {
            FifoDiagnosticCause::WorkerPreparationFailed {
                role,
                previous,
                stopped,
                returned_source,
                error: WorkerPreparationError::SourceRejected(reason),
            } => Ok((role.into_role(), previous, stopped, returned_source, reason)),
            cause => Err(Self { cause }),
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

#[cfg(test)]
mod live_activation_correlation {
    use super::super::requests::FifoRequests;
    use super::super::{FifoEvent, FifoOperating, FifoPool, PoolState};
    use super::{FifoDiagnostic, FifoDiagnosticCause};
    use crate::atomic::pool::ShutdownSequence;
    use crate::atomic::pool::assignment::{AcceptedJobSequence, AssignmentSequence};
    use crate::atomic::pool::worker::activation_pool_correlation::{
        ActivationEndpoint, ActivationWorker, assert_current_worker, initialized_activation,
    };
    use crate::atomic::pool::worker::{CurrentWorker, Member, MemberState, Worker, WorkerPhase};
    use crate::atomic::restart::RecoveryCount;
    use crate::atomic::restart::RestartBudget;
    use crate::atomic::worker::WorkerActivationOutcome;
    use crate::atomic::{
        ActivationPolicy, ActorDrainPolicy, BacklogCapacity, ImmediateActivation, Interruption,
        PoolFailureReaction, PoolRecovery, RoleName, WorkerAttempt,
    };
    use crate::{
        Active, ChildStopped, DiagnosticAction, DiagnosticDisposition, Exit, StopOnShutdown,
    };
    use behavior::{Actions, CreationSequence, EstablishedActor, Never, Step};
    use core::convert::Infallible;
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::time::Instant;

    #[tokio::test]
    async fn fifo_dispatched_live_returns_the_original_foreign_activation_twice() {
        let mut creations = CreationSequence::new();
        let creation = creations.issue().expect("one original worker creation");
        let worker = WorkerAttempt::issued(creation);
        let actor =
            EstablishedActor::<StopOnShutdown<ActivationWorker>>::issued(ActivationEndpoint(19));
        let expected_actor = actor.clone();
        let original = initialized_activation(worker.clone(), actor.recipient());
        let foreign = initialized_activation(worker.clone(), actor.recipient());
        let original_attempt = original.attempt();
        let foreign_attempt = foreign.attempt();
        assert!(original_attempt != foreign_attempt);
        let inputs = [
            (
                foreign.started(),
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Started,
            ),
            (
                foreign.activate().await,
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Ready(()),
            ),
        ];
        for (input, report) in inputs {
            let role = RoleName::new(23_u64);
            let expected_role = role.clone().into_role();
            let member = Member {
                role,
                recoveries: RecoveryCount::default(),
                state: MemberState::Worker(Worker {
                    current: CurrentWorker::new(worker.clone(), expected_actor.clone()),
                    phase: WorkerPhase::ActivationDispatched {
                        attempt: original_attempt.clone(),
                        stopped: None,
                    },
                }),
            };
            let pool: FifoPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                Infallible,
                u8,
                u16,
            > = FifoPool {
                state: PoolState::Operating(FifoOperating {
                    members: vec![member],
                    backlog: BTreeMap::new(),
                    cursor: 0,
                }),
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                interruption: Interruption::Fail,
                actor_drain: ActorDrainPolicy::WaitForActorGraph,
                diagnostics: DiagnosticDisposition::terminate(),
                creations: CreationSequence::new(),
                jobs: AcceptedJobSequence::new(),
                assignments: AssignmentSequence::new(),
                shutdowns: ShutdownSequence::new(),
                next_restart_timer: 1,
            };
            let mut pool = Active { behavior: pool };
            let mut returned = input;
            for _ in 0..2 {
                let actions = pool
                    .transition(FifoEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let FifoRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.as_slice().is_empty());
                assert!(worker_assignments.is_empty());
                assert!(worker_preparations.is_empty());
                assert!(restart_schedules.is_empty());
                assert!(worker_shutdowns.is_empty());
                assert!(creates.is_empty());
                assert!(matches!(become_, Step::Continue));
                let mut diagnostics = diagnostics.into_requests();
                assert_eq!(diagnostics.len(), 1);
                let DiagnosticAction::Terminal {
                    diagnostic:
                        FifoDiagnostic {
                            cause:
                                FifoDiagnosticCause::Unexpected(FifoEvent::WorkerActivationReported(
                                    input,
                                )),
                        },
                } = diagnostics.remove(0)
                else {
                    panic!(
                        "a foreign attempt returns the complete original activation through the existing Unexpected policy"
                    )
                };
                assert_eq!(input.worker(), worker);
                assert!(input.attempt() == &foreign_attempt);
                returned = input;
                let PoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.members.len(), 1);
                assert!(operating.backlog.is_empty());
                assert_eq!(operating.cursor, 0);
                let member = &operating.members[0];
                let role = member.role.clone().into_role();
                assert!(Arc::ptr_eq(&role, &expected_role));
                assert_eq!(*role, 23);
                assert_eq!(member.recoveries, RecoveryCount::default());
                let MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::ActivationDispatched { attempt, stopped },
                }) = &member.state
                else {
                    panic!("foreign report preserves the exact original worker phase")
                };
                assert_current_worker(current, &worker, &expected_actor);
                assert!(attempt == &original_attempt);
                assert!(stopped.is_none());
            }
            let (returned_worker, returned_attempt, outcome) = returned.into_parts();
            assert_eq!(returned_worker, worker);
            assert!(returned_attempt == foreign_attempt);
            match (report, outcome) {
                (WorkerActivationOutcome::Started, WorkerActivationOutcome::Started)
                | (WorkerActivationOutcome::Ready(()), WorkerActivationOutcome::Ready(())) => {}
                _ => panic!("the original complete activation outcome is preserved"),
            }
        }
        drop(original);
    }

    #[tokio::test]
    async fn fifo_dispatched_stopped_returns_the_original_foreign_activation_twice() {
        let mut creations = CreationSequence::new();
        let creation = creations.issue().expect("one original worker creation");
        let worker = WorkerAttempt::issued(creation);
        let actor =
            EstablishedActor::<StopOnShutdown<ActivationWorker>>::issued(ActivationEndpoint(19));
        let expected_actor = actor.clone();
        let original = initialized_activation(worker.clone(), actor.recipient());
        let foreign = initialized_activation(worker.clone(), actor.recipient());
        let original_attempt = original.attempt();
        let foreign_attempt = foreign.attempt();
        assert!(original_attempt != foreign_attempt);
        let stopped_at = Instant::now();
        let inputs = [
            (
                foreign.started(),
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Started,
            ),
            (
                foreign.activate().await,
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Ready(()),
            ),
        ];
        for (input, report) in inputs {
            let role = RoleName::new(23_u64);
            let expected_role = role.clone().into_role();
            let member = Member {
                role,
                recoveries: RecoveryCount::default(),
                state: MemberState::Worker(Worker {
                    current: CurrentWorker::new(worker.clone(), expected_actor.clone()),
                    phase: WorkerPhase::ActivationDispatched {
                        attempt: original_attempt.clone(),
                        stopped: Some(ChildStopped::new(creation, Ok(Exit::Normal), stopped_at)),
                    },
                }),
            };
            let pool: FifoPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                Infallible,
                u8,
                u16,
            > = FifoPool {
                state: PoolState::Operating(FifoOperating {
                    members: vec![member],
                    backlog: BTreeMap::new(),
                    cursor: 0,
                }),
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                interruption: Interruption::Fail,
                actor_drain: ActorDrainPolicy::WaitForActorGraph,
                diagnostics: DiagnosticDisposition::terminate(),
                creations: CreationSequence::new(),
                jobs: AcceptedJobSequence::new(),
                assignments: AssignmentSequence::new(),
                shutdowns: ShutdownSequence::new(),
                next_restart_timer: 1,
            };
            let mut pool = Active { behavior: pool };
            let mut returned = input;
            for _ in 0..2 {
                let actions = pool
                    .transition(FifoEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let FifoRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.as_slice().is_empty());
                assert!(worker_assignments.is_empty());
                assert!(worker_preparations.is_empty());
                assert!(restart_schedules.is_empty());
                assert!(worker_shutdowns.is_empty());
                assert!(creates.is_empty());
                assert!(matches!(become_, Step::Continue));
                let mut diagnostics = diagnostics.into_requests();
                assert_eq!(diagnostics.len(), 1);
                let DiagnosticAction::Terminal {
                    diagnostic:
                        FifoDiagnostic {
                            cause:
                                FifoDiagnosticCause::Unexpected(FifoEvent::WorkerActivationReported(
                                    input,
                                )),
                        },
                } = diagnostics.remove(0)
                else {
                    panic!(
                        "a foreign attempt returns the complete original activation through the existing Unexpected policy"
                    )
                };
                assert_eq!(input.worker(), worker);
                assert!(input.attempt() == &foreign_attempt);
                returned = input;
                let PoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.members.len(), 1);
                assert!(operating.backlog.is_empty());
                assert_eq!(operating.cursor, 0);
                let member = &operating.members[0];
                let role = member.role.clone().into_role();
                assert!(Arc::ptr_eq(&role, &expected_role));
                assert_eq!(*role, 23);
                assert_eq!(member.recoveries, RecoveryCount::default());
                let MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::ActivationDispatched { attempt, stopped },
                }) = &member.state
                else {
                    panic!("foreign report preserves the exact original worker phase")
                };
                assert_current_worker(current, &worker, &expected_actor);
                assert!(attempt == &original_attempt);
                let Some(stopped) = stopped else {
                    panic!("the original stopped child remains owned")
                };
                assert_eq!(stopped.child, creation);
                assert!(matches!(&stopped.outcome, Ok(Exit::Normal)));
                assert_eq!(stopped.at, stopped_at);
            }
            let (returned_worker, returned_attempt, outcome) = returned.into_parts();
            assert_eq!(returned_worker, worker);
            assert!(returned_attempt == foreign_attempt);
            match (report, outcome) {
                (WorkerActivationOutcome::Started, WorkerActivationOutcome::Started)
                | (WorkerActivationOutcome::Ready(()), WorkerActivationOutcome::Ready(())) => {}
                _ => panic!("the original complete activation outcome is preserved"),
            }
        }
        drop(original);
    }

    #[tokio::test]
    async fn fifo_activating_live_returns_the_original_foreign_activation_twice() {
        let mut creations = CreationSequence::new();
        let creation = creations.issue().expect("one original worker creation");
        let worker = WorkerAttempt::issued(creation);
        let actor =
            EstablishedActor::<StopOnShutdown<ActivationWorker>>::issued(ActivationEndpoint(19));
        let expected_actor = actor.clone();
        let original = initialized_activation(worker.clone(), actor.recipient());
        let foreign = initialized_activation(worker.clone(), actor.recipient());
        let original_attempt = original.attempt();
        let foreign_attempt = foreign.attempt();
        assert!(original_attempt != foreign_attempt);
        let inputs = [
            (
                foreign.started(),
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Started,
            ),
            (
                foreign.activate().await,
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Ready(()),
            ),
        ];
        for (input, report) in inputs {
            let role = RoleName::new(23_u64);
            let expected_role = role.clone().into_role();
            let member = Member {
                role,
                recoveries: RecoveryCount::default(),
                state: MemberState::Worker(Worker {
                    current: CurrentWorker::new(worker.clone(), expected_actor.clone()),
                    phase: WorkerPhase::Activating {
                        attempt: original_attempt.clone(),
                        stopped: None,
                    },
                }),
            };
            let pool: FifoPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                Infallible,
                u8,
                u16,
            > = FifoPool {
                state: PoolState::Operating(FifoOperating {
                    members: vec![member],
                    backlog: BTreeMap::new(),
                    cursor: 0,
                }),
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                interruption: Interruption::Fail,
                actor_drain: ActorDrainPolicy::WaitForActorGraph,
                diagnostics: DiagnosticDisposition::terminate(),
                creations: CreationSequence::new(),
                jobs: AcceptedJobSequence::new(),
                assignments: AssignmentSequence::new(),
                shutdowns: ShutdownSequence::new(),
                next_restart_timer: 1,
            };
            let mut pool = Active { behavior: pool };
            let mut returned = input;
            for _ in 0..2 {
                let actions = pool
                    .transition(FifoEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let FifoRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.as_slice().is_empty());
                assert!(worker_assignments.is_empty());
                assert!(worker_preparations.is_empty());
                assert!(restart_schedules.is_empty());
                assert!(worker_shutdowns.is_empty());
                assert!(creates.is_empty());
                assert!(matches!(become_, Step::Continue));
                let mut diagnostics = diagnostics.into_requests();
                assert_eq!(diagnostics.len(), 1);
                let DiagnosticAction::Terminal {
                    diagnostic:
                        FifoDiagnostic {
                            cause:
                                FifoDiagnosticCause::Unexpected(FifoEvent::WorkerActivationReported(
                                    input,
                                )),
                        },
                } = diagnostics.remove(0)
                else {
                    panic!(
                        "a foreign attempt returns the complete original activation through the existing Unexpected policy"
                    )
                };
                assert_eq!(input.worker(), worker);
                assert!(input.attempt() == &foreign_attempt);
                returned = input;
                let PoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.members.len(), 1);
                assert!(operating.backlog.is_empty());
                assert_eq!(operating.cursor, 0);
                let member = &operating.members[0];
                let role = member.role.clone().into_role();
                assert!(Arc::ptr_eq(&role, &expected_role));
                assert_eq!(*role, 23);
                assert_eq!(member.recoveries, RecoveryCount::default());
                let MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::Activating { attempt, stopped },
                }) = &member.state
                else {
                    panic!("foreign report preserves the exact original worker phase")
                };
                assert_current_worker(current, &worker, &expected_actor);
                assert!(attempt == &original_attempt);
                assert!(stopped.is_none());
            }
            let (returned_worker, returned_attempt, outcome) = returned.into_parts();
            assert_eq!(returned_worker, worker);
            assert!(returned_attempt == foreign_attempt);
            match (report, outcome) {
                (WorkerActivationOutcome::Started, WorkerActivationOutcome::Started)
                | (WorkerActivationOutcome::Ready(()), WorkerActivationOutcome::Ready(())) => {}
                _ => panic!("the original complete activation outcome is preserved"),
            }
        }
        drop(original);
    }

    #[tokio::test]
    async fn fifo_activating_stopped_returns_the_original_foreign_activation_twice() {
        let mut creations = CreationSequence::new();
        let creation = creations.issue().expect("one original worker creation");
        let worker = WorkerAttempt::issued(creation);
        let actor =
            EstablishedActor::<StopOnShutdown<ActivationWorker>>::issued(ActivationEndpoint(19));
        let expected_actor = actor.clone();
        let original = initialized_activation(worker.clone(), actor.recipient());
        let foreign = initialized_activation(worker.clone(), actor.recipient());
        let original_attempt = original.attempt();
        let foreign_attempt = foreign.attempt();
        assert!(original_attempt != foreign_attempt);
        let stopped_at = Instant::now();
        let inputs = [
            (
                foreign.started(),
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Started,
            ),
            (
                foreign.activate().await,
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Ready(()),
            ),
        ];
        for (input, report) in inputs {
            let role = RoleName::new(23_u64);
            let expected_role = role.clone().into_role();
            let member = Member {
                role,
                recoveries: RecoveryCount::default(),
                state: MemberState::Worker(Worker {
                    current: CurrentWorker::new(worker.clone(), expected_actor.clone()),
                    phase: WorkerPhase::Activating {
                        attempt: original_attempt.clone(),
                        stopped: Some(ChildStopped::new(creation, Ok(Exit::Normal), stopped_at)),
                    },
                }),
            };
            let pool: FifoPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                Infallible,
                u8,
                u16,
            > = FifoPool {
                state: PoolState::Operating(FifoOperating {
                    members: vec![member],
                    backlog: BTreeMap::new(),
                    cursor: 0,
                }),
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                interruption: Interruption::Fail,
                actor_drain: ActorDrainPolicy::WaitForActorGraph,
                diagnostics: DiagnosticDisposition::terminate(),
                creations: CreationSequence::new(),
                jobs: AcceptedJobSequence::new(),
                assignments: AssignmentSequence::new(),
                shutdowns: ShutdownSequence::new(),
                next_restart_timer: 1,
            };
            let mut pool = Active { behavior: pool };
            let mut returned = input;
            for _ in 0..2 {
                let actions = pool
                    .transition(FifoEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let FifoRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.as_slice().is_empty());
                assert!(worker_assignments.is_empty());
                assert!(worker_preparations.is_empty());
                assert!(restart_schedules.is_empty());
                assert!(worker_shutdowns.is_empty());
                assert!(creates.is_empty());
                assert!(matches!(become_, Step::Continue));
                let mut diagnostics = diagnostics.into_requests();
                assert_eq!(diagnostics.len(), 1);
                let DiagnosticAction::Terminal {
                    diagnostic:
                        FifoDiagnostic {
                            cause:
                                FifoDiagnosticCause::Unexpected(FifoEvent::WorkerActivationReported(
                                    input,
                                )),
                        },
                } = diagnostics.remove(0)
                else {
                    panic!(
                        "a foreign attempt returns the complete original activation through the existing Unexpected policy"
                    )
                };
                assert_eq!(input.worker(), worker);
                assert!(input.attempt() == &foreign_attempt);
                returned = input;
                let PoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.members.len(), 1);
                assert!(operating.backlog.is_empty());
                assert_eq!(operating.cursor, 0);
                let member = &operating.members[0];
                let role = member.role.clone().into_role();
                assert!(Arc::ptr_eq(&role, &expected_role));
                assert_eq!(*role, 23);
                assert_eq!(member.recoveries, RecoveryCount::default());
                let MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::Activating { attempt, stopped },
                }) = &member.state
                else {
                    panic!("foreign report preserves the exact original worker phase")
                };
                assert_current_worker(current, &worker, &expected_actor);
                assert!(attempt == &original_attempt);
                let Some(stopped) = stopped else {
                    panic!("the original stopped child remains owned")
                };
                assert_eq!(stopped.child, creation);
                assert!(matches!(&stopped.outcome, Ok(Exit::Normal)));
                assert_eq!(stopped.at, stopped_at);
            }
            let (returned_worker, returned_attempt, outcome) = returned.into_parts();
            assert_eq!(returned_worker, worker);
            assert!(returned_attempt == foreign_attempt);
            match (report, outcome) {
                (WorkerActivationOutcome::Started, WorkerActivationOutcome::Started)
                | (WorkerActivationOutcome::Ready(()), WorkerActivationOutcome::Ready(())) => {}
                _ => panic!("the original complete activation outcome is preserved"),
            }
        }
        drop(original);
    }

    #[tokio::test]
    async fn fifo_original_activation_started_then_ready_preserves_all_lanes() {
        let mut creations = CreationSequence::new();
        let creation = creations.issue().expect("one original worker creation");
        let worker = WorkerAttempt::issued(creation);
        let actor =
            EstablishedActor::<StopOnShutdown<ActivationWorker>>::issued(ActivationEndpoint(19));
        let expected_actor = actor.clone();
        let original = initialized_activation(worker.clone(), actor.recipient());
        let original_attempt = original.attempt();
        let started = original.started();
        let role = RoleName::new(23_u64);
        let expected_role = role.clone().into_role();
        let member = Member {
            role,
            recoveries: RecoveryCount::default(),
            state: MemberState::Worker(Worker {
                current: CurrentWorker::new(worker.clone(), expected_actor.clone()),
                phase: WorkerPhase::ActivationDispatched {
                    attempt: original_attempt.clone(),
                    stopped: None,
                },
            }),
        };
        let pool: FifoPool<u64, ActivationWorker, ImmediateActivation, Never, Infallible, u8, u16> =
            FifoPool {
                state: PoolState::Operating(FifoOperating {
                    members: vec![member],
                    backlog: BTreeMap::new(),
                    cursor: 0,
                }),
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                interruption: Interruption::Fail,
                actor_drain: ActorDrainPolicy::WaitForActorGraph,
                diagnostics: DiagnosticDisposition::terminate(),
                creations: CreationSequence::new(),
                jobs: AcceptedJobSequence::new(),
                assignments: AssignmentSequence::new(),
                shutdowns: ShutdownSequence::new(),
                next_restart_timer: 1,
            };
        let mut pool = Active { behavior: pool };

        let inputs = [
            (
                started,
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Started,
            ),
            (
                original.activate().await,
                WorkerActivationOutcome::<ActivationWorker, ImmediateActivation>::Ready(()),
            ),
        ];
        for (input, report) in inputs {
            let actions = pool
                .transition(FifoEvent::WorkerActivationReported(input))
                .unwrap_or_else(|error| {
                    panic!("the original activation returns complete Actions: {error}")
                });
            let Actions {
                sends,
                creates,
                become_,
            } = actions;
            let FifoRequests {
                worker_observations,
                worker_initializations,
                worker_activations,
                customer_outcomes,
                worker_assignments,
                worker_preparations,
                restart_schedules,
                worker_shutdowns,
                diagnostics,
            } = sends;
            assert!(worker_observations.is_empty());
            assert!(worker_initializations.is_empty());
            assert!(worker_activations.is_empty());
            assert!(customer_outcomes.as_slice().is_empty());
            assert!(worker_assignments.is_empty());
            assert!(worker_preparations.is_empty());
            assert!(restart_schedules.is_empty());
            assert!(worker_shutdowns.is_empty());
            assert!(diagnostics.is_empty());
            assert!(creates.is_empty());
            assert!(matches!(become_, Step::Continue));
            let PoolState::Operating(operating) = &pool.state else {
                panic!("original activation leaves the pool operating")
            };
            assert_eq!(operating.members.len(), 1);
            assert!(operating.backlog.is_empty());
            assert_eq!(operating.cursor, 0);
            let member = &operating.members[0];
            let actual_role = member.role.clone().into_role();
            assert!(Arc::ptr_eq(&actual_role, &expected_role));
            assert_eq!(*actual_role, 23);
            assert_eq!(member.recoveries, RecoveryCount::default());
            let MemberState::Worker(Worker { current, phase }) = &member.state else {
                panic!("the actual original worker stays owned")
            };
            assert_current_worker(current, &worker, &expected_actor);
            match (report, phase) {
                (
                    WorkerActivationOutcome::Started,
                    WorkerPhase::Activating {
                        attempt,
                        stopped: None,
                    },
                ) => {
                    assert!(attempt == &original_attempt);
                }
                (WorkerActivationOutcome::Ready(()), WorkerPhase::Idle) => {}
                _ => panic!("actual Started enters Activating and actual Ready enters Idle"),
            }
        }
    }
}
