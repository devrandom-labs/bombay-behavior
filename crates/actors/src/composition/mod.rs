//! Fundamental concrete behavior compositions.
//!
//! This module is the stable semantic home used by the Bombay façade. The
//! implementations remain split by construction so finite-state transitions,
//! initialization composition, forwarding, and deferred delivery do not form
//! a dumping ground.

pub use crate::compose::{Activate, Active, ChildrenResult, Compose, Initialized};
pub use crate::machine::{Machine, Move};
pub use crate::stash::{Stash, StashRoute, StashStatus};
