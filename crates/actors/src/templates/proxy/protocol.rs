//! StableProxy startup policy, owner control, outcomes, and diagnostics.

use behavior::{
    Behavior, BehaviorAddr, Births, ChildInputIngress, CreationsSettled, EndpointAddress,
    EventIngress, Here, InjectEvent, Protocol, RecoverEvent, User, UserEvent,
};

use crate::worker::{
    ImmediateStartup, StartWorker, StartupPermit, StartupPlan, StartupRejection,
    WorkerInitializationFailure,
};
use crate::{ChildStopped, EstablishedShutdownResolved, StopOnShutdown, WorkerSubmission};

use super::{
    StableProxy, WorkerAttempt, WorkerInitializationReport, WorkerStartResult, WorkerStartup,
};

/// Owner-only operation accepted by [`super::StableProxy`].
pub struct ProxyControl<W, P> {
    pub(super) command: ProxyCommand<W, P>,
}

pub(super) enum ProxyCommand<W, P> {
    Start(WorkerSubmission<W, P>),
    Replace(WorkerSubmission<W, P>),
    Shutdown,
}

impl<W> ProxyControl<W, ImmediateStartup> {
    /// Start an immediately ready worker without an unused plan argument.
    #[must_use]
    pub const fn start(worker: W) -> Self {
        Self {
            command: ProxyCommand::Start(WorkerSubmission::immediate(worker)),
        }
    }

    /// Replace with an immediately ready worker.
    #[must_use]
    pub const fn replace(worker: W) -> Self {
        Self {
            command: ProxyCommand::Replace(WorkerSubmission::immediate(worker)),
        }
    }
}

impl<W, P> ProxyControl<W, P> {
    /// Shut down the proxy without an unused worker or startup input.
    #[must_use]
    pub const fn shutdown() -> Self {
        Self {
            command: ProxyCommand::Shutdown,
        }
    }

    /// Start a worker with application-defined startup work.
    #[must_use]
    pub const fn start_with(worker: W, startup: P) -> Self {
        Self {
            command: ProxyCommand::Start(WorkerSubmission::with_startup(worker, startup)),
        }
    }

    /// Replace with a worker using application-defined startup work.
    #[must_use]
    pub const fn replace_with(worker: W, startup: P) -> Self {
        Self {
            command: ProxyCommand::Replace(WorkerSubmission::with_startup(worker, startup)),
        }
    }
}

/// Observable lifecycle phase carried by proxy rejections and diagnostics.
///
/// This value grants no transition or routing authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyPhase {
    Dormant,
    Creating,
    Initializing,
    Starting,
    Ready,
    EmptyInitial,
    EmptyAfter,
    Replacing,
    ReturningWorker,
    ShuttingDown,
    Stopped,
}

/// Complete disposition of one initial worker submission.
pub enum InitialWorkerOutcome<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol, Ph = W::Ph, Birth = W::Birth>,
{
    /// Another initial submission already owns the proxy.
    Overlap {
        worker: W,
        startup: P,
        phase: ProxyPhase,
    },
    /// The proxy cannot reserve another non-reused worker attempt.
    WorkerAttemptsExhausted { worker: W, startup: P },
    /// The worker-start operation reached one complete semantic result.
    Resolved { result: WorkerStartResult<W, P> },
}

/// Complete disposition of one submitted replacement worker.
pub enum ReplacementOutcome<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol, Ph = W::Ph, Birth = W::Birth>,
{
    /// The current proxy phase cannot accept another replacement.
    NotReplaceable {
        worker: W,
        startup: P,
        phase: ProxyPhase,
    },
    /// The proxy cannot reserve another non-reused worker attempt.
    WorkerAttemptsExhausted {
        replaces: WorkerAttempt,
        worker: W,
        startup: P,
    },
    /// Owner shutdown returned a successor before its creation was emitted.
    CancelledBeforeBirth {
        replaces: WorkerAttempt,
        worker: W,
        startup: P,
    },
    /// The successor start produced one complete result.
    Resolved {
        replaces: WorkerAttempt,
        result: WorkerStartResult<W, P>,
    },
}

/// Lifecycle input returned without changing the current proxy state.
pub enum ProxyDiagnostic<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    /// A worker-creation settlement did not belong to the current start.
    UnexpectedWorkerStart {
        phase: ProxyPhase,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    },
    /// A worker stop did not belong to the worker expected by this phase.
    UnexpectedWorkerStop {
        phase: ProxyPhase,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    /// A child-host result did not belong to the worker expected by this phase.
    UnexpectedWorkerInitialization {
        phase: ProxyPhase,
        initialization: WorkerInitializationReport<W, P>,
    },
    /// A startup input did not belong to the worker expected by this phase.
    UnexpectedWorkerStartup {
        phase: ProxyPhase,
        startup: WorkerStartup<W, P>,
    },
    /// An exact shutdown resolution did not belong to the current worker drain.
    UnexpectedWorkerShutdown {
        phase: ProxyPhase,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    },
}

