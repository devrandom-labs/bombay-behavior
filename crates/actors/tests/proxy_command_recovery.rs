//! Clean-room StableProxy application syntax regression.

use std::time::{Duration, Instant};

use behavior_actors::atomic::{
    ActivationPlan, ActivationStartRejection, BeginActivation, ImmediateActivation,
    InitialWorkerOutcome, InitializeWorker, ProxyControl, ProxyDiagnostic, ProxyDrain,
    ProxyOutcome, ProxyPhase, ReplacementOutcome, StableProxy, WorkerAttempt,
    WorkerCreationRejection, WorkerInitializationFailure, WorkerInitializationOutcome,
    WorkerInitializationReport, WorkerStartResult,
};
use behavior_actors::{
    ActionItem, Actions, Activate as _, Active, ActiveTurn, Address, Behavior, BehaviorActed,
    ChildCreationOutcome, ChildHead, ChildNamespaceExhausted, ChildStopped, CreateChild,
    CreationRejection, CreationSequence, CreationSettlement, CreationsSettled, EndpointAddress,
    EstablishedCreation, EstablishedRecipient, EstablishedShutdownResolved, Exit, ItemSettlement,
    Never, NoBirths, ObserveChild, Protocol, RoutedCreation, SettledItem, ShutdownId, Step,
    StopOnShutdown, User,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct Command(u8);

#[derive(Debug, Eq, PartialEq)]
struct Worker(u8);

#[derive(Debug, Eq, PartialEq)]
struct WorkerRejected(u8);

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = Command;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Command>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = WorkerRejected;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Hydrate(u8);

#[derive(Debug, Eq, PartialEq)]
struct HydrationUnavailable;

impl ActivationPlan for Hydrate {
    type Ready = u8;
    type Rejection = HydrationUnavailable;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        async move { Ok(self.0 + 1) }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct RejectHydration(u8);

#[derive(Clone, Copy)]
enum DrainArrival {
    StopThenShutdown,
    ShutdownThenStop,
}

impl ActivationPlan for RejectHydration {
    type Ready = Never;
    type Rejection = HydrationUnavailable;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        async move {
            let Self(_code) = self;
            Err(HydrationUnavailable)
        }
    }
}

fn worker_observation(_: &ObserveChild<Worker, ChildHead>) {}

fn worker_initialization<P>(_: &InitializeWorker<Worker, P>) {}

fn retain_initialization_failure(
    failure: WorkerInitializationFailure,
) -> WorkerInitializationFailure {
    match failure {
        WorkerInitializationFailure::EffectsRejected => {
            WorkerInitializationFailure::EffectsRejected
        }
        WorkerInitializationFailure::InterpreterCorrupt => {
            WorkerInitializationFailure::InterpreterCorrupt
        }
    }
}

fn created_worker(
    creation: CreateChild<RuntimeAddr, StopOnShutdown<Worker>>,
    endpoint: Endpoint,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<Worker>> {
    let (worker, _, kind) = creation.into_parts();
    worker_creation_result(ChildCreationOutcome::Established {
        established: EstablishedCreation::installed(
            worker,
            kind,
            EstablishedRecipient::issued(endpoint),
        ),
    })
}

fn worker_creation_result(
    result: ChildCreationOutcome<StopOnShutdown<Worker>, ChildHead>,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<Worker>> {
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(result))]
            .into_iter()
            .collect(),
    ))
}

#[test]
fn worker_start_result_has_three_domain_decisions() {
    fn classify(result: WorkerStartResult<Worker, Hydrate>) {
        match result {
            WorkerStartResult::CreationRejected {
                rejection,
                activation,
                stopped,
            } => {
                let _ = (rejection, activation, stopped);
            }
            WorkerStartResult::Ready { attempt, readiness } => {
                let _ = (attempt, readiness);
            }
            WorkerStartResult::Unavailable { attempt, drain } => {
                let (_attempt, _drain) = (attempt, drain);
            }
        }
    }

    let contract: fn(WorkerStartResult<Worker, Hydrate>) = classify;
    let _ = contract;
}

fn activation_request_item(_: &BeginActivation<Worker, Hydrate>)
where
    BeginActivation<Worker, Hydrate>:
        ActionItem<Accepted = (), Rejection = ActivationStartRejection, Prerequisite = Never>,
{
}

fn awaiting_activation<P>(
    worker: Worker,
    plan: P,
    endpoint: Endpoint,
) -> (Active<StableProxy<Worker, P>>, BeginActivation<Worker, P>)
where
    P: ActivationPlan,
{
    let (mut proxy, initialization) = awaiting_initialization(worker, plan, endpoint);
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("successful initialization begins activation");
    let mut activations = activating.sends.worker_activations.into_requests();
    let activation = activations
        .pop()
        .expect("the activation owner receives one request");
    assert!(activations.is_empty());
    (proxy, activation)
}

fn awaiting_initialization<P>(
    worker: Worker,
    plan: P,
    endpoint: Endpoint,
) -> (Active<StableProxy<Worker, P>>, InitializeWorker<Worker, P>)
where
    P: ActivationPlan,
{
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(worker, plan))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = created_worker(creation, endpoint);
    let committed = proxy
        .on(settled)
        .expect("committed creation enters initialization");
    let mut initializations = committed.sends.worker_initializations.into_requests();
    let initialization = initializations
        .pop()
        .expect("the child host receives one initialization request");
    assert!(initializations.is_empty());
    (proxy, initialization)
}

fn initial_worker_result<P>(outcome: ProxyOutcome<Worker, P>) -> WorkerStartResult<Worker, P>
where
    P: ActivationPlan,
{
    match outcome {
        ProxyOutcome::Initial {
            outcome: InitialWorkerOutcome::Resolved { result },
        } => result,
        _ => panic!("the proxy did not report its initial worker"),
    }
}

async fn ready_hydrated_proxy(
    worker: Worker,
    plan: Hydrate,
    endpoint: Endpoint,
) -> (Active<StableProxy<Worker, Hydrate>>, WorkerAttempt) {
    let (mut proxy, request) = awaiting_activation(worker, plan, endpoint);
    let worker = request.worker();
    let start = proxy
        .on(request.started())
        .expect("activation start is admitted before readiness");
    assert!(start.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(request.activate().await)
        .expect("matching readiness completes the worker start");
    match ready
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives initial readiness")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt: ready, .. },
                },
        } => assert_eq!(ready, worker),
        _ => panic!("initial readiness produced the wrong owner outcome"),
    }
    (proxy, worker)
}

fn commit_hydrated_worker(
    proxy: &mut Active<StableProxy<Worker, Hydrate>>,
    creation: CreateChild<RuntimeAddr, StopOnShutdown<Worker>>,
    endpoint: Endpoint,
) -> BeginActivation<Worker, Hydrate> {
    let settled = created_worker(creation, endpoint);
    let committed = proxy
        .on(settled)
        .expect("the successor creation commits exactly once");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the successor initialization is requested once");
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("the successor initialization authorizes activation");
    activating
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("the successor activation is requested once")
}

#[test]
fn dormant_owner_shutdown_stops_without_fabricating_an_owner_outcome() {
    let initialized = StableProxy::<Worker, Hydrate>::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;

    let stopped = proxy
        .on(ProxyControl::shutdown())
        .expect("owner shutdown is total while dormant");

    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert!(stopped.creates.is_empty());
    assert!(stopped.sends.worker_observations.is_empty());
    assert!(stopped.sends.worker_initializations.is_empty());
    assert!(stopped.sends.worker_activations.is_empty());
    assert!(stopped.sends.worker_shutdowns.is_empty());
    assert!(stopped.sends.worker_deliveries.is_empty());
    assert!(stopped.sends.owner_outcomes.is_empty());
    assert!(stopped.sends.diagnostics.is_empty());
}

