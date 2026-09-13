//! Direct workers with one bounded admission-order backlog.

use core::{num::NonZeroUsize, ops::ControlFlow};
use std::collections::BTreeMap;
use std::mem;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, Births,
    ChildCreationSettled, ChildHead, ChildNamespaceExhausted, ChildReport, CreateChild,
    CreationKind, CreationSequence, CreationSettlement, Creations, CreationsSettled,
    EndpointAddress, Here, InitializationTurn, InjectEvent, InterpreterFault, InterpreterRequests,
    ItemSettlement, MessageProtocol, Never, Protocol, SendEffects, SettledItem, SourceActions,
    User,
};
use thiserror::Error;

use crate::atomic::drain::{ForcedRetirementCause, ShutdownDeadline};
use crate::{
    DeliveryRoute, DiagnosticAction, DiagnosticDisposition, DiagnosticRoute,
    EstablishedShutdownResolved, ObserveChild, ReplyRoute, ScheduleAfter, ShutdownEstablished,
    ShutdownRequested, StopOnShutdown,
};

use super::RoleName;
use super::worker::{InitialWorkerRejection, prepare_initial_workers};
use super::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, BeginActivation, InitializeWorker,
    OrderedRoles, PrepareWorkers, PreparedWorker, WorkerCreationRejection, WorkerSource,
    WorkerSubmission,
};

mod event;
mod job;
mod protocol;
mod requests;

pub use event::FifoEvent;
pub use protocol::{
    AdmissionRejection, AssignedReturnReason, FifoCommand, FifoDiagnostic, FifoOutcome,
    FifoOutcomeKind, QueuedReturnReason,
};
pub use requests::FifoRequests;

use super::pool::assignment::{
    AcceptedJobSequence, AdmissionOrdinal, AssignedJob, AssignmentReceiptOutcome,
    AssignmentRejectionOutcome, AssignmentSequence, AssignmentShutdown, CorrelationMatch,
    CustomerJob, WorkerCompletionOutcome, WorkerExitOutcome,
};
use super::pool::worker as direct_worker;
use super::pool::worker::{
    Member, MemberState, PreparedReplacement, RetiringWorker, RetiringWorkerActivation,
    RetiringWorkerInitialization, ShutdownJoin, Worker, WorkerCreationAdmission, WorkerCustody,
    WorkerDeparture, WorkerPhase, WorkerPreparationError, WorkerRecoveryPreparation,
    WorkerReplacementError, WorkerReplacementRelease, creation_identity,
};
use super::pool::{
    AssignWorker, Assignment, AssignmentReceipt, BacklogCapacity, CompletesAssignments, Completion,
    Interruption, PoolFailureReaction, PoolRecovery, ShutdownSequence, SubmissionId,
};
use super::pool::{PoolRecoveryState, WorkerRecoveryDecision};
use super::restart::{RecoveryRelease, RestartAdmission, RestartBudget, admit_restart};
use super::schedule::ScheduleKey;
use super::worker::{WorkerActivationOutcome, preparation_result_accepts, stop_kind};
use job::QueuedJob;
use protocol::FifoDiagnosticCause;

type FifoActions<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult> = Actions<
    BehaviorAddr<W>,
    Never,
    FifoRequests<
        InterpreterRequests<ObserveChild<<W as Behavior>::Protocol, ChildHead>>,
        InterpreterRequests<InitializeWorker<W, P>>,
        InterpreterRequests<BeginActivation<W, P>>,
        <CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult> as DeliveryRoute>::Sends,
        SourceActions<AssignWorker<<W as Behavior>::Protocol, Job>>,
        SourceActions<PrepareWorkers<Source, Role, W, P>>,
        SourceActions<ScheduleAfter>,
        InterpreterRequests<ShutdownEstablished<StopOnShutdown<W>, Here>>,
        InterpreterRequests<
            DiagnosticAction<
                DiagnosticRoute,
                FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>,
            >,
        >,
    >,
    Births<StopOnShutdown<W>>,
>;

type CustomerRoute<A, Role, Job, WorkerResult> =
    ReplyRoute<MessageProtocol<A, FifoOutcome<Role, Job, WorkerResult>>>;

struct Operating<Role, W, P, Job, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    members: Vec<
        Member<
            Role,
            W,
            P,
            Job,
            WorkerResult,
            CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>,
        >,
    >,
    backlog: BTreeMap<
        AdmissionOrdinal,
        QueuedJob<RoleName<Role>, Job, CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>>,
    >,
    cursor: usize,
}

enum FifoDispatch<Role, W, Job, WorkerResult>
where
    W: Behavior,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    BehaviorAddr<W>: EndpointAddress,
{
    Assigned(AssignWorker<W::Protocol, Job>),
    WorkerUnavailable(
        QueuedJob<RoleName<Role>, Job, CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>>,
    ),
    CorrelationUnavailable(
        QueuedJob<RoleName<Role>, Job, CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>>,
    ),
}

#[derive(Clone, Copy)]
enum CreationBatchRejection {
    NamespaceExhausted,
    InterpreterCorrupt(InterpreterFault),
}

impl CreationBatchRejection {
    fn worker<W>(self, worker: W) -> WorkerCreationRejection<W>
    where
        W: Behavior,
    {
        match self {
            Self::NamespaceExhausted => WorkerCreationRejection::NamespaceExhausted { worker },
            Self::InterpreterCorrupt(fault) => {
                WorkerCreationRejection::InterpreterCorrupt { worker, fault }
            }
        }
    }

    fn settlement<W>(
        self,
        creations: Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
    ) -> CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>
    where
        W: Behavior + BehaviorBase,
        BehaviorAddr<W>: EndpointAddress,
        StopOnShutdown<W>:
            Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    {
        match self {
            Self::NamespaceExhausted => CreationsSettled::new(CreationSettlement::Rejected {
                creations,
                reason: ChildNamespaceExhausted,
            }),
            Self::InterpreterCorrupt(fault) => {
                CreationsSettled::new(CreationSettlement::Corrupt { creations, fault })
            }
        }
    }
}

impl<Role, W, P, Job, WorkerResult> Operating<Role, W, P, Job, WorkerResult>
where
    W: Behavior + BehaviorBase,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    Job: Clone,
{
    fn available_position(&self) -> Option<usize> {
        if self.members.is_empty() {
            return None;
        }
        (0..self.members.len()).find_map(|offset| {
            let position = (self.cursor + offset) % self.members.len();
            match &self.members[position].state {
                MemberState::Worker(Worker {
                    phase: WorkerPhase::Idle,
                    ..
                }) => Some(position),
                MemberState::Creating(_)
                | MemberState::Worker(_)
                | MemberState::Recovering(_)
                | MemberState::Retired => None,
            }
        })
    }

    fn creation_positions(
        &self,
        identities: impl IntoIterator<Item = (behavior::CreationId, CreationKind)>,
    ) -> Option<Vec<usize>> {
        direct_worker::ordered_creation_positions(
            self.members.iter().map(Member::expected_creation),
            identities,
        )
    }

    fn recoverable_position<Source>(&self, recovery: &PoolRecoveryState<Source>) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Retired => false,
            MemberState::Worker(Worker {
                phase: WorkerPhase::Stopping(_),
                ..
            })
            | MemberState::Recovering(_) => match recovery {
                PoolRecoveryState::Permanent { .. } | PoolRecoveryState::Transient { .. } => true,
                PoolRecoveryState::Temporary { .. } => false,
            },
            MemberState::Creating(_) | MemberState::Worker(_) => true,
        })
    }

    fn dispatch(
        &mut self,
        assignments: &mut AssignmentSequence,
        queued: QueuedJob<
            RoleName<Role>,
            Job,
            CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>,
        >,
    ) -> FifoDispatch<Role, W, Job, WorkerResult> {
        let Some(position) = self.available_position() else {
            return FifoDispatch::WorkerUnavailable(queued);
        };
        let member = &mut self.members[position];
        let MemberState::Worker(Worker { current, phase }) = &mut member.state else {
            return FifoDispatch::WorkerUnavailable(queued);
        };
        let WorkerPhase::Idle = phase else {
            return FifoDispatch::WorkerUnavailable(queued);
        };
        let Some((correlation, execution)) =
            assignments.assign(&current.attempt, queued.customer.payload.clone())
        else {
            return FifoDispatch::CorrelationUnavailable(queued);
        };
        let action = AssignWorker::new(current.recipient(), &correlation, execution);
        *phase = WorkerPhase::Busy(AssignedJob::new(queued.customer, correlation));
        self.cursor = position + 1;
        FifoDispatch::Assigned(action)
    }

    fn assignment_receipt_position(&self, receipt: &AssignmentReceipt) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Worker(Worker {
                phase: WorkerPhase::Busy(assignment),
                ..
            }) => matches!(assignment.compare_receipt(receipt), CorrelationMatch::Exact),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn completion_position(
        &self,
        child: behavior::CreationId,
        completion: &Completion<WorkerResult>,
    ) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Worker(Worker {
                current,
                phase: WorkerPhase::Busy(assignment),
            }) if current.attempt.creation() == child => matches!(
                assignment.compare_completion(completion),
                CorrelationMatch::Exact
            ),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn worker_position(&self, child: behavior::CreationId) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Creating(worker) => worker.attempt.creation() == child,
            MemberState::Worker(Worker { current, phase: _ }) => {
                current.attempt.creation() == child
            }
            MemberState::Recovering(_) | MemberState::Retired => false,
        })
    }

    fn worker_shutdown_position(&self, request: crate::ShutdownId) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Worker(Worker {
                phase: WorkerPhase::Stopping(join),
                ..
            }) => join.request() == request,
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn preparation_position<Source>(
        &self,
        input: &behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Option<usize>
    where
        Role: Send + Sync,
        W: Send,
        P: ActivationPlan,
        Source: WorkerSource<Role, W, P>,
    {
        self.members.iter().position(|member| match &member.state {
            MemberState::Recovering(direct_worker::RecoveringWorker::Preparing {
                preparation: expected,
                ..
            }) => preparation_result_accepts(input, expected),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn waiting_recovery_position(&self) -> Option<usize> {
        self.members.iter().position(|member| {
            matches!(
                &member.state,
                MemberState::Recovering(direct_worker::RecoveringWorker::WaitingForSource { .. })
            )
        })
    }

    fn restart_schedule_position(
        &self,
        input: &behavior::ActionItemResult<ScheduleAfter>,
    ) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Recovering(recovering) => recovering.accepts_restart_schedule(input),
            MemberState::Creating(_) | MemberState::Worker(_) | MemberState::Retired => false,
        })
    }

    fn restart_timer_position(&self, elapsed: &crate::TimerElapsed) -> Option<usize> {
        self.members.iter().position(|member| match &member.state {
            MemberState::Recovering(recovering) => recovering.accepts_restart_timer(elapsed),
            MemberState::Creating(_) | MemberState::Worker(_) | MemberState::Retired => false,
        })
    }
}

