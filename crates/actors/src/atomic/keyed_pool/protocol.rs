//! Binding management commands and replies.

use behavior::{Address, Behavior, BehaviorAddr, EndpointAddress, MessageProtocol, Protocol};

use crate::{ChildStopped, EstablishedShutdownResolved, ReplyRoute};

use super::super::pool::worker::{
    CurrentWorker, PendingWorkerReplacement, WorkerPreparationError, WorkerReplacementError,
};
use super::super::worker::{WorkerActivationGrant, WorkerActivationOutcome};
use super::super::{
    ActivationPermit, ActivationPlan, JobId, PrepareWorkers, RoleName, SubmissionId, WorkerSource,
};
use super::super::{WorkerAttempt, WorkerCreationRejection};
use super::{BindingEvidence, BindingExpectation, BindingRequestId};

pub(super) enum BindingChange<Role> {
    Rebalance(Role),
    Unbind,
}

/// One complete generation-exact binding management command.
///
/// Applications construct this value through [`super::KeyedCommand`] so binding
/// management has one canonical command spelling.
pub struct BindingCommand<A, Key, Role>
where
    A: Address + EndpointAddress,
{
    request: BindingRequestId,
    key: Key,
    expected: BindingExpectation,
    change: BindingChange<Role>,
    reply: ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>,
}

impl<A, Key, Role> BindingCommand<A, Key, Role>
where
    A: Address + EndpointAddress,
{
    pub(super) fn rebalance<Route>(
        request: BindingRequestId,
        key: Key,
        expected: BindingExpectation,
        target: Role,
        reply: Route,
    ) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>>,
    {
        Self {
            request,
            key,
            expected,
            change: BindingChange::Rebalance(target),
            reply: reply.into(),
        }
    }

    pub(super) fn unbind<Route>(
        request: BindingRequestId,
        key: Key,
        expected: BindingExpectation,
        reply: Route,
    ) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>>,
    {
        Self {
            request,
            key,
            expected,
            change: BindingChange::Unbind,
            reply: reply.into(),
        }
    }

    /// Borrow the caller-authored correlation.
    #[must_use]
    pub const fn request(&self) -> BindingRequestId {
        self.request
    }

    /// Borrow the caller-owned lookup key.
    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }

    /// Borrow the exact state required before mutation.
    #[must_use]
    pub const fn expectation(&self) -> &BindingExpectation {
        &self.expected
    }

    pub(super) fn reply(&self) -> &ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>> {
        &self.reply
    }

    pub(super) const fn change(&self) -> &BindingChange<Role> {
        &self.change
    }

    pub(super) fn from_parts(
        request: BindingRequestId,
        key: Key,
        expected: BindingExpectation,
        change: BindingChange<Role>,
        reply: ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>,
    ) -> Self {
        Self {
            request,
            key,
            expected,
            change,
            reply,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        BindingRequestId,
        Key,
        BindingExpectation,
        BindingChange<Role>,
        ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>,
    ) {
        (
            self.request,
            self.key,
            self.expected,
            self.change,
            self.reply,
        )
    }
}

/// Why one binding management command could not change retained affinity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingRejection {
    /// The expected absence or generation does not match current state.
    StaleExpectation {
        /// Current absent or exact generation state.
        actual: BindingExpectation,
    },
    /// The requested semantic role is not declared by this pool.
    UnknownTarget,
    /// The declared role cannot accept future affinity.
    TargetUnavailable,
    /// No additional binding can be retained.
    BindingCapacityExhausted,
    /// No fresh binding generation remains.
    GenerationExhausted,
    /// Global shutdown has closed binding management.
    ShuttingDown,
}

/// Complete result of one binding management command.
pub enum BindingReply<A, Key, Role>
where
    A: Address + EndpointAddress,
{
    /// An absent key was bound to the requested role.
    Bound {
        /// Caller-authored correlation.
        request: BindingRequestId,
        /// Newly retained role and generation.
        current: BindingEvidence<Role>,
    },
    /// An existing key moved to another role with a fresh generation.
    Rebalanced {
        /// Caller-authored correlation.
        request: BindingRequestId,
        /// Previously retained role and generation.
        prior: BindingEvidence<Role>,
        /// Newly retained role and generation.
        current: BindingEvidence<Role>,
    },
    /// A same-role rebalance preserved the exact generation.
    Unchanged {
        /// Caller-authored correlation.
        request: BindingRequestId,
        /// Still-current role and generation.
        current: BindingEvidence<Role>,
    },
    /// The exact retained binding was removed.
    Unbound {
        /// Caller-authored correlation.
        request: BindingRequestId,
        /// Key formerly owned by the binding table.
        key: Key,
        /// Removed role and generation.
        removed: BindingEvidence<Role>,
    },
    /// The command expected absence and the key was already absent.
    AlreadyUnbound {
        /// Complete unchanged command for retry or inspection.
        command: BindingCommand<A, Key, Role>,
    },
    /// The command was rejected without changing the table.
    Rejected {
        /// Complete unchanged command for retry or inspection.
        command: BindingCommand<A, Key, Role>,
        /// Exact rejection reason.
        reason: BindingRejection,
    },
}

