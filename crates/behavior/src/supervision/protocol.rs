//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::{
    ChildEvent, ChildStopped, CreationEvent, CreationResolved, PeerEvent, PeerStopped,
    ShutdownEvent, ShutdownRequested, TimeEvent, TimerElapsed, WorkerCreationEvent,
    WorkerCreationResolved, WorkerEvent, WorkerStopped,
};
use crate::{Address, Behavior, User, UserEvent};

#[derive(Clone, PartialEq, Eq)]
pub enum SupervisionEvent<E, A: Address> {
    Inner(E),
    ChildStopped(ChildStopped<A>),
    WorkerStopped(WorkerStopped<A>),
    CreationResolved(CreationResolved<A>),
    WorkerCreationResolved(WorkerCreationResolved<A>),
}

impl<E, A: Address> CreationEvent<A> for SupervisionEvent<E, A> {
    fn creation_resolved(event: CreationResolved<A>) -> Option<Self> {
        Some(Self::CreationResolved(event))
    }
}

impl<E, A: Address> WorkerCreationEvent<A> for SupervisionEvent<E, A> {
    fn worker_creation_resolved(event: WorkerCreationResolved<A>) -> Option<Self> {
        Some(Self::WorkerCreationResolved(event))
    }
}

impl<E, A: Address> ChildEvent<A> for SupervisionEvent<E, A> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        Some(Self::ChildStopped(event))
    }
}

impl<E, A: Address> WorkerEvent<A> for SupervisionEvent<E, A> {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self> {
        Some(Self::WorkerStopped(event))
    }
}

impl<E: UserEvent, A: Address> UserEvent for SupervisionEvent<E, A> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            service @ (Self::ChildStopped(_)
            | Self::WorkerStopped(_)
            | Self::CreationResolved(_)
            | Self::WorkerCreationResolved(_)) => Err(service),
        }
    }
}

impl<E: TimeEvent, A: Address> TimeEvent for SupervisionEvent<E, A> {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        E::time_reached(event).map(Self::Inner)
    }
}

impl<E: PeerEvent<A>, A: Address> PeerEvent<A> for SupervisionEvent<E, A> {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self> {
        E::peer_stopped(event).map(Self::Inner)
    }
}

impl<E: ShutdownEvent, A: Address> ShutdownEvent for SupervisionEvent<E, A> {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self> {
        E::shutdown_requested(event).map(Self::Inner)
    }
}

/// Commands accepted by a stable proxy.
#[derive(Debug)]
pub enum ProxyCommand<C: Behavior> {
    Forward(C::Msg),
    Replace(C),
}
