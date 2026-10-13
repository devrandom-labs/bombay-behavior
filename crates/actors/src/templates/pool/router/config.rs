//! Complete construction choices and refusal for a keyed worker router.

use behavior::{Behavior, BehaviorAddr, CreationSequence, EndpointAddress, Protocol};

use super::{
    AcceptedJobSequence, AssignmentSequence, BindingCapacity, KeyedPool, KeyedPoolState,
    ShutdownSequence,
};
use crate::diagnostic::DiagnosticDisposition;
use crate::lifecycle::ActorDrainPolicy;
use crate::pool::{Assignment, BacklogCapacity, CompletesAssignments, Interruption, PoolRecovery};
use crate::templates::worker::{
    InitialWorkerPreparation, InitialWorkerRejection, prepare_initial_workers,
};
use crate::worker::{OrderedRoles, StartupLimit};

/// Named choices for one direct-worker pool with per-role queues and key affinity.
///
/// The worker factory, ordered roster and key selector remain separate inputs.
/// Preparation either succeeds for every worker or returns this complete product
/// with the original selector, prepared prefix, refused role and untouched suffix.
/// Backlog and binding capacities retain their distinct validated meanings.
pub struct Config<Source, Diagnostics> {
    /// Bound concurrent worker startup.
    pub startup: StartupLimit,
    /// Select eligibility, worker source and recovery failure policy.
    pub recovery: PoolRecovery<Source>,
    /// Bound waiting jobs in each role's queue.
    pub backlog: BacklogCapacity,
    /// Bound the number of retained key-to-role bindings.
    pub bindings: BindingCapacity,
    /// Select whether interrupted jobs are returned or retried.
    pub interruption: Interruption,
    /// Select how the worker graph drains during shutdown.
    pub shutdown: ActorDrainPolicy,
    /// Deliver complete diagnostics through a typed route or terminate.
    pub diagnostics: DiagnosticDisposition<Diagnostics>,
}

/// Complete custody when initial keyed-worker preparation rejects.
pub struct KeyedConstructionRejected<Factory, Selector, Role, W, P, Rejection, Source, Diagnostics>
{
    /// Complete initial-worker rejection.
    pub workers: InitialWorkerRejection<Factory, Role, W, P, Rejection>,
    /// Complete key selector.
    pub selector: Selector,
    /// Complete named choices, unchanged from construction.
    pub config: Config<Source, Diagnostics>,
}

/// Construct one keyed pool after preparing every declared worker in order.
/// The factory may return `WorkerSubmission` directly or a fallible `Result`.
/// Rejection returns the original factory, prepared prefix, role, refusal and
/// untouched suffix together with the selector and every construction policy.
pub fn new<
    Factory,
    Selector,
    Role,
    W,
    P,
    Preparation,
    Source,
    Diagnostics,
    Key,
    Job,
    WorkerResult,
>(
    factory: Factory,
    roles: OrderedRoles<Role>,
    selector: Selector,
    config: Config<Source, Diagnostics>,
) -> Result<
    KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>,
    KeyedConstructionRejected<
        Factory,
        Selector,
        Role,
        W,
        P,
        Preparation::Rejection,
        Source,
        Diagnostics,
    >,
>
where
    Factory: FnMut(&Role) -> Preparation,
    Preparation: InitialWorkerPreparation<Worker = W, Plan = P>,
    Selector: Fn(&Key) -> Role,
    Role: Eq,
    W: Behavior,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    BehaviorAddr<W>: EndpointAddress,
{
    let prepared = match prepare_initial_workers(factory, roles) {
        Ok(prepared) => prepared,
        Err(workers) => {
            return Err(KeyedConstructionRejected {
                workers,
                selector,
                config,
            });
        }
    };
    let Config {
        startup,
        recovery,
        backlog,
        bindings: binding_capacity,
        interruption,
        shutdown: actor_drain,
        diagnostics,
    } = config;
    Ok(KeyedPool {
        state: KeyedPoolState::Constructed(prepared),
        selector,
        startup,
        recovery: recovery.into(),
        restarts: crate::templates::restart::RestartBudget::empty(),
        backlog,
        binding_capacity,
        interruption,
        actor_drain,
        diagnostics,
        creations: CreationSequence::new(),
        jobs: AcceptedJobSequence::new(),
        assignments: AssignmentSequence::new(),
        shutdowns: ShutdownSequence::new(),
        next_restart_timer: 1,
    })
}
