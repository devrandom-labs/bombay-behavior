//! Pure supervisors, pools, proxies and their shared worker contracts.

mod capacity;
pub mod diagnostic;
mod drain;
pub mod pool;
pub mod proxy;
mod proxy_creation;
mod restart;
mod roster;
mod schedule;
pub mod supervisor;
pub mod worker;

pub use capacity::ZeroCapacity;
pub use drain::ActorDrainPolicy;
