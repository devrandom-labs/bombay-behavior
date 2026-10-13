//! Complete construction inputs and rejection for a FIFO worker pool.

use behavior::{Behavior, BehaviorAddr, EndpointAddress};

use super::{AcceptedJobSequence, AssignmentSequence, FifoPool, PoolState, ShutdownSequence};
use crate::diagnostic::DiagnosticDisposition;
use crate::lifecycle::ActorDrainPolicy;
use crate::templates::pool::{
    Assignment, BacklogCapacity, CompletesAssignments, Interruption, PoolRecovery,
};
use crate::templates::worker::{
    InitialWorkerPreparation, InitialWorkerRejection, prepare_initial_workers,
};
use crate::worker::{OrderedRoles, StartupLimit};

/// Named choices for one direct-worker FIFO pool.
///
/// The factory and ordered roster remain separate construction inputs. Initial
/// preparation either succeeds for every worker or returns this complete product
/// with the prepared prefix, rejected role and untouched suffix. Validated
/// startup and capacity values retain their existing meanings.
///
/// Backlog capacity cannot stand in for startup capacity:
///
/// ```compile_fail,E0308
/// let config = behavior_actors::pool::queue::Config {
///     startup: behavior_actors::pool::BacklogCapacity::new(4),
///     recovery: behavior_actors::pool::PoolRecovery::temporary(
///         behavior_actors::pool::PoolFailureReaction::RetireRole,
///     ),
///     backlog: behavior_actors::pool::BacklogCapacity::new(4),
///     interruption: behavior_actors::pool::Interruption::Retry,
///     shutdown: behavior_actors::lifecycle::ActorDrainPolicy::WaitForActorGraph,
///     diagnostics: behavior_actors::diagnostic::DiagnosticDisposition::terminate(),
/// };
/// ```
pub struct Config<Source, DiagnosticRoute> {
    /// Bound concurrent worker startup.
    pub startup: StartupLimit,
    /// Select eligibility, worker source and recovery failure policy.
    pub recovery: PoolRecovery<Source>,
    /// Bound waiting jobs in the shared admission-order backlog.
    pub backlog: BacklogCapacity,
    /// Select whether interrupted jobs are returned or retried.
    pub interruption: Interruption,
    /// Select how the worker graph drains during shutdown.
    pub shutdown: ActorDrainPolicy,
    /// Deliver complete diagnostics through a typed route or terminate.
    pub diagnostics: DiagnosticDisposition<DiagnosticRoute>,
}

/// Complete custody when initial worker preparation rejects.
pub struct FifoConstructionRejected<Factory, Role, W, P, Rejection, Source, DiagnosticRoute> {
    /// Complete initial-worker rejection.
    pub workers: InitialWorkerRejection<Factory, Role, W, P, Rejection>,
    /// Complete named choices, unchanged from construction.
    pub config: Config<Source, DiagnosticRoute>,
}

/// Construct one FIFO pool after preparing every declared worker in order.
/// The factory may return `WorkerSubmission` directly or a fallible `Result`.
/// Rejection returns the original factory, prepared prefix, role, refusal and
/// untouched suffix together with every construction policy.
pub fn new<Factory, Role, W, P, Preparation, Source, DiagnosticRoute, Job, WorkerResult>(
    factory: Factory,
    roles: OrderedRoles<Role>,
    config: Config<Source, DiagnosticRoute>,
) -> Result<
    FifoPool<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult>,
    FifoConstructionRejected<Factory, Role, W, P, Preparation::Rejection, Source, DiagnosticRoute>,
>
where
    Factory: FnMut(&Role) -> Preparation,
    Preparation: InitialWorkerPreparation<Worker = W, Plan = P>,
    Role: Eq,
    W: Behavior,
    W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    BehaviorAddr<W>: EndpointAddress,
{
    let prepared = match prepare_initial_workers(factory, roles) {
        Ok(prepared) => prepared,
        Err(workers) => {
            return Err(FifoConstructionRejected { workers, config });
        }
    };
    let Config {
        startup,
        recovery,
        backlog,
        interruption,
        shutdown: actor_drain,
        diagnostics,
    } = config;
    Ok(FifoPool {
        state: PoolState::Constructed(prepared),
        startup,
        recovery: recovery.into(),
        restarts: crate::templates::restart::RestartBudget::empty(),
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
