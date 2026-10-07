use behavior::{
    ActionItem, ActionItemResult, ActionSettlement, Actions, Address, Behavior, BehaviorActed,
    BehaviorSettlements, Births, ChildCreationOutcome, ChildNamespaceExhausted, ClassifySettlement,
    CommittedChild, CreateChild, CreationId, CreationKind, CreationSequence, CreationSettlement,
    CreationSettlements, Creations, CreationsSettled, EndpointAddress, EstablishedActor,
    EventIngress, EventLayer, InterpretItem, InterpretSends, Interpretation,
    InterpretationProgress, InterpreterFault, ItemSettlement, Never, NoBirths, Own, Protocol,
    RetirementBirths, RetirementCreationSettlement, SendEffects, SendInput, SettledItem,
    SettlementStatus, SourceAction, SourceActions, SourceAdmission, SourceCustody, SourceProgress,
    SourceSettlementCustody, SourceSettlements, Step, Stopped, finish_item, prepare_item,
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
                    ChildCreationOutcome::Established(established),
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
    let established = CommittedChild::new(
        creation,
        CreationKind::Birth,
        EstablishedActor::issued(InstalledControl::new(Endpoint(41))),
    );
    (
        creation,
        CreationSettlement::Settled(Creations::one(SettledItem::Attempted(
            ItemSettlement::Accepted(ChildCreationOutcome::Established(established)),
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
    fn admit_source(
        &mut self,
        input: &mut Option<CreationsSettled<RuntimeAddr, ReturningCreatorChildren>>,
        reply: &mut Option<Result<(), CreationsSettled<RuntimeAddr, ReturningCreatorChildren>>>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if reply.is_none() {
                if let Some(_input) = input.take() {
                    let admission = { Ok(()) };
                    *reply = Some(admission);
                }
            }
        }
    }
}

struct NoCreationIngress;

struct NoticeOwner;

struct Notice;

impl ActionItem for Notice {
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;
    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (input, received) = custody;
        match (&*input, &*received) {
            (Some(_), None) => Some((input, received)),
            _ => None,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

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
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Notice>,
        received: &'a mut Option<<Notice as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Notice: 'a,
    {
        if received.is_none() {
            if let Some(_notice) = input.take() {
                *received = Some(ItemSettlement::Accepted(9));
            }
        }
        core::future::ready(())
    }
}

impl SourceAdmission<NoticeEvent, NoticeOwner, ActionItemResult<Notice>> for NoticeHost {
    fn admit_source(
        &mut self,
        input: &mut Option<ActionItemResult<Notice>>,
        reply: &mut Option<Result<(), ActionItemResult<Notice>>>,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if reply.is_none() {
                if let Some(input) = input.take() {
                    let admission = {
                        let NoticeEvent::Settled(input) = NoticeEvent::ingress(input);
                        let SettledItem::Attempted(ItemSettlement::Accepted(value)) = input else {
                            panic!("the fixture source settlement must remain accepted")
                        };
                        self.accepted.push(value);
                        Ok(())
                    };
                    *reply = Some(admission);
                }
            }
        }
    }
}

#[tokio::test]
async fn generated_retirement_policy_needs_no_creation_event_and_preserves_exact_custody() {
    requires_custody::<RetiringCreator, NoCreationIngress>();

    let settlement = retiring_rejection();
    let SourceCustody::Retained(settled) = ({
        let mut source_progress = Some(SourceProgress::Original(settlement));
        <_ as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::prepare_source(&mut source_progress);
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <<RetirementBirths<RetiringCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::offer_next_to_source(custody, &mut NoCreationIngress).await;
        }
        <_ as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::finish_source(&mut source_progress);
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    }) else {
        panic!("a non-empty retirement creation settlement must remain in terminal custody")
    };
    let CreationSettlement::Rejected { creations, .. } = settled.into_settlement() else {
        panic!("retirement custody changed the exact rejection classification")
    };
    let mut creations = creations.into_iter();
    let creation = creations.next().expect("the rejected child remains owned");
    assert_eq!(creation.child(), &Worker);
    let remaining_creations = creations.next();
    assert!(remaining_creations.is_none());
}

#[tokio::test]
async fn retirement_creation_custody_allows_later_source_results_before_terminal_retention() {
    let mut source_actions = SourceActions::<Notice>::empty();
    <SourceActions<Notice> as SendInput<Notice, Own>>::emit(&mut source_actions, Notice);
    let Interpretation::Complete(sends) = ({
        let mut progress = Some(InterpretationProgress::Original(source_actions));
        <SourceActions<Notice> as InterpretSends<
        NoticeInterpreter,
        NoticeEvent,
        behavior::Here,
    >>::interpret(&mut progress, &mut NoticeInterpreter).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual notice source must return its complete settlement");
        };
        settlement
    }) else {
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

    let SourceCustody::Admitted(settlement) = ({
        let mut source_progress = Some(SourceProgress::Original(settlement));
        <_ as SourceSettlementCustody<NoticeHost, NoticeEvent>>::prepare_source(
            &mut source_progress,
        );
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <ActionSettlement<<RetirementBirths<RetiringCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements, SourceSettlements<Notice>, Never> as SourceSettlementCustody<NoticeHost, NoticeEvent>>::offer_next_to_source(custody, &mut host).await;
        }
        <_ as SourceSettlementCustody<NoticeHost, NoticeEvent>>::finish_source(
            &mut source_progress,
        );
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    }) else {
        panic!("the later live-source result must progress before terminal retention")
    };
    assert_eq!(host.accepted, [9]);

    let SourceCustody::Retained(settlement) = ({
        let mut source_progress = Some(SourceProgress::Original(settlement));
        <_ as SourceSettlementCustody<NoticeHost, NoticeEvent>>::prepare_source(
            &mut source_progress,
        );
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <ActionSettlement<<RetirementBirths<RetiringCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements, SourceSettlements<Notice>, Never> as SourceSettlementCustody<NoticeHost, NoticeEvent>>::offer_next_to_source(custody, &mut host).await;
        }
        <_ as SourceSettlementCustody<NoticeHost, NoticeEvent>>::finish_source(
            &mut source_progress,
        );
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    }) else {
        panic!("the complete settlement must enter terminal custody after source admission")
    };
    let CreationSettlement::Rejected { creations, .. } = settlement.creations.into_settlement()
    else {
        panic!("terminal custody changed the creation rejection")
    };
    assert_eq!(creations.len(), 1);
    let sends = settlement.sends.into_inputs();
    assert!(sends.is_empty());
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

#[test]
fn retirement_creation_total_finish_preserves_original_and_offering_failures_on_replay() {
    let mut sequence = CreationSequence::new();
    let rejected_original_id = sequence.issue().expect("the fixture creation ID exists");
    let rejected_original_requests =
        Creations::one(CreateChild::birth(rejected_original_id, Worker));
    let rejected_original_pointer = rejected_original_requests.iter().as_slice().as_ptr();
    let rejected_original: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements = RetirementCreationSettlement::new(CreationSettlement::Rejected {
        creations: rejected_original_requests,
        reason: ChildNamespaceExhausted,
    });
    let rejected_offering_id = sequence.issue().expect("the fixture creation ID exists");
    let rejected_offering_requests =
        Creations::one(CreateChild::birth(rejected_offering_id, Worker));
    let rejected_offering_pointer = rejected_offering_requests.iter().as_slice().as_ptr();
    let rejected_offering: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements = RetirementCreationSettlement::new(CreationSettlement::Rejected {
        creations: rejected_offering_requests,
        reason: ChildNamespaceExhausted,
    });
    let corrupt_original_id = sequence.issue().expect("the fixture creation ID exists");
    let corrupt_original_requests = Creations::one(CreateChild::birth(corrupt_original_id, Worker));
    let corrupt_original_pointer = corrupt_original_requests.iter().as_slice().as_ptr();
    let corrupt_original: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements = RetirementCreationSettlement::new(CreationSettlement::Corrupt {
        creations: corrupt_original_requests,
        fault: InterpreterFault::CorruptTraversal,
    });
    let corrupt_offering_id = sequence.issue().expect("the fixture creation ID exists");
    let corrupt_offering_requests = Creations::one(CreateChild::birth(corrupt_offering_id, Worker));
    let corrupt_offering_pointer = corrupt_offering_requests.iter().as_slice().as_ptr();
    let corrupt_offering: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements = RetirementCreationSettlement::new(CreationSettlement::Corrupt {
        creations: corrupt_offering_requests,
        fault: InterpreterFault::CorruptTraversal,
    });
    for (id, pointer, expected_status, original) in [
        (
            rejected_original_id,
            rejected_original_pointer,
            SettlementStatus::Rejected,
            SourceProgress::Original(rejected_original),
        ),
        (
            rejected_offering_id,
            rejected_offering_pointer,
            SettlementStatus::Rejected,
            SourceProgress::Offering(rejected_offering),
        ),
        (
            corrupt_original_id,
            corrupt_original_pointer,
            SettlementStatus::Corrupt,
            SourceProgress::Original(corrupt_original),
        ),
        (
            corrupt_offering_id,
            corrupt_offering_pointer,
            SettlementStatus::Corrupt,
            SourceProgress::Offering(corrupt_offering),
        ),
    ] {
        let mut progress = Some(original);
        <_ as SourceSettlementCustody<NoCreationIngress, <RetiringCreator as Behavior>::Event>>::finish_source(&mut progress);
        <_ as SourceSettlementCustody<NoCreationIngress, <RetiringCreator as Behavior>::Event>>::finish_source(&mut progress);
        let Some(SourceProgress::Completed(SourceCustody::Retained(returned))) = progress else {
            panic!("total finish and replay must retain the complete nonempty retirement row");
        };
        let returned = returned.into_settlement();
        let actual_status = returned.settlement_status();
        let creations = match returned {
            CreationSettlement::Rejected {
                creations,
                reason: ChildNamespaceExhausted,
            } => creations,
            CreationSettlement::Corrupt {
                creations,
                fault: InterpreterFault::CorruptTraversal,
            } => creations,
            _ => panic!("total finish changed the exact original creation failure"),
        };
        let actual_pointer = creations.iter().as_slice().as_ptr();
        let mut creations = creations.into_iter();
        let creation = creations
            .next()
            .expect("the exact original child remains owned");
        let remaining = creations.next();
        assert_eq!(actual_status, expected_status);
        assert_eq!(actual_pointer, pointer);
        assert_eq!(creation.id(), id);
        assert_eq!(creation.kind(), CreationKind::Birth);
        assert_eq!(creation.child(), &Worker);
        assert!(remaining.is_none());
    }
}

#[test]
fn retirement_creation_total_finish_exhausts_only_the_original_empty_settled_batch() {
    let original: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements =
        RetirementCreationSettlement::new(CreationSettlement::Settled(Creations::empty()));
    let offering: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements =
        RetirementCreationSettlement::new(CreationSettlement::Settled(Creations::empty()));
    for original in [
        SourceProgress::Original(original),
        SourceProgress::Offering(offering),
    ] {
        let mut progress = Some(original);
        <_ as SourceSettlementCustody<NoCreationIngress, <RetiringCreator as Behavior>::Event>>::finish_source(&mut progress);
        <_ as SourceSettlementCustody<NoCreationIngress, <RetiringCreator as Behavior>::Event>>::finish_source(&mut progress);
        let Some(SourceProgress::Completed(SourceCustody::Exhausted(returned))) = progress else {
            panic!("only the actual empty settled retirement row is exhausted");
        };
        let CreationSettlement::Settled(creations) = returned.into_settlement() else {
            panic!("total finish changed the empty settled classification");
        };
        assert!(creations.is_empty());
    }
}

#[test]
fn retirement_creation_total_finish_preserves_nonempty_settled_original_and_offering() {
    let (original_id, original) = returning_established();
    let (offering_id, offering) = returning_established();
    let CreationSettlement::Settled(original_creations) = &original else {
        panic!("the existing successful producer returns a settled creation");
    };
    let original_pointer = original_creations.iter().as_slice().as_ptr();
    let CreationSettlement::Settled(offering_creations) = &offering else {
        panic!("the existing successful producer returns a settled creation");
    };
    let offering_pointer = offering_creations.iter().as_slice().as_ptr();
    let original: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements = RetirementCreationSettlement::new(original);
    let offering: <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements = RetirementCreationSettlement::new(offering);
    for (id, pointer, original) in [
        (
            original_id,
            original_pointer,
            SourceProgress::Original(original),
        ),
        (
            offering_id,
            offering_pointer,
            SourceProgress::Offering(offering),
        ),
    ] {
        let mut progress = Some(original);
        <_ as SourceSettlementCustody<NoCreationIngress, <RetiringCreator as Behavior>::Event>>::finish_source(&mut progress);
        <_ as SourceSettlementCustody<NoCreationIngress, <RetiringCreator as Behavior>::Event>>::finish_source(&mut progress);
        let Some(SourceProgress::Completed(SourceCustody::Retained(returned))) = progress else {
            panic!(
                "total finish and replay must retain the genuine successful nonempty retirement row"
            );
        };
        let CreationSettlement::Settled(creations) = returned.into_settlement() else {
            panic!("total finish changed the complete successful creation classification");
        };
        let actual_pointer = creations.iter().as_slice().as_ptr();
        let mut creations = creations.into_iter();
        let SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Established(
            child,
        ))) = creations
            .next()
            .expect("the original accepted child is retained")
        else {
            panic!("total finish changed the accepted child outcome");
        };
        let remaining = creations.next();
        assert_eq!(actual_pointer, pointer);
        assert_eq!(child.id(), id);
        assert_eq!(child.kind(), CreationKind::Birth);
        assert!(remaining.is_none());
    }
}

