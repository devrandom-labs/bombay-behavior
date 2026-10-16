//! Complete construction inputs and rejection for a FIFO worker pool.

use core::marker::PhantomData;

use crate::composition::replies::{ExactReplies, MixedReplies, ReplyRoutes};
use behavior::{Behavior, BehaviorAddr, EndpointAddress};

use super::{AcceptedJobSequence, AssignmentSequence, FifoPool, PoolState, ShutdownSequence};
use crate::diagnostic::DiagnosticDisposition;
use crate::lifecycle::ActorDrainPolicy;
use crate::templates::pool::{Assignment, BacklogCapacity, Interruption, PoolRecovery};
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
pub struct FifoConstructionRejected<
    Factory,
    Role,
    W,
    P,
    Rejection,
    Source,
    DiagnosticRoute,
    Replies,
> {
    /// Complete initial-worker rejection.
    pub workers: InitialWorkerRejection<Factory, Role, W, P, Rejection>,
    /// Complete named choices, unchanged from construction.
    pub config: Config<Source, DiagnosticRoute>,
    replies: PhantomData<fn() -> Replies>,
}

/// Own the complete pool inputs until ordered worker preparation is requested.
///
/// Construction performs no work. The selected reply family fixes the eventual
/// command and send capabilities without a caller-supplied route marker.
pub struct FifoBuilder<Factory, Role, Source, DiagnosticRoute, Replies> {
    factory: Factory,
    roles: OrderedRoles<Role>,
    config: Config<Source, DiagnosticRoute>,
    replies: PhantomData<fn() -> Replies>,
}

/// Begin pool construction with named policies and exact customer replies.
#[must_use]
pub fn new<Factory, Role, Source, DiagnosticRoute>(
    factory: Factory,
    roles: OrderedRoles<Role>,
    config: Config<Source, DiagnosticRoute>,
) -> FifoBuilder<Factory, Role, Source, DiagnosticRoute, ExactReplies> {
    FifoBuilder {
        factory,
        roles,
        config,
        replies: PhantomData,
    }
}

impl<Factory, Role, Source, DiagnosticRoute>
    FifoBuilder<Factory, Role, Source, DiagnosticRoute, ExactReplies>
{
    /// Accept logical customers as well as exact endpoints.
    ///
    /// This changes the concrete command and send types. Interpreters must
    /// provide the corresponding logical protocol namespace.
    #[must_use]
    pub fn logical_replies(
        self,
    ) -> FifoBuilder<Factory, Role, Source, DiagnosticRoute, MixedReplies> {
        FifoBuilder {
            factory: self.factory,
            roles: self.roles,
            config: self.config,
            replies: PhantomData,
        }
    }
}

impl<Factory, Role, Source, DiagnosticRoute, Replies: ReplyRoutes>
    FifoBuilder<Factory, Role, Source, DiagnosticRoute, Replies>
{
    /// Prepare every worker in declaration order or return complete custody.
    ///
    /// Workers may emit ordinary business sends alongside assignment completion.
    /// Construction infers the result from an unambiguous completion lane, including
    /// named products and wrappers.
    ///
    /// The factory may return `WorkerSubmission` directly or a fallible
    /// `Result`. Rejection preserves the factory, prepared prefix, rejected
    /// role and reason, untouched suffix, policies and reply capability choice.
    pub fn build<W, P, Preparation, Job, WorkerResult, Path>(
        self,
    ) -> Result<
        FifoPool<Role, W, P, Source, DiagnosticRoute, Job, WorkerResult, Replies>,
        FifoConstructionRejected<
            Factory,
            Role,
            W,
            P,
            Preparation::Rejection,
            Source,
            DiagnosticRoute,
            Replies,
        >,
    >
    where
        Factory: FnMut(&Role) -> Preparation,
        Preparation: InitialWorkerPreparation<Worker = W, Plan = P>,
        Role: Eq,
        W: Behavior,
        W::Protocol: behavior::Protocol<Msg = Assignment<Job>>,
        W::Sends:
            behavior::SendInput<behavior::ReportToParent<super::Completion<WorkerResult>>, Path>,
        WorkerResult: Send,
        BehaviorAddr<W>: EndpointAddress,
    {
        let Self {
            factory,
            roles,
            config,
            replies: _,
        } = self;
        let prepared = match prepare_initial_workers(factory, roles) {
            Ok(prepared) => prepared,
            Err(workers) => {
                return Err(FifoConstructionRejected {
                    workers,
                    config,
                    replies: PhantomData,
                });
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
}
