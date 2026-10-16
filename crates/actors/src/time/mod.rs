//! Pure time domains and behavior adapters.
//!
//! Ordinary OneShot, Periodic, Deadline and ReceiveTimeout wrappers own their
//! exact event ingress and generation; callers supply no timer name. Explicit
//! named requests share an actor-wide replacement namespace, even across
//! different event ports. This is deliberate Bombay composition policy. Both
//! forms remain pure requests interpreted by the actor's local clock service.

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
