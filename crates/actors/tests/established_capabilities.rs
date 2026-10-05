use behavior::{
    ActionItem, Actions, ActiveTurn, Behavior, BehaviorActed, BehaviorBase, Births, ChildHead,
    ChildOccurrence, ChildRole, CommittedChild, ComposedEvent, CreationId, CreationKind,
    CreationRejection, CreationSequence, Creations, DeclaredChildOccurrence, Delivery,
    EndpointAddress, EstablishedActor, EstablishedCreation, EstablishedDelivery,
    EstablishedRecipient, EventLayer, Here, Ingress, InjectEvent, Inside, InterpretEstablished,
    InterpretItem, InterpretSends, Interpretation, InterpretationProgress, InterpreterRequests,
    ItemSettlement, Never, NoBirths, NoSends, Protocol, Recipient, ResolveChildOccurrence,
    SendEffects, SendLayer, Step, User, UserEvent,
};
use behavior_actors::atomic::{ImmediateActivation, WorkerSubmission};
use behavior_actors::{
    Activate as _, CancelObservation, DeliveryRoute, EstablishedObservation,
    EstablishedTerminationMonitor, Exit, HeterogeneousShutdownPlan,
    InterpretEstablishedObservation, InterpretEstablishedShutdown, MessageAdapterWithRoute,
    NoShutdownTargets, ObservationAuthority, ObservationId, ObservationRejection,
    ObserveEstablished, ObserveEstablishedCreation, ReceiveTimeout, ReplyRoute, ShutdownChoice,
    ShutdownEstablished, ShutdownId, ShutdownRejection, ShutdownRequested, Stash, StopOnShutdown,
    TerminationMonitorError, TerminationObservation, Watch, established_child,
};
use core::future::{Future, ready};
use core::marker::PhantomData;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl behavior::Address for RuntimeAddr {
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

struct Installed<B: Behavior> {
    endpoint: Endpoint<B::Protocol>,
    control: mpsc::Sender<B::Event>,
    inbox: Arc<Mutex<mpsc::Receiver<B::Event>>>,
}

impl<B: Behavior> Clone for Installed<B> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint,
            control: self.control.clone(),
            inbox: Arc::clone(&self.inbox),
        }
    }
}

impl<B: Behavior> Installed<B> {
    fn new(endpoint: Endpoint<B::Protocol>) -> Self {
        let (control, inbox) = mpsc::channel();
        Self {
            endpoint,
            control,
            inbox: Arc::new(Mutex::new(inbox)),
        }
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;

    type Installed<B>
        = Installed<B>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint
    }
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

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
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

type WorkerCreationResult = EstablishedCreation<Worker, PrimaryWorker>;
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
    InterpreterRequests<ObserveEstablishedCreation<Worker, PrimaryWorker>>,
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

fn committed(
    id: CreationId,
    kind: CreationKind,
    endpoint: Endpoint<WorkerProtocol>,
) -> CommittedChild<Worker, PrimaryWorker> {
    CommittedChild::new(id, kind, EstablishedActor::issued(Installed::new(endpoint)))
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

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::new(
            SendLayer::new(
                InterpreterRequests::one(ObserveEstablishedCreation::new(self.child)),
                Vec::new(),
            ),
            Creations::one(behavior::CreateChild::birth(self.child, Worker)),
            Step::Continue,
        ))
    }

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
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
    creation_settlements = retain_for_retirement,
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
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Delivery<WorkerProtocol>>,
        received: &'a mut Option<<Delivery<WorkerProtocol> as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Delivery<WorkerProtocol>: 'a,
    {
        if received.is_some() {
            return ready(());
        }
        let Some(delivery) = input.take() else {
            return ready(());
        };
        self.order.push(DeliveryKind::Logical(
            delivery.to.address(),
            delivery.message,
        ));
        *received = Some(ItemSettlement::Accepted(()));
        ready(())
    }
}

impl<RootEvent, Path> InterpretItem<EstablishedDelivery<WorkerProtocol>, RootEvent, Path>
    for DeliveryRuntime
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<EstablishedDelivery<WorkerProtocol>>,
        received: &'a mut Option<<EstablishedDelivery<WorkerProtocol> as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        EstablishedDelivery<WorkerProtocol>: 'a,
    {
        if received.is_some() {
            return ready(());
        }
        let Some(delivery) = input.take() else {
            return ready(());
        };
        let endpoint = delivery.to.interpret(self);
        self.delivered
            .push((endpoint.address, endpoint.slot, delivery.message));
        self.order.push(DeliveryKind::Established(
            endpoint.address,
            endpoint.slot,
            delivery.message,
        ));
        *received = Some(ItemSettlement::Accepted(()));
        ready(())
    }
}