#[test]
fn empty_creation_sources_complete_without_admission_and_survive_replay() {
    type Returning =
        <Births<ReturningCreatorChildren> as CreationSettlements<RuntimeAddr>>::Settlements;
    let mut returning = Some(SourceProgress::Original(CreationSettlement::Settled(
        Creations::empty(),
    )));
    <Returning as SourceSettlementCustody<ReturnHost, <ReturningCreator as Behavior>::Event>>::prepare_source(&mut returning);
    <Returning as SourceSettlementCustody<ReturnHost, <ReturningCreator as Behavior>::Event>>::finish_source(&mut returning);
    <Returning as SourceSettlementCustody<ReturnHost, <ReturningCreator as Behavior>::Event>>::prepare_source(&mut returning);
    match returning {
        Some(SourceProgress::Completed(SourceCustody::Exhausted(CreationSettlement::Settled(
            settlements,
        )))) => {
            assert!(
                settlements.is_empty(),
                "the complete empty batch is exhausted without a source event"
            );
        }
        _ => panic!("an empty creation source must complete without admission"),
    }

    type Retiring = <RetirementBirths<RetiringCreatorChildren> as CreationSettlements<
        RuntimeAddr,
    >>::Settlements;
    let mut retiring = Some(SourceProgress::Original(RetirementCreationSettlement::new(
        CreationSettlement::Settled(Creations::empty()),
    )));
    <Retiring as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::prepare_source(&mut retiring);
    <Retiring as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::finish_source(&mut retiring);
    <Retiring as SourceSettlementCustody<
        NoCreationIngress,
        <RetiringCreator as Behavior>::Event,
    >>::prepare_source(&mut retiring);
    match retiring {
        Some(SourceProgress::Completed(SourceCustody::Exhausted(settlement))) => {
            match settlement.into_settlement() {
                CreationSettlement::Settled(settlements) => assert!(settlements.is_empty()),
                _ => panic!("empty retirement custody must preserve its exact settlement"),
            }
        }
        _ => panic!("an empty retirement source must exhaust rather than retain a phantom result"),
    }
}

