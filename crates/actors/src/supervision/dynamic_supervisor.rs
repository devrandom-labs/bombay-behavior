//! Explicitly managed dynamic stable-child topology.

use crate::{
    ChildShutdownRejected, ChildStopped, CreationRejection, CreationResolved,
    DeliveryRouteProtocol, ObserveChild, ObserveCreation, Own, Proxy, ProxyCommand,
    ProxyParentIngress, ProxyWithParent, SendInput, ShutdownChild, ShutdownRequested,
    WorkerCreationResolved, WorkerStopped,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, Births, ChildDelivery, ChildRoute, Here,
    InjectEvent, InterpreterRequests, Never, Protocol, Recipient, SendEffects, User, UserEvent,
};

/// One dynamically managed stable-child phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicChildPhase {
    /// The stable proxy or its first worker creation has not yet committed.
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
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DynamicSupervisorError<A: Address> {
    /// The runtime rejected orderly shutdown of an owned stable proxy.
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: A::Nonce,
        reason: crate::ChildShutdownRejection,
    },
    /// A stable-proxy creation fact did not match an installing slot.
    #[error("stable-proxy creation fact does not match an installing child")]
    UnexpectedCreation(CreationResolved<A>),
    /// A stable-proxy stop fact did not match a stopping or draining slot.
    #[error("stable-proxy stop fact does not match a stopping child")]
    UnexpectedChildStopped(ChildStopped<A>),
    /// A worker-creation fact did not match an explicit replacement.
    #[error("worker-creation fact does not match a replacing child")]
    UnexpectedWorkerCreation(WorkerCreationResolved<A::Nonce>),
    /// A worker fact exists for a proxy creation the interpreter rejected.
    #[error("worker creation was reported for a rejected stable-proxy creation")]
    ContradictoryInitialCreation {
        proxy: CreationResolved<A>,
        worker: WorkerCreationResolved<A::Nonce>,
    },
    /// A worker-stop fact named no stable proxy owned by this supervisor.
    #[error("worker-stop fact names an unknown stable proxy")]
    UnexpectedWorkerStopped(WorkerStopped<A>),
    /// A shutdown rejection did not match a pending stop or drain.
    #[error("child-shutdown rejection does not match a pending shutdown")]
    UnexpectedChildShutdownRejection(ChildShutdownRejected<A::Nonce>),
}

/// Commands for an explicitly managed dynamic child set.
pub enum DynamicSupervisorMessage<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    Start {
        nonce: A::Nonce,
        child: C,
        reply_to: Route,
    },
    Stop {
        nonce: A::Nonce,
        reply_to: Route,
    },
    Replace {
        nonce: A::Nonce,
        child: C,
        reply_to: Route,
    },
    Query {
        nonce: A::Nonce,
        reply_to: Route,
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

#[derive(Clone, Copy)]
enum DynamicWorker<N> {
    Running { incarnation: N },
    Vacant { last: N },
}

#[derive(Clone, Copy)]
enum InitialCreation<A: Address> {
    AwaitingBoth,
    ProxyCommitted {
        address: A,
    },
    WorkerResolved {
        resolved: WorkerCreationResolved<A::Nonce>,
    },
}

enum DynamicChild<A: Address, Route> {
    Installing {
        reply_to: Route,
        progress: InitialCreation<A>,
    },
    Available {
        worker: DynamicWorker<A::Nonce>,
    },
    Stopping {
        reply_to: Route,
        worker: DynamicWorker<A::Nonce>,
    },
    RetiringAfterStartFailure,
    StoppingWorkerForReplacement {
        reply_to: Route,
        worker: A::Nonce,
    },
    CreatingReplacement {
        reply_to: Route,
        replaces: A::Nonce,
    },
    Retired,
}

impl<A: Address, Route> DynamicChild<A, Route> {
    const fn phase(&self) -> DynamicChildPhase {
        match self {
            Self::Installing { .. } => DynamicChildPhase::Installing,
            Self::Available { .. } => DynamicChildPhase::Available,
            Self::Stopping { .. } | Self::RetiringAfterStartFailure => DynamicChildPhase::Stopping,
            Self::StoppingWorkerForReplacement { .. } | Self::CreatingReplacement { .. } => {
                DynamicChildPhase::Replacing
            }
            Self::Retired => DynamicChildPhase::Retired,
        }
    }
}

/// Runtime facts and user commands accepted by [`DynamicSupervisor`].
pub enum DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    Command(User<A, DynamicSupervisorMessage<A, C, Route>>),
    ChildStopped(ChildStopped<A>),
    CreationResolved(CreationResolved<A>),
    WorkerCreationResolved(WorkerCreationResolved<A::Nonce>),
    WorkerStopped(WorkerStopped<A>),
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<A::Nonce>),
}

