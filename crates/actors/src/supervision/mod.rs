mod adapter;
mod domain;
mod policy;
mod protocol;

pub use adapter::{Proxy, ProxySends, Supervisor, SupervisorError, SupervisorSends};
pub use domain::{FleetError, IncarnationError as ProxyError, IncarnationPhase};
pub use policy::{
    ReportSupervisionFailure, RestartPolicy, Strategy, SupervisionFailure,
    SupervisionFailureReaction, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use protocol::{ProxyCommand, ProxyEvent, SupervisionEvent};
