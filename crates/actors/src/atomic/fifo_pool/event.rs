//! Closed application and worker-runtime inputs for one FIFO pool.

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, Births, ChildReport, CreationsSettled,
    EndpointAddress, EventIngress, InjectEvent, Protocol, RecoverEvent, User, UserEvent,
};

use crate::{
    ChildStopped, EstablishedShutdownResolved, ScheduleAfter, ShutdownRequested, StopOnShutdown,
    TimerElapsed,
};

use super::super::{ActivationPlan, WorkerActivation, WorkerInitializationReport};
use super::super::{PrepareWorkers, WorkerPreparation, WorkerSource};
use super::{AssignWorker, Completion, FifoCommand};

/// Every typed input accepted by one FIFO pool.
#[doc(hidden)]
pub enum FifoEvent<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
where
    W: Behavior,
    W::Protocol: Protocol<Msg = super::Assignment<Job>>,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    Command(User<BehaviorAddr<W>, FifoCommand<BehaviorAddr<W>, Role, Job, WorkerResult>>),
    WorkerCreationsSettled(CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>),
    WorkerInitialization(WorkerInitializationReport<W, P>),
    WorkerActivationReported(WorkerActivation<W, P>),
    WorkerStopped(ChildStopped<BehaviorAddr<W>>),
    WorkerCompleted(ChildReport<Completion<WorkerResult>>),
    AssignmentSettled(ActionItemResult<AssignWorker<W::Protocol, Job>>),
    WorkerPreparationStarted(PreparationStart),
    WorkerPreparationReturned(PreparationReturn),
    RestartScheduleSettled(ActionItemResult<ScheduleAfter>),
    RestartElapsed(TimerElapsed),
    WorkerShutdownSettled(EstablishedShutdownResolved<W::Protocol>),
    Shutdown(ShutdownRequested),
}

impl<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn> UserEvent
    for FifoEvent<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
where
    W: Behavior,
    W::Protocol: Protocol<Msg = super::Assignment<Job>>,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    type Addr = BehaviorAddr<W>;
    type Message = FifoCommand<BehaviorAddr<W>, Role, Job, WorkerResult>;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Command(command) => Ok(command),
            input => Err(input),
        }
    }
}

macro_rules! injected_fifo_input {
    ($input:ty, $variant:ident) => {
        impl<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
            InjectEvent<$input, behavior::Here>
            for FifoEvent<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
        where
            W: Behavior,
            W::Protocol: Protocol<Msg = super::Assignment<Job>>,
            P: ActivationPlan,
            BehaviorAddr<W>: EndpointAddress,
            <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
            Job: Send,
        {
            fn inject_at(input: $input) -> Self {
                Self::$variant(input)
            }
        }

        impl<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
            RecoverEvent<$input, behavior::Here>
            for FifoEvent<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
        where
            W: Behavior,
            W::Protocol: Protocol<Msg = super::Assignment<Job>>,
            P: ActivationPlan,
            BehaviorAddr<W>: EndpointAddress,
            <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
            Job: Send,
        {
            fn recover(event: Self) -> Result<$input, Self> {
                match event {
                    Self::$variant(input) => Ok(input),
                    input => Err(input),
                }
            }
        }
    };
}

macro_rules! returned_fifo_input {
    ($request:ty, $input:ty, $variant:ident) => {
        impl<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
            EventIngress<$request, $input>
            for FifoEvent<Role, W, P, Job, WorkerResult, PreparationStart, PreparationReturn>
        where
            W: Behavior,
            W::Protocol: Protocol<Msg = super::Assignment<Job>>,
            P: ActivationPlan,
            BehaviorAddr<W>: EndpointAddress,
            <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
            Job: Send,
        {
            fn ingress(input: $input) -> Self {
                Self::$variant(input)
            }
        }
    };
}

injected_fifo_input!(
    CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    WorkerCreationsSettled
);
injected_fifo_input!(WorkerInitializationReport<W, P>, WorkerInitialization);
injected_fifo_input!(WorkerActivation<W, P>, WorkerActivationReported);
injected_fifo_input!(ChildStopped<BehaviorAddr<W>>, WorkerStopped);
injected_fifo_input!(ChildReport<Completion<WorkerResult>>, WorkerCompleted);
injected_fifo_input!(TimerElapsed, RestartElapsed);
injected_fifo_input!(
    EstablishedShutdownResolved<W::Protocol>,
    WorkerShutdownSettled
);
injected_fifo_input!(ShutdownRequested, Shutdown);

returned_fifo_input!(
    Births<StopOnShutdown<W>>,
    CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    WorkerCreationsSettled
);
returned_fifo_input!(
    Births<StopOnShutdown<W>>,
    ChildReport<Completion<WorkerResult>>,
    WorkerCompleted
);
returned_fifo_input!(
    AssignWorker<W::Protocol, Job>,
    ActionItemResult<AssignWorker<W::Protocol, Job>>,
    AssignmentSettled
);
returned_fifo_input!(
    ScheduleAfter,
    ActionItemResult<ScheduleAfter>,
    RestartScheduleSettled
);

impl<Role, W, P, Source, Job, WorkerResult>
    EventIngress<
        PrepareWorkers<Source, Role, W, P>,
        ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    >
    for FifoEvent<
        Role,
        W,
        P,
        Job,
        WorkerResult,
        ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        WorkerPreparation<Source, Role, W, P>,
    >
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
    fn ingress(input: ActionItemResult<PrepareWorkers<Source, Role, W, P>>) -> Self {
        Self::WorkerPreparationStarted(input)
    }
}

impl<Role, W, P, Source, Job, WorkerResult>
    InjectEvent<WorkerPreparation<Source, Role, W, P>, behavior::Here>
    for FifoEvent<
        Role,
        W,
        P,
        Job,
        WorkerResult,
        ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        WorkerPreparation<Source, Role, W, P>,
    >
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
    fn inject_at(returned: WorkerPreparation<Source, Role, W, P>) -> Self {
        Self::WorkerPreparationReturned(returned)
    }
}

impl<Role, W, P, Source, Job, WorkerResult>
    RecoverEvent<WorkerPreparation<Source, Role, W, P>, behavior::Here>
    for FifoEvent<
        Role,
        W,
        P,
        Job,
        WorkerResult,
        ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        WorkerPreparation<Source, Role, W, P>,
    >
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
    fn recover(event: Self) -> Result<WorkerPreparation<Source, Role, W, P>, Self> {
        match event {
            Self::WorkerPreparationReturned(returned) => Ok(returned),
            event => Err(event),
        }
    }
}
