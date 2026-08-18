//! Pure time domains and behavior adapters.
//!
//! Timer identity is the product `(TimerId, TimerGeneration)`. Nested adapters
//! that reuse that complete identity intentionally share one lane, owned by
//! the outermost matching adapter. Independent timers must use distinct IDs;
//! this is a Bombay composition policy rather than an actor-model law.

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
