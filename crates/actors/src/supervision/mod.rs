mod adapter;
mod backoff;
mod domain;
mod dynamic_supervisor;
mod fixed_supervisor;
mod policy;
mod protocol;

pub use adapter::{
    ChildTopology, Proxy, ProxyError, ProxySends, RestartConfiguration, RestartTiming, Supervise,
    SuperviseError, SupervisorSends,
};
pub use backoff::{Backoff, BackoffConfigError, BackoffError};
pub(crate) use domain::{FixedFleetOwnership, OwnershipError, OwnershipFold};
pub use domain::{FleetError, IncarnationError as ProxyLifecycleError, IncarnationPhase};
pub use dynamic_supervisor::{
    DynamicChildPhase, DynamicSupervisor, DynamicSupervisorError, DynamicSupervisorEvent,
    DynamicSupervisorMessage, DynamicSupervisorOutcome, DynamicSupervisorProtocol,
    DynamicSupervisorRejection, DynamicSupervisorSends,
};
pub use fixed_supervisor::Supervisor;
pub use policy::{
    ReportSupervisionFailure, RestartPolicy, Strategy, SupervisionFailure,
    SupervisionFailureReaction, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use protocol::{ProxyEvent, ProxyUnavailable, SupervisionEvent, SupervisionLifecycle};