impl<A, C, Route> UserEvent for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Addr = A;
    type Message = DynamicSupervisorMessage<A, C, Route>;
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

impl<A, C, Route> InjectEvent<ChildStopped<A>, Here> for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: ChildStopped<A>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<A, C, Route> InjectEvent<CreationResolved<A>, Here> for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: CreationResolved<A>) -> Self {
        Self::CreationResolved(value)
    }
}
impl<A, C, Route> InjectEvent<WorkerCreationResolved<A::Nonce>, Here>
    for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: WorkerCreationResolved<A::Nonce>) -> Self {
        Self::WorkerCreationResolved(value)
    }
}
impl<A, C, Route> InjectEvent<WorkerStopped<A>, Here> for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: WorkerStopped<A>) -> Self {
        Self::WorkerStopped(value)
    }
}
impl<A, C, Route> InjectEvent<ChildShutdownRejected<A::Nonce>, Here>
    for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: ChildShutdownRejected<A::Nonce>) -> Self {
        Self::ChildShutdownRejected(value)
    }
}

impl<A, C, Route> InjectEvent<ShutdownRequested, Here> for DynamicSupervisorEvent<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}

enum DynamicSupervisorState<N> {
    Running,
    Draining { awaiting: Vec<N> },
}

/// Named effect product for dynamic topology management.
pub struct DynamicSupervisorSends<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    pub outcomes: Route::Sends,
    pub child_observations: InterpreterRequests<ObserveChild<A, behavior::ChildHead>>,
    pub creation_observations: InterpreterRequests<ObserveCreation<A, behavior::ChildHead>>,
    pub shutdowns:
        InterpreterRequests<ShutdownChild<ProxyWithParent<C, ParentPath>, behavior::ChildHead>>,
    pub replacements: Vec<ChildDelivery<Proxy<C>, behavior::ChildHead>>,
}

impl<A, C, Route, ParentPath> SendEffects for DynamicSupervisorSends<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn empty() -> Self {
        Self {
            outcomes: Route::Sends::empty(),
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
            replacements: vec![],
        }
    }
    fn append(&mut self, other: Self) {
        self.outcomes.append(other.outcomes);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.shutdowns.append(other.shutdowns);
        self.replacements.extend(other.replacements);
    }
}

impl<A, C, Route, ParentPath> behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>
    for DynamicSupervisorSends<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    InterpreterRequests<ObserveChild<A, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
    InterpreterRequests<ObserveCreation<A, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
    InterpreterRequests<ShutdownChild<ProxyWithParent<C, ParentPath>, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
{
}

impl<I, RootEvent, Path, A, C, Route, ParentPath> behavior::InterpretSends<I, RootEvent, Path>
    for DynamicSupervisorSends<A, C, Route, ParentPath>
where
    I: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Route::Sends: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveChild<A, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveCreation<A, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<ProxyWithParent<C, ParentPath>, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    Vec<ChildDelivery<Proxy<C>, behavior::ChildHead>>: behavior::InterpretSends<I, RootEvent, Path>,
    DynamicSupervisorSends<A, C, Route, ParentPath>: Send,
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
impl<A, C, Route, ParentPath> SendInput<ObserveChild<A, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn emit(&mut self, value: ObserveChild<A, behavior::ChildHead>) {
        self.child_observations.send(value);
    }
}
impl<A, C, Route, ParentPath> SendInput<ObserveCreation<A, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn emit(&mut self, value: ObserveCreation<A, behavior::ChildHead>) {
        self.creation_observations.send(value);
    }
}
impl<A, C, Route, ParentPath>
    SendInput<ShutdownChild<ProxyWithParent<C, ParentPath>, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn emit(&mut self, value: ShutdownChild<ProxyWithParent<C, ParentPath>, behavior::ChildHead>) {
        self.shutdowns.send(value);
    }
}