impl<Role, W, P, Source, Diagnostics, Job, WorkerResult>
    FifoPool<Role, W, P, Source, Diagnostics, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + BehaviorBase + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    Diagnostics: DiagnosticRoute<FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>> + Clone,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    Job: Clone + Send,
    WorkerResult: Send,
{
    fn accept_submission(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        submission: SubmissionId,
        payload: Job,
        customer: CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>,
    ) -> (
        Operating<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        match operating.recoverable_position(&self.recovery) {
            Some(_) => {}
            None => {
                actions.sends.customer_outcomes = customer.deliver(FifoOutcome::rejected(
                    submission,
                    payload,
                    AdmissionRejection::NoRecoverableWorkers,
                ));
                return (operating, actions);
            }
        }

        let ready = match operating.available_position() {
            Some(position) => Some(position),
            None => match operating.backlog.len().cmp(&self.backlog.maximum()) {
                core::cmp::Ordering::Less => None,
                core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
                    actions.sends.customer_outcomes = customer.deliver(FifoOutcome::rejected(
                        submission,
                        payload,
                        AdmissionRejection::BacklogFull,
                    ));
                    return (operating, actions);
                }
            },
        };

        let Some((job, admitted)) = self.jobs.issue() else {
            actions.sends.customer_outcomes = customer.deliver(FifoOutcome::rejected(
                submission,
                payload,
                AdmissionRejection::JobCorrelationUnavailable,
            ));
            return (operating, actions);
        };
        let admission_customer = customer.clone();
        let queued = QueuedJob {
            customer: CustomerJob {
                id: job,
                admitted,
                payload,
                customer,
            },
            assigned_role: None,
        };

        match ready {
            None => {
                actions.sends.customer_outcomes =
                    admission_customer.deliver(FifoOutcome::accepted(submission, job));
                operating.backlog.insert(admitted, queued);
            }
            Some(_) => match operating.dispatch(&mut self.assignments, queued) {
                FifoDispatch::Assigned(assignment) => {
                    actions.sends.customer_outcomes =
                        admission_customer.deliver(FifoOutcome::accepted(submission, job));
                    actions.sends.worker_assignments.send(assignment);
                }
                FifoDispatch::WorkerUnavailable(queued) => {
                    match operating.backlog.len().cmp(&self.backlog.maximum()) {
                        core::cmp::Ordering::Less => {
                            actions.sends.customer_outcomes =
                                admission_customer.deliver(FifoOutcome::accepted(submission, job));
                            operating.backlog.insert(admitted, queued);
                        }
                        core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
                            actions.sends.customer_outcomes =
                                queued.customer.customer.deliver(FifoOutcome::rejected(
                                    submission,
                                    queued.customer.payload,
                                    AdmissionRejection::BacklogFull,
                                ));
                        }
                    }
                }
                FifoDispatch::CorrelationUnavailable(queued) => {
                    actions.sends.customer_outcomes =
                        queued.customer.customer.deliver(FifoOutcome::rejected(
                            submission,
                            queued.customer.payload,
                            AdmissionRejection::AssignmentCorrelationUnavailable,
                        ));
                }
            },
        }
        (operating, actions)
    }

    fn fill_fifo(
        &mut self,
        operating: &mut Operating<Role, W, P, Job, WorkerResult>,
        actions: &mut FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        loop {
            match operating.available_position() {
                Some(_) => {}
                None => return,
            }
            let Some((admitted, queued)) = operating.backlog.pop_first() else {
                return;
            };
            match operating.dispatch(&mut self.assignments, queued) {
                FifoDispatch::Assigned(assignment) => {
                    actions.sends.worker_assignments.send(assignment);
                }
                FifoDispatch::WorkerUnavailable(queued) => {
                    operating.backlog.insert(admitted, queued);
                    return;
                }
                FifoDispatch::CorrelationUnavailable(queued) => {
                    let CustomerJob {
                        id,
                        admitted: _,
                        payload,
                        customer,
                    } = queued.customer;
                    let outcome = match queued.assigned_role {
                        None => FifoOutcome::returned_queued(
                            id,
                            payload,
                            QueuedReturnReason::AssignmentCorrelationUnavailable,
                        ),
                        Some(role) => FifoOutcome::returned_assigned(
                            id,
                            role,
                            payload,
                            AssignedReturnReason::RetryPreparationRejected,
                        ),
                    };
                    actions
                        .sends
                        .customer_outcomes
                        .append(customer.deliver(outcome));
                }
            }
        }
    }

    fn return_unrecoverable_jobs(
        &self,
        operating: &mut Operating<Role, W, P, Job, WorkerResult>,
        actions: &mut FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        if operating.recoverable_position(&self.recovery).is_some() {
            return;
        }
        for (_, queued) in mem::take(&mut operating.backlog) {
            let CustomerJob {
                id,
                admitted: _,
                payload,
                customer,
            } = queued.customer;
            actions
                .sends
                .customer_outcomes
                .append(customer.deliver(FifoOutcome::returned_queued(
                    id,
                    payload,
                    QueuedReturnReason::NoRecoverableWorkers,
                )));
        }
    }

    fn request_preparation(
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        source: Source,
        mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        Member<
            Role,
            W,
            P,
            Job,
            WorkerResult,
            CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>,
        >,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let (ticket, request) = PrepareWorkers::new(source, role.clone(), Vec::new());
        actions.sends.worker_preparations.send(request);
        (
            Member {
                role,
                recoveries,
                state: MemberState::Recovering(direct_worker::RecoveringWorker::Preparing {
                    previous,
                    stopped,
                    preparation: ticket,
                }),
            },
            actions,
        )
    }

    fn prepare_next_waiting(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        Operating<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let Some(position) = operating.waiting_recovery_position() else {
            return (operating, actions);
        };
        let member = operating.members.remove(position);
        let Member {
            role,
            recoveries,
            state:
                MemberState::Recovering(direct_worker::RecoveringWorker::WaitingForSource {
                    previous,
                    stopped,
                }),
        } = member
        else {
            operating.members.insert(position, member);
            return (operating, actions);
        };
        let Some(source) = self.recovery.claim_waiting_source() else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state: MemberState::Recovering(
                        direct_worker::RecoveringWorker::WaitingForSource { previous, stopped },
                    ),
                },
            );
            return (operating, actions);
        };
        let (member, actions) =
            Self::request_preparation(role, recoveries, previous, stopped, source, actions);
        operating.members.insert(position, member);
        (operating, actions)
    }

    fn reject_replacement(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        submission: WorkerSubmission<W, P>,
        returned_source: Option<Source>,
        error: WorkerReplacementError,
        mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        actions
            .sends
            .diagnostics
            .append(InterpreterRequests::one(self.diagnostics.action(
                FifoDiagnostic::new(FifoDiagnosticCause::WorkerReplacementFailed {
                    role: role.clone(),
                    previous,
                    stopped,
                    submission,
                    returned_source,
                    error,
                }),
            )));
        self.retire_role_after_error(operating, position, role, recoveries, actions)
    }

    fn reject_preparation(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        source: Source,
        error: WorkerPreparationError<Source::WorkerRejection, Source::SourceRejection>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let (returned_source, error) = match self.recovery.restore_source(source) {
            Ok(()) => (None, error),
            Err(source) => (Some(source), WorkerPreparationError::SourceStateCorrupt),
        };
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        actions
            .sends
            .diagnostics
            .append(InterpreterRequests::one(self.diagnostics.action(
                FifoDiagnostic::new(FifoDiagnosticCause::WorkerPreparationFailed {
                    role: role.clone(),
                    previous,
                    stopped,
                    returned_source,
                    error,
                }),
            )));
        self.retire_role_after_error(operating, position, role, recoveries, actions)
    }

    fn retire_role_after_error(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        operating.members.insert(
            position,
            Member {
                role,
                recoveries,
                state: MemberState::Retired,
            },
        );
        match self.recovery.failure() {
            PoolFailureReaction::RetireRole => {
                let (mut operating, mut actions) = self.prepare_next_waiting(operating, actions);
                self.fill_fifo(&mut operating, &mut actions);
                self.return_unrecoverable_jobs(&mut operating, &mut actions);
                (PoolState::Operating(operating), actions)
            }
            PoolFailureReaction::StopPool => self.begin_shutdown(operating, actions),
        }
    }

    fn return_source_after_failed_replacement(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        source: Source,
        submission: WorkerSubmission<W, P>,
        error: WorkerReplacementError,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let (returned_source, error) = match self.recovery.restore_source(source) {
            Ok(()) => (None, error),
            Err(source) => (Some(source), WorkerReplacementError::SourceStateCorrupt),
        };
        self.reject_replacement(
            operating,
            position,
            role,
            recoveries,
            previous,
            stopped,
            submission,
            returned_source,
            error,
            Actions::cont(),
        )
    }

    fn recover_worker(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let stop = stop_kind(&stopped.outcome);
        match self.recovery.decide(stop) {
            WorkerRecoveryDecision::PrepareWorker(source) => {
                let (member, next_actions) =
                    Self::request_preparation(role, recoveries, previous, stopped, source, actions);
                actions = next_actions;
                operating.members.insert(position, member);
                self.fill_fifo(&mut operating, &mut actions);
                (PoolState::Operating(operating), actions)
            }
            WorkerRecoveryDecision::WaitForSource => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Recovering(
                            direct_worker::RecoveringWorker::WaitingForSource { previous, stopped },
                        ),
                    },
                );
                self.fill_fifo(&mut operating, &mut actions);
                (PoolState::Operating(operating), actions)
            }
            WorkerRecoveryDecision::RetireRole => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Retired,
                    },
                );
                self.fill_fifo(&mut operating, &mut actions);
                self.return_unrecoverable_jobs(&mut operating, &mut actions);
                (PoolState::Operating(operating), actions)
            }
            WorkerRecoveryDecision::StopPool => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Retired,
                    },
                );
                self.begin_shutdown(operating, actions)
            }
        }
    }

    fn continue_after_pre_ready_stop(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let (state, mut actions) = self.recover_worker(
            operating, position, role, recoveries, previous, stopped, actions,
        );
        match state {
            PoolState::Operating(mut operating) => {
                self.authorize_waiting(&mut operating, &mut actions);
                (PoolState::Operating(operating), actions)
            }
            state => (state, actions),
        }
    }

    fn continue_after_returned_activation(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        previous: super::WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        returned_worker: super::WorkerAttempt,
        returned_attempt: super::worker::ActivationAttempt,
        outcome: WorkerActivationOutcome<W, P>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        actions
            .sends
            .diagnostics
            .append(InterpreterRequests::one(self.diagnostics.action(
                FifoDiagnostic::new(FifoDiagnosticCause::WorkerActivationReturned {
                    role: role.clone(),
                    worker: returned_worker,
                    activation: returned_attempt,
                    outcome,
                }),
            )));
        self.continue_after_pre_ready_stop(
            operating, position, role, recoveries, previous, stopped, actions,
        )
    }

    fn accept_worker_preparation(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        ),
    >
    where
        Role: Eq,
    {
        let (limit, release) = match &self.recovery {
            PoolRecoveryState::Permanent { limit, release, .. }
            | PoolRecoveryState::Transient { limit, release, .. } => (*limit, *release),
            PoolRecoveryState::Temporary { .. } => return Err((operating, input)),
        };
        let Some(position) = operating.preparation_position(&input) else {
            return Err((operating, input));
        };
        let member = operating.members.remove(position);
        let prepared = match member.accept_preparation(input) {
            Ok(WorkerRecoveryPreparation::Ready(prepared)) => prepared,
            Ok(WorkerRecoveryPreparation::Failed {
                role,
                recoveries,
                previous,
                stopped,
                source,
                error,
            }) => {
                return Ok(self.reject_preparation(
                    operating, position, role, recoveries, previous, stopped, source, error,
                ));
            }
            Err((member, input)) => {
                operating.members.insert(position, member);
                return Err((operating, input));
            }
        };
        let PreparedReplacement {
            role,
            mut recoveries,
            previous,
            stopped,
            source,
            submission,
        } = prepared;
        let budget = mem::replace(&mut self.restarts, RestartBudget::empty());
        match admit_restart(
            &mut recoveries,
            budget,
            limit,
            release,
            stopped.at,
            NonZeroUsize::MIN,
        ) {
            RestartAdmission::Proposed(proposal) => {
                let release = proposal.release();
                let release = match release {
                    RecoveryRelease::Immediate => match self.creations.issue() {
                        Some(creation) => Ok(WorkerReplacementRelease::Now(creation)),
                        None => Err(WorkerReplacementError::WorkerCreationsExhausted),
                    },
                    RecoveryRelease::Delayed(delay) => match self.issue_restart_timer() {
                        Some(timer) => Ok(WorkerReplacementRelease::After { timer, delay }),
                        None => Err(WorkerReplacementError::RestartTimersExhausted),
                    },
                };
                let release = match release {
                    Ok(release) => release,
                    Err(error) => {
                        self.restarts = proposal.decline();
                        return Ok(self.return_source_after_failed_replacement(
                            operating, position, role, recoveries, previous, stopped, source,
                            submission, error,
                        ));
                    }
                };
                if let Err(source) = self.recovery.restore_source(source) {
                    self.restarts = proposal.decline();
                    return Ok(self.reject_replacement(
                        operating,
                        position,
                        role,
                        recoveries,
                        previous,
                        stopped,
                        submission,
                        Some(source),
                        WorkerReplacementError::SourceStateCorrupt,
                        Actions::cont(),
                    ));
                }
                self.restarts = proposal.accept();
                let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                match release {
                    WorkerReplacementRelease::Now(creation) => {
                        let prepared = PreparedWorker { role, submission };
                        let (member, worker, observation) = Member::begin_replacement(
                            prepared,
                            recoveries,
                            previous.creation(),
                            creation,
                        );
                        operating.members.insert(position, member);
                        actions.creates.extend([worker]);
                        actions
                            .sends
                            .worker_observations
                            .append(InterpreterRequests::one(observation));
                    }
                    WorkerReplacementRelease::After { timer, delay } => {
                        operating.members.insert(
                            position,
                            Member {
                                role,
                                recoveries,
                                state: MemberState::Recovering(
                                    direct_worker::RecoveringWorker::Scheduling {
                                        previous,
                                        stopped,
                                        submission,
                                        timer,
                                    },
                                ),
                            },
                        );
                        actions.sends.restart_schedules.send(timer.after(delay));
                    }
                }
                let (operating, actions) = self.prepare_next_waiting(operating, actions);
                Ok((PoolState::Operating(operating), actions))
            }
            RestartAdmission::Denied { budget, reason } => {
                self.restarts = budget;
                Ok(self.return_source_after_failed_replacement(
                    operating,
                    position,
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    submission,
                    WorkerReplacementError::RestartDenied(reason),
                ))
            }
        }
    }

    fn accept_restart_schedule(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        input: behavior::ActionItemResult<ScheduleAfter>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            behavior::ActionItemResult<ScheduleAfter>,
        ),
    > {
        let Some(position) = operating.restart_schedule_position(&input) else {
            return Err((operating, input));
        };
        let member = operating.members.remove(position);
        let Member {
            role,
            recoveries,
            state,
        } = member;
        let MemberState::Recovering(worker) = state else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state,
                },
            );
            return Err((operating, input));
        };
        match worker.admit_restart_schedule(input) {
            Ok(ControlFlow::Continue(worker)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Recovering(worker),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            Ok(ControlFlow::Break((replacement, settlement))) => {
                let direct_worker::PendingWorkerReplacement {
                    previous,
                    stopped,
                    submission,
                } = replacement;
                Ok(self.reject_replacement(
                    operating,
                    position,
                    role,
                    recoveries,
                    previous,
                    stopped,
                    submission,
                    None,
                    WorkerReplacementError::RestartScheduleReturned(settlement),
                    Actions::cont(),
                ))
            }
            Err((worker, settlement)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Recovering(worker),
                    },
                );
                Err((operating, settlement))
            }
        }
    }

    fn accept_restart_timer(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        elapsed: crate::TimerElapsed,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            crate::TimerElapsed,
        ),
    > {
        let Some(position) = operating.restart_timer_position(&elapsed) else {
            return Err((operating, elapsed));
        };
        let member = operating.members.remove(position);
        let Member {
            role,
            recoveries,
            state,
        } = member;
        let MemberState::Recovering(worker) = state else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state,
                },
            );
            return Err((operating, elapsed));
        };
        let replacement = match worker.admit_restart_timer(elapsed) {
            Ok(replacement) => replacement,
            Err((worker, elapsed)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Recovering(worker),
                    },
                );
                return Err((operating, elapsed));
            }
        };
        let direct_worker::PendingWorkerReplacement {
            previous,
            stopped,
            submission,
        } = replacement;
        let Some(creation) = self.creations.issue() else {
            return Ok(self.reject_replacement(
                operating,
                position,
                role,
                recoveries,
                previous,
                stopped,
                submission,
                None,
                WorkerReplacementError::WorkerCreationsExhausted,
                Actions::cont(),
            ));
        };
        let prepared = PreparedWorker { role, submission };
        let (member, worker, observation) =
            Member::begin_replacement(prepared, recoveries, previous.creation(), creation);
        operating.members.insert(position, member);
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        actions.creates.extend([worker]);
        actions
            .sends
            .worker_observations
            .append(InterpreterRequests::one(observation));
        Ok((PoolState::Operating(operating), actions))
    }

    fn accept_assignment_receipt(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        receipt: AssignmentReceipt,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (Operating<Role, W, P, Job, WorkerResult>, AssignmentReceipt),
    > {
        let Some(position) = operating.assignment_receipt_position(&receipt) else {
            return Err((operating, receipt));
        };
        let Member {
            role,
            recoveries,
            state,
        } = operating.members.remove(position);
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Busy(assignment),
        }) = state
        else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state,
                },
            );
            return Err((operating, receipt));
        };
        let acceptance = match assignment.accept_receipt(receipt) {
            Ok(acceptance) => acceptance,
            Err((assignment, receipt)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Busy(assignment),
                        }),
                    },
                );
                return Err((operating, receipt));
            }
        };
        match acceptance {
            AssignmentReceiptOutcome::AwaitingCompletion(assignment) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Busy(assignment),
                        }),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            AssignmentReceiptOutcome::JobCompleted {
                customer,
                result,
                stopped,
            } => Ok(self.complete_assignment(
                operating, position, role, recoveries, current, customer, result, stopped,
            )),
            AssignmentReceiptOutcome::JobInterrupted {
                customer,
                stopped,
                late_completion,
            } => Ok(self.interrupt_assignment(
                operating,
                position,
                role,
                recoveries,
                current,
                customer,
                stopped,
                late_completion,
            )),
        }
    }

    fn accept_completion(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        child: behavior::CreationId,
        completion: Completion<WorkerResult>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            ChildReport<Completion<WorkerResult>>,
        ),
    > {
        let Some(position) = operating.completion_position(child, &completion) else {
            return Err((operating, ChildReport::new(child, completion)));
        };
        let Member {
            role,
            recoveries,
            state,
        } = operating.members.remove(position);
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Busy(assignment),
        }) = state
        else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state,
                },
            );
            return Err((operating, ChildReport::new(child, completion)));
        };
        let admission = match assignment.accept_completion(completion) {
            Ok(admission) => admission,
            Err((assignment, completion)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Busy(assignment),
                        }),
                    },
                );
                return Err((operating, ChildReport::new(child, completion)));
            }
        };
        match admission {
            WorkerCompletionOutcome::AwaitingReceipt(assignment) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Busy(assignment),
                        }),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            WorkerCompletionOutcome::JobCompleted {
                customer,
                result,
                stopped,
            } => Ok(self.complete_assignment(
                operating, position, role, recoveries, current, customer, result, stopped,
            )),
        }
    }

    fn request_worker_shutdown(
        &self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        shutdown: crate::ShutdownId,
        mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        Operating<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        actions
            .sends
            .worker_shutdowns
            .append(InterpreterRequests::one(current.shutdown_request(shutdown)));
        operating.members.insert(
            position,
            Member {
                role,
                recoveries,
                state: MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::Stopping(ShutdownJoin::AwaitingBoth(shutdown)),
                }),
            },
        );
        (operating, actions)
    }

    fn reject_assignment_delivery(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        item: AssignWorker<W::Protocol, Job>,
        reason: behavior::ExactDeliveryReason,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let receipt = item.receipt();
        let Some(position) = operating.assignment_receipt_position(&receipt) else {
            return self.diagnose(
                PoolState::Operating(operating),
                FifoEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason,
                })),
            );
        };
        let Some(shutdown) = self
            .shutdowns
            .reserve(1)
            .and_then(|reserved| reserved.into_iter().next())
        else {
            let (state, actions) = self.diagnose(
                PoolState::Operating(operating),
                FifoEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason,
                })),
            );
            return match state {
                PoolState::Operating(operating) => self.begin_shutdown(operating, actions),
                state => (state, actions),
            };
        };
        let (target, returned, receipt) = item.into_parts();
        let Member {
            role,
            recoveries,
            state,
        } = operating.members.remove(position);
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Busy(assignment),
        }) = state
        else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state,
                },
            );
            return self.diagnose(
                PoolState::Operating(operating),
                FifoEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item: AssignWorker::returned(target, returned, receipt),
                    reason,
                })),
            );
        };
        let rejected = match assignment.accept_rejection(returned) {
            Ok(rejected) => rejected,
            Err((assignment, returned)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Busy(assignment),
                        }),
                    },
                );
                return self.diagnose(
                    PoolState::Operating(operating),
                    FifoEvent::AssignmentSettled(SettledItem::Attempted(
                        ItemSettlement::Rejected {
                            item: AssignWorker::returned(target, returned, receipt),
                            reason,
                        },
                    )),
                );
            }
        };
        match rejected {
            AssignmentRejectionOutcome::JobReturned { customer, stopped } => {
                operating.backlog.insert(
                    customer.admitted,
                    QueuedJob {
                        customer,
                        assigned_role: Some(role.clone()),
                    },
                );
                let actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                match stopped {
                    None => {
                        let (mut operating, mut actions) = self.request_worker_shutdown(
                            operating, position, role, recoveries, current, shutdown, actions,
                        );
                        self.fill_fifo(&mut operating, &mut actions);
                        (PoolState::Operating(operating), actions)
                    }
                    Some(stopped) => self.recover_worker(
                        operating,
                        position,
                        role,
                        recoveries,
                        current.attempt,
                        stopped,
                        actions,
                    ),
                }
            }
            AssignmentRejectionOutcome::ConflictingCompletion {
                customer,
                assignment,
                completion,
                stopped,
            } => {
                let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                actions.sends.customer_outcomes =
                    customer.customer.deliver(FifoOutcome::returned_assigned(
                        customer.id,
                        role.clone(),
                        customer.payload,
                        AssignedReturnReason::ContradictoryAssignmentSettlement,
                    ));
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics
                        .action(FifoDiagnostic::new(FifoDiagnosticCause::Unexpected(
                            FifoEvent::AssignmentSettled(SettledItem::Attempted(
                                ItemSettlement::Rejected {
                                    item: AssignWorker::returned(target, assignment, receipt),
                                    reason,
                                },
                            )),
                        ))),
                ));
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics
                        .action(FifoDiagnostic::new(FifoDiagnosticCause::Unexpected(
                            FifoEvent::WorkerCompleted(ChildReport::new(
                                current.attempt.creation(),
                                completion,
                            )),
                        ))),
                ));
                match stopped {
                    None => {
                        let (operating, actions) = self.request_worker_shutdown(
                            operating, position, role, recoveries, current, shutdown, actions,
                        );
                        self.begin_shutdown(operating, actions)
                    }
                    Some(stopped) => {
                        let (state, actions) = self.recover_worker(
                            operating,
                            position,
                            role,
                            recoveries,
                            current.attempt,
                            stopped,
                            actions,
                        );
                        match state {
                            PoolState::Operating(operating) => {
                                self.begin_shutdown(operating, actions)
                            }
                            state => (state, actions),
                        }
                    }
                }
            }
        }
    }

    fn accept_worker_stop(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            crate::ChildStopped<BehaviorAddr<W>>,
        ),
    > {
        let Some(position) = operating.worker_position(stopped.child) else {
            return Err((operating, stopped));
        };
        let Member {
            role,
            recoveries,
            state,
        } = operating.members.remove(position);
        let (current, phase) = match state {
            MemberState::Creating(mut worker) => {
                if worker.stopped.is_none() {
                    worker.stopped = Some(stopped);
                    operating.members.insert(
                        position,
                        Member {
                            role,
                            recoveries,
                            state: MemberState::Creating(worker),
                        },
                    );
                    return Ok((PoolState::Operating(operating), Actions::cont()));
                }
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Creating(worker),
                    },
                );
                return Err((operating, stopped));
            }
            MemberState::Worker(Worker { current, phase }) => (current, phase),
            state @ (MemberState::Recovering(_) | MemberState::Retired) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state,
                    },
                );
                return Err((operating, stopped));
            }
        };
        match phase {
            WorkerPhase::Initializing {
                initialization,
                stopped: None,
            } => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Initializing {
                                initialization,
                                stopped: Some(stopped),
                            },
                        }),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            WorkerPhase::WaitingForActivation { permit, activation } => {
                let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::UnusedActivation {
                            role: role.clone(),
                            permit: Some(permit),
                            activation,
                        },
                    )),
                ));
                Ok(self.continue_after_pre_ready_stop(
                    operating,
                    position,
                    role,
                    recoveries,
                    current.attempt,
                    stopped,
                    actions,
                ))
            }
            WorkerPhase::ActivationDispatched {
                attempt,
                stopped: None,
            } => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::ActivationDispatched {
                                attempt,
                                stopped: Some(stopped),
                            },
                        }),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            WorkerPhase::Activating {
                attempt,
                stopped: None,
            } => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Activating {
                                attempt,
                                stopped: Some(stopped),
                            },
                        }),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            WorkerPhase::Idle => Ok(self.recover_worker(
                operating,
                position,
                role,
                recoveries,
                current.attempt,
                stopped,
                Actions::cont(),
            )),
            WorkerPhase::Busy(assignment) => {
                let admission = match assignment.accept_worker_exit(stopped) {
                    Ok(admission) => admission,
                    Err((assignment, stopped)) => {
                        operating.members.insert(
                            position,
                            Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Busy(assignment),
                                }),
                            },
                        );
                        return Err((operating, stopped));
                    }
                };
                match admission {
                    WorkerExitOutcome::AwaitingReceipt(assignment) => {
                        operating.members.insert(
                            position,
                            Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Busy(assignment),
                                }),
                            },
                        );
                        Ok((PoolState::Operating(operating), Actions::cont()))
                    }
                    WorkerExitOutcome::JobInterrupted {
                        customer,
                        stopped,
                        late_completion,
                    } => Ok(self.interrupt_assignment(
                        operating,
                        position,
                        role,
                        recoveries,
                        current,
                        customer,
                        stopped,
                        late_completion,
                    )),
                }
            }
            WorkerPhase::Stopping(join) => match join.stopped(stopped, &current.attempt) {
                Ok(core::ops::ControlFlow::Continue(join)) => {
                    operating.members.insert(
                        position,
                        Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Stopping(join),
                            }),
                        },
                    );
                    Ok((PoolState::Operating(operating), Actions::cont()))
                }
                Ok(core::ops::ControlFlow::Break((shutdown, stopped))) => Ok(self
                    .finish_worker_shutdown(
                        operating, position, role, recoveries, current, shutdown, stopped,
                    )),
                Err((join, stopped)) => {
                    operating.members.insert(
                        position,
                        Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Stopping(join),
                            }),
                        },
                    );
                    Err((operating, stopped))
                }
            },
            phase @ (WorkerPhase::Initializing {
                stopped: Some(_), ..
            }
            | WorkerPhase::ActivationDispatched {
                stopped: Some(_), ..
            }
            | WorkerPhase::Activating {
                stopped: Some(_), ..
            }) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker { current, phase }),
                    },
                );
                Err((operating, stopped))
            }
        }
    }

    fn finish_worker_shutdown(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        match shutdown {
            EstablishedShutdownResolved::Accepted { .. } => {}
            shutdown @ EstablishedShutdownResolved::Rejected { .. } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::WorkerShutdownRejected {
                            role: role.clone(),
                            shutdown,
                        },
                    )),
                ));
            }
        }
        self.recover_worker(
            operating,
            position,
            role,
            recoveries,
            current.attempt,
            stopped,
            actions,
        )
    }

    fn accept_worker_shutdown(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            EstablishedShutdownResolved<W::Protocol>,
        ),
    > {
        let Some(position) = operating.worker_shutdown_position(shutdown.id()) else {
            return Err((operating, shutdown));
        };
        let Member {
            role,
            recoveries,
            state,
        } = operating.members.remove(position);
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Stopping(join),
        }) = state
        else {
            operating.members.insert(
                position,
                Member {
                    role,
                    recoveries,
                    state,
                },
            );
            return Err((operating, shutdown));
        };
        match join.settled(shutdown) {
            Ok(core::ops::ControlFlow::Continue(join)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Stopping(join),
                        }),
                    },
                );
                Ok((PoolState::Operating(operating), Actions::cont()))
            }
            Ok(core::ops::ControlFlow::Break((shutdown, stopped))) => Ok(self
                .finish_worker_shutdown(
                    operating, position, role, recoveries, current, shutdown, stopped,
                )),
            Err((join, shutdown)) => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Stopping(join),
                        }),
                    },
                );
                Err((operating, shutdown))
            }
        }
    }

    fn complete_assignment(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        customer: CustomerJob<Job, CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>>,
        result: WorkerResult,
        stopped: Option<crate::ChildStopped<BehaviorAddr<W>>>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        actions.sends.customer_outcomes =
            customer
                .customer
                .deliver(FifoOutcome::completed(customer.id, role.clone(), result));
        match stopped {
            None => {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Idle,
                        }),
                    },
                );
                self.fill_fifo(&mut operating, &mut actions);
                (PoolState::Operating(operating), actions)
            }
            Some(stopped) => self.recover_worker(
                operating,
                position,
                role,
                recoveries,
                current.attempt,
                stopped,
                actions,
            ),
        }
    }

    fn interrupt_assignment(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: super::restart::RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        customer: CustomerJob<Job, CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult>>,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        late_completion: Option<Completion<WorkerResult>>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        if let Some(completion) = late_completion {
            let input = FifoEvent::WorkerCompleted(ChildReport::new(
                current.attempt.creation(),
                completion,
            ));
            actions
                .sends
                .diagnostics
                .append(InterpreterRequests::one(self.diagnostics.action(
                    FifoDiagnostic::new(FifoDiagnosticCause::Unexpected(input)),
                )));
        }
        match self.interruption {
            Interruption::Fail => {
                actions.sends.customer_outcomes =
                    customer.customer.deliver(FifoOutcome::returned_assigned(
                        customer.id,
                        role.clone(),
                        customer.payload,
                        AssignedReturnReason::WorkerStopped,
                    ));
            }
            Interruption::Retry => {
                operating.backlog.insert(
                    customer.admitted,
                    QueuedJob {
                        customer,
                        assigned_role: Some(role.clone()),
                    },
                );
            }
        }
        self.recover_worker(
            operating,
            position,
            role,
            recoveries,
            current.attempt,
            stopped,
            actions,
        )
    }

    fn authorize_waiting(
        &mut self,
        operating: &mut Operating<Role, W, P, Job, WorkerResult>,
        actions: &mut FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let occupied = operating
            .members
            .iter()
            .filter_map(|member| match &member.state {
                MemberState::Worker(Worker {
                    phase:
                        WorkerPhase::ActivationDispatched { .. }
                        | WorkerPhase::Activating { .. },
                    ..
                }) => Some(()),
                MemberState::Creating(_)
                | MemberState::Worker(_)
                | MemberState::Recovering(_)
                | MemberState::Retired => None,
            })
            .count();
        let available = self.activation.maximum().saturating_sub(occupied);
        for _ in 0..available {
            let position = operating
                .members
                .iter()
                .enumerate()
                .find_map(|(position, member)| match &member.state {
                    MemberState::Worker(Worker {
                        phase: WorkerPhase::WaitingForActivation { .. },
                        ..
                    }) => Some(position),
                    MemberState::Creating(_)
                    | MemberState::Worker(_)
                    | MemberState::Recovering(_)
                    | MemberState::Retired => None,
                });
            let Some(position) = position else {
                return;
            };
            let member = operating.members.remove(position);
            match member.authorize_activation() {
                Ok((member, request)) => {
                    operating.members.insert(position, member);
                    actions
                        .sends
                        .worker_activations
                        .append(InterpreterRequests::one(request));
                }
                Err(member) => {
                    operating.members.insert(position, member);
                    return;
                }
            }
        }
    }

    fn accept_initialization(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        mut input: super::WorkerInitializationReport<W, P>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            super::WorkerInitializationReport<W, P>,
        ),
    > {
        if let Some(position) = operating
            .members
            .iter()
            .position(|member| member.worker_attempt() == Some(input.worker()))
        {
            let member = operating.members.remove(position);
            match member.admit_initialization(input) {
                Ok(member) => {
                    operating.members.insert(position, member);
                    let mut actions = Actions::cont();
                    self.authorize_waiting(&mut operating, &mut actions);
                    return Ok((PoolState::Operating(operating), actions));
                }
                Err((member, returned)) => {
                    operating.members.insert(position, member);
                    input = returned;
                }
            }
        }
        for position in 0..operating.members.len() {
            let Member {
                role,
                recoveries,
                state,
            } = operating.members.remove(position);
            let MemberState::Worker(Worker {
                current,
                phase:
                    WorkerPhase::Initializing {
                        initialization,
                        stopped,
                    },
            }) = state
            else {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state,
                    },
                );
                continue;
            };
            if &initialization != input.initialization() {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Initializing {
                                initialization,
                                stopped,
                            },
                        }),
                    },
                );
                continue;
            }
            match (stopped, input) {
                (
                    None,
                    super::WorkerInitializationReport::Stopped {
                        activation,
                        stopped,
                        ..
                    },
                ) if stopped.child == current.attempt.creation() => {
                    let mut actions: FifoActions<
                        Role,
                        W,
                        P,
                        Source,
                        Diagnostics,
                        Job,
                        WorkerResult,
                    > = Actions::cont();
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::UnusedActivation {
                                role: role.clone(),
                                permit: None,
                                activation,
                            },
                        )),
                    ));
                    return Ok(self.continue_after_pre_ready_stop(
                        operating,
                        position,
                        role,
                        recoveries,
                        current.attempt,
                        stopped,
                        actions,
                    ));
                }
                (Some(stopped), returned) => {
                    let mut actions: FifoActions<
                        Role,
                        W,
                        P,
                        Source,
                        Diagnostics,
                        Job,
                        WorkerResult,
                    > = Actions::cont();
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::Unexpected(FifoEvent::WorkerInitialization(
                                returned,
                            )),
                        )),
                    ));
                    return Ok(self.continue_after_pre_ready_stop(
                        operating,
                        position,
                        role,
                        recoveries,
                        current.attempt,
                        stopped,
                        actions,
                    ));
                }
                (None, returned @ super::WorkerInitializationReport::EffectsRejected { .. }) => {
                    let mut actions: FifoActions<
                        Role,
                        W,
                        P,
                        Source,
                        Diagnostics,
                        Job,
                        WorkerResult,
                    > = Actions::cont();
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::Unexpected(FifoEvent::WorkerInitialization(
                                returned,
                            )),
                        )),
                    ));
                    let shutdown = self
                        .shutdowns
                        .reserve(1)
                        .and_then(|reserved| reserved.into_iter().next());
                    let Some(shutdown) = shutdown else {
                        operating.members.insert(
                            position,
                            Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Initializing {
                                        initialization,
                                        stopped: None,
                                    },
                                }),
                            },
                        );
                        return Ok(self.begin_shutdown(operating, actions));
                    };
                    let (operating, actions) = self.request_worker_shutdown(
                        operating, position, role, recoveries, current, shutdown, actions,
                    );
                    return Ok((PoolState::Operating(operating), actions));
                }
                (None, returned) => {
                    input = returned;
                    operating.members.insert(
                        position,
                        Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Initializing {
                                    initialization,
                                    stopped: None,
                                },
                            }),
                        },
                    );
                }
            }
        }
        Err((operating, input))
    }

    fn accept_activation(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        mut input: super::WorkerActivation<W, P>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            super::WorkerActivation<W, P>,
        ),
    > {
        if let Some(position) = operating
            .members
            .iter()
            .position(|member| member.worker_attempt() == Some(&input.worker()))
        {
            let member = operating.members.remove(position);
            match member.admit_activation(input) {
                Ok(member) => {
                    let mut actions = Actions::cont();
                    match &member.state {
                        MemberState::Worker(Worker {
                            phase: WorkerPhase::Idle,
                            ..
                        }) => {
                            operating.members.insert(position, member);
                            self.fill_fifo(&mut operating, &mut actions);
                        }
                        MemberState::Creating(_)
                        | MemberState::Worker(_)
                        | MemberState::Recovering(_)
                        | MemberState::Retired => {
                            operating.members.insert(position, member);
                        }
                    }
                    self.authorize_waiting(&mut operating, &mut actions);
                    return Ok((PoolState::Operating(operating), actions));
                }
                Err((member, returned)) => {
                    operating.members.insert(position, member);
                    input = returned;
                }
            }
        }
        for position in 0..operating.members.len() {
            let Member {
                role,
                recoveries,
                state,
            } = operating.members.remove(position);
            let MemberState::Worker(Worker { current, phase }) = state else {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state,
                    },
                );
                continue;
            };
            if current.attempt != input.worker() {
                operating.members.insert(
                    position,
                    Member {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker { current, phase }),
                    },
                );
                continue;
            }
            match phase {
                WorkerPhase::ActivationDispatched {
                    attempt,
                    stopped: None,
                } => {
                    let (returned_worker, returned_attempt, outcome) = input.into_parts();
                    let mut actions: FifoActions<
                        Role,
                        W,
                        P,
                        Source,
                        Diagnostics,
                        Job,
                        WorkerResult,
                    > = Actions::cont();
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::WorkerActivationReturned {
                                role: role.clone(),
                                worker: returned_worker,
                                activation: returned_attempt,
                                outcome,
                            },
                        )),
                    ));
                    let shutdown = self
                        .shutdowns
                        .reserve(1)
                        .and_then(|reserved| reserved.into_iter().next());
                    let Some(shutdown) = shutdown else {
                        operating.members.insert(
                            position,
                            Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::ActivationDispatched {
                                        attempt,
                                        stopped: None,
                                    },
                                }),
                            },
                        );
                        return Ok(self.begin_shutdown(operating, actions));
                    };
                    let (mut operating, mut actions) = self.request_worker_shutdown(
                        operating, position, role, recoveries, current, shutdown, actions,
                    );
                    self.authorize_waiting(&mut operating, &mut actions);
                    return Ok((PoolState::Operating(operating), actions));
                }
                WorkerPhase::Activating {
                    attempt,
                    stopped: None,
                } => {
                    let (returned_worker, returned_attempt, outcome) = input.into_parts();
                    match outcome {
                        WorkerActivationOutcome::Started => {
                            let mut actions: FifoActions<
                                Role,
                                W,
                                P,
                                Source,
                                Diagnostics,
                                Job,
                                WorkerResult,
                            > = Actions::cont();
                            actions.sends.diagnostics.append(InterpreterRequests::one(
                                self.diagnostics.action(FifoDiagnostic::new(
                                    FifoDiagnosticCause::WorkerActivationReturned {
                                        role: role.clone(),
                                        worker: returned_worker,
                                        activation: returned_attempt,
                                        outcome: WorkerActivationOutcome::Started,
                                    },
                                )),
                            ));
                            operating.members.insert(
                                position,
                                Member {
                                    role,
                                    recoveries,
                                    state: MemberState::Worker(Worker {
                                        current,
                                        phase: WorkerPhase::Activating {
                                            attempt,
                                            stopped: None,
                                        },
                                    }),
                                },
                            );
                            return Ok((PoolState::Operating(operating), actions));
                        }
                        outcome @ (WorkerActivationOutcome::StartRejected { .. }
                        | WorkerActivationOutcome::Ready(_)
                        | WorkerActivationOutcome::Rejected(_)) => {
                            let mut actions: FifoActions<
                                Role,
                                W,
                                P,
                                Source,
                                Diagnostics,
                                Job,
                                WorkerResult,
                            > = Actions::cont();
                            actions.sends.diagnostics.append(InterpreterRequests::one(
                                self.diagnostics.action(FifoDiagnostic::new(
                                    FifoDiagnosticCause::WorkerActivationReturned {
                                        role: role.clone(),
                                        worker: returned_worker,
                                        activation: returned_attempt,
                                        outcome,
                                    },
                                )),
                            ));
                            let shutdown = self
                                .shutdowns
                                .reserve(1)
                                .and_then(|reserved| reserved.into_iter().next());
                            let Some(shutdown) = shutdown else {
                                operating.members.insert(
                                    position,
                                    Member {
                                        role,
                                        recoveries,
                                        state: MemberState::Worker(Worker {
                                            current,
                                            phase: WorkerPhase::Activating {
                                                attempt,
                                                stopped: None,
                                            },
                                        }),
                                    },
                                );
                                return Ok(self.begin_shutdown(operating, actions));
                            };
                            let (mut operating, mut actions) = self.request_worker_shutdown(
                                operating, position, role, recoveries, current, shutdown, actions,
                            );
                            self.authorize_waiting(&mut operating, &mut actions);
                            return Ok((PoolState::Operating(operating), actions));
                        }
                    }
                }
                WorkerPhase::ActivationDispatched {
                    attempt: _,
                    stopped: Some(stopped),
                } => {
                    let (returned_worker, returned_attempt, outcome) = input.into_parts();
                    match outcome {
                        outcome @ (WorkerActivationOutcome::Started
                        | WorkerActivationOutcome::StartRejected { .. }
                        | WorkerActivationOutcome::Ready(_)
                        | WorkerActivationOutcome::Rejected(_)) => {
                            return Ok(self.continue_after_returned_activation(
                                operating,
                                position,
                                role,
                                recoveries,
                                current.attempt,
                                stopped,
                                returned_worker,
                                returned_attempt,
                                outcome,
                            ));
                        }
                    }
                }
                WorkerPhase::Activating {
                    attempt,
                    stopped: Some(stopped),
                } => {
                    let (returned_worker, returned_attempt, outcome) = input.into_parts();
                    match outcome {
                        WorkerActivationOutcome::Started => {
                            let mut actions: FifoActions<
                                Role,
                                W,
                                P,
                                Source,
                                Diagnostics,
                                Job,
                                WorkerResult,
                            > = Actions::cont();
                            actions.sends.diagnostics.append(InterpreterRequests::one(
                                self.diagnostics.action(FifoDiagnostic::new(
                                    FifoDiagnosticCause::WorkerActivationReturned {
                                        role: role.clone(),
                                        worker: returned_worker,
                                        activation: returned_attempt,
                                        outcome: WorkerActivationOutcome::Started,
                                    },
                                )),
                            ));
                            operating.members.insert(
                                position,
                                Member {
                                    role,
                                    recoveries,
                                    state: MemberState::Worker(Worker {
                                        current,
                                        phase: WorkerPhase::Activating {
                                            attempt,
                                            stopped: Some(stopped),
                                        },
                                    }),
                                },
                            );
                            return Ok((PoolState::Operating(operating), actions));
                        }
                        outcome @ (WorkerActivationOutcome::StartRejected { .. }
                        | WorkerActivationOutcome::Ready(_)
                        | WorkerActivationOutcome::Rejected(_)) => {
                            return Ok(self.continue_after_returned_activation(
                                operating,
                                position,
                                role,
                                recoveries,
                                current.attempt,
                                stopped,
                                returned_worker,
                                returned_attempt,
                                outcome,
                            ));
                        }
                    }
                }
                phase => {
                    operating.members.insert(
                        position,
                        Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker { current, phase }),
                        },
                    );
                }
            }
        }
        Err((operating, input))
    }

    fn reject_creation_batch(
        &self,
        operating: &mut Operating<Role, W, P, Job, WorkerResult>,
        creations: Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
        rejection: CreationBatchRejection,
    ) -> Result<
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    > {
        let Some(positions) = operating.creation_positions(
            creations
                .iter()
                .map(|creation| (creation.id(), creation.kind())),
        ) else {
            return Err(rejection.settlement(creations));
        };
        let mut returned: BTreeMap<_, _> = positions.into_iter().zip(creations).collect();
        let mut members = Vec::with_capacity(operating.members.len());
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        for (position, member) in operating.members.drain(..).enumerate() {
            let Some(creation) = returned.remove(&position) else {
                members.push(member);
                continue;
            };
            let Member {
                role,
                recoveries,
                state,
            } = member;
            match state {
                MemberState::Creating(worker) => {
                    let (_, returned_worker, _) = creation.into_parts();
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::WorkerReturned {
                                role: role.clone(),
                                rejection: rejection.worker(returned_worker.into_inner()),
                                activation: worker.activation,
                                stopped: worker.stopped,
                            },
                        )),
                    ));
                    members.push(Member {
                        role,
                        recoveries,
                        state: MemberState::Retired,
                    });
                }
                state => {
                    members.push(Member {
                        role,
                        recoveries,
                        state,
                    });
                    let workers = rejection.settlement(Creations::one(creation));
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::WorkersReturned(workers),
                        )),
                    ));
                }
            }
        }
        operating.members = members;
        Ok(actions)
    }

    fn accept_creations(
        &mut self,
        mut operating: Operating<Role, W, P, Job, WorkerResult>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> Result<
        (
            PoolState<Role, W, P, Job, WorkerResult>,
            FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Job, WorkerResult>,
            CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
        ),
    > {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        let mut failure = None;
        match workers.into_settlement() {
            CreationSettlement::Corrupt { creations, fault } => {
                let rejection = CreationBatchRejection::InterpreterCorrupt(fault);
                match self.reject_creation_batch(&mut operating, creations, rejection) {
                    Ok(returned) => {
                        actions = returned;
                        failure = Some(self.recovery.failure());
                    }
                    Err(workers) => return Err((operating, workers)),
                }
            }
            CreationSettlement::Settled(settlements) => {
                let identities = settlements
                    .iter()
                    .map(creation_identity)
                    .collect::<Option<Vec<_>>>();
                let Some(positions) =
                    identities.and_then(|identities| operating.creation_positions(identities))
                else {
                    return Err((
                        operating,
                        CreationsSettled::new(CreationSettlement::Settled(settlements)),
                    ));
                };
                let mut returned: BTreeMap<_, _> = positions.into_iter().zip(settlements).collect();
                let mut members = Vec::with_capacity(operating.members.len());
                for (position, member) in operating.members.into_iter().enumerate() {
                    let Some(settlement) = returned.remove(&position) else {
                        members.push(member);
                        continue;
                    };
                    match member.admit_creation(ChildCreationSettled::new(settlement)) {
                        WorkerCreationAdmission::Initializing { member, request } => {
                            members.push(member);
                            actions
                                .sends
                                .worker_initializations
                                .append(InterpreterRequests::one(request));
                        }
                        WorkerCreationAdmission::Returned {
                            member,
                            role,
                            rejection,
                            activation,
                            stopped,
                        } => {
                            members.push(member);
                            actions.sends.diagnostics.append(InterpreterRequests::one(
                                self.diagnostics.action(FifoDiagnostic::new(
                                    FifoDiagnosticCause::WorkerReturned {
                                        role,
                                        rejection,
                                        activation,
                                        stopped,
                                    },
                                )),
                            ));
                            failure = Some(self.recovery.failure());
                        }
                        WorkerCreationAdmission::Unrelated { member, creation } => {
                            members.push(member);
                            let workers = CreationsSettled::new(CreationSettlement::Settled(
                                Creations::one(creation.into_settlement()),
                            ));
                            actions.sends.diagnostics.append(InterpreterRequests::one(
                                self.diagnostics.action(FifoDiagnostic::new(
                                    FifoDiagnosticCause::WorkersReturned(workers),
                                )),
                            ));
                        }
                    }
                }
                operating.members = members;
            }
            CreationSettlement::Rejected {
                creations,
                reason: _,
            } => {
                let rejection = CreationBatchRejection::NamespaceExhausted;
                match self.reject_creation_batch(&mut operating, creations, rejection) {
                    Ok(returned) => {
                        actions = returned;
                        failure = Some(self.recovery.failure());
                    }
                    Err(workers) => return Err((operating, workers)),
                }
            }
        }

        match failure {
            None => Ok((PoolState::Operating(operating), actions)),
            Some(PoolFailureReaction::RetireRole) => {
                self.fill_fifo(&mut operating, &mut actions);
                self.return_unrecoverable_jobs(&mut operating, &mut actions);
                Ok((PoolState::Operating(operating), actions))
            }
            Some(PoolFailureReaction::StopPool) => Ok(self.begin_shutdown(operating, actions)),
        }
    }

    fn issue_restart_timer(&mut self) -> Option<ScheduleKey> {
        ScheduleKey::issue(&mut self.next_restart_timer)
    }

    fn begin_shutdown(
        &mut self,
        operating: Operating<Role, W, P, Job, WorkerResult>,
        mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let Operating {
            members,
            backlog,
            cursor: _,
        } = operating;
        for (_, queued) in backlog {
            let CustomerJob {
                id,
                admitted: _,
                payload,
                customer,
            } = queued.customer;
            actions
                .sends
                .customer_outcomes
                .append(customer.deliver(FifoOutcome::returned_queued(
                    id,
                    payload,
                    QueuedReturnReason::PoolShutdown,
                )));
        }

        let required = members.iter().filter_map(Member::shutdown_target).count();
        let reserved = self.shutdowns.reserve(required);
        let mut ids = match reserved {
            Some(ids) => Some(ids.into_iter()),
            None => None,
        };
        let mut draining = Vec::with_capacity(members.len());
        for member in members {
            let role = member.role_name();
            let id = match (&mut ids, member.shutdown_target()) {
                (Some(ids), Some(_)) => ids.next(),
                _ => None,
            };
            let direct_worker::MemberRetirement {
                member,
                request,
                custody,
            } = member.shutdown(id);
            if let Some(request) = request {
                actions
                    .sends
                    .worker_shutdowns
                    .append(InterpreterRequests::one(request));
            }
            match custody {
                direct_worker::RetirementReturns::NoTransfer => {}
                direct_worker::RetirementReturns::Assignment(AssignmentShutdown {
                    customer,
                    worker,
                    stopped: _,
                    completion,
                }) => {
                    actions
                        .sends
                        .customer_outcomes
                        .append(customer.customer.deliver(FifoOutcome::returned_assigned(
                            customer.id,
                            role,
                            customer.payload,
                            AssignedReturnReason::PoolShutdown,
                        )));
                    if let Some(completion) = completion {
                        let input = FifoEvent::WorkerCompleted(ChildReport::new(
                            worker.creation(),
                            completion,
                        ));
                        actions.sends.diagnostics.append(InterpreterRequests::one(
                            self.diagnostics.action(FifoDiagnostic::new(
                                FifoDiagnosticCause::Unexpected(input),
                            )),
                        ));
                    }
                }
                direct_worker::RetirementReturns::Replacement(replacement) => {
                    self.retain_cancelled_replacement(
                        role,
                        replacement,
                        WorkerReplacementError::PoolShutdown,
                        &mut actions,
                    );
                }
            }
            draining.push(member);
        }

        match ids {
            None => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    PoolState::ForcedRetirement {
                        members: draining,
                        cause: ForcedRetirementCause::WorkerShutdownIdsExhausted,
                    },
                    actions,
                )
            }
            Some(_) => match draining.iter().find_map(RetiringWorker::waiting) {
                None => {
                    actions.become_ = behavior::Step::Stop(behavior::Stopped);
                    (PoolState::Stopped, actions)
                }
                Some(_) => {
                    let (deadline, schedule) = ShutdownDeadline::begin(self.actor_drain);
                    if let Some(schedule) = schedule {
                        actions.sends.restart_schedules.send(schedule);
                    }
                    (
                        PoolState::Draining {
                            members: draining,
                            deadline,
                        },
                        actions,
                    )
                }
            },
        }
    }

    fn retain_drained_worker(
        &self,
        worker: WorkerDeparture<Role, W, P>,
        actions: &mut FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let WorkerDeparture {
            role,
            startup,
            shutdown,
        } = worker;
        match startup {
            Some(direct_worker::WorkerStartupCustody::ActivationPlan(activation)) => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::UnusedActivation {
                            role: role.clone(),
                            permit: None,
                            activation,
                        },
                    )),
                ));
            }
            Some(direct_worker::WorkerStartupCustody::ActivationPermit { permit, activation }) => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::UnusedActivation {
                            role: role.clone(),
                            permit: Some(permit),
                            activation,
                        },
                    )),
                ));
            }
            Some(
                direct_worker::WorkerStartupCustody::Initializing(_)
                | direct_worker::WorkerStartupCustody::ActivationStart(_)
                | direct_worker::WorkerStartupCustody::Activation(_),
            )
            | None => {}
        }
        match shutdown {
            EstablishedShutdownResolved::Accepted { .. } => {}
            shutdown @ EstablishedShutdownResolved::Rejected { .. } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::WorkerShutdownRejected { role, shutdown },
                    )),
                ));
            }
        }
    }

    fn retain_worker(
        &mut self,
        custody: WorkerCustody<Role, W, P>,
        workers: &mut Vec<RetiringWorker<Role, W, P>>,
        actions: &mut FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> Option<ForcedRetirementCause> {
        match custody {
            WorkerCustody::Running {
                role,
                worker,
                activation,
            } => {
                let reserved = self.shutdowns.reserve(1);
                let shutdown = reserved.and_then(|mut shutdowns| shutdowns.pop());
                let exhausted = match shutdown {
                    Some(_) => None,
                    None => Some(ForcedRetirementCause::WorkerShutdownIdsExhausted),
                };
                let (worker, request) = direct_worker::retire_worker(
                    role,
                    worker,
                    Some(direct_worker::WorkerStartupCustody::ActivationPlan(
                        activation,
                    )),
                    shutdown,
                );
                workers.push(worker);
                if let Some(request) = request {
                    actions
                        .sends
                        .worker_shutdowns
                        .append(InterpreterRequests::one(request));
                }
                exhausted
            }
            WorkerCustody::Returned {
                role,
                rejection,
                activation,
                stopped,
            } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::WorkerReturned {
                            role,
                            rejection,
                            activation,
                            stopped,
                        },
                    )),
                ));
                None
            }
            WorkerCustody::Stopped {
                role,
                worker,
                activation,
                stopped,
            } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::StoppedWorkerEstablished {
                            role,
                            worker,
                            activation,
                            stopped,
                        },
                    )),
                ));
                None
            }
        }
    }

    fn retain_cancelled_replacement(
        &self,
        role: RoleName<Role>,
        replacement: direct_worker::PendingWorkerReplacement<W, P>,
        error: WorkerReplacementError,
        actions: &mut FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let direct_worker::PendingWorkerReplacement {
            previous,
            stopped,
            submission,
        } = replacement;
        actions
            .sends
            .diagnostics
            .append(InterpreterRequests::one(self.diagnostics.action(
                FifoDiagnostic::new(FifoDiagnosticCause::WorkerReplacementFailed {
                    role,
                    previous,
                    stopped,
                    submission,
                    returned_source: None,
                    error,
                }),
            )));
    }

    fn reject_drain_creations(
        &mut self,
        mut members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        creations: Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
        rejection: CreationBatchRejection,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let positions = direct_worker::ordered_creation_positions(
            members.iter().map(RetiringWorker::expected_creation),
            creations
                .iter()
                .map(|creation| (creation.id(), creation.kind())),
        );
        let Some(positions) = positions else {
            return self.diagnose(
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerCreationsSettled(rejection.settlement(creations)),
            );
        };
        let mut returned: BTreeMap<_, _> = positions.into_iter().zip(creations).collect();
        let mut draining = Vec::with_capacity(members.len());
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        let mut forced = None;
        for (position, member) in members.drain(..).enumerate() {
            let Some(creation) = returned.remove(&position) else {
                draining.push(member);
                continue;
            };
            let (id, returned_worker, kind) = creation.into_parts();
            let returned_worker = rejection.worker(returned_worker.into_inner());
            match member.accept_creation_rejection(id, kind, returned_worker) {
                Ok(custody) => {
                    let exhausted = self.retain_worker(custody, &mut draining, &mut actions);
                    forced = forced.or(exhausted);
                }
                Err((member, id, kind, rejection)) => {
                    draining.push(member);
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::UnmatchedWorkerReturn {
                                id,
                                kind,
                                rejection,
                            },
                        )),
                    ));
                }
            }
        }
        match forced {
            Some(cause) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    PoolState::ForcedRetirement {
                        members: draining,
                        cause,
                    },
                    actions,
                )
            }
            None => self.continue_draining(draining, deadline, actions),
        }
    }

    fn accept_drain_creations(
        &mut self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let settlements = match workers.into_settlement() {
            CreationSettlement::Rejected {
                creations,
                reason: _,
            } => {
                return self.reject_drain_creations(
                    members,
                    deadline,
                    creations,
                    CreationBatchRejection::NamespaceExhausted,
                );
            }
            CreationSettlement::Corrupt { creations, fault } => {
                return self.reject_drain_creations(
                    members,
                    deadline,
                    creations,
                    CreationBatchRejection::InterpreterCorrupt(fault),
                );
            }
            CreationSettlement::Settled(settlements) => settlements,
        };
        let identities = settlements
            .iter()
            .map(creation_identity)
            .collect::<Option<Vec<_>>>();
        let positions = identities.and_then(|identities| {
            direct_worker::ordered_creation_positions(
                members.iter().map(RetiringWorker::expected_creation),
                identities,
            )
        });
        let Some(positions) = positions else {
            return self.diagnose(
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerCreationsSettled(CreationsSettled::new(
                    CreationSettlement::Settled(settlements),
                )),
            );
        };
        let mut returned: BTreeMap<_, _> = positions.into_iter().zip(settlements).collect();
        let mut draining = Vec::with_capacity(members.len());
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        let mut forced = None;
        for (position, member) in members.into_iter().enumerate() {
            let Some(settlement) = returned.remove(&position) else {
                draining.push(member);
                continue;
            };
            match member.accept_creation(ChildCreationSettled::new(settlement)) {
                Ok(custody) => {
                    let exhausted = self.retain_worker(custody, &mut draining, &mut actions);
                    forced = forced.or(exhausted);
                }
                Err((member, creation)) => {
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(FifoDiagnostic::new(
                            FifoDiagnosticCause::WorkersReturned(CreationsSettled::new(
                                CreationSettlement::Settled(Creations::one(
                                    creation.into_settlement(),
                                )),
                            )),
                        )),
                    ));
                    draining.push(member);
                }
            }
        }
        match forced {
            Some(cause) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    PoolState::ForcedRetirement {
                        members: draining,
                        cause,
                    },
                    actions,
                )
            }
            None => self.continue_draining(draining, deadline, actions),
        }
    }

    fn accept_drain_initialization(
        &self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        input: super::WorkerInitializationReport<W, P>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_initialization(members, input) {
            RetiringWorkerInitialization::Returned { workers, report } => self.diagnose(
                PoolState::Draining {
                    members: workers,
                    deadline,
                },
                FifoEvent::WorkerInitialization(report),
            ),
            RetiringWorkerInitialization::WorkerStopped {
                workers,
                departure,
                role,
                activation,
            } => {
                let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::UnusedActivation {
                            role,
                            permit: None,
                            activation,
                        },
                    )),
                ));
                if let Some(departure) = departure {
                    self.retain_drained_worker(departure, &mut actions);
                }
                self.continue_draining(workers, deadline, actions)
            }
        }
    }

    fn accept_drain_activation(
        &self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        input: super::WorkerActivation<W, P>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_activation(members, input) {
            RetiringWorkerActivation::Started { workers } => {
                self.continue_draining(workers, deadline, Actions::cont())
            }
            RetiringWorkerActivation::Returned {
                workers,
                role,
                worker,
                activation,
                outcome,
            } => {
                let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(FifoDiagnostic::new(
                        FifoDiagnosticCause::WorkerActivationReturned {
                            role,
                            worker,
                            activation,
                            outcome,
                        },
                    )),
                ));
                self.continue_draining(workers, deadline, actions)
            }
            RetiringWorkerActivation::Unrelated {
                workers,
                activation,
            } => self.diagnose(
                PoolState::Draining {
                    members: workers,
                    deadline,
                },
                FifoEvent::WorkerActivationReported(activation),
            ),
        }
    }

    fn accept_drain_preparation(
        &mut self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    )
    where
        Role: Eq,
    {
        let (members, accepted) =
            match direct_worker::accept_retiring_worker_preparation(members, input) {
                Ok(accepted) => accepted,
                Err((workers, input)) => {
                    return self.diagnose(
                        PoolState::Draining {
                            members: workers,
                            deadline,
                        },
                        FifoEvent::WorkerPreparationSettled(input),
                    );
                }
            };
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        let diagnostic = match accepted {
            WorkerRecoveryPreparation::Ready(direct_worker::PreparedReplacement {
                role,
                recoveries: _,
                previous,
                stopped,
                source,
                submission,
            }) => {
                let (returned_source, error) = match self.recovery.restore_source(source) {
                    Ok(()) => (None, WorkerReplacementError::PoolShutdown),
                    Err(source) => (Some(source), WorkerReplacementError::SourceStateCorrupt),
                };
                FifoDiagnosticCause::WorkerReplacementFailed {
                    role,
                    previous,
                    stopped,
                    submission,
                    returned_source,
                    error,
                }
            }
            WorkerRecoveryPreparation::Failed {
                role,
                recoveries: _,
                previous,
                stopped,
                source,
                error,
            } => {
                let (returned_source, error) = match self.recovery.restore_source(source) {
                    Ok(()) => (None, error),
                    Err(source) => (Some(source), WorkerPreparationError::SourceStateCorrupt),
                };
                FifoDiagnosticCause::WorkerPreparationFailed {
                    role,
                    previous,
                    stopped,
                    returned_source,
                    error,
                }
            }
        };
        actions.sends.diagnostics.append(InterpreterRequests::one(
            self.diagnostics.action(FifoDiagnostic::new(diagnostic)),
        ));
        self.continue_draining(members, deadline, actions)
    }

    fn continue_draining(
        &self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        match deadline {
            ShutdownDeadline::NotScheduled(settlement) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    PoolState::ForcedRetirement {
                        members,
                        cause: ForcedRetirementCause::DeadlineNotScheduled(settlement),
                    },
                    actions,
                )
            }
            ShutdownDeadline::Elapsed(elapsed) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    PoolState::ForcedRetirement {
                        members,
                        cause: ForcedRetirementCause::DeadlineElapsed(elapsed),
                    },
                    actions,
                )
            }
            deadline @ (ShutdownDeadline::Unlimited
            | ShutdownDeadline::Scheduling(_)
            | ShutdownDeadline::Waiting(_)) => {
                match members.iter().find_map(RetiringWorker::waiting) {
                    Some(_) => (PoolState::Draining { members, deadline }, actions),
                    None => {
                        actions.become_ = behavior::Step::Stop(behavior::Stopped);
                        (PoolState::Stopped, actions)
                    }
                }
            }
        }
    }

    fn accept_drain_stop(
        &self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_exit(members, stopped) {
            Ok(ControlFlow::Continue(members)) => {
                self.continue_draining(members, deadline, Actions::cont())
            }
            Ok(ControlFlow::Break((members, worker))) => {
                let mut actions = Actions::cont();
                self.retain_drained_worker(worker, &mut actions);
                self.continue_draining(members, deadline, actions)
            }
            Err((members, stopped)) => self.diagnose(
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerStopped(stopped),
            ),
        }
    }

    fn accept_drain_shutdown(
        &self,
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        settled: EstablishedShutdownResolved<W::Protocol>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_shutdown(members, settled) {
            Ok(ControlFlow::Continue(members)) => {
                self.continue_draining(members, deadline, Actions::cont())
            }
            Ok(ControlFlow::Break((members, worker))) => {
                let mut actions = Actions::cont();
                self.retain_drained_worker(worker, &mut actions);
                self.continue_draining(members, deadline, actions)
            }
            Err((members, settled)) => self.diagnose(
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerShutdownSettled(settled),
            ),
        }
    }

    fn accept_drain_schedule(
        &self,
        mut members: Vec<RetiringWorker<Role, W, P>>,
        mut deadline: ShutdownDeadline,
        settled: behavior::ActionItemResult<ScheduleAfter>,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let replacement = members.iter().position(|member| match &member.state {
            direct_worker::RetirementStatus::AwaitingRestartSchedule { timer, .. } => {
                timer.accepts_result(&settled)
            }
            direct_worker::RetirementStatus::AwaitingCreation(_)
            | direct_worker::RetirementStatus::Established { .. }
            | direct_worker::RetirementStatus::AwaitingPreparation { .. }
            | direct_worker::RetirementStatus::Drained => false,
        });
        if let Some(position) = replacement {
            let RetiringWorker { role, state } = members.remove(position);
            let direct_worker::RetirementStatus::AwaitingRestartSchedule {
                replacement,
                timer: _,
            } = state
            else {
                members.insert(position, RetiringWorker { role, state });
                return self.diagnose(
                    PoolState::Draining { members, deadline },
                    FifoEvent::RestartScheduleSettled(settled),
                );
            };
            members.insert(
                position,
                RetiringWorker {
                    role: role.clone(),
                    state: direct_worker::RetirementStatus::Drained,
                },
            );
            let mut actions = Actions::cont();
            self.retain_cancelled_replacement(
                role,
                replacement,
                WorkerReplacementError::RestartScheduleReturned(settled),
                &mut actions,
            );
            return self.continue_draining(members, deadline, actions);
        }
        match deadline.accept_schedule(settled) {
            Ok(()) => self.continue_draining(members, deadline, Actions::cont()),
            Err(settled) => self.diagnose(
                PoolState::Draining { members, deadline },
                FifoEvent::RestartScheduleSettled(settled),
            ),
        }
    }

    fn accept_drain_deadline(
        &self,
        members: Vec<RetiringWorker<Role, W, P>>,
        mut deadline: ShutdownDeadline,
        elapsed: crate::TimerElapsed,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        match deadline.accept_elapsed(elapsed) {
            Ok(()) => self.continue_draining(members, deadline, Actions::cont()),
            Err(elapsed) => self.diagnose(
                PoolState::Draining { members, deadline },
                FifoEvent::RestartElapsed(elapsed),
            ),
        }
    }

    fn diagnose(
        &self,
        state: PoolState<Role, W, P, Job, WorkerResult>,
        input: FifoEvent<
            Role,
            W,
            P,
            Job,
            WorkerResult,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        >,
    ) -> (
        PoolState<Role, W, P, Job, WorkerResult>,
        FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult>,
    ) {
        let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
            Actions::cont();
        actions.sends.diagnostics = InterpreterRequests::one(
            self.diagnostics
                .action(FifoDiagnostic::new(FifoDiagnosticCause::Unexpected(input))),
        );
        (state, actions)
    }
}

