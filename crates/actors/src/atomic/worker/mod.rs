//! Worker submissions and shared start contracts used by atomic aggregates.

use std::sync::Arc;

use behavior::{
    Actions, Behavior, BehaviorAddr, ChildCreationOutcome, CreationId, CreationRejection,
    EndpointAddress, EstablishedActor, InterpreterFault, ItemSettlement, Never, RoutedCreation,
    SettledItem,
};

use crate::{Exit, StopOnShutdown, TerminalOutcome};

mod activation;
mod initialization;
mod preparation;
mod roster;

pub(in crate::atomic) use activation::{ActivationAttempt, WorkerActivationOutcome};
pub use activation::{ActivationStartRejection, BeginActivation, WorkerActivation};
pub use initialization::{
    ActivationPermit, InitializationAttempt, InitializeWorker, WorkerInitializationFailure,
    WorkerInitializationOutcome, WorkerInitializationReport,
};
pub use preparation::{PendingWorkerPreparation, PrepareWorkers, WorkerPreparation, WorkerSource};
pub(in crate::atomic) use preparation::{
    PreparationTicket, WorkerPreparationOutcome, preparation_result_accepts,
};
pub use roster::InitialWorkerRejection;
pub(in crate::atomic) use roster::prepare_initial_workers;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::atomic) enum StopKind {
    Normal,
    Abnormal,
}

pub(in crate::atomic) const fn stop_kind<A>(outcome: &TerminalOutcome<A>) -> StopKind
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
#[doc(hidden)]
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
    pub(in crate::atomic) const fn new(worker: W, initialization: HostedInitialization<W>) -> Self {
        Self {
            worker,
            initialization,
        }
    }

    /// Transfer both affine values to Bombay's terminal custodian.
    #[doc(hidden)]
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
    /// Bombay could not route the creator's child creation.
    NamespaceExhausted { worker: W },
    /// The worker's pure initialization transition rejected.
    WorkerRejected { worker: W, error: W::Error },
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

pub(in crate::atomic) type WorkerCreationSettlement<W> = SettledItem<
    RoutedCreation<BehaviorAddr<W>, StopOnShutdown<W>>,
    ItemSettlement<
        RoutedCreation<BehaviorAddr<W>, StopOnShutdown<W>>,
        ChildCreationOutcome<StopOnShutdown<W>, behavior::ChildHead>,
        CreationRejection,
        Never,
    >,
>;

pub(in crate::atomic) enum WorkerCreationOutcome<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
{
    Established(EstablishedActor<StopOnShutdown<W>>),
    Rejected(WorkerCreationRejection<W>),
    InvalidSettlement(WorkerCreationSettlement<W>),
}

pub(in crate::atomic) fn settle_worker_creation<W>(
    settlement: WorkerCreationSettlement<W>,
) -> WorkerCreationOutcome<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
{
    match settlement {
        SettledItem::Attempted(ItemSettlement::Accepted(created)) => match created.into_actor() {
            Ok(actor) => WorkerCreationOutcome::Established(actor),
            Err(created @ ChildCreationOutcome::Established { .. }) => {
                WorkerCreationOutcome::InvalidSettlement(SettledItem::Attempted(
                    ItemSettlement::Accepted(created),
                ))
            }
            Err(ChildCreationOutcome::InitializationRejected { creation, error }) => {
                WorkerCreationOutcome::Rejected(WorkerCreationRejection::WorkerRejected {
                    worker: recover_worker(creation),
                    error,
                })
            }
            Err(ChildCreationOutcome::HostRejected {
                creation,
                initialization,
                reason,
            }) => WorkerCreationOutcome::Rejected(WorkerCreationRejection::HostRejected {
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
/// Atomic aggregates never invoke application activation work in their pure
/// transitions.
pub trait ActivationPlan: Send {
    /// Evidence returned when activation makes the worker ready.
    type Ready: Send;
    /// Exact application rejection returned by activation.
    type Rejection: Send;

    /// Perform the selected activation work outside the worker behavior.
    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send;
}

/// A worker that needs no application activation work after initialization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImmediateActivation;

impl ActivationPlan for ImmediateActivation {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

/// Exact non-forgeable evidence for one worker creation attempt.
#[doc(hidden)]
#[derive(Clone)]
pub struct WorkerAttempt {
    creation: CreationId,
    token: Arc<()>,
}

impl WorkerAttempt {
    pub(in crate::atomic) fn issued(creation: CreationId) -> Self {
        Self {
            creation,
            token: Arc::new(()),
        }
    }

    /// Creator-local correlation for this worker attempt.
    #[doc(hidden)]
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
    pub(crate) activation: P,
}

impl<W> WorkerSubmission<W, ImmediateActivation> {
    /// Prepare a worker that needs no application activation work.
    #[must_use]
    pub const fn immediate(worker: W) -> Self {
        Self {
            worker,
            activation: ImmediateActivation,
        }
    }
}

impl<W, P> WorkerSubmission<W, P> {
    /// Prepare a worker with one concrete owned activation plan.
    #[must_use]
    pub const fn activated(worker: W, activation: P) -> Self {
        Self { worker, activation }
    }
}

/// One semantic role and its complete prepared worker submission.
#[derive(Debug, Eq, PartialEq)]
pub struct PreparedWorker<Role, W, P> {
    /// Application role owned by this worker relationship.
    pub role: Role,
    /// Worker behavior and activation work prepared for the role.
    pub submission: WorkerSubmission<W, P>,
}
