//! Typed event and command protocols used by supervision behaviors.

use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ReplacementRequested,
    ReportProxyUnavailable, ShutdownRequested, TimerElapsed, WorkerCreationResolved, WorkerStopped,
};
use crate::{Address, Behavior, BehaviorAddr, BehaviorMessage};
use behavior::{
    ChildHead, ChildInputIngress, ChildReport, ChildRoute, EventIngress, Here, InjectEvent, Inside,
    User, UserEvent,
};

/// Complete mailbox algebra of one stable proxy.
///
/// Domain commands retain the worker's exact public protocol. Fresh-worker
/// requests arrive on a distinct typed lifecycle lane and therefore never
/// fabricate a second logical protocol around the domain message.
pub enum ProxyEvent<C: Behavior<Ph = behavior::Never>> {
    Command(User<BehaviorAddr<C>, BehaviorMessage<C>>),
    WorkerRequested(ReplacementRequested<C>),
    ChildStopped(ChildStopped<BehaviorAddr<C>>),
    CreationResolved(CreationResolved<BehaviorAddr<C>>),
    ShutdownRequested(ShutdownRequested),
    ChildShutdownRejected(ChildShutdownRejected<<BehaviorAddr<C> as Address>::Nonce>),
}

impl<C: Behavior<Ph = behavior::Never>> UserEvent for ProxyEvent<C> {
    type Addr = BehaviorAddr<C>;
    type Message = BehaviorMessage<C>;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Command(event) => Ok(event),
            service => Err(service),
        }
    }
}

impl<C: Behavior<Ph = behavior::Never>> InjectEvent<ReplacementRequested<C>, Here>
    for ProxyEvent<C>
{
    fn inject_at(value: ReplacementRequested<C>) -> Self {
        Self::WorkerRequested(value)
    }
}

