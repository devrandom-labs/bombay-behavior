//! Explicitly managed dynamic stable-child topology.

use crate::{
    ChildShutdownRejected, ChildStopped, CreationRejection, DeliveryRoute, ObserveChild,
    ObserveEstablishedCreation, Own, ProxyUnavailable, ReplacementRequested,
    ReportProxyUnavailable, ReportWorkerCreationResolved, ReportWorkerStopped, SendInput,
    ShutdownChild, ShutdownRequested, WorkerCreationResolved, WorkerStopped,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorLayer, Births, ChildInput,
    ChildInputIngress, ChildReport, ChildRoute, EndpointAddress, EstablishedCreation,
    EstablishedRecipient, EventIngress, Here, InjectEvent, InterpreterRequests, Never, Protocol,
    SendEffects, User, UserEvent,
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
#[derive(thiserror::Error)]
pub enum DynamicSupervisorError<A, C>
where
    A: EndpointAddress,
    C: Behavior<Protocol: Protocol<Addr = A>>,
{
    /// The runtime rejected orderly shutdown of an owned stable proxy.
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: A::Nonce,
        reason: crate::ChildShutdownRejection,
    },
    /// A stable-proxy creation fact did not match an installing slot.
    #[error("stable-proxy creation fact does not match an installing child")]
    UnexpectedCreation(EstablishedCreation<C::Protocol, behavior::ChildHead>),
    /// A stable-proxy stop fact did not match a stopping or draining slot.
    #[error("stable-proxy stop fact does not match a stopping child")]
    UnexpectedChildStopped(ChildStopped<A>),
    /// A worker-creation fact did not match an explicit replacement.
    #[error("worker-creation fact does not match a replacing child")]
    UnexpectedWorkerCreation(WorkerCreationResolved<A::Nonce>),
    /// A worker fact exists for a proxy creation the interpreter rejected.
    #[error("worker creation was reported for a rejected stable-proxy creation")]
    ContradictoryInitialCreation {
        proxy: EstablishedCreation<C::Protocol, behavior::ChildHead>,
        worker: WorkerCreationResolved<A::Nonce>,
    },
    /// A worker-stop fact named no stable proxy owned by this supervisor.
    #[error("worker-stop fact names an unknown stable proxy")]
    UnexpectedWorkerStopped(WorkerStopped<A>),
    /// An unavailable-command report named no established owned stable child.
    #[error("unavailable-command report names no established owned stable child")]
    UnexpectedCommandUnavailable(ProxyUnavailable<A, crate::BehaviorMessage<C>>),
    /// A shutdown rejection did not match a pending stop or drain.
    #[error("child-shutdown rejection does not match a pending shutdown")]
    UnexpectedChildShutdownRejection(ChildShutdownRejected<A::Nonce>),
}

impl<A, C> core::fmt::Debug for DynamicSupervisorError<A, C>
where
    A: EndpointAddress,
    C: Behavior<Protocol: Protocol<Addr = A>>,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::ChildShutdownRejected { .. } => "ChildShutdownRejected",
            Self::UnexpectedCreation(_) => "UnexpectedCreation",
            Self::UnexpectedChildStopped(_) => "UnexpectedChildStopped",
            Self::UnexpectedWorkerCreation(_) => "UnexpectedWorkerCreation",
            Self::ContradictoryInitialCreation { .. } => "ContradictoryInitialCreation",
            Self::UnexpectedWorkerStopped(_) => "UnexpectedWorkerStopped",
            Self::UnexpectedCommandUnavailable(_) => "UnexpectedCommandUnavailable",
            Self::UnexpectedChildShutdownRejection(_) => "UnexpectedChildShutdownRejection",
        })
    }
}

/// Commands for an explicitly managed dynamic child set.
pub enum DynamicSupervisorMessage<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
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

/// Stable public identity of the dynamic-management command protocol.
///
/// The stable-child construction layer is deliberately absent: changing the
/// internal incarnation composition cannot change capabilities held by
/// management clients.
pub struct DynamicSupervisorProtocol<A, C, Route>(core::marker::PhantomData<fn(A, C, Route)>);

impl<A, C, Route> Protocol for DynamicSupervisorProtocol<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    type Addr = A;
    type Msg = DynamicSupervisorMessage<A, C, Route>;
}

