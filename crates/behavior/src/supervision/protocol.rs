//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::{
    ChildEvent, ChildStopped, CreationEvent, CreationResolved, PeerEvent, PeerStopped,
    ShutdownEvent, ShutdownRequested, TimeEvent, TimerElapsed, WorkerCreationEvent,
    WorkerCreationResolved, WorkerEvent, WorkerStopped,
};
use crate::{Address, Behavior, User, UserEvent};

#[derive(Clone, PartialEq, Eq)]
pub enum SupervisionEvent<E: UserEvent> {
    Inner(E),
    ChildStopped(ChildStopped<E::Addr>),
    WorkerStopped(WorkerStopped<E::Addr>),
    CreationResolved(CreationResolved<<E::Addr as Address>::Nonce>),
    WorkerCreationResolved(WorkerCreationResolved<<E::Addr as Address>::Nonce>),
}

impl<E: UserEvent> CreationEvent for SupervisionEvent<E> {
    fn creation_resolved(event: CreationResolved<<E::Addr as Address>::Nonce>) -> Option<Self> {
        Some(Self::CreationResolved(event))
    }
}

impl<E: UserEvent> WorkerCreationEvent for SupervisionEvent<E> {
    fn worker_creation_resolved(
        event: WorkerCreationResolved<<E::Addr as Address>::Nonce>,
    ) -> Option<Self> {
        Some(Self::WorkerCreationResolved(event))
    }
}

impl<E: UserEvent> ChildEvent for SupervisionEvent<E> {
    fn child_stopped(event: ChildStopped<E::Addr>) -> Option<Self> {
        Some(Self::ChildStopped(event))
    }
}

impl<E: UserEvent> WorkerEvent for SupervisionEvent<E> {
    fn worker_stopped(event: WorkerStopped<E::Addr>) -> Option<Self> {
        Some(Self::WorkerStopped(event))
    }
}

impl<E: UserEvent> UserEvent for SupervisionEvent<E> {
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

impl<E: TimeEvent> TimeEvent for SupervisionEvent<E> {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        E::time_reached(event).map(Self::Inner)
    }
}

impl<E: PeerEvent> PeerEvent for SupervisionEvent<E> {
    fn peer_stopped(event: PeerStopped<E::Addr>) -> Option<Self> {
        E::peer_stopped(event).map(Self::Inner)
    }
}

impl<E: ShutdownEvent> ShutdownEvent for SupervisionEvent<E> {
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
