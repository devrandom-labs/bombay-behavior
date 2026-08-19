mod adapter;
mod backoff;
mod backoff_supervisor;
mod domain;
mod dynamic_supervisor;
mod fixed_supervisor;
mod policy;
mod protocol;

pub use adapter::{
    ChildTopology, Proxy, ProxySends, ProxySendsWithParent, ProxyWithParent, RestartConfiguration,
    Supervise, SuperviseError, SuperviseWithParent, SupervisorSends,
};
pub use backoff::{Backoff, BackoffConfigError, BackoffError};
pub use backoff_supervisor::{
    BackoffSupervise, BackoffSupervisor, BackoffSupervisorError, BackoffSupervisorEvent,
    BackoffSupervisorSends,
};
pub(crate) use domain::{FixedFleetOwnership, OwnershipError};
pub use domain::{FleetError, IncarnationError as ProxyError, IncarnationPhase};
pub use dynamic_supervisor::{
    DynamicChildPhase, DynamicProxy, DynamicProxyWithParent, DynamicSupervisor,
    DynamicSupervisorError, DynamicSupervisorEvent, DynamicSupervisorMessage,
    DynamicSupervisorOutcome, DynamicSupervisorRejection, DynamicSupervisorSends,
    DynamicSupervisorWithParent,
};
pub use fixed_supervisor::{
    Supervisor, SupervisorError, SupervisorEvent, SupervisorProtocol, SupervisorWithParent,
    TopologyFailurePolicy,
};
pub use policy::{
    ReportSupervisionFailure, RestartPolicy, Strategy, SupervisionFailure,
    SupervisionFailureReaction, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use protocol::{ProxyCommand, ProxyEvent, SupervisionEvent};