async fn ready_owner_shutdown_finishes_in(arrival: DrainArrival) {
    let (mut proxy, worker) = ready_hydrated_proxy(Worker(80), Hydrate(8), Endpoint(180)).await;

    let started = proxy
        .on(ProxyControl::shutdown())
        .expect("ready owner shutdown begins exact worker departure");
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(started.become_, Step::Continue));
    let shutdown = started
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("ready shutdown emits one exact worker request");
    assert_eq!(shutdown.id, ShutdownId(worker.creation().get()));
    assert!(started.sends.owner_outcomes.is_empty());

    let repeated = proxy
        .on(ProxyControl::shutdown())
        .expect("repeated owner shutdown is idempotent");
    assert!(matches!(repeated.become_, Step::Continue));
    assert!(repeated.sends.worker_shutdowns.is_empty());
    assert!(repeated.sends.owner_outcomes.is_empty());

    let stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());
    let terminal = match arrival {
        DrainArrival::StopThenShutdown => {
            let waiting = proxy
                .on(stopped)
                .expect("exact stop waits for shutdown settlement");
            assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
            assert!(matches!(waiting.become_, Step::Continue));
            proxy
                .on(EstablishedShutdownResolved::accepted(shutdown.id))
                .expect("shutdown settlement closes exact departure")
        }
        DrainArrival::ShutdownThenStop => {
            let waiting = proxy
                .on(EstablishedShutdownResolved::accepted(shutdown.id))
                .expect("shutdown settlement waits for exact stop");
            assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
            assert!(matches!(waiting.become_, Step::Continue));
            proxy
                .on(stopped)
                .expect("exact stop closes worker departure")
        }
    };

    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
    assert!(terminal.sends.worker_shutdowns.is_empty());
}

#[tokio::test]
async fn ready_owner_shutdown_joins_stop_before_shutdown_resolution() {
    ready_owner_shutdown_finishes_in(DrainArrival::StopThenShutdown).await;
}

#[tokio::test]
async fn ready_owner_shutdown_joins_shutdown_resolution_before_stop() {
    ready_owner_shutdown_finishes_in(DrainArrival::ShutdownThenStop).await;
}

#[test]
fn owner_shutdown_waits_for_emitted_creation_rejection_and_retains_the_submission() {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(Worker(86), Hydrate(14)))
        .expect("initial start stages one creation");
    let creations = started.creates;

    let waiting = proxy
        .on(ProxyControl::shutdown())
        .expect("emitted creation cannot be reconstructed or cancelled");
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(waiting.become_, Step::Continue));
    assert!(waiting.creates.is_empty());

    let settled = CreationsSettled::new(CreationSettlement::Rejected {
        creations,
        reason: ChildNamespaceExhausted,
    });
    let terminal = proxy
        .on(settled)
        .expect("creation rejection closes terminal ownership");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[test]
fn owner_shutdown_after_committed_creation_waits_for_initialization_and_worker_departure() {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(Worker(87), Hydrate(15)))
        .expect("initial start stages one creation");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation was emitted");
    let worker = creation.id();
    let waiting = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown waits for the emitted creation result");
    assert!(matches!(waiting.become_, Step::Continue));
    assert!(waiting.sends.worker_shutdowns.is_empty());
    assert!(waiting.sends.owner_outcomes.is_empty());
    assert!(waiting.creates.is_empty());

    let settled = created_worker(creation, Endpoint(187));
    let draining = proxy
        .on(settled)
        .expect("committed worker starts initialization and exact departure");
    let initialization = draining
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("initialization still settles after shutdown");
    let duplicate = WorkerInitializationReport::EffectsRejected {
        worker: initialization.worker(),
        initialization: initialization.initialization(),
        activation: Hydrate(99),
        failure: WorkerInitializationFailure::EffectsRejected,
    };
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the committed worker begins one exact departure");

    let initialized = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("initialization settles without beginning activation");
    assert!(initialized.sends.worker_activations.is_empty());
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    let duplicate = proxy
        .on(duplicate)
        .expect("a duplicate initialization result is returned complete");
    match duplicate
        .sends
        .diagnostics
        .into_iter()
        .next()
        .expect("the duplicate initialization result is diagnosed")
        .into_inner()
    {
        ProxyDiagnostic::UnexpectedWorkerInitialization { initialization, .. } => {
            match initialization {
                WorkerInitializationReport::EffectsRejected { activation, .. } => {
                    assert_eq!(activation, Hydrate(99));
                }
                _ => panic!("the duplicate initialization result changed"),
            }
        }
        _ => panic!("the duplicate initialization produced the wrong diagnostic"),
    }
    let waiting = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .expect("shutdown result waits for exact stop");
    assert!(matches!(waiting.become_, Step::Continue));
    let terminal = proxy
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .expect("exact stop closes initialization and departure ownership");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[test]
fn owner_shutdown_after_stop_before_creation_never_sends_duplicate_worker_shutdown() {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(Worker(88), Hydrate(16)))
        .expect("initial start stages one creation");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation was emitted");
    let worker = creation.id();
    let waiting_for_creation = proxy
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .expect("early stop waits for creation settlement");
    assert!(matches!(waiting_for_creation.become_, Step::Continue));
    assert!(waiting_for_creation.sends.worker_shutdowns.is_empty());
    assert!(waiting_for_creation.sends.owner_outcomes.is_empty());
    assert!(waiting_for_creation.creates.is_empty());
    let waiting_for_creation = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown preserves the already observed stop");
    assert!(matches!(waiting_for_creation.become_, Step::Continue));
    assert!(waiting_for_creation.sends.worker_shutdowns.is_empty());
    assert!(waiting_for_creation.sends.owner_outcomes.is_empty());
    assert!(waiting_for_creation.creates.is_empty());

    let settled = created_worker(creation, Endpoint(188));
    let initializing = proxy
        .on(settled)
        .expect("committed creation still settles initialization");
    assert!(initializing.sends.worker_shutdowns.is_empty());
    let initialization = initializing
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("initialization result remains required");
    let terminal = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("initialization closes the already stopped worker");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.worker_activations.is_empty());
    assert!(terminal.sends.worker_shutdowns.is_empty());
}

