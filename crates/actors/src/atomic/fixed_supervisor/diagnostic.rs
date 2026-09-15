//! Owned operational diagnostics emitted by one fixed supervisor.

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, EndpointAddress, InterpreterFault, Protocol,
};

use crate::{
    ActivationPlan, ChildStopped, ProxyOperation, ProxyOutcome, ProxyPhase, ScheduleAfter,
    ScheduleAfterRejection, StableProxy, WorkerSubmission,
};

use behavior::ChildInputReason;

use super::super::{PreparedWorker, RoleName, WorkerSource};
use super::restart::RecoveryDenialReason;
use super::{FixedSupervisorEvent, PrepareWorkers};

/// One operational diagnostic emitted by a fixed supervisor.
pub enum FixedDiagnostic<Role, Worker, Plan, Source>
where
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// The exact initial StableProxy operation could not make its worker ready.
    ProxyOutcomeFailed(ProxyOutcomeFailure<Role, Worker, Plan>),
    /// An exact replacement input was rejected and returned complete.
    ProxyInputRejected(ProxyInputFailure<Role, Worker, Plan>),
    /// An operating recovery could not prepare every selected worker.
    WorkerPreparationFailed(
        WorkerPreparationFailure<
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
    ),
    /// An otherwise prepared recovery was denied by restart policy.
    RecoveryDenied(RecoveryDenied<Role, Worker>),
    /// A delayed recovery's exact timer request was rejected.
    RestartScheduleFailed(RestartScheduleFailure<Role>),
    /// A service command reached an exact proxy without a ready worker.
    WorkerUnavailable(WorkerUnavailable<Role, Worker>),
    /// One complete typed input did not belong to the current supervisor state.
    UnexpectedInput {
        /// Complete unexpected input, unchanged.
        input: FixedSupervisorEvent<
            Role,
            Worker,
            Plan,
            ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        >,
    },
}

/// Complete service command returned by one exact unavailable StableProxy.
pub struct WorkerUnavailable<Role, Worker>
where
    Worker: Behavior,
{
    role: RoleName<Role>,
    sender: BehaviorAddr<Worker>,
    phase: ProxyPhase,
    command: <Worker::Protocol as Protocol>::Msg,
}

impl<Role, Worker> WorkerUnavailable<Role, Worker>
where
    Worker: Behavior,
{
    pub(super) const fn new(
        role: RoleName<Role>,
        sender: BehaviorAddr<Worker>,
        phase: ProxyPhase,
        command: <Worker::Protocol as Protocol>::Msg,
    ) -> Self {
        Self {
            role,
            sender,
            phase,
            command,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        RoleName<Role>,
        BehaviorAddr<Worker>,
        ProxyPhase,
        <Worker::Protocol as Protocol>::Msg,
    ) {
        (self.role, self.sender, self.phase, self.command)
    }

    /// Inspect the semantic role whose worker was unavailable.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.role.role()
    }

    /// Inspect the original command sender.
    #[must_use]
    pub const fn sender(&self) -> &BehaviorAddr<Worker> {
        &self.sender
    }

    /// Inspect the exact StableProxy phase that rejected the command.
    #[must_use]
    pub const fn phase(&self) -> ProxyPhase {
        self.phase
    }

    /// Inspect the complete returned service command.
    #[must_use]
    pub const fn command(&self) -> &<Worker::Protocol as Protocol>::Msg {
        &self.command
    }
}

/// Complete replacement input returned by a rejecting StableProxy capability.
pub struct ProxyInputFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    role: RoleName<Role>,
    operation: ProxyOperation<behavior::Here, Worker, Plan>,
    reason: ChildInputReason,
}

impl<Role, Worker, Plan> ProxyInputFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) const fn new(
        role: RoleName<Role>,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
        reason: ChildInputReason,
    ) -> Self {
        Self {
            role,
            operation,
            reason,
        }
    }

    /// Inspect the semantic role whose replacement input was rejected.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.role.role()
    }

    /// Inspect the complete returned replacement operation.
    #[must_use]
    pub const fn operation(&self) -> &ProxyOperation<behavior::Here, Worker, Plan> {
        &self.operation
    }

    /// Inspect the exact capability rejection.
    #[must_use]
    pub const fn reason(&self) -> ChildInputReason {
        self.reason
    }
}

/// Exact rejected timer request for one delayed recovery.
pub struct RestartScheduleFailure<Role> {
    trigger: RoleName<Role>,
    request: ScheduleAfter,
    reason: ScheduleAfterRejection,
}

