//! Pure time domains and behavior adapters.
//!
//! Timer identity is the product `(TimerId, TimerGeneration)` within one
//! scheduling owner. Nested adapters remain distinct owners even when those
//! values are equal: each schedule request carries its structural ingress
//! destination. This is a Bombay composition policy rather than an actor-model
//! law.

mod deadline;
mod domain;
mod event;
mod lease;
mod one_shot;
mod periodic;
mod receive_timeout;

pub use deadline::{Deadline, DeadlineEvent, DeadlineReaction};
pub use event::TimedEvent;
pub use lease::{Lease, LeaseMessage, LeaseOutcome, LeaseRejection, LeaseSends, LeaseState};
pub use one_shot::{OneShot, OneShotEvent, OneShotReaction};
pub use periodic::{Periodic, PeriodicEvent, PeriodicReaction};
pub use receive_timeout::{ReceiveTimeout, ReceiveTimeoutEvent, ReceiveTimeoutReaction};
