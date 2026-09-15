//! Independent order model for StableProxy ready-worker shutdown.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, CreateChild,
    CreationId, CreationSequence, CreationSettlement, CreationsSettled, EndpointAddress,
    EstablishedCreation, EstablishedRecipient, ItemSettlement, Never, NoBirths, Protocol,
    SettledItem, Step, User,
};
use behavior_actors::atomic::{
    ActivationPlan, ActivationStartRejection, BeginActivation, ProxyControl, ProxyPhase,
    StableProxy, WorkerActivation, WorkerAttempt, WorkerInitializationOutcome,
};
use behavior_actors::{
    Activate as _, Active, ChildStopped, EstablishedShutdownResolved, Exit, ShutdownId,
    StopOnShutdown,
};
use proptest::strategy::Just;
use proptest::test_runner::Config;
use proptest::{collection, prop_oneof, proptest};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint;

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Prepare;

impl ActivationPlan for Prepare {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        async { Ok(()) }
    }
}

fn created_worker(
    creation: CreateChild<RuntimeAddr, StopOnShutdown<Worker>>,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<Worker>> {
    let (worker, _, kind) = creation.into_parts();
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(Endpoint),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

fn foreign_worker_id(expected: CreationId) -> CreationId {
    let mut workers = CreationSequence::new();
    let _first_worker = workers
        .issue()
        .expect("the foreign sequence issues its first worker ID");
    let foreign = workers
        .issue()
        .expect("the foreign sequence issues a distinct worker ID");
    assert_ne!(foreign, expected);
    foreign
}

fn worker_stopped(worker: CreationId) -> ChildStopped<RuntimeAddr> {
    ChildStopped::new(worker, Ok(Exit::Normal), Instant::now())
}

struct HeldPlan(Arc<AtomicUsize>);

impl Drop for HeldPlan {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl ActivationPlan for HeldPlan {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        async { Ok(()) }
    }
}

#[derive(Clone, Copy, Debug)]
enum Arrival {
    WorkerStopped,
    ShutdownReturned,
    ForeignWorkerStop,
    ForeignShutdownReturn,
}

#[derive(Clone, Copy, Debug)]
enum ArrivalOrder {
    StopFirst,
    ShutdownFirst,
}

enum Custody {
    BothExpected,
    StopRetained,
    ShutdownRetained,
    Complete,
}

enum Prediction {
    Wait,
    Reject,
    Stop,
}

#[derive(Clone, Copy)]
enum WorkerShutdownArrival {
    WorkReturned,
    WorkerStopped,
    ShutdownReturned,
    ForeignWorkerStop,
    ForeignShutdownReturn,
}

enum WorkerShutdownCustody {
    AllExpected,
    WorkRetained,
    StopRetained,
    ShutdownRetained,
    WorkAndStopRetained,
    WorkAndShutdownRetained,
    DepartureRetained,
    Complete,
}

enum ReturnedActivation {
    Available(WorkerActivation<Worker, Prepare>),
    Consumed,
}

const WORKER_SHUTDOWN_ARRIVAL_ORDERS: [[WorkerShutdownArrival; 3]; 6] = [
    [
        WorkerShutdownArrival::WorkReturned,
        WorkerShutdownArrival::WorkerStopped,
        WorkerShutdownArrival::ShutdownReturned,
    ],
    [
        WorkerShutdownArrival::WorkReturned,
        WorkerShutdownArrival::ShutdownReturned,
        WorkerShutdownArrival::WorkerStopped,
    ],
    [
        WorkerShutdownArrival::WorkerStopped,
        WorkerShutdownArrival::WorkReturned,
        WorkerShutdownArrival::ShutdownReturned,
    ],
    [
        WorkerShutdownArrival::WorkerStopped,
        WorkerShutdownArrival::ShutdownReturned,
        WorkerShutdownArrival::WorkReturned,
    ],
    [
        WorkerShutdownArrival::ShutdownReturned,
        WorkerShutdownArrival::WorkReturned,
        WorkerShutdownArrival::WorkerStopped,
    ],
    [
        WorkerShutdownArrival::ShutdownReturned,
        WorkerShutdownArrival::WorkerStopped,
        WorkerShutdownArrival::WorkReturned,
    ],
];

impl Custody {
    fn accept(self, arrival: Arrival) -> (Self, Prediction) {
        match (self, arrival) {
            (Self::BothExpected, Arrival::WorkerStopped) => (Self::StopRetained, Prediction::Wait),
            (Self::BothExpected, Arrival::ShutdownReturned) => {
                (Self::ShutdownRetained, Prediction::Wait)
            }
            (Self::StopRetained, Arrival::ShutdownReturned)
            | (Self::ShutdownRetained, Arrival::WorkerStopped) => {
                (Self::Complete, Prediction::Stop)
            }
            (custody, _) => (custody, Prediction::Reject),
        }
    }
}

impl WorkerShutdownCustody {
    fn accept(self, arrival: WorkerShutdownArrival) -> (Self, Prediction) {
        match (self, arrival) {
            (Self::AllExpected, WorkerShutdownArrival::WorkReturned) => {
                (Self::WorkRetained, Prediction::Wait)
            }
            (Self::AllExpected, WorkerShutdownArrival::WorkerStopped) => {
                (Self::StopRetained, Prediction::Wait)
            }
            (Self::AllExpected, WorkerShutdownArrival::ShutdownReturned) => {
                (Self::ShutdownRetained, Prediction::Wait)
            }
            (Self::WorkRetained, WorkerShutdownArrival::WorkerStopped)
            | (Self::StopRetained, WorkerShutdownArrival::WorkReturned) => {
                (Self::WorkAndStopRetained, Prediction::Wait)
            }
            (Self::WorkRetained, WorkerShutdownArrival::ShutdownReturned)
            | (Self::ShutdownRetained, WorkerShutdownArrival::WorkReturned) => {
                (Self::WorkAndShutdownRetained, Prediction::Wait)
            }
            (Self::StopRetained, WorkerShutdownArrival::ShutdownReturned)
            | (Self::ShutdownRetained, WorkerShutdownArrival::WorkerStopped) => {
                (Self::DepartureRetained, Prediction::Wait)
            }
            (Self::WorkAndStopRetained, WorkerShutdownArrival::ShutdownReturned)
            | (Self::WorkAndShutdownRetained, WorkerShutdownArrival::WorkerStopped)
            | (Self::DepartureRetained, WorkerShutdownArrival::WorkReturned) => {
                (Self::Complete, Prediction::Stop)
            }
            (custody, _) => (custody, Prediction::Reject),
        }
    }
}

impl ReturnedActivation {
    fn take(&mut self) -> WorkerActivation<Worker, Prepare> {
        match core::mem::replace(self, Self::Consumed) {
            Self::Available(activation) => activation,
            Self::Consumed => panic!("the model supplied the affine activation return twice"),
        }
    }
}

fn awaiting_activation_proxy<P>(
    plan: P,
) -> (Active<StableProxy<Worker, P>>, BeginActivation<Worker, P>)
where
    P: ActivationPlan,
{
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(ProxyControl::start_with(Worker, plan))
        .expect("worker start stages one worker");
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("one fresh worker is staged");
    let committed = proxy
        .on(created_worker(creation))
        .expect("worker creation commits");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("worker initialization is requested");
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("worker initialization authorizes activation");
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("worker activation is requested");
    (proxy, activation)
}

async fn ready_proxy() -> (Active<StableProxy<Worker, Prepare>>, WorkerAttempt) {
    let (mut proxy, activation) = awaiting_activation_proxy(Prepare);
    let worker = activation.worker();
    let admitted = proxy
        .on(activation.started())
        .expect("activation starts before it returns");
    assert!(admitted.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .expect("activation readiness opens service");
    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    assert_eq!(ready.sends.owner_outcomes.len(), 1);
    (proxy, worker)
}

fn compare_activation_custody(
    mut proxy: Active<StableProxy<Worker, Prepare>>,
    activation: WorkerActivation<Worker, Prepare>,
    shutdown: ShutdownId,
    order: &[WorkerShutdownArrival],
) {
    let mut custody = WorkerShutdownCustody::AllExpected;
    let worker = activation.worker().creation();
    let foreign_worker = foreign_worker_id(worker);
    let mut activation = ReturnedActivation::Available(activation);

    for arrival in order {
        let (next, prediction) = custody.accept(*arrival);
        custody = next;
        let actions = match arrival {
            WorkerShutdownArrival::WorkReturned => proxy
                .on(activation.take())
                .expect("the exact activation return is total"),
            WorkerShutdownArrival::WorkerStopped => proxy
                .on(worker_stopped(worker))
                .expect("the exact activation worker stop is total"),
            WorkerShutdownArrival::ShutdownReturned => proxy
                .on(EstablishedShutdownResolved::accepted(shutdown))
                .expect("the exact activation worker shutdown result is total"),
            WorkerShutdownArrival::ForeignWorkerStop => proxy
                .on(worker_stopped(foreign_worker))
                .expect("a foreign activation worker stop is returned"),
            WorkerShutdownArrival::ForeignShutdownReturn => proxy
                .on(EstablishedShutdownResolved::accepted(ShutdownId(999)))
                .expect("a foreign activation shutdown result is returned"),
        };
        match prediction {
            Prediction::Wait => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.sends.diagnostics.is_empty());
            }
            Prediction::Reject => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert_eq!(actions.sends.diagnostics.len(), 1);
            }
            Prediction::Stop => {
                assert_eq!(proxy.phase(), ProxyPhase::Stopped);
                assert!(matches!(actions.become_, Step::Stop(_)));
                assert!(actions.sends.diagnostics.is_empty());
            }
        }
        assert!(actions.sends.owner_outcomes.is_empty());
        assert!(actions.sends.worker_shutdowns.is_empty());
        assert!(actions.creates.is_empty());
    }
    assert!(matches!(custody, WorkerShutdownCustody::Complete));
}

