use behavior::{
    ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, ChildHead, CreateChild,
    CreationKind, CreationSequence, EndpointAddress, EstablishedCreation, EstablishedRecipient,
    Never, NoBirths, Protocol, RoutedCreation, User,
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

struct Worker;

#[derive(Debug, Eq, PartialEq)]
struct WorkerRejected;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = WorkerRejected;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[test]
fn committed_child_retains_its_exact_concrete_actor_capability() {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation ID exists");
    let creation = ChildCreationOutcome::<Worker, ChildHead>::Established {
        established: EstablishedCreation::installed(
            id,
            CreationKind::Birth,
            EstablishedRecipient::issued(Endpoint(31)),
        ),
    };

    let actor = match creation.into_actor() {
        Ok(actor) => actor,
        Err(_) => panic!("the committed child lost its exact actor capability"),
    };
    assert_eq!(
        actor.recipient(),
        EstablishedRecipient::issued(Endpoint(31))
    );
}

#[test]
fn non_committed_child_is_returned_without_strengthening() {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation ID exists");
    let creation = ChildCreationOutcome::<Worker, ChildHead>::InitializationRejected {
        creation: RoutedCreation::new(CreateChild::birth(id, Worker), 9),
        error: WorkerRejected,
    };

    let returned = match creation.into_actor() {
        Ok(_) => panic!("a rejected child became an exact actor capability"),
        Err(returned) => returned,
    };
    match returned {
        ChildCreationOutcome::InitializationRejected { creation, error } => {
            assert_eq!(creation.id(), id);
            assert_eq!(creation.route(), 9);
            assert_eq!(error, WorkerRejected);
        }
        _ => panic!("the child rejection changed shape"),
    }
}
