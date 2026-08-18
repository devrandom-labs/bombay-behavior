//! Stable proxy lifecycle and fresh worker incarnation replacement.

use super::super::domain::{
    Incarnation, IncarnationEffects, IncarnationError, IncarnationPhase, IncarnationStopEffects,
};
use super::super::protocol::{ProxyCommand, ProxyEvent};
use crate::protocol::{
    ObserveChild, ObserveCreation, ReportWorkerCreationResolved, ReportWorkerStopped, ShutdownChild,
};
use crate::{Own, SendInput};
use behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, InterpreterRequests, SendEffects, User,
};
use behavior::{Never, Step};

/// The concrete, statically dispatched effect lanes emitted by a [`Proxy`].
pub struct ProxySends<C: Behavior> {
    /// User payloads forwarded to the currently installed worker incarnation.
    pub deliveries: Vec<Delivery<C::Protocol>>,
    /// Requests to observe installed child incarnations.
    pub child_observations: InterpreterRequests<ObserveChild<crate::BehaviorAddr<C>>>,
    /// Requests for exact creation acceptance or rejection facts.
    pub creation_observations: InterpreterRequests<ObserveCreation<crate::BehaviorAddr<C>>>,
    /// Worker-stop facts reported to the owning supervisor.
    pub stopped_reports: InterpreterRequests<ReportWorkerStopped<crate::BehaviorAddr<C>>>,
    /// Creation-resolution facts reported to the owning supervisor.
    pub creation_reports: InterpreterRequests<
        ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>,
    >,
    /// Requests orderly termination of the exact installed worker incarnation.
    pub shutdowns: InterpreterRequests<ShutdownChild<C>>,
}

pub(crate) type ProxyActions<C> = Actions<crate::BehaviorAddr<C>, Never, ProxySends<C>, Births<C>>;

impl<C: Behavior> SendEffects for ProxySends<C> {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            stopped_reports: InterpreterRequests::empty(),
            creation_reports: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
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
    InterpreterRequests<ObserveChild<crate::BehaviorAddr<C>>>: behavior::SendsFor<Event>,
    InterpreterRequests<ObserveCreation<crate::BehaviorAddr<C>>>: behavior::SendsFor<Event>,
    InterpreterRequests<ShutdownChild<C>>: behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, C> behavior::InterpretSends<I, RootEvent, Path> for ProxySends<C>
where
    I: behavior::SendInterpreter,
    C: Behavior<Ph = Never>,
    Vec<Delivery<C::Protocol>>: behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveChild<crate::BehaviorAddr<C>>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ObserveCreation<crate::BehaviorAddr<C>>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ReportWorkerStopped<crate::BehaviorAddr<C>>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<C>>: behavior::InterpretSends<I, RootEvent, Path>,
    ProxySends<C>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.deliveries, interpreter).await?;
            behavior::InterpretSends::interpret(self.child_observations, interpreter).await?;
            behavior::InterpretSends::interpret(self.creation_observations, interpreter).await?;
            behavior::InterpretSends::interpret(self.stopped_reports, interpreter).await?;
            behavior::InterpretSends::interpret(self.creation_reports, interpreter).await?;
            behavior::InterpretSends::interpret(self.shutdowns, interpreter).await
        }
    }
}

impl<C: Behavior> SendInput<Delivery<C::Protocol>, Own> for ProxySends<C> {
    fn emit(&mut self, input: Delivery<C::Protocol>) {
        self.deliveries.push(input);
    }
}

impl<C: Behavior> SendInput<ObserveChild<crate::BehaviorAddr<C>>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ObserveChild<crate::BehaviorAddr<C>>) {
        self.child_observations.send(input);
    }
}

impl<C: Behavior> SendInput<ObserveCreation<crate::BehaviorAddr<C>>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ObserveCreation<crate::BehaviorAddr<C>>) {
        self.creation_observations.send(input);
    }
}

impl<C: Behavior> SendInput<ReportWorkerStopped<crate::BehaviorAddr<C>>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ReportWorkerStopped<crate::BehaviorAddr<C>>) {
        self.stopped_reports.send(input);
    }
}

impl<C: Behavior>
    SendInput<ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>, Own>
    for ProxySends<C>
{
    fn emit(
        &mut self,
        input: ReportWorkerCreationResolved<<crate::BehaviorAddr<C> as Address>::Nonce>,
    ) {
        self.creation_reports.send(input);
    }
}