#[test]
fn owner_shutdown_waiting_for_activation_start_retains_the_returned_request() {
    let (mut proxy, request) = awaiting_activation(Worker(89), Hydrate(17), Endpoint(189));
    let worker = request.worker();
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("activation-start shutdown begins exact worker departure");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("one exact shutdown is emitted");

    let activation = proxy
        .on(request.start_rejected(ActivationStartRejection::OwnerStopped))
        .expect("the complete unaccepted activation request is retained");
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(activation.become_, Step::Continue));
    assert!(activation.sends.owner_outcomes.is_empty());

    let waiting = proxy
        .on(ChildStopped::new(
            worker.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("worker stop waits for shutdown resolution");
    assert!(matches!(waiting.become_, Step::Continue));
    let terminal = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .expect("activation return and worker departure close shutdown");
    assert_eq!(shutdown.id, ShutdownId(worker.creation().get()));
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[tokio::test]
async fn owner_shutdown_during_running_activation_never_publishes_late_readiness() {
    let (mut proxy, request) = awaiting_activation(Worker(90), Hydrate(18), Endpoint(190));
    let worker = request.worker();
    let started = proxy
        .on(request.started())
        .expect("activation work is admitted before owner shutdown");
    assert!(started.sends.owner_outcomes.is_empty());
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("running activation closes readiness and begins worker departure");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("one exact shutdown is emitted");

    let activation = proxy
        .on(request.activate().await)
        .expect("late readiness is retained without opening service");
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(activation.become_, Step::Continue));
    assert!(activation.sends.owner_outcomes.is_empty());

    let waiting = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .expect("shutdown resolution waits for worker stop");
    assert!(matches!(waiting.become_, Step::Continue));
    let terminal = proxy
        .on(ChildStopped::new(
            worker.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("late readiness and worker departure close shutdown");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[tokio::test]
async fn worker_stop_during_running_activation_prevents_availability() {
    let (mut proxy, request) = awaiting_activation(Worker(93), Hydrate(21), Endpoint(193));
    let worker = request.worker();
    let admitted = proxy
        .on(request.started())
        .expect("activation work is admitted before the worker stops");
    assert!(admitted.sends.owner_outcomes.is_empty());

    let waiting = proxy
        .on(ChildStopped::new(
            worker.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the exact worker stop is retained until activation returns");
    assert!(waiting.sends.owner_outcomes.is_empty());

    let completed = proxy
        .on(request.activate().await)
        .expect("readiness after the exact stop closes the unavailable worker");
    assert_eq!(proxy.phase(), ProxyPhase::EmptyAfter);
    match completed
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives one stopped-before-ready outcome")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Unavailable { attempt, drain },
                },
        } => {
            assert_eq!(attempt, worker);
            let ProxyDrain::ActivationCompleted { readiness, stopped } = drain else {
                panic!("readiness or the exact stop was lost");
            };
            assert_eq!(readiness, 22);
            assert_eq!(stopped.child, worker.creation());
        }
        _ => panic!("readiness after stop produced the wrong owner outcome"),
    }
}

#[tokio::test]
async fn worker_stop_before_activation_admission_returns_the_plan_without_shutdown() {
    let (mut proxy, activation) = awaiting_activation(Worker(94), Hydrate(22), Endpoint(194));
    let worker = activation.worker();
    let worker_stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());

    let waiting_for_activation = proxy
        .on(worker_stopped)
        .expect("the proxy retains its worker's stop while activation admission is pending");
    assert!(waiting_for_activation.sends.worker_shutdowns.is_empty());

    let worker_unavailable = proxy
        .on(activation.start_rejected(ActivationStartRejection::OwnerStopped))
        .expect("rejected activation admission returns the stopped worker's plan");
    let result = initial_worker_result(
        worker_unavailable
            .sends
            .owner_outcomes
            .into_requests()
            .pop()
            .expect("the proxy owner receives the unavailable worker")
            .into_inner(),
    );
    let WorkerStartResult::Unavailable { attempt, drain } = result else {
        panic!("the stopped worker became available");
    };
    assert_eq!(attempt, worker);
    let ProxyDrain::ActivationStartRejected {
        request,
        reason,
        shutdown,
        stopped,
    } = drain
    else {
        panic!("the proxy lost the rejected activation request");
    };
    assert_eq!(reason, ActivationStartRejection::OwnerStopped);
    assert!(shutdown.is_none());
    assert_eq!(stopped, worker_stopped);
    let readiness = match request.activate().await.into_ready() {
        Ok(readiness) => readiness,
        Err(_) => panic!("the returned activation plan did not complete"),
    };
    assert_eq!(readiness, 23);
}

#[tokio::test]
async fn worker_stop_before_activation_failure_returns_the_failure_without_shutdown() {
    let (mut proxy, activation) =
        awaiting_activation(Worker(95), RejectHydration(30), Endpoint(195));
    let worker = activation.worker();
    let admitted = proxy
        .on(activation.started())
        .expect("the worker begins its activation plan");
    assert!(admitted.sends.worker_shutdowns.is_empty());
    let worker_stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());
    let waiting_for_activation = proxy
        .on(worker_stopped)
        .expect("the proxy retains its worker's stop while activation is running");
    assert!(waiting_for_activation.sends.worker_shutdowns.is_empty());

    let worker_unavailable = proxy
        .on(activation.activate().await)
        .expect("activation failure returns the already stopped worker");
    let result = initial_worker_result(
        worker_unavailable
            .sends
            .owner_outcomes
            .into_requests()
            .pop()
            .expect("the proxy owner receives the unavailable worker")
            .into_inner(),
    );
    let WorkerStartResult::Unavailable { attempt, drain } = result else {
        panic!("the stopped worker became available");
    };
    assert_eq!(attempt, worker);
    let ProxyDrain::ActivationRejected {
        rejection,
        shutdown,
        stopped,
    } = drain
    else {
        panic!("the proxy lost the activation failure");
    };
    assert_eq!(rejection, HydrationUnavailable);
    assert!(shutdown.is_none());
    assert_eq!(stopped, worker_stopped);
}

#[test]
fn owner_shutdown_after_activation_worker_stop_emits_no_duplicate_shutdown() {
    let (mut proxy, request) = awaiting_activation(Worker(91), Hydrate(19), Endpoint(191));
    let worker = request.worker();
    let stopped = proxy
        .on(ChildStopped::new(
            worker.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("early worker stop is retained while activation start settles");
    assert!(matches!(stopped.become_, Step::Continue));
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("owner shutdown preserves the observed stop");
    assert!(draining.sends.worker_shutdowns.is_empty());

    let terminal = proxy
        .on(request.start_rejected(ActivationStartRejection::OwnerStopped))
        .expect("activation return closes the already stopped worker");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[test]
fn owner_shutdown_during_pre_ready_return_emits_no_duplicate_shutdown_or_outcome() {
    let (mut proxy, request) = awaiting_activation(Worker(92), Hydrate(20), Endpoint(192));
    let worker = request.worker();
    let returning = proxy
        .on(request.start_rejected(ActivationStartRejection::OwnerStopped))
        .expect("activation rejection begins the existing worker departure");
    let shutdown = returning
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("pre-ready return emits one exact shutdown");
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("owner shutdown changes only terminal disposition");
    assert!(draining.sends.worker_shutdowns.is_empty());
    assert!(draining.sends.owner_outcomes.is_empty());

    let waiting = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .expect("shutdown result waits for exact worker stop");
    assert!(matches!(waiting.become_, Step::Continue));
    let terminal = proxy
        .on(ChildStopped::new(
            worker.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("pre-ready return closes into proxy retirement");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[tokio::test]
async fn predecessor_stop_stages_one_successor_before_late_shutdown_resolution() {
    let (mut proxy, previous) = ready_hydrated_proxy(Worker(64), Hydrate(8), Endpoint(164)).await;

    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(65), Hydrate(9)))
        .expect("replacement begins exact predecessor return");
    let shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");

    let stopped = proxy
        .on(ChildStopped::new(
            previous.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the exact predecessor stop stages the successor");
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    assert_eq!(stopped.creates.len(), 1);
    match stopped
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives the exact predecessor stop")
        .into_inner()
    {
        ProxyOutcome::WorkerStopped { worker, .. } => assert_eq!(worker, previous),
        _ => panic!("replacement stop produced the wrong owner outcome"),
    }

    let settled = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("late shutdown resolution belongs to the active replacement");
    assert!(settled.sends.diagnostics.is_empty());
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
}

#[tokio::test]
async fn predecessor_shutdown_resolution_then_stop_stages_one_successor() {
    let (mut proxy, previous) = ready_hydrated_proxy(Worker(66), Hydrate(10), Endpoint(166)).await;

    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(67), Hydrate(11)))
        .expect("replacement begins exact predecessor return");
    let shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");

    let resolved = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("the matching shutdown result settles its side of the join");
    assert!(resolved.creates.is_empty());
    assert!(resolved.sends.owner_outcomes.is_empty());

    let stopped = proxy
        .on(ChildStopped::new(
            previous.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the later predecessor stop stages the successor");
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    assert_eq!(stopped.creates.len(), 1);
    match stopped
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives the exact predecessor stop")
        .into_inner()
    {
        ProxyOutcome::WorkerStopped { worker, .. } => assert_eq!(worker, previous),
        _ => panic!("replacement stop produced the wrong owner outcome"),
    }
}

#[tokio::test]
async fn ready_successor_waits_for_late_predecessor_shutdown_resolution() {
    let (mut proxy, previous) = ready_hydrated_proxy(Worker(68), Hydrate(12), Endpoint(168)).await;

    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(69), Hydrate(13)))
        .expect("replacement begins exact predecessor return");
    let shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");
    let stopped = proxy
        .on(ChildStopped::new(
            previous.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the predecessor stop stages the successor");
    let creation = stopped
        .creates
        .into_iter()
        .next()
        .expect("the successor is created only after predecessor stop");
    let activation = commit_hydrated_worker(&mut proxy, creation, Endpoint(169));
    let successor = activation.worker();
    let activation_started = proxy
        .on(activation.started())
        .expect("successor activation begins exactly once");
    assert!(activation_started.creates.is_empty());
    assert!(activation_started.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .expect("successor readiness joins the pending predecessor settlement");

    assert_eq!(proxy.phase(), ProxyPhase::Replacing);
    assert!(ready.sends.owner_outcomes.is_empty());
    let published = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("late predecessor settlement publishes the retained replacement result");
    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    match published
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives one replacement result")
        .into_inner()
    {
        ProxyOutcome::Replacement {
            outcome:
                ReplacementOutcome::Resolved {
                    replaces,
                    result: WorkerStartResult::Ready { attempt, readiness },
                },
        } => {
            assert_eq!(replaces, previous);
            assert_eq!(attempt, successor);
            assert_eq!(readiness, 14);
        }
        _ => panic!("late predecessor settlement produced the wrong replacement outcome"),
    }
}

#[tokio::test]
async fn owner_shutdown_returns_ready_successor_before_late_predecessor_resolution() {
    let (mut proxy, predecessor) =
        ready_hydrated_proxy(Worker(93), Hydrate(21), Endpoint(193)).await;

    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(94), Hydrate(22)))
        .expect("replacement begins exact predecessor return");
    let predecessor_shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");
    let stopped = proxy
        .on(ChildStopped::new(
            predecessor.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the predecessor stop stages the successor");
    let creation = stopped
        .creates
        .into_iter()
        .next()
        .expect("the successor is created only after predecessor stop");
    let activation = commit_hydrated_worker(&mut proxy, creation, Endpoint(194));
    let successor = activation.worker();
    let started = proxy
        .on(activation.started())
        .expect("successor activation starts exactly once");
    assert!(started.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .expect("successor readiness remains private until predecessor settlement");
    assert!(ready.sends.owner_outcomes.is_empty());
    assert_eq!(proxy.phase(), ProxyPhase::Replacing);

    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("owner shutdown returns the unpublished ready successor");
    let successor_shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the ready successor begins one exact departure");
    let successor_creation = successor.creation();
    assert_eq!(successor_shutdown.id, ShutdownId(successor_creation.get()));
    assert!(draining.sends.owner_outcomes.is_empty());
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);

    let predecessor = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(
            predecessor_shutdown.id,
        ))
        .expect("the predecessor result is retained without publishing replacement");
    assert!(matches!(predecessor.become_, Step::Continue));
    assert!(predecessor.sends.owner_outcomes.is_empty());
    let successor_settled = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(
            successor_shutdown.id,
        ))
        .expect("the successor result waits for its exact stop");
    assert!(matches!(successor_settled.become_, Step::Continue));
    let terminal = proxy
        .on(ChildStopped::new(
            successor_creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("both worker returns close the proxy");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[tokio::test]
async fn replacement_from_empty_after_starts_a_fresh_worker() {
    let (mut proxy, previous) = ready_hydrated_proxy(Worker(70), Hydrate(14), Endpoint(170)).await;
    let stopped = proxy
        .on(ChildStopped::new(
            previous.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the current worker stop leaves replacement provenance");
    assert_eq!(stopped.sends.owner_outcomes.len(), 1);
    assert_eq!(proxy.phase(), ProxyPhase::EmptyAfter);

    let creating = proxy
        .on(ProxyControl::replace_with(Worker(71), Hydrate(15)))
        .expect("empty replacement stages the successor immediately");
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("empty replacement creates one successor");
    let activation = commit_hydrated_worker(&mut proxy, creation, Endpoint(171));
    let successor = activation.worker();
    let activation_started = proxy
        .on(activation.started())
        .expect("successor activation begins exactly once");
    assert!(activation_started.creates.is_empty());
    assert!(activation_started.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .expect("successor readiness completes empty replacement");
    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    match ready
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives one replacement result")
        .into_inner()
    {
        ProxyOutcome::Replacement {
            outcome:
                ReplacementOutcome::Resolved {
                    replaces,
                    result: WorkerStartResult::Ready { attempt, readiness },
                },
        } => {
            assert_eq!(replaces, previous);
            assert_eq!(attempt, successor);
            assert_eq!(readiness, 16);
        }
        _ => panic!("empty replacement produced the wrong owner outcome"),
    }
}

#[tokio::test]
async fn overlapping_replacement_returns_the_complete_successor() {
    let (mut proxy, _) = ready_hydrated_proxy(Worker(72), Hydrate(16), Endpoint(172)).await;
    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(73), Hydrate(17)))
        .expect("first replacement begins predecessor shutdown");
    assert_eq!(waiting.sends.worker_shutdowns.len(), 1);

    let overlap = proxy
        .on(ProxyControl::replace_with(Worker(74), Hydrate(18)))
        .expect("overlap rejection is a total proxy transition");
    match overlap
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the complete overlapping replacement is returned")
        .into_inner()
    {
        ProxyOutcome::Replacement {
            outcome:
                ReplacementOutcome::NotReplaceable {
                    worker,
                    activation,
                    phase,
                },
        } => {
            assert_eq!(worker, Worker(74));
            assert_eq!(activation, Hydrate(18));
            assert_eq!(phase, ProxyPhase::Replacing);
        }
        _ => panic!("overlap produced the wrong replacement outcome"),
    }
}

#[tokio::test]
async fn foreign_predecessor_shutdown_result_cannot_advance_replacement() {
    let (mut proxy, predecessor) =
        ready_hydrated_proxy(Worker(75), Hydrate(19), Endpoint(175)).await;
    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(76), Hydrate(20)))
        .expect("replacement begins exact predecessor return");
    let shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");

    let foreign = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(ShutdownId(
            999,
        )))
        .expect("a foreign shutdown result is returned as a diagnostic");
    assert_eq!(proxy.phase(), ProxyPhase::Replacing);
    assert!(foreign.creates.is_empty());
    assert!(foreign.sends.owner_outcomes.is_empty());
    match foreign
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("the foreign result remains owned by one diagnostic")
        .into_inner()
    {
        ProxyDiagnostic::UnexpectedWorkerShutdown { phase, shutdown } => {
            assert_eq!(phase, ProxyPhase::Replacing);
            assert_eq!(shutdown.id(), ShutdownId(999));
        }
        _ => panic!("foreign shutdown produced the wrong diagnostic"),
    }

    let matching = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("the matching shutdown result still advances the retained join");
    assert!(matching.creates.is_empty());
    assert!(matching.sends.diagnostics.is_empty());
    let stopped = proxy
        .on(ChildStopped::new(
            predecessor.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the exact stop still stages the retained successor");
    assert_eq!(stopped.creates.len(), 1);
}

#[tokio::test]
async fn rejected_successor_waits_for_late_predecessor_shutdown_resolution() {
    let (mut proxy, previous) = ready_hydrated_proxy(Worker(77), Hydrate(21), Endpoint(177)).await;
    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(78), Hydrate(22)))
        .expect("replacement begins exact predecessor return");
    let shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");
    let stopped = proxy
        .on(ChildStopped::new(
            previous.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the predecessor stop stages the successor");
    let creation = stopped
        .creates
        .into_iter()
        .next()
        .expect("the successor is created only after predecessor stop");
    let rejected = proxy
        .on(worker_creation_result(
            ChildCreationOutcome::InitializationRejected {
                creation: RoutedCreation::new(creation, 278),
                error: WorkerRejected(23),
            },
        ))
        .expect("successor rejection joins the pending predecessor settlement");

    assert_eq!(proxy.phase(), ProxyPhase::Replacing);
    assert!(rejected.sends.owner_outcomes.is_empty());
    let published = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("late predecessor settlement publishes the retained rejection");
    assert_eq!(proxy.phase(), ProxyPhase::EmptyAfter);
    match published
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the owner receives one complete replacement result")
        .into_inner()
    {
        ProxyOutcome::Replacement {
            outcome:
                ReplacementOutcome::Resolved {
                    replaces,
                    result:
                        WorkerStartResult::CreationRejected {
                            rejection: WorkerCreationRejection::WorkerRejected { worker, error },
                            activation,
                            stopped: None,
                        },
                },
        } => {
            assert_eq!(replaces, previous);
            assert_eq!(worker, Worker(78));
            assert_eq!(error, WorkerRejected(23));
            assert_eq!(activation, Hydrate(22));
        }
        _ => panic!("late predecessor settlement lost the successor rejection"),
    }
}

#[tokio::test]
async fn owner_shutdown_retains_rejected_successor_until_predecessor_resolution() {
    let (mut proxy, predecessor) =
        ready_hydrated_proxy(Worker(95), Hydrate(23), Endpoint(195)).await;
    let waiting = proxy
        .on(ProxyControl::replace_with(Worker(96), Hydrate(24)))
        .expect("replacement begins exact predecessor return");
    let predecessor_shutdown = waiting
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");
    let stopped = proxy
        .on(ChildStopped::new(
            predecessor.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .expect("the predecessor stop stages the successor");
    let creation = stopped
        .creates
        .into_iter()
        .next()
        .expect("the successor is created only after predecessor stop");
    let rejected = proxy
        .on(worker_creation_result(
            ChildCreationOutcome::InitializationRejected {
                creation: RoutedCreation::new(creation, 296),
                error: WorkerRejected(24),
            },
        ))
        .expect("successor rejection remains private until predecessor settlement");
    assert!(rejected.sends.owner_outcomes.is_empty());

    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("owner shutdown changes the retained rejection disposition");
    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(draining.become_, Step::Continue));
    assert!(draining.sends.worker_shutdowns.is_empty());
    assert!(draining.sends.owner_outcomes.is_empty());

    let terminal = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(
            predecessor_shutdown.id,
        ))
        .expect("the exact predecessor result closes all retained replacement values");
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(terminal.become_, Step::Stop(_)));
    assert!(terminal.sends.owner_outcomes.is_empty());
}

#[test]
fn canonical_construction_infers_worker_and_activation_without_runtime_types() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let start = proxy
        .on(ProxyControl::start(Worker(1)))
        .expect("initial worker start is total");
    assert_eq!(start.creates.len(), 1);
    assert_eq!(start.sends.worker_observations.len(), 1);
    worker_observation(&start.sends.worker_observations[0]);

    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let start = proxy
        .on(ProxyControl::start_with(Worker(2), Hydrate(9)))
        .expect("reported activation is inferred from its plan");
    assert_eq!(start.creates.len(), 1);
    assert_eq!(start.sends.worker_observations.len(), 1);
    worker_observation(&start.sends.worker_observations[0]);
}

#[test]
fn immediate_start_has_no_unused_plan_input() {
    fn accepts(_: ProxyControl<Worker, ImmediateActivation>) {}
    accepts(ProxyControl::start(Worker(1)));
}

#[test]
fn rejected_worker_is_returned_and_initial_empty_provenance_is_preserved() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(7)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = worker_creation_result(ChildCreationOutcome::InitializationRejected {
        creation: RoutedCreation::new(creation, 707),
        error: WorkerRejected(11),
    });

    let rejected = proxy
        .on(settled)
        .expect("worker rejection is a total proxy transition");
    assert_eq!(proxy.phase(), ProxyPhase::EmptyInitial);
    assert_eq!(rejected.sends.owner_outcomes.len(), 1);
    assert!(rejected.sends.diagnostics.is_empty());
    match rejected
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one complete outcome")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result:
                        WorkerStartResult::CreationRejected {
                            activation,
                            rejection: WorkerCreationRejection::WorkerRejected { worker, error },
                            stopped: None,
                        },
                },
        } => {
            assert_eq!(worker, Worker(7));
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(error, WorkerRejected(11));
        }
        _ => panic!("unexpected proxy outcome"),
    }
}

#[test]
fn overlapping_start_returns_the_complete_submission() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(1)))
        .expect("initial worker start is total");
    assert_eq!(started.creates.len(), 1);

    let overlap = proxy
        .on(ProxyControl::start(Worker(2)))
        .expect("overlap rejection is a total proxy transition");
    assert!(overlap.creates.is_empty());
    assert!(overlap.sends.diagnostics.is_empty());
    match overlap
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives the rejected submission")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Overlap {
                    worker,
                    activation,
                    phase,
                },
        } => {
            assert_eq!(worker, Worker(2));
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(phase, ProxyPhase::Creating);
        }
        _ => panic!("unexpected proxy outcome"),
    }
}

