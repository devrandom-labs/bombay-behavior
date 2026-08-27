//! Independent checks for the pure heterogeneous child creation product.

use behavior::{Activate as _, DynamicSupervisor, DynamicSupervisorOutcome, StopOnShutdown};
use core::marker::PhantomData;
use foundation::{
    Actions, Address, Behavior, BehaviorActed, ChildChoice, Children, ChildrenError, Create,
    CreationKind, EndpointAddress, Never, NoBirths, Recipient, User,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct RuntimeEndpoint<P>(PhantomData<fn() -> P>);

impl<P> Clone for RuntimeEndpoint<P> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = RuntimeEndpoint<P>
    where
        P: foundation::Protocol<Addr = Self>;
}

#[derive(Debug, PartialEq, Eq)]
struct Devices;

impl behavior::Protocol for Devices {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Devices {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Queries;

impl behavior::Protocol for Queries {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Queries {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct SupervisorReply;

impl behavior::Protocol for SupervisorReply {
    type Addr = RuntimeAddr;
    type Msg = DynamicSupervisorOutcome<RuntimeAddr, Devices>;
}

impl Behavior for SupervisorReply {
    type Protocol = Self;
    type Event = User<RuntimeAddr, behavior::BehaviorMessage<Self>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

type DeviceSupervisor = DynamicSupervisor<
    RuntimeAddr,
    Devices,
    Recipient<SupervisorReply>,
    fn(Devices) -> behavior::Proxy<Devices>,
>;
type RootChildren = ChildChoice<Queries, ChildChoice<DeviceSupervisor, Never>>;

struct Root;

impl behavior::BehaviorBase for Root {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl behavior::Protocol for Root {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Root {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = ChildrenError<u64>;
    type Birth = foundation::Births<RootChildren>;

    fn init(&mut self, _: foundation::InitializationTurn) -> BehaviorActed<Self> {
        let creates = Children::<RuntimeAddr>::new()
            .child(
                13,
                DeviceSupervisor::new(
                    behavior::Proxy::new as fn(Devices) -> behavior::Proxy<Devices>,
                ),
            )
            .child(17, Queries)
            .into_creates()?;
        Ok(Actions::create(creates))
    }

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[test]
fn heterogeneous_children_preserve_declaration_order_and_provenance() {
    let creates = Children::<RuntimeAddr>::new()
        .child(7, Devices)
        .create(Create::replacement_incarnation(11, 3, Queries))
        .into_creates()
        .unwrap();

    assert_eq!(creates.len(), 2);
    assert_eq!(creates[0].nonce, 7);
    assert_eq!(creates[0].kind, CreationKind::Birth);
    assert!(matches!(
        creates[0].child,
        ChildChoice::Tail(ChildChoice::Head(Devices))
    ));
    assert_eq!(creates[1].nonce, 11);
    assert_eq!(creates[1].kind, CreationKind::replacement_of(3));
    assert!(matches!(creates[1].child, ChildChoice::Head(Queries)));
}

#[test]
fn duplicate_nonce_rejects_the_complete_product() {
    let result = Children::<RuntimeAddr>::new()
        .child(7, Devices)
        .child(7, Queries)
        .into_creates();

    assert_eq!(result, Err(ChildrenError::DuplicateNonce { nonce: 7 }));
}

#[test]
fn empty_product_emits_no_creations() {
    assert!(
        Children::<RuntimeAddr>::new()
            .into_creates()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn product_is_pure_input_to_the_existing_actions_creation_leg() {
    let creates = Children::<RuntimeAddr>::new()
        .child(2, Devices)
        .child(5, Queries)
        .into_creates()
        .unwrap();
    let actions = Actions::<
        RuntimeAddr,
        Never,
        Vec<Never>,
        foundation::Births<ChildChoice<Queries, ChildChoice<Devices, Never>>>,
    >::create(creates);

    assert_eq!(actions.creates.len(), 2);
    assert!(actions.sends.is_empty());
}

#[test]
fn address_constrained_template_flows_through_root_shutdown_initialization() {
    let initialized = StopOnShutdown::new(Root).initialize().unwrap();
    let creates = initialized.actions.creates;

    assert_eq!(creates.len(), 2);
    assert_eq!(creates[0].nonce, 13);
    assert!(matches!(
        creates[0].child,
        ChildChoice::Tail(ChildChoice::Head(_))
    ));
    assert_eq!(creates[1].nonce, 17);
    assert!(matches!(creates[1].child, ChildChoice::Head(Queries)));
}
