//! Closed StableProxy lifecycle states and the values owned by each phase.

use behavior::{Behavior, BehaviorAddr, CreateChild, EndpointAddress};

use crate::{ChildStopped, EstablishedShutdownResolved, ShutdownId, StopOnShutdown};

use super::WorkerStopping;
use super::worker::PendingWorker;
use super::{
    CurrentWorker, ProxyPhase, StartWorker, StartupPermit, StartupPlan, StartupRejection,
    StoppedWorker, WorkerAttempt, WorkerInitializationFailure, WorkerStartResult,
    WorkerStartupGrant,
};

pub(super) enum PreReadyFailure<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    InitializationEffects {
        failure: WorkerInitializationFailure,
        startup: P,
    },
    StartupRefused {
        request: StartWorker<W, P>,
        reason: StartupRejection,
    },
    StartupRejected {
        rejection: P::Rejection,
    },
}

pub(super) enum WorkerStartKind {
    Initial,
    Replacement {
        replaces: WorkerAttempt,
        predecessor_shutdown: PredecessorShutdown,
    },
}

pub(super) enum PredecessorShutdown {
    Settled,
    Awaiting { id: ShutdownId },
}

pub(super) enum ReplacementCompletion<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Ready {
        worker: CurrentWorker<W>,
        result: WorkerStartResult<W, P>,
    },
    ReadyAfterStop {
        worker: WorkerAttempt,
        result: WorkerStartResult<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    Empty {
        worker: WorkerAttempt,
        result: WorkerStartResult<W, P>,
    },
}

pub(super) enum ProxyReplacement<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    ReturningPredecessor {
        departure: WorkerStopping<W>,
        successor: PendingWorker<P>,
        creation: CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
    },
    SuccessorResultAwaitingShutdown {
        replaces: WorkerAttempt,
        shutdown: ShutdownId,
        completion: ReplacementCompletion<W, P>,
    },
}

pub(super) enum StartupProgress<W, P> {
    WaitingForStart(WorkerStartupGrant<W, P>),
    Running(WorkerStartupGrant<W, P>),
}

pub(super) enum StartupDuringDeparture<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Pending(StartupProgress<W, P>),
    Returned(WorkerStartupRetirement<W, P>),
}

pub(super) enum WorkerStartupShutdown<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Departing {
        startup: StartupDuringDeparture<W, P>,
        departure: WorkerStopping<W>,
    },
    WaitingForStartup {
        startup: StartupProgress<W, P>,
        worker: CurrentWorker<W>,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

pub(super) enum WorkerStartPhase<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Creating {
        worker: PendingWorker<P>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Initializing {
        worker: CurrentWorker<W>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Starting {
        worker: CurrentWorker<W>,
        progress: StartupProgress<W, P>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    ReturningWorker {
        departure: WorkerStopping<W>,
        failure: PreReadyFailure<W, P>,
    },
}

pub(super) struct WorkerStart<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(super) kind: WorkerStartKind,
    pub(super) phase: WorkerStartPhase<W, P>,
}

pub(super) enum WorkerInitializationShutdown<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Departing {
        initialization: Option<WorkerInitializationRetirement<W, P>>,
        departure: WorkerStopping<W>,
    },
    WaitingForInitialization {
        worker: CurrentWorker<W>,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

#[expect(
    dead_code,
    reason = "Bombay retirement custody moves the complete stopped StableProxy"
)]
pub(super) enum WorkerInitializationRetirement<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Initialized {
        permit: StartupPermit<W>,
        startup: P,
    },
    EffectsRejected {
        failure: WorkerInitializationFailure,
        startup: P,
    },
    Stopped {
        startup: P,
        initialization_stop: Option<ChildStopped<BehaviorAddr<W>>>,
    },
}

#[expect(
    dead_code,
    reason = "Bombay retirement custody moves the complete stopped StableProxy"
)]
pub(super) enum WorkerStartupRetirement<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    StartRejected {
        request: StartWorker<W, P>,
        reason: StartupRejection,
    },
    Ready {
        readiness: P::Ready,
    },
    Rejected {
        rejection: P::Rejection,
    },
}

#[expect(
    dead_code,
    reason = "Bombay retirement custody moves the complete stopped StableProxy"
)]
pub(super) enum WorkerStartRetirement<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Result(WorkerStartResult<W, P>),
    Initialization {
        initialization: WorkerInitializationRetirement<W, P>,
        worker: CurrentWorker<W>,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    Startup {
        startup: WorkerStartupRetirement<W, P>,
        worker: CurrentWorker<W>,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    ReadyWorker {
        result: WorkerStartResult<W, P>,
        departure: StoppedWorker<W>,
    },
    ReplacementCompletion(ReplacementCompletion<W, P>),
}

#[expect(
    dead_code,
    reason = "Bombay retirement custody moves the complete stopped StableProxy"
)]
pub(super) enum ProxyRetirement<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Empty {
        previous: Option<WorkerAttempt>,
    },
    ReplacementCancelled {
        replaces: WorkerAttempt,
        successor: WorkerAttempt,
        predecessor: StoppedWorker<W>,
    },
    Worker(StoppedWorker<W>),
    WorkerStart {
        replaces: Option<WorkerAttempt>,
        retirement: WorkerStartRetirement<W, P>,
    },
    ReplacementAfterPredecessor {
        replaces: WorkerAttempt,
        predecessor: EstablishedShutdownResolved<W::Protocol>,
        retirement: WorkerStartRetirement<W, P>,
    },
}