/// Complete report emitted to the structural owner of a stable proxy.
pub enum ProxyOutcome<W, P>
where
    W: Behavior,
    P: StartupPlan,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol, Ph = W::Ph, Birth = W::Birth>,
    <W::Protocol as Protocol>::Addr: EndpointAddress,
{
    /// Result of the first submitted worker.
    Initial { outcome: InitialWorkerOutcome<W, P> },
    /// Result of one submitted replacement worker.
    Replacement { outcome: ReplacementOutcome<W, P> },
    /// The exact ready worker stopped and left the stable service unavailable.
    WorkerStopped {
        worker: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    /// A service command arrived while no worker was ready.
    Unavailable {
        sender: BehaviorAddr<W>,
        phase: ProxyPhase,
        command: <W::Protocol as Protocol>::Msg,
    },
}

/// Complete retained values proving how one unavailable worker was returned.
#[must_use = "a proxy return retains affine worker lifecycle values"]
pub enum ProxyDrain<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: StartupPlan,
{
    /// Pure initialization rejected after the worker was committed.
    InitializationRejected {
        failure: WorkerInitializationFailure,
        startup: P,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    /// Initialization stopped after the worker was committed.
    InitializationStopped {
        startup: P,
        observed: Option<ChildStopped<BehaviorAddr<W>>>,
        returned: ChildStopped<BehaviorAddr<W>>,
    },
    /// Initialization completed, but an exact worker stop prevented startup.
    InitializationCompleted {
        permit: StartupPermit<W>,
        startup: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    /// The startup request was rejected before the plan ran.
    StartupRefused {
        request: StartWorker<W, P>,
        reason: StartupRejection,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    /// The startup plan rejected after its request was accepted.
    StartupRejected {
        rejection: P::Rejection,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    /// Activation completed, but an exact worker stop prevented availability.
    StartupCompleted {
        readiness: P::Ready,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

#[doc(hidden)]
pub enum ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Service(User<BehaviorAddr<W>, <W::Protocol as Protocol>::Msg>),
    Owner(ProxyControl<W, P>),
    WorkerCreationsSettled(CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>),
    WorkerInitialization(WorkerInitializationReport<W, P>),
    WorkerStartupReported(WorkerStartup<W, P>),
    WorkerShutdownResolved(EstablishedShutdownResolved<W::Protocol>),
    WorkerStopped(ChildStopped<BehaviorAddr<W>>),
}

impl<W, P>
    EventIngress<Births<StopOnShutdown<W>>, CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>>
    for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn ingress(input: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>) -> Self {
        Self::WorkerCreationsSettled(input)
    }
}

impl<W, P> InjectEvent<CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>, Here>
    for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn inject_at(input: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>) -> Self {
        Self::WorkerCreationsSettled(input)
    }
}

impl<W, P> UserEvent for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    type Addr = BehaviorAddr<W>;
    type Message = <W::Protocol as Protocol>::Msg;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Service(User::new(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Service(service) => Ok(service),
            other => Err(other),
        }
    }
}

impl<W, P> InjectEvent<ProxyControl<W, P>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn inject_at(input: ProxyControl<W, P>) -> Self {
        Self::Owner(input)
    }
}

impl<W, P> ChildInputIngress<StableProxy<W, P>, ProxyControl<W, P>> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn child_input(input: ProxyControl<W, P>) -> Self {
        Self::Owner(input)
    }

    fn recover_child_input(event: Self) -> Result<ProxyControl<W, P>, Self> {
        <Self as RecoverEvent<ProxyControl<W, P>, Here>>::recover(event)
    }
}

impl<W, P> RecoverEvent<ProxyControl<W, P>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn recover(event: Self) -> Result<ProxyControl<W, P>, Self> {
        match event {
            Self::Owner(control) => Ok(control),
            other => Err(other),
        }
    }
}

impl<W, P> RecoverEvent<CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>, Here>
    for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn recover(event: Self) -> Result<CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>, Self> {
        match event {
            Self::WorkerCreationsSettled(workers) => Ok(workers),
            other => Err(other),
        }
    }
}

impl<W, P> InjectEvent<ChildStopped<BehaviorAddr<W>>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn inject_at(input: ChildStopped<BehaviorAddr<W>>) -> Self {
        Self::WorkerStopped(input)
    }
}

impl<W, P> RecoverEvent<ChildStopped<BehaviorAddr<W>>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn recover(event: Self) -> Result<ChildStopped<BehaviorAddr<W>>, Self> {
        match event {
            Self::WorkerStopped(stopped) => Ok(stopped),
            other => Err(other),
        }
    }
}

impl<W, P> InjectEvent<WorkerInitializationReport<W, P>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn inject_at(input: WorkerInitializationReport<W, P>) -> Self {
        Self::WorkerInitialization(input)
    }
}

impl<W, P> RecoverEvent<WorkerInitializationReport<W, P>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn recover(event: Self) -> Result<WorkerInitializationReport<W, P>, Self> {
        match event {
            Self::WorkerInitialization(initialization) => Ok(initialization),
            other => Err(other),
        }
    }
}

impl<W, P> InjectEvent<WorkerStartup<W, P>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn inject_at(input: WorkerStartup<W, P>) -> Self {
        Self::WorkerStartupReported(input)
    }
}

impl<W, P> RecoverEvent<WorkerStartup<W, P>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn recover(event: Self) -> Result<WorkerStartup<W, P>, Self> {
        match event {
            Self::WorkerStartupReported(startup) => Ok(startup),
            other => Err(other),
        }
    }
}

impl<W, P> InjectEvent<EstablishedShutdownResolved<W::Protocol>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn inject_at(input: EstablishedShutdownResolved<W::Protocol>) -> Self {
        Self::WorkerShutdownResolved(input)
    }
}

impl<W, P> RecoverEvent<EstablishedShutdownResolved<W::Protocol>, Here> for ProxyEvent<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn recover(event: Self) -> Result<EstablishedShutdownResolved<W::Protocol>, Self> {
        match event {
            Self::WorkerShutdownResolved(shutdown) => Ok(shutdown),
            other => Err(other),
        }
    }
}
