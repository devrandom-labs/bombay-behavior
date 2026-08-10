//! Stable proxy lifecycle and fresh worker incarnation replacement.

use super::super::domain::{Incarnation, IncarnationEffects, IncarnationPhase, IncarnationReport};
use super::super::protocol::{ProxyCommand, ProxyEvent};
use crate::behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, ServiceSends,
    User,
};
use crate::next::{Never, Step};
use crate::protocol::{
    ObserveChild, ObserveCreation, ReportWorkerCreationResolved, ReportWorkerStopped,
};
use crate::{Own, SendInput};

/// The concrete, statically dispatched effect lanes emitted by a [`Proxy`].
pub struct ProxySends<A: Address, M> {
    pub deliveries: Vec<Delivery<A, M>>,
    pub child_observations: ServiceSends<ObserveChild<A::Nonce>>,
    pub creation_observations: ServiceSends<ObserveCreation<A::Nonce>>,
    pub stopped_reports: ServiceSends<ReportWorkerStopped<A>>,
    pub creation_reports: ServiceSends<ReportWorkerCreationResolved<A::Nonce>>,
}

pub type ProxyActions<C> = Actions<
    <C as Behavior>::Addr,
    Never,
    ProxySends<<C as Behavior>::Addr, <C as Behavior>::Msg>,
    Births<C>,
>;

impl<A: Address, M> SendAlgebra for ProxySends<A, M> {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            child_observations: ServiceSends::empty(),
            creation_observations: ServiceSends::empty(),
            stopped_reports: ServiceSends::empty(),
            creation_reports: ServiceSends::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.stopped_reports.append(other.stopped_reports);
        self.creation_reports.append(other.creation_reports);
    }
}

impl<A: Address, M> SendInput<Delivery<A, M>, Own> for ProxySends<A, M> {
    fn emit(&mut self, input: Delivery<A, M>) {
        self.deliveries.push(input);
    }
}

impl<A: Address, M> SendInput<ObserveChild<A::Nonce>, Own> for ProxySends<A, M> {
    fn emit(&mut self, input: ObserveChild<A::Nonce>) {
        self.child_observations.send(input);
    }
}

impl<A: Address, M> SendInput<ObserveCreation<A::Nonce>, Own> for ProxySends<A, M> {
    fn emit(&mut self, input: ObserveCreation<A::Nonce>) {
        self.creation_observations.send(input);
    }
}

impl<A: Address, M> SendInput<ReportWorkerStopped<A>, Own> for ProxySends<A, M> {
    fn emit(&mut self, input: ReportWorkerStopped<A>) {
        self.stopped_reports.send(input);
    }
}

impl<A: Address, M> SendInput<ReportWorkerCreationResolved<A::Nonce>, Own> for ProxySends<A, M> {
    fn emit(&mut self, input: ReportWorkerCreationResolved<A::Nonce>) {
        self.creation_reports.send(input);
    }
}

/// A stable actor that serializes fresh worker-incarnation installation.
///
/// A worker is routable only in `Running`. Deadline most one creation can be
/// `Installing`; stale or provenance-mismatched results are inert. Rejection
/// leaves `last_installed` unchanged, so a later attempt still names the last
/// incarnation that actually existed.
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
        stopped: Option<&crate::ChildStopped<C::Addr>>,
    ) -> ProxyActions<C> {
        let mut sends = ProxySends::empty();
        if let Some((incarnation, message)) = effects.delivery {
            sends
                .deliveries
                .push(Delivery::new(Recipient::child(incarnation), message));
        }
        if let Some(report) = effects.report {
            match report {
                IncarnationReport::CreationResolved(resolved) => {
                    sends.creation_reports.extend([resolved.into()]);
                }
                IncarnationReport::Stopped { incarnation } => {
                    let event = stopped.expect("stop report originates from child stop input");
                    sends.stopped_reports.extend([ReportWorkerStopped::from(
                        crate::ChildStopped::new(incarnation, event.outcome, event.at),
                    )]);
                }
            }
        }
        let creates = effects.creation.map_or_else(Vec::new, |creation| {
            sends
                .child_observations
                .extend([ObserveChild::new(creation.attempt)]);
            sends
                .creation_observations
                .extend([ObserveCreation::new(creation.attempt)]);
            vec![Create::new(creation.attempt, creation.child, creation.kind)]
        });
        Actions::new(sends, creates, Step::Continue)
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
    type Sends = ProxySends<C::Addr, C::Msg>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<C>;

    fn init(&mut self) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, Never> {
        let effects = self
            .incarnation
            .initialize()
            .expect("a proxy initializes once");
        Ok(Self::actions(effects, None))
    }

    fn transition(
        &mut self,
        event: Self::Event,
    ) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, Never> {
        Ok(match event {
            ProxyEvent::CreationResolved(resolved) => Self::actions(
                self.incarnation
                    .creation_resolved(resolved.nonce, resolved.kind, resolved.result),
                None,
            ),
            ProxyEvent::ChildStopped(event) => {
                let effects = self.incarnation.child_stopped(event.nonce);
                Self::actions(effects, Some(&event))
            }
            ProxyEvent::Inner(event) => match event.message {
                ProxyCommand::Forward(message) => {
                    let effects = self.incarnation.forward(message);
                    Self::actions(effects, None)
                }
                ProxyCommand::Replace(child) => {
                    let effects = self.incarnation.replace(child);
                    Self::actions(effects, None)
                }
            },
        })
    }
}
