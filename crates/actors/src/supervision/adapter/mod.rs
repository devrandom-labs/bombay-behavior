//! Behavior adapters that translate supervision decisions into typed actions.

mod proxy;
mod supervisor;

pub use proxy::{Proxy, ProxySends, ProxySendsWithParent, ProxyWithParent};
pub use supervisor::{
    ChildTopology, RestartConfiguration, Supervisor, SupervisorError, SupervisorSends,
    SupervisorWithParent,
};
