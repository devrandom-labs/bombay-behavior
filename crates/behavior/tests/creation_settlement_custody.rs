use behavior::{
    Address, Behavior, BehaviorActed, ChildCreationOutcome, ChildCreationSettled, ChildHead,
    CreateChild, CreationSequence, EndpointAddress, EventLayer, Here, InjectEvent, ItemSettlement,
    Never, NoBirths, NoSends, Protocol, RecoverEvent, RoutedCreation, SettledItem, User, UserEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
struct Worker(u8);

#[derive(Debug, Eq, PartialEq)]
struct InitRejected(u8);

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = InitRejected;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

type CreationReturned = ChildCreationSettled<Worker, ChildHead>;
type CreatorEvent = EventLayer<CreationReturned, User<RuntimeAddr, ()>>;

fn closed(event: CreatorEvent) -> Result<(), CreatorEvent> {
    Err(event)
}

#[test]
fn closed_admission_returns_the_non_cloneable_worker_and_error() {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation ID exists");
    let creation = RoutedCreation::new(CreateChild::birth(id, Worker(11)), 7);
    let returned = ChildCreationSettled::new(SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::InitializationRejected {
            creation,
            error: InitRejected(13),
        },
    )));
    let event = <CreatorEvent as InjectEvent<CreationReturned, Here>>::inject_at(returned);
    let rejected = closed(event).expect_err("closed admission returns the complete event");
    let recovered = <CreatorEvent as RecoverEvent<CreationReturned, Here>>::recover(rejected)
        .expect("the exact creation-settlement lane is recoverable");

    match recovered.into_settlement() {
        SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::InitializationRejected { creation, error },
        )) => {
            assert_eq!(creation.id(), id);
            assert_eq!(creation.route(), 7);
            let (creation, _) = creation.into_parts();
            let (_, worker, _) = creation.into_parts();
            assert_eq!(worker, Worker(11));
            assert_eq!(error, InitRejected(13));
        }
        _ => panic!("the initialization rejection changed classification"),
    }
}

#[test]
fn recovering_the_wrong_lane_returns_the_event_unchanged() {
    let user = CreatorEvent::user(RuntimeAddr(3), ());
    let unchanged = <CreatorEvent as RecoverEvent<CreationReturned, Here>>::recover(user)
        .expect_err("a user event is not a creation settlement");
    let recovered = unchanged
        .into_user()
        .expect("the unrelated user event remains complete");

    assert_eq!(recovered.from, RuntimeAddr(3));
}

#[test]
fn worker_definition_is_not_cloneable() {
    fn requires_owned(_: Worker) {}

    requires_owned(Worker(17));
}
