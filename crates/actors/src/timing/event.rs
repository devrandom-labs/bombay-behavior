//! The event coproduct shared by timer-based behavior compositions.

use crate::protocol::TimerElapsed;
use crate::protocol::forward::forward_event_lane;
use behavior::{User, UserEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimedEvent<E> {
    Behavior(E),
    Elapsed(TimerElapsed),
}

impl<E: UserEvent> crate::RouteInput<TimerElapsed> for TimedEvent<E> {
    fn route(event: TimerElapsed) -> Result<Self, TimerElapsed> {
        Ok(Self::Elapsed(event))
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
        Self::Behavior(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Behavior(event) => event.into_user().map_err(Self::Behavior),
            elapsed @ Self::Elapsed(_) => Err(elapsed),
        }
    }
}

forward_event_lane!(TimedEvent, crate::PeerStopped<E::Addr>);
forward_event_lane!(TimedEvent, crate::ChildStopped<E::Addr>);
forward_event_lane!(TimedEvent, crate::WorkerStopped<E::Addr>);
forward_event_lane!(TimedEvent, crate::CreationResolved<E::Addr>);
forward_event_lane!(
    TimedEvent,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(TimedEvent, crate::ShutdownRequested);
