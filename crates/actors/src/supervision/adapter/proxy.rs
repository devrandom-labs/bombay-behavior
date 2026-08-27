//! Stable proxy lifecycle and fresh worker incarnation replacement.

use super::super::domain::{
    Incarnation, IncarnationEffects, IncarnationError, IncarnationPhase, IncarnationShutdownError,
    IncarnationStopEffects, IncarnationStopError,
};
use super::super::protocol::ProxyEvent;
use crate::protocol::{
    ObserveChild, ObserveCreation, ReportProxyUnavailable, ReportWorkerCreationResolved,
    ReportWorkerStopped, ShutdownChild,
};
use crate::{Own, SendInput};
use behavior::{
    Actions, Address, Behavior, Births, ChildDelivery, ChildRoute, InterpreterRequests, SendEffects,
};
use behavior::{Never, ReportToParent, Step};

/// The concrete, statically dispatched effect lanes emitted by a [`Proxy`].
pub struct ProxySends<C: Behavior<Ph = Never>> {
    /// User payloads forwarded to the currently installed worker incarnation.
    pub deliveries: Vec<ChildDelivery<C::Protocol, behavior::ChildHead>>,
    /// Expected command rejections reported through the established parent.
    pub unavailable_reports: InterpreterRequests<
        ReportToParent<ReportProxyUnavailable<crate::BehaviorAddr<C>, crate::BehaviorMessage<C>>>,
    >,
    /// Requests to observe installed child incarnations.
    pub child_observations:
        InterpreterRequests<ObserveChild<crate::BehaviorAddr<C>, behavior::ChildHead>>,
    /// Requests for exact creation acceptance or rejection facts.
    pub creation_observations:
        InterpreterRequests<ObserveCreation<crate::BehaviorAddr<C>, behavior::ChildHead>>,
    /// Worker-stop facts reported to the owning supervisor.
    pub stopped_reports:
        InterpreterRequests<ReportToParent<ReportWorkerStopped<crate::BehaviorAddr<C>>>>,
    /// Creation-resolution facts reported to the owning supervisor.
    pub creation_reports: InterpreterRequests<
        ReportToParent<ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>>,
    >,
    /// Requests orderly termination of the exact installed worker incarnation.
    pub shutdowns: InterpreterRequests<ShutdownChild<C, behavior::ChildHead>>,
}

pub(crate) type ProxyActions<C> = Actions<crate::BehaviorAddr<C>, Never, ProxySends<C>, Births<C>>;

/// Controlled failure of one stable-proxy transition.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProxyError<A: Address, C> {
    /// The worker-incarnation lifecycle rejected the transition.
    #[error(transparent)]
    Lifecycle(#[from] IncarnationError),
    /// A creation fact does not match the one pending worker attempt.
    #[error("the worker creation fact does not match the pending incarnation")]
    UnexpectedCreation {
        phase: IncarnationPhase<A::Nonce>,
        observed: crate::CreationResolved<A>,
    },
    /// A child-stop fact does not name the worker incarnation currently owned.
    #[error("the worker stop fact does not match the current incarnation")]
    UnexpectedChildStopped {
        phase: IncarnationPhase<A::Nonce>,
        observed: crate::ChildStopped<A>,
    },
    /// A shutdown rejection does not belong to the outstanding worker shutdown.
    #[error("the child-shutdown rejection does not match an outstanding request")]
    UnexpectedChildShutdownRejection {
        phase: IncarnationPhase<A::Nonce>,
        observed: crate::ChildShutdownRejected<A::Nonce>,
    },
    /// The exact outstanding worker-shutdown request was rejected.
    #[error("the worker shutdown request was rejected")]
    ChildShutdownRejected(crate::ChildShutdownRejected<A::Nonce>),
    /// A replacement could not receive a fresh incarnation nonce; ownership
    /// of the uninstalled behavior is returned.
    #[error("the stable proxy could not start the replacement")]
    ReplacementNotAccepted {
        phase: IncarnationPhase<A::Nonce>,
        reason: IncarnationError,
        replacement: C,
    },
}