fn waiting_activation_follows(order: &[WorkerShutdownArrival]) {
    let (mut proxy, activation) = awaiting_activation_proxy(Prepare);
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown closes activation admission");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("waiting activation shutdown emits one worker shutdown");
    let returned = activation.start_rejected(ActivationStartRejection::OwnerStopped);
    compare_activation_custody(proxy, returned, shutdown.id, order);
}

async fn running_activation_follows(order: &[WorkerShutdownArrival]) {
    let (mut proxy, activation) = awaiting_activation_proxy(Prepare);
    let admitted = proxy
        .on(activation.started())
        .expect("activation admission is total");
    assert!(admitted.sends.owner_outcomes.is_empty());
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown closes readiness publication");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("running activation shutdown emits one worker shutdown");
    let returned = activation.activate().await;
    compare_activation_custody(proxy, returned, shutdown.id, order);
}

async fn completed_replacement_follows(order: &[WorkerShutdownArrival]) {
    let (mut proxy, predecessor_worker) = ready_proxy().await;
    let returning = proxy
        .on(ProxyControl::replace_with(Worker, Prepare))
        .expect("replacement begins predecessor return");
    let predecessor = returning
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the predecessor receives one shutdown");
    let creating = proxy
        .on(worker_stopped(predecessor_worker.creation()))
        .expect("the predecessor stop stages the successor");
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("the successor is created after predecessor stop");
    let successor_worker = creation.id();
    let foreign_worker = predecessor_worker.creation();
    assert_ne!(foreign_worker, successor_worker);
    let committed = proxy
        .on(created_worker(creation))
        .expect("successor creation commits");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("successor initialization is requested");
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("successor initialization authorizes activation");
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("successor activation is requested");
    let admitted = proxy
        .on(activation.started())
        .expect("successor activation starts");
    assert!(admitted.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .expect("successor readiness waits for predecessor return");
    assert!(ready.sends.owner_outcomes.is_empty());
    assert_eq!(proxy.phase(), ProxyPhase::Replacing);
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("owner shutdown begins successor departure");
    let successor = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the successor receives one shutdown");
    let mut custody = WorkerShutdownCustody::AllExpected;

    for arrival in order {
        let (next, prediction) = custody.accept(*arrival);
        custody = next;
        let actions = match arrival {
            WorkerShutdownArrival::WorkReturned => proxy
                .on(EstablishedShutdownResolved::accepted(predecessor.id))
                .expect("the exact predecessor return is total"),
            WorkerShutdownArrival::WorkerStopped => proxy
                .on(worker_stopped(successor_worker))
                .expect("the exact successor stop is total"),
            WorkerShutdownArrival::ShutdownReturned => proxy
                .on(EstablishedShutdownResolved::accepted(successor.id))
                .expect("the exact successor shutdown return is total"),
            WorkerShutdownArrival::ForeignWorkerStop => proxy
                .on(worker_stopped(foreign_worker))
                .expect("a foreign replacement stop is returned"),
            WorkerShutdownArrival::ForeignShutdownReturn => proxy
                .on(EstablishedShutdownResolved::accepted(ShutdownId(999)))
                .expect("a foreign replacement shutdown result is returned"),
        };
        match prediction {
            Prediction::Wait => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.sends.diagnostics.is_empty());
            }
            Prediction::Reject => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert_eq!(actions.sends.diagnostics.len(), 1);
            }
            Prediction::Stop => {
                assert_eq!(proxy.phase(), ProxyPhase::Stopped);
                assert!(matches!(actions.become_, Step::Stop(_)));
                assert!(actions.sends.diagnostics.is_empty());
            }
        }
        assert!(actions.sends.owner_outcomes.is_empty());
        assert!(actions.sends.worker_shutdowns.is_empty());
        assert!(actions.creates.is_empty());
    }
    assert!(matches!(custody, WorkerShutdownCustody::Complete));
}

