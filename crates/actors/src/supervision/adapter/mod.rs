//! Behavior adapters that translate supervision decisions into typed actions.

mod proxy;
mod supervisor;

pub use proxy::{Proxy, ProxyError, ProxySends, ProxySendsWithParent, ProxyWithParent};
pub use supervisor::{
    ChildTopology, RestartConfiguration, Supervise, SuperviseError, SuperviseWithParent,
    SupervisorSends,
};
