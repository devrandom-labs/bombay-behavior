use core::marker::PhantomData;

use behavior::{
    ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, ChildHead, CommittedChild,
    CreateChild, CreationKind, CreationRejection, CreationSequence, EndpointAddress,
    EstablishedActor, EstablishedCreation, EstablishedRecipient, Never, NoBirths, Protocol,
    RoutedCreation, User,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

struct Installed<B: Behavior> {
    endpoint: Endpoint,
    control: u64,
    behavior: PhantomData<fn() -> B>,
}

impl<B: Behavior> Clone for Installed<B> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint,
            control: self.control,
            behavior: PhantomData,
        }
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
    type Installed<B>
        = Installed<B>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> Self::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint
    }
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

fn installed(control: u64) -> EstablishedActor<Worker> {
    EstablishedActor::issued(Installed {
        endpoint: Endpoint(31),
        control,
        behavior: PhantomData,
    })
}

#[test]
fn committed_child_and_later_report_retain_exact_actor_and_provenance() {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation ID exists");
    let kind = CreationKind::Birth;
    let creation = ChildCreationOutcome::<Worker, ChildHead>::Established(CommittedChild::new(
        id,
        kind,
        installed(7),
    ));

    let committed = match creation.into_committed() {
        Ok(committed) => committed,
        Err(_) => panic!("child did not commit"),
    };
    assert_eq!(committed.id(), id);
    assert_eq!(committed.kind(), kind);
    assert_eq!(
        committed.actor().recipient(),
        EstablishedRecipient::issued(Endpoint(31))
    );
    let report = EstablishedCreation::<Worker, ChildHead>::installed(committed);
    assert_eq!(report.id(), id);
    assert_eq!(report.kind(), kind);
    let committed = report.into_committed().expect("named child committed");
    let (_, _, actor) = committed.into_parts();
    assert_eq!(
        actor.recipient(),
        EstablishedRecipient::issued(Endpoint(31))
    );
}

#[test]
fn non_committed_child_returns_complete_owned_creation() {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation ID exists");
    let creation = ChildCreationOutcome::<Worker, ChildHead>::InitializationRejected {
        creation: RoutedCreation::new(CreateChild::birth(id, Worker), 9),
        error: WorkerRejected,
    };

    let returned = match creation.into_committed() {
        Ok(_) => panic!("child was rejected"),
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

#[test]
fn named_child_rejection_contains_no_installed_actor() {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation ID exists");
    let report = EstablishedCreation::<Worker, ChildHead>::rejected(
        id,
        CreationKind::Birth,
        CreationRejection::EnvironmentFailed,
    );
    let rejected = report.into_committed();
    assert!(matches!(
        rejected,
        Err(CreationRejection::EnvironmentFailed)
    ));
}