fn initialization_follows(order: &[WorkerShutdownArrival]) {
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(ProxyControl::start_with(Worker, Prepare))
        .expect("worker start stages one worker");
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("one fresh worker is staged");
    let worker = creation.id();
    let foreign_worker = foreign_worker_id(worker);
    let waiting = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown waits for the emitted creation result");
    assert!(waiting.sends.worker_shutdowns.is_empty());
    let committed = proxy
        .on(created_worker(creation))
        .expect("committed creation starts initialization and worker shutdown");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("initialization remains owned after shutdown");
    let shutdown = committed
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the committed worker receives one shutdown");
    let mut initialization =
        Some(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation));
    let mut custody = WorkerShutdownCustody::AllExpected;

    for arrival in order {
        let (next, prediction) = custody.accept(*arrival);
        custody = next;
        let actions = match arrival {
            WorkerShutdownArrival::WorkReturned => proxy
                .on(initialization.take().expect("initialization returns once"))
                .expect("the exact initialization return is total"),
            WorkerShutdownArrival::WorkerStopped => proxy
                .on(worker_stopped(worker))
                .expect("the exact initializing worker stop is total"),
            WorkerShutdownArrival::ShutdownReturned => proxy
                .on(EstablishedShutdownResolved::accepted(shutdown.id))
                .expect("the exact initializing worker shutdown result is total"),
            WorkerShutdownArrival::ForeignWorkerStop => proxy
                .on(worker_stopped(foreign_worker))
                .expect("a foreign initializing worker stop is returned"),
            WorkerShutdownArrival::ForeignShutdownReturn => proxy
                .on(EstablishedShutdownResolved::accepted(ShutdownId(999)))
                .expect("a foreign initializing shutdown result is returned"),
        };
        match prediction {
            Prediction::Wait => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.sends.diagnostics.is_empty());
            }
            Prediction::Reject => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert_eq!(actions.sends.diagnostics.len(), 1);
            }
            Prediction::Stop => {
                assert_eq!(proxy.phase(), ProxyPhase::Stopped);
                assert!(matches!(actions.become_, Step::Stop(_)));
                assert!(actions.sends.diagnostics.is_empty());
            }
        }
        assert!(actions.sends.owner_outcomes.is_empty());
        assert!(actions.sends.worker_activations.is_empty());
        assert!(actions.sends.worker_shutdowns.is_empty());
        assert!(actions.creates.is_empty());
    }
    assert!(matches!(custody, WorkerShutdownCustody::Complete));
}