impl<C: Behavior> SendInput<ShutdownChild<C>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ShutdownChild<C>) {
        self.shutdowns.send(input);
    }
}

/// A stable actor that serializes fresh worker-incarnation installation and
/// orderly termination of its owned worker subtree.
///
/// A worker is routable only in `Running`. Deadline most one creation can be
/// `Installing`; stale or provenance-mismatched results are inert. Rejection
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
        let mut sends = ProxySends::empty();
        let creates = match effects {
            IncarnationEffects::None => Vec::new(),
            IncarnationEffects::Create(creation) => {
                sends
                    .child_observations
                    .extend([ObserveChild::new(creation.attempt)]);
                sends
                    .creation_observations
                    .extend([ObserveCreation::new(creation.attempt)]);
                vec![Create::new(creation.attempt, creation.child, creation.kind)]
            }
            IncarnationEffects::Deliver {
                incarnation,
                message,
            } => {
                sends.deliveries.push(Delivery::local_child(
                    behavior::ChildRecipient::new(incarnation),
                    message,
                ));
                Vec::new()
            }
            IncarnationEffects::Report(resolved) => {
                sends.creation_reports.extend([resolved.into()]);
                Vec::new()
            }
            IncarnationEffects::ReportAndShutdown {
                resolved,
                incarnation,
            } => {
                sends.creation_reports.extend([resolved.into()]);
                sends.shutdowns.send(ShutdownChild::new(incarnation));
                Vec::new()
            }
            IncarnationEffects::ReportAndStop(resolved) => {
                sends.creation_reports.extend([resolved.into()]);
                return Actions::new(sends, Vec::new(), Step::Stop(behavior::Stopped));
            }
            IncarnationEffects::Shutdown(incarnation) => {
                sends.shutdowns.send(ShutdownChild::new(incarnation));
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
        let mut actions = Self::actions(
            effects
                .creation
                .map_or(IncarnationEffects::None, IncarnationEffects::Create),
        );
        if let Some(incarnation) = effects.stopped {
            actions
                .sends
                .stopped_reports
                .extend([ReportWorkerStopped::from(crate::ChildStopped::new(
                    incarnation,
                    event.outcome,
                    event.at,
                ))]);
        }
        actions
    }
}

impl<C> behavior::Protocol for Proxy<C>
where
    C: Behavior<Ph = Never>,
    <crate::BehaviorAddr<C> as Address>::Nonce: From<u64>,
{
    type Addr = crate::BehaviorAddr<C>;
    type Msg = ProxyCommand<C>;
}

impl<C> Behavior for Proxy<C>
where
    C: Behavior<Ph = Never>,
    <crate::BehaviorAddr<C> as Address>::Nonce: From<u64>,
{
    type Protocol = Self;
    type Event = ProxyEvent<User<crate::BehaviorAddr<C>, ProxyCommand<C>>>;
    type Sends = ProxySends<C>;
    type Ph = Never;
    type Error = IncarnationError;
    type Birth = Births<C>;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<Actions<crate::BehaviorAddr<C>, Never, Self::Sends, Births<C>>, IncarnationError>
    {
        let effects = self.incarnation.initialize()?;
        Ok(Self::actions(effects))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<Actions<crate::BehaviorAddr<C>, Never, Self::Sends, Births<C>>, IncarnationError>
    {
        Ok(match event {
            ProxyEvent::CreationResolved(resolved) => Self::actions(
                self.incarnation
                    .creation_resolved(resolved.nonce, resolved.kind, resolved.result),
            ),
            ProxyEvent::ChildStopped(event) => {
                let completes_shutdown = self.incarnation.shutdown_complete_after(event.nonce);
                let effects = self.incarnation.child_stopped(event.nonce)?;
                let mut actions = Self::stopped_actions(effects, event);
                if completes_shutdown {
                    actions.become_ = Step::Stop(behavior::Stopped);
                }
                actions
            }
            ProxyEvent::ShutdownRequested(_) => Self::actions(self.incarnation.shutdown()),
            ProxyEvent::ChildShutdownRejected(rejected) => Self::actions(
                self.incarnation
                    .shutdown_rejected(rejected.nonce, rejected.reason)?,
            ),
            ProxyEvent::Command(event) => match event.message {
                ProxyCommand::Forward(message) => {
                    let effects = self.incarnation.forward(message);
                    Self::actions(effects)
                }
                ProxyCommand::Replace(child) => {
                    let effects = self.incarnation.replace(child)?;
                    Self::actions(effects)
                }
            },
        })
    }
}