/// Complete command or realization outcome returned to a typed recipient.
pub enum DynamicSupervisorOutcome<A, C>
where
    A: EndpointAddress,
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
        child: EstablishedRecipient<C::Protocol>,
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
    /// A command admitted by an established stable child could not be
    /// forwarded to a live incarnation. The start owner receives the exact
    /// sender, command, and lifecycle phase and can retry or reject it without
    /// reconstructing payload state.
    CommandUnavailable {
        nonce: A::Nonce,
        from: A,
        phase: crate::IncarnationPhase<A::Nonce>,
        command: crate::BehaviorMessage<C>,
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

enum InitialCreation<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    AwaitingBoth,
    ProxyCommitted {
        recipient: EstablishedRecipient<P>,
    },
    WorkerResolved {
        resolved: WorkerCreationResolved<<P::Addr as Address>::Nonce>,
    },
}

enum DynamicChild<A, P, Route>
where
    A: EndpointAddress,
    P: Protocol<Addr = A>,
{
    Installing {
        reply_to: Route,
        progress: InitialCreation<P>,
    },
    Available {
        worker: DynamicWorker<A::Nonce>,
        events_to: Route,
    },
    Stopping {
        reply_to: Route,
        events_to: Route,
        worker: DynamicWorker<A::Nonce>,
    },
    RetiringAfterStartFailure {
        events_to: Route,
    },
    StoppingWorkerForReplacement {
        reply_to: Route,
        events_to: Route,
        worker: A::Nonce,
    },
    CreatingReplacement {
        reply_to: Route,
        events_to: Route,
        replaces: A::Nonce,
    },
    Retired,
}

impl<A, P, Route> DynamicChild<A, P, Route>
where
    A: EndpointAddress,
    P: Protocol<Addr = A>,
{
    const fn phase(&self) -> DynamicChildPhase {
        match self {
            Self::Installing { .. } => DynamicChildPhase::Installing,
            Self::Available { .. } => DynamicChildPhase::Available,
            Self::Stopping { .. } | Self::RetiringAfterStartFailure { .. } => {
                DynamicChildPhase::Stopping
            }
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
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    Command(User<A, DynamicSupervisorMessage<A, C, Route>>),
    ChildStopped(ChildStopped<A>),
    CreationResolved(EstablishedCreation<C::Protocol, behavior::ChildHead>),
    WorkerCreationResolved(WorkerCreationResolved<A::Nonce>),
    WorkerStopped(WorkerStopped<A>),
    CommandUnavailable(ProxyUnavailable<A, crate::BehaviorMessage<C>>),
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<A::Nonce>),
}

impl<A, C, Route> UserEvent for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
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
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: ChildStopped<A>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<A, C, Route> InjectEvent<EstablishedCreation<C::Protocol, behavior::ChildHead>, Here>
    for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: EstablishedCreation<C::Protocol, behavior::ChildHead>) -> Self {
        Self::CreationResolved(value)
    }
}
impl<A, C, Route> InjectEvent<WorkerCreationResolved<A::Nonce>, Here>
    for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: WorkerCreationResolved<A::Nonce>) -> Self {
        Self::WorkerCreationResolved(value)
    }
}
impl<A, C, Route> InjectEvent<WorkerStopped<A>, Here> for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: WorkerStopped<A>) -> Self {
        Self::WorkerStopped(value)
    }
}
impl<A, C, Route> InjectEvent<ChildShutdownRejected<A::Nonce>, Here>
    for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: ChildShutdownRejected<A::Nonce>) -> Self {
        Self::ChildShutdownRejected(value)
    }
}

impl<A, C, Route> InjectEvent<ShutdownRequested, Here> for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}

impl<A, C, Route, Stable>
    behavior::EventIngress<
        ChildRoute<Stable, behavior::ChildHead>,
        ChildReport<A, ReportWorkerStopped<A>>,
    > for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Protocol = C::Protocol>,
{
    fn ingress(input: ChildReport<A, ReportWorkerStopped<A>>) -> Self {
        Self::WorkerStopped(WorkerStopped::from((input.child, input.report)))
    }
}

impl<A, C, Route, Stable>
    behavior::EventIngress<
        ChildRoute<Stable, behavior::ChildHead>,
        ChildReport<A, ReportWorkerCreationResolved<A::Nonce>>,
    > for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Protocol = C::Protocol>,
{
    fn ingress(input: ChildReport<A, ReportWorkerCreationResolved<A::Nonce>>) -> Self {
        Self::WorkerCreationResolved(WorkerCreationResolved::from((input.child, input.report)))
    }
}

