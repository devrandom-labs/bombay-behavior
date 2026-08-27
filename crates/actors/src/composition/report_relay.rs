//! Structural relaying of one direct-child report across one parent edge.

use crate::ShutdownRequested;
use behavior::{
    Actions, Address, Behavior, BehaviorActed, ChildInputIngress, ChildReport, ChildRoute,
    ComposedEvent, EventIngress, Here, InjectEvent, Inside, InterpreterRequests, ReportToParent,
    SendEffects, SendLayer, User, UserEvent,
};

/// Complete event algebra of a behavior that relays one direct-child report.
///
/// `Child` is the concrete source role. Equal nonce and report representations
/// from another child role cannot enter this lane.
pub enum RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
{
    Report {
        fact: ChildReport<A, Report>,
        source: core::marker::PhantomData<fn() -> Child>,
    },
    Inner(Inner),
}

impl<A, Child, Report, Inner> UserEvent for RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
    Inner: UserEvent<Addr = A>,
{
    type Addr = A;
    type Message = Inner::Message;

    fn user(from: A, message: Self::Message) -> Self {
        Self::Inner(Inner::user(from, message))
    }

    fn into_user(self) -> Result<User<A, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            report => Err(report),
        }
    }
}

impl<A, Child, Report, Inner> ComposedEvent for RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
    Inner: UserEvent<Addr = A>,
{
    type Inner = Inner;

    fn from_inner(event: Inner) -> Self {
        Self::Inner(event)
    }
}

impl<A, Child, Report, Inner, Occurrence>
    EventIngress<ChildRoute<Child, Occurrence>, ChildReport<A, Report>>
    for RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
    Child: Behavior<Protocol: behavior::Protocol<Addr = A>>,
    Inner: UserEvent<Addr = A>,
{
    fn ingress(fact: ChildReport<A, Report>) -> Self {
        Self::Report {
            fact,
            source: core::marker::PhantomData,
        }
    }
}

impl<A, Child, Report, Inner, Input, Path> InjectEvent<Input, Inside<Path>>
    for RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
    Inner: UserEvent<Addr = A> + InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Inner(Inner::inject_at(input))
    }
}

impl<A, Child, Report, Inner> InjectEvent<ShutdownRequested, Here>
    for RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
    Inner: UserEvent<Addr = A> + InjectEvent<ShutdownRequested, Here>,
{
    fn inject_at(input: ShutdownRequested) -> Self {
        Self::Inner(Inner::inject_at(input))
    }
}

impl<A, Child, Report, Inner, Source, Input> ChildInputIngress<Source, Input>
    for RelayChildReportEvent<A, Child, Report, Inner>
where
    A: Address,
    Inner: UserEvent<Addr = A> + ChildInputIngress<Source, Input>,
{
    fn child_input(input: Input) -> Self {
        Self::Inner(Inner::child_input(input))
    }
}

/// Relay each report from one concrete direct-child role to this actor's
/// established parent.
///
/// This composition owns no mutable state and never interprets, deduplicates,
/// or reconstructs the report. One accepted child fact produces exactly one
/// [`ReportToParent`] containing that complete fact. All inner initialization,
/// events, effects, births, errors, phases, and next-behavior decisions are
/// preserved structurally.
pub struct RelayChildReports<B, Child, Report>
where
    B: Behavior,
    Child: Behavior,
{
    inner: B,
    marker: core::marker::PhantomData<fn() -> (Child, Report)>,
}

impl<B, Child, Report> RelayChildReports<B, Child, Report>
where
    B: Behavior,
    Child: Behavior,
{
    #[must_use]
    pub const fn new(inner: B) -> Self {
        Self {
            inner,
            marker: core::marker::PhantomData,
        }
    }

    fn wrap(actions: BehaviorActed<B>) -> BehaviorActed<Self>
    where
        B::Protocol: behavior::Protocol,
        Child: Behavior<Protocol: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>>,
    {
        actions.map(|actions| {
            Actions::new(
                SendLayer::new(InterpreterRequests::empty(), actions.sends),
                actions.creates,
                actions.become_,
            )
        })
    }
}

impl<B, Child, Report> crate::BehaviorBase for RelayChildReports<B, Child, Report>
where
    B: Behavior + crate::BehaviorBase,
    Child: Behavior,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, Child, Report> Behavior for RelayChildReports<B, Child, Report>
where
    B: Behavior,
    Child: Behavior<Protocol: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>>,
{
    type Protocol = B::Protocol;
    type Event = RelayChildReportEvent<behavior::BehaviorAddr<B>, Child, Report, B::Event>;
    type Sends = SendLayer<
        InterpreterRequests<ReportToParent<ChildReport<behavior::BehaviorAddr<B>, Report>>>,
        B::Sends,
    >;
    type Ph = B::Ph;
    type Error = B::Error;
    type Birth = B::Birth;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        Self::wrap(behavior::initialize(&mut self.inner))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            RelayChildReportEvent::Report { fact, .. } => Ok(Actions::send(SendLayer::new(
                InterpreterRequests::new(vec![ReportToParent::new(fact)]),
                B::Sends::empty(),
            ))),
            RelayChildReportEvent::Inner(event) => {
                Self::wrap(behavior::delegate_transition(&mut self.inner, event))
            }
        }
    }
}
