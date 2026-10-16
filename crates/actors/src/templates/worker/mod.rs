//! Worker submissions and shared start contracts used by atomic aggregates.

use std::sync::Arc;

use behavior::{
    Actions, Behavior, BehaviorAddr, ChildCreationOutcome, CreationId, CreationKind,
    CreationRejection, EndpointAddress, EstablishedActor, InterpreterFault, ItemSettlement, Never,
    RoutedCreation, SettledItem,
};

use crate::{Exit, StopOnShutdown, TerminalOutcome};

mod initialization;
mod limit;
mod preparation;
mod roster;
mod startup;

pub use initialization::{
    InitializationAttempt, InitializeWorker, StartupPermit, WorkerInitializationFailure,
    WorkerInitializationOutcome, WorkerInitializationReport,
};
pub use preparation::{
    PendingWorkerPreparation, PrepareWorkers, StartingWorkerPreparation, WorkerPreparation,
    WorkerPreparationStarted, WorkerSource,
};
pub(in crate::templates) use preparation::{
    PreparationTicket, WorkerPreparationExpectation, WorkerPreparationOutcome,
};
pub(in crate::templates) use roster::prepare_initial_workers;
pub use roster::{InitialWorkerPreparation, InitialWorkerRejection};
pub(in crate::templates) use startup::WorkerStartupOutcome;
pub use startup::{StartWorker, StartupRejection, WorkerStartup, WorkerStartupGrant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::templates) enum StopKind {
    Normal,
    Abnormal,
}

pub(in crate::templates) const fn stop_kind<A>(outcome: &TerminalOutcome<A>) -> StopKind
where
    A: behavior::Address,
{
    match outcome {
        Ok(Exit::Normal | Exit::Collected) => StopKind::Normal,
        Ok(Exit::LinkDied(_) | Exit::SupervisionFailed(_)) | Err(_) => StopKind::Abnormal,
    }
}

/// Initialization actions retained together with an uncommitted worker.
#[doc(hidden)]
pub type HostedInitialization<W> = Actions<
    BehaviorAddr<W>,
    <W as Behavior>::Ph,
    <StopOnShutdown<W> as Behavior>::Sends,
    <W as Behavior>::Birth,
>;

/// An uncommitted worker returned together with initialization actions that
/// the host rejected before interpreting.
#[must_use = "a rejected worker and its initialization actions require custody"]
pub struct WorkerRecovery<W>
where
    W: Behavior,
{
    worker: W,
    initialization: HostedInitialization<W>,
}

impl<W> WorkerRecovery<W>
where
    W: Behavior,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol, Ph = W::Ph, Birth = W::Birth>,
{
    pub(in crate::templates) const fn new(
        worker: W,
        initialization: HostedInitialization<W>,
    ) -> Self {
        Self {
            worker,
            initialization,
        }
    }

    /// Transfer both affine values to Bombay's terminal custodian.
    #[must_use]
    pub fn into_retirement(self) -> (W, HostedInitialization<W>) {
        (self.worker, self.initialization)
    }
}

impl<W> core::fmt::Debug for WorkerRecovery<W>
where
    W: Behavior,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol, Ph = W::Ph, Birth = W::Birth>,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WorkerRecovery")
            .finish_non_exhaustive()
    }
}

/// Complete rejection returned by one worker-creation attempt.
pub enum WorkerCreationRejection<W>
where
    W: Behavior,
{
    /// Creation did not commit and no worker definition survived for return.
    Unavailable { id: CreationId, kind: CreationKind },
    /// Bombay could not route the creator's child creation.
    NamespaceExhausted { worker: W },
    /// The worker's pure initialization transition rejected.
    WorkerRejected { worker: W, error: W::Error },
    /// The worker's pure initialization panicked before host commitment.
    WorkerPanicked { worker: W },
    /// Host establishment rejected after initialization produced actions.
    HostRejected {
        recovery: WorkerRecovery<W>,
        reason: CreationRejection,
    },
    /// The creation capability lawfully rejected and returned the worker.
    CreationRejected {
        worker: W,
        reason: CreationRejection,
    },
    /// The interpreter violated its creation contract and returned the worker.
    InterpreterCorrupt { worker: W, fault: InterpreterFault },
    /// Product traversal stopped before attempting this worker creation.
    InterpretationSkipped { worker: W },
}

pub(in crate::templates) type WorkerCreationSettlement<W> = SettledItem<
    RoutedCreation<BehaviorAddr<W>, StopOnShutdown<W>>,
    ItemSettlement<
        RoutedCreation<BehaviorAddr<W>, StopOnShutdown<W>>,
        ChildCreationOutcome<StopOnShutdown<W>, behavior::ChildHead>,
        CreationRejection,
        Never,
    >,
>;

pub(in crate::templates) enum WorkerCreationOutcome<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
{
    Established(EstablishedActor<StopOnShutdown<W>>),
    Rejected(WorkerCreationRejection<W>),
}

#[cfg(test)]
mod creation_outcome_tests {
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    use super::WorkerCreationOutcome;
    use crate::StopOnShutdown;

