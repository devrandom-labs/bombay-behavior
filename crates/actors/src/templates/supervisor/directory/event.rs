//! Closed application and StableProxy inputs for one dynamic supervisor.

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, Births, ChildReport, CreationsSettled,
    EndpointAddress, EventIngress, InjectEvent, RecoverEvent, User, UserEvent,
};

use crate::proxy::ProxyDiagnostic;
use crate::{
    ChildStopped, ProxyInputResult, ProxyOutcome, ScheduleAfter, ShutdownRequested, StableProxy,
    StartupPlan, TimerElapsed,
};

use super::DynamicCommand;
use crate::composition::replies::ReplyRoutes;

/// Complete typed input sum for one dynamic supervisor.
#[doc(hidden)]
pub enum DynamicSupervisorEvent<Key, Worker, Plan, Replies: ReplyRoutes>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Command(User<BehaviorAddr<Worker>, DynamicCommand<Key, Worker, Plan, Replies>>),
    ProxyCreationsSettled(CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>),
    ProxyInputSettled(ProxyInputResult<behavior::Here, Worker, Plan>),
    ProxyReported(ChildReport<ProxyOutcome<Worker, Plan>>),
    ProxyDiagnosed(ChildReport<ProxyDiagnostic<Worker, Plan>>),
    ProxyStopped(ChildStopped<BehaviorAddr<Worker>>),
    Shutdown(ShutdownRequested),
    ShutdownScheduleSettled(ActionItemResult<ScheduleAfter>),
    ShutdownElapsed(TimerElapsed),
}

impl<Key, Worker, Plan, Replies: ReplyRoutes> InjectEvent<ShutdownRequested, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ShutdownRequested) -> Self {
        Self::Shutdown(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    EventIngress<ScheduleAfter, ActionItemResult<ScheduleAfter>>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ActionItemResult<ScheduleAfter>) -> Self {
        Self::ShutdownScheduleSettled(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes> InjectEvent<TimerElapsed, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: TimerElapsed) -> Self {
        Self::ShutdownElapsed(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes> RecoverEvent<TimerElapsed, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(event: Self) -> Result<TimerElapsed, Self> {
        match event {
            Self::ShutdownElapsed(elapsed) => Ok(elapsed),
            input => Err(input),
        }
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes> UserEvent
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Addr = BehaviorAddr<Worker>;
    type Message = DynamicCommand<Key, Worker, Plan, Replies>;

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

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    EventIngress<
        Births<StableProxy<Worker, Plan>>,
        CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    > for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>) -> Self {
        Self::ProxyCreationsSettled(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    InjectEvent<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>) -> Self {
        Self::ProxyCreationsSettled(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    EventIngress<behavior::Here, ProxyInputResult<behavior::Here, Worker, Plan>>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ProxyInputResult<behavior::Here, Worker, Plan>) -> Self {
        Self::ProxyInputSettled(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    InjectEvent<ProxyInputResult<behavior::Here, Worker, Plan>, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ProxyInputResult<behavior::Here, Worker, Plan>) -> Self {
        Self::ProxyInputSettled(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    EventIngress<Births<StableProxy<Worker, Plan>>, ChildReport<ProxyOutcome<Worker, Plan>>>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ChildReport<ProxyOutcome<Worker, Plan>>) -> Self {
        Self::ProxyReported(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    EventIngress<Births<StableProxy<Worker, Plan>>, ChildReport<ProxyDiagnostic<Worker, Plan>>>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ChildReport<ProxyDiagnostic<Worker, Plan>>) -> Self {
        Self::ProxyDiagnosed(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    InjectEvent<ChildReport<ProxyDiagnostic<Worker, Plan>>, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ChildReport<ProxyDiagnostic<Worker, Plan>>) -> Self {
        Self::ProxyDiagnosed(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    RecoverEvent<ChildReport<ProxyDiagnostic<Worker, Plan>>, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(event: Self) -> Result<ChildReport<ProxyDiagnostic<Worker, Plan>>, Self> {
        match event {
            Self::ProxyDiagnosed(report) => Ok(report),
            input => Err(input),
        }
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    InjectEvent<ChildReport<ProxyOutcome<Worker, Plan>>, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ChildReport<ProxyOutcome<Worker, Plan>>) -> Self {
        Self::ProxyReported(input)
    }
}

impl<Key, Worker, Plan, Replies: ReplyRoutes>
    InjectEvent<ChildStopped<BehaviorAddr<Worker>>, behavior::Here>
    for DynamicSupervisorEvent<Key, Worker, Plan, Replies>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: ChildStopped<BehaviorAddr<Worker>>) -> Self {
        Self::ProxyStopped(input)
    }
}
