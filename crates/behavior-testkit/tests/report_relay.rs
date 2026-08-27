//! Independent pure-fold model for one structural child-report relay.

use behavior::composition::{RelayChildReportEvent, RelayChildReports};
use behavior::{
    Actions, Behavior, BehaviorActed, ChildHead, ChildInputIngress, ChildReport, ChildRoute,
    EventIngress, Here, InjectEvent, MailAddr, Never, NoBirths, Protocol, ShutdownRequested, Stash,
    StashRoute, Step, StopOnShutdown, User, UserEvent,
};

struct Child;

impl Protocol for Child {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Child {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Domain;

impl Protocol for Domain {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Domain {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<u8>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::send(vec![event.message]))
    }
}

type Subject = RelayChildReports<Domain, Child, u16>;

struct PrivateOwner;

enum PrivateDomainEvent {
    Private(u32),
    User(User<MailAddr, u8>),
}

impl UserEvent for PrivateDomainEvent {
    type Addr = MailAddr;
    type Message = u8;

    fn user(from: MailAddr, message: u8) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<MailAddr, u8>, Self> {
        match self {
            Self::User(event) => Ok(event),
            private => Err(private),
        }
    }
}

impl ChildInputIngress<PrivateOwner, u32> for PrivateDomainEvent {
    fn child_input(input: u32) -> Self {
        Self::Private(input)
    }
}

#[test]
fn one_child_fact_becomes_one_unchanged_parent_report() {
    let mut subject = RelayChildReports::<_, Child, u16>::new(Domain);
    let fact = ChildReport::new(7, 41);
    let event = <RelayChildReportEvent<MailAddr, Child, u16, User<MailAddr, u8>> as EventIngress<
        ChildRoute<Child, ChildHead>,
        ChildReport<MailAddr, u16>,
    >>::ingress(fact);
    let actions = behavior::delegate_transition(&mut subject, event).unwrap();

    assert_eq!(actions.sends.owned.len(), 1);
    assert_eq!(actions.sends.owned.as_slice()[0].report, fact);
    assert!(actions.sends.inner.is_empty());
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn unrelated_domain_transitions_are_preserved_without_a_report() {
    let mut subject = RelayChildReports::<_, Child, u16>::new(Domain);
    let actions = behavior::delegate_transition(
        &mut subject,
        RelayChildReportEvent::Inner(User::new(MailAddr(3), 9)),
    )
    .unwrap();

    assert!(actions.sends.owned.is_empty());
    assert_eq!(actions.sends.inner, [9]);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn initialization_is_delegated_once_without_fabricating_a_report() {
    let mut subject: Subject = RelayChildReports::new(Domain);
    let actions = behavior::initialize(&mut subject).unwrap();

    assert!(actions.sends.owned.is_empty());
    assert!(actions.sends.inner.is_empty());
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn every_inner_private_child_input_is_preserved_without_naming_its_law() {
    type Event = RelayChildReportEvent<MailAddr, Child, u16, PrivateDomainEvent>;

    let event = <Event as ChildInputIngress<PrivateOwner, u32>>::child_input(73);

    match event {
        RelayChildReportEvent::Inner(PrivateDomainEvent::Private(value)) => {
            assert_eq!(value, 73);
        }
        RelayChildReportEvent::Inner(PrivateDomainEvent::User(_))
        | RelayChildReportEvent::Report { .. } => {
            panic!("the relay reinterpreted an inner private input")
        }
    }
}

#[test]
fn shutdown_ingress_reaches_the_inner_behavior_at_the_relay_root() {
    type Subject = RelayChildReports<StopOnShutdown<Domain>, Child, u16>;
    type Event = <Subject as Behavior>::Event;

    let mut subject = RelayChildReports::<_, Child, u16>::new(StopOnShutdown::new(Domain));
    let event = <Event as InjectEvent<ShutdownRequested, Here>>::inject_at(ShutdownRequested);
    let actions = behavior::delegate_transition(&mut subject, event).unwrap();

    assert!(actions.sends.owned.is_empty());
    assert_eq!(actions.sends.inner.owned, behavior::NoSends);
    assert!(actions.sends.inner.inner.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(_)));
}

fn deliver(_: &u8) -> StashRoute {
    StashRoute::Deliver
}

#[test]
fn shutdown_ingress_survives_a_transparent_outer_composition() {
    type Relay = RelayChildReports<StopOnShutdown<Domain>, Child, u16>;
    type Subject = Stash<Relay>;
    type Event = <Subject as Behavior>::Event;

    let relay = RelayChildReports::<_, Child, u16>::new(StopOnShutdown::new(Domain));
    let mut subject = Stash::new(relay, deliver);
    let event = <Event as InjectEvent<ShutdownRequested, Here>>::inject_at(ShutdownRequested);
    let actions = behavior::delegate_transition(&mut subject, event).unwrap();

    assert!(actions.sends.owned.is_empty());
    assert_eq!(actions.sends.inner.owned, behavior::NoSends);
    assert!(actions.sends.inner.inner.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Stop(_)));
}