impl<A, C, Route, Stable>
    EventIngress<
        ChildRoute<Stable, behavior::ChildHead>,
        ChildReport<A, ReportProxyUnavailable<A, crate::BehaviorMessage<C>>>,
    > for DynamicSupervisorEvent<A, C, Route>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Protocol = C::Protocol>,
{
    fn ingress(
        input: ChildReport<A, ReportProxyUnavailable<A, crate::BehaviorMessage<C>>>,
    ) -> Self {
        Self::CommandUnavailable(ProxyUnavailable::from((input.child, input.report)))
    }
}

enum DynamicSupervisorState<N> {
    Running,
    Draining { awaiting: Vec<N> },
}

/// Named effect product for dynamic topology management.
pub struct DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    pub outcomes: Route::Sends,
    pub child_observations: InterpreterRequests<ObserveChild<A, behavior::ChildHead>>,
    pub creation_observations:
        InterpreterRequests<ObserveEstablishedCreation<C::Protocol, behavior::ChildHead>>,
    pub shutdowns: InterpreterRequests<ShutdownChild<Stable, behavior::ChildHead>>,
    pub replacement_inputs:
        Vec<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>>,
}

impl<A, C, Route, Stable> SendEffects for DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    fn empty() -> Self {
        Self {
            outcomes: Route::Sends::empty(),
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
            replacement_inputs: Vec::new(),
        }
    }
    fn append(&mut self, mut other: Self) {
        self.outcomes.append(other.outcomes);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.shutdowns.append(other.shutdowns);
        self.replacement_inputs
            .append(&mut other.replacement_inputs);
    }
}

impl<A, C, Route, Stable> behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>
    for DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    InterpreterRequests<ObserveChild<A, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
    InterpreterRequests<ObserveEstablishedCreation<C::Protocol, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
    InterpreterRequests<ShutdownChild<Stable, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
    Vec<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>>:
        behavior::SendsFor<DynamicSupervisorEvent<A, C, Route>>,
{
}

impl<I, RootEvent, Path, A, C, Route, Stable> behavior::InterpretSends<I, RootEvent, Path>
    for DynamicSupervisorSends<A, C, Route, Stable>
where
    I: behavior::SendInterpreter,
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    Route::Sends: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveChild<A, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveEstablishedCreation<C::Protocol, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<Stable, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    Vec<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    DynamicSupervisorSends<A, C, Route, Stable>: Send,
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
            behavior::InterpretSends::interpret(self.replacement_inputs, interpreter).await
        }
    }
}
impl<A, C, Route, Stable> SendInput<ObserveChild<A, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    fn emit(&mut self, value: ObserveChild<A, behavior::ChildHead>) {
        self.child_observations.send(value);
    }
}
impl<A, C, Route, Stable>
    SendInput<ObserveEstablishedCreation<C::Protocol, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    fn emit(&mut self, value: ObserveEstablishedCreation<C::Protocol, behavior::ChildHead>) {
        self.creation_observations.send(value);
    }
}
impl<A, C, Route, Stable> SendInput<ShutdownChild<Stable, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    fn emit(&mut self, value: ShutdownChild<Stable, behavior::ChildHead>) {
        self.shutdowns.send(value);
    }
}
impl<A, C, Route, Stable>
    SendInput<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>, Own>
    for DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    fn emit(&mut self, value: ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>) {
        self.replacement_inputs.push(value);
    }
}

/// A pure dynamic supervisor whose stable proxy set changes only through its
/// typed command protocol and committed runtime facts.
pub struct DynamicSupervisor<A, C, Route, L>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    children: Vec<(A::Nonce, DynamicChild<A, C::Protocol, Route>)>,
    state: DynamicSupervisorState<A::Nonce>,
    layer: L,
}