#[test]
fn non_ready_service_input_is_returned_to_the_owner() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(1)))
        .expect("initial worker start is total");
    assert_eq!(started.creates.len(), 1);

    let unavailable = proxy
        .receive(RuntimeAddr(19), Command(23))
        .expect("unavailable service input is a total proxy transition");
    match unavailable
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives the complete service input")
        .into_inner()
    {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(19));
            assert_eq!(phase, ProxyPhase::Creating);
            assert_eq!(command, Command(23));
        }
        _ => panic!("unexpected proxy outcome"),
    }
}

#[test]
fn foreign_worker_result_is_returned_without_disturbing_the_expected_start() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(1)))
        .expect("initial worker start is total");
    assert_eq!(started.creates.len(), 1);
    let target_worker = started
        .creates
        .iter()
        .next()
        .expect("one target worker creation is staged")
        .id();
    let mut foreign_workers = CreationSequence::new();
    let _first_worker = foreign_workers
        .issue()
        .expect("the foreign sequence issues its first worker ID");
    let foreign_worker = foreign_workers
        .issue()
        .expect("the foreign sequence issues a distinct worker ID");
    assert_ne!(foreign_worker, target_worker);
    let foreign_route = 909;
    let foreign = CreateChild::birth(foreign_worker, StopOnShutdown::new(Worker(9)));
    let settlement = worker_creation_result(ChildCreationOutcome::InitializationRejected {
        creation: RoutedCreation::new(foreign, foreign_route),
        error: WorkerRejected(4),
    });

    let returned = proxy
        .on(settlement)
        .expect("foreign worker result is returned explicitly");
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    assert!(returned.sends.owner_outcomes.is_empty());
    let diagnostic = returned
        .sends
        .diagnostics
        .into_iter()
        .next()
        .expect("the owner receives the foreign result")
        .into_inner();
    let ProxyDiagnostic::UnexpectedWorkerStart { phase, workers } = diagnostic else {
        panic!("a creation settlement changed lifecycle category");
    };
    assert_eq!(phase, ProxyPhase::Creating);
    let CreationSettlement::Settled(workers) = workers.into_settlement() else {
        panic!("the routed worker result changed settlement category");
    };
    let SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::InitializationRejected { creation, error },
    )) = workers
        .into_iter()
        .next()
        .expect("the complete foreign worker result is retained")
    else {
        panic!("unexpected returned worker result");
    };
    assert_eq!(creation.id(), foreign_worker);
    assert_eq!(creation.route(), foreign_route);
    assert_eq!(error, WorkerRejected(4));
}

