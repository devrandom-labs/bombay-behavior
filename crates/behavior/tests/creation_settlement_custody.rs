use behavior::{
    Address, Behavior, BehaviorActed, ChildCreationOutcome, ChildCreationSettled, ChildHead,
    ClassifySettlement, CreateChild, CreationKind, CreationRejection, CreationSequence, Creations,
    EndpointAddress, EstablishedCreation, EventLayer, Here, InjectEvent, ItemSettlement, Never,
    NoBirths, NoSends, Protocol, RecoverEvent, RoutedCreation, SettledItem, SettlementStatus, User,
    UserEvent,
};

mod installed_control;
use installed_control::InstalledControl;

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

    type Installed<B>
        = InstalledControl<B, Endpoint>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        *installed.endpoint()
    }
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

#[test]
fn recorded_correlation_returns_the_complete_birth_or_replacement_request() {
    let mut sequence = CreationSequence::new();
    let previous = sequence.issue().expect("the prior creation ID exists");
    let id = sequence.issue().expect("the requested creation ID exists");
    for kind in [CreationKind::Birth, CreationKind::replacement(previous)] {
        let child: Box<[u8]> = Box::from([17, 23]);
        let allocation = child.as_ptr();
        let creation = match kind {
            CreationKind::Birth => CreateChild::<RuntimeAddr, _>::birth(id, child),
            CreationKind::Replacement { previous } => CreateChild::replacement(id, previous, child),
        };
        let request = RoutedCreation::new(creation, 7);
        let settlement: ItemSettlement<_, (), _, Never> = ItemSettlement::Rejected {
            item: request,
            reason: CreationRejection::CorrelationAlreadyRecorded,
        };
        assert_eq!(settlement.settlement_status(), SettlementStatus::Rejected);
        let ItemSettlement::Rejected { item, reason } = settlement else {
            panic!("recorded correlation retains the original staged request");
        };
        assert_eq!(reason, CreationRejection::CorrelationAlreadyRecorded);
        let (request, route) = item.into_parts();
        let (returned_id, child, returned_kind) = request.into_parts();
        assert_eq!(returned_id, id);
        assert_eq!(returned_kind, kind);
        assert_eq!(route, 7);
        assert_eq!(child.as_ref(), [17, 23]);
        assert_eq!(child.as_ptr(), allocation);

        let rejected = EstablishedCreation::<Worker, ChildHead>::rejected(id, kind, reason);
        let EstablishedCreation::Rejected {
            id: returned_id,
            kind: returned_kind,
            reason,
            ..
        } = rejected
        else {
            panic!("a recorded correlation is never an established actor");
        };
        assert_eq!(returned_id, id);
        assert_eq!(returned_kind, kind);
        assert_eq!(reason, CreationRejection::CorrelationAlreadyRecorded);
    }
}

#[test]
fn exact_one_creation_batch_returns_every_move_only_item_on_mismatch() {
    let one = Creations::one(Worker(3));
    let worker = one.into_one().expect("one item is accepted");
    assert_eq!(worker, Worker(3));

    let empty = Creations::<Worker>::empty()
        .into_one()
        .expect_err("an empty batch is returned");
    assert!(empty.is_empty());

    let pair = Creations::one(Worker(5)).and(Worker(8));
    let returned = pair.into_one().expect_err("both items are returned");
    let returned = returned
        .into_iter()
        .map(|worker| worker.0)
        .collect::<Vec<_>>();
    assert_eq!(returned, vec![5, 8]);
}
