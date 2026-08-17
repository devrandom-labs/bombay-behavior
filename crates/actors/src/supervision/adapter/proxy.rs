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
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, ServiceSends,
    User,
};
use behavior::{Never, Step};

/// The concrete, statically dispatched effect lanes emitted by a [`Proxy`].
pub struct ProxySends<C: Behavior> {
    /// User payloads forwarded to the currently installed worker incarnation.
    pub deliveries: Vec<Delivery<C>>,
    /// Requests to observe installed child incarnations.
    pub child_observations: ServiceSends<ObserveChild<<C::Addr as Address>::Nonce>>,
    /// Requests for exact creation acceptance or rejection facts.
    pub creation_observations: ServiceSends<ObserveCreation<<C::Addr as Address>::Nonce>>,
    /// Worker-stop facts reported to the owning supervisor.
    pub stopped_reports: ServiceSends<ReportWorkerStopped<C::Addr>>,
    /// Creation-resolution facts reported to the owning supervisor.
    pub creation_reports: ServiceSends<ReportWorkerCreationResolved<<C::Addr as Address>::Nonce>>,
    /// Requests orderly termination of the exact installed worker incarnation.
    pub shutdowns: ServiceSends<ShutdownChild<C>>,
}

pub(crate) type ProxyActions<C> = Actions<<C as Behavior>::Addr, Never, ProxySends<C>, Births<C>>;

impl<C: Behavior> SendAlgebra for ProxySends<C> {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            child_observations: ServiceSends::empty(),
            creation_observations: ServiceSends::empty(),
            stopped_reports: ServiceSends::empty(),
            creation_reports: ServiceSends::empty(),
            shutdowns: ServiceSends::empty(),
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

impl<C: Behavior> SendInput<Delivery<C>, Own> for ProxySends<C> {
    fn emit(&mut self, input: Delivery<C>) {
        self.deliveries.push(input);
    }
}

impl<C: Behavior> SendInput<ObserveChild<<C::Addr as Address>::Nonce>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ObserveChild<<C::Addr as Address>::Nonce>) {
        self.child_observations.send(input);
    }
}

impl<C: Behavior> SendInput<ObserveCreation<<C::Addr as Address>::Nonce>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ObserveCreation<<C::Addr as Address>::Nonce>) {
        self.creation_observations.send(input);
    }
}

impl<C: Behavior> SendInput<ReportWorkerStopped<C::Addr>, Own> for ProxySends<C> {
    fn emit(&mut self, input: ReportWorkerStopped<C::Addr>) {
        self.stopped_reports.send(input);
    }
}

impl<C: Behavior> SendInput<ReportWorkerCreationResolved<<C::Addr as Address>::Nonce>, Own>
    for ProxySends<C>
{
    fn emit(&mut self, input: ReportWorkerCreationResolved<<C::Addr as Address>::Nonce>) {
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
    incarnation: Incarnation<<C::Addr as Address>::Nonce, C>,
}

impl<C: Behavior<Ph = Never>> Proxy<C> {
    #[must_use]
    pub fn new(worker: C) -> Self {
        Self {
            incarnation: Incarnation::new(worker),
        }
    }

    #[must_use]
    pub const fn phase(&self) -> IncarnationPhase<<C::Addr as Address>::Nonce> {
        self.incarnation.phase()
    }
}

impl<C> Proxy<C>
where
    C: Behavior<Ph = Never>,
    C::Addr: Address,
    <C::Addr as Address>::Nonce: From<u64>,
{
    fn actions(
        effects: IncarnationEffects<<C::Addr as Address>::Nonce, C, C::Msg>,
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
                sends
                    .deliveries
                    .push(Delivery::new(Recipient::child(incarnation), message));
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
        effects: IncarnationStopEffects<<C::Addr as Address>::Nonce, C>,
        event: crate::ChildStopped<C::Addr>,
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

impl<C> Behavior for Proxy<C>
where
    C: Behavior<Ph = Never>,
    <C::Addr as Address>::Nonce: From<u64>,
{
    type Addr = C::Addr;
    type Msg = ProxyCommand<C>;
    type Event = ProxyEvent<User<C::Addr, ProxyCommand<C>>>;
    type Sends = ProxySends<C>;
    type Ph = Never;
    type Error = IncarnationError;
    type Birth = Births<C>;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, IncarnationError> {
        let effects = self.incarnation.initialize()?;
        Ok(Self::actions(effects))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, IncarnationError> {
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