/// A pure dynamic supervisor whose stable proxy set changes only through its
/// typed command protocol and committed runtime facts.
pub struct DynamicSupervisorWithParent<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    children: Vec<(A::Nonce, DynamicChild<A, Route>)>,
    state: DynamicSupervisorState<A::Nonce>,
    proxy_parent: ProxyParentIngress<A, ParentPath>,
    marker: core::marker::PhantomData<fn() -> C>,
}

/// A dynamic supervisor whose proxy reports are owned by its direct event layer.
pub type DynamicSupervisor<A, C, Route> = DynamicSupervisorWithParent<A, C, Route, Here>;

impl<A, C, Route, ParentPath> DynamicSupervisorWithParent<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
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
impl<A, C, Route> DynamicSupervisorWithParent<A, C, Route, Here>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    #[must_use]
    pub const fn new() -> Self {
        Self::with_parent(ProxyParentIngress::new())
    }
}
impl<A, C, Route> Default for DynamicSupervisor<A, C, Route>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn default() -> Self {
        Self::new()
    }
}
impl<A, C, Route, ParentPath> crate::BehaviorBase
    for DynamicSupervisorWithParent<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, C, Route, ParentPath> behavior::Protocol
    for DynamicSupervisorWithParent<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Addr = A;
    type Msg = DynamicSupervisorMessage<A, C, Route>;
}