async fn compare(order: &[Arrival]) {
    let (mut proxy, worker) = ready_proxy().await;
    let worker = worker.creation();
    let foreign_worker = foreign_worker_id(worker);
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("ready shutdown begins exact worker return");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("one exact shutdown is emitted");
    let mut custody = Custody::BothExpected;

    for arrival in order {
        let (next, prediction) = custody.accept(*arrival);
        custody = next;
        let actions = match arrival {
            Arrival::WorkerStopped => proxy
                .on(worker_stopped(worker))
                .expect("the exact worker stop is total"),
            Arrival::ShutdownReturned => proxy
                .on(EstablishedShutdownResolved::accepted(shutdown.id))
                .expect("the exact shutdown result is total"),
            Arrival::ForeignWorkerStop => proxy
                .on(worker_stopped(foreign_worker))
                .expect("a foreign worker stop is returned as a diagnostic"),
            Arrival::ForeignShutdownReturn => proxy
                .on(EstablishedShutdownResolved::accepted(
                    behavior_actors::ShutdownId(999),
                ))
                .expect("a foreign shutdown result is returned as a diagnostic"),
        };
        match prediction {
            Prediction::Wait => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.sends.diagnostics.is_empty());
            }
            Prediction::Reject => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert_eq!(actions.sends.diagnostics.len(), 1);
            }
            Prediction::Stop => {
                assert_eq!(proxy.phase(), ProxyPhase::Stopped);
                assert!(matches!(actions.become_, Step::Stop(_)));
                assert!(actions.sends.diagnostics.is_empty());
            }
        }
        assert!(actions.sends.owner_outcomes.is_empty());
        assert!(actions.sends.worker_shutdowns.is_empty());
        assert!(actions.creates.is_empty());
    }
    assert!(matches!(custody, Custody::Complete));
}