/// Exact reason a keyed submission never entered pool ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyedAdmissionRejection {
    /// Global shutdown has closed admission.
    ShuttingDown,
    /// The bounded binding table cannot retain another key.
    BindingCapacityExhausted,
    /// The selector returned a role absent from the declared roster.
    UnknownSelectedRole,
    /// The selected or retained role cannot serve or queue work.
    RoleUnavailable,
    /// The selected role's own waiting capacity is full.
    RoleBacklogFull,
    /// No direct worker can serve now or recover later.
    NoRecoverableWorkers,
    /// No fresh binding generation remains.
    BindingGenerationExhausted,
    /// No fresh job correlation remains.
    JobCorrelationExhausted,
    /// Immediate assignment cannot issue fresh completion authority.
    AssignmentCorrelationExhausted,
}

/// Exact reason accepted queued work returned without assignment completion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyedQueuedReturnReason {
    /// Global pool shutdown extracted the waiting work.
    PoolShutdown,
    /// Its permanently unavailable role can no longer serve it.
    RolePermanentlyUnavailable,
    /// No recoverable direct worker remains in the pool.
    NoRecoverableWorkers,
    /// A ready worker could not obtain fresh assignment authority.
    AssignmentCorrelationExhausted,
}

/// Exact reason accepted assigned work returned without a completed result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyedAssignedReturnReason {
    /// Global pool shutdown extracted the assignment.
    PoolShutdown,
    /// Its admitted role became permanently unavailable.
    RolePermanentlyUnavailable,
    /// The exact direct worker stopped and policy selected failure.
    WorkerStopped,
    /// Worker delivery returned the complete unaccepted assignment.
    AssignmentReturned,
    /// Retry could not reserve fresh assignment authority.
    RetryPreparationRejected,
    /// Delivery settlement contradicted an already received completion.
    ContradictoryAssignmentSettlement,
}

/// Complete customer-visible progress or terminal result for keyed work.
pub enum KeyedOutcome<Key, Role, Job, WorkerResult> {
    /// One submission and its binding were accepted together.
    Accepted {
        /// Customer-authored admission correlation.
        submission: SubmissionId,
        /// Pool-issued accepted-job correlation.
        job: JobId,
        /// Immutable admitted role and generation.
        binding: BindingEvidence<Role>,
    },
    /// Admission rejected without retaining the key or payload.
    Rejected {
        /// Customer-authored admission correlation.
        submission: SubmissionId,
        /// Complete submitted key.
        key: Key,
        /// Complete submitted payload.
        payload: Job,
        /// Exact rejection reason.
        reason: KeyedAdmissionRejection,
    },
    /// The exact accepted assignment completed.
    Completed {
        /// Pool-issued accepted-job correlation.
        job: JobId,
        /// Immutable admitted role and generation.
        binding: BindingEvidence<Role>,
        /// Worker-produced domain result.
        worker_result: WorkerResult,
    },
    /// Accepted queued work returned before assignment.
    ReturnedQueued {
        /// Pool-issued accepted-job correlation.
        job: JobId,
        /// Immutable admitted role and generation.
        binding: BindingEvidence<Role>,
        /// Complete retained payload.
        payload: Job,
        /// Exact terminal reason.
        reason: KeyedQueuedReturnReason,
    },
    /// Accepted assigned work returned without completion.
    ReturnedAssigned {
        /// Pool-issued accepted-job correlation.
        job: JobId,
        /// Immutable admitted role and generation.
        binding: BindingEvidence<Role>,
        /// Complete retained payload.
        payload: Job,
        /// Exact terminal reason.
        reason: KeyedAssignedReturnReason,
    },
}

