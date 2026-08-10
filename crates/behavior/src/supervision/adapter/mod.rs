//! Behavior adapters that translate supervision decisions into typed actions.

mod proxy;
mod supervisor;

pub use proxy::{Proxy, ProxyActions, ProxySends};
pub use supervisor::{Supervising, SupervisorActions, SupervisorSends};