#[test]
fn committed_worker_enters_initializing_without_publishing_readiness() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(1)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = created_worker(creation, Endpoint(31));

    let committed = proxy
        .on(settled)
        .expect("committed creation is a total proxy transition");
    assert_eq!(proxy.phase(), ProxyPhase::Initializing);
    assert_eq!(committed.sends.worker_initializations.len(), 1);
    worker_initialization(&committed.sends.worker_initializations[0]);
    assert!(committed.sends.owner_outcomes.is_empty());
    assert!(committed.sends.diagnostics.is_empty());

    let request = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let expected_worker = request.worker();
    let expected_initialization = request.initialization();
    match request.resolve(WorkerInitializationOutcome::ReadyForActivation) {
        WorkerInitializationReport::ReadyForActivation {
            worker,
            initialization,
            activation,
            permit,
        } => {
            assert_eq!(worker, expected_worker);
            assert_eq!(initialization, expected_initialization);
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(permit.worker(), expected_worker);
            assert_eq!(permit.initialization(), expected_initialization);
        }
        _ => panic!("successful initialization returned the wrong authority"),
    }
}

#[tokio::test]
async fn successful_initialization_authorizes_one_correlated_activation() {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(Worker(2), Hydrate(9)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = created_worker(creation, Endpoint(51));
    let committed = proxy
        .on(settled)
        .expect("committed creation is a total proxy transition");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let expected_worker = initialization.worker();
    let expected_initialization = initialization.initialization();
    let worker_initialized =
        initialization.resolve(WorkerInitializationOutcome::ReadyForActivation);
    let activating = proxy
        .on(worker_initialized)
        .expect("matching initialization is a total proxy transition");
    assert_eq!(proxy.phase(), ProxyPhase::Activating);
    assert!(activating.sends.owner_outcomes.is_empty());
    assert!(activating.sends.worker_deliveries.is_empty());
    assert_eq!(activating.sends.worker_activations.len(), 1);
    let mut activation_requests = activating.sends.worker_activations.into_requests();
    let request = activation_requests
        .pop()
        .expect("successful initialization emits one activation request");
    assert!(activation_requests.is_empty());

    assert_eq!(request.target(), EstablishedRecipient::issued(Endpoint(51)));
    assert_eq!(request.worker(), expected_worker);
    assert_eq!(request.initialization(), expected_initialization);
    activation_request_item(&request);

    match ActivationStartRejection::OwnerStopped {
        ActivationStartRejection::OwnerStopped => {}
    }

    let activation_started = request.started();
    assert_eq!(activation_started.worker(), expected_worker);
    let accepted = proxy
        .on(activation_started)
        .expect("activation start is a total proxy transition");
    assert_eq!(proxy.phase(), ProxyPhase::Activating);
    assert!(accepted.sends.owner_outcomes.is_empty());

    let ready = request.activate().await;
    let activated = proxy
        .on(ready)
        .expect("matching readiness is a total proxy transition");
    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    match activated
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one ready result")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, readiness },
                },
        } => {
            assert_eq!(attempt, expected_worker);
            assert_eq!(readiness, 10);
        }
        _ => panic!("matching activation produced the wrong owner result"),
    }

    let forwarded = proxy
        .receive(RuntimeAddr(61), Command(17))
        .expect("ready service delivery is a total proxy transition");
    assert_eq!(forwarded.sends.worker_deliveries.len(), 1);
    assert_eq!(
        forwarded.sends.worker_deliveries[0].to,
        EstablishedRecipient::issued(Endpoint(51))
    );
    assert_eq!(forwarded.sends.worker_deliveries[0].message, Command(17));
    assert!(forwarded.sends.owner_outcomes.is_empty());
}

