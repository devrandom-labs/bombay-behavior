//! Pure supervision state machines and bookkeeping domains.

mod fleet;
mod incarnation;
mod ownership;
mod restart_budget;

pub use fleet::FleetError;
pub(super) use fleet::{Fleet, SlotRegistrationError};
pub use incarnation::IncarnationError;
pub use incarnation::IncarnationPhase;
pub(super) use incarnation::{
    Incarnation, IncarnationEffects, IncarnationShutdownError, IncarnationStopEffects,
    IncarnationStopError,
};
pub(crate) use ownership::{
    FixedFleetOwnership, OwnershipError, OwnershipFold, WorkerCommandRejection,
};
pub(super) use restart_budget::RestartBudget;
