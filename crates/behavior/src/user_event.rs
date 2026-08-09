//! The user-message lane and its composition contracts.

use crate::addressing::Address;
use crate::protocol::{
    ChildEvent, ChildStopped, PeerEvent, PeerStopped, ShutdownEvent, ShutdownRequested, TimeEvent,
    TimerElapsed, WorkerEvent, WorkerStopped,
};

/// The user-message event at the Agha floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User<A, M> {
    pub from: A,
    pub message: M,
}

/// Construction/extraction of the user lane through a composed event type.
pub trait UserEvent: Sized {
    type Addr: Address;
    type Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self;

    /// # Errors
    /// Returns the unchanged event when it belongs to another composed lane.
    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self>;
}

impl<A: Address, M> UserEvent for User<A, M> {
    type Addr = A;
    type Message = M;

    fn user(from: A, message: M) -> Self {
        Self { from, message }
    }
    fn into_user(self) -> Result<Self, Self> {
        Ok(self)
    }
}

impl<A: Address, M> TimeEvent for User<A, M> {
    fn time_reached(_: TimerElapsed) -> Option<Self> {
        None
    }
}
impl<A: Address, M> PeerEvent<A> for User<A, M> {
    fn peer_stopped(_: PeerStopped<A>) -> Option<Self> {
        None
    }
}
impl<A: Address, M> ChildEvent<A> for User<A, M> {
    fn child_stopped(_: ChildStopped<A>) -> Option<Self> {
        None
    }
}
impl<A: Address, M> WorkerEvent<A> for User<A, M> {
    fn worker_stopped(_: WorkerStopped<A>) -> Option<Self> {
        None
    }
}
impl<A: Address, M> ShutdownEvent for User<A, M> {
    fn shutdown_requested(_: ShutdownRequested) -> Option<Self> {
        None
    }
}