#[tokio::test]
async fn readiness_before_activation_start_is_returned_and_cannot_open_service() {
    let (mut proxy, request) = awaiting_activation(Worker(40), Hydrate(19), Endpoint(89));
    let worker = request.worker();
    let started = request.started();
    let ready = request.activate().await;
    let rejected = proxy
        .on(ready)
        .expect("readiness before activation start is a total diagnostic transition");
    assert_eq!(proxy.phase(), ProxyPhase::Activating);
    assert!(rejected.sends.owner_outcomes.is_empty());
    assert!(rejected.sends.worker_deliveries.is_empty());
    let ready = match rejected
        .sends
        .diagnostics
        .into_iter()
        .next()
        .expect("the early readiness is returned")
        .into_inner()
    {
        ProxyDiagnostic::UnexpectedWorkerActivation { phase, activation } => {
            assert_eq!(phase, ProxyPhase::Activating);
            activation
        }
        _ => panic!("early readiness changed lifecycle category"),
    };

    let accepted = proxy
        .on(started)
        .expect("the exact activation start advances its phase");
    assert!(accepted.sends.owner_outcomes.is_empty());
    let completed = proxy
        .on(ready)
        .expect("returned readiness remains the exact authorized result");
    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    match completed
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one readiness result")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, readiness },
                },
        } => {
            assert_eq!(attempt, worker);
            assert_eq!(readiness, 20);
        }
        _ => panic!("returned readiness produced the wrong owner result"),
    }
}

#[tokio::test]
async fn activation_start_rejection_drains_the_exact_worker_before_reporting() {
    let (mut proxy, request) = awaiting_activation(Worker(41), Hydrate(23), Endpoint(97));
    let worker = request.worker();
    let draining = proxy
        .on(request.start_rejected(ActivationStartRejection::OwnerStopped))
        .expect("exact activation start rejection begins worker drain");
    assert_eq!(proxy.phase(), ProxyPhase::ReturningWorker);
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    assert!(draining.sends.owner_outcomes.is_empty());
    let mut shutdowns = draining.sends.worker_shutdowns.into_requests();
    let shutdown = shutdowns
        .pop()
        .expect("one exact worker shutdown is requested");
    let stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());
    let waiting = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("shutdown resolution waits for exact worker stop");
    assert!(waiting.sends.owner_outcomes.is_empty());
    let completed = proxy
        .on(stopped)
        .expect("worker stop completes activation-start rejection drain");

    match completed
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one activation-start rejection")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Unavailable { attempt, drain },
                },
        } => {
            assert_eq!(attempt, worker);
            let ProxyDrain::ActivationStartRejected {
                request,
                reason,
                shutdown: Some(shutdown),
                stopped,
            } = drain
            else {
                panic!("the drain lost the complete unaccepted request");
            };
            assert_eq!(reason, ActivationStartRejection::OwnerStopped);
            assert_eq!(shutdown.id(), ShutdownId(worker.creation().get()));
            assert_eq!(stopped.child, worker.creation());
            let readiness = match request.activate().await.into_ready() {
                Ok(readiness) => readiness,
                Err(_) => panic!("the returned request lost its activation plan"),
            };
            assert_eq!(readiness, 24);
        }
        _ => panic!("activation-start rejection produced the wrong owner result"),
    }
}

#[tokio::test]
async fn accepted_activation_rejection_drains_the_exact_worker_before_reporting() {
    let (mut proxy, request) = awaiting_activation(Worker(42), RejectHydration(29), Endpoint(101));
    let worker = request.worker();
    let started = request.started();
    let accepted = proxy
        .on(started)
        .expect("activation start is a total proxy transition");
    assert!(accepted.sends.owner_outcomes.is_empty());
    let rejected = request.activate().await;
    let draining = proxy
        .on(rejected)
        .expect("application activation rejection begins worker drain");
    assert_eq!(proxy.phase(), ProxyPhase::ReturningWorker);
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    assert!(draining.sends.owner_outcomes.is_empty());
    let mut shutdowns = draining.sends.worker_shutdowns.into_requests();
    let shutdown = shutdowns
        .pop()
        .expect("one exact worker shutdown is requested");
    let stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());
    let waiting = proxy
        .on(stopped)
        .expect("worker stop waits for shutdown resolution");
    assert!(waiting.sends.owner_outcomes.is_empty());
    let completed = proxy
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("shutdown resolution completes activation rejection drain");

    match completed
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one activation rejection")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Unavailable { attempt, drain },
                },
        } => {
            assert_eq!(attempt, worker);
            let ProxyDrain::ActivationRejected {
                rejection,
                shutdown: Some(shutdown),
                stopped,
            } = drain
            else {
                panic!("the drain lost the application rejection");
            };
            assert_eq!(rejection, HydrationUnavailable);
            assert_eq!(shutdown.id(), ShutdownId(worker.creation().get()));
            assert_eq!(stopped.child, worker.creation());
        }
        _ => panic!("activation rejection produced the wrong owner result"),
    }
}

#[tokio::test]
async fn activation_plan_rejection_remains_distinct_from_start_rejection() {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(Worker(4), RejectHydration(7)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = created_worker(creation, Endpoint(53));
    let committed = proxy
        .on(settled)
        .expect("committed creation is a total proxy transition");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let request = match initialization.resolve(WorkerInitializationOutcome::ReadyForActivation) {
        WorkerInitializationReport::ReadyForActivation {
            activation, permit, ..
        } => BeginActivation::new(activation, permit),
        _ => panic!("successful initialization returned the wrong authority"),
    };

    match request.activate().await.into_rejection() {
        Ok(rejection) => assert_eq!(rejection, HydrationUnavailable),
        Err(_) => panic!("the plan rejection was not retained"),
    }
}

#[test]
fn rejected_initialization_effects_return_the_plan_without_activation_authority() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(3)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = created_worker(creation, Endpoint(33));
    let committed = proxy
        .on(settled)
        .expect("committed creation is a total proxy transition");
    let request = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let expected_worker = request.worker();
    let expected_initialization = request.initialization();
    match request.resolve(WorkerInitializationOutcome::EffectsRejected(
        WorkerInitializationFailure::EffectsRejected,
    )) {
        WorkerInitializationReport::EffectsRejected {
            worker,
            initialization,
            activation,
            failure,
        } => {
            assert_eq!(worker, expected_worker);
            assert_eq!(initialization, expected_initialization);
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(failure, WorkerInitializationFailure::EffectsRejected);
        }
        _ => panic!("rejected initialization effects returned activation authority"),
    }
}

