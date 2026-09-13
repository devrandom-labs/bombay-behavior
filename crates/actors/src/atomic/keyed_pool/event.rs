//! Closed application and worker-runtime inputs for one keyed pool.

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, Births, ChildReport, CreationsSettled,
    EndpointAddress, EventIngress, InjectEvent, Protocol, RecoverEvent, User, UserEvent,
};

use crate::{
    ChildStopped, EstablishedShutdownResolved, ScheduleAfter, ShutdownRequested, StopOnShutdown,
    TimerElapsed,
};

use super::super::pool::{AssignWorker, Assignment, Completion};
use super::super::{
    ActivationPlan, PrepareWorkers, WorkerActivation, WorkerInitializationReport, WorkerSource,
};
use super::KeyedCommand;

/// Every typed input accepted by one keyed pool.
#[doc(hidden)]
pub enum KeyedEvent<Role, W, P, Key, Job, WorkerResult, Preparation>
where
    W: Behavior,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    Command(User<BehaviorAddr<W>, KeyedCommand<BehaviorAddr<W>, Key, Role, Job, WorkerResult>>),
    WorkerCreationsSettled(CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>),
    WorkerInitialization(WorkerInitializationReport<W, P>),
    WorkerActivationReported(WorkerActivation<W, P>),
    WorkerStopped(ChildStopped<BehaviorAddr<W>>),
    WorkerCompleted(ChildReport<Completion<WorkerResult>>),
    AssignmentSettled(ActionItemResult<AssignWorker<W::Protocol, Job>>),
    WorkerPreparationSettled(Preparation),
    RestartScheduleSettled(ActionItemResult<ScheduleAfter>),
    RestartElapsed(TimerElapsed),
    WorkerShutdownSettled(EstablishedShutdownResolved<W::Protocol>),
    Shutdown(ShutdownRequested),
}

impl<Role, W, P, Key, Job, WorkerResult, Preparation> UserEvent
    for KeyedEvent<Role, W, P, Key, Job, WorkerResult, Preparation>
where
    W: Behavior,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    type Addr = BehaviorAddr<W>;
    type Message = KeyedCommand<BehaviorAddr<W>, Key, Role, Job, WorkerResult>;

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

macro_rules! injected_keyed_input {
    ($input:ty, $variant:ident) => {
        impl<Role, W, P, Key, Job, WorkerResult, Preparation> InjectEvent<$input, behavior::Here>
            for KeyedEvent<Role, W, P, Key, Job, WorkerResult, Preparation>
        where
            W: Behavior,
            W::Protocol: Protocol<Msg = Assignment<Job>>,
            P: ActivationPlan,
            BehaviorAddr<W>: EndpointAddress,
            <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
            Job: Send,
        {
            fn inject_at(input: $input) -> Self {
                Self::$variant(input)
            }
        }

        impl<Role, W, P, Key, Job, WorkerResult, Preparation> RecoverEvent<$input, behavior::Here>
            for KeyedEvent<Role, W, P, Key, Job, WorkerResult, Preparation>
        where
            W: Behavior,
            W::Protocol: Protocol<Msg = Assignment<Job>>,
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

macro_rules! returned_keyed_input {
    ($request:ty, $input:ty, $variant:ident) => {
        impl<Role, W, P, Key, Job, WorkerResult, Preparation> EventIngress<$request, $input>
            for KeyedEvent<Role, W, P, Key, Job, WorkerResult, Preparation>
        where
            W: Behavior,
            W::Protocol: Protocol<Msg = Assignment<Job>>,
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

injected_keyed_input!(
    CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    WorkerCreationsSettled
);
injected_keyed_input!(WorkerInitializationReport<W, P>, WorkerInitialization);
injected_keyed_input!(WorkerActivation<W, P>, WorkerActivationReported);
injected_keyed_input!(ChildStopped<BehaviorAddr<W>>, WorkerStopped);
injected_keyed_input!(ChildReport<Completion<WorkerResult>>, WorkerCompleted);
injected_keyed_input!(TimerElapsed, RestartElapsed);
injected_keyed_input!(
    EstablishedShutdownResolved<W::Protocol>,
    WorkerShutdownSettled
);
injected_keyed_input!(ShutdownRequested, Shutdown);

returned_keyed_input!(
    Births<StopOnShutdown<W>>,
    CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    WorkerCreationsSettled
);
returned_keyed_input!(
    Births<StopOnShutdown<W>>,
    ChildReport<Completion<WorkerResult>>,
    WorkerCompleted
);
returned_keyed_input!(
    AssignWorker<W::Protocol, Job>,
    ActionItemResult<AssignWorker<W::Protocol, Job>>,
    AssignmentSettled
);
returned_keyed_input!(
    ScheduleAfter,
    ActionItemResult<ScheduleAfter>,
    RestartScheduleSettled
);

impl<Role, W, P, Source, Key, Job, WorkerResult>
    EventIngress<
        PrepareWorkers<Source, Role, W, P>,
        ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    >
    for KeyedEvent<
        Role,
        W,
        P,
        Key,
        Job,
        WorkerResult,
        ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    >
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    Job: Send,
{
    fn ingress(input: ActionItemResult<PrepareWorkers<Source, Role, W, P>>) -> Self {
        Self::WorkerPreparationSettled(input)
    }
}
