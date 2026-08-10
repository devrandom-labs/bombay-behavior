//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::forward::forward_event_lane;
use crate::protocol::{
    ChildEvent, ChildStopped, CreationEvent, CreationResolved, WorkerCreationEvent,
    WorkerCreationResolved, WorkerEvent, WorkerStopped,
};
use crate::{Address, Behavior, User, UserEvent};

#[derive(Clone, PartialEq, Eq)]
pub enum ProxyEvent<E: UserEvent> {
    Inner(E),
    ChildStopped(ChildStopped<E::Addr>),
    CreationResolved(CreationResolved<<E::Addr as Address>::Nonce>),
}

impl<E: UserEvent> CreationEvent for ProxyEvent<E> {
    fn creation_resolved(event: CreationResolved<<E::Addr as Address>::Nonce>) -> Option<Self> {
        Some(Self::CreationResolved(event))
    }
}

impl<E: UserEvent> ChildEvent for ProxyEvent<E> {
    fn child_stopped(event: ChildStopped<E::Addr>) -> Option<Self> {
        Some(Self::ChildStopped(event))
    }
}

impl<E: UserEvent> UserEvent for ProxyEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            service @ (Self::ChildStopped(_) | Self::CreationResolved(_)) => Err(service),
        }
    }
}

forward_event_lane!(ProxyEvent, TimeEvent, time_reached, crate::TimerElapsed);
forward_event_lane!(
    ProxyEvent,
    PeerEvent,
    peer_stopped,
    crate::PeerStopped<E::Addr>
);
forward_event_lane!(
    ProxyEvent,
    WorkerEvent,
    worker_stopped,
    crate::WorkerStopped<E::Addr>
);
forward_event_lane!(
    ProxyEvent,
    WorkerCreationEvent,
    worker_creation_resolved,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    ProxyEvent,
    ShutdownEvent,
    shutdown_requested,
    crate::ShutdownRequested
);

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

forward_event_lane!(
    SupervisionEvent,
    TimeEvent,
    time_reached,
    crate::TimerElapsed
);
forward_event_lane!(
    SupervisionEvent,
    PeerEvent,
    peer_stopped,
    crate::PeerStopped<E::Addr>
);
forward_event_lane!(
    SupervisionEvent,
    ShutdownEvent,
    shutdown_requested,
    crate::ShutdownRequested
);

/// Commands accepted by a stable proxy.
#[derive(Debug)]
pub enum ProxyCommand<C: Behavior> {
    Forward(C::Msg),
    Replace(C),
}
