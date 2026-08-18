//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ShutdownRequested,
    WorkerCreationResolved, WorkerStopped,
};
use crate::{Address, Behavior};
use behavior::{Here, InjectEvent, Inside, User, UserEvent};

#[derive(Clone, PartialEq, Eq)]
pub enum ProxyEvent<E: UserEvent> {
    Command(E),
    ChildStopped(ChildStopped<E::Addr>),
    CreationResolved(CreationResolved<E::Addr>),
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<<E::Addr as Address>::Nonce>),
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
            service => Err(service),
        }
    }
}

impl<E: UserEvent> InjectEvent<ChildStopped<E::Addr>, Here> for ProxyEvent<E> {
    fn inject_at(value: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<E: UserEvent> InjectEvent<CreationResolved<E::Addr>, Here> for ProxyEvent<E> {
    fn inject_at(value: CreationResolved<E::Addr>) -> Self {
        Self::CreationResolved(value)
    }
}
impl<E: UserEvent> InjectEvent<ShutdownRequested, Here> for ProxyEvent<E> {
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}
impl<E: UserEvent> InjectEvent<ChildShutdownRejected<<E::Addr as Address>::Nonce>, Here>
    for ProxyEvent<E>
{
    fn inject_at(value: ChildShutdownRejected<<E::Addr as Address>::Nonce>) -> Self {
        Self::ChildShutdownRejected(value)
    }
}

impl<E, Input, Path> InjectEvent<Input, Inside<Path>> for ProxyEvent<E>
where
    E: UserEvent + InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Command(E::inject_at(input))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum SupervisionEvent<E: UserEvent> {
    Behavior(E),
    ChildStopped(ChildStopped<E::Addr>),
    WorkerStopped(WorkerStopped<E::Addr>),
    CreationResolved(CreationResolved<E::Addr>),
    WorkerCreationResolved(WorkerCreationResolved<<E::Addr as Address>::Nonce>),
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
            service => Err(service),
        }
    }
}

impl<E: UserEvent> behavior::ComposedEvent for SupervisionEvent<E> {
    type Inner = E;

    fn from_inner(event: E) -> Self {
        Self::Behavior(event)
    }
}

impl<E: UserEvent> InjectEvent<ChildStopped<E::Addr>, Here> for SupervisionEvent<E> {
    fn inject_at(value: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<E: UserEvent> InjectEvent<WorkerStopped<E::Addr>, Here> for SupervisionEvent<E> {
    fn inject_at(value: WorkerStopped<E::Addr>) -> Self {
        Self::WorkerStopped(value)
    }
}
impl<E: UserEvent> InjectEvent<CreationResolved<E::Addr>, Here> for SupervisionEvent<E> {
    fn inject_at(value: CreationResolved<E::Addr>) -> Self {
        Self::CreationResolved(value)
    }
}
impl<E: UserEvent> InjectEvent<WorkerCreationResolved<<E::Addr as Address>::Nonce>, Here>
    for SupervisionEvent<E>
{
    fn inject_at(value: WorkerCreationResolved<<E::Addr as Address>::Nonce>) -> Self {
        Self::WorkerCreationResolved(value)
    }
}

impl<E, Input, Path> InjectEvent<Input, Inside<Path>> for SupervisionEvent<E>
where
    E: UserEvent + InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Behavior(E::inject_at(input))
    }
}

/// Commands accepted by a stable proxy.
pub enum ProxyCommand<C: Behavior> {
    Forward(crate::BehaviorMessage<C>),
    Replace(C),
}
