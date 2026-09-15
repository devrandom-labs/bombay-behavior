//! StableProxy creation uses creator-local IDs, never runtime route values.

use std::collections::BTreeMap;
use std::time::Instant;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, ChildInput,
    ChildNamespaceExhausted, CreationId, CreationSequence, CreationSettlement, CreationsSettled,
    EndpointAddress, EstablishedCreation, EstablishedRecipient, ItemSettlement, Never, NoBirths,
    Protocol, SettledItem, Step, User,
};
use behavior_actors::atomic::{
    BeginActivation, ImmediateActivation, InitialWorkerOutcome, InitializeWorker, ProxyControl,
    ProxyOutcome, ProxyPhase, ReplacementOutcome, StableProxy, WorkerActivation,
    WorkerCreationRejection, WorkerInitializationOutcome, WorkerInitializationReport,
    WorkerStartResult,
};
use behavior_actors::{Activate, Active, ChildStopped, EstablishedShutdownResolved, Exit};

#[derive(Clone, Copy, Eq, PartialEq)]
struct RuntimeAddress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeRoute(u16);

impl Address for RuntimeAddress {
    type Nonce = RuntimeRoute;
}

#[derive(Clone, Copy)]
struct WorkerEndpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = WorkerEndpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
struct Worker(u8);

impl Protocol for Worker {
    type Addr = RuntimeAddress;
    type Msg = ();
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn first_creation() -> CreationId {
    CreationSequence::new()
        .issue()
        .unwrap_or_else(|| panic!("a new sequence issues its first creation ID"))
}

async fn ready_proxy() -> (Active<StableProxy<Worker, ImmediateActivation>>, CreationId) {
    let (mut proxy, id, initialization) = initializing_proxy();
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("worker initialization authorizes activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker activation is requested"));
    let started = proxy
        .on(activation.started())
        .unwrap_or_else(|_| panic!("worker activation starts"));
    assert!(matches!(started.become_, Step::Continue));
    assert!(started.sends.owner_outcomes.is_empty());

    let ready = proxy
        .on(activation.activate().await)
        .unwrap_or_else(|_| panic!("worker activation makes the proxy ready"));
    assert!(matches!(ready.become_, Step::Continue));
    assert_eq!(ready.sends.owner_outcomes.len(), 1);

    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    (proxy, id)
}

fn initializing_proxy() -> (
    Active<StableProxy<Worker, ImmediateActivation>>,
    CreationId,
    InitializeWorker<Worker, ImmediateActivation>,
) {
    let initialized = StableProxy::immediate()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(ProxyControl::start(Worker(1)))
        .unwrap_or_else(|_| panic!("worker admission emits one creation"));
    let creation = creating
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker creation is emitted"));
    let (worker, _behavior, kind) = creation.into_parts();
    let initialized = proxy
        .on(CreationsSettled::new(CreationSettlement::Settled(
            [SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        worker,
                        kind,
                        EstablishedRecipient::issued(WorkerEndpoint),
                    ),
                },
            ))]
            .into_iter()
            .collect(),
        )))
        .unwrap_or_else(|_| panic!("worker creation commits"));
    let initialization = initialized
        .sends
        .worker_initializations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker initialization is requested"));

    (proxy, worker, initialization)
}

fn activating_proxy() -> (
    Active<StableProxy<Worker, ImmediateActivation>>,
    CreationId,
    BeginActivation<Worker, ImmediateActivation>,
) {
    let (mut proxy, worker, initialization) = initializing_proxy();
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("worker initialization authorizes activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker activation is requested"));

    (proxy, worker, activation)
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum InitializationShutdownKind {
    Initialization,
    Shutdown,
    Stop,
}

use InitializationShutdownKind::{Initialization, Shutdown, Stop};

enum InitializationShutdownEvent {
    Initialized(WorkerInitializationReport<Worker, ImmediateActivation>),
    ShutdownResolved(EstablishedShutdownResolved<Worker>),
    WorkerStopped(ChildStopped<RuntimeAddress>),
}

fn admit_initialization_shutdown(
    proxy: &mut Active<StableProxy<Worker, ImmediateActivation>>,
    input: InitializationShutdownEvent,
) -> BehaviorActed<StableProxy<Worker, ImmediateActivation>> {
    match input {
        InitializationShutdownEvent::Initialized(initialized) => proxy.on(initialized),
        InitializationShutdownEvent::ShutdownResolved(resolved) => proxy.on(resolved),
        InitializationShutdownEvent::WorkerStopped(stopped) => proxy.on(stopped),
    }
}

