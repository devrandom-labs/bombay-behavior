//! Closed FixedSupervisor application and child-runtime inputs.

use crate::composition::replies::ReplyRoutes;

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, Births, ChildReport, CreationsSettled,
    EndpointAddress, EventIngress, InjectEvent, RecoverEvent, User, UserEvent,
};

use crate::proxy::ProxyDiagnostic;
use crate::{
    ChildStopped, ProxyInputResult, ProxyOutcome, ScheduleAfter, StableProxy, TimerElapsed,
};

use super::{FixedCommand, PrepareWorkers, StartupPlan, WorkerPreparation, WorkerSource};

/// Closed application and child-runtime input sum for one fixed supervisor.
#[doc(hidden)]
pub enum FixedSupervisorEvent<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Command(
        User<
            BehaviorAddr<Worker>,
            FixedCommand<
                BehaviorAddr<Worker>,
                Role,
                Worker::Protocol,
                StatusReplies,
                CapabilityReplies,
            >,
        >,
    ),
    ProxyCreationsSettled(CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>),
    ProxyInputSettled(ProxyInputResult<behavior::Here, Worker, Plan>),
    ProxyReported(ChildReport<ProxyOutcome<Worker, Plan>>),
    ProxyDiagnosed(ChildReport<ProxyDiagnostic<Worker, Plan>>),
    ProxyStopped(ChildStopped<BehaviorAddr<Worker>>),
    WorkerPreparationStarted(PreparationStart),
    WorkerPreparationReturned(PreparationReturn),
    RestartScheduleSettled(ActionItemResult<ScheduleAfter>),
    RestartElapsed(TimerElapsed),
}

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> EventIngress<ScheduleAfter, ActionItemResult<ScheduleAfter>>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ActionItemResult<ScheduleAfter>) -> Self {
        Self::RestartScheduleSettled(input)
    }
}

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> InjectEvent<TimerElapsed, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(input: TimerElapsed) -> Self {
        Self::RestartElapsed(input)
    }
}

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> RecoverEvent<TimerElapsed, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> EventIngress<Births<StableProxy<Worker, Plan>>, ChildReport<ProxyOutcome<Worker, Plan>>>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> EventIngress<Births<StableProxy<Worker, Plan>>, ChildReport<ProxyDiagnostic<Worker, Plan>>>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> InjectEvent<ChildReport<ProxyDiagnostic<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> RecoverEvent<ChildReport<ProxyDiagnostic<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> InjectEvent<ChildReport<ProxyOutcome<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> RecoverEvent<ChildReport<ProxyOutcome<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> EventIngress<behavior::Here, ProxyInputResult<behavior::Here, Worker, Plan>>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> InjectEvent<ProxyInputResult<behavior::Here, Worker, Plan>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<Role, Worker, Plan, Source, StatusReplies: ReplyRoutes, CapabilityReplies: ReplyRoutes>
    EventIngress<
        PrepareWorkers<Source, Role, Worker, Plan>,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    >
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        WorkerPreparation<Source, Role, Worker, Plan>,
        StatusReplies,
        CapabilityReplies,
    >
where
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn ingress(input: ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>) -> Self {
        Self::WorkerPreparationStarted(input)
    }
}

impl<Role, Worker, Plan, Source, StatusReplies: ReplyRoutes, CapabilityReplies: ReplyRoutes>
    InjectEvent<WorkerPreparation<Source, Role, Worker, Plan>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        WorkerPreparation<Source, Role, Worker, Plan>,
        StatusReplies,
        CapabilityReplies,
    >
where
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn inject_at(returned: WorkerPreparation<Source, Role, Worker, Plan>) -> Self {
        Self::WorkerPreparationReturned(returned)
    }
}

impl<Role, Worker, Plan, Source, StatusReplies: ReplyRoutes, CapabilityReplies: ReplyRoutes>
    RecoverEvent<WorkerPreparation<Source, Role, Worker, Plan>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        WorkerPreparation<Source, Role, Worker, Plan>,
        StatusReplies,
        CapabilityReplies,
    >
where
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn recover(event: Self) -> Result<WorkerPreparation<Source, Role, Worker, Plan>, Self> {
        match event {
            Self::WorkerPreparationReturned(returned) => Ok(returned),
            event => Err(event),
        }
    }
}

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> UserEvent
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Addr = BehaviorAddr<Worker>;
    type Message = FixedCommand<
        BehaviorAddr<Worker>,
        Role,
        Worker::Protocol,
        StatusReplies,
        CapabilityReplies,
    >;

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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
>
    EventIngress<
        Births<StableProxy<Worker, Plan>>,
        CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    >
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> InjectEvent<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> RecoverEvent<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> InjectEvent<ChildStopped<BehaviorAddr<Worker>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
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

impl<
    Role,
    Worker,
    Plan,
    PreparationStart,
    PreparationReturn,
    StatusReplies: ReplyRoutes,
    CapabilityReplies: ReplyRoutes,
> RecoverEvent<ChildStopped<BehaviorAddr<Worker>>, behavior::Here>
    for FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        PreparationStart,
        PreparationReturn,
        StatusReplies,
        CapabilityReplies,
    >
where
    Worker: Behavior,
    Plan: StartupPlan,
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