impl<A: Address, C> core::fmt::Debug for ProxyError<A, C>
where
    A: core::fmt::Debug,
    A::Nonce: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Lifecycle(error) => formatter.debug_tuple("Lifecycle").field(error).finish(),
            Self::UnexpectedCreation { phase, observed } => formatter
                .debug_struct("UnexpectedCreation")
                .field("phase", phase)
                .field("observed", observed)
                .finish(),
            Self::UnexpectedChildStopped { phase, observed } => formatter
                .debug_struct("UnexpectedChildStopped")
                .field("phase", phase)
                .field("observed", observed)
                .finish(),
            Self::UnexpectedChildShutdownRejection { phase, observed } => formatter
                .debug_struct("UnexpectedChildShutdownRejection")
                .field("phase", phase)
                .field("observed", observed)
                .finish(),
            Self::ChildShutdownRejected(observed) => formatter
                .debug_tuple("ChildShutdownRejected")
                .field(observed)
                .finish(),
            Self::ReplacementNotAccepted { phase, reason, .. } => formatter
                .debug_struct("ReplacementNotAccepted")
                .field("phase", phase)
                .field("reason", reason)
                .field("replacement", &"<owned behavior>")
                .finish(),
        }
    }
}

impl<C> SendEffects for ProxySends<C>
where
    C: Behavior<Ph = Never>,
{
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            unavailable_reports: InterpreterRequests::empty(),
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            stopped_reports: InterpreterRequests::empty(),
            creation_reports: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.unavailable_reports.append(other.unavailable_reports);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.stopped_reports.append(other.stopped_reports);
        self.creation_reports.append(other.creation_reports);
        self.shutdowns.append(other.shutdowns);
    }
}

impl<C, Event> behavior::SendsFor<Event> for ProxySends<C>
where
    C: Behavior<Ph = Never>,
    InterpreterRequests<ObserveChild<crate::BehaviorAddr<C>, behavior::ChildHead>>:
        behavior::SendsFor<Event>,
    InterpreterRequests<ObserveCreation<crate::BehaviorAddr<C>, behavior::ChildHead>>:
        behavior::SendsFor<Event>,
    InterpreterRequests<ShutdownChild<C, behavior::ChildHead>>: behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, C> behavior::InterpretSends<I, RootEvent, Path> for ProxySends<C>
where
    I: behavior::SendInterpreter,
    C: Behavior<Ph = Never>,
    Vec<ChildDelivery<C::Protocol, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<
        ReportToParent<ReportProxyUnavailable<crate::BehaviorAddr<C>, crate::BehaviorMessage<C>>>,
    >: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveChild<crate::BehaviorAddr<C>, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveCreation<crate::BehaviorAddr<C>, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ReportToParent<ReportWorkerStopped<crate::BehaviorAddr<C>>>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<
        ReportToParent<ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>>,
    >: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<C, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    ProxySends<C>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.deliveries, interpreter).await?;
            behavior::InterpretSends::interpret(self.unavailable_reports, interpreter).await?;
            behavior::InterpretSends::interpret(self.child_observations, interpreter).await?;
            behavior::InterpretSends::interpret(self.creation_observations, interpreter).await?;
            behavior::InterpretSends::interpret(self.stopped_reports, interpreter).await?;
            behavior::InterpretSends::interpret(self.creation_reports, interpreter).await?;
            behavior::InterpretSends::interpret(self.shutdowns, interpreter).await
        }
    }
}

impl<C> SendInput<ChildDelivery<C::Protocol, behavior::ChildHead>, Own> for ProxySends<C>
where
    C: Behavior<Ph = Never>,
{
    fn emit(&mut self, input: ChildDelivery<C::Protocol, behavior::ChildHead>) {
        self.deliveries.push(input);
    }
}

impl<C: Behavior<Ph = Never>>
    SendInput<ObserveChild<crate::BehaviorAddr<C>, behavior::ChildHead>, Own> for ProxySends<C>
{
    fn emit(&mut self, input: ObserveChild<crate::BehaviorAddr<C>, behavior::ChildHead>) {
        self.child_observations.send(input);
    }
}

impl<C: Behavior<Ph = Never>>
    SendInput<ObserveCreation<crate::BehaviorAddr<C>, behavior::ChildHead>, Own> for ProxySends<C>
{
    fn emit(&mut self, input: ObserveCreation<crate::BehaviorAddr<C>, behavior::ChildHead>) {
        self.creation_observations.send(input);
    }
}

impl<C: Behavior<Ph = Never>>
    SendInput<ReportToParent<ReportWorkerStopped<crate::BehaviorAddr<C>>>, Own> for ProxySends<C>
{
    fn emit(&mut self, input: ReportToParent<ReportWorkerStopped<crate::BehaviorAddr<C>>>) {
        self.stopped_reports.send(input);
    }
}

impl<C: Behavior<Ph = Never>>
    SendInput<
        ReportToParent<ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>>,
        Own,
    > for ProxySends<C>
{
    fn emit(
        &mut self,
        input: ReportToParent<
            ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>,
        >,
    ) {
        self.creation_reports.send(input);
    }
}