impl<Role> RestartScheduleFailure<Role> {
    pub(super) const fn new(
        trigger: RoleName<Role>,
        request: ScheduleAfter,
        reason: ScheduleAfterRejection,
    ) -> Self {
        Self {
            trigger,
            request,
            reason,
        }
    }

    /// Inspect the recovery-triggering semantic role.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.trigger.role()
    }

    /// Recover the exact rejected timer request.
    #[must_use]
    pub const fn request(&self) -> ScheduleAfter {
        self.request
    }

    /// Inspect the exact timer rejection.
    #[must_use]
    pub const fn reason(&self) -> ScheduleAfterRejection {
        self.reason
    }
}

/// Complete restart-policy denial emitted by a fixed supervisor.
///
/// The triggering role and complete worker stop are borrowed through
/// [`RecoveryDenied::role`] and [`RecoveryDenied::stopped`]. Restart history,
/// the worker source, and roster ownership remain private to the supervisor.
pub struct RecoveryDenied<Role, Worker>
where
    Worker: Behavior,
{
    trigger: RoleName<Role>,
    stopped: ChildStopped<BehaviorAddr<Worker>>,
    reason: RecoveryDenialReason,
}

impl<Role, Worker> RecoveryDenied<Role, Worker>
where
    Worker: Behavior,
{
    pub(super) const fn new(
        trigger: RoleName<Role>,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
        reason: RecoveryDenialReason,
    ) -> Self {
        Self {
            trigger,
            stopped,
            reason,
        }
    }

    /// Inspect the recovery-triggering semantic role.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.trigger.role()
    }

    /// Inspect the complete worker stop that triggered the denied recovery.
    #[must_use]
    pub const fn stopped(&self) -> &ChildStopped<BehaviorAddr<Worker>> {
        &self.stopped
    }

    /// Inspect the exact denial without exposing private restart history.
    #[must_use]
    pub const fn reason(&self) -> &RecoveryDenialReason {
        &self.reason
    }
}

enum PreparationFailureState<Role, Worker, Plan, WorkerRejection, SourceRejection> {
    WorkerRejected {
        prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
        failed_role: RoleName<Role>,
        reason: WorkerRejection,
        remaining: Vec<RoleName<Role>>,
    },
    SourceRejected {
        reason: SourceRejection,
        remaining: Vec<RoleName<Role>>,
    },
    InterpreterFault {
        fault: InterpreterFault,
        remaining: Vec<RoleName<Role>>,
    },
    Unattempted(Vec<RoleName<Role>>),
}

/// Complete operating worker-preparation failure emitted by a fixed supervisor.
///
/// The worker source is not stored here. It returns separately to the
/// supervisor's recovery policy. Semantic roles are exposed by reference so
/// they need not implement `Clone`.
pub struct WorkerPreparationFailure<Role, Worker, Plan, WorkerRejection, SourceRejection> {
    trigger: RoleName<Role>,
    state: PreparationFailureState<Role, Worker, Plan, WorkerRejection, SourceRejection>,
}

/// Exact borrowed reason for one operating worker-preparation failure.
pub enum WorkerPreparationFailureReason<'a, Role, WorkerRejection, SourceRejection> {
    /// The source accepted the request but rejected one selected worker.
    WorkerRejected {
        /// Exact semantic role whose worker was rejected.
        role: &'a Role,
        /// Source-selected worker rejection.
        reason: &'a WorkerRejection,
    },
    /// The source rejected the complete request before preparing a worker.
    SourceRejected(&'a SourceRejection),
    /// The interpreter returned the request with controlled corruption evidence.
    InterpreterFault(InterpreterFault),
    /// Product interpretation stopped before attempting the request.
    Unattempted,
}