fn initialization_effect_rejection_drains_in(
    order: DrainArrival,
    expected_failure: WorkerInitializationFailure,
) {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(30)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let worker = creation.id();
    let settled = created_worker(creation, Endpoint(71));
    let committed = proxy
        .on(settled)
        .expect("committed creation enters initialization");
    let request = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let draining = proxy
        .on(
            request.resolve(WorkerInitializationOutcome::EffectsRejected(
                expected_failure,
            )),
        )
        .expect("matching initialization rejection begins exact worker drain");
    assert_eq!(proxy.phase(), ProxyPhase::ReturningWorker);
    assert!(draining.sends.worker_activations.is_empty());
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    assert!(draining.sends.owner_outcomes.is_empty());
    let mut shutdowns = draining.sends.worker_shutdowns.into_requests();
    let shutdown = shutdowns
        .pop()
        .expect("one exact worker shutdown is requested");
    assert!(shutdowns.is_empty());
    assert_eq!(shutdown.id, ShutdownId(worker.get()));
    assert_eq!(
        shutdown.actor().recipient(),
        EstablishedRecipient::issued(Endpoint(71))
    );

    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let resolved = EstablishedShutdownResolved::<Worker>::accepted(shutdown.id);
    let completed = match order {
        DrainArrival::StopThenShutdown => {
            let waiting = proxy
                .on(stopped)
                .expect("the matching worker stop joins the unresolved shutdown");
            assert_eq!(proxy.phase(), ProxyPhase::ReturningWorker);
            assert!(waiting.sends.owner_outcomes.is_empty());
            proxy
                .on(resolved)
                .expect("shutdown resolution completes the exact worker drain")
        }
        DrainArrival::ShutdownThenStop => {
            let waiting = proxy
                .on(resolved)
                .expect("shutdown resolution joins the unresolved worker stop");
            assert_eq!(proxy.phase(), ProxyPhase::ReturningWorker);
            assert!(waiting.sends.owner_outcomes.is_empty());
            proxy
                .on(stopped)
                .expect("the matching worker stop completes the exact worker drain")
        }
    };

    assert_eq!(proxy.phase(), ProxyPhase::EmptyAfter);
    match completed
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one terminal initialization result")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Unavailable { attempt, drain },
                },
        } => {
            assert_eq!(attempt.creation(), worker);
            let ProxyDrain::InitializationRejected {
                failure,
                activation,
                shutdown: Some(shutdown),
                stopped,
            } = drain
            else {
                panic!("the drain lost the initialization rejection");
            };
            assert_eq!(failure, expected_failure);
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(shutdown.id(), ShutdownId(worker.get()));
            assert_eq!(stopped.child, worker);
        }
        _ => panic!("initialization rejection produced the wrong owner result"),
    }
}

#[test]
fn initialization_effect_rejection_joins_stop_before_shutdown_resolution() {
    initialization_effect_rejection_drains_in(
        DrainArrival::StopThenShutdown,
        WorkerInitializationFailure::EffectsRejected,
    );
}

#[test]
fn initialization_effect_rejection_joins_shutdown_resolution_before_stop() {
    initialization_effect_rejection_drains_in(
        DrainArrival::ShutdownThenStop,
        WorkerInitializationFailure::EffectsRejected,
    );
}

#[test]
fn corrupt_initialization_interpretation_drains_without_becoming_effect_rejection() {
    assert_eq!(
        retain_initialization_failure(WorkerInitializationFailure::EffectsRejected),
        WorkerInitializationFailure::EffectsRejected
    );
    assert_eq!(
        retain_initialization_failure(WorkerInitializationFailure::InterpreterCorrupt),
        WorkerInitializationFailure::InterpreterCorrupt
    );
    initialization_effect_rejection_drains_in(
        DrainArrival::ShutdownThenStop,
        WorkerInitializationFailure::InterpreterCorrupt,
    );
}

#[test]
fn initialization_stop_never_emits_activation() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(32)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let worker = creation.id();
    let settled = created_worker(creation, Endpoint(73));
    let committed = proxy
        .on(settled)
        .expect("committed creation enters initialization");
    let request = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let completed = proxy
        .on(request.resolve(WorkerInitializationOutcome::Stopped(stopped)))
        .expect("initialization stop is a total proxy transition");

    assert_eq!(proxy.phase(), ProxyPhase::EmptyAfter);
    assert!(completed.sends.worker_activations.is_empty());
    assert!(completed.sends.worker_shutdowns.is_empty());
    match completed
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one stopped-before-ready result")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Unavailable { attempt, .. },
                },
        } => assert_eq!(attempt.creation(), worker),
        _ => panic!("initialization stop produced the wrong owner result"),
    }
}

#[test]
fn worker_stop_before_initialization_rejection_returns_the_plan_without_shutdown() {
    let (mut proxy, initialization) =
        awaiting_initialization(Worker(96), Hydrate(24), Endpoint(196));
    let worker = initialization.worker();
    let worker_stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());
    let waiting_for_initialization = proxy
        .on(worker_stopped)
        .expect("the proxy retains its worker's stop while initialization is pending");
    assert!(waiting_for_initialization.sends.worker_shutdowns.is_empty());

    let worker_unavailable = proxy
        .on(
            initialization.resolve(WorkerInitializationOutcome::EffectsRejected(
                WorkerInitializationFailure::EffectsRejected,
            )),
        )
        .expect("initialization rejection returns the already stopped worker");
    let result = initial_worker_result(
        worker_unavailable
            .sends
            .owner_outcomes
            .into_requests()
            .pop()
            .expect("the proxy owner receives the unavailable worker")
            .into_inner(),
    );
    let WorkerStartResult::Unavailable { attempt, drain } = result else {
        panic!("the stopped worker became available");
    };
    assert_eq!(attempt, worker);
    let ProxyDrain::InitializationRejected {
        failure,
        activation,
        shutdown,
        stopped,
    } = drain
    else {
        panic!("the proxy lost the initialization rejection");
    };
    assert_eq!(failure, WorkerInitializationFailure::EffectsRejected);
    assert_eq!(activation, Hydrate(24));
    assert!(shutdown.is_none());
    assert_eq!(stopped, worker_stopped);
}

#[test]
fn initialization_return_keeps_both_worker_stop_observations() {
    let (mut proxy, initialization) =
        awaiting_initialization(Worker(97), Hydrate(25), Endpoint(197));
    let worker = initialization.worker();
    let first_observation_at = Instant::now();
    let proxy_observation =
        ChildStopped::new(worker.creation(), Ok(Exit::Normal), first_observation_at);
    let host_observation = ChildStopped::new(
        worker.creation(),
        Ok(Exit::Normal),
        first_observation_at + Duration::from_nanos(1),
    );
    let waiting_for_initialization = proxy
        .on(proxy_observation)
        .expect("the proxy retains its direct worker observation");
    assert!(waiting_for_initialization.sends.owner_outcomes.is_empty());

    let worker_unavailable = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::Stopped(host_observation)))
        .expect("the initialization host returns its own worker observation");
    let result = initial_worker_result(
        worker_unavailable
            .sends
            .owner_outcomes
            .into_requests()
            .pop()
            .expect("the proxy owner receives the unavailable worker")
            .into_inner(),
    );
    let WorkerStartResult::Unavailable { attempt, drain } = result else {
        panic!("the stopped worker became available");
    };
    assert_eq!(attempt, worker);
    let ProxyDrain::InitializationStopped {
        activation,
        observed,
        returned,
    } = drain
    else {
        panic!("the proxy lost one worker stop observation");
    };
    assert_eq!(activation, Hydrate(25));
    assert_eq!(observed, Some(proxy_observation));
    assert_eq!(returned, host_observation);
}

#[test]
fn earlier_worker_stop_prevents_activation_after_initialization_succeeds() {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start_with(Worker(33), Hydrate(11)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let worker = creation.id();
    let settled = created_worker(creation, Endpoint(79));
    let committed = proxy
        .on(settled)
        .expect("committed creation enters initialization");
    let request = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let waiting = proxy
        .on(stopped)
        .expect("the matching stop waits for initialization");
    assert!(waiting.sends.owner_outcomes.is_empty());
    let completed = proxy
        .on(request.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("initialization after worker stop is a total transition");

    assert_eq!(proxy.phase(), ProxyPhase::EmptyAfter);
    assert!(completed.sends.worker_activations.is_empty());
    match completed
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one stopped-before-ready result")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Unavailable { attempt, drain },
                },
        } => {
            assert_eq!(attempt.creation(), worker);
            let ProxyDrain::InitializationCompleted {
                permit,
                activation,
                stopped,
            } = drain
            else {
                panic!("the drain lost the unused permit and plan");
            };
            assert_eq!(permit.worker().creation(), worker);
            assert_eq!(activation, Hydrate(11));
            assert_eq!(stopped.child, worker);
        }
        _ => panic!("initialization after stop produced the wrong owner result"),
    }
}

