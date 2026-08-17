//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::forward::forward_event_lane;
use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ShutdownRequested,
    WorkerCreationResolved, WorkerStopped,
};
use crate::{Address, Behavior, User, UserEvent};

#[derive(Clone, PartialEq, Eq)]
pub enum ProxyEvent<E: UserEvent> {
    Command(E),
    ChildStopped(ChildStopped<E::Addr>),
    CreationResolved(CreationResolved<<E::Addr as Address>::Nonce>),
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<<E::Addr as Address>::Nonce>),
}

impl<E: UserEvent> crate::RouteInput<CreationResolved<<E::Addr as Address>::Nonce>>
    for ProxyEvent<E>
{
    fn route(
        event: CreationResolved<<E::Addr as Address>::Nonce>,
    ) -> Result<Self, CreationResolved<<E::Addr as Address>::Nonce>> {
        Ok(Self::CreationResolved(event))
    }
}

impl<E: UserEvent> crate::EventInput<CreationResolved<<E::Addr as Address>::Nonce>>
    for ProxyEvent<E>
{
    fn inject(event: CreationResolved<<E::Addr as Address>::Nonce>) -> Self {
        Self::CreationResolved(event)
    }
}

impl<E: UserEvent> crate::RouteInput<ChildStopped<E::Addr>> for ProxyEvent<E> {
    fn route(event: ChildStopped<E::Addr>) -> Result<Self, ChildStopped<E::Addr>> {
        Ok(Self::ChildStopped(event))
    }
}

impl<E: UserEvent> crate::EventInput<ChildStopped<E::Addr>> for ProxyEvent<E> {
    fn inject(event: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(event)
    }
}

impl<E: UserEvent> UserEvent for ProxyEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Command(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Command(event) => event.into_user().map_err(Self::Command),
            service @ (Self::ChildStopped(_)
            | Self::CreationResolved(_)
            | Self::ShutdownRequested(_)
            | Self::ChildShutdownRejected(_)) => Err(service),
        }
    }
}

forward_event_lane!(ProxyEvent, crate::TimerElapsed, Command);
forward_event_lane!(ProxyEvent, crate::PeerStopped<E::Addr>, Command);
forward_event_lane!(ProxyEvent, crate::WorkerStopped<E::Addr>, Command);
forward_event_lane!(
    ProxyEvent,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>,
    Command
);
impl<E: UserEvent> crate::RouteInput<ShutdownRequested> for ProxyEvent<E> {
    fn route(event: ShutdownRequested) -> Result<Self, ShutdownRequested> {
        Ok(Self::ShutdownRequested(event))
    }
}

impl<E: UserEvent> crate::EventInput<ShutdownRequested> for ProxyEvent<E> {
    fn inject(event: ShutdownRequested) -> Self {
        Self::ShutdownRequested(event)
    }
}

impl<E: UserEvent> crate::RouteInput<ChildShutdownRejected<<E::Addr as Address>::Nonce>>
    for ProxyEvent<E>
{
    fn route(
        event: ChildShutdownRejected<<E::Addr as Address>::Nonce>,
    ) -> Result<Self, ChildShutdownRejected<<E::Addr as Address>::Nonce>> {
        Ok(Self::ChildShutdownRejected(event))
    }
}

impl<E: UserEvent> crate::EventInput<ChildShutdownRejected<<E::Addr as Address>::Nonce>>
    for ProxyEvent<E>
{
    fn inject(event: ChildShutdownRejected<<E::Addr as Address>::Nonce>) -> Self {
        Self::ChildShutdownRejected(event)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum SupervisionEvent<E: UserEvent> {
    Behavior(E),
    ChildStopped(ChildStopped<E::Addr>),
    WorkerStopped(WorkerStopped<E::Addr>),
    CreationResolved(CreationResolved<<E::Addr as Address>::Nonce>),
    WorkerCreationResolved(WorkerCreationResolved<<E::Addr as Address>::Nonce>),
}

impl<E: UserEvent> crate::RouteInput<CreationResolved<<E::Addr as Address>::Nonce>>
    for SupervisionEvent<E>
{
    fn route(
        event: CreationResolved<<E::Addr as Address>::Nonce>,
    ) -> Result<Self, CreationResolved<<E::Addr as Address>::Nonce>> {
        Ok(Self::CreationResolved(event))
    }
}

impl<E: UserEvent> crate::EventInput<CreationResolved<<E::Addr as Address>::Nonce>>
    for SupervisionEvent<E>
{
    fn inject(event: CreationResolved<<E::Addr as Address>::Nonce>) -> Self {
        Self::CreationResolved(event)
    }
}

impl<E: UserEvent> crate::RouteInput<WorkerCreationResolved<<E::Addr as Address>::Nonce>>
    for SupervisionEvent<E>
{
    fn route(
        event: WorkerCreationResolved<<E::Addr as Address>::Nonce>,
    ) -> Result<Self, WorkerCreationResolved<<E::Addr as Address>::Nonce>> {
        Ok(Self::WorkerCreationResolved(event))
    }
}

impl<E: UserEvent> crate::EventInput<WorkerCreationResolved<<E::Addr as Address>::Nonce>>
    for SupervisionEvent<E>
{
    fn inject(event: WorkerCreationResolved<<E::Addr as Address>::Nonce>) -> Self {
        Self::WorkerCreationResolved(event)
    }
}

impl<E: UserEvent> crate::RouteInput<ChildStopped<E::Addr>> for SupervisionEvent<E> {
    fn route(event: ChildStopped<E::Addr>) -> Result<Self, ChildStopped<E::Addr>> {
        Ok(Self::ChildStopped(event))
    }
}

impl<E: UserEvent> crate::EventInput<ChildStopped<E::Addr>> for SupervisionEvent<E> {
    fn inject(event: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(event)
    }
}

impl<E: UserEvent> crate::RouteInput<WorkerStopped<E::Addr>> for SupervisionEvent<E> {
    fn route(event: WorkerStopped<E::Addr>) -> Result<Self, WorkerStopped<E::Addr>> {
        Ok(Self::WorkerStopped(event))
    }
}

impl<E: UserEvent> crate::EventInput<WorkerStopped<E::Addr>> for SupervisionEvent<E> {
    fn inject(event: WorkerStopped<E::Addr>) -> Self {
        Self::WorkerStopped(event)
    }
}

impl<E: UserEvent> UserEvent for SupervisionEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Behavior(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Behavior(event) => event.into_user().map_err(Self::Behavior),
            service @ (Self::ChildStopped(_)
            | Self::WorkerStopped(_)
            | Self::CreationResolved(_)
            | Self::WorkerCreationResolved(_)) => Err(service),
        }
    }
}

forward_event_lane!(SupervisionEvent, crate::TimerElapsed);
forward_event_lane!(SupervisionEvent, crate::PeerStopped<E::Addr>);
forward_event_lane!(SupervisionEvent, crate::ShutdownRequested);

/// Commands accepted by a stable proxy.
#[derive(Debug)]
pub enum ProxyCommand<C: Behavior> {
    Forward(C::Msg),
    Replace(C),
}
