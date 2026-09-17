use behavior::{
    ActionItem, ActionItemResult, ActionSettlement, Actions, Address, Behavior, BehaviorActed,
    BehaviorSettlements, Births, ChildCreationOutcome, ChildNamespaceExhausted, ClassifySettlement,
    CreateChild, CreationId, CreationSequence, CreationSettlement, CreationSettlements, Creations,
    CreationsSettled, EndpointAddress, EstablishedCreation, EstablishedRecipient, EventIngress,
    EventLayer, InterpretItem, InterpretSends, Interpretation, ItemSettlement, Never, Own,
    Protocol, RetirementBirths, RetirementCreationSettlement, SendEffects, SendInput, SettledItem,
    SettlementStatus, SourceAction, SourceActions, SourceAdmission, SourceCustody,
    SourceSettlementCustody, Step, Stopped,
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
struct Worker;

#[behavior::behavior(addr = RuntimeAddr, message = Never)]
impl Worker {
    fn receive(&mut self, _: RuntimeAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

struct ReturningCreator {
    last_creation_status: Option<SettlementStatus>,
    established_creation: Option<CreationId>,
}

#[behavior::behavior(
    addr = RuntimeAddr,
    message = Never,
    births = { worker: Worker },
    creation_settlements = return_to_creator,
)]
impl ReturningCreator {
    fn receive(&mut self, _: RuntimeAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }

    fn creations_settled(
        &mut self,
        settled: CreationsSettled<RuntimeAddr, ReturningCreatorChildren>,
    ) -> BehaviorActed<Self> {
        let settlement = settled.into_settlement();
        self.last_creation_status = Some(settlement.settlement_status());
        if let CreationSettlement::Settled(creations) = settlement {
            for creation in creations {
                if let SettledItem::Attempted(ItemSettlement::Accepted(
                    ChildCreationOutcome::Established { established },
                )) = creation
                {
                    self.established_creation = Some(established.id());
                }
            }
        }
        Ok(Actions::cont())
    }
}

struct RetiringCreator;

#[behavior::behavior(
    addr = RuntimeAddr,
    message = Never,
    births = { worker: Worker },
    creation_settlements = retain_for_retirement,
)]
impl RetiringCreator {
    fn receive(&mut self, _: RuntimeAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

fn returning_rejection()
-> <Births<ReturningCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements {
    let mut sequence = CreationSequence::new();
    let creation = sequence.issue().expect("the first creation ID exists");
    CreationSettlement::Rejected {
        creations: Creations::one(CreateChild::birth(creation, Worker)),
        reason: ChildNamespaceExhausted,
    }
}

fn retiring_rejection()
-> <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements {
    let mut sequence = CreationSequence::new();
    let creation = sequence.issue().expect("the first creation ID exists");
    RetirementCreationSettlement::new(CreationSettlement::Rejected {
        creations: Creations::one(CreateChild::birth(creation, Worker)),
        reason: ChildNamespaceExhausted,
    })
}

fn returning_established() -> (
    CreationId,
    <Births<ReturningCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements,
) {
    let mut sequence = CreationSequence::new();
    let creation = sequence.issue().expect("the first creation ID exists");
    let established = EstablishedCreation::installed(
        creation,
        behavior::CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint(41)),
    );
    (
        creation,
        CreationSettlement::Settled(Creations::one(SettledItem::Attempted(
            ItemSettlement::Accepted(ChildCreationOutcome::Established { established }),
        ))),
    )
}

fn requires_custody<B, Host>()
where
    B: BehaviorSettlements,
    B::Settlements: SourceSettlementCustody<Host, B::Event>,
{
}

#[test]
fn generated_return_policy_admits_the_exact_batch_to_the_authored_transition() {
    requires_custody::<ReturningCreator, ReturnHost>();

    let settled = CreationsSettled::new(returning_rejection());
    let event = <<ReturningCreator as Behavior>::Event as EventIngress<
        Births<ReturningCreatorChildren>,
        CreationsSettled<RuntimeAddr, ReturningCreatorChildren>,
    >>::ingress(settled);
    let mut creator = ReturningCreator {
        last_creation_status: None,
        established_creation: None,
    };

    let actions = behavior::delegate_transition(&mut creator, event)
        .expect("the authored creation-settlement transition succeeds");

    assert_eq!(
        creator.last_creation_status,
        Some(SettlementStatus::Rejected)
    );
    assert!(actions.creates.is_empty());
    assert_eq!(actions.sends, behavior::NoSends);
}

#[test]
fn generated_return_policy_delivers_the_exact_established_child() {
    let (creation, settlement) = returning_established();
    let event = <<ReturningCreator as Behavior>::Event as EventIngress<
        Births<ReturningCreatorChildren>,
        CreationsSettled<RuntimeAddr, ReturningCreatorChildren>,
    >>::ingress(CreationsSettled::new(settlement));
    let mut creator = ReturningCreator {
        last_creation_status: None,
        established_creation: None,
    };

    let actions = behavior::delegate_transition(&mut creator, event)
        .expect("the established creation transition succeeds");

    assert_eq!(
        creator.last_creation_status,
        Some(SettlementStatus::Accepted)
    );
    assert_eq!(creator.established_creation, Some(creation));
    assert!(actions.creates.is_empty());
}

struct ReturnHost;

impl
    behavior::SourceAdmission<
        <ReturningCreator as Behavior>::Event,
        Births<ReturningCreatorChildren>,
        CreationsSettled<RuntimeAddr, ReturningCreatorChildren>,
    > for ReturnHost
{
    async fn admit_source(
        &mut self,
        _: CreationsSettled<RuntimeAddr, ReturningCreatorChildren>,
    ) -> Result<(), CreationsSettled<RuntimeAddr, ReturningCreatorChildren>> {
        Ok(())
    }
}

struct NoCreationIngress;

struct NoticeOwner;

struct Notice;

impl ActionItem for Notice {
    type Accepted = u8;
    type Rejection = Never;
    type Prerequisite = Never;
}

impl SourceAction for Notice {
    type Source = NoticeOwner;
}

enum NoticeEvent {
    Settled(ActionItemResult<Notice>),
}

impl EventIngress<NoticeOwner, ActionItemResult<Notice>> for NoticeEvent {
    fn ingress(input: ActionItemResult<Notice>) -> Self {
        Self::Settled(input)
    }
}

struct NoticeHost {
    accepted: Vec<u8>,
}

struct NoticeInterpreter;

impl<RootEvent, Path> InterpretItem<Notice, RootEvent, Path> for NoticeInterpreter {
    fn interpret_item(
        &mut self,
        _: Notice,
    ) -> impl core::future::Future<Output = ItemSettlement<Notice, u8, Never, Never>> + Send {
        core::future::ready(ItemSettlement::Accepted(9))
    }
}

impl SourceAdmission<NoticeEvent, NoticeOwner, ActionItemResult<Notice>> for NoticeHost {
    async fn admit_source(
        &mut self,
        input: ActionItemResult<Notice>,
    ) -> Result<(), ActionItemResult<Notice>> {
        let NoticeEvent::Settled(input) = NoticeEvent::ingress(input);
        let SettledItem::Attempted(ItemSettlement::Accepted(value)) = input else {
            panic!("the fixture source settlement must remain accepted")
        };
        self.accepted.push(value);
        Ok(())
    }
}

#[tokio::test]
async fn generated_retirement_policy_needs_no_creation_event_and_preserves_exact_custody() {
    requires_custody::<RetiringCreator, NoCreationIngress>();

    let settlement = retiring_rejection();
    let SourceCustody::Retained(settled) = <_ as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::offer_next_to_source(
        settlement, &mut NoCreationIngress
    )
    .await
    else {
        panic!("a non-empty retirement creation settlement must remain in terminal custody")
    };
    let CreationSettlement::Rejected { creations, .. } = settled.into_settlement() else {
        panic!("retirement custody changed the exact rejection classification")
    };
    let mut creations = creations.into_iter();
    let creation = creations.next().expect("the rejected child remains owned");
    assert_eq!(creation.child(), &Worker);
    assert!(creations.next().is_none());
}

#[tokio::test]
async fn retirement_creation_custody_allows_later_source_results_before_terminal_retention() {
    let mut source_actions = SourceActions::<Notice>::empty();
    <SourceActions<Notice> as SendInput<Notice, Own>>::emit(&mut source_actions, Notice);
    let Interpretation::Complete(sends) = <SourceActions<Notice> as InterpretSends<
        NoticeInterpreter,
        NoticeEvent,
        behavior::Here,
    >>::interpret(source_actions, &mut NoticeInterpreter)
    .await
    else {
        panic!("the fixture source action must settle")
    };
    let settlement = ActionSettlement {
        creations: retiring_rejection(),
        sends,
        become_: Step::<Never, Stopped>::Continue,
    };
    let mut host = NoticeHost {
        accepted: Vec::new(),
    };

    let SourceCustody::Admitted(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("the later live-source result must progress before terminal retention")
    };
    assert_eq!(host.accepted, [9]);

    let SourceCustody::Retained(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("the complete settlement must enter terminal custody after source admission")
    };
    let CreationSettlement::Rejected { creations, .. } = settlement.creations.into_settlement()
    else {
        panic!("terminal custody changed the creation rejection")
    };
    assert_eq!(creations.len(), 1);
    assert!(settlement.sends.into_inputs().is_empty());
}

// --- The Bombay stop shape ----------------------------------------------------
//
// The Bombay ledger's BEH1 blocker: a generated actor that creates one
// declared child and stops. Retirement custody must hold without any
// creation-ingress event lane, one staged creation must compose with an
// explicit termination verdict in a single `Actions` value, and a composed
// behavior layer must lift the creation-settlement ingress through its inner
// event.

struct ShutdownRequested;

struct Shard;

#[behavior::behavior(addr = RuntimeAddr, message = Never)]
impl Shard {
    fn receive(&mut self, _: RuntimeAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

struct StoppingSupervisor;

#[behavior::behavior(
    addr = RuntimeAddr,
    message = Never,
    births = { shard: Shard },
    creation_settlements = retain_for_retirement,
)]
impl StoppingSupervisor {
    fn receive(&mut self, _: RuntimeAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

#[test]
fn generated_actor_that_creates_one_child_and_stops_keeps_retirement_custody() {
    requires_custody::<StoppingSupervisor, NoCreationIngress>();

    let mut sequence = CreationSequence::new();
    let creation = sequence.issue().expect("the first creation ID exists");
    let actions = Actions::<
        RuntimeAddr,
        Never,
        behavior::NoSends,
        <StoppingSupervisor as Behavior>::Birth,
    >::create(Creations::one(CreateChild::birth(creation, Shard)))
    .map_become::<Never>(|_| Step::Stop(Stopped));

    assert_eq!(actions.creates.iter().count(), 1);
    assert!(matches!(actions.become_, Step::Stop(Stopped)));
}

#[test]
fn composed_behavior_layers_lift_the_creation_settlement_ingress() {
    let (_, settlement) = returning_established();
    let event =
        <EventLayer<ShutdownRequested, <ReturningCreator as Behavior>::Event> as EventIngress<
            Births<ReturningCreatorChildren>,
            CreationsSettled<RuntimeAddr, ReturningCreatorChildren>,
        >>::ingress(CreationsSettled::new(settlement));

    assert!(matches!(event, EventLayer::Inner(_)));
}
