//! Independent checks for pure heterogeneous child creation.

use behavior::{Activate, StopOnShutdown};
use foundation::{
    Actions, Address, Behavior, BehaviorActed, Births, ChildChoice, Children, CreateChild,
    CreationKind, CreationSequence, Never, NoBirths, Protocol, User,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Debug, PartialEq, Eq)]
struct Devices;

impl Protocol for Devices {
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

impl Protocol for Queries {
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

type RootChildren = ChildChoice<Queries, ChildChoice<Devices, Never>>;

struct Root {
    creations: CreationSequence,
}

impl Protocol for Root {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Root {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<RootChildren>;

    fn init(&mut self, _: foundation::InitializationTurn) -> BehaviorActed<Self> {
        let devices = self
            .creations
            .issue()
            .expect("the device fixture ID exists");
        let queries = self.creations.issue().expect("the query fixture ID exists");
        let creates = Children::<RuntimeAddr>::new()
            .child(devices, Devices)
            .child(queries, Queries)
            .into_creates();
        Ok(Actions::create(creates))
    }

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[test]
fn heterogeneous_children_preserve_declaration_order_and_creation_kind() {
    let mut device_ids = CreationSequence::new();
    let device = device_ids.issue().expect("the device ID exists");
    let mut query_ids = CreationSequence::new();
    let previous_query = query_ids.issue().expect("the previous query ID exists");
    let query = query_ids.issue().expect("the replacement query ID exists");
    let creates = Children::<RuntimeAddr>::new()
        .child(device, Devices)
        .create(CreateChild::replacement(query, previous_query, Queries))
        .into_creates();

    let mut creates = creates.into_iter();
    let devices = creates.next().expect("the device child is retained");
    assert_eq!(devices.id(), device);
    assert_eq!(devices.kind(), CreationKind::Birth);
    assert!(matches!(
        devices.child(),
        ChildChoice::Tail(ChildChoice::Head(Devices))
    ));

    let queries = creates.next().expect("the query child is retained");
    assert_eq!(queries.id(), query);
    assert_eq!(queries.kind(), CreationKind::replacement(previous_query));
    assert!(matches!(queries.child(), ChildChoice::Head(Queries)));
    assert!(creates.next().is_none());
}

#[test]
fn empty_product_emits_no_creations() {
    assert!(Children::<RuntimeAddr>::new().into_creates().is_empty());
}

#[test]
fn shutdown_wrapper_preserves_root_child_initialization() {
    let initialized = Activate::initialize(StopOnShutdown::new(Root {
        creations: CreationSequence::new(),
    }))
    .expect("root initialization succeeds");
    let mut creates = initialized.actions.creates.into_iter();

    let devices = creates.next().expect("the device child is retained");
    assert!(matches!(
        devices.child(),
        ChildChoice::Tail(ChildChoice::Head(Devices))
    ));
    let queries = creates.next().expect("the query child is retained");
    assert!(matches!(queries.child(), ChildChoice::Head(Queries)));
    assert!(creates.next().is_none());
}