#[test]
fn initialization_shutdown_accepts_every_arrival_order() {
    let orders = [
        [Initialization, Stop, Shutdown],
        [Initialization, Shutdown, Stop],
        [Stop, Initialization, Shutdown],
        [Stop, Shutdown, Initialization],
        [Shutdown, Initialization, Stop],
        [Shutdown, Stop, Initialization],
    ];

    for order in orders {
        let (mut proxy, worker, initialization) = initializing_proxy();
        let shutting_down = proxy
            .on(ProxyControl::shutdown())
            .unwrap_or_else(|_| panic!("proxy shutdown is accepted"));
        let shutdown = shutting_down
            .sends
            .worker_shutdowns
            .into_requests()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("one exact worker shutdown is requested"));
        let mut inputs = BTreeMap::from([
            (
                Initialization,
                InitializationShutdownEvent::Initialized(
                    initialization.resolve(WorkerInitializationOutcome::ReadyForActivation),
                ),
            ),
            (
                Shutdown,
                InitializationShutdownEvent::ShutdownResolved(
                    EstablishedShutdownResolved::accepted(shutdown.id),
                ),
            ),
            (
                Stop,
                InitializationShutdownEvent::WorkerStopped(ChildStopped::new(
                    worker,
                    Ok(Exit::Normal),
                    Instant::now(),
                )),
            ),
        ]);
        let [first, second, terminal] = order;
        for kind in [first, second] {
            let input = inputs
                .remove(&kind)
                .unwrap_or_else(|| panic!("each order names every input once"));
            let waiting = admit_initialization_shutdown(&mut proxy, input)
                .unwrap_or_else(|_| panic!("each exact input is admitted"));
            assert!(matches!(waiting.become_, Step::Continue));
        }
        let terminal = inputs
            .remove(&terminal)
            .unwrap_or_else(|| panic!("each order ends with one exact input"));
        let terminal = admit_initialization_shutdown(&mut proxy, terminal)
            .unwrap_or_else(|_| panic!("the final exact input closes the proxy"));

        assert_eq!(proxy.phase(), ProxyPhase::Stopped);
        assert!(matches!(terminal.become_, Step::Stop(_)));
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum ActivationShutdownKind {
    ActivationStarted,
    ActivationReady,
    ShutdownResolved,
    WorkerStopped,
}

use ActivationShutdownKind::{ActivationReady, ActivationStarted, ShutdownResolved, WorkerStopped};

enum ActivationShutdownEvent {
    Activation(WorkerActivation<Worker, ImmediateActivation>),
    ShutdownResolved(EstablishedShutdownResolved<Worker>),
    WorkerStopped(ChildStopped<RuntimeAddress>),
}

fn admit_activation_shutdown(
    proxy: &mut Active<StableProxy<Worker, ImmediateActivation>>,
    input: ActivationShutdownEvent,
) -> BehaviorActed<StableProxy<Worker, ImmediateActivation>> {
    match input {
        ActivationShutdownEvent::Activation(activation) => proxy.on(activation),
        ActivationShutdownEvent::ShutdownResolved(resolved) => proxy.on(resolved),
        ActivationShutdownEvent::WorkerStopped(stopped) => proxy.on(stopped),
    }
}

#[tokio::test]
async fn activation_shutdown_waiting_for_start_accepts_every_lawful_arrival_order() {
    let orders = [
        [
            ActivationStarted,
            ActivationReady,
            ShutdownResolved,
            WorkerStopped,
        ],
        [
            ActivationStarted,
            ActivationReady,
            WorkerStopped,
            ShutdownResolved,
        ],
        [
            ActivationStarted,
            ShutdownResolved,
            ActivationReady,
            WorkerStopped,
        ],
        [
            ActivationStarted,
            ShutdownResolved,
            WorkerStopped,
            ActivationReady,
        ],
        [
            ActivationStarted,
            WorkerStopped,
            ActivationReady,
            ShutdownResolved,
        ],
        [
            ActivationStarted,
            WorkerStopped,
            ShutdownResolved,
            ActivationReady,
        ],
        [
            ShutdownResolved,
            ActivationStarted,
            ActivationReady,
            WorkerStopped,
        ],
        [
            ShutdownResolved,
            ActivationStarted,
            WorkerStopped,
            ActivationReady,
        ],
        [
            ShutdownResolved,
            WorkerStopped,
            ActivationStarted,
            ActivationReady,
        ],
        [
            WorkerStopped,
            ActivationStarted,
            ActivationReady,
            ShutdownResolved,
        ],
        [
            WorkerStopped,
            ActivationStarted,
            ShutdownResolved,
            ActivationReady,
        ],
        [
            WorkerStopped,
            ShutdownResolved,
            ActivationStarted,
            ActivationReady,
        ],
    ];

    for order in orders {
        let (mut proxy, worker, activation) = activating_proxy();
        let shutdown = begin_shutdown(&mut proxy);
        let started = activation.started();
        let ready = activation.activate().await;
        let mut inputs = BTreeMap::from([
            (
                ActivationStarted,
                ActivationShutdownEvent::Activation(started),
            ),
            (ActivationReady, ActivationShutdownEvent::Activation(ready)),
            (
                ShutdownResolved,
                ActivationShutdownEvent::ShutdownResolved(EstablishedShutdownResolved::accepted(
                    shutdown,
                )),
            ),
            (
                WorkerStopped,
                ActivationShutdownEvent::WorkerStopped(ChildStopped::new(
                    worker,
                    Ok(Exit::Normal),
                    Instant::now(),
                )),
            ),
        ]);
        let [first, second, third, terminal] = order;
        for kind in [first, second, third] {
            let input = inputs
                .remove(&kind)
                .unwrap_or_else(|| panic!("each order names every input once"));
            let waiting = admit_activation_shutdown(&mut proxy, input)
                .unwrap_or_else(|_| panic!("each exact input is admitted"));
            assert!(matches!(waiting.become_, Step::Continue));
        }
        let terminal = inputs
            .remove(&terminal)
            .unwrap_or_else(|| panic!("each order ends with one exact input"));
        let terminal = admit_activation_shutdown(&mut proxy, terminal)
            .unwrap_or_else(|_| panic!("the final exact input closes the proxy"));

        assert_eq!(proxy.phase(), ProxyPhase::Stopped);
        assert!(matches!(terminal.become_, Step::Stop(_)));
    }
}

#[tokio::test]
async fn activation_shutdown_running_accepts_every_arrival_order() {
    let orders = [
        [ActivationReady, ShutdownResolved, WorkerStopped],
        [ActivationReady, WorkerStopped, ShutdownResolved],
        [ShutdownResolved, ActivationReady, WorkerStopped],
        [ShutdownResolved, WorkerStopped, ActivationReady],
        [WorkerStopped, ActivationReady, ShutdownResolved],
        [WorkerStopped, ShutdownResolved, ActivationReady],
    ];

    for order in orders {
        let (mut proxy, worker, activation) = activating_proxy();
        let started = proxy
            .on(activation.started())
            .unwrap_or_else(|_| panic!("worker activation starts"));
        assert!(matches!(started.become_, Step::Continue));
        let shutdown = begin_shutdown(&mut proxy);
        let ready = activation.activate().await;
        let mut inputs = BTreeMap::from([
            (ActivationReady, ActivationShutdownEvent::Activation(ready)),
            (
                ShutdownResolved,
                ActivationShutdownEvent::ShutdownResolved(EstablishedShutdownResolved::accepted(
                    shutdown,
                )),
            ),
            (
                WorkerStopped,
                ActivationShutdownEvent::WorkerStopped(ChildStopped::new(
                    worker,
                    Ok(Exit::Normal),
                    Instant::now(),
                )),
            ),
        ]);
        let [first, second, terminal] = order;
        for kind in [first, second] {
            let input = inputs
                .remove(&kind)
                .unwrap_or_else(|| panic!("each order names every input once"));
            let waiting = admit_activation_shutdown(&mut proxy, input)
                .unwrap_or_else(|_| panic!("each exact input is admitted"));
            assert!(matches!(waiting.become_, Step::Continue));
        }
        let terminal = inputs
            .remove(&terminal)
            .unwrap_or_else(|| panic!("each order ends with one exact input"));
        let terminal = admit_activation_shutdown(&mut proxy, terminal)
            .unwrap_or_else(|_| panic!("the final exact input closes the proxy"));

        assert_eq!(proxy.phase(), ProxyPhase::Stopped);
        assert!(matches!(terminal.become_, Step::Stop(_)));
    }
}

fn begin_shutdown(
    proxy: &mut Active<StableProxy<Worker, ImmediateActivation>>,
) -> behavior_actors::ShutdownId {
    proxy
        .on(ProxyControl::shutdown())
        .unwrap_or_else(|_| panic!("proxy shutdown requests exact worker shutdown"))
        .sends
        .worker_shutdowns
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker shutdown is requested"))
        .id
}

#[test]
fn owner_control_uses_a_creation_id_not_a_runtime_route() {
    let _input: ChildInput<
        StableProxy<Worker, ImmediateActivation>,
        StableProxy<Worker, ImmediateActivation>,
        ProxyControl<Worker, ImmediateActivation>,
        behavior::ChildHead,
    > = ChildInput::after(first_creation(), ProxyControl::start(Worker(4)));
}

#[test]
fn never_started_proxy_stops_without_runtime_work() {
    let initialized = StableProxy::<Worker, ImmediateActivation>::immediate()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let stopped = proxy
        .on(ProxyControl::shutdown())
        .unwrap_or_else(|_| panic!("an empty proxy stops immediately"));

    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert!(stopped.sends.worker_shutdowns.is_empty());
}

#[test]
fn a_non_integer_runtime_route_does_not_enter_proxy_creation() {
    let initialized = StableProxy::immediate()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(ProxyControl::start(Worker(1)))
        .unwrap_or_else(|_| panic!("worker admission emits one creation"));

    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    assert_eq!(creating.creates.len(), 1);
    let creation = creating
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker creation is emitted"));
    assert_eq!(creation.id(), first_creation());
}

#[test]
fn namespace_rejection_returns_the_worker_and_closes_initial_start() {
    let initialized = StableProxy::immediate()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(ProxyControl::start(Worker(1)))
        .unwrap_or_else(|_| panic!("worker admission emits one creation"));
    let rejected = proxy
        .on(CreationsSettled::new(CreationSettlement::Rejected {
            creations: creating.creates,
            reason: ChildNamespaceExhausted,
        }))
        .unwrap_or_else(|_| panic!("namespace rejection is a total transition"));

    assert_eq!(proxy.phase(), ProxyPhase::EmptyInitial);
    let outcome = rejected
        .sends
        .owner_outcomes
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the owner receives the rejected worker"))
        .into_inner();
    match outcome {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result:
                        WorkerStartResult::CreationRejected {
                            rejection: WorkerCreationRejection::NamespaceExhausted { worker },
                            activation,
                            stopped: None,
                        },
                },
        } => {
            assert_eq!(worker, Worker(1));
            assert_eq!(activation, ImmediateActivation);
        }
        _ => panic!("namespace rejection returned a different owner outcome"),
    }

    let overlap = proxy
        .on(ProxyControl::start(Worker(2)))
        .unwrap_or_else(|_| panic!("a second initial start is classified"));
    assert_eq!(overlap.creates.len(), 0);

    let stopped = proxy
        .on(ProxyControl::shutdown())
        .unwrap_or_else(|_| panic!("an initial-empty proxy stops immediately"));
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert!(stopped.sends.worker_shutdowns.is_empty());
}