#[test]
fn creation_result_is_protocol_and_occurrence_indexed() {
    fn accepts(_: EstablishedCreation<Worker, PrimaryWorker>) {}

    accepts(EstablishedCreation::installed(committed(
        first_creation(),
        CreationKind::Birth,
        Endpoint::new(RuntimeAddr(91), 3),
    )));
}

#[test]
fn committed_named_child_preserves_creation_and_exact_actor_together() {
    let creation = first_creation();
    let result = EstablishedCreation::<Worker, PrimaryWorker>::installed(committed(
        creation,
        CreationKind::Birth,
        Endpoint::new(RuntimeAddr(41), 3),
    ));

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
    let result = EstablishedCreation::<Worker, PrimaryWorker>::rejected(
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
    let result = EstablishedCreation::installed(committed(
        creation,
        CreationKind::Birth,
        Endpoint::new(RuntimeAddr(91), 3),
    ));
    let actions = active
        .on(result)
        .expect("the first committed creation is accepted");
    let ParentState::Active(retained) = &active.state else {
        panic!("the exact capability is retained")
    };
    assert_eq!(retained, &actions.sends.inner[0].to);
    assert!(actions.creates.is_empty());

    let mut runtime = DeliveryRuntime::default();
    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(actions.sends.inner));
        <_ as InterpretSends<_, ParentEvent, Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual capability interpretation must retain its complete settlement");
        };
        settlement
    };
    assert!(matches!(settlement, Interpretation::Complete(_)));
    assert_eq!(runtime.delivered, [(RuntimeAddr(91), 3, 41)]);

    let stale = EstablishedCreation::rejected(
        creation,
        CreationKind::Birth,
        CreationRejection::EnvironmentFailed,
    );
    let rejected = active.on(stale);
    assert!(matches!(
        rejected,
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

enum ObservationCall {
    Start(ObserveEstablished<WorkerProtocol>, Endpoint<WorkerProtocol>),
    Cancel(CancelObservation<WorkerProtocol>),
}

struct ObservationRuntime;

impl InterpretEstablishedObservation<WorkerProtocol> for ObservationRuntime {
    type Output = ObservationCall;

    fn observe(
        &mut self,
        request: ObserveEstablished<WorkerProtocol>,
        endpoint: Endpoint<WorkerProtocol>,
    ) -> Self::Output {
        ObservationCall::Start(request, endpoint)
    }

    fn cancel(&mut self, request: CancelObservation<WorkerProtocol>) -> Self::Output {
        ObservationCall::Cancel(request)
    }
}

#[test]
fn observation_transfers_whole_original_request_and_exact_endpoint() {
    let endpoint = Endpoint::new(RuntimeAddr(44), 8);
    let recipient = EstablishedRecipient::issued(endpoint);
    let request = ObserveEstablished::new(ObservationId(5), recipient);
    let ObservationCall::Start(request, retained_endpoint) =
        request.interpret(&mut ObservationRuntime)
    else {
        panic!("the start request owns its actual endpoint");
    };
    assert_eq!(retained_endpoint, endpoint);
    assert_eq!(request.id(), ObservationId(5));
    // The advanced host owns actual admission. A never-accepted whole request
    // can be rejected and serially retried without inventing another attempt.
    let rejected =
        EstablishedObservation::observe_rejected(request, ObservationRejection::IdAlreadyBound);
    let EstablishedObservation::ObserveRejected { request, reason } = rejected else {
        panic!("rejection preserves the original request");
    };
    assert_eq!(reason, ObservationRejection::IdAlreadyBound);
    let ObservationCall::Start(request, retry_endpoint) =
        request.interpret(&mut ObservationRuntime)
    else {
        panic!("serial retry preserves the whole never-accepted original");
    };
    assert_eq!(retry_endpoint, endpoint);
    let authority = ObservationAuthority::issued(request);
    let relationship = authority.relationship().clone();
    let cancellation = CancelObservation::new(authority);
    let ObservationCall::Cancel(cancellation) = cancellation.interpret(&mut ObservationRuntime)
    else {
        panic!("the whole cancellation grant transfers once");
    };
    let rejected =
        EstablishedObservation::cancel_rejected(cancellation, ObservationRejection::NotObserved);
    let EstablishedObservation::CancelRejected { request, reason } = rejected else {
        panic!("rejected cancellation returns the original grant");
    };
    assert_eq!(reason, ObservationRejection::NotObserved);
    assert!(request.relationship() == &relationship);
    let consumed = EstablishedObservation::cancelled(request);
    let EstablishedObservation::Cancelled {
        relationship: cancelled,
    } = consumed
    else {
        panic!("successful cancellation consumes permission into a receipt");
    };
    assert!(cancelled == relationship);
}

struct Observer {
    events: Vec<EstablishedObservation<WorkerProtocol>>,
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
    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn record_observation(
    observer: &mut Observer,
    observation: EstablishedObservation<WorkerProtocol>,
) -> Actions<RuntimeAddr, Never, Vec<Never>, NoBirths> {
    observer.events.push(observation);
    Actions::cont()
}

/// A pure aggregate chooses cancellation and returns it in its named owning
/// InterpreterRequests lane. It owns no sender, runtime or imperative effect.
struct CancelObserver {
    monitor: EstablishedTerminationMonitor<Observer, WorkerProtocol>,
}

impl BehaviorBase for CancelObserver {
    type Base = Observer;
    fn base(&self) -> &Observer {
        self.monitor.base()
    }
}

impl Behavior for CancelObserver {
    type Protocol = ParentProtocol;
    type Event = EventLayer<
        EstablishedObservation<WorkerProtocol>,
        <EstablishedTerminationMonitor<Observer, WorkerProtocol> as Behavior>::Event,
    >;
    type Sends = SendLayer<
        InterpreterRequests<CancelObservation<WorkerProtocol>>,
        <EstablishedTerminationMonitor<Observer, WorkerProtocol> as Behavior>::Sends,
    >;
    type Ph = Never;
    type Error = TerminationMonitorError<Never, EstablishedObservation<WorkerProtocol>>;
    type Birth = NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        behavior::initialize(&mut self.monitor).map(|actions| {
            actions.map_sends(|inner| SendLayer::new(InterpreterRequests::empty(), inner))
        })
    }

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Inner(EventLayer::Inner(_)) => {
                let cancellation = match self.monitor.take_cancellation() {
                    Some(request) => InterpreterRequests::one(request),
                    None => InterpreterRequests::empty(),
                };
                Ok(Actions::cont().map_sends(|_: Self::Sends| {
                    SendLayer::new(
                        cancellation,
                        SendLayer::new(InterpreterRequests::empty(), Vec::new()),
                    )
                }))
            }
            EventLayer::Owned(report) | EventLayer::Inner(EventLayer::Owned(report)) => {
                behavior::delegate_transition(&mut self.monitor, EventLayer::Owned(report)).map(
                    |actions| {
                        actions
                            .map_sends(|inner| SendLayer::new(InterpreterRequests::empty(), inner))
                    },
                )
            }
        }
    }
}

