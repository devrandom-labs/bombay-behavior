use behavior::{
    Address, Behavior, BehaviorActed, Births, ChildChoice, ChildNamespaceExhausted,
    CreationSequence, CreationSettlement, CreationSettlements, Creations, CreationsSettled,
    EndpointAddress, EventIngress, Never, NoBirths, NoSends, Protocol, SourceAdmission,
    SourceCustody, SourceProgress, SourceSettlementCustody, User,
};
use core::future::Future;

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
    fn admit_source(
        &mut self,
        input: &mut Option<Returned>,
        reply: &mut Option<Result<(), Returned>>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if reply.is_none() {
                if let Some(input) = input.take() {
                    let admission = {
                        match self.admission {
                            Admission::Open => {
                                let CreatorEvent::Creations(input) = CreatorEvent::ingress(input);
                                self.received.push(input);
                                Ok(())
                            }
                            Admission::Closed => Err(input),
                        }
                    };
                    *reply = Some(admission);
                }
            }
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

    let residual = {
        let mut source_progress = Some(SourceProgress::Original(rejected_children()));
        <_ as SourceSettlementCustody<Host, CreatorEvent>>::prepare_source(&mut source_progress);
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <<Births<Children> as CreationSettlements<RuntimeAddr>>::Settlements as SourceSettlementCustody<Host, CreatorEvent>>::offer_next_to_source(custody, &mut host).await;
        }
        <_ as SourceSettlementCustody<Host, CreatorEvent>>::finish_source(&mut source_progress);
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    };

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

    let residual = {
        let mut source_progress = Some(SourceProgress::Original(rejected_children()));
        <_ as SourceSettlementCustody<Host, CreatorEvent>>::prepare_source(&mut source_progress);
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <<Births<Children> as CreationSettlements<RuntimeAddr>>::Settlements as SourceSettlementCustody<Host, CreatorEvent>>::offer_next_to_source(custody, &mut host).await;
        }
        <_ as SourceSettlementCustody<Host, CreatorEvent>>::finish_source(&mut source_progress);
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    };

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
    let residual = {
        let mut source_progress = Some(SourceProgress::Original(Creations::empty()));
        <Creations<Never> as SourceSettlementCustody<NoHost, NoEvent>>::prepare_source(
            &mut source_progress,
        );
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <Creations<Never> as SourceSettlementCustody<NoHost, NoEvent>>::offer_next_to_source(
                custody,
                &mut NoHost,
            )
            .await;
        }
        <Creations<Never> as SourceSettlementCustody<NoHost, NoEvent>>::finish_source(
            &mut source_progress,
        );
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    };

    assert!(matches!(residual, SourceCustody::Exhausted(creations) if creations.is_empty()));
}
