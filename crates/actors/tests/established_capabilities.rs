use behavior_actors::{
    Actions, Activate as _, Behavior, BehaviorActed, BehaviorBase, Births, CancelObservation,
    ChildHead, ChildOccurrence, ChildRole, ComposedEvent, CreationId, CreationKind,
    CreationRejection, CreationSequence, Creations, DeclaredChildOccurrence, Delivery,
    DeliveryRoute, EndpointAddress, EstablishedCreation, EstablishedDelivery,
    EstablishedObservation, EstablishedRecipient, EstablishedTerminationMonitor, EventLayer,
    ExactDeliveryReason, Exit, Here, HeterogeneousShutdownPlan, Ingress, InjectEvent,
    InterpretEstablished, InterpretEstablishedObservation, InterpretEstablishedShutdown,
    InterpretItem, InterpretSends, Interpretation, InterpreterRequests, ItemSettlement,
    LogicalDeliveryReason, MessageAdapterWithRoute, Never, NoBirths, NoShutdownTargets,
    ObservationId, ObservationOperation, ObservationRejection, ObserveEstablished,
    ObserveEstablishedCreation, Protocol, ReceiveTimeout, Recipient, ReplyRoute,
    ResolveChildOccurrence, SendEffects, SendLayer, ShutdownChoice, ShutdownEstablished,
    ShutdownId, ShutdownRejection, ShutdownRequested, Stash, StopOnShutdown,
    TerminationMonitorError, TerminationObservation, User, UserEvent, Watch, established_child,
};
use core::future::Future;
use core::marker::PhantomData;
use std::time::Instant;

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

struct PrimaryWorker;

impl ChildRole<Parent> for PrimaryWorker {
    type Child = Worker;
    type Position = ChildHead;
}

impl ChildOccurrence<Parent> for PrimaryWorker {
    type Resolution = DeclaredChildOccurrence;
}

type WorkerCreationResult = EstablishedCreation<WorkerProtocol, PrimaryWorker>;
enum ParentEvent {
    Creation(WorkerCreationResult),
    Command(User<RuntimeAddr, ()>),
}

impl UserEvent for ParentEvent {
    type Addr = RuntimeAddr;
    type Message = ();

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Command(user) => Ok(user),
            other => Err(other),
        }
    }
}

impl ComposedEvent for ParentEvent {
    type Inner = User<RuntimeAddr, ()>;

    fn from_inner(event: Self::Inner) -> Self {
        Self::Command(event)
    }
}

impl InjectEvent<WorkerCreationResult, Here> for ParentEvent {
    fn inject_at(value: WorkerCreationResult) -> Self {
        Self::Creation(value)
    }
}

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
    UnexpectedWorkerCreation,
}

struct Parent {
    child: CreationId,
    state: ParentState,
}

fn first_creation() -> CreationId {
    let mut creations = CreationSequence::new();
    let Some(child) = creations.issue() else {
        panic!("the first creation ID is always available");
    };
    child
}