#[test]
fn outer_shutdown_wrapper_reindexes_exact_observation_return_ingress_only() {
    fn accepts_inside<B, Input>()
    where
        B: Behavior,
        B::Event: InjectEvent<Input, behavior::Inside<Here>>,
    {
    }
    accepts_inside::<
        StopOnShutdown<EstablishedTerminationMonitor<Observer, WorkerProtocol>>,
        EstablishedObservation<WorkerProtocol>,
    >();
    accepts_inside::<CancelObserver, EstablishedObservation<WorkerProtocol>>();
    fn accepts_nested<B, Input>()
    where
        B: Behavior,
        B::Event: InjectEvent<Input, Inside<Inside<Here>>>,
    {
    }
    accepts_nested::<StopOnShutdown<CancelObserver>, EstablishedObservation<WorkerProtocol>>();
}

enum ObservationArrivalOrder {
    CancellationFirst,
    CompletionFirst,
}

#[test]
fn exact_monitor_retains_cancel_rejection_and_reacts_once_in_both_arrival_orders() {
    // These pure report rows are a consumer oracle. The actual producer
    // reversal requires the independent runtime conversion-barrier test.
    for first in [
        ObservationArrivalOrder::CancellationFirst,
        ObservationArrivalOrder::CompletionFirst,
    ] {
        let request = ObserveEstablished::new(
            ObservationId(6),
            EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 9)),
        );
        let mut observer = CancelObserver {
            monitor: EstablishedTerminationMonitor::established(
                Observer { events: Vec::new() },
                request,
                record_observation,
            ),
        };
        let initialization = behavior::initialize(&mut observer).unwrap();
        assert_eq!(initialization.sends.owned.len(), 0);
        assert_eq!(initialization.sends.inner.owned.len(), 1);
        assert_eq!(initialization.sends.inner.inner.len(), 0);
        assert_eq!(initialization.creates.len(), 0);
        assert_eq!(initialization.become_, Step::Continue);
        let mut requests = initialization.sends.inner.owned.into_iter();
        let request = requests.next().expect("original emitted request");
        let extra_request = requests.next();
        assert!(extra_request.is_none());
        let authority = ObservationAuthority::issued(request);
        let relationship = authority.relationship().clone();
        let foreign_requests = [
            ObserveEstablished::new(
                ObservationId(6),
                EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 9)),
            ),
            ObserveEstablished::new(
                ObservationId(6),
                EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 9)),
            ),
            ObserveEstablished::new(
                ObservationId(66),
                EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 9)),
            ),
        ];
        for request in foreign_requests {
            let foreign_authority = ObservationAuthority::issued(request);
            let foreign_relationship = foreign_authority.relationship().clone();
            let denied = behavior::delegate_transition(
                &mut observer,
                EventLayer::Inner(EventLayer::Owned(EstablishedObservation::started(
                    foreign_authority,
                ))),
            );
            let Err(TerminationMonitorError::UnexpectedReport {
                observation: TerminationObservation::Requested,
                report: EstablishedObservation::Started { authority },
            }) = denied
            else {
                panic!("foreign Started must return its complete original grant");
            };
            assert!(authority.relationship() == &foreign_relationship);
            drop(authority);
        }
        let started = behavior::delegate_transition(
            &mut observer,
            EventLayer::Inner(EventLayer::Owned(EstablishedObservation::started(
                authority,
            ))),
        )
        .unwrap();
        assert_eq!(started.sends.owned.len(), 0);
        assert_eq!(started.sends.inner.owned.len(), 0);
        assert_eq!(started.sends.inner.inner.len(), 0);
        assert_eq!(started.creates.len(), 0);
        assert_eq!(started.become_, Step::Continue);
        let cancellation = behavior::delegate_transition(
            &mut observer,
            EventLayer::Inner(EventLayer::Inner(User::new(RuntimeAddr(4), ()))),
        )
        .unwrap();
        assert_eq!(cancellation.sends.owned.len(), 1);
        assert_eq!(cancellation.sends.inner.owned.len(), 0);
        assert_eq!(cancellation.sends.inner.inner.len(), 0);
        assert_eq!(cancellation.creates.len(), 0);
        assert_eq!(cancellation.become_, Step::Continue);
        let mut requests = cancellation.sends.owned.into_iter();
        let request = requests.next().expect("original cancellation");
        let extra_request = requests.next();
        assert!(extra_request.is_none());
        assert!(request.relationship() == &relationship);
        let at = Instant::now();
        let stopped = EstablishedObservation::stopped(
            relationship.clone(),
            Err(behavior_actors::Crash::Panicked),
            at,
        );
        let rejected =
            EstablishedObservation::cancel_rejected(request, ObservationRejection::NotObserved);
        let stopped = EventLayer::Inner(EventLayer::Owned(stopped));
        let rejected = EventLayer::Owned(rejected);
        let reports = match first {
            ObservationArrivalOrder::CancellationFirst => [
                (rejected, TerminationObservation::Observing),
                (stopped, TerminationObservation::Observed),
            ],
            ObservationArrivalOrder::CompletionFirst => [
                (stopped, TerminationObservation::Observed),
                (rejected, TerminationObservation::Observed),
            ],
        };
        for (report, expected_observation) in reports {
            let actions = behavior::delegate_transition(&mut observer, report).unwrap();
            assert_eq!(observer.monitor.observation(), expected_observation);
            assert_eq!(actions.sends.owned.len(), 0);
            assert_eq!(actions.sends.inner.owned.len(), 0);
            assert_eq!(actions.sends.inner.inner.len(), 0);
            assert_eq!(actions.creates.len(), 0);
            assert_eq!(actions.become_, Step::Continue);
        }
        assert_eq!(
            observer.monitor.observation(),
            TerminationObservation::Observed
        );
        let replay = EstablishedObservation::stopped(
            relationship.clone(),
            Err(behavior_actors::Crash::Panicked),
            at,
        );
        let replay = behavior::delegate_transition(
            &mut observer,
            EventLayer::Inner(EventLayer::Owned(replay)),
        );
        let Err(TerminationMonitorError::UnexpectedReport {
            observation,
            report,
        }) = replay
        else {
            panic!("the same terminal fact cannot be accepted twice");
        };
        assert_eq!(observation, TerminationObservation::Observed);
        let EstablishedObservation::Stopped {
            relationship: returned,
            outcome,
            at: returned_at,
        } = report
        else {
            panic!("replay returns the whole original terminal fact");
        };
        assert!(returned == relationship);
        assert_eq!(outcome, Err(behavior_actors::Crash::Panicked));
        assert_eq!(returned_at, at);
        let (mut observer, target) = observer.monitor.into_parts();
        assert_eq!(observer.events.len(), 1);
        let EstablishedObservation::Stopped {
            relationship: retained,
            outcome,
            at: retained_at,
        } = observer.events.pop().expect("one terminal callback")
        else {
            panic!("reaction retains the exact complete stopped fact");
        };
        assert!(retained == relationship);
        assert_eq!(outcome, Err(behavior_actors::Crash::Panicked));
        assert_eq!(retained_at, at);
        let (request, reason) = target
            .into_rejected_cancel()
            .unwrap_or_else(|_| panic!("whole rejected cancellation"));
        assert_eq!(reason, ObservationRejection::NotObserved);
        assert!(request.relationship() == &relationship);
        drop((request, relationship));
    }
}