#[tokio::test]
async fn independent_custody_model_matches_both_ready_worker_orders() {
    compare(&[Arrival::WorkerStopped, Arrival::ShutdownReturned]).await;
    compare(&[Arrival::ShutdownReturned, Arrival::WorkerStopped]).await;
    compare(&[
        Arrival::WorkerStopped,
        Arrival::WorkerStopped,
        Arrival::ShutdownReturned,
    ])
    .await;
    compare(&[
        Arrival::ShutdownReturned,
        Arrival::ShutdownReturned,
        Arrival::WorkerStopped,
    ])
    .await;
    compare(&[
        Arrival::ForeignWorkerStop,
        Arrival::WorkerStopped,
        Arrival::ShutdownReturned,
    ])
    .await;
    compare(&[
        Arrival::ForeignShutdownReturn,
        Arrival::ShutdownReturned,
        Arrival::WorkerStopped,
    ])
    .await;
}

#[tokio::test]
async fn independent_activation_model_matches_every_return_stop_and_shutdown_order() {
    for order in WORKER_SHUTDOWN_ARRIVAL_ORDERS {
        waiting_activation_follows(&order);
        running_activation_follows(&order).await;
    }

    let mut waiting_with_foreign_inputs = vec![
        WorkerShutdownArrival::ForeignWorkerStop,
        WorkerShutdownArrival::ForeignShutdownReturn,
    ];
    waiting_with_foreign_inputs.extend(WORKER_SHUTDOWN_ARRIVAL_ORDERS[0]);
    waiting_activation_follows(&waiting_with_foreign_inputs);

    let mut running_with_foreign_inputs = vec![
        WorkerShutdownArrival::ForeignShutdownReturn,
        WorkerShutdownArrival::ForeignWorkerStop,
    ];
    running_with_foreign_inputs.extend(WORKER_SHUTDOWN_ARRIVAL_ORDERS[5]);
    running_activation_follows(&running_with_foreign_inputs).await;
}

#[test]
fn independent_initialization_model_matches_every_return_stop_and_shutdown_order() {
    for order in WORKER_SHUTDOWN_ARRIVAL_ORDERS {
        initialization_follows(&order);
    }

    let mut with_foreign_inputs = vec![
        WorkerShutdownArrival::ForeignWorkerStop,
        WorkerShutdownArrival::ForeignShutdownReturn,
    ];
    with_foreign_inputs.extend(WORKER_SHUTDOWN_ARRIVAL_ORDERS[3]);
    initialization_follows(&with_foreign_inputs);
}