impl BehaviorBase for Parent {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Parent {
    fn new() -> Self {
        Self {
            child: first_creation(),
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
                InterpreterRequests::one(ObserveEstablishedCreation::new(self.child)),
                Vec::new(),
            ),
            Creations::one(behavior_actors::CreateChild::birth(self.child, Worker)),
            behavior_actors::Step::Continue,
        ))
    }

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event {
            ParentEvent::Command(_) => Ok(Actions::cont()),
            ParentEvent::Creation(creation) => {
                if !matches!(self.state, ParentState::Awaiting) {
                    return Err(ParentError::UnexpectedWorkerCreation);
                }
                if creation.id() != self.child {
                    return Err(ParentError::UnexpectedWorkerCreation);
                }
                match creation.into_recipient() {
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

struct GeneratedParent;

#[behavior::behavior(
    addr = RuntimeAddr,
    message = (),
    births = { worker: Worker },
)]
impl GeneratedParent {
    fn receive(&mut self, _: RuntimeAddr, _: ()) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn resolves_occurrence<Emitter, Occurrence, Child, Position>()
where
    Emitter: ResolveChildOccurrence<Occurrence, Child = Child, Position = Position>,
    Child: Behavior,
{
}

#[test]
fn nominal_occurrences_cross_only_topology_transparent_wrappers() {
    resolves_occurrence::<Parent, PrimaryWorker, Worker, ChildHead>();
    resolves_occurrence::<StopOnShutdown<Parent>, PrimaryWorker, Worker, ChildHead>();
    resolves_occurrence::<
        StopOnShutdown<GeneratedParent>,
        GeneratedParentChildrenWorker,
        Worker,
        ChildHead,
    >();
    resolves_occurrence::<
        StopOnShutdown<ReceiveTimeout<Watch<Stash<GeneratedParent>>>>,
        GeneratedParentChildrenWorker,
        Worker,
        ChildHead,
    >();
    resolves_occurrence::<
        Stash<StopOnShutdown<GeneratedParent>>,
        GeneratedParentChildrenWorker,
        Worker,
        ChildHead,
    >();
}

#[derive(Default)]
struct DeliveryRuntime {
    delivered: Vec<(RuntimeAddr, u64, u8)>,
    order: Vec<DeliveryKind>,
}

#[derive(Debug, PartialEq, Eq)]
enum DeliveryKind {
    Logical(RuntimeAddr, u8),
    Established(RuntimeAddr, u64, u8),
}

impl InterpretEstablished<WorkerProtocol> for DeliveryRuntime {
    type Output = Endpoint<WorkerProtocol>;

    fn interpret_established(&mut self, endpoint: Endpoint<WorkerProtocol>) -> Self::Output {
        endpoint
    }
}

impl<RootEvent, Path> InterpretItem<Delivery<WorkerProtocol>, RootEvent, Path> for DeliveryRuntime {
    fn interpret_item(
        &mut self,
        delivery: Delivery<WorkerProtocol>,
    ) -> impl Future<
        Output = ItemSettlement<Delivery<WorkerProtocol>, (), LogicalDeliveryReason, Never>,
    > + Send {
        self.order.push(DeliveryKind::Logical(
            delivery.to.address(),
            delivery.message,
        ));
        async { ItemSettlement::Accepted(()) }
    }
}

impl<RootEvent, Path> InterpretItem<EstablishedDelivery<WorkerProtocol>, RootEvent, Path>
    for DeliveryRuntime
{
    fn interpret_item(
        &mut self,
        delivery: EstablishedDelivery<WorkerProtocol>,
    ) -> impl Future<
        Output = ItemSettlement<
            EstablishedDelivery<WorkerProtocol>,
            (),
            ExactDeliveryReason,
            Never,
        >,
    > + Send {
        let endpoint = delivery.to.interpret(self);
        self.delivered
            .push((endpoint.address, endpoint.slot, delivery.message));
        self.order.push(DeliveryKind::Established(
            endpoint.address,
            endpoint.slot,
            delivery.message,
        ));
        async { ItemSettlement::Accepted(()) }
    }
}

#[test]
fn creation_result_is_protocol_and_occurrence_indexed() {
    fn accepts(_: EstablishedCreation<WorkerProtocol, PrimaryWorker>) {}

    accepts(EstablishedCreation::installed(
        first_creation(),
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(91), 3)),
    ));
}

#[test]
fn committed_named_child_preserves_creation_and_exact_actor_together() {
    let creation = first_creation();
    let result = EstablishedCreation::<WorkerProtocol, PrimaryWorker>::installed(
        creation,
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(41), 3)),
    );

    let child = established_child::<Parent, PrimaryWorker>(result).unwrap();
    assert_eq!(child.creation(), creation);

    type Targets = ShutdownChoice<Worker, NoShutdownTargets<RuntimeAddr>>;
    let target: Targets = child.shutdown_target::<Parent, Targets>();
    let plan = HeterogeneousShutdownPlan::new([vec![target]]).unwrap();
    assert_eq!(plan.phases().len(), 1);

    let exact_actor = child.actor();
    let exact_delivery = EstablishedDelivery::new(exact_actor.into_recipient(), 18);
    assert_eq!(exact_delivery.message, 18);

    let (returned_creation, actor) = child.into_parts();
    assert_eq!(returned_creation, creation);
    let delivery = EstablishedDelivery::new(actor.into_recipient(), 19);
    assert_eq!(delivery.message, 19);
}

#[test]
fn rejected_named_child_produces_neither_local_nor_exact_capability() {
    let result = EstablishedCreation::<WorkerProtocol, PrimaryWorker>::rejected(
        first_creation(),
        CreationKind::Birth,
        CreationRejection::InitializationFailed,
    );

    assert!(matches!(
        established_child::<Parent, PrimaryWorker>(result),
        Err(CreationRejection::InitializationFailed)
    ));
}