#[test]
fn cancelled_monitor_returns_late_stopped_without_a_terminal_reaction() {
    let request = ObserveEstablished::new(
        ObservationId(7),
        EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 10)),
    );
    let mut monitor = EstablishedTerminationMonitor::established(
        Observer { events: Vec::new() },
        request,
        record_observation,
    );
    let initialization = behavior::initialize(&mut monitor).unwrap();
    assert_eq!(initialization.sends.owned.len(), 1);
    assert_eq!(initialization.sends.inner.len(), 0);
    assert_eq!(initialization.creates.len(), 0);
    assert_eq!(initialization.become_, Step::Continue);
    let mut requests = initialization.sends.owned.into_iter();
    let authority = ObservationAuthority::issued(requests.next().expect("actual request"));
    let extra_request = requests.next();
    assert!(extra_request.is_none());
    let relationship = authority.relationship().clone();
    let actions = behavior::delegate_transition(
        &mut monitor,
        EventLayer::Owned(EstablishedObservation::started(authority)),
    )
    .unwrap();
    assert_eq!(actions.sends.owned.len(), 0);
    assert_eq!(actions.sends.inner.len(), 0);
    assert_eq!(actions.creates.len(), 0);
    assert_eq!(actions.become_, Step::Continue);
    let request = monitor.take_cancellation().expect("sole affine grant");
    let cancelled = behavior::delegate_transition(
        &mut monitor,
        EventLayer::Owned(EstablishedObservation::cancelled(request)),
    )
    .unwrap();
    assert_eq!(cancelled.sends.owned.len(), 0);
    assert_eq!(cancelled.sends.inner.len(), 0);
    assert_eq!(cancelled.creates.len(), 0);
    assert_eq!(cancelled.become_, Step::Continue);
    let at = Instant::now();
    let late = behavior::delegate_transition(
        &mut monitor,
        EventLayer::Owned(EstablishedObservation::stopped(
            relationship.clone(),
            Ok(Exit::Normal),
            at,
        )),
    );
    let Err(TerminationMonitorError::UnexpectedReport {
        observation,
        report,
    }) = late
    else {
        panic!("Cancelled plus later Stopped is an unlawful producer sequence");
    };
    assert_eq!(observation, TerminationObservation::Cancelled);
    let EstablishedObservation::Stopped {
        relationship: returned,
        outcome,
        at: returned_at,
    } = report
    else {
        panic!("whole defensive rejection");
    };
    assert!(returned == relationship);
    assert_eq!(outcome, Ok(Exit::Normal));
    assert_eq!(returned_at, at);
    assert_eq!(monitor.base().events.len(), 0);
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
    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(actions.sends));
        <_ as InterpretSends<_, <ExactAdapter as Behavior>::Event, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual capability interpretation must retain its complete settlement");
        };
        settlement
    };
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
    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual capability interpretation must retain its complete settlement");
        };
        settlement
    };
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

