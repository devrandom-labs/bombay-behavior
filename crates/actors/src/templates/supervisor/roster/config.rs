//! Complete construction inputs and rejection for an ordered supervisor roster.

use core::convert::Infallible;

use behavior::{Behavior, BehaviorAddr, CreationSequence, EndpointAddress};

use super::{FailureReaction, FixedRoster, FixedSupervisor, Recovery};
use crate::diagnostic::DiagnosticDisposition;
use crate::lifecycle::ActorDrainPolicy;
use crate::proxy::StableProxy;
use crate::templates::worker::{
    InitialWorkerPreparation, InitialWorkerRejection, WorkerSource, prepare_initial_workers,
};
use crate::worker::{OrderedRoles, StartupLimit, StartupPlan};

/// Named choices for one fixed, ordered supervisor roster.
///
/// Construction performs no work. The worker factory and roster remain separate
/// inputs; [`FixedBuilder::build`] either prepares the entire roster or returns
/// this complete configuration with every initial-worker rejection value.
/// Startup, recovery delay, and restart limits retain their validated values.
///
/// Required choices cannot be omitted:
///
/// ```compile_fail,E0063
/// let config = behavior_actors::supervisor::roster::Config {
///     recovery: behavior_actors::supervisor::roster::Recovery::temporary(),
///     failure: behavior_actors::supervisor::roster::FailureReaction::RetireMember,
///     shutdown: behavior_actors::lifecycle::ActorDrainPolicy::WaitForActorGraph,
///     diagnostics: behavior_actors::diagnostic::DiagnosticDisposition::terminate(),
/// };
/// ```
pub struct Config<Source, DiagnosticRoute> {
    /// Bound concurrent worker startup.
    pub startup: StartupLimit,
    /// Select eligibility, worker source, and automatic recovery policy.
    pub recovery: Recovery<Source>,
    /// Select what happens when a roster member becomes unavailable.
    pub failure: FailureReaction,
    /// Select how the actor graph drains during shutdown.
    pub shutdown: ActorDrainPolicy,
    /// Deliver complete diagnostics through a typed route or terminate.
    pub diagnostics: DiagnosticDisposition<DiagnosticRoute>,
}

/// Inferred construction value that owns every required supervisor input.
pub struct FixedBuilder<Factory, Role, Source, DiagnosticRoute, LifecycleRoute> {
    factory: Factory,
    roles: OrderedRoles<Role>,
    config: Config<Source, DiagnosticRoute>,
    lifecycle: Option<LifecycleRoute>,
}

/// Complete construction custody returned when one worker cannot be prepared.
pub struct FixedConstructionRejected<
    Factory,
    Role,
    Worker,
    Plan,
    Rejection,
    Source,
    DiagnosticRoute,
    LifecycleRoute,
> {
    /// Complete initial-worker rejection.
    pub workers: InitialWorkerRejection<Factory, Role, Worker, Plan, Rejection>,
    /// Complete named choices, unchanged from construction.
    pub config: Config<Source, DiagnosticRoute>,
    /// Optional lifecycle route; absence means autonomous operation.
    pub lifecycle: Option<LifecycleRoute>,
}

/// Begin the sole fixed-supervisor construction path with every required policy.
#[must_use]
pub fn new<Factory, Role, Source, DiagnosticRoute>(
    factory: Factory,
    roles: OrderedRoles<Role>,
    config: Config<Source, DiagnosticRoute>,
) -> FixedBuilder<Factory, Role, Source, DiagnosticRoute, Infallible> {
    FixedBuilder {
        factory,
        roles,
        config,
        lifecycle: None,
    }
}

impl<Factory, Role, Source, DiagnosticRoute>
    FixedBuilder<Factory, Role, Source, DiagnosticRoute, Infallible>
{
    /// Publish lifecycle events through one typed route.
    #[must_use]
    pub fn publish_lifecycle<Route>(
        self,
        route: Route,
    ) -> FixedBuilder<Factory, Role, Source, DiagnosticRoute, Route> {
        FixedBuilder {
            factory: self.factory,
            roles: self.roles,
            config: self.config,
            lifecycle: Some(route),
        }
    }
}

impl<Factory, Role, Source, DiagnosticRoute, LifecycleRoute>
    FixedBuilder<Factory, Role, Source, DiagnosticRoute, LifecycleRoute>
{
    /// Prepare every initial worker in declaration order or return all custody.
    /// The factory returns `WorkerSubmission` directly when preparation cannot
    /// fail, or `Result<WorkerSubmission, Rejection>` for an actual refusal.
    /// Its return value determines the rejection type without a separate slot.
    pub fn build<Worker, Plan, Preparation>(
        self,
    ) -> Result<
        FixedSupervisor<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute>,
        FixedConstructionRejected<
            Factory,
            Role,
            Worker,
            Plan,
            Preparation::Rejection,
            Source,
            DiagnosticRoute,
            LifecycleRoute,
        >,
    >
    where
        Factory: FnMut(&Role) -> Preparation,
        Preparation: InitialWorkerPreparation<Worker = Worker, Plan = Plan>,
        Role: Send + Sync,
        Source: WorkerSource<Role, Worker, Plan>,
        Worker: behavior::Behavior + Send,
        Plan: StartupPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        let Self {
            factory,
            roles,
            config,
            lifecycle,
        } = self;
        let prepared = match prepare_initial_workers(factory, roles) {
            Ok(prepared) => prepared,
            Err(workers) => {
                return Err(FixedConstructionRejected {
                    workers,
                    config,
                    lifecycle,
                });
            }
        };
        let Config {
            startup,
            recovery,
            failure: failure_reaction,
            shutdown: actor_drain,
            diagnostics,
        } = config;
        Ok(FixedSupervisor {
            roster: FixedRoster::new(prepared),
            creations: CreationSequence::new(),
            startup,
            recovery: recovery.into(),
            failure_reaction,
            actor_drain,
            diagnostics,
            lifecycle,
        })
    }
}