impl<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult> BehaviorBase
    for FifoPool<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    Job: Send,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

enum PoolState<Role, W, P, Job, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Constructed(Vec<PreparedWorker<Role, W, P>>),
    Operating(Operating<Role, W, P, Job, WorkerResult>),
    Draining {
        members: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
    },
    Stopped,
    ForcedRetirement {
        #[expect(
            dead_code,
            reason = "Bombay's retirement custodian receives every unresolved member"
        )]
        members: Vec<RetiringWorker<Role, W, P>>,
        #[expect(
            dead_code,
            reason = "Bombay's retirement custodian receives the exact forced-retirement cause"
        )]
        cause: ForcedRetirementCause,
    },
}

/// Controlled failure while starting or transitioning one FIFO pool.
#[doc(hidden)]
#[derive(Debug, Error)]
pub enum FifoError {
    /// Initialization was invoked after the prepared roster had already advanced.
    #[error("FIFO pool initialization is no longer available")]
    InitializationUnavailable,
    /// The pool could not reserve an identifier for every initial worker.
    #[error("FIFO pool worker creation identifiers are exhausted")]
    WorkerCreationsExhausted,
}

impl<Role, W, P, Job, WorkerResult> PoolState<Role, W, P, Job, WorkerResult>
where
    W: Behavior + BehaviorBase,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
{
    fn start(
        self,
        creations: &mut CreationSequence,
    ) -> Result<
        (
            Self,
            Vec<ObserveChild<W::Protocol, behavior::ChildHead>>,
            Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
        ),
        (Self, FifoError),
    > {
        let prepared = match self {
            Self::Constructed(prepared) => prepared,
            state => return Err((state, FifoError::InitializationUnavailable)),
        };
        let mut ids = Vec::with_capacity(prepared.len());
        for _ in 0..prepared.len() {
            let Some(id) = creations.issue() else {
                return Err((
                    Self::Constructed(prepared),
                    FifoError::WorkerCreationsExhausted,
                ));
            };
            ids.push(id);
        }

        let mut observations = Vec::with_capacity(prepared.len());
        let mut workers = Creations::empty();
        let members = prepared
            .into_iter()
            .zip(ids)
            .map(|(prepared, creation)| {
                let (member, worker, observation) = Member::begin(prepared, creation);
                workers.extend([worker]);
                observations.push(observation);
                member
            })
            .collect();

        Ok((
            Self::Operating(Operating {
                members,
                backlog: BTreeMap::new(),
                cursor: 0,
            }),
            observations,
            workers,
        ))
    }
}

