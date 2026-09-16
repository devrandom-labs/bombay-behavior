use behavior::{
    Address, Behavior, BehaviorActed, Births, ChildChoice, ChildNamespaceExhausted,
    CreationSequence, CreationSettlement, CreationSettlements, Creations, CreationsSettled,
    EndpointAddress, EventIngress, Never, NoBirths, NoSends, Protocol, SourceAdmission,
    SourceCustody, SourceSettlementCustody, User,
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

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Account(u8);

impl Protocol for Account {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Account {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

type Children = ChildChoice<Worker, Account>;
type Returned = CreationsSettled<RuntimeAddr, Children>;

enum CreatorEvent {
    Creations(Returned),
}

impl EventIngress<Births<Children>, Returned> for CreatorEvent {
    fn ingress(input: Returned) -> Self {
        Self::Creations(input)
    }
}

enum Admission {
    Open,
    Closed,
}

struct Host {
    admission: Admission,
    received: Vec<Returned>,
}

impl SourceAdmission<CreatorEvent, Births<Children>, Returned> for Host {
    async fn admit_source(&mut self, input: Returned) -> Result<(), Returned> {
        match self.admission {
            Admission::Open => {
                let CreatorEvent::Creations(input) = CreatorEvent::ingress(input);
                self.received.push(input);
                Ok(())
            }
            Admission::Closed => Err(input),
        }
    }
}

fn rejected_children() -> <Births<Children> as CreationSettlements<RuntimeAddr>>::Settlements {
    let mut ids = CreationSequence::new();
    let worker = ids.issue().expect("worker creation ID");
    let account = ids.issue().expect("account creation ID");
    CreationSettlement::Rejected {
        creations: Creations::one(behavior::CreateChild::birth(
            worker,
            ChildChoice::Head(Worker(3)),
        ))
        .and(behavior::CreateChild::birth(
            account,
            ChildChoice::Tail(Account(5)),
        )),
        reason: ChildNamespaceExhausted,
    }
}

#[tokio::test]
async fn open_creator_receives_one_ordered_batch() {
    let mut host = Host {
        admission: Admission::Open,
        received: Vec::new(),
    };

    let residual = rejected_children().offer_next_to_source(&mut host).await;

    assert!(matches!(residual, SourceCustody::Admitted(_)));
    let returned = host.received.pop().expect("one creation batch returned");
    let CreationSettlement::Rejected { creations, .. } = returned.into_settlement() else {
        panic!("route rejection changed classification");
    };
    let children: Vec<_> = creations.into_iter().collect();
    assert!(matches!(children[0].child(), ChildChoice::Head(Worker(3))));
    assert!(matches!(children[1].child(), ChildChoice::Tail(Account(5))));
}

#[tokio::test]
async fn closed_creator_returns_the_complete_batch() {
    let mut host = Host {
        admission: Admission::Closed,
        received: Vec::new(),
    };

    let residual = rejected_children().offer_next_to_source(&mut host).await;

    let SourceCustody::Closed(CreationSettlement::Rejected { creations, .. }) = residual else {
        panic!("closed admission lost the rejected creation batch");
    };
    let children: Vec<_> = creations.into_iter().collect();
    assert_eq!(children.len(), 2);
    assert!(matches!(children[0].child(), ChildChoice::Head(Worker(3))));
    assert!(matches!(children[1].child(), ChildChoice::Tail(Account(5))));
}

struct NoHost;
enum NoEvent {}

#[tokio::test]
async fn no_births_requires_no_admission_port() {
    let residual =
        <Creations<Never> as SourceSettlementCustody<NoHost, NoEvent>>::offer_next_to_source(
            Creations::empty(),
            &mut NoHost,
        )
        .await;

    assert!(matches!(residual, SourceCustody::Exhausted(creations) if creations.is_empty()));
}