impl<C: Behavior<Ph = behavior::Never>> ChildInputIngress<C, ReplacementRequested<C>>
    for ProxyEvent<C>
{
    fn child_input(value: ReplacementRequested<C>) -> Self {
        Self::WorkerRequested(value)
    }
}
impl<C: Behavior<Ph = behavior::Never>> InjectEvent<ChildStopped<BehaviorAddr<C>>, Here>
    for ProxyEvent<C>
{
    fn inject_at(value: ChildStopped<BehaviorAddr<C>>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<C: Behavior<Ph = behavior::Never>> InjectEvent<CreationResolved<BehaviorAddr<C>>, Here>
    for ProxyEvent<C>
{
    fn inject_at(value: CreationResolved<BehaviorAddr<C>>) -> Self {
        Self::CreationResolved(value)
    }
}
impl<C: Behavior<Ph = behavior::Never>> InjectEvent<ShutdownRequested, Here> for ProxyEvent<C> {
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}
impl<C: Behavior<Ph = behavior::Never>>
    InjectEvent<ChildShutdownRejected<<BehaviorAddr<C> as Address>::Nonce>, Here>
    for ProxyEvent<C>
{
    fn inject_at(value: ChildShutdownRejected<<BehaviorAddr<C> as Address>::Nonce>) -> Self {
        Self::ChildShutdownRejected(value)
    }
}

/// Application-facing evidence of one committed supervised-slot lifecycle change.
///
/// This is Bombay policy derived from authoritative creation, termination,
/// restart-admission, and shutdown facts. It is not an actor-model primitive.
/// `Ready` identifies the exact worker incarnation by its creator-local nonce;
/// domain commands still target the stable proxy rather than that replaceable
/// worker directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisionLifecycle<A: Address> {
    /// Both the stable proxy and this exact worker incarnation have committed.
    Ready {
        proxy: A::Nonce,
        worker: A::Nonce,
        kind: behavior::CreationKind<A::Nonce>,
    },
    /// One atomic restart decision made these slots unavailable.
    ///
    /// `trigger` owns the stopped incarnation that caused the decision.
    /// `replacing` retains exact creation facts for other running members
    /// selected by one-for-all or rest-for-one. `awaiting_initial` names
    /// selected slots whose first worker had not committed yet.
    ReplacementStarted {
        trigger: WorkerStopped<A>,
        replacing: Vec<WorkerCreationResolved<A::Nonce>>,
        awaiting_initial: Vec<A::Nonce>,
    },
    /// Restart policy deliberately declined replacement after this stop.
    RetiredAfterStop { stopped: WorkerStopped<A> },
    /// A typed topology failure permanently retired the affected slot.
    Retired {
        failure: crate::SupervisionFailure<A>,
    },
    /// The first accepted shutdown request ended availability for these slots.
    ShuttingDown { proxies: Vec<A::Nonce> },
}

/// Application events plus the complete stable-proxy ownership facts.
pub enum SupervisionEvent<E: UserEvent> {
    Behavior(E),
    ChildStopped(ChildStopped<E::Addr>),
    WorkerStopped(WorkerStopped<E::Addr>),
    CreationResolved(CreationResolved<E::Addr>),
    WorkerCreationResolved(WorkerCreationResolved<<E::Addr as Address>::Nonce>),
    TimerElapsed(TimerElapsed),
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

impl<E, Stable>
    EventIngress<
        ChildRoute<Stable, ChildHead>,
        ChildReport<E::Addr, crate::ReportWorkerStopped<E::Addr>>,
    > for SupervisionEvent<E>
where
    E: UserEvent,
    Stable: Behavior<Protocol: crate::Protocol<Addr = E::Addr>>,
{
    fn ingress(input: ChildReport<E::Addr, crate::ReportWorkerStopped<E::Addr>>) -> Self {
        Self::WorkerStopped(WorkerStopped::from((input.child, input.report)))
    }
}

impl<E, Stable>
    EventIngress<
        ChildRoute<Stable, ChildHead>,
        ChildReport<E::Addr, crate::ReportWorkerCreationResolved<<E::Addr as Address>::Nonce>>,
    > for SupervisionEvent<E>
where
    E: UserEvent,
    Stable: Behavior<Protocol: crate::Protocol<Addr = E::Addr>>,
{
    fn ingress(
        input: ChildReport<
            E::Addr,
            crate::ReportWorkerCreationResolved<<E::Addr as Address>::Nonce>,
        >,
    ) -> Self {
        Self::WorkerCreationResolved(WorkerCreationResolved::from((input.child, input.report)))
    }
}

impl<E, Stable, M>
    EventIngress<
        ChildRoute<Stable, ChildHead>,
        ChildReport<E::Addr, ReportProxyUnavailable<E::Addr, M>>,
    > for SupervisionEvent<E>
where
    E: UserEvent + EventIngress<ChildRoute<Stable, ChildHead>, ProxyUnavailable<E::Addr, M>>,
    Stable: Behavior<Protocol: crate::Protocol<Addr = E::Addr>>,
{
    fn ingress(input: ChildReport<E::Addr, ReportProxyUnavailable<E::Addr, M>>) -> Self {
        Self::Behavior(E::ingress(ProxyUnavailable::from((
            input.child,
            input.report,
        ))))
    }
}

impl<E, Stable, Report>
    EventIngress<ChildRoute<Stable, ChildHead>, ChildReport<E::Addr, ChildReport<E::Addr, Report>>>
    for SupervisionEvent<E>
where
    E: UserEvent
        + EventIngress<
            ChildRoute<Stable, ChildHead>,
            ChildReport<E::Addr, ChildReport<E::Addr, Report>>,
        >,
    Stable: Behavior<Protocol: crate::Protocol<Addr = E::Addr>>,
{
    fn ingress(input: ChildReport<E::Addr, ChildReport<E::Addr, Report>>) -> Self {
        Self::Behavior(E::ingress(input))
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
impl<E: UserEvent> InjectEvent<TimerElapsed, Here> for SupervisionEvent<E> {
    fn inject_at(value: TimerElapsed) -> Self {
        Self::TimerElapsed(value)
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
/// Expected lifecycle unavailability reported through the stable child's
/// established parent relationship. It is not a behavior-fold failure. The
/// complete original communication remains owned by this value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProxyUnavailable<A: Address, M> {
    /// Established creator-local proxy that returned the command.
    pub proxy: A::Nonce,
    /// Original sender carried by the admitted public communication.
    pub from: A,
    /// Exact proxy lifecycle phase that rejected forwarding.
    pub phase: crate::IncarnationPhase<A::Nonce>,
    /// Complete command whose ownership is returned.
    pub command: M,
}

impl<A: Address, M> From<(A::Nonce, ReportProxyUnavailable<A, M>)> for ProxyUnavailable<A, M> {
    fn from((proxy, report): (A::Nonce, ReportProxyUnavailable<A, M>)) -> Self {
        Self {
            proxy,
            from: report.from,
            phase: report.phase,
            command: report.command,
        }
    }
}