/// A direct-worker FIFO pool with one bounded global backlog.
pub struct FifoPool<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    state: PoolState<Role, W, P, Job, WorkerResult>,
    activation: ActivationPolicy,
    recovery: PoolRecoveryState<Source>,
    restarts: super::restart::RestartBudget,
    backlog: BacklogCapacity,
    interruption: Interruption,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
    creations: behavior::CreationSequence,
    jobs: AcceptedJobSequence,
    assignments: AssignmentSequence,
    shutdowns: ShutdownSequence,
    next_restart_timer: u64,
}

/// Complete custody when initial worker preparation rejects.
pub struct FifoConstructionRejected<Factory, Role, W, P, Rejection, Source, DiagnosticRoute> {
    /// Complete initial-worker rejection.
    pub workers: InitialWorkerRejection<Factory, Role, W, P, Rejection>,
    /// Complete activation policy.
    pub activation: ActivationPolicy,
    /// Complete recovery policy.
    pub recovery: PoolRecovery<Source>,
    /// Complete waiting-job capacity.
    pub backlog: BacklogCapacity,
    /// Complete interruption policy.
    pub interruption: Interruption,
    /// Complete actor-graph drain policy.
    pub actor_drain: ActorDrainPolicy,
    /// Complete diagnostic disposition.
    pub diagnostics: DiagnosticDisposition<DiagnosticRoute>,
}

