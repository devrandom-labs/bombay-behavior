//! Pure supervision state machines and bookkeeping domains.

mod fleet;
mod incarnation;
mod ownership;
mod restart_budget;

pub use fleet::FleetError;
pub use incarnation::IncarnationError;
pub use incarnation::IncarnationPhase;
pub(super) use incarnation::{
    Incarnation, IncarnationEffects, IncarnationShutdownError, IncarnationStopEffects,
    IncarnationStopError,
};
pub(crate) use ownership::{FixedFleetOwnership, OwnershipError, OwnershipFold};
pub(super) use restart_budget::RestartBudget;