impl<B, Path> InterpretEstablishedShutdown<B, Path> for ShutdownRuntime
where
    B: Behavior<Protocol: Protocol<Addr = RuntimeAddr>>,
    B::Event: InjectEvent<ShutdownRequested, Path>,
{
    fn shutdown(
        &mut self,
        id: ShutdownId,
        installed: Installed<B>,
        ingress: Ingress<ShutdownRequested, Path>,
    ) -> Result<(), ShutdownRejection> {
        let endpoint = installed.endpoint;
        installed
            .control
            .send(ingress.event(ShutdownRequested))
            .expect("live control lane");
        installed
            .inbox
            .lock()
            .unwrap()
            .try_recv()
            .expect("shutdown event delivered");
        self.calls.push((id, endpoint.address, endpoint.slot));
        Ok(())
    }
}

#[test]
fn both_wrapper_orders_accept_exact_installed_shutdown() {
    type Outer = StopOnShutdown<ReceiveTimeout<Watch<Stash<GeneratedParent>>>>;
    type Inner = Stash<StopOnShutdown<GeneratedParent>>;

    let outer =
        EstablishedActor::<Outer>::issued(Installed::new(Endpoint::new(RuntimeAddr(81), 1)));
    let inner =
        EstablishedActor::<Inner>::issued(Installed::new(Endpoint::new(RuntimeAddr(82), 2)));
    let mut runtime = ShutdownRuntime::default();

    let outer_result = ShutdownEstablished::new(ShutdownId(11), outer, Ingress::<_, Here>::new())
        .settle(&mut runtime);
    let inner_result = ShutdownEstablished::new(ShutdownId(12), inner, Ingress::<_, Here>::new())
        .settle(&mut runtime);

    assert!(matches!(
        outer_result,
        ItemSettlement::Accepted(ShutdownId(11))
    ));
    assert!(matches!(
        inner_result,
        ItemSettlement::Accepted(ShutdownId(12))
    ));
    assert_eq!(
        runtime.calls,
        [
            (ShutdownId(11), RuntimeAddr(81), 1),
            (ShutdownId(12), RuntimeAddr(82), 2)
        ]
    );
}

