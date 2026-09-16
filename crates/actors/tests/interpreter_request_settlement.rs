use behavior::{
    ActionItem, Address, CreationCorrelation, CreationId, CreationSequence, EndpointAddress, Here,
    InterpretItem, InterpretSends, Interpretation, InterpreterRequests, ItemSettlement, Never,
    Protocol, SendSettlements, SettledItem,
};
use behavior_actors::{
    CancelObservation, Exit, InterpretEstablishedObservation, ObservationId, ObserveEstablished,
    ObserveEstablishedCreation, ReportTerminalOutcome,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProbeAddr(u64);

impl Address for ProbeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProbeEndpoint(u64);

impl EndpointAddress for ProbeAddr {
    type Established<P>
        = ProbeEndpoint
    where
        P: Protocol<Addr = Self>;
}

struct ProbeProtocol;

impl Protocol for ProbeProtocol {
    type Addr = ProbeAddr;
    type Msg = ();
}

struct ProbeRole;

fn requires_action_item<T: ActionItem>() {}

fn requires_creation_correlation<T>()
where
    T: ActionItem<
            Accepted = (),
            Rejection = Never,
            Prerequisite = CreationCorrelation<ProbeProtocol, ProbeRole>,
        >,
{
}

struct AcceptanceRuntime;

impl<Item> InterpretItem<Item, (), Here> for AcceptanceRuntime
where
    Item: ActionItem<Accepted = ()>,
{
    async fn interpret_item(
        &mut self,
        _item: Item,
    ) -> ItemSettlement<Item, (), Item::Rejection, Item::Prerequisite> {
        ItemSettlement::Accepted(())
    }
}

struct CreationBlockedRuntime;

impl InterpretItem<ObserveEstablishedCreation<ProbeProtocol, ProbeRole>, (), Here>
    for CreationBlockedRuntime
{
    async fn interpret_item(
        &mut self,
        item: ObserveEstablishedCreation<ProbeProtocol, ProbeRole>,
    ) -> ItemSettlement<
        ObserveEstablishedCreation<ProbeProtocol, ProbeRole>,
        (),
        Never,
        CreationCorrelation<ProbeProtocol, ProbeRole>,
    > {
        let prerequisite = CreationCorrelation::new(item.creation);
        ItemSettlement::Blocked { item, prerequisite }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum CapturedObservation {
    Started(ObservationId, ProbeEndpoint),
    Cancelled(ObservationId),
}

struct ObservationCapture;

impl InterpretEstablishedObservation<ProbeProtocol> for ObservationCapture {
    type Output = CapturedObservation;

    fn observe(&mut self, id: ObservationId, endpoint: ProbeEndpoint) -> Self::Output {
        CapturedObservation::Started(id, endpoint)
    }

    fn cancel(&mut self, id: ObservationId) -> Self::Output {
        CapturedObservation::Cancelled(id)
    }
}

#[derive(Default)]
struct ObservationSettlementCapture {
    calls: Vec<CapturedObservation>,
}

impl InterpretEstablishedObservation<ProbeProtocol> for ObservationSettlementCapture {
    type Output = ();

    fn observe(&mut self, id: ObservationId, endpoint: ProbeEndpoint) {
        self.calls.push(CapturedObservation::Started(id, endpoint));
    }

    fn cancel(&mut self, id: ObservationId) {
        self.calls.push(CapturedObservation::Cancelled(id));
    }
}

fn creation_id() -> CreationId {
    CreationSequence::new()
        .issue()
        .expect("the test sequence has an initial creation ID")
}

async fn require_accepted_settlement<Item>(item: Item)
where
    Item: ActionItem<Accepted = ()>,
{
    let settlement =
        <InterpreterRequests<Item> as InterpretSends<AcceptanceRuntime, (), Here>>::interpret(
            InterpreterRequests::one(item),
            &mut AcceptanceRuntime,
        )
        .await;

    assert!(matches!(
        settlement,
        Interpretation::Complete(settlements)
            if matches!(
                settlements.as_slice(),
                [SettledItem::Attempted(ItemSettlement::Accepted(()))]
            )
    ));
}

fn recover_unattempted<Item>(item: Item) -> Item
where
    Item: ActionItem,
{
    let settlements =
        <InterpreterRequests<Item> as SendSettlements>::unattempted(InterpreterRequests::one(item));
    let mut settlements = settlements.into_iter();
    let recovered = match settlements.next() {
        Some(SettledItem::Unattempted(item)) => item,
        Some(SettledItem::Attempted(_)) => panic!("an unattempted product attempted its item"),
        None => panic!("the source item was lost"),
    };
    assert!(settlements.next().is_none());
    recovered
}

#[test]
fn public_interpreter_requests_own_action_item_contracts() {
    requires_action_item::<ReportTerminalOutcome<ProbeAddr>>();
    requires_action_item::<ObserveEstablished<ProbeProtocol>>();
    requires_action_item::<CancelObservation<ProbeProtocol>>();
    requires_action_item::<ObserveEstablishedCreation<ProbeProtocol, ProbeRole>>();
    requires_creation_correlation::<ObserveEstablishedCreation<ProbeProtocol, ProbeRole>>();
}

#[tokio::test]
async fn public_interpreter_requests_have_accepted_settlements() {
    require_accepted_settlement(ReportTerminalOutcome::<ProbeAddr>::new(Ok(Exit::Normal))).await;
    require_accepted_settlement(ObserveEstablished::<ProbeProtocol>::new(
        ObservationId(11),
        behavior::EstablishedRecipient::issued(ProbeEndpoint(12)),
    ))
    .await;
    require_accepted_settlement(CancelObservation::<ProbeProtocol>::new(ObservationId(13))).await;
    require_accepted_settlement(ObserveEstablishedCreation::<ProbeProtocol, ProbeRole>::new(
        creation_id(),
    ))
    .await;
}

#[test]
fn unattempted_settlements_return_each_exact_source_request() {
    let report = recover_unattempted(ReportTerminalOutcome::new(Ok(Exit::LinkDied(ProbeAddr(
        21,
    )))));
    assert_eq!(report.outcome, Ok(Exit::LinkDied(ProbeAddr(21))));

    let observation = recover_unattempted(ObserveEstablished::<ProbeProtocol>::new(
        ObservationId(22),
        behavior::EstablishedRecipient::issued(ProbeEndpoint(23)),
    ));
    assert_eq!(
        observation.interpret(&mut ObservationCapture),
        CapturedObservation::Started(ObservationId(22), ProbeEndpoint(23))
    );

    let cancellation =
        recover_unattempted(CancelObservation::<ProbeProtocol>::new(ObservationId(24)));
    assert_eq!(
        cancellation.interpret(&mut ObservationCapture),
        CapturedObservation::Cancelled(ObservationId(24))
    );

    let creation = creation_id();
    let observation = recover_unattempted(
        ObserveEstablishedCreation::<ProbeProtocol, ProbeRole>::new(creation),
    );
    assert_eq!(observation.creation, creation);
}

#[tokio::test]
async fn established_creation_observation_retains_its_blocking_correlation() {
    let creation = creation_id();
    let settlement = <InterpreterRequests<
        ObserveEstablishedCreation<ProbeProtocol, ProbeRole>,
    > as InterpretSends<CreationBlockedRuntime, (), Here>>::interpret(
        InterpreterRequests::one(ObserveEstablishedCreation::new(creation)),
        &mut CreationBlockedRuntime,
    )
    .await;

    assert!(matches!(
        settlement,
        Interpretation::Complete(settlements)
            if matches!(
                settlements.as_slice(),
                [SettledItem::Attempted(ItemSettlement::Blocked {
                    item,
                    prerequisite,
                })] if item.creation == creation && prerequisite.id() == creation
            )
    ));
}

#[test]
fn exact_observation_requests_own_their_accepted_settlement() {
    let mut runtime = ObservationSettlementCapture::default();
    let observed = ObserveEstablished::<ProbeProtocol>::new(
        ObservationId(31),
        behavior::EstablishedRecipient::issued(ProbeEndpoint(32)),
    )
    .settle(&mut runtime);
    let cancelled = CancelObservation::<ProbeProtocol>::new(ObservationId(33)).settle(&mut runtime);

    assert!(matches!(observed, ItemSettlement::Accepted(())));
    assert!(matches!(cancelled, ItemSettlement::Accepted(())));
    assert_eq!(
        runtime.calls,
        [
            CapturedObservation::Started(ObservationId(31), ProbeEndpoint(32)),
            CapturedObservation::Cancelled(ObservationId(33)),
        ]
    );
}
