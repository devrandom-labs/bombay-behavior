//! Behavior adapters that translate supervision decisions into typed actions.

mod proxy;
mod supervisor;

pub use proxy::{Proxy, ProxyError, ProxySends};
pub use supervisor::{Supervisor, SupervisorError, SupervisorSends};