impl<A, C, Route, ParentPath> Behavior for DynamicSupervisorWithParent<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRouteProtocol + Clone,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Protocol = Self;
    type Event = DynamicSupervisorEvent<A, C, Route>;
    type Sends = DynamicSupervisorSends<A, C, Route, ParentPath>;
    type Ph = Never;
    type Error = DynamicSupervisorError<A>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

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
                        sends.outcomes.append(reply_to.deliver(
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
                        sends.outcomes.append(reply_to.deliver(
                            DynamicSupervisorOutcome::StartRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::AlreadyExists,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    if let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        *state = DynamicChild::Installing {
                            reply_to: reply_to.clone(),
                            progress: InitialCreation::AwaitingBoth,
                        };
                    } else {
                        self.children.push((
                            nonce,
                            DynamicChild::Installing {
                                reply_to: reply_to.clone(),
                                progress: InitialCreation::AwaitingBoth,
                            },
                        ));
                    }
                    sends.outcomes.append(
                        reply_to.deliver(DynamicSupervisorOutcome::StartAccepted { nonce }),
                    );
                    let route =
                        ChildRoute::<ProxyWithParent<C, ParentPath>, behavior::ChildHead>::new(
                            nonce,
                        );
                    sends.child_observations.send(ObserveChild::at(route));
                    sends.creation_observations.send(ObserveCreation::at(route));
                    Ok(Actions::new(
                        sends,
                        vec![route.birth(ProxyWithParent::with_parent(child, self.proxy_parent))],
                        crate::Step::Continue,
                    ))
                }
                DynamicSupervisorMessage::Stop { nonce, reply_to } => {
                    if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                        sends.outcomes.append(reply_to.deliver(
                            DynamicSupervisorOutcome::StopRejected {
                                nonce,
                                reason: DynamicSupervisorRejection::ShuttingDown,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    match self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        Some((_, state)) => match state {
                            DynamicChild::Available { worker } => {
                                let worker = *worker;
                                *state = DynamicChild::Stopping {
                                    reply_to: reply_to.clone(),
                                    worker,
                                };
                                let route = ChildRoute::<
                                    ProxyWithParent<C, ParentPath>,
                                    behavior::ChildHead,
                                >::new(nonce);
                                sends.shutdowns.send(ShutdownChild::<
                                    ProxyWithParent<C, ParentPath>,
                                    behavior::ChildHead,
                                >::at(route));
                                sends.outcomes.append(
                                    reply_to
                                        .deliver(DynamicSupervisorOutcome::StopAccepted { nonce }),
                                );
                            }
                            _ => sends.outcomes.append(reply_to.deliver(
                                DynamicSupervisorOutcome::StopRejected {
                                    nonce,
                                    reason: DynamicSupervisorRejection::NotAvailable,
                                },
                            )),
                        },
                        None => sends.outcomes.append(reply_to.deliver(
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
                        sends.outcomes.append(reply_to.deliver(
                            DynamicSupervisorOutcome::ReplaceRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::ShuttingDown,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    match self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        Some((_, state)) => match state {
                            DynamicChild::Available { worker } => {
                                let worker = *worker;
                                *state = match worker {
                                    DynamicWorker::Running { incarnation } => {
                                        DynamicChild::StoppingWorkerForReplacement {
                                            reply_to: reply_to.clone(),
                                            worker: incarnation,
                                        }
                                    }
                                    DynamicWorker::Vacant { last } => {
                                        DynamicChild::CreatingReplacement {
                                            reply_to: reply_to.clone(),
                                            replaces: last,
                                        }
                                    }
                                };
                                let route = ChildRoute::<
                                    ProxyWithParent<C, ParentPath>,
                                    behavior::ChildHead,
                                >::new(nonce);
                                sends
                                    .replacements
                                    .push(ChildDelivery::at(route, ProxyCommand::Replace(child)));
                                sends.outcomes.append(
                                    reply_to.deliver(DynamicSupervisorOutcome::ReplaceAccepted {
                                        nonce,
                                    }),
                                );
                            }
                            _ => sends.outcomes.append(reply_to.deliver(
                                DynamicSupervisorOutcome::ReplaceRejected {
                                    nonce,
                                    child,
                                    reason: DynamicSupervisorRejection::NotAvailable,
                                },
                            )),
                        },
                        None => sends.outcomes.append(reply_to.deliver(
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
                    sends
                        .outcomes
                        .append(reply_to.deliver(DynamicSupervisorOutcome::State {
                            nonce,
                            phase: self.phase(nonce),
                        }));
                    Ok(Actions::send(sends))
                }
            },
            DynamicSupervisorEvent::CreationResolved(resolved) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == resolved.nonce)
                else {
                    return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                };
                if resolved.kind != behavior::CreationKind::Birth {
                    return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                }
                let DynamicChild::Installing { reply_to, progress } = state else {
                    return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                };
                let reply = reply_to.clone();
                match (*progress, resolved.result) {
                    (InitialCreation::AwaitingBoth, Ok(address)) => {
                        *progress = InitialCreation::ProxyCommitted { address };
                        if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                            sends.shutdowns.send(ShutdownChild::<
                                ProxyWithParent<C, ParentPath>,
                                behavior::ChildHead,
                            >::new(resolved.nonce));
                        }
                    }
                    (InitialCreation::AwaitingBoth, Err(reason)) => {
                        *state = DynamicChild::Retired;
                        sends.outcomes.append(reply.deliver(
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
                    (InitialCreation::WorkerResolved { resolved: worker }, Ok(address)) => {
                        if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                            sends.shutdowns.send(ShutdownChild::<
                                ProxyWithParent<C, ParentPath>,
                                behavior::ChildHead,
                            >::new(resolved.nonce));
                        }
                        match worker.result {
                            Ok(()) => {
                                *state = DynamicChild::Available {
                                    worker: DynamicWorker::Running {
                                        incarnation: worker.worker,
                                    },
                                };
                                sends.outcomes.append(reply.deliver(
                                    DynamicSupervisorOutcome::Started {
                                        nonce: resolved.nonce,
                                        child: Recipient::global(address),
                                    },
                                ));
                            }
                            Err(reason) => {
                                *state = DynamicChild::RetiringAfterStartFailure;
                                sends.outcomes.append(reply.deliver(
                                    DynamicSupervisorOutcome::StartFailed {
                                        nonce: resolved.nonce,
                                        reason,
                                    },
                                ));
                                if !matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                                    sends.shutdowns.send(ShutdownChild::<
                                        ProxyWithParent<C, ParentPath>,
                                        behavior::ChildHead,
                                    >::new(
                                        resolved.nonce
                                    ));
                                }
                            }
                        }
                    }
                    (InitialCreation::WorkerResolved { resolved: worker }, Err(_)) => {
                        return Err(DynamicSupervisorError::ContradictoryInitialCreation {
                            proxy: resolved,
                            worker,
                        });
                    }
                    (InitialCreation::ProxyCommitted { .. }, _) => {
                        return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                    }
                }
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::ChildStopped(stopped) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == stopped.nonce)
                else {
                    return Err(DynamicSupervisorError::UnexpectedChildStopped(stopped));
                };
                let reply = match state {
                    DynamicChild::Stopping { reply_to, .. } => Some(reply_to.clone()),
                    DynamicChild::RetiringAfterStartFailure => None,
                    _ if matches!(self.state, DynamicSupervisorState::Draining { .. }) => None,
                    _ => return Err(DynamicSupervisorError::UnexpectedChildStopped(stopped)),
                };
                *state = DynamicChild::Retired;
                if let Some(reply) = reply {
                    sends
                        .outcomes
                        .append(reply.deliver(DynamicSupervisorOutcome::Stopped {
                            nonce: stopped.nonce,
                            outcome: stopped.outcome,
                        }));
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
                    return Err(DynamicSupervisorError::UnexpectedWorkerCreation(resolved));
                };
                if resolved.kind == behavior::CreationKind::Birth {
                    if let DynamicChild::Installing { reply_to, progress } = state {
                        let reply = reply_to.clone();
                        match *progress {
                            InitialCreation::AwaitingBoth => {
                                *progress = InitialCreation::WorkerResolved { resolved };
                            }
                            InitialCreation::ProxyCommitted { address } => match resolved.result {
                                Ok(()) => {
                                    *state = DynamicChild::Available {
                                        worker: DynamicWorker::Running {
                                            incarnation: resolved.worker,
                                        },
                                    };
                                    sends.outcomes.append(reply.deliver(
                                        DynamicSupervisorOutcome::Started {
                                            nonce: resolved.proxy,
                                            child: Recipient::global(address),
                                        },
                                    ));
                                }
                                Err(reason) => {
                                    *state = DynamicChild::RetiringAfterStartFailure;
                                    sends.outcomes.append(reply.deliver(
                                        DynamicSupervisorOutcome::StartFailed {
                                            nonce: resolved.proxy,
                                            reason,
                                        },
                                    ));
                                    if !matches!(
                                        self.state,
                                        DynamicSupervisorState::Draining { .. }
                                    ) {
                                        sends.shutdowns.send(ShutdownChild::<
                                            ProxyWithParent<C, ParentPath>,
                                            behavior::ChildHead,
                                        >::new(
                                            resolved.proxy
                                        ));
                                    }
                                }
                            },
                            InitialCreation::WorkerResolved { .. } => {
                                return Err(DynamicSupervisorError::UnexpectedWorkerCreation(
                                    resolved,
                                ));
                            }
                        }
                        return Ok(Actions::send(sends));
                    }
                }
                match state {
                    DynamicChild::CreatingReplacement { reply_to, replaces }
                        if resolved.kind == behavior::CreationKind::replacement_of(*replaces) =>
                    {
                        let reply = reply_to.clone();
                        let outcome = match resolved.result {
                            Ok(()) => {
                                *state = DynamicChild::Available {
                                    worker: DynamicWorker::Running {
                                        incarnation: resolved.worker,
                                    },
                                };
                                DynamicSupervisorOutcome::Replaced {
                                    nonce: resolved.proxy,
                                }
                            }
                            Err(reason) => {
                                *state = DynamicChild::Available {
                                    worker: DynamicWorker::Vacant { last: *replaces },
                                };
                                DynamicSupervisorOutcome::ReplacementFailed {
                                    nonce: resolved.proxy,
                                    reason,
                                }
                            }
                        };
                        sends.outcomes.append(reply.deliver(outcome));
                    }
                    _ => return Err(DynamicSupervisorError::UnexpectedWorkerCreation(resolved)),
                }
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::WorkerStopped(stopped) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == stopped.proxy)
                else {
                    return Err(DynamicSupervisorError::UnexpectedWorkerStopped(stopped));
                };
                match state {
                    DynamicChild::Available {
                        worker: DynamicWorker::Running { incarnation },
                    } if *incarnation == stopped.worker => {
                        *state = DynamicChild::Available {
                            worker: DynamicWorker::Vacant {
                                last: stopped.worker,
                            },
                        };
                    }
                    DynamicChild::StoppingWorkerForReplacement { reply_to, worker }
                        if *worker == stopped.worker =>
                    {
                        *state = DynamicChild::CreatingReplacement {
                            reply_to: reply_to.clone(),
                            replaces: stopped.worker,
                        };
                    }
                    DynamicChild::Stopping { worker, .. } => match worker {
                        DynamicWorker::Running { incarnation }
                            if *incarnation == stopped.worker =>
                        {
                            *worker = DynamicWorker::Vacant {
                                last: stopped.worker,
                            };
                        }
                        _ => return Err(DynamicSupervisorError::UnexpectedWorkerStopped(stopped)),
                    },
                    _ => return Err(DynamicSupervisorError::UnexpectedWorkerStopped(stopped)),
                }
                Ok(Actions::cont())
            }
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
                        DynamicChild::Installing {
                            progress: InitialCreation::ProxyCommitted { .. },
                            ..
                        } | DynamicChild::Available { .. }
                            | DynamicChild::StoppingWorkerForReplacement { .. }
                            | DynamicChild::CreatingReplacement { .. }
                    ) {
                        sends.shutdowns.send(ShutdownChild::at(ChildRoute::<
                            ProxyWithParent<C, ParentPath>,
                            behavior::ChildHead,
                        >::new(
                            *nonce
                        )));
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
                    return Err(DynamicSupervisorError::UnexpectedChildShutdownRejection(
                        rejected,
                    ));
                };
                if matches!(state, DynamicChild::RetiringAfterStartFailure) {
                    return Err(DynamicSupervisorError::ChildShutdownRejected {
                        nonce: rejected.nonce,
                        reason: rejected.reason,
                    });
                }
                let DynamicChild::Stopping { reply_to, worker } = state else {
                    return Err(DynamicSupervisorError::UnexpectedChildShutdownRejection(
                        rejected,
                    ));
                };
                let reply = reply_to.clone();
                let worker = *worker;
                *state = DynamicChild::Available { worker };
                sends
                    .outcomes
                    .append(reply.deliver(DynamicSupervisorOutcome::StopRejected {
                        nonce: rejected.nonce,
                        reason: DynamicSupervisorRejection::ShutdownRejected(rejected.reason),
                    }));
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
        let initialized = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
        assert_eq!(active.phase(7), Some(DynamicChildPhase::Installing));
        assert!(installed.sends.outcomes.is_empty());

        let worker_installed = active
            .on_path(WorkerCreationResolved::new(
                7,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert_eq!(active.phase(7), Some(DynamicChildPhase::Available));
        assert!(matches!(
            worker_installed.sends.outcomes[0].message,
            DynamicSupervisorOutcome::Started { nonce: 7, child }
                if child.address() == MailAddr(70)
        ));
    }

    #[test]
    fn rejected_start_resolution_retires_the_slot_without_losing_the_observation_request() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
    fn foreign_lifecycle_facts_are_returned_complete_without_creating_state() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
            .initialize()
            .unwrap()
            .behavior;

        let creation = CreationResolved::birth(41, MailAddr(141));
        assert!(matches!(
            active.on_path(creation),
            Err(DynamicSupervisorError::UnexpectedCreation(returned)) if returned == creation
        ));

        let stopped = ChildStopped::new(42, Ok(Exit::Normal), Instant::now());
        assert!(matches!(
            active.on_path(stopped),
            Err(DynamicSupervisorError::UnexpectedChildStopped(returned)) if returned == stopped
        ));

        let replacement =
            WorkerCreationResolved::new(43, 1, CreationKind::replacement_of(0), Ok(()));
        assert!(matches!(
            active.on_path(replacement),
            Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned))
                if returned == replacement
        ));
        assert_eq!(active.phase(41), None);
        assert_eq!(active.phase(42), None);
        assert_eq!(active.phase(43), None);
    }

    #[test]
    fn stop_and_replace_wait_for_their_exact_runtime_fact() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
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
            .on_path(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        let wrong_replacement =
            WorkerCreationResolved::new(3, 4, CreationKind::replacement_of(9), Ok(()));
        assert!(matches!(
            active.on_path(wrong_replacement),
            Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned))
                if returned == wrong_replacement
        ));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Replacing));
        active
            .on_path(WorkerCreationResolved::new(
                3,
                4,
                CreationKind::replacement_of(0),
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
        active
            .on_path(WorkerStopped::new(3, 4, Ok(Exit::Normal), Instant::now()))
            .unwrap();
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
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
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
    fn worker_stop_vacates_only_the_exact_running_incarnation() {
        let initialized = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
        active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        let actions = active
            .on_path(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();

        assert!(actions.sends.outcomes.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, crate::Step::Continue));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));

        let duplicate = WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now());
        assert!(matches!(
            active.on_path(duplicate.clone()),
            Err(DynamicSupervisorError::UnexpectedWorkerStopped(returned))
                if returned == duplicate
        ));
    }

    #[test]
    fn initial_worker_failure_is_reported_and_the_empty_proxy_is_retired() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
        let proxy_committed = active
            .on_path(CreationResolved::birth(3, MailAddr(30)))
            .unwrap();
        assert!(proxy_committed.sends.outcomes.is_empty());

        let failed = active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Err(CreationRejection::EnvironmentFailed),
            ))
            .unwrap();
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Stopping));
        assert_eq!(failed.sends.shutdowns.as_slice(), [ShutdownChild::new(3)]);
        assert!(matches!(
            failed.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartFailed {
                nonce: 3,
                reason: CreationRejection::EnvironmentFailed,
            }
        ));

        let stopped = active
            .on_path(ChildStopped::new(3, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        assert!(stopped.sends.outcomes.is_empty());
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Retired));
    }

    #[test]
    fn wrong_creation_provenance_is_returned_without_advancing_either_stage() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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

        let wrong_proxy =
            CreationResolved::installed(3, CreationKind::replacement_of(9), MailAddr(30));
        assert!(matches!(
            active.on_path(wrong_proxy),
            Err(DynamicSupervisorError::UnexpectedCreation(returned))
                if returned == wrong_proxy
        ));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Installing));
        active
            .on_path(CreationResolved::birth(3, MailAddr(30)))
            .unwrap();

        let wrong_worker =
            WorkerCreationResolved::new(3, 0, CreationKind::replacement_of(9), Ok(()));
        assert!(matches!(
            active.on_path(wrong_worker),
            Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned))
                if returned == wrong_worker
        ));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Installing));
    }

    #[test]
    fn shutdown_drains_established_and_installing_proxies_before_stopping() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Recipient<Reply>>::new()
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
        active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
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
        active
            .on_path(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        let failed_during_drain = active
            .on_path(WorkerCreationResolved::new(
                4,
                0,
                CreationKind::Birth,
                Err(CreationRejection::EnvironmentFailed),
            ))
            .unwrap();
        assert!(failed_during_drain.sends.shutdowns.is_empty());
        assert!(matches!(
            failed_during_drain.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartFailed {
                nonce: 4,
                reason: CreationRejection::EnvironmentFailed,
            }
        ));
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