impl<Role, Worker, Plan, WorkerRejection, SourceRejection>
    WorkerPreparationFailure<Role, Worker, Plan, WorkerRejection, SourceRejection>
{
    pub(super) fn worker_rejected(
        trigger: RoleName<Role>,
        prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
        failed_role: RoleName<Role>,
        reason: WorkerRejection,
        remaining: Vec<RoleName<Role>>,
    ) -> Self {
        Self {
            trigger,
            state: PreparationFailureState::WorkerRejected {
                prepared,
                failed_role,
                reason,
                remaining,
            },
        }
    }

    pub(super) fn source_rejected(
        trigger: RoleName<Role>,
        reason: SourceRejection,
        remaining: Vec<RoleName<Role>>,
    ) -> Self {
        Self {
            trigger,
            state: PreparationFailureState::SourceRejected { reason, remaining },
        }
    }

    pub(super) fn interpreter_fault(
        trigger: RoleName<Role>,
        fault: InterpreterFault,
        remaining: Vec<RoleName<Role>>,
    ) -> Self {
        Self {
            trigger,
            state: PreparationFailureState::InterpreterFault { fault, remaining },
        }
    }

    pub(super) fn unattempted(trigger: RoleName<Role>, remaining: Vec<RoleName<Role>>) -> Self {
        Self {
            trigger,
            state: PreparationFailureState::Unattempted(remaining),
        }
    }

    /// Inspect the recovery-triggering semantic role.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.trigger.role()
    }

    /// Inspect the exact failure alternative without exposing stored role names.
    #[must_use]
    pub fn reason(
        &self,
    ) -> WorkerPreparationFailureReason<'_, Role, WorkerRejection, SourceRejection> {
        match &self.state {
            PreparationFailureState::WorkerRejected {
                failed_role,
                reason,
                ..
            } => WorkerPreparationFailureReason::WorkerRejected {
                role: failed_role.role(),
                reason,
            },
            PreparationFailureState::SourceRejected { reason, .. } => {
                WorkerPreparationFailureReason::SourceRejected(reason)
            }
            PreparationFailureState::InterpreterFault { fault, .. } => {
                WorkerPreparationFailureReason::InterpreterFault(*fault)
            }
            PreparationFailureState::Unattempted(_) => WorkerPreparationFailureReason::Unattempted,
        }
    }

    /// Inspect every worker prepared before the failure in selection order.
    pub fn prepared(
        &self,
    ) -> impl ExactSizeIterator<Item = (&Role, &WorkerSubmission<Worker, Plan>)> {
        let prepared = match &self.state {
            PreparationFailureState::WorkerRejected { prepared, .. } => prepared.as_slice(),
            PreparationFailureState::SourceRejected { .. }
            | PreparationFailureState::InterpreterFault { .. }
            | PreparationFailureState::Unattempted(_) => &[],
        };
        prepared
            .iter()
            .map(|prepared| (prepared.role.role(), &prepared.submission))
    }

    /// Inspect every selected role left untouched after the failure.
    pub fn remaining_roles(&self) -> impl ExactSizeIterator<Item = &Role> {
        let remaining = match &self.state {
            PreparationFailureState::WorkerRejected { remaining, .. }
            | PreparationFailureState::SourceRejected { remaining, .. }
            | PreparationFailureState::InterpreterFault { remaining, .. } => remaining,
            PreparationFailureState::Unattempted(remaining) => remaining,
        };
        remaining.iter().map(RoleName::role)
    }
}

/// Complete role and proxy outcome retained after initial startup failed.
pub struct ProxyOutcomeFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    role: RoleName<Role>,
    outcome: ProxyOutcome<Worker, Plan>,
}

impl<Role, Worker, Plan> ProxyOutcomeFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) const fn new(role: RoleName<Role>, outcome: ProxyOutcome<Worker, Plan>) -> Self {
        Self { role, outcome }
    }

    /// Inspect the semantic role without exposing its private shared storage.
    #[must_use]
    pub fn role(&self) -> &Role {
        self.role.role()
    }

    /// Inspect the complete non-ready proxy outcome.
    #[must_use]
    pub const fn outcome(&self) -> &ProxyOutcome<Worker, Plan> {
        &self.outcome
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkerPreparationFailure, WorkerPreparationFailureReason};
    use crate::WorkerSubmission;
    use crate::atomic::{PreparedWorker, RoleName};
    use behavior::Never;

    fn submission(worker: u8) -> WorkerSubmission<u8, u16> {
        WorkerSubmission::activated(worker, u16::from(worker))
    }

    #[test]
    fn worker_rejection_preserves_prepared_order_and_untouched_roles() {
        let trigger = RoleName::new(1_u8);
        let failure: WorkerPreparationFailure<u8, u8, u16, u32, Never> =
            WorkerPreparationFailure::worker_rejected(
                trigger.clone(),
                vec![
                    PreparedWorker {
                        role: RoleName::new(2),
                        submission: submission(31),
                    },
                    PreparedWorker {
                        role: RoleName::new(3),
                        submission: submission(37),
                    },
                ],
                RoleName::new(4),
                41,
                vec![RoleName::new(5)],
            );

        assert_eq!(failure.role(), trigger.role());
        let prepared: Vec<_> = failure.prepared().collect();
        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0], (&2, &submission(31)));
        assert_eq!(prepared[1], (&3, &submission(37)));
        let WorkerPreparationFailureReason::WorkerRejected { role, reason } = failure.reason()
        else {
            panic!("worker rejection retains its role and reason");
        };
        assert_eq!(role, &4);
        assert_eq!(reason, &41);
        assert_eq!(failure.remaining_roles().collect::<Vec<_>>(), [&5]);
    }
}
