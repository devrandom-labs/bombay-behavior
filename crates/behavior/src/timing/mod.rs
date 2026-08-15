//! Pure time domains and behavior adapters.

mod deadline;
mod domain;
mod event;
mod receive_timeout;

pub use deadline::{Deadline, DeadlineEvent, DeadlineReaction, DeadlineSends};
pub use event::TimedEvent;
pub use receive_timeout::{
    ReceiveTimeout, ReceiveTimeoutEvent, ReceiveTimeoutReaction, ReceiveTimeoutSends,
};