    #[expect(
        dead_code,
        reason = "compile contract for complete worker creation custody"
    )]
    fn settled_worker_outcome<W>(outcome: WorkerCreationOutcome<W>)
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
        StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
    {
        match outcome {
            WorkerCreationOutcome::Established(_) | WorkerCreationOutcome::Rejected(_) => {}
        }
    }
}

pub(in crate::templates) fn settle_worker_creation<W>(
    settlement: WorkerCreationSettlement<W>,
) -> WorkerCreationOutcome<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
{
    match settlement {
        SettledItem::Attempted(ItemSettlement::Accepted(created)) => match created {
            ChildCreationOutcome::Unavailable { id, kind } => {
                WorkerCreationOutcome::Rejected(WorkerCreationRejection::Unavailable { id, kind })
            }
            ChildCreationOutcome::Established(child) => {
                WorkerCreationOutcome::Established(child.into_parts().2)
            }
            ChildCreationOutcome::InitializationRejected { creation, error } => {
                WorkerCreationOutcome::Rejected(WorkerCreationRejection::WorkerRejected {
                    worker: recover_worker(creation),
                    error,
                })
            }
            ChildCreationOutcome::InitializationPanicked { creation } => {
                WorkerCreationOutcome::Rejected(WorkerCreationRejection::WorkerPanicked {
                    worker: recover_worker(creation),
                })
            }
            ChildCreationOutcome::HostRejected {
                creation,
                initialization,
                reason,
            } => WorkerCreationOutcome::Rejected(WorkerCreationRejection::HostRejected {
                recovery: WorkerRecovery::new(recover_worker(creation), initialization),
                reason,
            }),
        },
        SettledItem::Attempted(ItemSettlement::Rejected { item, reason }) => {
            WorkerCreationOutcome::Rejected(WorkerCreationRejection::CreationRejected {
                worker: recover_worker(item),
                reason,
            })
        }
        SettledItem::Attempted(ItemSettlement::Blocked {
            item: _,
            prerequisite,
        }) => match prerequisite {},
        SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }) => {
            WorkerCreationOutcome::Rejected(WorkerCreationRejection::InterpreterCorrupt {
                worker: recover_worker(item),
                fault,
            })
        }
        SettledItem::Unattempted(item) => {
            WorkerCreationOutcome::Rejected(WorkerCreationRejection::InterpretationSkipped {
                worker: recover_worker(item),
            })
        }
    }
}

fn recover_worker<W>(creation: RoutedCreation<BehaviorAddr<W>, StopOnShutdown<W>>) -> W
where
    W: Behavior,
{
    let (creation, _) = creation.into_parts();
    let (_, worker, _) = creation.into_parts();
    worker.into_inner()
}

/// Concrete work required before a newly created worker may serve commands.
///
/// Bombay interprets the owned plan after creation and initialization settle.
/// Atomic aggregates never invoke application startup work in their pure
/// transitions.
pub trait StartupPlan: Send {
    /// Evidence returned when startup makes the worker ready.
    type Ready: Send;
    /// Exact application rejection returned by startup.
    type Rejection: Send;

    /// Perform the selected startup work outside the worker behavior.
    fn start(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send;
}

/// A worker that needs no application startup work after initialization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImmediateStartup;

impl StartupPlan for ImmediateStartup {
    type Ready = ();
    type Rejection = Never;

    fn start(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

/// Exact non-forgeable evidence for one worker creation attempt.
#[derive(Clone)]
pub struct WorkerAttempt {
    creation: CreationId,
    token: Arc<()>,
}

impl WorkerAttempt {
    pub(in crate::templates) fn issued(creation: CreationId) -> Self {
        Self {
            creation,
            token: Arc::new(()),
        }
    }

    /// Creator-local correlation for this worker attempt. This number is not
    /// actor identity or proof that creation committed.
    #[must_use]
    pub const fn creation(&self) -> CreationId {
        self.creation
    }
}

impl core::fmt::Debug for WorkerAttempt {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("WorkerAttempt")
            .field("creation", &self.creation)
            .finish_non_exhaustive()
    }
}

impl PartialEq for WorkerAttempt {
    fn eq(&self, other: &Self) -> bool {
        self.creation == other.creation && Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for WorkerAttempt {}

/// Complete affine input for one worker start or replacement.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerSubmission<W, P> {
    pub(crate) worker: W,
    pub(crate) startup: P,
}

impl<W> WorkerSubmission<W, ImmediateStartup> {
    /// Prepare a worker that needs no application startup work.
    #[must_use]
    pub const fn immediate(worker: W) -> Self {
        Self {
            worker,
            startup: ImmediateStartup,
        }
    }
}

impl<W, P> WorkerSubmission<W, P> {
    /// Prepare a worker with one concrete owned startup plan.
    #[must_use]
    pub const fn with_startup(worker: W, startup: P) -> Self {
        Self { worker, startup }
    }
}

/// One semantic role and its complete prepared worker submission.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedWorker<Role, W, P> {
    /// Application role owned by this worker relationship.
    pub role: Role,
    /// Worker behavior and startup work prepared for the role.
    pub submission: WorkerSubmission<W, P>,
}

pub use super::restart::{
    RestartLimit, RestartRelease, RestartReleaseError, RestartReleaseFailure,
};
pub use super::roster::{DuplicateRole, OrderedRoles};
pub use limit::StartupLimit;
