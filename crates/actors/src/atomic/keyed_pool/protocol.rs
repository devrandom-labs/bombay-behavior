//! Binding management commands and replies.

use behavior::{Address, Behavior, BehaviorAddr, EndpointAddress, MessageProtocol, Protocol};

use crate::{ChildStopped, EstablishedShutdownResolved, ReplyRoute};

use super::super::pool::worker::{
    CurrentWorker, PendingWorkerReplacement, WorkerPreparationError, WorkerReplacementError,
};
use super::super::worker::{ActivationAttempt, WorkerActivationOutcome};
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
        activation: ActivationAttempt,
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
