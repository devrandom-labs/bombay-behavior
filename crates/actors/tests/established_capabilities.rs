use behavior_actors::{
    Actions, Activate as _, Behavior, BehaviorActed, Births, CancelObservation, ChildHead,
    ChildRole, ChildRoute, CreationKind, CreationRejection, EndpointAddress, EstablishedCreation,
    EstablishedDelivery, EstablishedObservation, EstablishedRecipient, EventLayer, Here, Ingress,
    InterpretEstablishedDelivery, InterpretEstablishedObservation, InterpretEstablishedShutdown,
    InterpretSends, InterpreterRequests, Never, NoBirths, ObservationId, ObservationOperation,
    ObservationRejection, ObserveEstablished, ObserveEstablishedCreation, Protocol,
    SendInterpreter, SendLayer, ShutdownEstablished, ShutdownId, ShutdownRequested, User,
};
use core::future::Future;
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl behavior_actors::Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P> {
    address: RuntimeAddr,
    slot: u64,
    protocol: PhantomData<fn() -> P>,
}

impl<P> Endpoint<P> {
    const fn new(address: RuntimeAddr, slot: u64) -> Self {
        Self {
            address,
            slot,
            protocol: PhantomData,
        }
    }
}

impl<P> Copy for Endpoint<P> {}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> core::fmt::Debug for Endpoint<P> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Endpoint")
            .field("address", &self.address)
            .field("slot", &self.slot)
            .finish()
    }
}

impl<P> PartialEq for Endpoint<P> {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address && self.slot == other.slot
    }
}

impl<P> Eq for Endpoint<P> {}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct WorkerProtocol;

impl Protocol for WorkerProtocol {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

type WorkerEvent = EventLayer<ShutdownRequested, User<RuntimeAddr, u8>>;

struct Worker;

impl Behavior for Worker {
    type Protocol = WorkerProtocol;
    type Event = WorkerEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(_) => Ok(Actions::stop()),
            EventLayer::Inner(_) => Ok(Actions::cont()),
        }
    }
}

struct ParentProtocol;

impl Protocol for ParentProtocol {
    type Addr = RuntimeAddr;
    type Msg = ();
}

enum PrimaryWorker {}

impl ChildRole<Parent> for PrimaryWorker {
    type Child = Worker;
    type Position = ChildHead;
}

type CreationFact = EstablishedCreation<WorkerProtocol, PrimaryWorker>;
type ParentEvent = EventLayer<CreationFact, User<RuntimeAddr, ()>>;
type ParentSends = SendLayer<
    InterpreterRequests<ObserveEstablishedCreation<WorkerProtocol, PrimaryWorker>>,
    Vec<EstablishedDelivery<WorkerProtocol>>,
>;

enum ParentState {
    Awaiting,
    Active(EstablishedRecipient<WorkerProtocol>),
    Rejected(CreationRejection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParentError {
    StaleCreationFact,
}

struct Parent {
    state: ParentState,
}

impl Parent {
    const ROUTE: ChildRoute<Worker, PrimaryWorker> = ChildRoute::new(7);

    const fn new() -> Self {
        Self {
            state: ParentState::Awaiting,
        }
    }
}

impl Behavior for Parent {
    type Protocol = ParentProtocol;
    type Event = ParentEvent;
    type Sends = ParentSends;
    type Ph = Never;
    type Error = ParentError;
    type Birth = Births<Worker>;

    fn init(&mut self, _: behavior_actors::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::new(
            SendLayer::new(
                InterpreterRequests::one(ObserveEstablishedCreation::at(Self::ROUTE)),
                Vec::new(),
            ),
            vec![Self::ROUTE.birth(Worker)],
            behavior_actors::Step::Continue,
        ))
    }

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event {
            EventLayer::Inner(_) => Ok(Actions::cont()),
            EventLayer::Owned(fact) => {
                if !matches!(self.state, ParentState::Awaiting) {
                    return Err(ParentError::StaleCreationFact);
                }
                match fact.into_recipient() {
                    Ok(recipient) => {
                        self.state = ParentState::Active(recipient.clone());
                        Ok(Actions::send(SendLayer::new(
                            InterpreterRequests::new(Vec::new()),
                            vec![EstablishedDelivery::new(recipient, 41)],
                        )))
                    }
                    Err(reason) => {
                        self.state = ParentState::Rejected(reason);
                        Ok(Actions::cont())
                    }
                }
            }
        }
    }
}

#[derive(Default)]
struct DeliveryRuntime {
    delivered: Vec<(RuntimeAddr, u64, u8)>,
}

impl SendInterpreter for DeliveryRuntime {
    type Error = Never;
}

impl InterpretEstablishedDelivery<WorkerProtocol> for DeliveryRuntime {
    fn interpret_established_delivery(
        &mut self,
        endpoint: Endpoint<WorkerProtocol>,
        message: u8,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.delivered
            .push((endpoint.address, endpoint.slot, message));
        async { Ok(()) }
    }
}

#[test]
fn creation_fact_is_protocol_and_occurrence_indexed_without_parent_state_in_its_type() {
    fn accepts(_: EstablishedCreation<WorkerProtocol, PrimaryWorker>) {}

    accepts(EstablishedCreation::installed(
        7,
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(91), 3)),
    ));
}

