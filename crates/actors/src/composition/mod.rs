//! Fundamental concrete behavior compositions.
//!
//! This module is the stable semantic home used by the Bombay façade. The
//! implementations remain split by construction so finite-state transitions,
//! initialization composition, forwarding, and deferred delivery do not form
//! a dumping ground.

mod delivery_route;
mod message_adapter;
mod recipes;

pub use crate::activation::{Activate, Active, Initialized};
pub use crate::lifecycle::{CoordinatedTerminalApplication, coordinated_terminal_application};
pub use crate::machine::{Machine, Move};
pub use crate::stash::{Stash, StashRoute, StashStatus};
pub use delivery_route::DeliveryRoute;
pub use message_adapter::{MessageAdapter, MessageAdapterWithRoute};
pub use recipes::{supervised_backoff, supervised_backoff_with_parent};
