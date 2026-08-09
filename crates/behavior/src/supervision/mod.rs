mod policy;
mod protocol;
mod proxy;
mod supervisor;

pub use policy::{
    RestartPolicy, Strategy, SupervisionFailure, SupervisionFailureReaction, restart_all,
    restart_one, restart_rest, retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use protocol::{ProxyCommand, SupervisionEvent};
pub use proxy::Proxy;
pub use supervisor::Supervising;
