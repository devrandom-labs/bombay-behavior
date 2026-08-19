//! Explicitly managed dynamic stable-child topology.

use crate::{
    ChildShutdownRejected, ChildStopped, CreationRejection, CreationResolved, ObserveChild,
    ObserveCreation, Own, Proxy, ProxyCommand, ProxyParentIngress, ProxyWithParent, SendInput,
    ShutdownChild, ShutdownRequested, WorkerCreationResolved, WorkerStopped,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, Births, Create, Delivery, Here, InjectEvent,
    InterpreterRequests, Never, Protocol, Recipient, SendEffects, User, UserEvent,
};

/// One dynamically managed stable-child phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicChildPhase {
    /// The stable proxy creation has been staged but not committed.
    Installing,
    /// The stable proxy is established and accepts management commands.
    ///
    /// This describes the proxy slot, not one worker incarnation behind it.
    Available,
    /// Orderly shutdown of the stable proxy has been accepted.
    Stopping,
    /// The stable proxy is realizing a fresh worker incarnation.
    Replacing,
    /// The stable proxy slot has terminated or failed installation.
    Retired,
}

/// Admission rejection produced without consuming an existing child slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSupervisorRejection {
    AlreadyExists,
    NotAvailable,
    NotFound,
    /// The supervisor has begun terminal subtree shutdown.
    ShuttingDown,
    /// The interpreter rejected orderly shutdown of the selected established
    /// proxy.  The exact capability rejection is preserved rather than
    /// collapsed into an admission-state guess.
    ShutdownRejected(crate::ChildShutdownRejection),
}

/// A terminal failure while draining the dynamic supervisor's owned proxies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DynamicSupervisorError<N> {
    /// The runtime rejected orderly shutdown of an owned stable proxy.
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: N,
        reason: crate::ChildShutdownRejection,
    },
}

/// Commands for an explicitly managed dynamic child set.
pub enum DynamicSupervisorMessage<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: Protocol<Addr = A>,
{
    Start {
        nonce: A::Nonce,
        child: C,
        reply_to: Recipient<Reply>,
    },
    Stop {
        nonce: A::Nonce,
        reply_to: Recipient<Reply>,
    },
    Replace {
        nonce: A::Nonce,
        child: C,
        reply_to: Recipient<Reply>,
    },
    Query {
        nonce: A::Nonce,
        reply_to: Recipient<Reply>,
    },
}

/// Complete command or realization outcome returned to a typed recipient.
pub enum DynamicSupervisorOutcome<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    StartAccepted {
        nonce: A::Nonce,
    },
    StartRejected {
        nonce: A::Nonce,
        child: C,
        reason: DynamicSupervisorRejection,
    },
    Started {
        nonce: A::Nonce,
        child: Recipient<Proxy<C>>,
    },
    StartFailed {
        nonce: A::Nonce,
        reason: CreationRejection,
    },
    StopAccepted {
        nonce: A::Nonce,
    },
    StopRejected {
        nonce: A::Nonce,
        reason: DynamicSupervisorRejection,
    },
    Stopped {
        nonce: A::Nonce,
        outcome: crate::TerminalOutcome<A>,
    },
    ReplaceAccepted {
        nonce: A::Nonce,
    },
    ReplaceRejected {
        nonce: A::Nonce,
        child: C,
        reason: DynamicSupervisorRejection,
    },
    Replaced {
        nonce: A::Nonce,
    },
    ReplacementFailed {
        nonce: A::Nonce,
        reason: CreationRejection,
    },
    State {
        nonce: A::Nonce,
        phase: Option<DynamicChildPhase>,
    },
}

enum DynamicChild<R: Protocol> {
    Installing { reply_to: Recipient<R> },
    Available,
    Stopping { reply_to: Recipient<R> },
    Replacing { reply_to: Recipient<R> },
    Retired,
}

impl<R: Protocol> DynamicChild<R> {
    const fn phase(&self) -> DynamicChildPhase {
        match self {
            Self::Installing { .. } => DynamicChildPhase::Installing,
            Self::Available => DynamicChildPhase::Available,
            Self::Stopping { .. } => DynamicChildPhase::Stopping,
            Self::Replacing { .. } => DynamicChildPhase::Replacing,
            Self::Retired => DynamicChildPhase::Retired,
        }
    }
}