/// Construct one FIFO pool after preparing every declared worker in order.
pub fn fifo<Factory, Role, W, P, Rejection, Source, DiagnosticRoute, Job, WorkerResult>(
    factory: Factory,
    roles: OrderedRoles<Role>,
    activation: ActivationPolicy,
    recovery: PoolRecovery<Source>,
    backlog: BacklogCapacity,
    interruption: Interruption,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> Result<
    FifoPool<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult>,
    FifoConstructionRejected<Factory, Role, W, P, Rejection, Source, DiagnosticRoute>,
>
where
    Factory: FnMut(&Role) -> core::result::Result<WorkerSubmission<W, P>, Rejection>,
    Role: Eq,
    W: Behavior,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    BehaviorAddr<W>: EndpointAddress,
{
    let prepared = match prepare_initial_workers(factory, roles) {
        Ok(prepared) => prepared,
        Err(workers) => {
            return Err(FifoConstructionRejected {
                workers,
                activation,
                recovery,
                backlog,
                interruption,
                actor_drain,
                diagnostics,
            });
        }
    };
    Ok(FifoPool {
        state: PoolState::Constructed(prepared),
        activation,
        recovery: recovery.into(),
        restarts: super::restart::RestartBudget::empty(),
        backlog,
        interruption,
        actor_drain,
        diagnostics,
        creations: behavior::CreationSequence::new(),
        jobs: AcceptedJobSequence::new(),
        assignments: AssignmentSequence::new(),
        shutdowns: ShutdownSequence::new(),
        next_restart_timer: 1,
    })
}

impl<Role, W, P, Source, Diagnostics, Job, WorkerResult> Behavior
    for FifoPool<Role, W, P, Source, Diagnostics, Job, WorkerResult>
where
    Role: Eq + Send + Sync,
    W: Behavior + BehaviorBase + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    Diagnostics: DiagnosticRoute<FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>> + Clone,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Send,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    Job: Clone + Send,
    WorkerResult: Send,
{
    type Protocol =
        MessageProtocol<BehaviorAddr<W>, FifoCommand<BehaviorAddr<W>, Role, Job, WorkerResult>>;
    type Event = FifoEvent<
        Role,
        W,
        P,
        Job,
        WorkerResult,
        behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    >;
    type Sends = FifoRequests<
        InterpreterRequests<ObserveChild<W::Protocol, ChildHead>>,
        InterpreterRequests<InitializeWorker<W, P>>,
        InterpreterRequests<BeginActivation<W, P>>,
        <CustomerRoute<BehaviorAddr<W>, Role, Job, WorkerResult> as DeliveryRoute>::Sends,
        SourceActions<AssignWorker<W::Protocol, Job>>,
        SourceActions<PrepareWorkers<Source, Role, W, P>>,
        SourceActions<ScheduleAfter>,
        InterpreterRequests<ShutdownEstablished<StopOnShutdown<W>, Here>>,
        InterpreterRequests<
            DiagnosticAction<Diagnostics, FifoDiagnostic<Role, W, P, Source, Job, WorkerResult>>,
        >,
    >;
    type Ph = Never;
    type Error = FifoError;
    type Birth = Births<StopOnShutdown<W>>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        let state = mem::replace(&mut self.state, PoolState::Stopped);
        match state.start(&mut self.creations) {
            Ok((state, observations, workers)) => {
                self.state = state;
                let mut sends = FifoRequests::empty();
                sends.worker_observations = InterpreterRequests::new(observations);
                Ok(Actions::new(sends, workers, behavior::Step::Continue))
            }
            Err((state, error)) => {
                self.state = state;
                Err(error)
            }
        }
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        let state = mem::replace(&mut self.state, PoolState::Stopped);
        let (state, actions) = match (state, input) {
            (
                PoolState::Operating(operating),
                FifoEvent::Command(User {
                    message:
                        FifoCommand::Submit {
                            submission,
                            payload,
                            customer,
                        },
                    ..
                }),
            ) => {
                let (operating, actions) =
                    self.accept_submission(operating, submission, payload, customer);
                (PoolState::Operating(operating), actions)
            }
            (
                PoolState::Operating(operating),
                FifoEvent::Command(User {
                    message: FifoCommand::Shutdown,
                    ..
                })
                | FifoEvent::Shutdown(_),
            ) => self.begin_shutdown(operating, Actions::cont()),
            (PoolState::Operating(operating), FifoEvent::WorkerCreationsSettled(workers)) => {
                match self.accept_creations(operating, workers) {
                    Ok(result) => result,
                    Err((operating, workers)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerCreationsSettled(workers),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::WorkerInitialization(input)) => {
                match self.accept_initialization(operating, input) {
                    Ok(result) => result,
                    Err((operating, input)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerInitialization(input),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::WorkerActivationReported(input)) => {
                match self.accept_activation(operating, input) {
                    Ok(result) => result,
                    Err((operating, input)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerActivationReported(input),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::WorkerPreparationSettled(input)) => {
                match self.accept_worker_preparation(operating, input) {
                    Ok(result) => result,
                    Err((operating, input)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerPreparationSettled(input),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::RestartScheduleSettled(input)) => {
                match self.accept_restart_schedule(operating, input) {
                    Ok(result) => result,
                    Err((operating, input)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::RestartScheduleSettled(input),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::RestartElapsed(elapsed)) => {
                match self.accept_restart_timer(operating, elapsed) {
                    Ok(result) => result,
                    Err((operating, elapsed)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::RestartElapsed(elapsed),
                    ),
                }
            }
            (
                PoolState::Operating(operating),
                FifoEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Accepted(
                    receipt,
                ))),
            ) => match self.accept_assignment_receipt(operating, receipt) {
                Ok(result) => result,
                Err((operating, receipt)) => self.diagnose(
                    PoolState::Operating(operating),
                    FifoEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Accepted(
                        receipt,
                    ))),
                ),
            },
            (
                PoolState::Operating(operating),
                FifoEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason,
                })),
            ) => self.reject_assignment_delivery(operating, item, reason),
            (PoolState::Operating(operating), FifoEvent::WorkerCompleted(completion)) => {
                let ChildReport { child, report } = completion;
                match self.accept_completion(operating, child, report) {
                    Ok(result) => result,
                    Err((operating, completion)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerCompleted(completion),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::WorkerStopped(stopped)) => {
                match self.accept_worker_stop(operating, stopped) {
                    Ok(result) => result,
                    Err((operating, stopped)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerStopped(stopped),
                    ),
                }
            }
            (PoolState::Operating(operating), FifoEvent::WorkerShutdownSettled(shutdown)) => {
                match self.accept_worker_shutdown(operating, shutdown) {
                    Ok(result) => result,
                    Err((operating, shutdown)) => self.diagnose(
                        PoolState::Operating(operating),
                        FifoEvent::WorkerShutdownSettled(shutdown),
                    ),
                }
            }
            (
                PoolState::Draining { members, deadline },
                FifoEvent::Command(User {
                    message:
                        FifoCommand::Submit {
                            submission,
                            payload,
                            customer,
                        },
                    ..
                }),
            ) => {
                let mut actions: FifoActions<Role, W, P, Source, Diagnostics, Job, WorkerResult> =
                    Actions::cont();
                actions.sends.customer_outcomes = customer.deliver(FifoOutcome::rejected(
                    submission,
                    payload,
                    AdmissionRejection::ShuttingDown,
                ));
                (PoolState::Draining { members, deadline }, actions)
            }
            (
                state @ PoolState::Draining { .. },
                FifoEvent::Command(User {
                    message: FifoCommand::Shutdown,
                    ..
                })
                | FifoEvent::Shutdown(_),
            ) => (state, Actions::cont()),
            (
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerCreationsSettled(workers),
            ) => self.accept_drain_creations(members, deadline, workers),
            (PoolState::Draining { members, deadline }, FifoEvent::WorkerStopped(stopped)) => {
                self.accept_drain_stop(members, deadline, stopped)
            }
            (
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerInitialization(initialization),
            ) => self.accept_drain_initialization(members, deadline, initialization),
            (
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerActivationReported(activation),
            ) => self.accept_drain_activation(members, deadline, activation),
            (
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerShutdownSettled(settled),
            ) => self.accept_drain_shutdown(members, deadline, settled),
            (
                PoolState::Draining { members, deadline },
                FifoEvent::WorkerPreparationSettled(prepared),
            ) => self.accept_drain_preparation(members, deadline, prepared),
            (
                PoolState::Draining { members, deadline },
                FifoEvent::RestartScheduleSettled(settled),
            ) => self.accept_drain_schedule(members, deadline, settled),
            (PoolState::Draining { members, deadline }, FifoEvent::RestartElapsed(elapsed)) => {
                self.accept_drain_deadline(members, deadline, elapsed)
            }
            (state @ (PoolState::Stopped | PoolState::ForcedRetirement { .. }), input) => {
                let (state, mut actions) = self.diagnose(state, input);
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (state, actions)
            }
            (state, input) => self.diagnose(state, input),
        };
        self.state = state;
        Ok(actions)
    }
}
