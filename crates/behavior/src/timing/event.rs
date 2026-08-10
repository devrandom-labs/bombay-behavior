//! The event coproduct shared by timer-based behavior compositions.

use crate::behavior::{User, UserEvent};
use crate::protocol::forward::forward_event_lane;
use crate::protocol::{TimeEvent, TimerElapsed};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimedEvent<E> {
    Inner(E),
    Elapsed(TimerElapsed),
}

impl<E: UserEvent> TimeEvent for TimedEvent<E> {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        Some(Self::Elapsed(event))
    }
}

impl<E: UserEvent> crate::EventInput<TimerElapsed> for TimedEvent<E> {
    fn inject(event: TimerElapsed) -> Self {
        Self::Elapsed(event)
    }
}

impl<E: UserEvent> UserEvent for TimedEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            elapsed @ Self::Elapsed(_) => Err(elapsed),
        }
    }
}

forward_event_lane!(
    TimedEvent,
    PeerEvent,
    peer_stopped,
    crate::PeerStopped<E::Addr>
);
forward_event_lane!(
    TimedEvent,
    ChildEvent,
    child_stopped,
    crate::ChildStopped<E::Addr>
);
forward_event_lane!(
    TimedEvent,
    WorkerEvent,
    worker_stopped,
    crate::WorkerStopped<E::Addr>
);
forward_event_lane!(
    TimedEvent,
    CreationEvent,
    creation_resolved,
    crate::CreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    TimedEvent,
    WorkerCreationEvent,
    worker_creation_resolved,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    TimedEvent,
    ShutdownEvent,
    shutdown_requested,
    crate::ShutdownRequested
);
