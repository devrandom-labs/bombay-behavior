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

pub use deadline::{Deadline, DeadlineReaction};
pub use event::{TimedEvent, TimedReaction};
pub use lease::{
    Lease, LeaseMessage, LeaseOutcome, LeaseRejection, LeaseRequest, LeaseSends, LeaseState,
};
pub use one_shot::OneShot;
pub use periodic::Periodic;
pub use receive_timeout::ReceiveTimeout;