#[test]
fn initialization_result_sent_to_another_proxy_is_returned_unchanged() {
    let (mut target, expected) = awaiting_initialization(Worker(33), Hydrate(12), Endpoint(82));
    let (_source, foreign) = awaiting_initialization(Worker(34), Hydrate(13), Endpoint(83));
    let foreign_worker = foreign.worker();
    let rejected = target
        .on(foreign.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("foreign initialization is a total diagnostic transition");
    assert_eq!(target.phase(), ProxyPhase::Initializing);
    assert!(rejected.sends.worker_activations.is_empty());
    assert!(rejected.sends.owner_outcomes.is_empty());
    match rejected
        .sends
        .diagnostics
        .into_iter()
        .next()
        .expect("one foreign initialization is returned")
        .into_inner()
    {
        ProxyDiagnostic::UnexpectedWorkerInitialization {
            phase,
            initialization,
        } => {
            assert_eq!(phase, ProxyPhase::Initializing);
            match initialization {
                WorkerInitializationReport::ReadyForActivation {
                    worker, activation, ..
                } => {
                    assert_eq!(worker, foreign_worker);
                    assert_eq!(activation, Hydrate(13));
                }
                _ => panic!("foreign initialization changed its result"),
            }
        }
        _ => panic!("foreign initialization produced the wrong diagnostic"),
    }

    let accepted = target
        .on(expected.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("the target retains its exact initialization");
    assert!(accepted.sends.diagnostics.is_empty());
    assert_eq!(accepted.sends.worker_activations.len(), 1);
}

#[test]
fn activation_from_another_proxy_is_returned_unchanged() {
    let (mut target, expected) = awaiting_activation(Worker(41), Hydrate(17), Endpoint(91));
    let (_source, foreign) = awaiting_activation(Worker(42), Hydrate(18), Endpoint(92));
    assert_ne!(expected.worker(), foreign.worker());

    let rejected = target
        .on(foreign.started())
        .expect("foreign activation is a total diagnostic transition");
    assert_eq!(target.phase(), ProxyPhase::Activating);
    assert!(rejected.sends.owner_outcomes.is_empty());
    assert_eq!(rejected.sends.diagnostics.len(), 1);

    let accepted = target
        .on(expected.started())
        .expect("the target retains its exact activation");
    assert!(accepted.sends.diagnostics.is_empty());
}

#[test]
fn host_rejection_keeps_worker_and_initialization_actions_together() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(5)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let settled = worker_creation_result(ChildCreationOutcome::HostRejected {
        creation: RoutedCreation::new(creation, 505),
        initialization: Actions::cont(),
        reason: CreationRejection::EnvironmentFailed,
    });

    let rejected = proxy
        .on(settled)
        .expect("host rejection is a total proxy transition");
    match rejected
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives the complete host rejection")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result:
                        WorkerStartResult::CreationRejected {
                            activation,
                            rejection: WorkerCreationRejection::HostRejected { recovery, reason },
                            stopped: None,
                        },
                },
        } => {
            let (worker, initialization) = recovery.into_retirement();
            assert_eq!(worker, Worker(5));
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(reason, CreationRejection::EnvironmentFailed);
            assert!(initialization.creates.is_empty());
        }
        _ => panic!("unexpected proxy outcome"),
    }
}

#[test]
fn stop_before_creation_rejection_returns_one_complete_contradiction() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(13)))
        .expect("initial worker start is total");
    let worker_id = started
        .creates
        .iter()
        .next()
        .expect("one worker creation is staged")
        .id();
    let creations = started.creates;
    let stopped = ChildStopped::new(worker_id, Ok(Exit::Normal), Instant::now());

    let waiting = proxy
        .on(stopped)
        .expect("a matching stop waits for creation settlement");
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    assert!(waiting.sends.worker_observations.is_empty());
    assert!(waiting.sends.owner_outcomes.is_empty());
    assert!(waiting.sends.diagnostics.is_empty());

    let settled = CreationsSettled::new(CreationSettlement::Rejected {
        creations,
        reason: ChildNamespaceExhausted,
    });
    let contradicted = proxy
        .on(settled)
        .expect("rejection after a stop retains both inputs");
    assert_eq!(proxy.phase(), ProxyPhase::EmptyInitial);
    match contradicted
        .sends
        .owner_outcomes
        .into_iter()
        .next()
        .expect("the owner receives one complete contradiction")
        .into_inner()
    {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result:
                        WorkerStartResult::CreationRejected {
                            rejection: WorkerCreationRejection::NamespaceExhausted { worker },
                            activation,
                            stopped: Some(stopped),
                        },
                },
        } => {
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(stopped.child, worker_id);
            assert_eq!(stopped.outcome, Ok(Exit::Normal));
            assert_eq!(worker, Worker(13));
        }
        _ => panic!("unexpected proxy outcome"),
    }
}

#[test]
fn stop_before_committed_creation_remains_owned_during_initialization() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(17)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let worker_id = creation.id();
    let stopped = ChildStopped::new(worker_id, Ok(Exit::Normal), Instant::now());
    let waiting = proxy
        .on(stopped)
        .expect("a matching stop waits for creation settlement");
    assert!(waiting.sends.owner_outcomes.is_empty());

    let settled = created_worker(creation, Endpoint(37));
    let committed = proxy
        .on(settled)
        .expect("committed creation retains the earlier stop");
    assert_eq!(proxy.phase(), ProxyPhase::Initializing);
    assert!(committed.sends.worker_observations.is_empty());
    assert_eq!(committed.sends.worker_initializations.len(), 1);
    worker_initialization(&committed.sends.worker_initializations[0]);
    assert!(committed.sends.owner_outcomes.is_empty());
    assert!(committed.sends.diagnostics.is_empty());

    let request = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("the child host receives one initialization request");
    let expected_worker = request.worker();
    let expected_initialization = request.initialization();
    match request.resolve(WorkerInitializationOutcome::Stopped(stopped)) {
        WorkerInitializationReport::Stopped {
            worker,
            initialization,
            activation,
            stopped: returned,
        } => {
            assert_eq!(worker, expected_worker);
            assert_eq!(initialization, expected_initialization);
            assert_eq!(activation, ImmediateActivation);
            assert_eq!(returned.child, worker_id);
            assert_eq!(returned.outcome, Ok(Exit::Normal));
        }
        _ => panic!("stopped initialization returned activation authority"),
    }
}

#[test]
fn committed_creation_then_stop_remains_owned_during_initialization() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(18)))
        .expect("initial worker start is total");
    let creation = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged");
    let worker = creation.id();
    let settled = created_worker(creation, Endpoint(41));
    let committed = proxy
        .on(settled)
        .expect("committed creation enters initialization");
    assert_eq!(proxy.phase(), ProxyPhase::Initializing);
    assert!(committed.sends.owner_outcomes.is_empty());

    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let waiting = proxy
        .on(stopped)
        .expect("the matching stop joins the unresolved initialization");
    assert_eq!(proxy.phase(), ProxyPhase::Initializing);
    assert!(waiting.sends.worker_observations.is_empty());
    assert!(waiting.sends.owner_outcomes.is_empty());
    assert!(waiting.sends.diagnostics.is_empty());
}

#[test]
fn foreign_and_duplicate_worker_stops_are_diagnostics_without_state_change() {
    let initialized = StableProxy::immediate()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let started = proxy
        .on(ProxyControl::start(Worker(19)))
        .expect("initial worker start is total");
    assert_eq!(started.sends.worker_observations.len(), 1);
    let worker = started
        .creates
        .into_iter()
        .next()
        .expect("one worker creation is staged")
        .id();
    let mut foreign_workers = CreationSequence::new();
    let _first_worker = foreign_workers
        .issue()
        .expect("the foreign sequence issues its first worker ID");
    let foreign_worker = foreign_workers
        .issue()
        .expect("the foreign sequence issues a distinct worker ID");

    let foreign = ChildStopped::new(foreign_worker, Ok(Exit::Normal), Instant::now());
    let rejected = proxy
        .on(foreign)
        .expect("foreign stop is returned as a diagnostic");
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    match rejected
        .sends
        .diagnostics
        .into_iter()
        .next()
        .expect("one foreign stop is returned")
        .into_inner()
    {
        ProxyDiagnostic::UnexpectedWorkerStop { phase, stopped } => {
            assert_eq!(phase, ProxyPhase::Creating);
            assert_eq!(stopped.child, foreign_worker);
        }
        _ => panic!("unexpected proxy diagnostic"),
    }

    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let waiting = proxy
        .on(stopped)
        .expect("matching stop remains stored while creation is unresolved");
    assert!(waiting.sends.diagnostics.is_empty());
    let duplicate = proxy
        .on(stopped)
        .expect("duplicate stop is returned as a diagnostic");
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    match duplicate
        .sends
        .diagnostics
        .into_iter()
        .next()
        .expect("one duplicate stop is returned")
        .into_inner()
    {
        ProxyDiagnostic::UnexpectedWorkerStop { phase, stopped } => {
            assert_eq!(phase, ProxyPhase::Creating);
            assert_eq!(stopped.child, worker);
        }
        _ => panic!("unexpected proxy diagnostic"),
    }
}