pub(super) enum KeyedRequest<A, Key, Role, Job, WorkerResult>
where
    A: Address + EndpointAddress,
{
    Submit {
        submission: SubmissionId,
        key: Key,
        payload: Job,
        customer: ReplyRoute<MessageProtocol<A, KeyedOutcome<Key, Role, Job, WorkerResult>>>,
    },
    Binding(BindingCommand<A, Key, Role>),
    Shutdown,
}

/// One application command accepted by a keyed pool.
pub struct KeyedCommand<A, Key, Role, Job, WorkerResult>
where
    A: Address + EndpointAddress,
{
    request: KeyedRequest<A, Key, Role, Job, WorkerResult>,
}

impl<A, Key, Role, Job, WorkerResult> KeyedCommand<A, Key, Role, Job, WorkerResult>
where
    A: Address + EndpointAddress,
{
    /// Submit one affinity key, payload, correlation, and customer capability.
    #[must_use]
    pub fn submit<Route>(submission: SubmissionId, key: Key, payload: Job, customer: Route) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, KeyedOutcome<Key, Role, Job, WorkerResult>>>>,
    {
        Self {
            request: KeyedRequest::Submit {
                submission,
                key,
                payload,
                customer: customer.into(),
            },
        }
    }

    /// Request generation-exact affinity for an absent or retained key.
    #[must_use]
    pub fn rebalance<Route>(
        request: BindingRequestId,
        key: Key,
        expected: BindingExpectation,
        target: Role,
        reply: Route,
    ) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>>,
    {
        Self {
            request: KeyedRequest::Binding(BindingCommand::rebalance(
                request, key, expected, target, reply,
            )),
        }
    }

    /// Remove one exact retained binding or report that absence was current.
    #[must_use]
    pub fn unbind<Route>(
        request: BindingRequestId,
        key: Key,
        expected: BindingExpectation,
        reply: Route,
    ) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>>,
    {
        Self {
            request: KeyedRequest::Binding(BindingCommand::unbind(request, key, expected, reply)),
        }
    }

    /// Retry a complete binding command returned by an earlier reply.
    #[must_use]
    pub const fn binding(command: BindingCommand<A, Key, Role>) -> Self {
        Self {
            request: KeyedRequest::Binding(command),
        }
    }

    /// Close admission and retire the complete owned actor graph.
    #[must_use]
    pub const fn shutdown() -> Self {
        Self {
            request: KeyedRequest::Shutdown,
        }
    }

    pub(super) fn into_request(self) -> KeyedRequest<A, Key, Role, Job, WorkerResult> {
        self.request
    }

    pub(super) const fn from_request(
        request: KeyedRequest<A, Key, Role, Job, WorkerResult>,
    ) -> Self {
        Self { request }
    }
}

#[expect(dead_code, reason = "transferred intact to diagnostic custody")]
pub(super) enum KeyedDiagnosticCause<Role, W, P, Source, Key, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: Protocol<Msg = super::Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    BindingRemoved {
        key: Key,
        binding: BindingEvidence<Role>,
    },
    WorkerReturned {
        role: RoleName<Role>,
        rejection: WorkerCreationRejection<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    StoppedWorkerEstablished {
        role: RoleName<Role>,
        worker: CurrentWorker<W>,
        activation: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    UnmatchedWorkerReturn {
        id: behavior::CreationId,
        kind: behavior::CreationKind,
        rejection: WorkerCreationRejection<W>,
    },
    UnusedActivation {
        role: RoleName<Role>,
        permit: Option<ActivationPermit<W>>,
        activation: P,
    },
    WorkerActivationReturned {
        role: RoleName<Role>,
        worker: WorkerAttempt,
        activation: WorkerActivationGrant<W, P>,
        outcome: WorkerActivationOutcome<W, P>,
    },
    WorkerShutdownRejected {
        role: RoleName<Role>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    },
    WorkerReplacementCancelled {
        role: RoleName<Role>,
        replacement: PendingWorkerReplacement<W, P>,
    },
    WorkerPreparationFailed {
        role: RoleName<Role>,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        returned_source: Option<Source>,
        error: WorkerPreparationError<Source::WorkerRejection, Source::SourceRejection>,
    },
    WorkerReplacementFailed {
        role: RoleName<Role>,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        submission: super::super::WorkerSubmission<W, P>,
        returned_source: Option<Source>,
        error: WorkerReplacementError,
    },
    Unexpected(
        super::KeyedEvent<
            Role,
            W,
            P,
            Key,
            Job,
            WorkerResult,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
            super::super::WorkerPreparation<Source, Role, W, P>,
        >,
    ),
}

/// Complete keyed-pool input or precise aggregate failure selected for diagnostics.
pub struct KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: Protocol<Msg = super::Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    #[expect(
        dead_code,
        reason = "Bombay's diagnostic custodian receives the complete opaque cause"
    )]
    cause: KeyedDiagnosticCause<Role, W, P, Source, Key, Job, WorkerResult>,
}