impl<A, C, Route, L> DynamicSupervisor<A, C, Route, L>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    #[must_use]
    pub const fn new(layer: L) -> Self {
        Self {
            children: vec![],
            state: DynamicSupervisorState::Running,
            layer,
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
impl<A, C, Route, L> crate::BehaviorBase for DynamicSupervisor<A, C, Route, L>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, C, Route, L> Behavior for DynamicSupervisor<A, C, Route, L>
where
    A: EndpointAddress,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Route: DeliveryRoute + Clone,
    Route::Protocol: behavior::Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    type Protocol = DynamicSupervisorProtocol<A, C, Route>;
    type Event = DynamicSupervisorEvent<A, C, Route>;
    type Sends = DynamicSupervisorSends<A, C, Route, L::Output>;
    type Ph = Never;
    type Error = DynamicSupervisorError<A, C>;
    type Birth = Births<L::Output>;

    #[allow(
        clippy::collapsible_if,
        clippy::too_many_lines,
        reason = "separate provenance and state guards keep the exhaustive dynamic-child fold inspectable"
    )]
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
                    let route = ChildRoute::<L::Output, behavior::ChildHead>::new(nonce);
                    sends.child_observations.send(ObserveChild::at(route));
                    sends
                        .creation_observations
                        .send(ObserveEstablishedCreation::at(route));
                    Ok(Actions::new(
                        sends,
                        vec![route.birth(self.layer.layer(child))],
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
                            DynamicChild::Available { worker, events_to } => {
                                let worker = *worker;
                                let events_to = events_to.clone();
                                *state = DynamicChild::Stopping {
                                    reply_to: reply_to.clone(),
                                    events_to,
                                    worker,
                                };
                                let route =
                                    ChildRoute::<L::Output, behavior::ChildHead>::new(nonce);
                                sends.shutdowns.send(
                                    ShutdownChild::<L::Output, behavior::ChildHead>::at(route),
                                );
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
                            DynamicChild::Available { worker, events_to } => {
                                let worker = *worker;
                                let events_to = events_to.clone();
                                *state = match worker {
                                    DynamicWorker::Running { incarnation } => {
                                        DynamicChild::StoppingWorkerForReplacement {
                                            reply_to: reply_to.clone(),
                                            events_to,
                                            worker: incarnation,
                                        }
                                    }
                                    DynamicWorker::Vacant { last } => {
                                        DynamicChild::CreatingReplacement {
                                            reply_to: reply_to.clone(),
                                            events_to,
                                            replaces: last,
                                        }
                                    }
                                };
                                let route =
                                    ChildRoute::<L::Output, behavior::ChildHead>::new(nonce);
                                sends
                                    .replacement_inputs
                                    .push(ChildInput::at(route, ReplacementRequested::new(child)));
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
                let nonce = resolved.nonce();
                let kind = resolved.kind();
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == nonce) else {
                    return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                };
                if kind != behavior::CreationKind::Birth {
                    return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                }
                let DynamicChild::Installing { reply_to, progress } = state else {
                    return Err(DynamicSupervisorError::UnexpectedCreation(resolved));
                };
                let reply = reply_to.clone();
                match progress {
                    InitialCreation::AwaitingBoth => match resolved {
                        EstablishedCreation::Installed { recipient, .. } => {
                            *progress = InitialCreation::ProxyCommitted { recipient };
                            if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                                sends.shutdowns.send(
                                    ShutdownChild::<L::Output, behavior::ChildHead>::new(nonce),
                                );
                            }
                        }
                        EstablishedCreation::Rejected { reason, .. } => {
                            *state = DynamicChild::Retired;
                            sends.outcomes.append(
                                reply.deliver(DynamicSupervisorOutcome::StartFailed {
                                    nonce,
                                    reason,
                                }),
                            );
                            if let DynamicSupervisorState::Draining { awaiting } = &mut self.state {
                                awaiting.retain(|awaiting_nonce| *awaiting_nonce != nonce);
                                if awaiting.is_empty() {
                                    return Ok(Actions::new(
                                        sends,
                                        Vec::new(),
                                        crate::Step::Stop(behavior::Stopped),
                                    ));
                                }
                            }
                        }
                    },
                    InitialCreation::WorkerResolved { resolved: worker } => match resolved {
                        EstablishedCreation::Installed { recipient, .. } => {
                            if matches!(self.state, DynamicSupervisorState::Draining { .. }) {
                                sends.shutdowns.send(
                                    ShutdownChild::<L::Output, behavior::ChildHead>::new(nonce),
                                );
                            }
                            match worker.result {
                                Ok(()) => {
                                    *state = DynamicChild::Available {
                                        worker: DynamicWorker::Running {
                                            incarnation: worker.worker,
                                        },
                                        events_to: reply.clone(),
                                    };
                                    sends.outcomes.append(reply.deliver(
                                        DynamicSupervisorOutcome::Started {
                                            nonce,
                                            child: recipient,
                                        },
                                    ));
                                }
                                Err(reason) => {
                                    *state = DynamicChild::RetiringAfterStartFailure {
                                        events_to: reply.clone(),
                                    };
                                    sends.outcomes.append(reply.deliver(
                                        DynamicSupervisorOutcome::StartFailed { nonce, reason },
                                    ));
                                    if !matches!(
                                        self.state,
                                        DynamicSupervisorState::Draining { .. }
                                    ) {
                                        sends.shutdowns.send(ShutdownChild::<
                                            L::Output,
                                            behavior::ChildHead,
                                        >::new(
                                            nonce
                                        ));
                                    }
                                }
                            }
                        }
                        EstablishedCreation::Rejected { reason, .. } => {
                            return Err(DynamicSupervisorError::ContradictoryInitialCreation {
                                proxy: EstablishedCreation::rejected(nonce, kind, reason),
                                worker: *worker,
                            });
                        }
                    },
                    InitialCreation::ProxyCommitted { .. } => {
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
                    DynamicChild::RetiringAfterStartFailure { .. } => None,
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
                        match progress {
                            InitialCreation::AwaitingBoth => {
                                *progress = InitialCreation::WorkerResolved { resolved };
                            }
                            InitialCreation::ProxyCommitted { recipient } => {
                                match resolved.result {
                                    Ok(()) => {
                                        let recipient = recipient.clone();
                                        *state = DynamicChild::Available {
                                            worker: DynamicWorker::Running {
                                                incarnation: resolved.worker,
                                            },
                                            events_to: reply.clone(),
                                        };
                                        sends.outcomes.append(reply.deliver(
                                            DynamicSupervisorOutcome::Started {
                                                nonce: resolved.proxy,
                                                child: recipient,
                                            },
                                        ));
                                    }
                                    Err(reason) => {
                                        *state = DynamicChild::RetiringAfterStartFailure {
                                            events_to: reply.clone(),
                                        };
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
                                            L::Output,
                                            behavior::ChildHead,
                                        >::new(
                                            resolved.proxy
                                        ));
                                        }
                                    }
                                }
                            }
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
                    DynamicChild::CreatingReplacement {
                        reply_to,
                        events_to,
                        replaces,
                    } if resolved.kind == behavior::CreationKind::replacement_of(*replaces) => {
                        let reply = reply_to.clone();
                        let events_to = events_to.clone();
                        let outcome = match resolved.result {
                            Ok(()) => {
                                *state = DynamicChild::Available {
                                    worker: DynamicWorker::Running {
                                        incarnation: resolved.worker,
                                    },
                                    events_to,
                                };
                                DynamicSupervisorOutcome::Replaced {
                                    nonce: resolved.proxy,
                                }
                            }
                            Err(reason) => {
                                *state = DynamicChild::Available {
                                    worker: DynamicWorker::Vacant { last: *replaces },
                                    events_to,
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
                        events_to,
                    } if *incarnation == stopped.worker => {
                        let events_to = events_to.clone();
                        *state = DynamicChild::Available {
                            worker: DynamicWorker::Vacant {
                                last: stopped.worker,
                            },
                            events_to,
                        };
                    }
                    DynamicChild::StoppingWorkerForReplacement {
                        reply_to,
                        events_to,
                        worker,
                    } if *worker == stopped.worker => {
                        *state = DynamicChild::CreatingReplacement {
                            reply_to: reply_to.clone(),
                            events_to: events_to.clone(),
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
            DynamicSupervisorEvent::CommandUnavailable(unavailable) => {
                let Some((_, state)) = self
                    .children
                    .iter()
                    .find(|(nonce, _)| *nonce == unavailable.proxy)
                else {
                    return Err(DynamicSupervisorError::UnexpectedCommandUnavailable(
                        unavailable,
                    ));
                };
                let events_to = match state {
                    DynamicChild::Installing {
                        reply_to: events_to,
                        ..
                    }
                    | DynamicChild::Available { events_to, .. }
                    | DynamicChild::Stopping { events_to, .. }
                    | DynamicChild::RetiringAfterStartFailure { events_to }
                    | DynamicChild::StoppingWorkerForReplacement { events_to, .. }
                    | DynamicChild::CreatingReplacement { events_to, .. } => events_to,
                    _ => {
                        return Err(DynamicSupervisorError::UnexpectedCommandUnavailable(
                            unavailable,
                        ));
                    }
                };
                sends.outcomes.append(events_to.clone().deliver(
                    DynamicSupervisorOutcome::CommandUnavailable {
                        nonce: unavailable.proxy,
                        from: unavailable.from,
                        phase: unavailable.phase,
                        command: unavailable.command,
                    },
                ));
                Ok(Actions::send(sends))
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
                            L::Output,
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
                if matches!(state, DynamicChild::RetiringAfterStartFailure { .. }) {
                    return Err(DynamicSupervisorError::ChildShutdownRejected {
                        nonce: rejected.nonce,
                        reason: rejected.reason,
                    });
                }
                let DynamicChild::Stopping {
                    reply_to,
                    events_to,
                    worker,
                } = state
                else {
                    return Err(DynamicSupervisorError::UnexpectedChildShutdownRejection(
                        rejected,
                    ));
                };
                let reply = reply_to.clone();
                let events_to = events_to.clone();
                let worker = *worker;
                *state = DynamicChild::Available { worker, events_to };
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
    use core::marker::PhantomData;
    use std::time::Instant;

    use super::*;
    use crate::{Activate as _, CreationKind, Exit};
    use behavior::{EstablishedRecipient, NoBirths, Recipient};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestAddr(u64);

    impl Address for TestAddr {
        type Nonce = u64;
    }

    struct TestEndpoint<P>(u64, PhantomData<fn() -> P>);

    impl<P> Clone for TestEndpoint<P> {
        fn clone(&self) -> Self {
            Self(self.0, PhantomData)
        }
    }

    impl EndpointAddress for TestAddr {
        type Established<P>
            = TestEndpoint<P>
        where
            P: Protocol<Addr = Self>;
    }

    fn creation(
        nonce: u64,
        kind: CreationKind<u64>,
        endpoint: u64,
    ) -> EstablishedCreation<Worker, behavior::ChildHead> {
        EstablishedCreation::installed(
            nonce,
            kind,
            EstablishedRecipient::issued(TestEndpoint(endpoint, PhantomData)),
        )
    }

    struct EndpointId;

    impl behavior::InterpretEstablished<Worker> for EndpointId {
        type Output = u64;

        fn interpret_established(&mut self, endpoint: TestEndpoint<Worker>) -> Self::Output {
            endpoint.0
        }
    }

    struct Worker;
    impl behavior::Protocol for Worker {
        type Addr = TestAddr;
        type Msg = u8;
    }

    impl Behavior for Worker {
        type Protocol = Self;
        type Event = User<TestAddr, u8>;
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
        type Addr = TestAddr;
        type Msg = DynamicSupervisorOutcome<TestAddr, Worker>;
    }

    impl Behavior for Reply {
        type Protocol = Self;
        type Event = User<TestAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn reply() -> Recipient<Reply> {
        Recipient::global(TestAddr(99))
    }

    fn dynamic_protocol<B>(behavior: B) -> B
    where
        B: Behavior,
        B::Protocol: Protocol<
                Addr = TestAddr,
                Msg = DynamicSupervisorMessage<TestAddr, Worker, Recipient<Reply>>,
            >,
    {
        behavior
    }

    macro_rules! assert_action_counts {
        (
            $actions:expr;
            outcomes = $outcomes:expr,
            child_observations = $child_observations:expr,
            creation_observations = $creation_observations:expr,
            shutdowns = $shutdowns:expr,
            replacement_inputs = $replacement_inputs:expr,
            creates = $creates:expr,
            become = $become:pat_param
        ) => {{
            let actions = &$actions;
            assert_eq!(actions.sends.outcomes.len(), $outcomes);
            assert_eq!(
                actions.sends.child_observations.as_slice().len(),
                $child_observations
            );
            assert_eq!(
                actions.sends.creation_observations.as_slice().len(),
                $creation_observations
            );
            assert_eq!(actions.sends.shutdowns.as_slice().len(), $shutdowns);
            assert_eq!(actions.sends.replacement_inputs.len(), $replacement_inputs);
            assert_eq!(actions.creates.len(), $creates);
            assert!(matches!(&actions.become_, $become));
        }};
    }

    #[test]
    fn start_is_distinct_from_committed_installation_and_duplicate_returns_child() {
        let initialized = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        let accepted = active
            .receive(
                TestAddr(1),
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
        assert_eq!(accepted.sends.creation_observations.as_slice().len(), 1);
        assert_eq!(accepted.sends.creation_observations.as_slice()[0].nonce, 7);
        assert!(matches!(
            accepted.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartAccepted { nonce: 7 }
        ));

        let duplicate = active
            .receive(
                TestAddr(1),
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
            .on_path(creation(7, CreationKind::Birth, 70))
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
        match worker_installed
            .sends
            .outcomes
            .into_iter()
            .next()
            .unwrap()
            .message
        {
            DynamicSupervisorOutcome::Started { nonce: 7, child } => {
                assert_eq!(child.interpret(&mut EndpointId), 70);
            }
            _ => panic!("the joined creation must emit Started"),
        }
    }

    #[test]
    fn rejected_start_resolution_retires_the_slot_without_losing_the_observation_request() {
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        let start = active
            .receive(
                TestAddr(1),
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
        assert_eq!(start.sends.creation_observations.as_slice().len(), 1);
        assert_eq!(start.sends.creation_observations.as_slice()[0].nonce, 5);

        let rejected = active
            .on_path(
                EstablishedCreation::<Worker, behavior::ChildHead>::rejected(
                    5,
                    CreationKind::Birth,
                    CreationRejection::EnvironmentFailed,
                ),
            )
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
    fn unavailable_commands_return_to_the_start_owner_in_every_live_slot_phase() {
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 1,
            creation_observations = 1,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 1,
            become = crate::Step::Continue
        );

        let installing = ProxyUnavailable {
            proxy: 7,
            from: TestAddr(41),
            phase: crate::IncarnationPhase::Installing {
                attempt: 0,
                kind: CreationKind::Birth,
            },
            command: 11,
        };
        let installing_actions = active
            .transition(DynamicSupervisorEvent::CommandUnavailable(installing))
            .unwrap();
        assert!(installing_actions.creates.is_empty());
        assert!(installing_actions.sends.child_observations.is_empty());
        assert!(installing_actions.sends.creation_observations.is_empty());
        assert!(installing_actions.sends.shutdowns.is_empty());
        assert!(installing_actions.sends.replacement_inputs.is_empty());
        assert!(matches!(
            installing_actions.sends.outcomes.as_slice(),
            [behavior::Delivery {
                message: DynamicSupervisorOutcome::CommandUnavailable {
                    nonce: 7,
                    from: TestAddr(41),
                    phase: crate::IncarnationPhase::Installing {
                        attempt: 0,
                        kind: CreationKind::Birth,
                    },
                    command: 11,
                },
                ..
            }]
        ));

        assert_action_counts!(active
            .on_path(creation(7, CreationKind::Birth, 70))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(WorkerCreationResolved::new(
                7,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        let running_actions = active
            .transition(DynamicSupervisorEvent::CommandUnavailable(
                ProxyUnavailable {
                    proxy: 7,
                    from: TestAddr(42),
                    phase: crate::IncarnationPhase::Running { incarnation: 0 },
                    command: 12,
                },
            ))
            .unwrap();
        assert!(matches!(
            running_actions.sends.outcomes[0].message,
            DynamicSupervisorOutcome::CommandUnavailable {
                nonce: 7,
                from: TestAddr(42),
                phase: crate::IncarnationPhase::Running { incarnation: 0 },
                command: 12,
            }
        ));

        assert_action_counts!(active
            .receive(
                TestAddr(2),
                DynamicSupervisorMessage::Replace {
                    nonce: 7,
                    child: Worker,
                    reply_to: Recipient::global(TestAddr(98)),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 1,
            creates = 0,
            become = crate::Step::Continue
        );
        let replacing_actions = active
            .transition(DynamicSupervisorEvent::CommandUnavailable(
                ProxyUnavailable {
                    proxy: 7,
                    from: TestAddr(43),
                    phase: crate::IncarnationPhase::AwaitingStop { incarnation: 0 },
                    command: 13,
                },
            ))
            .unwrap();
        assert_eq!(
            replacing_actions.sends.outcomes[0].to.address(),
            TestAddr(99)
        );
        assert!(matches!(
            replacing_actions.sends.outcomes[0].message,
            DynamicSupervisorOutcome::CommandUnavailable { command: 13, .. }
        ));

        let stale = ProxyUnavailable {
            proxy: 99,
            from: TestAddr(44),
            phase: crate::IncarnationPhase::Vacant {
                last_installed: None,
            },
            command: 14,
        };
        assert!(matches!(
            active.transition(DynamicSupervisorEvent::CommandUnavailable(stale.clone())),
            Err(DynamicSupervisorError::UnexpectedCommandUnavailable(returned))
                if returned == stale
        ));
    }

    #[test]
    fn foreign_lifecycle_facts_are_returned_complete_without_creating_state() {
        let mut active = dynamic_protocol(DynamicSupervisor::new(|worker: Worker| {
            crate::Proxy::new(worker)
        }))
        .initialize()
        .unwrap()
        .behavior;

        match active.on_path(creation(41, CreationKind::Birth, 141)) {
            Err(DynamicSupervisorError::UnexpectedCreation(returned)) => {
                assert_eq!(returned.nonce(), 41);
                assert_eq!(returned.kind(), CreationKind::Birth);
                assert_eq!(
                    returned
                        .into_recipient()
                        .unwrap()
                        .interpret(&mut EndpointId),
                    141
                );
            }
            _ => panic!("the foreign exact creation must be returned"),
        }

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
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 1,
            creation_observations = 1,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 1,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(creation(3, CreationKind::Birth, 30))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );

        let replacing = active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Replace {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(replacing.sends.replacement_inputs.len(), 1);
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Replacing));
        assert_action_counts!(active
            .on_path(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        let wrong_replacement =
            WorkerCreationResolved::new(3, 4, CreationKind::replacement_of(9), Ok(()));
        assert!(matches!(
            active.on_path(wrong_replacement),
            Err(DynamicSupervisorError::UnexpectedWorkerCreation(returned))
                if returned == wrong_replacement
        ));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Replacing));
        assert_action_counts!(active
            .on_path(WorkerCreationResolved::new(
                3,
                4,
                CreationKind::replacement_of(0),
                Ok(()),
            ))
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));

        let stopping = active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Stop {
                    nonce: 3,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(stopping.sends.shutdowns.as_slice(), [ShutdownChild::new(3)]);
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Stopping));
        assert_action_counts!(active
            .on_path(WorkerStopped::new(3, 4, Ok(Exit::Normal), Instant::now()))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
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
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 1,
            creation_observations = 1,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 1,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(creation(3, CreationKind::Birth, 30))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Stop {
                    nonce: 3,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 1,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );

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
        let initialized = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 1,
            creation_observations = 1,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 1,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(creation(3, CreationKind::Birth, 30))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
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
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 1,
            creation_observations = 1,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 1,
            become = crate::Step::Continue
        );
        let proxy_committed = active
            .on_path(creation(3, CreationKind::Birth, 30))
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
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        assert_action_counts!(active
            .receive(
                TestAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
            outcomes = 1,
            child_observations = 1,
            creation_observations = 1,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 1,
            become = crate::Step::Continue
        );

        match active.on_path(creation(3, CreationKind::replacement_of(9), 30)) {
            Err(DynamicSupervisorError::UnexpectedCreation(returned)) => {
                assert_eq!(returned.nonce(), 3);
                assert_eq!(returned.kind(), CreationKind::replacement_of(9));
                assert_eq!(
                    returned
                        .into_recipient()
                        .unwrap()
                        .interpret(&mut EndpointId),
                    30
                );
            }
            _ => panic!("the wrong-provenance exact creation must be returned"),
        }
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Installing));
        assert_action_counts!(active
            .on_path(creation(3, CreationKind::Birth, 30))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );

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
        let mut active = DynamicSupervisor::new(|worker: Worker| crate::Proxy::new(worker))
            .initialize()
            .unwrap()
            .behavior;
        for nonce in [3, 4] {
            assert_action_counts!(active
                .receive(
                    TestAddr(1),
                    DynamicSupervisorMessage::Start {
                        nonce,
                        child: Worker,
                        reply_to: reply(),
                    },
                )
                .unwrap();
                outcomes = 1,
                child_observations = 1,
                creation_observations = 1,
                shutdowns = 0,
                replacement_inputs = 0,
                creates = 1,
                become = crate::Step::Continue
            );
        }
        assert_action_counts!(active
            .on_path(creation(3, CreationKind::Birth, 30))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
        assert_action_counts!(active
            .on_path(WorkerCreationResolved::new(
                3,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            outcomes = 1,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );

        let requested = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(
            requested.sends.shutdowns.as_slice(),
            [ShutdownChild::new(3)]
        );
        assert!(matches!(requested.become_, crate::Step::Continue));

        let installed_during_drain = active
            .on_path(creation(4, CreationKind::Birth, 40))
            .unwrap();
        assert_eq!(
            installed_during_drain.sends.shutdowns.as_slice(),
            [ShutdownChild::new(4)]
        );
        assert_action_counts!(active
            .on_path(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();
            outcomes = 0,
            child_observations = 0,
            creation_observations = 0,
            shutdowns = 0,
            replacement_inputs = 0,
            creates = 0,
            become = crate::Step::Continue
        );
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