#[test]
fn concrete_actor_proof_authorizes_the_typed_shutdown_request() {
    let result = EstablishedCreation::<Worker, PrimaryWorker>::installed(committed(
        first_creation(),
        CreationKind::Birth,
        Endpoint::new(RuntimeAddr(70), 12),
    ));
    let actor = result
        .into_committed()
        .expect("creation committed")
        .into_parts()
        .2;
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

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual capability interpretation must retain its complete settlement");
        };
        settlement
    };
    assert!(matches!(settlement, Interpretation::Complete(_)));

    assert_eq!(
        runtime.delivered,
        [(RuntimeAddr(5), 21, 2), (RuntimeAddr(5), 21, 1)]
    );
}

type TerminalPublicationProtocol =
    behavior::MessageProtocol<RuntimeAddr, EstablishedObservation<WorkerProtocol>>;

// This domain behavior owns a genuine typed reply acquaintance, not a
// mutable wrapper access mechanism. Both callbacks transfer the whole fact.
struct TerminalPublisher {
    reply_to: Recipient<TerminalPublicationProtocol>,
}

impl BehaviorBase for TerminalPublisher {
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl Behavior for TerminalPublisher {
    type Protocol = ParentProtocol;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Delivery<TerminalPublicationProtocol>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn publish_terminal(
    publisher: &mut TerminalPublisher,
    report: EstablishedObservation<WorkerProtocol>,
) -> Actions<RuntimeAddr, Never, Vec<Delivery<TerminalPublicationProtocol>>, NoBirths> {
    Actions::new(
        vec![Delivery::new(publisher.reply_to.clone(), report)],
        Creations::empty(),
        Step::Continue,
    )
}

fn publish_terminal_through_shutdown(
    publisher: &mut StopOnShutdown<TerminalPublisher>,
    report: EstablishedObservation<WorkerProtocol>,
) -> Actions<
    RuntimeAddr,
    Never,
    SendLayer<NoSends, Vec<Delivery<TerminalPublicationProtocol>>>,
    NoBirths,
> {
    Actions::new(
        SendLayer::new(
            NoSends,
            vec![Delivery::new(publisher.base().reply_to.clone(), report)],
        ),
        Creations::empty(),
        Step::Continue,
    )
}

#[test]
fn outer_shutdown_preserves_whole_rejected_cancel_after_both_report_orders() {
    for arrival in [
        ObservationArrivalOrder::CancellationFirst,
        ObservationArrivalOrder::CompletionFirst,
    ] {
        let request = ObserveEstablished::new(
            ObservationId(93),
            EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 13)),
        );
        let publisher = TerminalPublisher {
            reply_to: Recipient::global(RuntimeAddr(94)),
        };
        let mut monitor =
            EstablishedTerminationMonitor::established(publisher, request, publish_terminal);
        let initialized = behavior::initialize(&mut monitor).unwrap();
        assert_eq!(initialized.sends.owned.len(), 1);
        assert_eq!(initialized.sends.inner.len(), 0);
        assert_eq!(initialized.creates.len(), 0);
        assert_eq!(initialized.become_, Step::Continue);
        let mut emitted = initialized.sends.owned.into_iter();
        let authority = ObservationAuthority::issued(emitted.next().expect("whole actual Observe"));
        let extra = emitted.next();
        assert!(extra.is_none());
        let relationship = authority.relationship().clone();
        let started = behavior::delegate_transition(
            &mut monitor,
            EventLayer::Owned(EstablishedObservation::started(authority)),
        )
        .unwrap();
        assert_eq!(started.sends.owned.len(), 0);
        assert_eq!(started.sends.inner.len(), 0);
        assert_eq!(started.creates.len(), 0);
        assert_eq!(started.become_, Step::Continue);
        // This is the target's pure consuming recovery comparison. The
        // separate CancelObserver test proves emission from a real fold.
        let cancellation = monitor.take_cancellation().expect("sole original grant");
        let emitted = InterpreterRequests::one(cancellation);
        let mut cancellations = emitted.into_iter();
        let cancellation = cancellations.next().expect("whole named request lane");
        let extra = cancellations.next();
        assert!(extra.is_none());
        let at = Instant::now();
        let stopped = EstablishedObservation::stopped(
            relationship.clone(),
            Err(behavior_actors::Crash::Panicked),
            at,
        );
        let rejected = EstablishedObservation::cancel_rejected(
            cancellation,
            ObservationRejection::NotObserved,
        );
        let reports = match arrival {
            ObservationArrivalOrder::CancellationFirst => [rejected, stopped],
            ObservationArrivalOrder::CompletionFirst => [stopped, rejected],
        };
        let mut publications = Vec::new();
        for report in reports {
            let actions =
                behavior::delegate_transition(&mut monitor, EventLayer::Owned(report)).unwrap();
            assert_eq!(actions.sends.owned.len(), 0);
            publications.extend(actions.sends.inner);
            assert_eq!(actions.creates.len(), 0);
            assert_eq!(actions.become_, Step::Continue);
        }
        let mut shutdown = StopOnShutdown::new(monitor);
        let stopped =
            behavior::delegate_transition(&mut shutdown, EventLayer::Owned(ShutdownRequested))
                .unwrap();
        assert_eq!(stopped.sends.owned, NoSends);
        assert_eq!(stopped.sends.inner.owned.len(), 0);
        assert_eq!(stopped.sends.inner.inner.len(), 0);
        assert_eq!(stopped.creates.len(), 0);
        assert!(matches!(stopped.become_, Step::Stop(_)));
        let (publisher, target) = shutdown.into_inner().into_parts();
        // Complete pure shutdown actions precede the consuming custody oracle.
        let Err(target) = target.into_rejected_observe() else {
            panic!("wrong extraction must return all original target values");
        };
        let Ok((original, reason)) = target.into_rejected_cancel() else {
            panic!("retired wrapper retains whole rejected cancellation");
        };
        assert!(original.relationship() == &relationship);
        assert_eq!(original.id(), ObservationId(93));
        assert_eq!(reason, ObservationRejection::NotObserved);
        assert_eq!(publisher.reply_to.address(), RuntimeAddr(94));
        assert_eq!(publications.len(), 1);
        let mut published = publications.into_iter();
        let original_delivery = published.next().expect("one whole terminal publication");
        let extra = published.next();
        assert!(extra.is_none());
        assert!(original_delivery.to == publisher.reply_to);
        assert_eq!(original_delivery.to.address(), RuntimeAddr(94));
        let EstablishedObservation::Stopped {
            relationship: published_relationship,
            outcome,
            at: published_at,
        } = original_delivery.message
        else {
            panic!("whole original terminal fact");
        };
        assert!(published_relationship == relationship);
        assert_eq!(outcome, Err(behavior_actors::Crash::Panicked));
        assert_eq!(published_at, at);
        drop(original);
    }
}