impl<Role, W, P, Source, Key, Job, WorkerResult>
    From<KeyedDiagnosticCause<Role, W, P, Source, Key, Job, WorkerResult>>
    for KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: Protocol<Msg = super::Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    fn from(cause: KeyedDiagnosticCause<Role, W, P, Source, Key, Job, WorkerResult>) -> Self {
        Self { cause }
    }
}

impl<Role, W, P, Source, Key, Job, WorkerResult> core::fmt::Debug
    for KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: Protocol<Msg = super::Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KeyedDiagnostic")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod live_activation_correlation {
    use super::super::binding::{BindingCapacity, BindingTable};
    use super::super::requests::KeyedRequests;
    use super::super::role::RoleCell;
    use super::super::{KeyedEvent, KeyedOperating, KeyedPool, KeyedPoolState};
    use super::{KeyedDiagnostic, KeyedDiagnosticCause};
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
    use std::sync::Arc;
    use std::time::Instant;
    fn role_for_key(_: &u64) -> u64 {
        23
    }

    #[tokio::test]
    async fn keyed_dispatched_live_returns_the_original_foreign_activation_twice() {
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
            let binding_capacity = BindingCapacity::new(2).expect("two real binding slots");
            let pool: KeyedPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                fn(&u64) -> u64,
                Infallible,
                u64,
                u8,
                u16,
            > = KeyedPool {
                state: KeyedPoolState::Operating(KeyedOperating {
                    roles: vec![RoleCell::new(member, BacklogCapacity::new(2))],
                    bindings: BindingTable::new(binding_capacity),
                }),
                selector: role_for_key,
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                binding_capacity,
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
                    .transition(KeyedEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let KeyedRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    binding_replies,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.is_empty());
                assert!(binding_replies.as_slice().is_empty());
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
                        KeyedDiagnostic {
                            cause:
                                KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerActivationReported(
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
                let KeyedPoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.roles.len(), 1);
                assert!(operating.roles[0].queue.is_empty());
                assert!(operating.bindings.binding(&7_u64).is_none());
                let member = &operating.roles[0].member;
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
    async fn keyed_dispatched_stopped_returns_the_original_foreign_activation_twice() {
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
            let binding_capacity = BindingCapacity::new(2).expect("two real binding slots");
            let pool: KeyedPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                fn(&u64) -> u64,
                Infallible,
                u64,
                u8,
                u16,
            > = KeyedPool {
                state: KeyedPoolState::Operating(KeyedOperating {
                    roles: vec![RoleCell::new(member, BacklogCapacity::new(2))],
                    bindings: BindingTable::new(binding_capacity),
                }),
                selector: role_for_key,
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                binding_capacity,
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
                    .transition(KeyedEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let KeyedRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    binding_replies,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.is_empty());
                assert!(binding_replies.as_slice().is_empty());
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
                        KeyedDiagnostic {
                            cause:
                                KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerActivationReported(
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
                let KeyedPoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.roles.len(), 1);
                assert!(operating.roles[0].queue.is_empty());
                assert!(operating.bindings.binding(&7_u64).is_none());
                let member = &operating.roles[0].member;
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
    async fn keyed_activating_live_returns_the_original_foreign_activation_twice() {
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
            let binding_capacity = BindingCapacity::new(2).expect("two real binding slots");
            let pool: KeyedPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                fn(&u64) -> u64,
                Infallible,
                u64,
                u8,
                u16,
            > = KeyedPool {
                state: KeyedPoolState::Operating(KeyedOperating {
                    roles: vec![RoleCell::new(member, BacklogCapacity::new(2))],
                    bindings: BindingTable::new(binding_capacity),
                }),
                selector: role_for_key,
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                binding_capacity,
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
                    .transition(KeyedEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let KeyedRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    binding_replies,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.is_empty());
                assert!(binding_replies.as_slice().is_empty());
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
                        KeyedDiagnostic {
                            cause:
                                KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerActivationReported(
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
                let KeyedPoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.roles.len(), 1);
                assert!(operating.roles[0].queue.is_empty());
                assert!(operating.bindings.binding(&7_u64).is_none());
                let member = &operating.roles[0].member;
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
    async fn keyed_activating_stopped_returns_the_original_foreign_activation_twice() {
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
            let binding_capacity = BindingCapacity::new(2).expect("two real binding slots");
            let pool: KeyedPool<
                u64,
                ActivationWorker,
                ImmediateActivation,
                Never,
                fn(&u64) -> u64,
                Infallible,
                u64,
                u8,
                u16,
            > = KeyedPool {
                state: KeyedPoolState::Operating(KeyedOperating {
                    roles: vec![RoleCell::new(member, BacklogCapacity::new(2))],
                    bindings: BindingTable::new(binding_capacity),
                }),
                selector: role_for_key,
                activation: ActivationPolicy::new(1).expect("one actual activation slot"),
                recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
                restarts: RestartBudget::empty(),
                backlog: BacklogCapacity::new(2),
                binding_capacity,
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
                    .transition(KeyedEvent::WorkerActivationReported(returned))
                    .unwrap_or_else(|error| {
                        panic!("the activation fold returns its real Actions: {error}")
                    });
                let Actions {
                    sends,
                    creates,
                    become_,
                } = actions;
                let KeyedRequests {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    customer_outcomes,
                    binding_replies,
                    worker_assignments,
                    worker_preparations,
                    restart_schedules,
                    worker_shutdowns,
                    diagnostics,
                } = sends;
                assert!(worker_observations.is_empty());
                assert!(worker_initializations.is_empty());
                assert!(worker_activations.is_empty());
                assert!(customer_outcomes.is_empty());
                assert!(binding_replies.as_slice().is_empty());
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
                        KeyedDiagnostic {
                            cause:
                                KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerActivationReported(
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
                let KeyedPoolState::Operating(operating) = &pool.state else {
                    panic!("foreign activation must not retire the original pool")
                };
                assert_eq!(operating.roles.len(), 1);
                assert!(operating.roles[0].queue.is_empty());
                assert!(operating.bindings.binding(&7_u64).is_none());
                let member = &operating.roles[0].member;
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
    async fn keyed_original_activation_started_then_ready_preserves_all_lanes() {
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
        let binding_capacity = BindingCapacity::new(2).expect("two real binding slots");
        let pool: KeyedPool<
            u64,
            ActivationWorker,
            ImmediateActivation,
            Never,
            fn(&u64) -> u64,
            Infallible,
            u64,
            u8,
            u16,
        > = KeyedPool {
            state: KeyedPoolState::Operating(KeyedOperating {
                roles: vec![RoleCell::new(member, BacklogCapacity::new(2))],
                bindings: BindingTable::new(binding_capacity),
            }),
            selector: role_for_key,
            activation: ActivationPolicy::new(1).expect("one actual activation slot"),
            recovery: PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole).into(),
            restarts: RestartBudget::empty(),
            backlog: BacklogCapacity::new(2),
            binding_capacity,
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
                .transition(KeyedEvent::WorkerActivationReported(input))
                .unwrap_or_else(|error| {
                    panic!("the original activation returns complete Actions: {error}")
                });
            let Actions {
                sends,
                creates,
                become_,
            } = actions;
            let KeyedRequests {
                worker_observations,
                worker_initializations,
                worker_activations,
                customer_outcomes,
                binding_replies,
                worker_assignments,
                worker_preparations,
                restart_schedules,
                worker_shutdowns,
                diagnostics,
            } = sends;
            assert!(worker_observations.is_empty());
            assert!(worker_initializations.is_empty());
            assert!(worker_activations.is_empty());
            assert!(customer_outcomes.is_empty());
            assert!(binding_replies.as_slice().is_empty());
            assert!(worker_assignments.is_empty());
            assert!(worker_preparations.is_empty());
            assert!(restart_schedules.is_empty());
            assert!(worker_shutdowns.is_empty());
            assert!(diagnostics.is_empty());
            assert!(creates.is_empty());
            assert!(matches!(become_, Step::Continue));
            let KeyedPoolState::Operating(operating) = &pool.state else {
                panic!("original activation leaves the pool operating")
            };
            assert_eq!(operating.roles.len(), 1);
            assert!(operating.roles[0].queue.is_empty());
            assert!(operating.bindings.binding(&7_u64).is_none());
            let member = &operating.roles[0].member;
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