#[tokio::test]
async fn independent_completed_replacement_model_matches_every_return_order() {
    for order in WORKER_SHUTDOWN_ARRIVAL_ORDERS {
        completed_replacement_follows(&order).await;
    }

    let mut with_foreign_inputs = vec![
        WorkerShutdownArrival::ForeignShutdownReturn,
        WorkerShutdownArrival::ForeignWorkerStop,
    ];
    with_foreign_inputs.extend(WORKER_SHUTDOWN_ARRIVAL_ORDERS[4]);
    completed_replacement_follows(&with_foreign_inputs).await;
}

proptest! {
    #![proptest_config(Config::with_cases(96))]

    #[test]
    fn generated_noise_and_duplicates_preserve_ready_worker_custody(
        foreign in collection::vec(
            prop_oneof![
                Just(Arrival::ForeignWorkerStop),
                Just(Arrival::ForeignShutdownReturn),
            ],
            0..24,
        ),
        repeats in 0_usize..16,
        order in prop_oneof![
            Just(ArrivalOrder::StopFirst),
            Just(ArrivalOrder::ShutdownFirst),
        ],
    ) {
        let mut sequence = foreign;
        let (opening_arrival, closing_arrival) = match order {
            ArrivalOrder::StopFirst => (Arrival::WorkerStopped, Arrival::ShutdownReturned),
            ArrivalOrder::ShutdownFirst => (Arrival::ShutdownReturned, Arrival::WorkerStopped),
        };
        sequence.push(opening_arrival);
        sequence.extend((0..repeats).map(|_| opening_arrival));
        sequence.push(closing_arrival);

        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("the property runtime is test setup");
        runtime.block_on(compare(&sequence));
    }
}

#[test]
fn stopped_proxy_retains_initialized_plan_until_behavior_retirement() {
    let drops = Arc::new(AtomicUsize::new(0));
    let initialized = StableProxy::activated()
        .initialize()
        .expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(ProxyControl::start_with(
            Worker,
            HeldPlan(Arc::clone(&drops)),
        ))
        .expect("worker start stages one worker");
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("one fresh worker is staged");
    let worker = creation.id();
    let committed = proxy
        .on(created_worker(creation))
        .expect("worker creation commits");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("worker initialization is requested");
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown begins exact worker departure");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the worker receives one shutdown");
    let returned = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("the initialized plan returns after admission closes");
    assert!(returned.sends.worker_activations.is_empty());
    let waiting = proxy
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .expect("worker stop waits for shutdown return");
    assert!(matches!(waiting.become_, Step::Continue));
    let terminal = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .expect("the shutdown return closes proxy admission");
    assert!(matches!(terminal.become_, Step::Stop(_)));
    drop(terminal);
    assert_eq!(drops.load(Ordering::SeqCst), 0);

    drop(proxy);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn stopped_proxy_retains_unaccepted_activation_request_until_behavior_retirement() {
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut proxy, activation) = awaiting_activation_proxy(HeldPlan(Arc::clone(&drops)));
    let worker = activation.worker().creation();
    let draining = proxy
        .on(ProxyControl::shutdown())
        .expect("shutdown closes activation admission");
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("the worker receives one shutdown");
    let returned = proxy
        .on(activation.start_rejected(ActivationStartRejection::OwnerStopped))
        .expect("the unaccepted activation request returns whole");
    assert!(returned.sends.owner_outcomes.is_empty());
    let waiting = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .expect("shutdown return waits for exact worker stop");
    assert!(matches!(waiting.become_, Step::Continue));
    let terminal = proxy
        .on(worker_stopped(worker))
        .expect("worker stop closes proxy admission");
    assert!(matches!(terminal.become_, Step::Stop(_)));
    drop(terminal);
    assert_eq!(drops.load(Ordering::SeqCst), 0);

    drop(proxy);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}