#[tokio::test]
async fn parent_retains_the_exact_capability_and_emits_delivery_only_through_actions() {
    let parent = Parent::new();
    let creation = parent.child;
    let initialized = parent.initialize().expect("parent initialization succeeds");
    assert_eq!(initialized.actions.creates.len(), 1);
    let Some(created) = initialized.actions.creates.iter().next() else {
        panic!("one worker creation is emitted");
    };
    assert_eq!(created.id(), creation);
    let [observation] = initialized.actions.sends.owned.as_slice() else {
        panic!("one worker creation observation is emitted");
    };
    assert_eq!(observation.creation, creation);

    let mut active = initialized.behavior;
    let result = EstablishedCreation::installed(
        creation,
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(91), 3)),
    );
    let actions = active
        .on(result)
        .expect("the first committed creation is accepted");
    let ParentState::Active(retained) = &active.state else {
        panic!("the exact capability is retained")
    };
    assert_eq!(retained, &actions.sends.inner[0].to);
    assert!(actions.creates.is_empty());

    let mut runtime = DeliveryRuntime::default();
    let settlement =
        <_ as InterpretSends<_, ParentEvent, Here>>::interpret(actions.sends.inner, &mut runtime)
            .await;
    assert!(matches!(settlement, Interpretation::Complete(_)));
    assert_eq!(runtime.delivered, [(RuntimeAddr(91), 3, 41)]);

    let stale = EstablishedCreation::rejected(
        creation,
        CreationKind::Birth,
        CreationRejection::EnvironmentFailed,
    );
    assert!(matches!(
        active.on(stale),
        Err(ParentError::UnexpectedWorkerCreation)
    ));
}

