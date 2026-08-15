//! Pure time domains and behavior adapters.

mod deadline;
mod domain;
mod event;
mod lease;
mod one_shot;
mod periodic;
mod receive_timeout;

pub use deadline::{Deadline, DeadlineEvent, DeadlineReaction, DeadlineSends};
pub use event::TimedEvent;
pub use lease::{Lease, LeaseMessage, LeaseOutcome, LeaseRejection, LeaseSends, LeaseState};
pub use one_shot::{OneShot, OneShotEvent, OneShotReaction, OneShotSends};
pub use periodic::{Periodic, PeriodicEvent, PeriodicReaction, PeriodicSends};
pub use receive_timeout::{
    ReceiveTimeout, ReceiveTimeoutEvent, ReceiveTimeoutReaction, ReceiveTimeoutSends,
};