/// Runtime facts and user commands accepted by [`DynamicSupervisor`].
pub enum DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    Command(User<A, DynamicSupervisorMessage<A, C, Reply>>),
    ChildStopped(ChildStopped<A>),
    CreationResolved(CreationResolved<A>),
    WorkerCreationResolved(WorkerCreationResolved<A::Nonce>),
    WorkerStopped(WorkerStopped<A>),
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<A::Nonce>),
}

impl<A, C, Reply> UserEvent for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    type Addr = A;
    type Message = DynamicSupervisorMessage<A, C, Reply>;
    fn user(from: A, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }
    fn into_user(self) -> Result<User<A, Self::Message>, Self> {
        match self {
            Self::Command(user) => Ok(user),
            other => Err(other),
        }
    }
}

impl<A, C, Reply> InjectEvent<ChildStopped<A>, Here> for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn inject_at(value: ChildStopped<A>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<A, C, Reply> InjectEvent<CreationResolved<A>, Here> for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn inject_at(value: CreationResolved<A>) -> Self {
        Self::CreationResolved(value)
    }
}
impl<A, C, Reply> InjectEvent<WorkerCreationResolved<A::Nonce>, Here>
    for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn inject_at(value: WorkerCreationResolved<A::Nonce>) -> Self {
        Self::WorkerCreationResolved(value)
    }
}
impl<A, C, Reply> InjectEvent<WorkerStopped<A>, Here> for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn inject_at(value: WorkerStopped<A>) -> Self {
        Self::WorkerStopped(value)
    }
}
impl<A, C, Reply> InjectEvent<ChildShutdownRejected<A::Nonce>, Here>
    for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn inject_at(value: ChildShutdownRejected<A::Nonce>) -> Self {
        Self::ChildShutdownRejected(value)
    }
}

impl<A, C, Reply> InjectEvent<ShutdownRequested, Here> for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}

enum DynamicSupervisorState<N> {
    Running,
    Draining { awaiting: Vec<N> },
}

/// Shutdown-capable stable proxy protocol installed by [`DynamicSupervisor`].
///
/// The proxy makes orderly subtree shutdown an explicit part of its concrete
/// event and effect products while preserving stable-address construction.
pub type DynamicProxy<C> = Proxy<C>;
pub type DynamicProxyWithParent<C, ParentPath> = ProxyWithParent<C, ParentPath>;

/// Named effect product for dynamic topology management.
pub struct DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    pub outcomes: Vec<Delivery<Reply>>,
    pub child_observations: InterpreterRequests<ObserveChild<A>>,
    pub creation_observations: InterpreterRequests<ObserveCreation<A>>,
    pub shutdowns: InterpreterRequests<ShutdownChild<DynamicProxyWithParent<C, ParentPath>>>,
    pub replacements: Vec<Delivery<Proxy<C>>>,
}

impl<A, C, Reply, ParentPath> SendEffects for DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn empty() -> Self {
        Self {
            outcomes: vec![],
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
            replacements: vec![],
        }
    }
    fn append(&mut self, other: Self) {
        self.outcomes.extend(other.outcomes);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.shutdowns.append(other.shutdowns);
        self.replacements.extend(other.replacements);
    }
}

impl<A, C, Reply, ParentPath> behavior::SendsFor<DynamicSupervisorEvent<A, C, Reply>>
    for DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
    InterpreterRequests<ObserveChild<A>>: behavior::SendsFor<DynamicSupervisorEvent<A, C, Reply>>,
    InterpreterRequests<ObserveCreation<A>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Reply>>,
    InterpreterRequests<ShutdownChild<DynamicProxyWithParent<C, ParentPath>>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Reply>>,
{
}

impl<I, RootEvent, Path, A, C, Reply, ParentPath> behavior::InterpretSends<I, RootEvent, Path>
    for DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    I: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
    Vec<Delivery<Reply>>: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveChild<A>>: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveCreation<A>>: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<DynamicProxyWithParent<C, ParentPath>>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    Vec<Delivery<Proxy<C>>>: behavior::InterpretSends<I, RootEvent, Path>,
    DynamicSupervisorSends<A, C, Reply, ParentPath>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.outcomes, interpreter).await?;
            behavior::InterpretSends::interpret(self.child_observations, interpreter).await?;
            behavior::InterpretSends::interpret(self.creation_observations, interpreter).await?;
            behavior::InterpretSends::interpret(self.shutdowns, interpreter).await?;
            behavior::InterpretSends::interpret(self.replacements, interpreter).await
        }
    }
}
impl<A, C, Reply, ParentPath> SendInput<ObserveChild<A>, Own>
    for DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn emit(&mut self, value: ObserveChild<A>) {
        self.child_observations.send(value);
    }
}
impl<A, C, Reply, ParentPath> SendInput<ObserveCreation<A>, Own>
    for DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn emit(&mut self, value: ObserveCreation<A>) {
        self.creation_observations.send(value);
    }
}
impl<A, C, Reply, ParentPath> SendInput<ShutdownChild<DynamicProxyWithParent<C, ParentPath>>, Own>
    for DynamicSupervisorSends<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A>,
{
    fn emit(&mut self, value: ShutdownChild<DynamicProxyWithParent<C, ParentPath>>) {
        self.shutdowns.send(value);
    }
}

