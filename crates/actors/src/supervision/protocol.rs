//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ShutdownRequested,
    WorkerCreationResolved, WorkerStopped,
};
use crate::{Address, Behavior};
use behavior::{Here, InjectEvent, Inside, MessageProtocol, User, UserEvent};

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
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<<E::Addr as Address>::Nonce>),
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
impl<E: UserEvent> InjectEvent<ShutdownRequested, Here> for SupervisionEvent<E> {
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}
impl<E: UserEvent> InjectEvent<ChildShutdownRejected<<E::Addr as Address>::Nonce>, Here>
    for SupervisionEvent<E>
{
    fn inject_at(value: ChildShutdownRejected<<E::Addr as Address>::Nonce>) -> Self {
        Self::ChildShutdownRejected(value)
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

/// A mailbox-admitted command that no current worker incarnation could accept.
///
/// Expected lifecycle unavailability is returned through the route carried by
/// [`ProxyCommand::Forward`]; it is not a behavior-fold failure. The complete
/// command remains owned by this value so a customer can retry, redirect, or
/// reject it according to its own protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyUnavailable<A: Address, M> {
    /// Exact proxy lifecycle phase that rejected forwarding.
    pub phase: crate::IncarnationPhase<A::Nonce>,
    /// Complete command whose ownership is returned.
    pub command: M,
}

/// Commands accepted by a stable proxy.
pub enum ProxyCommand<C: Behavior<Ph = behavior::Never>> {
    /// Forward one domain command or return it to the explicit logical owner.
    Forward {
        /// Domain command intended for the current worker incarnation.
        command: crate::BehaviorMessage<C>,
        /// Logical customer route for expected lifecycle unavailability.
        unavailable_to: behavior::Recipient<
            MessageProtocol<
                crate::BehaviorAddr<C>,
                ProxyUnavailable<crate::BehaviorAddr<C>, crate::BehaviorMessage<C>>,
            >,
        >,
    },
    /// Install a fresh successor after orderly termination of the current worker.
    Replace(C),
}

/// Supervision events plus one typed lane for a returned proxy command.
///
/// Lifecycle facts retain their direct `Here` path; this sum does not add an
/// actor wrapper or reinterpret the underlying supervision transition.
pub enum CommandSupervisionEvent<E, M>
where
    E: UserEvent,
{
    /// Application-owned protocol event.
    Behavior(E),
    /// Stable proxy child stopped.
    ChildStopped(ChildStopped<E::Addr>),
    /// Replaceable worker incarnation stopped behind a stable proxy.
    WorkerStopped(WorkerStopped<E::Addr>),
    /// Stable proxy creation resolved.
    CreationResolved(CreationResolved<E::Addr>),
    /// Worker-incarnation creation resolved.
    WorkerCreationResolved(WorkerCreationResolved<<E::Addr as Address>::Nonce>),
    /// Orderly pool shutdown was requested.
    ShutdownRequested(ShutdownRequested),
    /// One stable proxy rejected its shutdown request.
    ChildShutdownRejected(ChildShutdownRejected<<E::Addr as Address>::Nonce>),
    /// A stable proxy returned one pool-owned command it could not forward.
    CommandUnavailable(User<E::Addr, ProxyUnavailable<E::Addr, M>>),
}

impl<E, M> UserEvent for CommandSupervisionEvent<E, M>
where
    E: UserEvent,
{
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

impl<E, M> behavior::ComposedEvent for CommandSupervisionEvent<E, M>
where
    E: UserEvent,
{
    type Inner = E;

    fn from_inner(event: E) -> Self {
        Self::Behavior(event)
    }
}

macro_rules! command_supervision_fact {
    ($fact:ty, $variant:ident) => {
        impl<E, M> InjectEvent<$fact, Here> for CommandSupervisionEvent<E, M>
        where
            E: UserEvent,
        {
            fn inject_at(value: $fact) -> Self {
                Self::$variant(value)
            }
        }
    };
}

command_supervision_fact!(ChildStopped<E::Addr>, ChildStopped);
command_supervision_fact!(CreationResolved<E::Addr>, CreationResolved);
command_supervision_fact!(WorkerStopped<E::Addr>, WorkerStopped);
command_supervision_fact!(
    WorkerCreationResolved<<E::Addr as Address>::Nonce>,
    WorkerCreationResolved
);
command_supervision_fact!(ShutdownRequested, ShutdownRequested);
command_supervision_fact!(
    ChildShutdownRejected<<E::Addr as Address>::Nonce>,
    ChildShutdownRejected
);

impl<E, M> InjectEvent<User<E::Addr, ProxyUnavailable<E::Addr, M>>, Here>
    for CommandSupervisionEvent<E, M>
where
    E: UserEvent,
{
    fn inject_at(value: User<E::Addr, ProxyUnavailable<E::Addr, M>>) -> Self {
        Self::CommandUnavailable(value)
    }
}

impl<E, M, Input, Path> InjectEvent<Input, Inside<Path>> for CommandSupervisionEvent<E, M>
where
    E: UserEvent + InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Behavior(E::inject_at(input))
    }
}