#[test]
fn inner_shutdown_preserves_whole_rejected_cancel_after_both_report_orders() {
    for arrival in [
        ObservationArrivalOrder::CancellationFirst,
        ObservationArrivalOrder::CompletionFirst,
    ] {
        let request = ObserveEstablished::new(
            ObservationId(93),
            EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 13)),
        );
        let publisher = TerminalPublisher {
            reply_to: Recipient::global(RuntimeAddr(94)),
        };
        let mut monitor = EstablishedTerminationMonitor::established(
            StopOnShutdown::new(publisher),
            request,
            publish_terminal_through_shutdown,
        );
        let initialized = behavior::initialize(&mut monitor).unwrap();
        assert_eq!(initialized.sends.owned.len(), 1);
        assert_eq!(initialized.sends.inner.owned, NoSends);
        assert_eq!(initialized.sends.inner.inner.len(), 0);
        assert_eq!(initialized.creates.len(), 0);
        assert_eq!(initialized.become_, Step::Continue);
        let mut emitted = initialized.sends.owned.into_iter();
        let authority = ObservationAuthority::issued(emitted.next().expect("whole actual Observe"));
        let extra = emitted.next();
        assert!(extra.is_none());
        let relationship = authority.relationship().clone();
        let started = behavior::delegate_transition(
            &mut monitor,
            EventLayer::Owned(EstablishedObservation::started(authority)),
        )
        .unwrap();
        assert_eq!(started.sends.owned.len(), 0);
        assert_eq!(started.sends.inner.owned, NoSends);
        assert_eq!(started.sends.inner.inner.len(), 0);
        assert_eq!(started.creates.len(), 0);
        assert_eq!(started.become_, Step::Continue);
        // This is the target's pure consuming recovery comparison. The
        // separate CancelObserver test proves emission from a real fold.
        let cancellation = monitor.take_cancellation().expect("sole original grant");
        let emitted = InterpreterRequests::one(cancellation);
        let mut cancellations = emitted.into_iter();
        let cancellation = cancellations.next().expect("whole named request lane");
        let extra = cancellations.next();
        assert!(extra.is_none());
        let at = Instant::now();
        let stopped = EstablishedObservation::stopped(
            relationship.clone(),
            Err(behavior_actors::Crash::Panicked),
            at,
        );
        let rejected = EstablishedObservation::cancel_rejected(
            cancellation,
            ObservationRejection::NotObserved,
        );
        let reports = match arrival {
            ObservationArrivalOrder::CancellationFirst => [rejected, stopped],
            ObservationArrivalOrder::CompletionFirst => [stopped, rejected],
        };
        let mut publications = Vec::new();
        for report in reports {
            let actions =
                behavior::delegate_transition(&mut monitor, EventLayer::Owned(report)).unwrap();
            assert_eq!(actions.sends.owned.len(), 0);
            assert_eq!(actions.sends.inner.owned, NoSends);
            publications.extend(actions.sends.inner.inner);
            assert_eq!(actions.creates.len(), 0);
            assert_eq!(actions.become_, Step::Continue);
        }
        let stopped = behavior::delegate_transition(
            &mut monitor,
            EventLayer::Inner(EventLayer::Owned(ShutdownRequested)),
        )
        .unwrap();
        assert_eq!(stopped.sends.owned.len(), 0);
        assert_eq!(stopped.sends.inner.owned, NoSends);
        assert_eq!(stopped.sends.inner.inner.len(), 0);
        assert_eq!(stopped.creates.len(), 0);
        assert!(matches!(stopped.become_, Step::Stop(_)));
        let (shutdown, target) = monitor.into_parts();
        let publisher = shutdown.into_inner();
        // Complete pure shutdown actions precede the consuming custody oracle.
        let Err(target) = target.into_rejected_observe() else {
            panic!("wrong extraction must return all original target values");
        };
        let Ok((original, reason)) = target.into_rejected_cancel() else {
            panic!("retired wrapper retains whole rejected cancellation");
        };
        assert!(original.relationship() == &relationship);
        assert_eq!(original.id(), ObservationId(93));
        assert_eq!(reason, ObservationRejection::NotObserved);
        assert_eq!(publisher.reply_to.address(), RuntimeAddr(94));
        assert_eq!(publications.len(), 1);
        let mut published = publications.into_iter();
        let original_delivery = published.next().expect("one whole terminal publication");
        let extra = published.next();
        assert!(extra.is_none());
        assert!(original_delivery.to == publisher.reply_to);
        assert_eq!(original_delivery.to.address(), RuntimeAddr(94));
        let EstablishedObservation::Stopped {
            relationship: published_relationship,
            outcome,
            at: published_at,
        } = original_delivery.message
        else {
            panic!("whole original terminal fact");
        };
        assert!(published_relationship == relationship);
        assert_eq!(outcome, Err(behavior_actors::Crash::Panicked));
        assert_eq!(published_at, at);
        drop(original);
    }
}

