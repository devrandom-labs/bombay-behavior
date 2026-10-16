//! Pure message holding, finite admission and ordered replay.

mod limit;
mod queue;

pub use limit::{Stash, StashSends};
pub use queue::{StashRoute, StashStatus, StaticallyInfallible, UnlimitedStash};