/// A pure dynamic supervisor whose stable proxy set changes only through its
/// typed command protocol and committed runtime facts.
pub struct DynamicSupervisorWithParent<A, C, Reply, ParentPath>
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Reply: Protocol<Addr = A>,
{
    children: Vec<(A::Nonce, DynamicChild<Reply>)>,
    state: DynamicSupervisorState<A::Nonce>,
    proxy_parent: ProxyParentIngress<A, ParentPath>,
    marker: core::marker::PhantomData<fn() -> C>,
}

/// A dynamic supervisor whose proxy reports are owned by its direct event layer.
pub type DynamicSupervisor<A, C, Reply> = DynamicSupervisorWithParent<A, C, Reply, Here>;

impl<A, C, Reply, ParentPath> DynamicSupervisorWithParent<A, C, Reply, ParentPath>
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Reply: Protocol<Addr = A>,
{
    #[must_use]
    pub const fn with_parent(proxy_parent: ProxyParentIngress<A, ParentPath>) -> Self {
        Self {
            children: vec![],
            state: DynamicSupervisorState::Running,
            proxy_parent,
            marker: core::marker::PhantomData,
        }
    }
    #[must_use]
    pub fn phase(&self, nonce: A::Nonce) -> Option<DynamicChildPhase>
    where
        A::Nonce: Eq,
    {
        self.children
            .iter()
            .find(|(n, _)| *n == nonce)
            .map(|(_, s)| s.phase())
    }
}
impl<A, C, Reply> DynamicSupervisorWithParent<A, C, Reply, Here>
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Reply: Protocol<Addr = A>,
{
    #[must_use]
    pub const fn new() -> Self {
        Self::with_parent(ProxyParentIngress::new())
    }
}
impl<A, C, Reply> Default for DynamicSupervisor<A, C, Reply>
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Reply: Protocol<Addr = A>,
{
    fn default() -> Self {
        Self::new()
    }
}
impl<A, C, Reply, ParentPath> crate::BehaviorBase
    for DynamicSupervisorWithParent<A, C, Reply, ParentPath>
where
    A: Address,
    C: Behavior,
    C::Protocol: Protocol<Addr = A>,
    Reply: Protocol<Addr = A>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, C, Reply, ParentPath> behavior::Protocol
    for DynamicSupervisorWithParent<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Addr = A;
    type Msg = DynamicSupervisorMessage<A, C, Reply>;
}

