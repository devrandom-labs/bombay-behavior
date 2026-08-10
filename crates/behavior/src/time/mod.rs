//! Pure time domains and behavior adapters.

mod deadline;
mod domain;
mod event;
mod receive_timeout;

pub use deadline::{At, AtActions, AtEvent, AtReaction, AtSends};
pub use event::TimedEvent;
pub use receive_timeout::{
    ReceiveTimeout, ReceiveTimeoutActions, ReceiveTimeoutError, ReceiveTimeoutEvent,
    ReceiveTimeoutReaction, ReceiveTimeoutSends,
};