#[test]
fn absent_creation_lane_finalization_returns_its_complete_empty_product() {
    let mut progress = Some(InterpretationProgress::Original(Creations::empty()));
    <NoBirths as CreationSettlements<RuntimeAddr>>::finish_interpretation(&mut progress);
    assert!(
        matches!(
            &progress,
            Some(InterpretationProgress::Completed(Interpretation::Complete(
                _
            )))
        ),
        "finalization itself must acquire the complete absent lane"
    );
    <NoBirths as CreationSettlements<RuntimeAddr>>::prepare_interpretation(&mut progress);
    <NoBirths as CreationSettlements<RuntimeAddr>>::finish_interpretation(&mut progress);
    match progress {
        Some(InterpretationProgress::Completed(Interpretation::Complete(settlements))) => {
            assert!(settlements.is_empty())
        }
        _ => panic!("an absent creation lane must finish without an interpreter call"),
    }

    let mut source = Some(SourceProgress::Original(Creations::<Never>::empty()));
    <Creations<Never> as SourceSettlementCustody<NoCreationIngress, ()>>::finish_source(
        &mut source,
    );
    assert!(
        matches!(
            &source,
            Some(SourceProgress::Completed(SourceCustody::Exhausted(_)))
        ),
        "source finalization itself must exhaust the absent lane"
    );
    <Creations<Never> as SourceSettlementCustody<NoCreationIngress, ()>>::prepare_source(
        &mut source,
    );
    <Creations<Never> as SourceSettlementCustody<NoCreationIngress, ()>>::finish_source(
        &mut source,
    );
    match source {
        Some(SourceProgress::Completed(SourceCustody::Exhausted(settlements))) => {
            assert!(settlements.is_empty())
        }
        _ => panic!("an absent creation source must finish with its complete empty product"),
    }
}