impl<A, C, Reply, ParentPath> Behavior for DynamicSupervisorWithParent<A, C, Reply, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Protocol = Self;
    type Event = DynamicSupervisorEvent<A, C, Reply>;
    type Sends = DynamicSupervisorSends<A, C, Reply, ParentPath>;
    type Ph = Never;
    type Error = DynamicSupervisorError<A::Nonce>;
    type Birth = Births<DynamicProxyWithParent<C, ParentPath>>;

    #[allow(clippy::too_many_lines)]
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let mut sends = Self::Sends::empty();
        match event {
            DynamicSupervisorEvent::Command(user) => match user.message {
                DynamicSupervisorMessage::Start {
                    nonce,
                    child,
                    reply_to,
                } => {
                    if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                        sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StartRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::ShuttingDown,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    if self
                        .children
                        .iter()
                        .any(|(n, s)| *n == nonce && !matches!(s, DynamicChild::Retired))
                    {
                        sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StartRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::AlreadyExists,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    if let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        *state = DynamicChild::Installing { reply_to };
                    } else {
                        self.children
                            .push((nonce, DynamicChild::Installing { reply_to }));
                    }
                    sends.outcomes.push(Delivery::new(
                        reply_to,
                        DynamicSupervisorOutcome::StartAccepted { nonce },
                    ));
                    sends.child_observations.send(ObserveChild::new(nonce));
                    sends
                        .creation_observations
                        .send(ObserveCreation::new(nonce));
                    Ok(Actions::new(
                        sends,
                        vec![Create::birth(
                            nonce,
                            ProxyWithParent::with_parent(child, self.proxy_parent),
                        )],
                        crate::Step::Continue,
                    ))
                }
                DynamicSupervisorMessage::Stop { nonce, reply_to } => {
                    if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                        sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StopRejected {
                                nonce,
                                reason: DynamicSupervisorRejection::ShuttingDown,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    match self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        Some((_, state @ DynamicChild::Available)) => {
                            *state = DynamicChild::Stopping { reply_to };
                            sends.shutdowns.send(ShutdownChild::<
                                DynamicProxyWithParent<C, ParentPath>,
                            >::new(nonce));
                            sends.outcomes.push(Delivery::new(
                                reply_to,
                                DynamicSupervisorOutcome::StopAccepted { nonce },
                            ));
                        }
                        Some(_) => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StopRejected {
                                nonce,
                                reason: DynamicSupervisorRejection::NotAvailable,
                            },
                        )),
                        None => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StopRejected {
                                nonce,
                                reason: DynamicSupervisorRejection::NotFound,
                            },
                        )),
                    }
                    Ok(Actions::send(sends))
                }
                DynamicSupervisorMessage::Replace {
                    nonce,
                    child,
                    reply_to,
                } => {
                    if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                        sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::ReplaceRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::ShuttingDown,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    match self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        Some((_, state @ DynamicChild::Available)) => {
                            *state = DynamicChild::Replacing { reply_to };
                            sends.replacements.push(Delivery::local_child(
                                behavior::ChildRecipient::new(nonce),
                                ProxyCommand::Replace(child),
                            ));
                            sends.outcomes.push(Delivery::new(
                                reply_to,
                                DynamicSupervisorOutcome::ReplaceAccepted { nonce },
                            ));
                        }
                        Some(_) => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::ReplaceRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::NotAvailable,
                            },
                        )),
                        None => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::ReplaceRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::NotFound,
                            },
                        )),
                    }
                    Ok(Actions::send(sends))
                }
                DynamicSupervisorMessage::Query { nonce, reply_to } => {
                    sends.outcomes.push(Delivery::new(
                        reply_to,
                        DynamicSupervisorOutcome::State {
                            nonce,
                            phase: self.phase(nonce),
                        },
                    ));
                    Ok(Actions::send(sends))
                }
            },
            DynamicSupervisorEvent::CreationResolved(resolved) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == resolved.nonce)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Installing { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                match resolved.result {
                    Ok(address) => {
                        *state = DynamicChild::Available;
                        sends.outcomes.push(Delivery::new(
                            reply,
                            DynamicSupervisorOutcome::Started {
                                nonce: resolved.nonce,
                                child: Recipient::global(address),
                            },
                        ));
                        if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                            sends.shutdowns.send(ShutdownChild::<
                                DynamicProxyWithParent<C, ParentPath>,
                            >::new(resolved.nonce));
                        }
                    }
                    Err(reason) => {
                        *state = DynamicChild::Retired;
                        sends.outcomes.push(Delivery::new(
                            reply,
                            DynamicSupervisorOutcome::StartFailed {
                                nonce: resolved.nonce,
                                reason,
                            },
                        ));
                        if let DynamicSupervisorState::Draining { awaiting } = &mut self.state {
                            awaiting.retain(|nonce| *nonce != resolved.nonce);
                            if awaiting.is_empty() {
                                return Ok(Actions::new(
                                    sends,
                                    Vec::new(),
                                    crate::Step::Stop(behavior::Stopped),
                                ));
                            }
                        }
                    }
                }
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::ChildStopped(stopped) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == stopped.nonce)
                else {
                    return Ok(Actions::cont());
                };
                let reply = match state {
                    DynamicChild::Stopping { reply_to } => Some(*reply_to),
                    _ if matches!(self.state, DynamicSupervisorState::Draining { .. }) => None,
                    _ => return Ok(Actions::cont()),
                };
                *state = DynamicChild::Retired;
                if let Some(reply) = reply {
                    sends.outcomes.push(Delivery::new(
                        reply,
                        DynamicSupervisorOutcome::Stopped {
                            nonce: stopped.nonce,
                            outcome: stopped.outcome,
                        },
                    ));
                }
                if let DynamicSupervisorState::Draining { awaiting } = &mut self.state {
                    awaiting.retain(|nonce| *nonce != stopped.nonce);
                    if awaiting.is_empty() {
                        return Ok(Actions::new(
                            sends,
                            Vec::new(),
                            crate::Step::Stop(behavior::Stopped),
                        ));
                    }
                }
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::WorkerCreationResolved(resolved) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == resolved.proxy)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Replacing { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                *state = DynamicChild::Available;
                let outcome = match resolved.result {
                    Ok(()) => DynamicSupervisorOutcome::Replaced {
                        nonce: resolved.proxy,
                    },
                    Err(reason) => DynamicSupervisorOutcome::ReplacementFailed {
                        nonce: resolved.proxy,
                        reason,
                    },
                };
                sends.outcomes.push(Delivery::new(reply, outcome));
                Ok(Actions::send(sends))
            }
            // The phase describes availability of the stable proxy slot, not
            // liveness of one worker incarnation behind it. Replacement
            // realization remains the separate `WorkerCreationResolved` fact.
            DynamicSupervisorEvent::WorkerStopped(_) => Ok(Actions::cont()),
            DynamicSupervisorEvent::ShutdownRequested(_) => {
                if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                    return Ok(Actions::cont());
                }
                let awaiting = self
                    .children
                    .iter()
                    .filter_map(|(nonce, state)| {
                        (!matches!(state, DynamicChild::Retired)).then_some(*nonce)
                    })
                    .collect::<Vec<_>>();
                if awaiting.is_empty() {
                    return Ok(Actions::stop());
                }
                for (nonce, state) in &self.children {
                    if matches!(
                        state,
                        DynamicChild::Available | DynamicChild::Replacing { .. }
                    ) {
                        sends.shutdowns.send(
                            ShutdownChild::<DynamicProxyWithParent<C, ParentPath>>::new(*nonce),
                        );
                    }
                }
                self.state = DynamicSupervisorState::Draining { awaiting };
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::ChildShutdownRejected(rejected) => {
                if matches!(&self.state, DynamicSupervisorState::Draining { awaiting } if awaiting.contains(&rejected.nonce))
                {
                    return Err(DynamicSupervisorError::ChildShutdownRejected {
                        nonce: rejected.nonce,
                        reason: rejected.reason,
                    });
                }
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == rejected.nonce)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Stopping { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                *state = DynamicChild::Available;
                sends.outcomes.push(Delivery::new(
                    reply,
                    DynamicSupervisorOutcome::StopRejected {
                        nonce: rejected.nonce,
                        reason: DynamicSupervisorRejection::ShutdownRejected(rejected.reason),
                    },
                ));
                Ok(Actions::send(sends))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::{Activate as _, CreationKind, Exit};
    use behavior::{MailAddr, NoBirths};

    struct Worker;
    impl behavior::Protocol for Worker {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Worker {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    struct Reply;
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = DynamicSupervisorOutcome<MailAddr, Worker>;
    }

    impl Behavior for Reply {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(99))
    }

    #[test]
    fn start_is_distinct_from_committed_installation_and_duplicate_returns_child() {
        let initialized = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        let accepted = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(active.phase(7), Some(DynamicChildPhase::Installing));
        assert_eq!(accepted.creates.len(), 1);
        assert_eq!(
            accepted.sends.child_observations.as_slice(),
            [ObserveChild::new(7)]
        );
        assert_eq!(
            accepted.sends.creation_observations.as_slice(),
            [ObserveCreation::new(7)]
        );
        assert!(matches!(
            accepted.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartAccepted { nonce: 7 }
        ));

        let duplicate = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(duplicate.creates.is_empty());
        assert!(duplicate.sends.child_observations.is_empty());
        assert!(duplicate.sends.creation_observations.is_empty());
        assert!(matches!(
            duplicate.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartRejected {
                nonce: 7,
                reason: DynamicSupervisorRejection::AlreadyExists,
                ..
            }
        ));

        let installed = active
            .on_path(CreationResolved::installed(
                7,
                CreationKind::Birth,
                MailAddr(70),
            ))
            .unwrap();
        assert_eq!(active.phase(7), Some(DynamicChildPhase::Available));
        assert!(matches!(
            installed.sends.outcomes[0].message,
            DynamicSupervisorOutcome::Started { nonce: 7, child }
                if child.address() == MailAddr(70)
        ));
    }

    #[test]
    fn rejected_start_resolution_retires_the_slot_without_losing_the_observation_request() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap()
            .behavior;
        let start = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 5,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(
            start.sends.child_observations.as_slice(),
            [ObserveChild::new(5)]
        );
        assert_eq!(
            start.sends.creation_observations.as_slice(),
            [ObserveCreation::new(5)]
        );

        let rejected = active
            .on_path(CreationResolved::rejected(
                5,
                CreationKind::Birth,
                CreationRejection::EnvironmentFailed,
            ))
            .unwrap();
        assert_eq!(active.phase(5), Some(DynamicChildPhase::Retired));
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartFailed {
                nonce: 5,
                reason: CreationRejection::EnvironmentFailed,
            }
        ));
    }

    #[test]
    fn stop_and_replace_wait_for_their_exact_runtime_fact() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap()
            .behavior;
        active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        active
            .on_path(CreationResolved::birth(3, MailAddr(30)))
            .unwrap();

        let replacing = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Replace {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(replacing.sends.replacements.len(), 1);
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Replacing));
        active
            .on_path(WorkerCreationResolved::new(
                3,
                4,
                CreationKind::replacement_of(2),
                Ok(()),
            ))
            .unwrap();
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));

        let stopping = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Stop {
                    nonce: 3,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(stopping.sends.shutdowns.as_slice(), [ShutdownChild::new(3)]);
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Stopping));
        let outcome = Err(crate::Crash::Panicked);
        let stopped = active
            .on_path(ChildStopped::new(3, outcome, Instant::now()))
            .unwrap();
        assert!(matches!(
            stopped.sends.outcomes[0].message,
            DynamicSupervisorOutcome::Stopped {
                nonce: 3,
                outcome: reported,
            } if reported == outcome
        ));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Retired));
    }

    #[test]
    fn explicit_stop_rejection_preserves_the_capability_reason() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap()
            .behavior;
        active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        active
            .on_path(CreationResolved::birth(3, MailAddr(30)))
            .unwrap();
        active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Stop {
                    nonce: 3,
                    reply_to: reply(),
                },
            )
            .unwrap();

        let rejected = active
            .on_path(ChildShutdownRejected::new(
                3,
                crate::ChildShutdownRejection::AlreadyStopping,
            ))
            .unwrap();
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StopRejected {
                nonce: 3,
                reason: DynamicSupervisorRejection::ShutdownRejected(
                    crate::ChildShutdownRejection::AlreadyStopping
                ),
            }
        ));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));
    }

    #[test]
    fn worker_stop_is_accepted_without_reinterpreting_proxy_slot_state() {
        let initialized = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        active
            .on_path(CreationResolved::birth(3, MailAddr(30)))
            .unwrap();
        let actions = active
            .on_path(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();

        assert!(actions.sends.outcomes.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, crate::Step::Continue));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));
    }

    #[test]
    fn shutdown_drains_established_and_installing_proxies_before_stopping() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap()
            .behavior;
        for nonce in [3, 4] {
            active
                .receive(
                    MailAddr(1),
                    DynamicSupervisorMessage::Start {
                        nonce,
                        child: Worker,
                        reply_to: reply(),
                    },
                )
                .unwrap();
        }
        active
            .on_path(CreationResolved::birth(3, MailAddr(30)))
            .unwrap();

        let requested = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(
            requested.sends.shutdowns.as_slice(),
            [ShutdownChild::new(3)]
        );
        assert!(matches!(requested.become_, crate::Step::Continue));

        let installed_during_drain = active
            .on_path(CreationResolved::birth(4, MailAddr(40)))
            .unwrap();
        assert_eq!(
            installed_during_drain.sends.shutdowns.as_slice(),
            [ShutdownChild::new(4)]
        );
        assert!(matches!(
            active
                .on_path(ChildStopped::new(3, Ok(Exit::Normal), Instant::now(),))
                .unwrap()
                .become_,
            crate::Step::Continue
        ));
        assert!(matches!(
            active
                .on_path(ChildStopped::new(4, Ok(Exit::Normal), Instant::now(),))
                .unwrap()
                .become_,
            crate::Step::Stop(behavior::Stopped)
        ));
    }
}