#[test]
fn rejected_creation_retains_no_capability_and_produces_no_effect() {
    let parent = Parent::new();
    let creation = parent.child;
    let mut active = parent.initialize().unwrap().behavior;
    let actions = active
        .on(EstablishedCreation::rejected(
            creation,
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

struct Observer {
    events: Vec<&'static str>,
}

impl BehaviorBase for Observer {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for Observer {
    type Protocol = ParentProtocol;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn record_observation(
    observer: &mut Observer,
    observation: EstablishedObservation<WorkerProtocol>,
) -> Actions<RuntimeAddr, Never, Vec<Never>, NoBirths> {
    observer.events.push(match observation {
        EstablishedObservation::Started { .. } => "started",
        EstablishedObservation::Cancelled { .. } => "cancelled",
        EstablishedObservation::Rejected { .. } => "rejected",
        EstablishedObservation::Stopped { .. } => "stopped",
    });
    Actions::cont()
}

#[test]
fn outer_shutdown_wrapper_reindexes_exact_observation_return_ingress_only() {
    fn accepts_inside<B, Input>()
    where
        B: Behavior,
        B::Event: behavior_actors::InjectEvent<Input, behavior_actors::Inside<Here>>,
    {
    }

    accepts_inside::<
        StopOnShutdown<EstablishedTerminationMonitor<Observer, WorkerProtocol>>,
        EstablishedObservation<WorkerProtocol>,
    >();
}

#[test]
fn exact_termination_monitor_commits_each_complete_relationship_phase() {
    let recipient = EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 9));
    let mut active = EstablishedTerminationMonitor::established(
        Observer { events: Vec::new() },
        ObservationId(6),
        recipient,
        record_observation,
    )
    .initialize()
    .unwrap()
    .behavior;

    assert_eq!(
        active.observation(),
        behavior_actors::TerminationObservation::Requested
    );
    let started = active
        .on_path(EstablishedObservation::<WorkerProtocol>::started(
            ObservationId(6),
        ))
        .unwrap();
    assert!(started.sends.owned.is_empty());
    assert!(started.sends.inner.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, behavior_actors::Step::Continue);
    assert_eq!(
        active.observation(),
        behavior_actors::TerminationObservation::Observing
    );
    let cancelled = active
        .on_path(EstablishedObservation::<WorkerProtocol>::cancelled(
            ObservationId(6),
        ))
        .unwrap();
    assert!(cancelled.sends.owned.is_empty());
    assert!(cancelled.sends.inner.is_empty());
    assert!(cancelled.creates.is_empty());
    assert_eq!(cancelled.become_, behavior_actors::Step::Continue);
    assert_eq!(
        active.observation(),
        behavior_actors::TerminationObservation::Cancelled
    );
    assert!(matches!(
        active.on_path(EstablishedObservation::<WorkerProtocol>::stopped(
            ObservationId(6),
            Ok(Exit::Normal),
            Instant::now(),
        )),
        Err(TerminationMonitorError::UnexpectedReport {
            observation: TerminationObservation::Cancelled,
            report: unexpected,
        }) if unexpected.id() == ObservationId(6)
    ));
    assert!(matches!(
        active.on_path(EstablishedObservation::<WorkerProtocol>::started(
            ObservationId(99),
        )),
        Err(TerminationMonitorError::UnexpectedReport {
            observation: TerminationObservation::Cancelled,
            report: unexpected,
        }) if unexpected.id() == ObservationId(99)
    ));
    assert!(active.base().events.is_empty());
    assert_eq!(
        active.observation(),
        behavior_actors::TerminationObservation::Cancelled
    );
}

#[test]
fn exact_termination_monitor_reacts_once_to_the_matching_stop() {
    let recipient = EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 10));
    let mut active = EstablishedTerminationMonitor::established(
        Observer { events: Vec::new() },
        ObservationId(7),
        recipient,
        record_observation,
    )
    .initialize()
    .unwrap()
    .behavior;

    let started = active
        .on_path(EstablishedObservation::<WorkerProtocol>::started(
            ObservationId(7),
        ))
        .unwrap();
    assert!(started.sends.owned.is_empty());
    assert!(started.sends.inner.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, behavior_actors::Step::Continue);
    let stopped = active
        .on_path(EstablishedObservation::<WorkerProtocol>::stopped(
            ObservationId(7),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap();
    assert!(stopped.sends.owned.is_empty());
    assert!(stopped.sends.inner.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, behavior_actors::Step::Continue);
    assert!(matches!(
        active.on_path(EstablishedObservation::<WorkerProtocol>::stopped(
            ObservationId(7),
            Ok(Exit::Normal),
            Instant::now(),
        )),
        Err(TerminationMonitorError::UnexpectedReport {
            observation: TerminationObservation::Observed,
            report: unexpected,
        }) if unexpected.id() == ObservationId(7)
    ));

    assert_eq!(active.base().events, ["stopped"]);
    assert_eq!(
        active.observation(),
        behavior_actors::TerminationObservation::Observed
    );
}

fn adapt_exact(value: u16) -> u8 {
    u8::try_from(value).unwrap()
}

#[tokio::test]
async fn message_adapter_selects_exact_delivery_without_logical_resolution() {
    type ExactAdapter = MessageAdapterWithRoute<u16, EstablishedRecipient<WorkerProtocol>>;
    let recipient = EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(46), 10));
    let mut active = MessageAdapterWithRoute::new(recipient, adapt_exact)
        .initialize()
        .unwrap()
        .behavior;
    let actions = active.receive(RuntimeAddr(1), 7).unwrap();

    let mut runtime = DeliveryRuntime::default();
    let settlement = <_ as InterpretSends<_, <ExactAdapter as Behavior>::Event, Here>>::interpret(
        actions.sends,
        &mut runtime,
    )
    .await;
    assert!(matches!(settlement, Interpretation::Complete(_)));
    assert_eq!(runtime.delivered, [(RuntimeAddr(46), 10, 7)]);
}

#[tokio::test]
async fn mixed_reply_routes_preserve_capability_and_interpretation_order() {
    let mut sends =
        ReplyRoute::<WorkerProtocol>::logical(Recipient::global(RuntimeAddr(1))).deliver(10);
    sends.append(
        ReplyRoute::established(EstablishedRecipient::issued(Endpoint::new(
            RuntimeAddr(2),
            7,
        )))
        .deliver(20),
    );
    sends.append(
        ReplyRoute::<WorkerProtocol>::logical(Recipient::global(RuntimeAddr(3))).deliver(30),
    );

    let mut runtime = DeliveryRuntime::default();
    let settlement =
        <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(sends, &mut runtime).await;
    assert!(matches!(settlement, Interpretation::Complete(_)));

    assert_eq!(
        runtime.order,
        [
            DeliveryKind::Logical(RuntimeAddr(1), 10),
            DeliveryKind::Established(RuntimeAddr(2), 7, 20),
            DeliveryKind::Logical(RuntimeAddr(3), 30),
        ]
    );
}

#[derive(Default)]
struct ShutdownRuntime {
    calls: Vec<(ShutdownId, RuntimeAddr, u64)>,
}

impl InterpretEstablishedShutdown<Worker, Here> for ShutdownRuntime {
    fn shutdown(
        &mut self,
        id: ShutdownId,
        endpoint: Endpoint<WorkerProtocol>,
        _ingress: Ingress<ShutdownRequested, Here>,
    ) -> Result<(), ShutdownRejection> {
        self.calls.push((id, endpoint.address, endpoint.slot));
        Ok(())
    }
}

#[test]
fn concrete_actor_proof_authorizes_the_typed_shutdown_request() {
    let result = EstablishedCreation::<WorkerProtocol, PrimaryWorker>::installed(
        first_creation(),
        CreationKind::Birth,
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(70), 12)),
    );
    let actor = result
        .into_actor::<Parent>()
        .expect("declared role proves the concrete installed behavior");
    let mut runtime = ShutdownRuntime::default();
    let settlement = ShutdownEstablished::new(ShutdownId(2), actor, Ingress::<_, Here>::new())
        .settle(&mut runtime);

    assert!(matches!(
        settlement,
        ItemSettlement::Accepted(ShutdownId(2))
    ));
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

    let settlement =
        <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(sends, &mut runtime).await;
    assert!(matches!(settlement, Interpretation::Complete(_)));

    assert_eq!(
        runtime.delivered,
        [(RuntimeAddr(5), 21, 2), (RuntimeAddr(5), 21, 1)]
    );
}
