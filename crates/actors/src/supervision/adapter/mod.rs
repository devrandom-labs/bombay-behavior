//! Behavior adapters that translate supervision decisions into typed actions.

mod proxy;
mod supervisor;

pub(crate) enum StableProxyChildRole {}
pub(crate) enum WorkerIncarnationChildRole {}

pub use proxy::{Proxy, ProxySends, ProxySendsWithParent, ProxyWithParent};
pub use supervisor::{
    ChildTopology, RestartConfiguration, Supervise, SuperviseError, SuperviseWithParent,
    SupervisorSends,
};
