//! Pure policies derived from durable or retained state.
//!
//! Mnesis owns event sourcing, journals, snapshots, projections, checkpoints,
//! sagas, and stores. This module contains only passive folds; Mnesis-backed
//! host templates are admitted here only when `mnesis-bombay` exposes a
//! complete typed adapter path.

mod cache;

pub use cache::{Cache, CacheConfigError, CacheEntry, CacheMessage, CacheResult, CacheState};