#[tokio::test]
async fn shutdown_result_then_worker_stop_closes_the_proxy() {
    let (mut proxy, worker) = ready_proxy().await;
    let shutdown = begin_shutdown(&mut proxy);
    let waiting = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown))
        .unwrap_or_else(|_| panic!("the exact shutdown result is accepted"));

    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(waiting.become_, Step::Continue));

    let stopped = proxy
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact worker stop is accepted"));
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn worker_stop_then_shutdown_result_closes_the_proxy() {
    let (mut proxy, worker) = ready_proxy().await;
    let shutdown = begin_shutdown(&mut proxy);
    let waiting = proxy
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact worker stop is accepted"));

    assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
    assert!(matches!(waiting.become_, Step::Continue));

    let stopped = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown))
        .unwrap_or_else(|_| panic!("the exact shutdown result is accepted"));
    assert_eq!(proxy.phase(), ProxyPhase::Stopped);
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn replacement_report_waits_for_the_predecessor_shutdown_result() {
    let (mut proxy, predecessor) = ready_proxy().await;
    let replacing = proxy
        .on(ProxyControl::replace(Worker(2)))
        .unwrap_or_else(|_| panic!("replacement begins predecessor drain"));
    let shutdown = replacing
        .sends
        .worker_shutdowns
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("replacement requests predecessor shutdown"));

    let creating = proxy
        .on(ChildStopped::new(
            predecessor,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("predecessor stop permits successor creation"));
    let creation = creating
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one successor creation is emitted"));
    let (successor, _worker, kind) = creation.into_parts();
    let initialized = proxy
        .on(CreationsSettled::new(CreationSettlement::Settled(
            [SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        successor,
                        kind,
                        EstablishedRecipient::issued(WorkerEndpoint),
                    ),
                },
            ))]
            .into_iter()
            .collect(),
        )))
        .unwrap_or_else(|_| panic!("successor creation commits"));
    let initialization = initialized
        .sends
        .worker_initializations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("successor initialization is requested"));
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("successor initialization permits activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("successor activation is requested"));
    let started = proxy
        .on(activation.started())
        .unwrap_or_else(|_| panic!("successor activation starts"));
    assert!(started.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .unwrap_or_else(|_| panic!("successor becomes ready"));

    assert!(ready.sends.owner_outcomes.is_empty());

    let replaced = proxy
        .on(EstablishedShutdownResolved::accepted(shutdown.id))
        .unwrap_or_else(|_| panic!("predecessor settlement completes replacement"));
    let outcome = replaced
        .sends
        .owner_outcomes
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("owner receives one completed replacement"))
        .into_inner();
    match outcome {
        ProxyOutcome::Replacement {
            outcome:
                ReplacementOutcome::Resolved {
                    replaces,
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => {
            assert_eq!(replaces.creation(), predecessor);
            assert_eq!(attempt.creation(), successor);
        }
        _ => panic!("proxy published a different replacement outcome"),
    }
}
