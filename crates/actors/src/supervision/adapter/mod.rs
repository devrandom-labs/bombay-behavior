//! Behavior adapters that translate supervision decisions into typed actions.

mod proxy;
mod supervisor;

pub use proxy::{Proxy, ProxyError, ProxySends};
pub(crate) use supervisor::map_ownership_error;
pub use supervisor::{
    ChildTopology, RestartConfiguration, RestartTiming, Supervise, SuperviseError, SupervisorSends,
};
