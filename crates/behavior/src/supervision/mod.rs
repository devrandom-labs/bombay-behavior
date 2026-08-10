mod adapter;
mod domain;
mod policy;
mod protocol;

pub use adapter::{
    Proxy, ProxyActions, ProxySends, Supervising, SupervisorActions, SupervisorSends,
};
pub use domain::IncarnationPhase;
pub use policy::{
    RestartPolicy, Strategy, SupervisionFailure, SupervisionFailureReaction, restart_all,
    restart_one, restart_rest, retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use protocol::{ProxyCommand, ProxyEvent, SupervisionEvent};