impl<C: Behavior<Ph = Never>> SendInput<ShutdownChild<C, behavior::ChildHead>, Own>
    for ProxySends<C>
{
    fn emit(&mut self, input: ShutdownChild<C, behavior::ChildHead>) {
        self.shutdowns.send(input);
    }
}

/// A stable actor that serializes fresh worker-incarnation installation and
/// orderly termination of its owned worker subtree.
///
/// A worker is routable only in `Running`. Deadline most one creation can be
/// `Installing`; stale or provenance-mismatched results are returned intact as
/// typed errors. Rejection
/// leaves `last_installed` unchanged, so a later attempt still names the last
/// incarnation that actually existed.
///
/// On typed shutdown, an installed worker receives one [`ShutdownChild`]
/// request and the proxy stops only after the matching child-stop fact. If
/// creation is unresolved, the proxy first waits for its exact resolution.
/// Shutdown rejection is a typed [`IncarnationError`] and never fabricates a
/// successful child termination.
pub struct Proxy<C: Behavior<Ph = Never>> {
    incarnation: Incarnation<<crate::BehaviorAddr<C> as Address>::Nonce, C>,
}

impl<C> crate::BehaviorBase for Proxy<C>
where
    C: Behavior<Ph = Never>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<C: Behavior<Ph = Never>> Proxy<C> {
    #[must_use]
    pub fn new(worker: C) -> Self {
        Self {
            incarnation: Incarnation::new(worker),
        }
    }

    #[must_use]
    pub const fn phase(&self) -> IncarnationPhase<<crate::BehaviorAddr<C> as Address>::Nonce> {
        self.incarnation.phase()
    }
}

impl<C> Proxy<C>
where
    C: Behavior<Ph = Never>,
    crate::BehaviorAddr<C>: Address,
    <crate::BehaviorAddr<C> as Address>::Nonce: From<u64>,
{
    fn actions(
        effects: IncarnationEffects<
            <crate::BehaviorAddr<C> as Address>::Nonce,
            C,
            crate::BehaviorMessage<C>,
            crate::BehaviorAddr<C>,
        >,
    ) -> ProxyActions<C> {
        let mut sends = ProxySends::<C>::empty();
        let creates = match effects {
            IncarnationEffects::None => Vec::new(),
            IncarnationEffects::Create(creation) => {
                let route = ChildRoute::<C, behavior::ChildHead>::new(creation.attempt);
                sends.child_observations.extend([ObserveChild::at(route)]);
                sends
                    .creation_observations
                    .extend([ObserveCreation::at(route)]);
                vec![route.stage(creation.child, creation.kind)]
            }
            IncarnationEffects::Deliver {
                incarnation,
                message,
            } => {
                let route = ChildRoute::<C, behavior::ChildHead>::new(incarnation);
                sends.deliveries.extend(
                    <ChildRoute<C, behavior::ChildHead> as crate::DeliveryRouteFor<Self>>::deliver_for(
                        route, message,
                    ),
                );
                Vec::new()
            }
            IncarnationEffects::Report(resolved) => {
                sends.creation_reports.extend([ReportToParent::new(
                    ReportWorkerCreationResolved::new(
                        resolved.nonce,
                        resolved.kind,
                        resolved.result.map(|_| ()),
                    ),
                )]);
                Vec::new()
            }
            IncarnationEffects::ReportAndShutdown {
                resolved,
                incarnation,
            } => {
                sends.creation_reports.extend([ReportToParent::new(
                    ReportWorkerCreationResolved::new(
                        resolved.nonce,
                        resolved.kind,
                        resolved.result.map(|_| ()),
                    ),
                )]);
                sends.shutdowns.send(ShutdownChild::at(
                    ChildRoute::<C, behavior::ChildHead>::new(incarnation),
                ));
                Vec::new()
            }
            IncarnationEffects::ReportAndStop(resolved) => {
                sends.creation_reports.extend([ReportToParent::new(
                    ReportWorkerCreationResolved::new(
                        resolved.nonce,
                        resolved.kind,
                        resolved.result.map(|_| ()),
                    ),
                )]);
                return Actions::new(sends, Vec::new(), Step::Stop(behavior::Stopped));
            }
            IncarnationEffects::Shutdown(incarnation) => {
                sends.shutdowns.send(ShutdownChild::at(
                    ChildRoute::<C, behavior::ChildHead>::new(incarnation),
                ));
                Vec::new()
            }
            IncarnationEffects::Stop => {
                return Actions::new(sends, Vec::new(), Step::Stop(behavior::Stopped));
            }
        };
        Actions::new(sends, creates, Step::Continue)
    }

    fn stopped_actions(
        effects: IncarnationStopEffects<<crate::BehaviorAddr<C> as Address>::Nonce, C>,
        event: crate::ChildStopped<crate::BehaviorAddr<C>>,
    ) -> ProxyActions<C> {
        match effects {
            IncarnationStopEffects::Stopped { incarnation } => {
                Self::reported_stop_actions(IncarnationEffects::None, incarnation, event)
            }
            IncarnationStopEffects::StoppedAndCreate {
                incarnation,
                creation,
            } => Self::reported_stop_actions(
                IncarnationEffects::Create(creation),
                incarnation,
                event,
            ),
        }
    }

    fn reported_stop_actions(
        effect: IncarnationEffects<
            <crate::BehaviorAddr<C> as Address>::Nonce,
            C,
            crate::BehaviorMessage<C>,
            crate::BehaviorAddr<C>,
        >,
        incarnation: <crate::BehaviorAddr<C> as Address>::Nonce,
        event: crate::ChildStopped<crate::BehaviorAddr<C>>,
    ) -> ProxyActions<C> {
        let mut actions = Self::actions(effect);
        actions
            .sends
            .stopped_reports
            .extend([ReportToParent::new(ReportWorkerStopped::new(
                incarnation,
                event.outcome,
                event.at,
            ))]);
        actions
    }
}

