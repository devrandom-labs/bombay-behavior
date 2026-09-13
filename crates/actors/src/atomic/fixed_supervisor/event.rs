//! Closed FixedSupervisor application and child-runtime inputs.

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, Births, ChildReport, CreationsSettled,
    EndpointAddress, EventIngress, InjectEvent, RecoverEvent, User, UserEvent,
};

use crate::{
    ChildStopped, ProxyInputResult, ProxyOutcome, ScheduleAfter, StableProxy, TimerElapsed,
};

use super::{ActivationPlan, FixedCommand, PrepareWorkers, WorkerSource};

/// Closed application and child-runtime input sum for one fixed supervisor.
#[doc(hidden)]
pub enum FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Command(User<BehaviorAddr<Worker>, FixedCommand<BehaviorAddr<Worker>, Role, Worker::Protocol>>),
    ProxyCreationsSettled(CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>),
    ProxyInputSettled(ProxyInputResult<behavior::Here, Worker, Plan>),
    ProxyReported(ChildReport<ProxyOutcome<Worker, Plan>>),
    ProxyStopped(ChildStopped<BehaviorAddr<Worker>>),
    WorkerPreparationSettled(Preparation),
    RestartScheduleSettled(ActionItemResult<ScheduleAfter>),
    RestartElapsed(TimerElapsed),
}

impl<Role, Worker, Plan, Preparation> EventIngress<ScheduleAfter, ActionItemResult<ScheduleAfter>>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ActionItemResult<ScheduleAfter>) -> Self {
        Self::RestartScheduleSettled(input)
    }
}

impl<Role, Worker, Plan, Preparation> InjectEvent<TimerElapsed, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: TimerElapsed) -> Self {
        Self::RestartElapsed(input)
    }
}

impl<Role, Worker, Plan, Preparation> RecoverEvent<TimerElapsed, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(event: Self) -> Result<TimerElapsed, Self> {
        match event {
            Self::RestartElapsed(elapsed) => Ok(elapsed),
            input => Err(input),
        }
    }
}

impl<Role, Worker, Plan, Preparation>
    EventIngress<Births<StableProxy<Worker, Plan>>, ChildReport<ProxyOutcome<Worker, Plan>>>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ChildReport<ProxyOutcome<Worker, Plan>>) -> Self {
        Self::ProxyReported(input)
    }
}

impl<Role, Worker, Plan, Preparation>
    InjectEvent<ChildReport<ProxyOutcome<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ChildReport<ProxyOutcome<Worker, Plan>>) -> Self {
        Self::ProxyReported(input)
    }
}

impl<Role, Worker, Plan, Preparation>
    RecoverEvent<ChildReport<ProxyOutcome<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(event: Self) -> Result<ChildReport<ProxyOutcome<Worker, Plan>>, Self> {
        match event {
            Self::ProxyReported(report) => Ok(report),
            input => Err(input),
        }
    }
}

impl<Role, Worker, Plan, Preparation>
    EventIngress<behavior::Here, ProxyInputResult<behavior::Here, Worker, Plan>>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ProxyInputResult<behavior::Here, Worker, Plan>) -> Self {
        Self::ProxyInputSettled(input)
    }
}

impl<Role, Worker, Plan, Preparation>
    InjectEvent<ProxyInputResult<behavior::Here, Worker, Plan>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ProxyInputResult<behavior::Here, Worker, Plan>) -> Self {
        Self::ProxyInputSettled(input)
    }
}

impl<Role, Worker, Plan, Source>
    EventIngress<
        PrepareWorkers<Source, Role, Worker, Plan>,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    >
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    >
where
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>) -> Self {
        Self::WorkerPreparationSettled(input)
    }
}

impl<Role, Worker, Plan, Preparation> UserEvent
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Addr = BehaviorAddr<Worker>;
    type Message = FixedCommand<BehaviorAddr<Worker>, Role, Worker::Protocol>;

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

impl<Role, Worker, Plan, Preparation>
    EventIngress<
        Births<StableProxy<Worker, Plan>>,
        CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    > for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>) -> Self {
        Self::ProxyCreationsSettled(input)
    }
}

impl<Role, Worker, Plan, Preparation>
    InjectEvent<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>) -> Self {
        Self::ProxyCreationsSettled(input)
    }
}

impl<Role, Worker, Plan, Preparation>
    RecoverEvent<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(
        event: Self,
    ) -> Result<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>, Self> {
        match event {
            Self::ProxyCreationsSettled(proxies) => Ok(proxies),
            input => Err(input),
        }
    }
}

impl<Role, Worker, Plan, Preparation>
    InjectEvent<ChildStopped<BehaviorAddr<Worker>>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ChildStopped<BehaviorAddr<Worker>>) -> Self {
        Self::ProxyStopped(input)
    }
}

impl<Role, Worker, Plan, Preparation>
    RecoverEvent<ChildStopped<BehaviorAddr<Worker>>, behavior::Here>
    for FixedSupervisorEvent<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(event: Self) -> Result<ChildStopped<BehaviorAddr<Worker>>, Self> {
        match event {
            Self::ProxyStopped(stopped) => Ok(stopped),
            input => Err(input),
        }
    }
}