#[test]
fn existing_worker_factory_result_accepts_affine_observation_definitions() {
    type Definition = EstablishedTerminationMonitor<Observer, WorkerProtocol>;
    fn owning_factory<F>(factory: F) -> F
    where
        F: FnMut(&u64) -> Result<WorkerSubmission<Definition, ImmediateActivation>, u64>,
    {
        factory
    }

    // Whole-request construction occurs in the interpreted factory, outside every fold.
    let mut factory = owning_factory(move |role: &u64| {
        let recipient = EstablishedRecipient::issued(Endpoint::new(RuntimeAddr(45), 13));
        let request = ObserveEstablished::new(ObservationId(*role), recipient);
        let monitor = EstablishedTerminationMonitor::established(
            Observer { events: Vec::new() },
            request,
            record_observation,
        );
        Ok(WorkerSubmission::immediate(monitor))
    });
    let first_role = 101;
    let second_role = 103;
    let first = factory(&first_role);
    let second = factory(&second_role);
    let Ok(first) = first else {
        panic!("actual first affine definition fits the existing factory result");
    };
    let Ok(second) = second else {
        panic!("actual next definition fits the same existing factory result");
    };
    // This is a constructor/trait-syntax proof, not a worker installation or
    // initializer trace. Distinct request construction has no finite ordinal
    // budget; whole input recovery remains covered by its owning request tests.
    drop((first, second));
}