impl<C> Behavior for Proxy<C>
where
    C: Behavior<Ph = Never>,
    <crate::BehaviorAddr<C> as Address>::Nonce: From<u64>,
{
    type Protocol = C::Protocol;
    type Event = ProxyEvent<C>;
    type Sends = ProxySends<C>;
    type Ph = Never;
    type Error = ProxyError<crate::BehaviorAddr<C>, C>;
    type Birth = Births<C>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        let effects = self.incarnation.initialize()?;
        Ok(Self::actions(effects))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        Ok(match event {
            ProxyEvent::CreationResolved(resolved) => {
                let phase = self.incarnation.phase();
                let effects = self
                    .incarnation
                    .creation_resolved(resolved.nonce, resolved.kind, resolved.result)
                    .map_err(|observed| ProxyError::UnexpectedCreation { phase, observed })?;
                Self::actions(effects)
            }
            ProxyEvent::ChildStopped(event) => {
                let phase = self.incarnation.phase();
                let completes_shutdown = self.incarnation.shutdown_complete_after(event.nonce);
                let effects = self
                    .incarnation
                    .child_stopped(event.nonce)
                    .map_err(|error| match error {
                        IncarnationStopError::Unexpected(_) => ProxyError::UnexpectedChildStopped {
                            phase,
                            observed: event,
                        },
                        IncarnationStopError::Lifecycle(error) => ProxyError::Lifecycle(error),
                    })?;
                let mut actions = Self::stopped_actions(effects, event);
                if completes_shutdown {
                    actions.become_ = Step::Stop(behavior::Stopped);
                }
                actions
            }
            ProxyEvent::ShutdownRequested(_) => Self::actions(self.incarnation.shutdown()),
            ProxyEvent::ChildShutdownRejected(rejected) => {
                let phase = self.incarnation.phase();
                let effects = self
                    .incarnation
                    .shutdown_rejected(rejected.nonce, rejected.reason)
                    .map_err(|error| match error {
                        IncarnationShutdownError::Unexpected { .. } => {
                            ProxyError::UnexpectedChildShutdownRejection {
                                phase,
                                observed: rejected,
                            }
                        }
                        IncarnationShutdownError::Rejected(_) => {
                            ProxyError::ChildShutdownRejected(rejected)
                        }
                    })?;
                Self::actions(effects)
            }
            ProxyEvent::Command(event) => match self.incarnation.forward(event.message) {
                Ok(effects) => Self::actions(effects),
                Err((phase, command)) => {
                    let mut actions = Self::actions(IncarnationEffects::None);
                    actions.sends.unavailable_reports.send(ReportToParent::new(
                        ReportProxyUnavailable::new(event.from, phase, command),
                    ));
                    actions
                }
            },
            ProxyEvent::WorkerRequested(requested) => {
                let phase = self.incarnation.phase();
                let effects = self.incarnation.replace(requested.worker).map_err(
                    |(reason, replacement)| ProxyError::ReplacementNotAccepted {
                        phase,
                        reason,
                        replacement,
                    },
                )?;
                Self::actions(effects)
            }
        })
    }
}