#[tokio::test]
async fn pure_fold_retains_the_exact_capability_and_emits_delivery_only_through_actions() {
    let initialized = Parent::new()
        .initialize()
        .expect("parent initialization succeeds");
    assert_eq!(initialized.actions.creates.len(), 1);
    assert_eq!(initialized.actions.creates[0].nonce, 7);
    assert_eq!(initialized.actions.sends.owned[0].nonce, 7);

    let mut active = initialized.behavior;
    let fact = EstablishedCreation::installed(
        7,
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(91), 3)),
    );
    let actions = active
        .on(fact)
        .expect("the first committed fact is accepted");
    let ParentState::Active(retained) = &active.state else {
        panic!("the exact capability is retained")
    };
    assert_eq!(retained, &actions.sends.inner[0].to);
    assert!(actions.creates.is_empty());

    let mut runtime = DeliveryRuntime::default();
    <_ as InterpretSends<_, ParentEvent, Here>>::interpret(actions.sends.inner, &mut runtime)
        .await
        .expect("exact delivery succeeds");
    assert_eq!(runtime.delivered, [(RuntimeAddr(91), 3, 41)]);

    let stale =
        EstablishedCreation::rejected(7, CreationKind::Birth, CreationRejection::NonceAlreadyBound);
    assert!(matches!(
        active.on(stale),
        Err(ParentError::StaleCreationFact)
    ));
}

#[test]
fn rejected_creation_retains_no_capability_and_produces_no_effect() {
    let mut active = Parent::new().initialize().unwrap().behavior;
    let actions = active
        .on(EstablishedCreation::rejected(
            7,
            CreationKind::Birth,
            CreationRejection::InitializationFailed,
        ))
        .unwrap();

    assert!(actions.sends.owned.is_empty());
    assert!(actions.sends.inner.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(
        active.state,
        ParentState::Rejected(CreationRejection::InitializationFailed)
    ));
}

#[derive(Debug, PartialEq, Eq)]
enum ObservationCall {
    Start(ObservationId, RuntimeAddr, u64),
    Cancel(ObservationId),
}

#[derive(Default)]
struct ObservationRuntime(Vec<ObservationCall>);

impl InterpretEstablishedObservation<WorkerProtocol> for ObservationRuntime {
    type Output = ();

    fn observe(&mut self, id: ObservationId, endpoint: Endpoint<WorkerProtocol>) {
        self.0
            .push(ObservationCall::Start(id, endpoint.address, endpoint.slot));
    }

    fn cancel(&mut self, id: ObservationId) {
        self.0.push(ObservationCall::Cancel(id));
    }
}

#[test]
fn observation_uses_exact_endpoint_and_separate_relationship_correlation() {
    let recipient = EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(44), 8));
    let mut runtime = ObservationRuntime::default();

    ObserveEstablished::new(ObservationId(5), recipient).interpret(&mut runtime);
    CancelObservation::<WorkerProtocol>::new(ObservationId(5)).interpret(&mut runtime);

    assert_eq!(
        runtime.0,
        [
            ObservationCall::Start(ObservationId(5), RuntimeAddr(44), 8),
            ObservationCall::Cancel(ObservationId(5)),
        ]
    );
    let rejected = EstablishedObservation::<WorkerProtocol>::rejected(
        ObservationId(9),
        ObservationOperation::Cancel,
        ObservationRejection::NotObserved,
    );
    assert_eq!(rejected.id(), ObservationId(9));
    assert!(matches!(
        rejected,
        EstablishedObservation::Rejected {
            operation: ObservationOperation::Cancel,
            reason: ObservationRejection::NotObserved,
            ..
        }
    ));
}

#[derive(Default)]
struct ShutdownRuntime {
    calls: Vec<(ShutdownId, RuntimeAddr, u64)>,
}

impl InterpretEstablishedShutdown<Worker, Here> for ShutdownRuntime {
    type Output = WorkerEvent;

    fn shutdown(
        &mut self,
        id: ShutdownId,
        endpoint: Endpoint<WorkerProtocol>,
        ingress: Ingress<ShutdownRequested, Here>,
    ) -> Self::Output {
        self.calls.push((id, endpoint.address, endpoint.slot));
        ingress.event(ShutdownRequested)
    }
}

#[test]
fn concrete_actor_proof_strengthens_only_at_the_typed_shutdown_boundary() {
    let fact = EstablishedCreation::<WorkerProtocol, PrimaryWorker>::installed(
        7,
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(70), 12)),
    );
    let actor = fact
        .into_actor::<Parent>()
        .expect("declared role proves the concrete installed behavior");
    let mut runtime = ShutdownRuntime::default();
    let event = ShutdownEstablished::new(ShutdownId(2), actor, Ingress::<_, Here>::new())
        .interpret(&mut runtime);

    assert!(matches!(event, EventLayer::Owned(ShutdownRequested)));
    assert_eq!(runtime.calls, [(ShutdownId(2), RuntimeAddr(70), 12)]);
}

#[tokio::test]
async fn nested_products_interpret_each_exact_delivery_once_in_structural_order() {
    let recipient = || EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(5), 21));
    let sends = SendLayer::new(
        vec![EstablishedDelivery::new(recipient(), 1)],
        SendLayer::new(
            vec![EstablishedDelivery::new(recipient(), 2)],
            Vec::<Never>::new(),
        ),
    );
    let mut runtime = DeliveryRuntime::default();

    <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(sends, &mut runtime)
        .await
        .unwrap();

    assert_eq!(
        runtime.delivered,
        [(RuntimeAddr(5), 21, 2), (RuntimeAddr(5), 21, 1)]
    );
}