pub(super) enum ProxyShutdown<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    WaitingForCreation(WorkerStart<W, P>),
    ReturningWorker(WorkerStopping<W>),
    ReturningWorkerStart {
        kind: WorkerStartKind,
        departure: WorkerStopping<W>,
        failure: PreReadyFailure<W, P>,
    },
    ReturningPredecessor {
        departure: WorkerStopping<W>,
        replaces: WorkerAttempt,
        successor: WorkerAttempt,
    },
    ReturningSuccessor {
        replaces: WorkerAttempt,
        predecessor: PredecessorReturn<W>,
        departure: WorkerStopping<W>,
        result: WorkerStartResult<W, P>,
    },
    Initializing {
        kind: WorkerStartKind,
        initialization: WorkerInitializationShutdown<W, P>,
    },
    Starting {
        kind: WorkerStartKind,
        startup: WorkerStartupShutdown<W, P>,
    },
    WaitingForPredecessor {
        replaces: WorkerAttempt,
        shutdown: ShutdownId,
        retirement: WorkerStartRetirement<W, P>,
    },
}

pub(super) enum PredecessorReturn<W>
where
    W: Behavior,
{
    Awaiting {
        shutdown: ShutdownId,
    },
    Returned {
        resolution: EstablishedShutdownResolved<W::Protocol>,
    },
}

pub(super) enum ProxyState<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Dormant,
    Starting(WorkerStart<W, P>),
    Ready { worker: CurrentWorker<W> },
    EmptyInitial,
    EmptyAfter { previous: WorkerAttempt },
    Replacing(ProxyReplacement<W, P>),
    ShuttingDown(ProxyShutdown<W, P>),
    Stopped(ProxyRetirement<W, P>),
}

impl<W, P> ProxyState<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(super) const fn phase(&self) -> ProxyPhase {
        match self {
            Self::Dormant => ProxyPhase::Dormant,
            Self::Starting(start) => match &start.phase {
                WorkerStartPhase::Creating { .. } => ProxyPhase::Creating,
                WorkerStartPhase::Initializing { .. } => ProxyPhase::Initializing,
                WorkerStartPhase::Starting { .. } => ProxyPhase::Starting,
                WorkerStartPhase::ReturningWorker { .. } => ProxyPhase::ReturningWorker,
            },
            Self::Ready { .. } => ProxyPhase::Ready,
            Self::EmptyInitial => ProxyPhase::EmptyInitial,
            Self::EmptyAfter { .. } => ProxyPhase::EmptyAfter,
            Self::Replacing(_) => ProxyPhase::Replacing,
            Self::ShuttingDown(_) => ProxyPhase::ShuttingDown,
            Self::Stopped(_) => ProxyPhase::Stopped,
        }
    }
}

#[cfg(test)]
mod direct_stop_presence_contract {
    use super::WorkerStartPhase;
    use super::{StartupPlan, WorkerInitializationRetirement, WorkerInitializationShutdown};
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    #[expect(dead_code, reason = "compile contract for the private state product")]
    fn creation<W, P>(phase: WorkerStartPhase<W, P>)
    where
        W: Behavior,
        P: StartupPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        if let WorkerStartPhase::Creating {
            worker: _,
            stopped: _,
        } = phase
        {}
    }

    #[expect(dead_code, reason = "compile contract for the private state product")]
    fn initialization<W, P>(phase: WorkerStartPhase<W, P>)
    where
        W: Behavior,
        P: StartupPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        if let WorkerStartPhase::Initializing {
            worker: _,
            stopped: _,
        } = phase
        {}
    }

    #[expect(dead_code, reason = "compile contract for exact optional return")]
    fn shutdown<W, P>(shutdown: WorkerInitializationShutdown<W, P>)
    where
        W: Behavior,
        P: StartupPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        if let WorkerInitializationShutdown::Departing { initialization, .. } = shutdown {
            let _: Option<WorkerInitializationRetirement<W, P>> = initialization;
        }
    }

    #[expect(dead_code, reason = "compile contract for the private state product")]
    fn startup<W, P>(phase: WorkerStartPhase<W, P>)
    where
        W: Behavior,
        P: StartupPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        if let WorkerStartPhase::Starting {
            worker: _,
            progress: _,
            stopped: _,
        } = phase
        {}
    }
}

#[cfg(test)]
mod terminal_ownership_contract {
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    use crate::{ChildStopped, EstablishedShutdownResolved};

    use super::{StartupPlan, WorkerInitializationRetirement, WorkerStartRetirement};

    #[expect(
        dead_code,
        reason = "compile contract for initialization stop ownership"
    )]
    fn initialization_stop<W, P>(retired: WorkerInitializationRetirement<W, P>)
    where
        W: Behavior,
        P: StartupPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        if let WorkerInitializationRetirement::Stopped {
            startup: _,
            initialization_stop,
        } = retired
        {
            let _: Option<ChildStopped<BehaviorAddr<W>>> = initialization_stop;
        }
    }

    #[expect(dead_code, reason = "compile contract for terminal worker ownership")]
    fn shutdown<W, P>(
        retired: WorkerStartRetirement<W, P>,
    ) -> Option<EstablishedShutdownResolved<W::Protocol>>
    where
        W: Behavior,
        P: StartupPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        match retired {
            WorkerStartRetirement::Initialization { shutdown, .. }
            | WorkerStartRetirement::Startup { shutdown, .. } => shutdown,
            _ => None,
        }
    }
}
