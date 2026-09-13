#![expect(
    clippy::drop_non_drop,
    reason = "the interpreter and child host consume these exact values at this point"
)]

use core::convert::Infallible;
use core::num::NonZeroUsize;
use core::ops::ControlFlow;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use behavior_actors::atomic::{
    self, ActivationPlan, ActivationPolicy, ActorDrainPolicy, CapabilityResult, DiagnosticAction,
    DiagnosticDisposition, FailureReaction, FixedCommand, FixedDiagnostic, FixedLifecycle,
    FixedLifecycleEvent, FixedSupervisor, FixedSupervisorEvent, InitialWorkerOutcome, MemberStatus,
    OrderedRoles, PendingWorkerPreparation, PrepareWorkers, ProxyControl, ProxyInputReceipt,
    ProxyInputResult, ProxyOperation, ProxyOutcome, ProxyPhase, Recovery, RecoveryDenialReason,
    ReplacementOutcome, RestartLimit, RestartRelease, StableProxy, Strategy,
    WorkerInitializationOutcome, WorkerPreparation, WorkerPreparationFailureReason, WorkerSource,
    WorkerStartResult, WorkerSubmission, fixed,
};
use behavior_actors::{
    ActionItemResult, Actions, Activate as _, Active, ActiveTurn, Address, Behavior, BehaviorActed,
    ChildCreationOutcome, ChildInputReason, ChildNamespaceExhausted, ChildReport, ChildStopped,
    Crash, CreateChild, CreationId, CreationKind, CreationSequence, CreationSettlement, Creations,
    CreationsSettled, Delivery, EndpointAddress, EstablishedActor, EstablishedCreation,
    EstablishedDelivery, EstablishedRecipient, EventIngress, Exit, Here, InjectEvent,
    InterpreterFault, ItemSettlement, MessageProtocol, Never, NoBirths, NoSends, Protocol,
    Recipient, RecoverEvent, ReplyDelivery, ScheduleAfter, ScheduleAfterRejection, SendSettlements,
    SettledItem, Step, TimerElapsed, TimerGeneration, TimerId, TimerScheduled, User, UserEvent,
};

const ONE_WORKER: NonZeroUsize = NonZeroUsize::new(1).expect("one is positive");
const THREE_WORKERS: NonZeroUsize = NonZeroUsize::new(3).expect("three is positive");

fn fixed_runtime_sources_are_distinct<Event>()
where
    Event: EventIngress<Here, ProxyInputResult<Here, SearchWorker, SearchActivation>>,
    Event: EventIngress<
            PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
            ActionItemResult<
                PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
            >,
        >,
{
}

struct SearchWorkshop;

impl WorkerSource<SearchRole, SearchWorker, SearchActivation> for SearchWorkshop {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

struct TrackedWorkshop {
    drops: Arc<AtomicUsize>,
}

impl Drop for TrackedWorkshop {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl WorkerSource<SearchRole, SearchWorker, SearchActivation> for TrackedWorkshop {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

impl WorkerSource<TrackedRole, TrackedWorker, TrackedActivation> for TrackedWorkshop {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

struct FallibleWorkshop;

#[derive(Debug, Eq, PartialEq)]
enum WorkshopRejection {
    WorkerUnavailable,
    SourceUnavailable,
}

impl WorkerSource<SearchRole, SearchWorker, SearchActivation> for FallibleWorkshop {
    type WorkerRejection = WorkshopRejection;
    type SourceRejection = WorkshopRejection;
}

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

fn logical_reply<P>(mut replies: Vec<ReplyDelivery<Delivery<P>, EstablishedDelivery<P>>>) -> P::Msg
where
    P: Protocol<Addr = RuntimeAddr>,
{
    match replies.pop().expect("one logical reply") {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the logical route remains logical"),
    }
}

fn established_reply<P>(
    mut replies: Vec<ReplyDelivery<Delivery<P>, EstablishedDelivery<P>>>,
) -> P::Msg
where
    P: Protocol<Addr = RuntimeAddr>,
{
    match replies.pop().expect("one exact reply") {
        ReplyDelivery::Established(delivery) => delivery.message,
        ReplyDelivery::Logical(_) => panic!("the exact route remains exact"),
    }
}

fn terminal_unexpected<Source>(
    mut diagnostics: Vec<
        DiagnosticAction<
            Infallible,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>,
        >,
    >,
) -> FixedSupervisorEvent<
    SearchRole,
    SearchWorker,
    SearchActivation,
    ActionItemResult<PrepareWorkers<Source, SearchRole, SearchWorker, SearchActivation>>,
>
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
{
    assert_eq!(diagnostics.len(), 1);
    match diagnostics.remove(0) {
        DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::UnexpectedInput { input },
        } => input,
        DiagnosticAction::Deliver { route, .. } => match route {},
        DiagnosticAction::Terminal {
            diagnostic:
                FixedDiagnostic::ProxyOutcomeFailed(_)
                | FixedDiagnostic::ProxyInputRejected(_)
                | FixedDiagnostic::WorkerPreparationFailed(_)
                | FixedDiagnostic::RecoveryDenied(_)
                | FixedDiagnostic::RestartScheduleFailed(_)
                | FixedDiagnostic::WorkerUnavailable(_),
        } => panic!("the input keeps its unexpected-input diagnostic"),
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SearchRole {
    Search,
    Index,
    Spellcheck,
}

const SEARCH_ROLES: [SearchRole; 3] = [
    SearchRole::Search,
    SearchRole::Index,
    SearchRole::Spellcheck,
];

enum FailedProxyBirthBatch {
    NamespaceRejected,
    InterpreterCorrupt,
}

#[derive(Debug, Eq, PartialEq)]
struct SearchWorker(SearchRole);

#[derive(Debug, Eq, PartialEq)]
enum SearchCommand {
    Find(String),
}

impl Behavior for SearchWorker {
    type Protocol = MessageProtocol<RuntimeAddr, SearchCommand>;
    type Event = User<RuntimeAddr, SearchCommand>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            SearchCommand::Find(_) => Ok(Actions::cont()),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SearchActivation;

struct SearchLifecycleProtocol;

struct ReadyMember {
    proxy: CreationId,
    worker: atomic::WorkerAttempt,
}

impl Protocol for SearchLifecycleProtocol {
    type Addr = RuntimeAddr;
    type Msg = FixedLifecycle<SearchRole, SearchWorker, SearchActivation>;
}

struct SearchDiagnosticProtocol;

impl Protocol for SearchDiagnosticProtocol {
    type Addr = RuntimeAddr;
    type Msg = FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Never>;
}

struct SearchRecoveryDiagnosticProtocol;

impl Protocol for SearchRecoveryDiagnosticProtocol {
    type Addr = RuntimeAddr;
    type Msg = FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>;
}

impl ActivationPlan for SearchActivation {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

type SearchSupervisorEvent = FixedSupervisorEvent<
    SearchRole,
    SearchWorker,
    SearchActivation,
    ActionItemResult<PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>>,
>;

fn shutdown_supervisor_event(from: RuntimeAddr) -> SearchSupervisorEvent {
    <SearchSupervisorEvent as UserEvent>::user(from, FixedCommand::shutdown())
}

fn assert_shutdown_supervisor_event(event: SearchSupervisorEvent, from: RuntimeAddr) {
    let Ok(user) = <SearchSupervisorEvent as UserEvent>::into_user(event) else {
        panic!("the foreign event preserves the complete command")
    };
    assert_eq!(user.from, from);
    assert!(matches!(user.message, FixedCommand::Shutdown));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrackedRole {
    Primary,
    Replica,
}

struct TrackedWorker {
    drops: Arc<AtomicUsize>,
}

impl Drop for TrackedWorker {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl Behavior for TrackedWorker {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct TrackedActivation {
    readiness_drops: Option<Arc<AtomicUsize>>,
}

struct TrackedReadiness {
    drops: Arc<AtomicUsize>,
}

impl Drop for TrackedReadiness {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl ActivationPlan for TrackedActivation {
    type Ready = Option<TrackedReadiness>;
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(self
            .readiness_drops
            .map(|drops| TrackedReadiness { drops })))
    }
}

fn worker(role: &SearchRole) -> SearchWorker {
    match role {
        SearchRole::Search => SearchWorker(SearchRole::Search),
        SearchRole::Index => SearchWorker(SearchRole::Index),
        SearchRole::Spellcheck => SearchWorker(SearchRole::Spellcheck),
    }
}

fn roles() -> OrderedRoles<SearchRole> {
    OrderedRoles::new(
        SearchRole::Search,
        [SearchRole::Index, SearchRole::Spellcheck],
    )
    .expect("search roles are unique")
}

fn initial_worker(
    role: &SearchRole,
) -> Result<WorkerSubmission<SearchWorker, SearchActivation>, Never> {
    Ok(WorkerSubmission::activated(worker(role), SearchActivation))
}

fn commit_proxy_births(
    creations: Creations<CreateChild<RuntimeAddr, StableProxy<SearchWorker, SearchActivation>>>,
    endpoint: u64,
) -> CreationsSettled<RuntimeAddr, StableProxy<SearchWorker, SearchActivation>> {
    let settlements = creations
        .into_iter()
        .enumerate()
        .map(|(position, creation)| {
            let (id, proxy, kind) = creation.into_parts();
            drop(proxy);
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        id,
                        kind,
                        EstablishedRecipient::issued(Endpoint(endpoint + position as u64)),
                    ),
                },
            ))
        })
        .collect();
    CreationsSettled::new(CreationSettlement::Settled(settlements))
}

fn first_proxy_dispatched(
    endpoint: u64,
) -> (
    Active<
        FixedSupervisor<SearchRole, SearchWorker, SearchActivation, Never, Infallible, Infallible>,
    >,
    ProxyOperation<Here, SearchWorker, SearchActivation>,
) {
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits proxy births"));
    let mut fixed = initialized.behavior;
    assert_eq!(initialized.actions.creates.len(), 3);
    assert_eq!(initialized.actions.sends.proxy_observations.len(), 3);
    let first = fixed
        .on(commit_proxy_births(initialized.actions.creates, endpoint))
        .unwrap_or_else(|_| panic!("the proxy batch receives authorization"));
    let mut operations = first.sends.proxy_operations.unattempted().into_inputs();
    assert_eq!(operations.len(), 1);
    let operation = match operations.pop().expect("one operation is authorized") {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted operation")
        }
    };
    (fixed, operation)
}

fn routed_start(
    roles: OrderedRoles<SearchRole>,
    endpoint: u64,
    diagnostics: EstablishedRecipient<SearchDiagnosticProtocol>,
    reaction: FailureReaction,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Never,
            EstablishedRecipient<SearchDiagnosticProtocol>,
            Infallible,
        >,
    >,
    ProxyOperation<Here, SearchWorker, SearchActivation>,
) {
    let initialized = fixed(
        initial_worker,
        roles,
        ActivationPolicy::new(ONE_WORKER.get()).expect("activation capacity is positive"),
        Recovery::temporary(),
        reaction,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::deliver_to(diagnostics),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits proxy births"));
    let mut fixed = initialized.behavior;
    let count = initialized.actions.creates.len();
    assert_eq!(initialized.actions.sends.proxy_observations.len(), count);
    let authorized = fixed
        .on(commit_proxy_births(initialized.actions.creates, endpoint))
        .unwrap_or_else(|_| panic!("the proxy batch receives its first operation"));
    let operation = match authorized
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one operation is authorized")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the operation has not been interpreted"),
    };
    (fixed, operation)
}

async fn ready_proxy_outcome(
    control: ProxyControl<SearchWorker, SearchActivation>,
) -> ProxyOutcome<SearchWorker, SearchActivation> {
    let initialized = StableProxy::activated()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(control)
        .unwrap_or_else(|_| panic!("initial worker admission emits one creation"));
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("one worker is created");
    let (worker, child, kind) = creation.into_parts();
    drop(child);
    let committed = CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(Endpoint(990)),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ));
    let initializing = proxy
        .on(committed)
        .unwrap_or_else(|_| panic!("worker commit requests initialization"));
    let initialization = initializing
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("one worker initialization is awaited");
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("initialization requests activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("one worker activation is requested");
    let started = proxy
        .on(activation.started())
        .unwrap_or_else(|_| panic!("activation begins once"));
    assert_eq!(started.sends.owner_outcomes.len(), 0);
    proxy
        .on(activation.activate().await)
        .unwrap_or_else(|_| panic!("activation readiness completes startup"))
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("ready proxy reports one initial outcome")
        .into_inner()
}

async fn ready_tracked_proxy_outcome(
    control: ProxyControl<TrackedWorker, TrackedActivation>,
) -> ProxyOutcome<TrackedWorker, TrackedActivation> {
    let initialized = StableProxy::activated()
        .initialize()
        .unwrap_or_else(|_| panic!("tracked proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let creating = proxy
        .on(control)
        .unwrap_or_else(|_| panic!("tracked worker admission emits one creation"));
    let creation = creating
        .creates
        .into_iter()
        .next()
        .expect("one tracked worker is created");
    let (worker, child, kind) = creation.into_parts();
    drop(child);
    let committed = CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(Endpoint(1_812)),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ));
    let initializing = proxy
        .on(committed)
        .unwrap_or_else(|_| panic!("tracked worker commit requests initialization"));
    let initialization = initializing
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("one tracked worker initialization is awaited");
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("tracked initialization requests activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("one tracked worker activation is requested");
    let started = proxy
        .on(activation.started())
        .unwrap_or_else(|_| panic!("tracked activation begins once"));
    assert!(started.sends.owner_outcomes.is_empty());
    proxy
        .on(activation.activate().await)
        .unwrap_or_else(|_| panic!("tracked readiness completes startup"))
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("tracked proxy reports one initial outcome")
        .into_inner()
}

#[test]
fn fixed_runtime_inputs_use_their_declared_sources() {
    fixed_runtime_sources_are_distinct::<
        FixedSupervisorEvent<
            SearchRole,
            SearchWorker,
            SearchActivation,
            ActionItemResult<
                PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
            >,
        >,
    >();
}

#[test]
fn fixed_event_adapters_recover_exact_values_and_return_foreign_events() {
    let elapsed = TimerElapsed::new(TimerId(41), TimerGeneration(43));
    let event = <SearchSupervisorEvent as InjectEvent<TimerElapsed, Here>>::inject_at(elapsed);
    let Ok(recovered) = <SearchSupervisorEvent as RecoverEvent<TimerElapsed, Here>>::recover(event)
    else {
        panic!("the timer event recovers its exact payload")
    };
    assert_eq!(recovered, elapsed);
    let Err(foreign) = <SearchSupervisorEvent as RecoverEvent<TimerElapsed, Here>>::recover(
        shutdown_supervisor_event(RuntimeAddr(45)),
    ) else {
        panic!("a command cannot recover as a timer")
    };
    assert_shutdown_supervisor_event(foreign, RuntimeAddr(45));

    let mut creations = CreationSequence::new();
    let report_child = creations.issue().expect("the report child ID is fresh");
    let report = ChildReport::new(
        report_child,
        ProxyOutcome::Unavailable {
            sender: RuntimeAddr(47),
            phase: ProxyPhase::Dormant,
            command: SearchCommand::Find("event recovery".to_owned()),
        },
    );
    let event = <SearchSupervisorEvent as InjectEvent<
        ChildReport<ProxyOutcome<SearchWorker, SearchActivation>>,
        Here,
    >>::inject_at(report);
    let Ok(recovered) = <SearchSupervisorEvent as RecoverEvent<
        ChildReport<ProxyOutcome<SearchWorker, SearchActivation>>,
        Here,
    >>::recover(event) else {
        panic!("the proxy report recovers its exact payload")
    };
    assert_eq!(recovered.child, report_child);
    match recovered.report {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(47));
            assert_eq!(phase, ProxyPhase::Dormant);
            assert_eq!(command, SearchCommand::Find("event recovery".to_owned()));
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the proxy report retains its unavailable outcome")
        }
    }
    let Err(foreign) = <SearchSupervisorEvent as RecoverEvent<
        ChildReport<ProxyOutcome<SearchWorker, SearchActivation>>,
        Here,
    >>::recover(shutdown_supervisor_event(RuntimeAddr(49))) else {
        panic!("a command cannot recover as a proxy report")
    };
    assert_shutdown_supervisor_event(foreign, RuntimeAddr(49));

    let settlement: CreationsSettled<RuntimeAddr, StableProxy<SearchWorker, SearchActivation>> =
        CreationsSettled::new(CreationSettlement::Settled(Creations::empty()));
    let event = <SearchSupervisorEvent as InjectEvent<
        CreationsSettled<RuntimeAddr, StableProxy<SearchWorker, SearchActivation>>,
        Here,
    >>::inject_at(settlement);
    let Ok(recovered) = <SearchSupervisorEvent as RecoverEvent<
        CreationsSettled<RuntimeAddr, StableProxy<SearchWorker, SearchActivation>>,
        Here,
    >>::recover(event) else {
        panic!("the creation settlement recovers its exact payload")
    };
    let CreationSettlement::Settled(settlements) = recovered.into_settlement() else {
        panic!("the empty settled creation batch retains its alternative")
    };
    assert!(settlements.is_empty());
    let Err(foreign) = <SearchSupervisorEvent as RecoverEvent<
        CreationsSettled<RuntimeAddr, StableProxy<SearchWorker, SearchActivation>>,
        Here,
    >>::recover(shutdown_supervisor_event(RuntimeAddr(51))) else {
        panic!("a command cannot recover as a creation settlement")
    };
    assert_shutdown_supervisor_event(foreign, RuntimeAddr(51));

    let stopped_child = creations.issue().expect("the stopped child ID is fresh");
    let stopped_at = Instant::now();
    let stopped = ChildStopped::new(stopped_child, Ok(Exit::Normal), stopped_at);
    let event =
        <SearchSupervisorEvent as InjectEvent<ChildStopped<RuntimeAddr>, Here>>::inject_at(stopped);
    let Ok(recovered) =
        <SearchSupervisorEvent as RecoverEvent<ChildStopped<RuntimeAddr>, Here>>::recover(event)
    else {
        panic!("the proxy stop recovers its exact payload")
    };
    assert_eq!(recovered, stopped);
    let Err(foreign) =
        <SearchSupervisorEvent as RecoverEvent<ChildStopped<RuntimeAddr>, Here>>::recover(
            shutdown_supervisor_event(RuntimeAddr(53)),
        )
    else {
        panic!("a command cannot recover as a proxy stop")
    };
    assert_shutdown_supervisor_event(foreign, RuntimeAddr(53));

    let user = shutdown_supervisor_event(RuntimeAddr(55));
    assert_shutdown_supervisor_event(user, RuntimeAddr(55));
    let elapsed = TimerElapsed::new(TimerId(57), TimerGeneration(59));
    let event = <SearchSupervisorEvent as InjectEvent<TimerElapsed, Here>>::inject_at(elapsed);
    let Err(foreign) = <SearchSupervisorEvent as UserEvent>::into_user(event) else {
        panic!("a timer cannot recover as a command")
    };
    match foreign {
        FixedSupervisorEvent::RestartElapsed(returned) => assert_eq!(returned, elapsed),
        FixedSupervisorEvent::Command(_)
        | FixedSupervisorEvent::ProxyCreationsSettled(_)
        | FixedSupervisorEvent::ProxyInputSettled(_)
        | FixedSupervisorEvent::ProxyReported(_)
        | FixedSupervisorEvent::ProxyStopped(_)
        | FixedSupervisorEvent::WorkerPreparationSettled(_)
        | FixedSupervisorEvent::RestartScheduleSettled(_) => {
            panic!("the user projection returns the complete timer event")
        }
    }
}

#[test]
fn automatic_recovery_has_one_named_worker_preparation_lane() {
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(2).expect("activation capacity is positive"),
        Recovery::permanent(
            SearchWorkshop,
            Strategy::OneForOne,
            RestartLimit::new(3, Duration::from_secs(60)),
            RestartRelease::immediate(),
        ),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits reservations"));

    assert_eq!(
        initialized
            .actions
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
}

#[test]
fn proxy_birth_batch_requires_each_roles_creation_and_birth_kind() {
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(3).expect("three proxies may start together"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits three proxy births"));
    let mut supervisor = initialized.behavior;
    let requested = initialized
        .actions
        .creates
        .into_iter()
        .map(|creation| {
            let (creation, proxy, kind) = creation.into_parts();
            drop(proxy);
            (creation, kind)
        })
        .collect::<Vec<_>>();
    assert_eq!(requested.len(), 3);
    let rotated = [requested[1].0, requested[2].0, requested[0].0];
    let submitted = requested
        .into_iter()
        .enumerate()
        .map(|(position, (_, kind))| {
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        rotated[position],
                        kind,
                        EstablishedRecipient::issued(Endpoint(1200 + position as u64)),
                    ),
                },
            ))
        })
        .collect();
    let rejected = supervisor
        .on(CreationsSettled::new(CreationSettlement::Settled(
            submitted,
        )))
        .unwrap_or_else(|_| panic!("rotated proxy creations become one diagnostic"));
    assert!(rejected.sends.proxy_operations.is_empty());
    let returned = match terminal_unexpected(rejected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyCreationsSettled(returned) => returned,
        _ => panic!("the rotated batch keeps its proxy-creation meaning"),
    };
    let CreationSettlement::Settled(returned) = returned.into_settlement() else {
        panic!("the rotated batch remains settled")
    };
    let returned = returned
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established { established },
            )) => (established.id(), established.kind()),
            SettledItem::Attempted(ItemSettlement::Accepted(_))
            | SettledItem::Attempted(ItemSettlement::Rejected { .. })
            | SettledItem::Attempted(ItemSettlement::Corrupt { .. })
            | SettledItem::Unattempted(_) => {
                panic!("each rotated established proxy is returned complete")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        returned,
        rotated.map(|creation| (creation, CreationKind::Birth))
    );

    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(3).expect("three proxies may start together"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits three proxy births"));
    let mut supervisor = initialized.behavior;
    let submitted = initialized
        .actions
        .creates
        .into_iter()
        .enumerate()
        .map(|(position, creation)| {
            let (creation, proxy, kind) = creation.into_parts();
            drop(proxy);
            let kind = match position {
                0 => CreationKind::replacement(creation),
                _ => kind,
            };
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        creation,
                        kind,
                        EstablishedRecipient::issued(Endpoint(1300 + position as u64)),
                    ),
                },
            ))
        })
        .collect();
    let rejected = supervisor
        .on(CreationsSettled::new(CreationSettlement::Settled(
            submitted,
        )))
        .unwrap_or_else(|_| panic!("one replacement kind rejects the complete batch"));
    assert!(rejected.sends.proxy_operations.is_empty());
    let returned = match terminal_unexpected(rejected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyCreationsSettled(returned) => returned,
        _ => panic!("the wrong-kind batch keeps its proxy-creation meaning"),
    };
    let CreationSettlement::Settled(returned) = returned.into_settlement() else {
        panic!("the wrong-kind batch remains settled")
    };
    let returned = returned
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established { established },
            )) => (established.id(), established.kind()),
            SettledItem::Attempted(ItemSettlement::Accepted(_))
            | SettledItem::Attempted(ItemSettlement::Rejected { .. })
            | SettledItem::Attempted(ItemSettlement::Corrupt { .. })
            | SettledItem::Unattempted(_) => {
                panic!("each established proxy in the wrong-kind batch is returned complete")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(returned.len(), 3);
    assert_eq!(returned[0].1, CreationKind::replacement(returned[0].0));
    assert_eq!(returned[1].1, CreationKind::Birth);
    assert_eq!(returned[2].1, CreationKind::Birth);
}

#[test]
fn failed_proxy_birth_batches_require_exact_pending_creations() {
    let shutdown_with_pending_proxy_births = || {
        let initialized = fixed(
            initial_worker,
            roles(),
            ActivationPolicy::new(3).expect("three proxies may start together"),
            Recovery::temporary(),
            FailureReaction::StopSupervisor,
            ActorDrainPolicy::WaitForActorGraph,
            DiagnosticDisposition::terminate(),
        )
        .build::<SearchWorker, SearchActivation, Never>()
        .unwrap_or_else(|_| panic!("every initial worker prepares"))
        .initialize()
        .unwrap_or_else(|_| panic!("fixed initialization emits three proxy births"));
        let mut supervisor = initialized.behavior;
        let creations = initialized.actions.creates.into_iter().collect::<Vec<_>>();
        let shutdown = supervisor
            .receive(RuntimeAddr(951), atomic::FixedCommand::shutdown())
            .unwrap_or_else(|_| panic!("shutdown retains all three pending proxy births"));
        assert!(matches!(shutdown.become_, Step::Continue));
        assert!(shutdown.sends.proxy_operations.is_empty());
        (supervisor, creations)
    };

    for failure in [
        FailedProxyBirthBatch::NamespaceRejected,
        FailedProxyBirthBatch::InterpreterCorrupt,
    ] {
        let (mut exact_owner, exact_creations) = shutdown_with_pending_proxy_births();
        let exact = match &failure {
            FailedProxyBirthBatch::NamespaceRejected => CreationSettlement::Rejected {
                creations: exact_creations.into_iter().collect(),
                reason: ChildNamespaceExhausted,
            },
            FailedProxyBirthBatch::InterpreterCorrupt => CreationSettlement::Corrupt {
                creations: exact_creations.into_iter().collect(),
                fault: InterpreterFault::CorruptTraversal,
            },
        };
        let accepted = exact_owner
            .on(CreationsSettled::new(exact))
            .unwrap_or_else(|_| panic!("the exact failed proxy-birth batch retires cleanly"));
        assert!(matches!(accepted.become_, Step::Stop(_)));
        assert!(accepted.sends.proxy_operations.is_empty());
        assert!(accepted.sends.diagnostics.is_empty());

        let (mut rotated_owner, mut rotated_creations) = shutdown_with_pending_proxy_births();
        rotated_creations.rotate_left(1);
        let expected = rotated_creations
            .iter()
            .map(|creation| (creation.id(), creation.kind()))
            .collect::<Vec<_>>();
        let rotated = match &failure {
            FailedProxyBirthBatch::NamespaceRejected => CreationSettlement::Rejected {
                creations: rotated_creations.into_iter().collect(),
                reason: ChildNamespaceExhausted,
            },
            FailedProxyBirthBatch::InterpreterCorrupt => CreationSettlement::Corrupt {
                creations: rotated_creations.into_iter().collect(),
                fault: InterpreterFault::CorruptTraversal,
            },
        };
        let rejected = rotated_owner
            .on(CreationsSettled::new(rotated))
            .unwrap_or_else(|_| panic!("the rotated failed batch becomes one diagnostic"));
        assert!(rejected.sends.proxy_operations.is_empty());
        let returned = match terminal_unexpected(rejected.sends.diagnostics.into_requests()) {
            FixedSupervisorEvent::ProxyCreationsSettled(returned) => returned,
            _ => panic!("the failed batch keeps its proxy-creation meaning"),
        };
        let returned = match (&failure, returned.into_settlement()) {
            (
                FailedProxyBirthBatch::NamespaceRejected,
                CreationSettlement::Rejected { creations, reason },
            ) => {
                assert_eq!(reason, ChildNamespaceExhausted);
                creations
            }
            (
                FailedProxyBirthBatch::InterpreterCorrupt,
                CreationSettlement::Corrupt { creations, fault },
            ) => {
                assert_eq!(fault, InterpreterFault::CorruptTraversal);
                creations
            }
            _ => panic!("the failed batch retains its exact result class"),
        };
        assert_eq!(
            returned
                .into_iter()
                .map(|creation| (creation.id(), creation.kind()))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn accepted_proxy_input_keeps_its_authorization_occupied() {
    let (mut fixed, operation) = first_proxy_dispatched(301);
    let (creation, _, operation) = operation.into_parts();
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(401));
    let accepted = SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
        creation, proxy, operation,
    )));
    let settled = fixed
        .on(accepted)
        .unwrap_or_else(|_| panic!("exact accepted input advances its dispatched member"));

    assert_eq!(
        settled
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
}

#[tokio::test]
async fn exact_ready_proxy_report_releases_capacity_for_the_next_role() {
    let (mut fixed, operation) = first_proxy_dispatched(401);
    let (creation, control, operation) = operation.into_parts();
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(501));
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(creation, proxy, operation),
        )))
        .unwrap_or_else(|_| panic!("accepted input awaits the atomic proxy outcome"));
    assert_eq!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    let outcome = ready_proxy_outcome(control).await;
    let released = fixed
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("the exact initial ready outcome advances the member"));
    let NoSends = released.sends.lifecycle;
    let creations = released
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.creation().get(),
            SettledItem::Attempted(_) => {
                panic!("the next authorized operation remains complete")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(creations, [2]);
}

#[test]
fn exact_non_ready_proxy_report_transfers_one_terminal_diagnostic_and_stops() {
    let (mut fixed, operation) = first_proxy_dispatched(451);
    let (creation, _, operation) = operation.into_parts();
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(551));
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(creation, proxy, operation),
        )))
        .unwrap_or_else(|_| panic!("accepted input awaits its exact outcome"));
    assert_eq!(accepted.creates.len(), 0);
    assert!(matches!(accepted.become_, Step::Continue));
    assert_eq!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    assert_eq!(accepted.sends.diagnostics.into_requests().len(), 0);

    let stopped = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Initial {
                outcome: InitialWorkerOutcome::Overlap {
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                    phase: ProxyPhase::Creating,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("terminal diagnostic policy accepts the complete failure"));

    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(
        stopped
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    let mut diagnostics = stopped.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 1);
    match diagnostics.remove(0) {
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::ProxyOutcomeFailed(failure),
        } => {
            assert_eq!(failure.role(), &SearchRole::Search);
            match failure.outcome() {
                ProxyOutcome::Initial {
                    outcome:
                        InitialWorkerOutcome::Overlap {
                            phase: ProxyPhase::Creating,
                            ..
                        },
                } => {}
                ProxyOutcome::Initial { .. }
                | ProxyOutcome::Replacement { .. }
                | ProxyOutcome::WorkerStopped { .. }
                | ProxyOutcome::Unavailable { .. } => {
                    panic!("the complete non-ready outcome remains in the diagnostic")
                }
            }
        }
        atomic::DiagnosticAction::Deliver { .. } => {
            panic!("terminate policy cannot fabricate a diagnostic route")
        }
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::WorkerPreparationFailed(_),
        } => panic!("an initial proxy failure cannot become a preparation failure"),
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::RecoveryDenied(_),
        } => panic!("an initial proxy failure cannot become a recovery denial"),
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::RestartScheduleFailed(_),
        } => panic!("an initial proxy failure cannot become a schedule failure"),
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::ProxyInputRejected(_),
        } => panic!("an initial proxy failure cannot become an input rejection"),
        atomic::DiagnosticAction::Terminal {
            diagnostic:
                FixedDiagnostic::WorkerUnavailable(_) | FixedDiagnostic::UnexpectedInput { .. },
        } => panic!("an initial proxy failure cannot become an unavailable command"),
    }
}

#[test]
fn terminal_initial_failure_retains_other_workers_until_supervisor_retirement() {
    let primary_drops = Arc::new(AtomicUsize::new(0));
    let replica_drops = Arc::new(AtomicUsize::new(0));
    let failed_worker_drops = Arc::new(AtomicUsize::new(0));
    let primary_tracker = Arc::clone(&primary_drops);
    let replica_tracker = Arc::clone(&replica_drops);
    let roles = OrderedRoles::new(TrackedRole::Primary, [TrackedRole::Replica])
        .expect("tracked roles are unique");
    let initialized = fixed(
        move |role: &TrackedRole| {
            let drops = match role {
                TrackedRole::Primary => Arc::clone(&primary_tracker),
                TrackedRole::Replica => Arc::clone(&replica_tracker),
            };
            Ok::<_, Never>(WorkerSubmission::activated(
                TrackedWorker { drops },
                TrackedActivation {
                    readiness_drops: None,
                },
            ))
        },
        roles,
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<TrackedWorker, TrackedActivation, Never>()
    .unwrap_or_else(|_| panic!("both tracked workers prepare"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits proxy births"));
    let mut fixed = initialized.behavior;
    let settlements = initialized
        .actions
        .creates
        .into_iter()
        .enumerate()
        .map(|(position, creation)| {
            let (id, proxy, kind) = creation.into_parts();
            drop(proxy);
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        id,
                        kind,
                        EstablishedRecipient::issued(Endpoint(1_700 + position as u64)),
                    ),
                },
            ))
        })
        .collect();
    let started = fixed
        .on(CreationsSettled::new(CreationSettlement::Settled(
            settlements,
        )))
        .unwrap_or_else(|_| panic!("the proxy batch authorizes its first worker"));
    let operation = match started
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one worker is authorized")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the operation remains uninterpreted"),
    };
    let (creation, control, operation) = operation.into_parts();
    drop(control);
    assert_eq!(primary_drops.load(Ordering::SeqCst), 1);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<TrackedWorker, TrackedActivation>>::issued(
                    Endpoint(1_800),
                ),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact proxy input is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));

    let stopped = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Initial {
                outcome: InitialWorkerOutcome::Overlap {
                    worker: TrackedWorker {
                        drops: Arc::clone(&failed_worker_drops),
                    },
                    activation: TrackedActivation {
                        readiness_drops: None,
                    },
                    phase: ProxyPhase::Creating,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact initial failure stops the supervisor"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(replica_drops.load(Ordering::SeqCst), 0);
    assert_eq!(failed_worker_drops.load(Ordering::SeqCst), 0);

    drop(fixed);
    assert_eq!(replica_drops.load(Ordering::SeqCst), 1);
    assert_eq!(failed_worker_drops.load(Ordering::SeqCst), 0);
    drop(stopped);
    assert_eq!(failed_worker_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn terminal_restart_denial_retains_member_and_prepared_worker_until_retirement() {
    let initial_worker_drops = Arc::new(AtomicUsize::new(0));
    let readiness_drops = Arc::new(AtomicUsize::new(0));
    let replacement_drops = Arc::new(AtomicUsize::new(0));
    let source_drops = Arc::new(AtomicUsize::new(0));
    let initialized = fixed(
        {
            let initial_worker_drops = Arc::clone(&initial_worker_drops);
            let readiness_drops = Arc::clone(&readiness_drops);
            move |_: &TrackedRole| {
                Ok::<_, Never>(WorkerSubmission::activated(
                    TrackedWorker {
                        drops: Arc::clone(&initial_worker_drops),
                    },
                    TrackedActivation {
                        readiness_drops: Some(Arc::clone(&readiness_drops)),
                    },
                ))
            }
        },
        OrderedRoles::new(TrackedRole::Primary, []).expect("one tracked role is unique"),
        ActivationPolicy::new(1).expect("one tracked proxy may start"),
        Recovery::permanent(
            TrackedWorkshop {
                drops: Arc::clone(&source_drops),
            },
            Strategy::OneForOne,
            RestartLimit::new(0, Duration::from_secs(60)),
            RestartRelease::immediate(),
        ),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<TrackedWorker, TrackedActivation, Never>()
    .unwrap_or_else(|_| panic!("the tracked worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one tracked proxy"));
    let mut fixed = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .expect("one tracked proxy creation is emitted");
    let (route, proxy, kind) = creation.into_parts();
    drop(proxy);
    let dispatched = fixed
        .on(CreationsSettled::new(CreationSettlement::Settled(
            [SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        route,
                        kind,
                        EstablishedRecipient::issued(Endpoint(1_810)),
                    ),
                },
            ))]
            .into_iter()
            .collect(),
        )))
        .unwrap_or_else(|_| panic!("the tracked proxy receives its initial input"));
    let operation = match dispatched
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one tracked initial operation is emitted")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the test intercepts the initial operation"),
    };
    let (route, control, operation) = operation.into_parts();
    let ready = ready_tracked_proxy_outcome(control).await;
    let attempt = match &ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the tracked proxy reaches ready"),
    };
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<TrackedWorker, TrackedActivation>>::issued(
                    Endpoint(1_811),
                ),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the tracked initial input is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));
    let online = fixed
        .on(ChildReport::new(route, ready))
        .unwrap_or_else(|_| panic!("the tracked proxy opens its role"));
    assert!(matches!(online.become_, Step::Continue));

    let preparing = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::WorkerStopped {
                worker: attempt.clone(),
                stopped: ChildStopped::new(attempt.creation(), Ok(Exit::Normal), Instant::now()),
            },
        ))
        .unwrap_or_else(|_| panic!("the exact tracked stop starts recovery"));
    let request = match preparing
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one tracked replacement is requested")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts tracked preparation"),
    };
    let preparation = match request.accept(WorkerSubmission::activated(
        TrackedWorker {
            drops: Arc::clone(&replacement_drops),
        },
        TrackedActivation {
            readiness_drops: None,
        },
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => panic!("one tracked role completes preparation"),
    };
    let denied = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the tracked recovery reaches restart denial"));
    assert!(matches!(denied.become_, Step::Stop(_)));
    assert_eq!(readiness_drops.load(Ordering::SeqCst), 0);
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 0);
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);

    drop(fixed);
    assert_eq!(readiness_drops.load(Ordering::SeqCst), 1);
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 1);
    assert_eq!(source_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn routed_non_ready_report_delivers_diagnostic_and_retires_after_proxy_exit() {
    let diagnostic_route = EstablishedRecipient::<SearchDiagnosticProtocol>::issued(Endpoint(651));
    let (mut fixed, operation) = routed_start(
        OrderedRoles::new(SearchRole::Search, []).expect("one role is non-empty"),
        652,
        diagnostic_route.clone(),
        FailureReaction::RetireMember,
    );
    let (creation, control, operation) = operation.into_parts();
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(654));
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(creation, proxy, operation),
        )))
        .unwrap_or_else(|_| panic!("the exact operation is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));

    let failing = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Initial {
                outcome: InitialWorkerOutcome::Overlap {
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                    phase: ProxyPhase::Creating,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("routed failure starts exact proxy retirement"));
    assert!(matches!(failing.become_, Step::Continue));
    let mut diagnostics = failing.sends.diagnostics.into_requests();
    match diagnostics.remove(0) {
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyOutcomeFailed(failure),
            ..
        } => {
            assert_eq!(failure.role(), &SearchRole::Search);
            assert!(matches!(
                failure.outcome(),
                ProxyOutcome::Initial {
                    outcome: InitialWorkerOutcome::Overlap { .. }
                }
            ));
        }
        atomic::DiagnosticAction::Terminal { .. } => {
            panic!("routed policy cannot become terminal custody")
        }
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::WorkerPreparationFailed(_),
            ..
        } => panic!("an initial proxy failure cannot become a preparation failure"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RecoveryDenied(_),
            ..
        } => panic!("an initial proxy failure cannot become a recovery denial"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RestartScheduleFailed(_),
            ..
        } => panic!("an initial proxy failure cannot become a schedule failure"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyInputRejected(_),
            ..
        } => panic!("an initial proxy failure cannot become an input rejection"),
        atomic::DiagnosticAction::Deliver {
            diagnostic:
                FixedDiagnostic::WorkerUnavailable(_) | FixedDiagnostic::UnexpectedInput { .. },
            ..
        } => panic!("an initial proxy failure cannot become an unavailable command"),
    }
    let shutdown = match failing
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("retiring one member emits one proxy shutdown")
    {
        SettledItem::Unattempted(shutdown) => shutdown,
        SettledItem::Attempted(_) => panic!("the shutdown has not been interpreted"),
    };
    let fleet_shutdown = fixed
        .receive(RuntimeAddr(653), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("fleet shutdown adopts a member stop already in progress"));
    assert!(matches!(fleet_shutdown.become_, Step::Continue));
    assert_eq!(
        fleet_shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let shutdown_accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                shutdown_creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    654,
                )),
                shutdown_operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("shutdown acceptance still awaits exact proxy exit"));
    assert!(matches!(shutdown_accepted.become_, Step::Continue));

    let retired = fixed
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("exact proxy exit closes the stop join"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    let unexpected = fixed
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the duplicate exit becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Continue));
    match unexpected
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one routed diagnostic owns the duplicate exit")
    {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyStopped(_),
                },
        } => assert_eq!(route, diagnostic_route),
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("one exact proxy exit cannot retire the member twice")
        }
    }
    drop(control);
}

#[test]
fn routed_initial_failure_stops_the_complete_supervisor() {
    let diagnostics = EstablishedRecipient::<SearchDiagnosticProtocol>::issued(Endpoint(661));
    let (mut fixed, operation) = routed_start(
        roles(),
        662,
        diagnostics.clone(),
        FailureReaction::StopSupervisor,
    );
    let (route, control, operation) = operation.into_parts();
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    762,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact operation is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));

    let failed = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::Initial {
                outcome: InitialWorkerOutcome::Overlap {
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                    phase: ProxyPhase::Creating,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact failure begins complete supervisor shutdown"));
    assert!(matches!(failed.become_, Step::Continue));
    let mut diagnostic_actions = failed.sends.diagnostics.into_requests();
    match diagnostic_actions.pop().expect("one routed diagnostic") {
        atomic::DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::ProxyOutcomeFailed(failure),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(failure.role(), &SearchRole::Search);
        }
        atomic::DiagnosticAction::Deliver { .. } | atomic::DiagnosticAction::Terminal { .. } => {
            panic!("the exact initial outcome keeps its routed diagnostic")
        }
    }
    assert_eq!(diagnostic_actions.len(), 0);
    assert_eq!(
        failed
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        3
    );

    let repeated = fixed
        .receive(RuntimeAddr(762), FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("repeated shutdown preserves the current drain"));
    assert_eq!(
        repeated
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );

    drop(control);
}

#[tokio::test]
async fn replacement_outcome_cannot_satisfy_initial_startup() {
    let (mut fixed, operation) = first_proxy_dispatched(1001);
    let (creation, control, operation) = operation.into_parts();
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(1101));
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(creation, proxy, operation),
        )))
        .unwrap_or_else(|_| panic!("initial input awaits an initial outcome"));
    assert_eq!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );

    let wrong = ChildReport::new(
        creation,
        ProxyOutcome::Replacement {
            outcome: ReplacementOutcome::NotReplaceable {
                worker: SearchWorker(SearchRole::Search),
                activation: SearchActivation,
                phase: ProxyPhase::Ready,
            },
        },
    );
    let unexpected = fixed
        .on(wrong)
        .unwrap_or_else(|_| panic!("the replacement result becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("a replacement result cannot complete initial startup"),
    };
    match returned.report {
        ProxyOutcome::Replacement {
            outcome: ReplacementOutcome::NotReplaceable { .. },
        } => {}
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the wrong-kind outcome remains complete"),
    }

    let outcome = ready_proxy_outcome(control).await;
    let exact = fixed
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("wrong-kind input did not consume initial startup"));
    assert_eq!(
        exact
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );
}

#[test]
fn another_supervisors_proxy_input_settlement_returns_to_its_owner() {
    let (mut owner, owner_operation) = first_proxy_dispatched(501);
    let (mut source, source_operation) = first_proxy_dispatched(601);
    let (source_creation, _, source_operation) = source_operation.into_parts();
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(701));
    let foreign = SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
        source_creation,
        proxy,
        source_operation,
    )));

    let unexpected = owner
        .on(foreign)
        .unwrap_or_else(|_| panic!("the foreign settlement becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyInputSettled(settlement) => settlement,
        _ => panic!("a foreign operation cannot advance this supervisor"),
    };
    let accepted = source
        .on(returned)
        .unwrap_or_else(|_| panic!("the unchanged settlement remains valid for its owner"));
    assert_eq!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    let (owner_creation, _, owner_operation) = owner_operation.into_parts();
    let owner_accepted = owner
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                owner_creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    702,
                )),
                owner_operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("foreign traversal preserves the owner's exact operation"));
    assert_eq!(
        owner_accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
}

fn single_proxy_dispatched(
    nonce: u64,
) -> (
    Active<
        FixedSupervisor<SearchRole, SearchWorker, SearchActivation, Never, Infallible, Infallible>,
    >,
    ProxyOperation<Here, SearchWorker, SearchActivation>,
) {
    single_proxy_dispatched_with_recovery(nonce, Recovery::temporary())
}

fn single_proxy_dispatched_with_recovery<Source>(
    nonce: u64,
    recovery: Recovery<Source>,
) -> (
    Active<
        FixedSupervisor<SearchRole, SearchWorker, SearchActivation, Source, Infallible, Infallible>,
    >,
    ProxyOperation<Here, SearchWorker, SearchActivation>,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
{
    single_proxy_dispatched_with_policies(
        nonce,
        recovery,
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
}

fn single_proxy_dispatched_with_policies<Source, DiagnosticRoute>(
    nonce: u64,
    recovery: Recovery<Source>,
    failure_reaction: FailureReaction,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            Infallible,
        >,
    >,
    ProxyOperation<Here, SearchWorker, SearchActivation>,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(SearchRole::Search, [])
            .unwrap_or_else(|_| panic!("the single role is unique")),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        recovery,
        failure_reaction,
        actor_drain,
        diagnostics,
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("the initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one proxy birth"));
    let mut fixed = initialized.behavior;
    let dispatched = fixed
        .on(commit_proxy_births(
            initialized.actions.creates,
            nonce + 100,
        ))
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"));
    let initial = match dispatched
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one initial proxy operation is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted operation")
        }
    };
    (fixed, initial)
}

async fn ready_single_proxy_with_worker() -> (
    Active<
        FixedSupervisor<SearchRole, SearchWorker, SearchActivation, Never, Infallible, Infallible>,
    >,
    ReadyMember,
) {
    ready_single_proxy_with_recovery(Recovery::temporary()).await
}

async fn ready_single_proxy_with_recovery<Source>(
    recovery: Recovery<Source>,
) -> (
    Active<
        FixedSupervisor<SearchRole, SearchWorker, SearchActivation, Source, Infallible, Infallible>,
    >,
    ReadyMember,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
{
    ready_single_proxy_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .await
}

async fn ready_single_proxy_with_policies<Source, DiagnosticRoute>(
    recovery: Recovery<Source>,
    failure_reaction: FailureReaction,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            Infallible,
        >,
    >,
    ReadyMember,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let (mut fixed, initial) = single_proxy_dispatched_with_policies(
        701,
        recovery,
        failure_reaction,
        actor_drain,
        diagnostics,
    );
    let (route, control, operation) = initial.into_parts();
    let ready = ready_proxy_outcome(control).await;
    let worker = match &ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: atomic::WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the proxy fixture reaches ready"),
    };
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));
    let online = fixed
        .on(ChildReport::new(route, ready))
        .unwrap_or_else(|_| panic!("the exact ready outcome opens the role"));
    assert!(matches!(online.become_, Step::Continue));
    (
        fixed,
        ReadyMember {
            proxy: route,
            worker,
        },
    )
}

async fn ready_single_proxy_with_lifecycle() -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Never,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    ReadyMember,
) {
    ready_single_proxy_with_lifecycle_policies(
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
    .await
}

async fn ready_single_proxy_with_lifecycle_policies<Source, DiagnosticRoute>(
    recovery: Recovery<Source>,
    failure_reaction: FailureReaction,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    ReadyMember,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(SearchRole::Search, []).expect("one role is non-empty"),
        ActivationPolicy::new(ONE_WORKER.get()).expect("activation capacity is positive"),
        recovery,
        failure_reaction,
        ActorDrainPolicy::WaitForActorGraph,
        diagnostics,
    )
    .publish_lifecycle(EstablishedRecipient::<SearchLifecycleProtocol>::issued(
        Endpoint(970),
    ))
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("the initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one proxy birth"));
    let mut fixed = initialized.behavior;
    let dispatched = fixed
        .on(commit_proxy_births(initialized.actions.creates, 801))
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"));
    let initial = match dispatched
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one initial proxy operation is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted operation")
        }
    };
    let (route, control, operation) = initial.into_parts();
    let ready = ready_proxy_outcome(control).await;
    let worker = match &ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the proxy fixture reaches ready"),
    };
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let online = fixed
        .on(ChildReport::new(route, ready))
        .unwrap_or_else(|_| panic!("the exact ready outcome opens the role"));
    assert_eq!(online.sends.lifecycle.len(), 1);
    assert!(matches!(
        online.sends.lifecycle[0].message.event(),
        FixedLifecycleEvent::Started { .. }
    ));
    (
        fixed,
        ReadyMember {
            proxy: route,
            worker,
        },
    )
}

#[tokio::test]
async fn logical_lifecycle_route_publishes_exact_started_event() {
    let lifecycle = Recipient::<SearchLifecycleProtocol>::global(RuntimeAddr(972));
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(SearchRole::Search, []).expect("one role is non-empty"),
        ActivationPolicy::new(ONE_WORKER.get()).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .publish_lifecycle(lifecycle.clone())
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("the initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one proxy birth"));
    let mut fixed = initialized.behavior;
    let dispatched = fixed
        .on(commit_proxy_births(initialized.actions.creates, 802))
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"));
    let operation = match dispatched
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one initial proxy operation is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted operation")
        }
    };
    let (route, control, operation) = operation.into_parts();
    let ready = ready_proxy_outcome(control).await;
    let proxy =
        EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(802));
    let expected_proxy = proxy.clone();
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(route, proxy, operation),
        )))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());

    let online = fixed
        .on(ChildReport::new(route, ready))
        .unwrap_or_else(|_| panic!("the exact ready outcome opens the role"));
    let mut deliveries = online.sends.lifecycle;
    assert_eq!(deliveries.len(), 1);
    let delivery = deliveries.pop().expect("one logical lifecycle delivery");
    assert_eq!(delivery.to, lifecycle);
    match delivery.message.event() {
        FixedLifecycleEvent::Started { role, proxy } => {
            assert_eq!(role, &SearchRole::Search);
            assert_eq!(proxy, &expected_proxy);
        }
        FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::Unavailable { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("the first ready worker publishes Started")
        }
    }
}

fn three_role_initial_operations(
    strategy: Strategy,
    maximum: NonZeroUsize,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    Vec<ProxyOperation<Here, SearchWorker, SearchActivation>>,
) {
    three_role_initial_operations_with_policies(
        Recovery::permanent(
            SearchWorkshop,
            strategy,
            RestartLimit::new(3, Duration::from_secs(60)),
            RestartRelease::immediate(),
        ),
        maximum,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
}

fn three_role_initial_operations_with_policies<Source, DiagnosticRoute>(
    recovery: Recovery<Source>,
    maximum: NonZeroUsize,
    failure_reaction: FailureReaction,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    Vec<ProxyOperation<Here, SearchWorker, SearchActivation>>,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(maximum.get()).expect("activation capacity is positive"),
        recovery,
        failure_reaction,
        ActorDrainPolicy::WaitForActorGraph,
        diagnostics,
    )
    .publish_lifecycle(EstablishedRecipient::<SearchLifecycleProtocol>::issued(
        Endpoint(970),
    ))
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits proxy births"));
    let mut fixed = initialized.behavior;
    assert_eq!(initialized.actions.creates.len(), 3);
    assert_eq!(initialized.actions.sends.proxy_observations.len(), 3);
    let committed = fixed
        .on(commit_proxy_births(initialized.actions.creates, 801))
        .unwrap_or_else(|_| panic!("the proxy batch commits"));
    let operations = committed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation,
            SettledItem::Attempted(_) => panic!("the test intercepts uninterpreted operations"),
        })
        .collect();
    (fixed, operations)
}

async fn accept_initial_operation<Source, DiagnosticRoute>(
    fixed: &mut Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    operation: ProxyOperation<Here, SearchWorker, SearchActivation>,
) -> (
    ReadyMember,
    Vec<ProxyOperation<Here, SearchWorker, SearchActivation>>,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let (creation, control, operation) = operation.into_parts();
    let outcome = ready_proxy_outcome(control).await;
    let attempt = match &outcome {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: atomic::WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("each proxy reaches ready"),
    };
    let settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    creation.get() + 100,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("each initial input settles"));
    assert!(settled.creates.is_empty());
    assert!(settled.sends.proxy_observations.is_empty());
    assert!(
        settled
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(
        settled
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(settled.sends.diagnostics.is_empty());
    let opened = fixed
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("each ready result enters the roster"));
    assert!(opened.creates.is_empty());
    assert!(opened.sends.proxy_observations.is_empty());
    assert_eq!(opened.sends.lifecycle.len(), 1);
    match opened.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::Started { role, .. } => match creation.get() {
            1 => assert_eq!(role, &SearchRole::Search),
            2 => assert_eq!(role, &SearchRole::Index),
            3 => assert_eq!(role, &SearchRole::Spellcheck),
            _ => panic!("the fixture uses exactly three proxy routes"),
        },
        FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::Unavailable { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("initial readiness publishes only Started")
        }
    }
    assert!(
        opened
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let operations = opened
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation,
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted operations")
            }
        })
        .collect();
    assert!(opened.sends.diagnostics.is_empty());
    (
        ReadyMember {
            proxy: creation,
            worker: attempt,
        },
        operations,
    )
}

async fn ready_three_role_roster(
    strategy: Strategy,
    maximum: NonZeroUsize,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    Vec<ReadyMember>,
) {
    ready_three_role_roster_with_policies(
        Recovery::permanent(
            SearchWorkshop,
            strategy,
            RestartLimit::new(3, Duration::from_secs(60)),
            RestartRelease::immediate(),
        ),
        maximum,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
    .await
}

async fn ready_three_role_roster_with_policies<Source, DiagnosticRoute>(
    recovery: Recovery<Source>,
    maximum: NonZeroUsize,
    failure_reaction: FailureReaction,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    Vec<ReadyMember>,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let (mut fixed, operations) = three_role_initial_operations_with_policies(
        recovery,
        maximum,
        failure_reaction,
        diagnostics,
    );
    let mut operations = VecDeque::from(operations);
    let mut members = Vec::new();
    while let Some(operation) = operations.pop_front() {
        let (member, authorized) = accept_initial_operation(&mut fixed, operation).await;
        members.push(member);
        operations.extend(authorized);
    }
    (fixed, members)
}

async fn coordinated_preparation(
    strategy: Strategy,
    maximum: NonZeroUsize,
    trigger_position: usize,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
    Vec<ReadyMember>,
) {
    let (mut fixed, members) = ready_three_role_roster(strategy, maximum).await;
    let trigger = members
        .get(trigger_position)
        .expect("the trigger is one declared roster position");
    let worker_stop =
        ChildStopped::new(trigger.worker.creation(), Ok(Exit::Normal), Instant::now());
    let actions = fixed
        .on(ChildReport::new(
            trigger.proxy,
            ProxyOutcome::WorkerStopped {
                worker: trigger.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the exact middle-role stop selects one recovery"));
    let request = match actions
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one coordinated preparation request is emitted")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts an uninterpreted request"),
    };
    (fixed, request, members)
}

fn complete_coordinated_preparation(
    request: PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
) -> WorkerPreparation<SearchWorkshop, SearchRole, SearchWorker, SearchActivation> {
    prepare_selected_workers(request, &SEARCH_ROLES)
}

fn prepare_selected_workers(
    request: PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
    selected_roles: &[SearchRole],
) -> WorkerPreparation<SearchWorkshop, SearchRole, SearchWorker, SearchActivation> {
    let (role, remaining_roles) = selected_roles
        .split_first()
        .expect("every admitted recovery selects at least one role");
    let preparation = request.accept(WorkerSubmission::activated(
        SearchWorker(*role),
        SearchActivation,
    ));
    match (remaining_roles, preparation) {
        ([], ControlFlow::Break(preparation)) => preparation,
        ([_, ..], ControlFlow::Continue(request)) => {
            prepare_remaining_workers(request, remaining_roles)
        }
        ([], ControlFlow::Continue(_)) => {
            panic!("the final selected role completes preparation")
        }
        ([_, ..], ControlFlow::Break(_)) => {
            panic!("preparation cannot finish before every selected role")
        }
    }
}

fn prepare_remaining_workers(
    request: PendingWorkerPreparation<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
    selected_roles: &[SearchRole],
) -> WorkerPreparation<SearchWorkshop, SearchRole, SearchWorker, SearchActivation> {
    let (role, remaining_roles) = selected_roles
        .split_first()
        .expect("a pending preparation has another selected role");
    let preparation = request.accept(WorkerSubmission::activated(
        SearchWorker(*role),
        SearchActivation,
    ));
    match (remaining_roles, preparation) {
        ([], ControlFlow::Break(preparation)) => preparation,
        ([_, ..], ControlFlow::Continue(request)) => {
            prepare_remaining_workers(request, remaining_roles)
        }
        ([], ControlFlow::Continue(_)) => {
            panic!("the final selected role completes preparation")
        }
        ([_, ..], ControlFlow::Break(_)) => {
            panic!("preparation cannot finish before every selected role")
        }
    }
}

#[derive(Clone, Copy)]
enum CoordinatedShutdownArrival {
    Preparation,
    ProxyOperation(SearchRole),
    ProxyExit(SearchRole),
}

fn permutations<T, const N: usize>(mut values: [T; N]) -> Vec<[T; N]>
where
    T: Copy,
{
    fn permute<T, const N: usize>(values: &mut [T; N], next: usize, permutations: &mut Vec<[T; N]>)
    where
        T: Copy,
    {
        if next == values.len() {
            permutations.push(*values);
            return;
        }

        for selected in next..values.len() {
            values.swap(next, selected);
            permute(values, next + 1, permutations);
            values.swap(next, selected);
        }
    }

    let mut permutations = Vec::new();
    permute(&mut values, 0, &mut permutations);
    permutations
}

fn coordinated_shutdown_orders() -> Vec<[CoordinatedShutdownArrival; 7]> {
    permutations([
        CoordinatedShutdownArrival::Preparation,
        CoordinatedShutdownArrival::ProxyOperation(SearchRole::Search),
        CoordinatedShutdownArrival::ProxyExit(SearchRole::Search),
        CoordinatedShutdownArrival::ProxyOperation(SearchRole::Index),
        CoordinatedShutdownArrival::ProxyExit(SearchRole::Index),
        CoordinatedShutdownArrival::ProxyOperation(SearchRole::Spellcheck),
        CoordinatedShutdownArrival::ProxyExit(SearchRole::Spellcheck),
    ])
}

#[tokio::test]
async fn coordinated_preparation_shutdown_accepts_every_arrival_order() {
    let orders = coordinated_shutdown_orders();
    assert_eq!(orders.len(), 5_040);

    for arrivals in orders {
        let (mut fixed, request, members) =
            coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
        let mut preparation = Some(complete_coordinated_preparation(request));
        let shutdown = fixed
            .receive(RuntimeAddr(991), FixedCommand::shutdown())
            .unwrap_or_else(|_| panic!("shutdown adopts every coordinated obligation"));
        let operations = shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .into_iter()
            .map(|operation| match operation {
                SettledItem::Unattempted(operation) => Some(operation),
                SettledItem::Attempted(_) => {
                    panic!("the test intercepts each proxy shutdown before execution")
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 3);
        for (operation, member) in operations.iter().zip(&members) {
            assert_eq!(
                operation.as_ref().map(ProxyOperation::creation),
                Some(member.proxy)
            );
        }
        let roles = [
            SearchRole::Search,
            SearchRole::Index,
            SearchRole::Spellcheck,
        ];
        let proxies = roles
            .into_iter()
            .zip(members.iter().map(|member| member.proxy))
            .collect::<BTreeMap<_, _>>();
        let mut operations = roles
            .into_iter()
            .zip(operations)
            .collect::<BTreeMap<_, _>>();

        for (arrival_position, arrival) in arrivals.into_iter().enumerate() {
            let actions = match arrival {
                CoordinatedShutdownArrival::Preparation => {
                    fixed.transition(FixedSupervisorEvent::WorkerPreparationSettled(
                        SettledItem::Attempted(ItemSettlement::Accepted(
                            preparation
                                .take()
                                .expect("the preparation returns exactly once"),
                        )),
                    ))
                }
                CoordinatedShutdownArrival::ProxyOperation(role) => {
                    let (route, control, operation) = operations
                        .get_mut(&role)
                        .and_then(Option::take)
                        .expect("each role's proxy shutdown returns exactly once")
                        .into_parts();
                    drop(control);
                    fixed.transition(FixedSupervisorEvent::ProxyInputSettled(
                        SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
                            route,
                            EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(
                                Endpoint(992),
                            ),
                            operation,
                        ))),
                    ))
                }
                CoordinatedShutdownArrival::ProxyExit(role) => {
                    let proxy = proxies
                        .get(&role)
                        .copied()
                        .expect("every declared role owns one stable proxy");
                    fixed.transition(FixedSupervisorEvent::ProxyStopped(ChildStopped::new(
                        proxy,
                        Ok(Exit::Normal),
                        Instant::now(),
                    )))
                }
            }
            .unwrap_or_else(|_| panic!("every exact shutdown arrival remains admissible"));

            assert!(actions.sends.proxy_observations.is_empty());
            assert!(actions.sends.worker_preparations.is_empty());
            assert!(actions.sends.proxy_operations.is_empty());
            assert!(actions.sends.restart_schedules.is_empty());
            assert!(actions.sends.lifecycle.is_empty());
            assert!(actions.sends.diagnostics.is_empty());
            assert!(actions.sends.status_replies.into_deliveries().is_empty());
            assert!(
                actions
                    .sends
                    .capability_replies
                    .into_deliveries()
                    .is_empty()
            );
            if arrival_position + 1 == arrivals.len() {
                assert!(matches!(actions.become_, Step::Stop(_)));
            } else {
                assert!(matches!(actions.become_, Step::Continue));
            }
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ReplacementArrival {
    ProxyReceipt(SearchRole),
    WorkerReady(SearchRole),
}

fn search_role_orders() -> Vec<[SearchRole; 3]> {
    permutations(SEARCH_ROLES)
}

fn lawful_replacement_orders() -> Vec<[ReplacementArrival; 6]> {
    permutations([
        ReplacementArrival::ProxyReceipt(SearchRole::Search),
        ReplacementArrival::WorkerReady(SearchRole::Search),
        ReplacementArrival::ProxyReceipt(SearchRole::Index),
        ReplacementArrival::WorkerReady(SearchRole::Index),
        ReplacementArrival::ProxyReceipt(SearchRole::Spellcheck),
        ReplacementArrival::WorkerReady(SearchRole::Spellcheck),
    ])
    .into_iter()
    .filter(|arrivals| {
        SEARCH_ROLES.into_iter().all(|role| {
            let receipt = arrivals
                .iter()
                .position(|arrival| *arrival == ReplacementArrival::ProxyReceipt(role))
                .expect("each role has one proxy receipt");
            let ready = arrivals
                .iter()
                .position(|arrival| *arrival == ReplacementArrival::WorkerReady(role))
                .expect("each role has one worker-ready report");
            receipt < ready
        })
    })
    .collect()
}

#[tokio::test]
async fn rest_for_one_classifies_every_distinct_second_worker_stop() {
    for first_role in SEARCH_ROLES {
        let selected_roles = expected_recovery_roles(Strategy::RestForOne, first_role);
        for second_role in SEARCH_ROLES.into_iter().filter(|role| *role != first_role) {
            let (mut fixed, members) =
                ready_three_role_roster(Strategy::RestForOne, THREE_WORKERS).await;
            let members = SEARCH_ROLES
                .into_iter()
                .zip(&members)
                .collect::<BTreeMap<_, _>>();
            let first_member = members
                .get(&first_role)
                .copied()
                .expect("the first stopped role belongs to the roster");
            let preparation = begin_recovery(&mut fixed, first_member);
            let preparation = prepare_selected_workers(preparation, &selected_roles);
            let admitted = fixed
                .transition(FixedSupervisorEvent::WorkerPreparationSettled(
                    SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
                ))
                .unwrap_or_else(|_| panic!("the first recovery is admitted atomically"));
            let replacements = admitted.sends.proxy_operations.unattempted().into_inputs();
            assert_eq!(replacements.len(), selected_roles.len());
            assert!(admitted.sends.worker_preparations.is_empty());
            assert!(admitted.sends.restart_schedules.is_empty());
            assert!(admitted.sends.lifecycle.is_empty());
            assert!(admitted.sends.diagnostics.is_empty());
            let second_member = members
                .get(&second_role)
                .copied()
                .expect("the second stopped role belongs to the roster");
            let second_stop = ChildStopped::new(
                second_member.worker.creation(),
                Ok(Exit::Normal),
                Instant::now(),
            );
            let actions = fixed
                .on(ChildReport::new(
                    second_member.proxy,
                    ProxyOutcome::WorkerStopped {
                        worker: second_member.worker.clone(),
                        stopped: second_stop,
                    },
                ))
                .unwrap_or_else(|_| panic!("every exact second worker stop is classified"));

            assert!(actions.creates.is_empty());
            assert!(actions.sends.proxy_observations.is_empty());
            assert!(actions.sends.worker_preparations.is_empty());
            assert!(actions.sends.proxy_operations.is_empty());
            assert!(actions.sends.restart_schedules.is_empty());
            assert!(actions.sends.lifecycle.is_empty());
            assert!(actions.sends.status_replies.into_deliveries().is_empty());
            assert!(
                actions
                    .sends
                    .capability_replies
                    .into_deliveries()
                    .is_empty()
            );

            if selected_roles.contains(&second_role) {
                assert!(actions.sends.diagnostics.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
            } else {
                let returned = match terminal_unexpected(actions.sends.diagnostics.into_requests())
                {
                    FixedSupervisorEvent::ProxyReported(returned) => returned,
                    _ => panic!("an intersecting recovery returns the second stop"),
                };
                assert_eq!(returned.child, second_member.proxy);
                match returned.report {
                    ProxyOutcome::WorkerStopped { worker, stopped } => {
                        assert_eq!(worker, second_member.worker);
                        assert_eq!(stopped, second_stop);
                    }
                    ProxyOutcome::Initial { .. }
                    | ProxyOutcome::Replacement { .. }
                    | ProxyOutcome::Unavailable { .. } => {
                        panic!("overlap preserves the worker-stop report")
                    }
                }
                assert!(matches!(actions.become_, Step::Stop(_)));
            }
            drop(replacements);
        }
    }
}

#[tokio::test]
async fn three_disjoint_recoveries_keep_exact_correlation_in_every_lawful_order() {
    let role_orders = search_role_orders();
    let replacement_orders = lawful_replacement_orders();
    assert_eq!(role_orders.len(), 6);
    assert_eq!(replacement_orders.len(), 90);

    for recovery_order in &role_orders {
        for replacement_order in &replacement_orders {
            let (mut fixed, members) =
                ready_three_role_roster(Strategy::OneForOne, THREE_WORKERS).await;
            let (_, successor_members) =
                ready_three_role_roster(Strategy::OneForOne, THREE_WORKERS).await;
            let members = SEARCH_ROLES
                .into_iter()
                .zip(&members)
                .collect::<BTreeMap<_, _>>();
            let mut successors = SEARCH_ROLES
                .into_iter()
                .zip(
                    successor_members
                        .into_iter()
                        .map(|member| Some(member.worker)),
                )
                .collect::<BTreeMap<_, _>>();
            let mut operations = BTreeMap::new();
            for role in recovery_order {
                let member = members
                    .get(role)
                    .copied()
                    .expect("every recovery starts from its declared role");
                let mut request = begin_recovery(&mut fixed, member);
                assert_eq!(request.source_and_role().1, role);
                let preparation = complete_one_for_one_preparation(request, *role);
                let actions = fixed
                    .transition(FixedSupervisorEvent::WorkerPreparationSettled(
                        SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
                    ))
                    .unwrap_or_else(|_| panic!("every disjoint preparation remains exact"));
                let mut emitted = actions.sends.proxy_operations.unattempted().into_inputs();
                let operation = match emitted
                    .pop()
                    .expect("each prepared role emits one replacement")
                {
                    SettledItem::Unattempted(operation) => operation,
                    SettledItem::Attempted(_) => {
                        panic!("the test intercepts each replacement before execution")
                    }
                };
                assert!(emitted.is_empty());
                assert_eq!(
                    operation.creation(),
                    members
                        .get(role)
                        .expect("the prepared role belongs to the roster")
                        .proxy
                );
                assert!(operations.insert(*role, Some(operation)).is_none());
                assert!(actions.creates.is_empty());
                assert!(actions.sends.proxy_observations.is_empty());
                assert!(actions.sends.worker_preparations.is_empty());
                assert!(actions.sends.restart_schedules.is_empty());
                assert!(actions.sends.lifecycle.is_empty());
                assert!(actions.sends.diagnostics.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
            }

            let mut predecessors = SEARCH_ROLES
                .into_iter()
                .map(|role| {
                    let worker = members
                        .get(&role)
                        .expect("every recovering role retains its predecessor")
                        .worker
                        .clone();
                    (role, Some(worker))
                })
                .collect::<BTreeMap<_, _>>();
            let mut ready_roles = BTreeSet::new();

            for arrival in replacement_order {
                let role = match arrival {
                    ReplacementArrival::ProxyReceipt(role)
                    | ReplacementArrival::WorkerReady(role) => *role,
                };
                let actions =
                    match arrival {
                        ReplacementArrival::ProxyReceipt(role) => {
                            let (route, control, operation) = operations
                                .get_mut(role)
                                .and_then(Option::take)
                                .expect("each proxy receipt returns exactly once")
                                .into_parts();
                            drop(control);
                            fixed.transition(FixedSupervisorEvent::ProxyInputSettled(
                                SettledItem::Attempted(ItemSettlement::Accepted(
                                    ProxyInputReceipt::new(
                                        route,
                                        EstablishedActor::<
                                            StableProxy<SearchWorker, SearchActivation>,
                                        >::issued(Endpoint(
                                            996,
                                        )),
                                        operation,
                                    ),
                                )),
                            ))
                        }
                        ReplacementArrival::WorkerReady(role) => {
                            let member = members
                                .get(role)
                                .expect("the ready report belongs to one stable proxy");
                            let predecessor = predecessors
                                .get_mut(role)
                                .and_then(Option::take)
                                .expect("each predecessor is replaced exactly once");
                            let successor = successors
                                .get_mut(role)
                                .and_then(Option::take)
                                .expect("each successor becomes ready exactly once");
                            fixed.transition(FixedSupervisorEvent::ProxyReported(ChildReport::new(
                                member.proxy,
                                ProxyOutcome::Replacement {
                                    outcome: ReplacementOutcome::Resolved {
                                        replaces: predecessor,
                                        result: WorkerStartResult::Ready {
                                            attempt: successor,
                                            readiness: (),
                                        },
                                    },
                                },
                            )))
                        }
                    }
                    .unwrap_or_else(|_| panic!("every exact disjoint return remains admissible"));

                assert!(actions.creates.is_empty());
                assert!(actions.sends.proxy_observations.is_empty());
                assert!(actions.sends.worker_preparations.is_empty());
                assert!(actions.sends.proxy_operations.is_empty());
                assert!(actions.sends.restart_schedules.is_empty());
                assert!(actions.sends.diagnostics.is_empty());
                assert!(actions.sends.status_replies.into_deliveries().is_empty());
                assert!(
                    actions
                        .sends
                        .capability_replies
                        .into_deliveries()
                        .is_empty()
                );
                assert!(matches!(actions.become_, Step::Continue));

                match arrival {
                    ReplacementArrival::ProxyReceipt(_) => {
                        assert!(actions.sends.lifecycle.is_empty());
                    }
                    ReplacementArrival::WorkerReady(_) => {
                        assert_eq!(actions.sends.lifecycle.len(), 2);
                        assert!(matches!(
                            actions.sends.lifecycle[0].message.event(),
                            FixedLifecycleEvent::WorkerStoppedAfterAdmission {
                                role: lifecycle_role,
                                ..
                            } if *lifecycle_role == role
                        ));
                        assert!(matches!(
                            actions.sends.lifecycle[1].message.event(),
                            FixedLifecycleEvent::Restarted {
                                role: lifecycle_role,
                                ..
                            } if *lifecycle_role == role
                        ));
                        assert!(ready_roles.insert(role));
                    }
                }

                for queried_role in SEARCH_ROLES {
                    let capability = fixed
                        .receive(
                            RuntimeAddr(997),
                            FixedCommand::capability(
                                queried_role,
                                Recipient::global(RuntimeAddr(998)),
                            ),
                        )
                        .unwrap_or_else(|_| {
                            panic!("each recovering role remains independently queryable")
                        });
                    let capability =
                        logical_reply(capability.sends.capability_replies.into_deliveries());
                    if ready_roles.contains(&queried_role) {
                        assert!(matches!(
                            capability,
                            CapabilityResult::Ready { role, .. } if role == queried_role
                        ));
                    } else {
                        assert!(matches!(
                            capability,
                            CapabilityResult::Unavailable {
                                role,
                                phase: atomic::UnavailablePhase::Recovering,
                            } if role == queried_role
                        ));
                    }
                }
            }

            assert_eq!(ready_roles, SEARCH_ROLES.into_iter().collect());
            assert!(operations.values().all(Option::is_none));
            assert!(predecessors.values().all(Option::is_none));
            assert!(successors.values().all(Option::is_none));
        }
    }
}

async fn admitted_one_for_all(
    maximum: NonZeroUsize,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    Vec<ReadyMember>,
    Vec<ProxyOperation<Here, SearchWorker, SearchActivation>>,
) {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::OneForAll, maximum, 1).await;
    let preparation = complete_coordinated_preparation(request);
    let admitted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the coordinated recovery is admitted"));
    let operations = admitted
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|operation| match operation {
            SettledItem::Unattempted(operation) => operation,
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted replacements")
            }
        })
        .collect();
    (fixed, members, operations)
}

async fn one_for_all_with_dispatched_initial_peers() -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
    ReadyMember,
    Vec<ProxyOperation<Here, SearchWorker, SearchActivation>>,
) {
    let (mut fixed, mut operations) =
        three_role_initial_operations(Strategy::OneForAll, THREE_WORKERS);
    let search = accept_initial_operation(&mut fixed, operations.remove(0))
        .await
        .0;
    let worker_stop = ChildStopped::new(search.worker.creation(), Ok(Exit::Normal), Instant::now());
    let selected = fixed
        .on(ChildReport::new(
            search.proxy,
            ProxyOutcome::WorkerStopped {
                worker: search.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("dispatched initial peers are eligible for coordination"));
    assert!(selected.creates.is_empty());
    assert!(selected.sends.proxy_observations.is_empty());
    assert!(
        selected
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(selected.sends.diagnostics.is_empty());
    let request = match selected
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one coordinated preparation is emitted")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts an uninterpreted request"),
    };
    (fixed, request, search, operations)
}

fn begin_recovery<Source, DiagnosticRoute>(
    fixed: &mut Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    member: &ReadyMember,
) -> PrepareWorkers<Source, SearchRole, SearchWorker, SearchActivation>
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let worker_stop = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), Instant::now());
    let selected = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the exact stop starts one disjoint recovery"));
    assert!(selected.creates.is_empty());
    assert!(selected.sends.proxy_observations.is_empty());
    assert!(
        selected
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(selected.sends.lifecycle.is_empty());
    assert!(selected.sends.diagnostics.is_empty());
    assert!(matches!(selected.become_, Step::Continue));
    match selected
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one worker preparation is emitted")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts an uninterpreted request"),
    }
}

fn expected_recovery_roles(strategy: Strategy, trigger: SearchRole) -> Vec<SearchRole> {
    match strategy {
        Strategy::OneForOne => vec![trigger],
        Strategy::OneForAll => vec![
            SearchRole::Search,
            SearchRole::Index,
            SearchRole::Spellcheck,
        ],
        Strategy::RestForOne => match trigger {
            SearchRole::Search => vec![
                SearchRole::Search,
                SearchRole::Index,
                SearchRole::Spellcheck,
            ],
            SearchRole::Index => vec![SearchRole::Index, SearchRole::Spellcheck],
            SearchRole::Spellcheck => vec![SearchRole::Spellcheck],
        },
    }
}

const fn roster_position(role: SearchRole) -> usize {
    match role {
        SearchRole::Search => 0,
        SearchRole::Index => 1,
        SearchRole::Spellcheck => 2,
    }
}

fn reject_selected_worker(
    mut preparation: PrepareWorkers<FallibleWorkshop, SearchRole, SearchWorker, SearchActivation>,
    selected_roles: &[SearchRole],
    rejected_role: SearchRole,
) -> WorkerPreparation<FallibleWorkshop, SearchRole, SearchWorker, SearchActivation> {
    let rejected_position = selected_roles
        .iter()
        .position(|role| *role == rejected_role)
        .expect("the rejected role belongs to this recovery selection");
    assert_eq!(preparation.source_and_role().1, &selected_roles[0]);
    if rejected_position == 0 {
        return preparation.reject(WorkshopRejection::WorkerUnavailable);
    }

    let mut preparation = match preparation.accept(WorkerSubmission::activated(
        worker(&selected_roles[0]),
        SearchActivation,
    )) {
        ControlFlow::Continue(preparation) => preparation,
        ControlFlow::Break(_) => panic!("the selected recovery has another role"),
    };
    assert_eq!(preparation.source_and_role().1, &selected_roles[1]);
    if rejected_position == 1 {
        return preparation.reject(WorkshopRejection::WorkerUnavailable);
    }

    let mut preparation = match preparation.accept(WorkerSubmission::activated(
        worker(&selected_roles[1]),
        SearchActivation,
    )) {
        ControlFlow::Continue(preparation) => preparation,
        ControlFlow::Break(_) => panic!("the selected recovery has a third role"),
    };
    assert_eq!(preparation.source_and_role().1, &selected_roles[2]);
    assert_eq!(rejected_position, 2);
    preparation.reject(WorkshopRejection::WorkerUnavailable)
}

fn complete_one_for_one_preparation(
    request: PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
    role: SearchRole,
) -> WorkerPreparation<SearchWorkshop, SearchRole, SearchWorker, SearchActivation> {
    prepare_selected_workers(request, core::slice::from_ref(&role))
}

#[tokio::test]
async fn one_for_all_prepares_every_role_in_declaration_order() {
    let (_, mut request, _) = coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    assert_eq!(request.source_and_role().1, &SearchRole::Search);
    let mut request = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Continue(request) => request,
        ControlFlow::Break(_) => {
            panic!("two selected roles remain")
        }
    };
    assert_eq!(request.source_and_role().1, &SearchRole::Index);
    let mut request = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Index),
        SearchActivation,
    )) {
        ControlFlow::Continue(request) => request,
        ControlFlow::Break(_) => {
            panic!("one selected role remains")
        }
    };
    assert_eq!(request.source_and_role().1, &SearchRole::Spellcheck);
    match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Spellcheck),
        SearchActivation,
    )) {
        ControlFlow::Break(_) => {}
        ControlFlow::Continue(_) => {
            panic!("the final selected role completes preparation")
        }
    }
}

#[tokio::test]
async fn every_strategy_selects_each_declared_trigger_range() {
    let roster = [
        SearchRole::Search,
        SearchRole::Index,
        SearchRole::Spellcheck,
    ];
    for strategy in [
        Strategy::OneForOne,
        Strategy::OneForAll,
        Strategy::RestForOne,
    ] {
        for trigger in 0..roster.len() {
            let (_, mut request, _) =
                coordinated_preparation(strategy, THREE_WORKERS, trigger).await;
            let expected: &[SearchRole] = match strategy {
                Strategy::OneForOne => &roster[trigger..=trigger],
                Strategy::OneForAll => &roster,
                Strategy::RestForOne => &roster[trigger..],
            };
            let first = expected.first().expect("every strategy selects a trigger");
            assert_eq!(request.source_and_role().1, first);
            match request.accept(WorkerSubmission::activated(worker(first), SearchActivation)) {
                ControlFlow::Break(_) => assert_eq!(expected.len(), 1),
                ControlFlow::Continue(request) => {
                    assert_ne!(expected.len(), 1);
                    let mut pending = Some(request);
                    for (selected, expected_role) in expected[1..].iter().enumerate() {
                        let mut request = pending
                            .take()
                            .expect("every remaining role has one pending preparation");
                        assert_eq!(request.source_and_role().1, expected_role);
                        match request.accept(WorkerSubmission::activated(
                            worker(expected_role),
                            SearchActivation,
                        )) {
                            ControlFlow::Continue(request) => {
                                assert_ne!(selected + 2, expected.len());
                                pending = Some(request);
                            }
                            ControlFlow::Break(_) => assert_eq!(selected + 2, expected.len()),
                        }
                    }
                    assert!(pending.is_none());
                }
            }
        }
    }
}

#[tokio::test]
async fn one_for_all_includes_dispatched_initial_peers_without_marking_them_ready() {
    let (mut fixed, request, search, operations) =
        one_for_all_with_dispatched_initial_peers().await;
    let pending_proxy_ids = operations
        .iter()
        .map(ProxyOperation::creation)
        .map(CreationId::get)
        .collect::<Vec<_>>();
    let mut request = request;
    assert_eq!(request.source_and_role().1, &SearchRole::Search);
    let mut request = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Continue(request) => request,
        ControlFlow::Break(_) => {
            panic!("two initial peers remain selected")
        }
    };
    assert_eq!(request.source_and_role().1, &SearchRole::Index);
    let mut request = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Index),
        SearchActivation,
    )) {
        ControlFlow::Continue(request) => request,
        ControlFlow::Break(_) => {
            panic!("one initial peer remains selected")
        }
    };
    assert_eq!(request.source_and_role().1, &SearchRole::Spellcheck);
    assert_eq!(pending_proxy_ids, [2, 3]);
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Spellcheck),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("the final initial peer completes preparation")
        }
    };
    let accepted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("prepared initial peers retain their exact startup state"));
    let replacement_proxy_ids = accepted
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.creation().get(),
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted replacements")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(replacement_proxy_ids, [search.proxy.get()]);
    let shutdown = fixed
        .receive(RuntimeAddr(944), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains initial operations and prepared workers"));
    assert_eq!(
        shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        3
    );
}

#[tokio::test]
async fn preparing_recovery_maps_a_dispatched_peer_only_from_its_proxy() {
    let (mut fixed, mut request, search, operations) =
        one_for_all_with_dispatched_initial_peers().await;
    assert_eq!(request.source_and_role().1, &SearchRole::Search);
    assert_eq!(operations.len(), 2);
    let pending_proxy = operations[0].creation();
    let unavailable = fixed
        .on(ChildReport::new(
            pending_proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(901),
                phase: ProxyPhase::Activating,
                command: SearchCommand::Find("pending initial worker".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the pending peer retains its stable proxy"));
    assert!(matches!(unavailable.become_, Step::Continue));
    assert!(unavailable.sends.diagnostics.is_empty());
    assert_eq!(unavailable.sends.lifecycle.len(), 1);
    match unavailable.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::Unavailable {
            role,
            sender,
            phase,
            command,
        } => {
            assert_eq!(role, &SearchRole::Index);
            assert_eq!(sender, &RuntimeAddr(901));
            assert_eq!(phase, ProxyPhase::Activating);
            assert_eq!(
                command,
                &SearchCommand::Find("pending initial worker".to_owned())
            );
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("the pending peer keeps the unavailable lifecycle event")
        }
    }

    let mut sequence = CreationSequence::new();
    let candidates = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
        sequence.issue().expect("the sequence has a third ID"),
        sequence.issue().expect("the sequence has a fourth ID"),
    ];
    let foreign = candidates
        .into_iter()
        .find(|candidate| {
            *candidate != search.proxy
                && operations
                    .iter()
                    .all(|operation| operation.creation() != *candidate)
        })
        .expect("four distinct IDs include one outside the three-proxy roster");
    let unexpected = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(902),
                phase: ProxyPhase::Activating,
                command: SearchCommand::Find("outside roster".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the unowned child report becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete unowned unavailable report returns unchanged"),
    };
    assert_eq!(returned.child, foreign);
    match returned.report {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(902));
            assert_eq!(phase, ProxyPhase::Activating);
            assert_eq!(command, SearchCommand::Find("outside roster".to_owned()));
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the unowned report keeps the unavailable command")
        }
    }
}

#[tokio::test]
async fn preparing_recovery_maps_an_online_peer_only_from_its_proxy() {
    let (mut fixed, mut request, members) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    assert_eq!(request.source_and_role().1, &SearchRole::Search);
    let unavailable = fixed
        .on(ChildReport::new(
            members[0].proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(903),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("online recovery peer".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the online peer retains its stable proxy"));
    assert!(matches!(unavailable.become_, Step::Continue));
    assert!(unavailable.sends.diagnostics.is_empty());
    assert_eq!(unavailable.sends.lifecycle.len(), 1);
    match unavailable.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::Unavailable {
            role,
            sender,
            phase,
            command,
        } => {
            assert_eq!(role, &SearchRole::Search);
            assert_eq!(sender, &RuntimeAddr(903));
            assert_eq!(phase, ProxyPhase::EmptyAfter);
            assert_eq!(
                command,
                &SearchCommand::Find("online recovery peer".to_owned())
            );
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("the online peer keeps the unavailable lifecycle event")
        }
    }

    let mut sequence = CreationSequence::new();
    let candidates = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
        sequence.issue().expect("the sequence has a third ID"),
        sequence.issue().expect("the sequence has a fourth ID"),
    ];
    let foreign = candidates
        .into_iter()
        .find(|candidate| members.iter().all(|member| member.proxy != *candidate))
        .expect("four distinct IDs include one outside the three-proxy roster");
    let unexpected = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(904),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("outside online roster".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the unowned child report becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete unowned unavailable report returns unchanged"),
    };
    assert_eq!(returned.child, foreign);
    match returned.report {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(904));
            assert_eq!(phase, ProxyPhase::EmptyAfter);
            assert_eq!(
                command,
                SearchCommand::Find("outside online roster".to_owned())
            );
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the unowned report keeps the unavailable command")
        }
    }
}

#[tokio::test]
async fn rest_for_one_prepares_the_trigger_and_declared_suffix() {
    let (_, mut request, _) = coordinated_preparation(Strategy::RestForOne, THREE_WORKERS, 1).await;
    assert_eq!(request.source_and_role().1, &SearchRole::Index);
    let mut request = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Index),
        SearchActivation,
    )) {
        ControlFlow::Continue(request) => request,
        ControlFlow::Break(_) => {
            panic!("one selected suffix role remains")
        }
    };
    assert_eq!(request.source_and_role().1, &SearchRole::Spellcheck);
    match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Spellcheck),
        SearchActivation,
    )) {
        ControlFlow::Break(_) => {}
        ControlFlow::Continue(_) => {
            panic!("the selected suffix completes preparation")
        }
    }
}

#[tokio::test]
async fn shutdown_drains_every_proxy_selected_for_coordinated_recovery_in_roster_order() {
    let (mut fixed, mut preparation, members) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    assert_eq!(preparation.source_and_role().1, &SearchRole::Search);

    let shutdown = fixed
        .receive(RuntimeAddr(941), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown adopts every selected proxy"));
    assert!(shutdown.creates.is_empty());
    assert!(shutdown.sends.proxy_observations.is_empty());
    assert!(
        shutdown
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(shutdown.sends.diagnostics.is_empty());
    let proxy_ids = shutdown
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.into_parts().0.get(),
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted shutdown operations")
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(
        proxy_ids,
        [
            members[0].proxy.get(),
            members[1].proxy.get(),
            members[2].proxy.get(),
        ]
    );
    assert_eq!(preparation.source_and_role().1, &SearchRole::Search);
}

#[tokio::test]
async fn coordinated_preparation_issues_ready_replacements_in_declaration_order() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    let preparation = complete_coordinated_preparation(request);
    let accepted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact coordinated result restores its worker source"));
    assert!(accepted.creates.is_empty());
    assert!(accepted.sends.proxy_observations.is_empty());
    assert!(
        accepted
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let replacement_proxy_ids = accepted
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.creation().get(),
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted replacements")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        replacement_proxy_ids,
        [
            members[0].proxy.get(),
            members[1].proxy.get(),
            members[2].proxy.get(),
        ]
    );
    assert!(accepted.sends.diagnostics.is_empty());
    assert!(matches!(accepted.become_, Step::Continue));

    let shutdown = fixed
        .receive(RuntimeAddr(942), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains every prepared participant"));
    assert_eq!(
        shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        3
    );
}

#[tokio::test]
async fn waiting_peer_stop_requires_its_proxy_and_worker() {
    let (mut exact_owner, exact_members, exact_operations) = admitted_one_for_all(ONE_WORKER).await;
    assert_eq!(exact_operations.len(), 1);
    assert_eq!(exact_operations[0].creation(), exact_members[0].proxy);
    let exact_worker = exact_members[2].worker.clone();
    let exact_stop = ChildStopped::new(exact_worker.creation(), Ok(Exit::Normal), Instant::now());
    let accepted = exact_owner
        .on(ChildReport::new(
            exact_members[2].proxy,
            ProxyOutcome::WorkerStopped {
                worker: exact_worker,
                stopped: exact_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the waiting peer accepts its exact stop"));
    assert!(matches!(accepted.become_, Step::Continue));
    assert!(accepted.sends.diagnostics.is_empty());
    assert!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    let (mut proxy_owner, proxy_members, proxy_operations) = admitted_one_for_all(ONE_WORKER).await;
    assert_eq!(proxy_operations.len(), 1);
    let proxy_worker = proxy_members[2].worker.clone();
    let proxy_stop = ChildStopped::new(proxy_worker.creation(), Ok(Exit::Normal), Instant::now());
    let wrong_proxy = proxy_owner
        .on(ChildReport::new(
            proxy_members[0].proxy,
            ProxyOutcome::WorkerStopped {
                worker: proxy_worker.clone(),
                stopped: proxy_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the wrong proxy returns one diagnostic"));
    let returned = match terminal_unexpected(wrong_proxy.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete wrong-proxy stop returns unchanged"),
    };
    assert_eq!(returned.child, proxy_members[0].proxy);
    match returned.report {
        ProxyOutcome::WorkerStopped { worker, stopped } => {
            assert_eq!(worker, proxy_worker);
            assert_eq!(stopped, proxy_stop);
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::Unavailable { .. } => {
            panic!("the returned report keeps the worker-stop input")
        }
    }

    let (mut worker_owner, worker_members, worker_operations) =
        admitted_one_for_all(ONE_WORKER).await;
    assert_eq!(worker_operations.len(), 1);
    let wrong_worker = worker_members[0].worker.clone();
    let worker_stop = ChildStopped::new(wrong_worker.creation(), Ok(Exit::Normal), Instant::now());
    let wrong_worker = worker_owner
        .on(ChildReport::new(
            worker_members[2].proxy,
            ProxyOutcome::WorkerStopped {
                worker: wrong_worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the wrong worker returns one diagnostic"));
    let returned = match terminal_unexpected(wrong_worker.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete wrong-worker stop returns unchanged"),
    };
    assert_eq!(returned.child, worker_members[2].proxy);
    match returned.report {
        ProxyOutcome::WorkerStopped { worker, stopped } => {
            assert_eq!(worker, worker_members[0].worker);
            assert_eq!(stopped, worker_stop);
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::Unavailable { .. } => {
            panic!("the returned report keeps the wrong worker")
        }
    }
}

#[tokio::test]
async fn coordinated_peer_restarts_when_replacement_outcome_precedes_worker_stop() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    let preparation = complete_coordinated_preparation(request);
    let issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the coordinated recovery is admitted"));
    let mut replacements = issued.sends.proxy_operations.unattempted().into_inputs();
    let replacement = match replacements.remove(0) {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the first replacement is still uninterpreted")
        }
    };
    let successor = ready_single_proxy_with_worker().await.1.worker;
    let predecessor = members[0].worker.clone();
    let (creation, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    811,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());

    let outcome_first = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: predecessor.clone(),
                    result: WorkerStartResult::Ready {
                        attempt: successor,
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact outcome waits for the predecessor stop"));
    assert!(outcome_first.sends.lifecycle.is_empty());

    let predecessor_stop = ChildStopped::new(
        members[0].worker.creation(),
        Ok(Exit::Normal),
        Instant::now(),
    );
    let completed = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::WorkerStopped {
                worker: predecessor,
                stopped: predecessor_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the exact predecessor stop completes the restart"));
    assert_eq!(completed.sends.lifecycle.len(), 2);
    assert!(matches!(
        completed.sends.lifecycle[0].message.event(),
        FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
    ));
    assert!(matches!(
        completed.sends.lifecycle[1].message.event(),
        FixedLifecycleEvent::Restarted { .. }
    ));

    let shutdown = fixed
        .receive(RuntimeAddr(943), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the declared roster"));
    let proxy_ids = shutdown
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.creation().get(),
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted shutdown operations")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        proxy_ids,
        [
            members[0].proxy.get(),
            members[1].proxy.get(),
            members[2].proxy.get(),
        ]
    );
}

#[tokio::test]
async fn replacement_outcome_releases_capacity_while_predecessor_stop_is_pending() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::OneForAll, ONE_WORKER, 1).await;
    let preparation = complete_coordinated_preparation(request);
    let issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the coordinated recovery is admitted"));
    let mut replacements = issued.sends.proxy_operations.unattempted().into_inputs();
    let replacement = match replacements
        .pop()
        .expect("one replacement occupies the single activation slot")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the first replacement is still uninterpreted")
        }
    };
    assert!(replacements.is_empty());
    assert_eq!(replacement.creation(), members[0].proxy);

    let successor = ready_single_proxy_with_worker().await.1.worker;
    let predecessor = members[0].worker.clone();
    let (creation, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    811,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt is accepted"));
    assert!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    let outcome_first = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: predecessor.clone(),
                    result: WorkerStartResult::Ready {
                        attempt: successor,
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact outcome releases its activation slot"));
    let next = match outcome_first
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the next declared replacement uses the released slot")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the next replacement is still uninterpreted")
        }
    };
    assert_eq!(next.creation(), members[1].proxy);
    assert!(outcome_first.sends.lifecycle.is_empty());

    let predecessor_stop = ChildStopped::new(
        members[0].worker.creation(),
        Ok(Exit::Normal),
        Instant::now(),
    );
    let completed = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::WorkerStopped {
                worker: predecessor,
                stopped: predecessor_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the predecessor stop completes the first restart"));
    assert_eq!(completed.sends.lifecycle.len(), 2);
}

#[tokio::test]
async fn coordinated_peer_restarts_when_worker_stop_precedes_replacement_outcome() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    let preparation = complete_coordinated_preparation(request);
    let issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the coordinated recovery is admitted"));
    let mut replacements = issued.sends.proxy_operations.unattempted().into_inputs();
    let replacement = match replacements.remove(0) {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the first replacement is still uninterpreted")
        }
    };
    let successor = ready_single_proxy_with_worker().await.1.worker;
    let predecessor = members[0].worker.clone();
    let (creation, control, operation) = replacement.into_parts();
    drop(control);
    let predecessor_stop = ChildStopped::new(
        members[0].worker.creation(),
        Ok(Exit::Normal),
        Instant::now(),
    );
    let stop_first = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::WorkerStopped {
                worker: predecessor.clone(),
                stopped: predecessor_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the exact predecessor stop waits for replacement"));
    assert!(stop_first.sends.lifecycle.is_empty());
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    811,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());

    let completed = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: predecessor,
                    result: WorkerStartResult::Ready {
                        attempt: successor,
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact replacement outcome completes the restart"));
    assert_eq!(completed.sends.lifecycle.len(), 2);
    assert!(matches!(
        completed.sends.lifecycle[0].message.event(),
        FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
    ));
    assert!(matches!(
        completed.sends.lifecycle[1].message.event(),
        FixedLifecycleEvent::Restarted { .. }
    ));
}

#[tokio::test]
async fn rest_for_one_rejects_returned_trigger_while_its_suffix_is_still_recovering() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::RestForOne, THREE_WORKERS, 1).await;
    let request = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Index),
        SearchActivation,
    )) {
        ControlFlow::Continue(request) => request,
        ControlFlow::Break(_) => {
            panic!("the suffix still needs one prepared worker")
        }
    };
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Spellcheck),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("the second suffix worker completes preparation")
        }
    };
    let issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the suffix recovery is admitted"));
    let mut replacements = issued.sends.proxy_operations.unattempted().into_inputs();
    let replacement = match replacements.remove(0) {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the trigger replacement is still uninterpreted")
        }
    };
    assert_eq!(replacement.creation(), members[1].proxy);
    assert_eq!(replacements.len(), 1);
    let successor = ready_single_proxy_with_worker().await.1.worker;
    let previous = members[1].worker.clone();
    let (creation, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    812,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the trigger replacement receipt is accepted"));
    assert!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let returned = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: previous,
                    result: WorkerStartResult::Ready {
                        attempt: successor.clone(),
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the trigger returns independently"));
    assert_eq!(returned.sends.lifecycle.len(), 2);

    let successor_stop = ChildStopped::new(successor.creation(), Ok(Exit::Normal), Instant::now());
    let unexpected = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::WorkerStopped {
                worker: successor,
                stopped: successor_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the overlapping stop becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(returned) => returned,
        _ => panic!("the recovering suffix prevents overlapping RestForOne admission"),
    };
    assert_eq!(returned.child, members[1].proxy);
    assert_eq!(replacements.len(), 1);
}

#[tokio::test]
async fn returned_rest_for_one_suffix_can_begin_disjoint_recovery() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::RestForOne, THREE_WORKERS, 1).await;
    let preparation =
        prepare_selected_workers(request, &[SearchRole::Index, SearchRole::Spellcheck]);
    let issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the suffix recovery is admitted"));
    let mut replacements = issued.sends.proxy_operations.unattempted().into_inputs();
    let index_replacement = match replacements.remove(0) {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the Index replacement is still uninterpreted")
        }
    };
    let spellcheck_replacement = match replacements.remove(0) {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the Spellcheck replacement is still uninterpreted")
        }
    };
    assert!(replacements.is_empty());
    assert_eq!(index_replacement.creation(), members[1].proxy);
    assert_eq!(spellcheck_replacement.creation(), members[2].proxy);

    let successor = ready_single_proxy_with_worker().await.1.worker;
    let predecessor = members[2].worker.clone();
    let (creation, control, operation) = spellcheck_replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    813,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the Spellcheck replacement receipt is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let stopped = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::WorkerStopped {
                worker: predecessor.clone(),
                stopped: ChildStopped::new(
                    predecessor.creation(),
                    Ok(Exit::Normal),
                    Instant::now(),
                ),
            },
        ))
        .unwrap_or_else(|_| panic!("the Spellcheck predecessor stop is retained"));
    assert!(stopped.sends.lifecycle.is_empty());
    let returned = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: predecessor,
                    result: WorkerStartResult::Ready {
                        attempt: successor.clone(),
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("Spellcheck returns while Index remains recovering"));
    assert_eq!(returned.sends.lifecycle.len(), 2);

    let successor = ReadyMember {
        proxy: creation,
        worker: successor,
    };
    let mut second = begin_recovery(&mut fixed, &successor);
    assert_eq!(second.source_and_role().1, &SearchRole::Spellcheck);
    match second.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Spellcheck),
        SearchActivation,
    )) {
        ControlFlow::Break(_) => {}
        ControlFlow::Continue(_) => {
            panic!("the disjoint suffix contains only Spellcheck")
        }
    }
    drop(index_replacement);
}

#[tokio::test]
async fn released_capacity_authorizes_waiting_recoveries_in_roster_order() {
    let (mut fixed, members) = ready_three_role_roster(Strategy::OneForOne, ONE_WORKER).await;

    let search = begin_recovery(&mut fixed, &members[0]);
    let search = complete_one_for_one_preparation(search, SearchRole::Search);
    let search_issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(search)),
        ))
        .unwrap_or_else(|_| panic!("the first recovery occupies the activation slot"));
    let search_operation = match search_issued
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the first replacement is issued")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the first replacement is still uninterpreted")
        }
    };
    assert_eq!(search_operation.creation(), members[0].proxy);

    let spellcheck = begin_recovery(&mut fixed, &members[2]);
    let spellcheck = complete_one_for_one_preparation(spellcheck, SearchRole::Spellcheck);
    let spellcheck_waiting = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(spellcheck)),
        ))
        .unwrap_or_else(|_| panic!("the later role waits for activation capacity"));
    assert!(
        spellcheck_waiting
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    let index = begin_recovery(&mut fixed, &members[1]);
    let index = complete_one_for_one_preparation(index, SearchRole::Index);
    let index_waiting = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(index)),
        ))
        .unwrap_or_else(|_| panic!("the earlier role also waits for activation capacity"));
    assert!(
        index_waiting
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    let waiting_unavailable = fixed
        .on(ChildReport::new(
            members[2].proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(809),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("waiting for activation capacity".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("a waiting replacement retains its live proxy"));
    assert!(matches!(waiting_unavailable.become_, Step::Continue));
    assert_eq!(waiting_unavailable.sends.lifecycle.len(), 1);
    match waiting_unavailable.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::Unavailable { role, .. } => {
            assert_eq!(role, &SearchRole::Spellcheck)
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("the waiting proxy keeps its unavailable meaning")
        }
    }

    let successor = ready_single_proxy_with_worker().await.1.worker;
    let previous = members[0].worker.clone();
    let (creation, control, operation) = search_operation.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    811,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the first replacement receipt is accepted"));
    assert!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    let released = fixed
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: previous,
                    result: WorkerStartResult::Ready {
                        attempt: successor,
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the first replacement releases capacity"));
    let next = match released
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one waiting replacement receives the released slot")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the next replacement is still uninterpreted")
        }
    };
    assert_eq!(next.creation(), members[1].proxy);
    assert_eq!(released.sends.lifecycle.len(), 2);
}

#[tokio::test]
async fn rest_for_one_returns_the_stop_when_its_suffix_is_already_recovering() {
    let (mut fixed, members) = ready_three_role_roster(Strategy::RestForOne, THREE_WORKERS).await;
    let spellcheck_stop = ChildStopped::new(
        members[2].worker.creation(),
        Ok(Exit::Normal),
        Instant::now(),
    );
    let first_recovery = fixed
        .on(ChildReport::new(
            members[2].proxy,
            ProxyOutcome::WorkerStopped {
                worker: members[2].worker.clone(),
                stopped: spellcheck_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the final role starts its own recovery"));
    assert_eq!(
        first_recovery
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );

    let index_stop = ChildStopped::new(
        members[1].worker.creation(),
        Ok(Exit::Normal),
        Instant::now(),
    );
    let unexpected = fixed
        .on(ChildReport::new(
            members[1].proxy,
            ProxyOutcome::WorkerStopped {
                worker: members[1].worker.clone(),
                stopped: index_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the overlapping stop becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(returned) => returned,
        _ => panic!("a suffix already in recovery rejects the complete choice"),
    };
    assert_eq!(returned.child, members[1].proxy);
    match returned.report {
        ProxyOutcome::WorkerStopped { worker, .. } => assert_eq!(worker, members[1].worker),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the returned stop keeps its exact kind"),
    }

    let shutdown = fixed
        .receive(RuntimeAddr(943), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("the unchanged roster still owns all three proxies"));
    assert_eq!(
        shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        3
    );
}

#[test]
fn shutdown_cancels_unemitted_initial_inputs_and_stops_proxies_in_roster_order() {
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits proxy births"));
    let mut fixed = initialized.behavior;
    let proxy_ids = initialized
        .actions
        .creates
        .iter()
        .map(|creation| creation.id())
        .collect::<Vec<_>>();
    let search_started = fixed
        .on(commit_proxy_births(initialized.actions.creates, 841))
        .unwrap_or_else(|_| panic!("the proxy batch authorizes search"));
    let initial_search = search_started
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("only search receives an initial input");
    let shutting_down = fixed
        .receive(RuntimeAddr(931), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("every committed proxy can begin shutdown"));
    assert!(matches!(shutting_down.become_, Step::Continue));
    let operations = shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs();
    assert_eq!(operations.len(), 3);

    for (settlement, expected_proxy) in operations.into_iter().zip(proxy_ids) {
        let operation = match settlement {
            SettledItem::Unattempted(operation) => operation,
            SettledItem::Attempted(_) => {
                panic!("shutdown emits uninterpreted proxy operations")
            }
        };
        let (creation, control, operation_id) = operation.into_parts();
        assert_eq!(creation, expected_proxy);
        let initialized = StableProxy::activated()
            .initialize()
            .unwrap_or_else(|_| panic!("proxy initialization is pure"));
        let mut proxy = initialized.behavior;
        let stopped = proxy
            .on(control)
            .unwrap_or_else(|_| panic!("each emitted operation is proxy shutdown"));
        assert!(matches!(stopped.become_, Step::Stop(_)));
        drop(operation_id);
    }
    drop(initial_search);
}

#[test]
fn shutdown_rejects_another_supervisors_initial_input_without_consuming_its_own() {
    let (mut owner, owner_initial) = single_proxy_dispatched(751);
    let (mut visitor, visitor_initial) = single_proxy_dispatched(752);
    let shutting_down = owner
        .receive(RuntimeAddr(932), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("owner begins shutdown"));
    let owner_shutdown = shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("owner emits one proxy shutdown");

    let unexpected = owner
        .on(SettledItem::Unattempted(visitor_initial))
        .unwrap_or_else(|_| panic!("the foreign initial input becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyInputSettled(settlement) => settlement,
        _ => panic!("foreign initial input cannot enter another shutdown"),
    };
    let visitor_retained = visitor
        .on(returned)
        .unwrap_or_else(|_| panic!("the source supervisor accepts its returned input"));
    assert!(matches!(visitor_retained.become_, Step::Continue));

    let (route, control, operation) = owner_initial.into_parts();
    drop(control);
    let owner_retained = owner
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    851,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("foreign rejection did not consume the owner's input"));
    assert!(matches!(owner_retained.become_, Step::Continue));
    drop(owner_shutdown);
}

#[test]
fn shutdown_retains_pending_proxy_creation_and_stops_an_exact_committed_proxy() {
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(SearchRole::Search, [])
            .unwrap_or_else(|_| panic!("the single role is unique")),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("the initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one proxy birth"));
    let mut fixed = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .expect("one proxy creation is emitted");
    let (proxy_id, proxy, _) = creation.into_parts();
    drop(proxy);

    let shutting_down = fixed
        .receive(RuntimeAddr(933), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains an unresolved proxy creation"));
    assert!(matches!(shutting_down.become_, Step::Continue));
    assert_eq!(
        shutting_down
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    let proxy_recipient = EstablishedRecipient::issued(Endpoint(862));
    let wrong_kind = CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    proxy_id,
                    CreationKind::replacement(proxy_id),
                    proxy_recipient.clone(),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ));
    let unexpected = fixed
        .on(wrong_kind)
        .unwrap_or_else(|_| panic!("the wrong creation kind becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyCreationsSettled(settlement) => settlement,
        _ => panic!("a replacement creation cannot satisfy the initial proxy birth"),
    };
    let CreationSettlement::Settled(settlements) = returned.into_settlement() else {
        panic!("the routed wrong-kind settlement remains routed")
    };
    let SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Established {
        established,
    })) = settlements
        .into_iter()
        .next()
        .expect("the returned batch has one proxy")
    else {
        panic!("the wrong-kind creation remains complete")
    };
    assert_eq!(established.id(), proxy_id);
    assert_eq!(established.kind(), CreationKind::replacement(proxy_id));

    let exact_birth = CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    proxy_id,
                    CreationKind::Birth,
                    proxy_recipient,
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ));
    let committed = fixed
        .on(exact_birth)
        .unwrap_or_else(|_| panic!("the exact commit starts proxy shutdown"));
    assert!(matches!(committed.become_, Step::Continue));
    let shutdown = match committed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("commit emits one proxy shutdown and no initial input")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let (creation, control, operation) = shutdown.into_parts();
    assert_eq!(creation, proxy_id);
    let initialized = StableProxy::activated()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let stopped_proxy = proxy
        .on(control)
        .unwrap_or_else(|_| panic!("the emitted operation is proxy shutdown"));
    assert!(matches!(stopped_proxy.become_, Step::Stop(_)));

    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                creation,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    861,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("shutdown settlement still awaits exact proxy exit"));
    assert!(matches!(accepted.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(
            proxy_id,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("exact proxy exit completes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn shutdown_retains_emitted_initial_input_and_its_late_outcome() {
    let (mut fixed, initial) = single_proxy_dispatched(731);
    let (initial_route, initial_control, initial_operation) = initial.into_parts();
    let initial_outcome = ready_proxy_outcome(initial_control).await;

    let shutting_down = fixed
        .receive(RuntimeAddr(930), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("committed startup accepts shutdown"));
    assert!(matches!(shutting_down.become_, Step::Continue));
    let proxy_shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("startup shutdown emits one distinct proxy operation")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };

    let initial_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                initial_route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    831,
                )),
                initial_operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the prior initial-input settlement remains admissible"));
    assert!(matches!(initial_settled.become_, Step::Continue));

    let wrong = ChildReport::new(
        initial_route,
        ProxyOutcome::Replacement {
            outcome: ReplacementOutcome::NotReplaceable {
                worker: SearchWorker(SearchRole::Search),
                activation: SearchActivation,
                phase: ProxyPhase::Ready,
            },
        },
    );
    let unexpected = fixed
        .on(wrong)
        .unwrap_or_else(|_| panic!("the wrong outcome kind becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("replacement outcome cannot satisfy retained initial startup"),
    };
    match returned.report {
        ProxyOutcome::Replacement {
            outcome: ReplacementOutcome::NotReplaceable { .. },
        } => {}
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the wrong-kind outcome remains complete"),
    }

    let outcome_retained = fixed
        .on(ChildReport::new(initial_route, initial_outcome))
        .unwrap_or_else(|_| panic!("the exact initial outcome remains admissible"));
    assert!(matches!(outcome_retained.become_, Step::Continue));

    let (shutdown_route, shutdown_control, shutdown_operation) = proxy_shutdown.into_parts();
    drop(shutdown_control);
    let shutdown_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                shutdown_route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    831,
                )),
                shutdown_operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the distinct shutdown settlement is accepted"));
    assert!(matches!(shutdown_settled.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(
            shutdown_route,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit completes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn ready_roster_shutdown_waits_for_operation_and_proxy_exit() {
    let (mut fixed, _) = ready_single_proxy_with_worker().await;

    let shutting_down = fixed
        .receive(RuntimeAddr(900), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("a ready roster accepts shutdown"));
    assert!(matches!(shutting_down.become_, Step::Continue));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one exact proxy shutdown is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };

    let repeated = fixed
        .receive(RuntimeAddr(901), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("repeated shutdown is idempotent"));
    assert!(matches!(repeated.become_, Step::Continue));
    assert_eq!(
        repeated
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );

    let (route, control, shutdown) = shutdown.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                shutdown,
            ),
        )))
        .unwrap_or_else(|_| panic!("shutdown acceptance alone keeps custody open"));
    assert!(matches!(accepted.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact proxy exit closes fleet shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn temporary_worker_stop_leaves_one_empty_member_with_a_live_proxy() {
    let (mut fixed, member) = ready_single_proxy_with_worker().await;
    let worker_stop = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), Instant::now());
    let left_empty = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("temporary policy accepts the exact worker stop"));
    assert!(matches!(left_empty.become_, Step::Continue));
    let NoSends = left_empty.sends.lifecycle;
    assert_eq!(
        left_empty
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );

    let unexpected = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the duplicate stop becomes a diagnostic"));
    let duplicate = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("an empty member cannot accept the same worker stop twice"),
    };
    match duplicate.report {
        ProxyOutcome::WorkerStopped {
            worker: returned_worker,
            stopped,
        } => {
            assert_eq!(returned_worker, member.worker);
            assert_eq!(stopped, worker_stop);
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the duplicate remains complete"),
    }

    let shutting_down = fixed
        .receive(RuntimeAddr(930), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("the empty member still owns its live proxy"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the live proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown result remains required"));
    assert!(matches!(accepted.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact proxy exit completes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn transient_normal_stop_leaves_one_empty_member_with_a_live_proxy() {
    let recovery = Recovery::transient(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, member) = ready_single_proxy_with_recovery(recovery).await;
    let worker_stop = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), Instant::now());
    let left_empty = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker,
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("transient policy accepts the exact normal stop"));
    assert!(matches!(left_empty.become_, Step::Continue));
    assert!(left_empty.creates.is_empty());
    assert!(left_empty.sends.proxy_observations.is_empty());
    assert!(
        left_empty
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(
        left_empty
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(
        left_empty
            .sends
            .restart_schedules
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let NoSends = left_empty.sends.lifecycle;
    assert!(left_empty.sends.status_replies.into_deliveries().is_empty());
    assert!(
        left_empty
            .sends
            .capability_replies
            .into_deliveries()
            .is_empty()
    );
    assert!(left_empty.sends.diagnostics.is_empty());

    let shutting_down = fixed
        .receive(RuntimeAddr(931), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("the empty transient member retains its live proxy"));
    let mut operations = shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs();
    let operation = match operations
        .pop()
        .expect("the retained live proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the test intercepts shutdown before execution"),
    };
    assert!(operations.is_empty());
    assert_eq!(operation.creation(), member.proxy);
}

#[tokio::test]
async fn configured_temporary_stop_publishes_exact_ineligible_lifecycle_once() {
    let (mut fixed, member) = ready_single_proxy_with_lifecycle().await;
    let stopped_at = Instant::now();
    let worker_stop = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), stopped_at);
    let left_empty = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("temporary policy accepts the exact worker stop"));
    assert!(matches!(left_empty.become_, Step::Continue));
    assert!(left_empty.creates.is_empty());
    assert!(left_empty.sends.proxy_observations.is_empty());
    assert!(
        left_empty
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(
        left_empty
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(
        left_empty
            .sends
            .restart_schedules
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(left_empty.sends.diagnostics.is_empty());
    assert_eq!(left_empty.sends.lifecycle.len(), 1);
    match left_empty.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::WorkerStoppedIneligible { role, stopped } => {
            assert_eq!(role, &SearchRole::Search);
            assert_eq!(stopped.child, member.worker.creation());
            assert_eq!(stopped.outcome, Ok(Exit::Normal));
            assert_eq!(stopped.at, stopped_at);
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::Unavailable { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("temporary stop publishes only WorkerStoppedIneligible")
        }
    }

    let unavailable = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(972),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("empty role".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the empty role still owns its live proxy"));
    assert_eq!(unavailable.sends.lifecycle.len(), 1);
    match unavailable.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::Unavailable { role, command, .. } => {
            assert_eq!(role, &SearchRole::Search);
            assert_eq!(command, &SearchCommand::Find("empty role".to_owned()));
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("the empty proxy report keeps its unavailable meaning")
        }
    }

    let duplicate_stop = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), stopped_at);
    let unexpected = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker,
                stopped: duplicate_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the duplicate stop becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the empty role rejects a duplicate stop"),
    };
    assert_eq!(returned.child, member.proxy);

    let shutdown = fixed
        .receive(RuntimeAddr(971), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("the empty role still owns its live proxy"));
    assert_eq!(shutdown.sends.lifecycle.len(), 0);
    assert_eq!(
        shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );
}

#[tokio::test]
async fn online_unavailable_command_requires_the_exact_proxy() {
    let (mut fixed, member) = ready_single_proxy_with_lifecycle().await;
    let mut sequence = CreationSequence::new();
    let foreign = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
    ]
    .into_iter()
    .find(|candidate| *candidate != member.proxy)
    .expect("two distinct IDs include one other than the live proxy");
    let unexpected = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(980),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("foreign actors".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the foreign report becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    assert!(unexpected.sends.lifecycle.is_empty());
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete foreign unavailable report returns unchanged"),
    };
    assert_eq!(returned.child, foreign);
    match returned.report {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(980));
            assert_eq!(phase, ProxyPhase::EmptyAfter);
            assert_eq!(command, SearchCommand::Find("foreign actors".to_owned()));
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the foreign report keeps its unavailable command")
        }
    }

    let unavailable = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(981),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("distributed actors".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the exact live proxy report is accepted"));

    assert!(matches!(unavailable.become_, Step::Continue));
    assert!(unavailable.sends.diagnostics.is_empty());
    assert_eq!(unavailable.sends.lifecycle.len(), 1);
    match unavailable.sends.lifecycle[0].message.event() {
        FixedLifecycleEvent::Unavailable {
            role,
            sender,
            phase,
            command,
        } => {
            assert_eq!(role, &SearchRole::Search);
            assert_eq!(sender, &RuntimeAddr(981));
            assert_eq!(phase, ProxyPhase::EmptyAfter);
            assert_eq!(
                command,
                &SearchCommand::Find("distributed actors".to_owned())
            );
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("the unavailable command keeps its lifecycle meaning")
        }
    }
}

#[tokio::test]
async fn exact_unavailable_command_uses_diagnostics_when_lifecycle_is_omitted() {
    let diagnostics = EstablishedRecipient::<SearchDiagnosticProtocol>::issued(Endpoint(982));
    let (mut fixed, member) = ready_single_proxy_with_policies(
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;
    let unavailable = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(983),
                phase: ProxyPhase::Activating,
                command: SearchCommand::Find("typed actors".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the exact live proxy report is accepted"));

    assert!(matches!(unavailable.become_, Step::Continue));
    assert!(
        unavailable
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let NoSends = unavailable.sends.lifecycle;
    let mut delivered = unavailable.sends.diagnostics.into_requests();
    assert_eq!(delivered.len(), 1);
    match delivered.remove(0) {
        atomic::DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(983));
            assert_eq!(unavailable.phase(), ProxyPhase::Activating);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("typed actors".to_owned())
            );
        }
        atomic::DiagnosticAction::Terminal { .. } => {
            panic!("a routed diagnostic cannot become terminal custody")
        }
        atomic::DiagnosticAction::Deliver { .. } => {
            panic!("the unavailable command keeps its diagnostic meaning")
        }
    }
}

#[test]
fn starting_roles_accept_unavailable_only_from_their_proxy() {
    let diagnostics = EstablishedRecipient::<SearchDiagnosticProtocol>::issued(Endpoint(992));
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(1).expect("one proxy input may remain outside the supervisor"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits three proxy births"));
    let mut fixed = initialized.behavior;
    let mut proxies = Vec::new();
    let births = initialized
        .actions
        .creates
        .into_iter()
        .enumerate()
        .map(|(position, creation)| {
            let (creation, proxy, kind) = creation.into_parts();
            drop(proxy);
            proxies.push(creation);
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        creation,
                        kind,
                        EstablishedRecipient::issued(Endpoint(1200 + position as u64)),
                    ),
                },
            ))
        })
        .collect();
    let started = fixed
        .on(CreationsSettled::new(CreationSettlement::Settled(births)))
        .unwrap_or_else(|_| panic!("the proxy batch starts its first declared role"));
    assert_eq!(proxies.len(), 3);
    let mut operations = started.sends.proxy_operations.unattempted().into_inputs();
    assert_eq!(operations.len(), 1);
    let initial = match operations
        .pop()
        .expect("Search receives the first proxy input")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts the uninterpreted proxy input")
        }
    };
    assert_eq!(initial.creation(), proxies[0]);

    let waiting = fixed
        .on(ChildReport::new(
            proxies[1],
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(993),
                phase: ProxyPhase::Dormant,
                command: SearchCommand::Find("waiting proxy".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("Index retains its proxy while awaiting authorization"));
    assert!(matches!(waiting.become_, Step::Continue));
    let mut delivered = waiting.sends.diagnostics.into_requests();
    match delivered.pop().expect("Index unavailability is published") {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Index);
            assert_eq!(unavailable.sender(), &RuntimeAddr(993));
            assert_eq!(unavailable.phase(), ProxyPhase::Dormant);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("waiting proxy".to_owned())
            );
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("the waiting role keeps its unavailable-command meaning")
        }
    }
    assert!(delivered.is_empty());

    let dispatched = fixed
        .on(ChildReport::new(
            proxies[0],
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(994),
                phase: ProxyPhase::Dormant,
                command: SearchCommand::Find("dispatched proxy".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("Search retains its proxy while input is unsettled"));
    assert!(matches!(dispatched.become_, Step::Continue));
    let mut delivered = dispatched.sends.diagnostics.into_requests();
    match delivered.pop().expect("Search unavailability is published") {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(994));
            assert_eq!(unavailable.phase(), ProxyPhase::Dormant);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("dispatched proxy".to_owned())
            );
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("the dispatched role keeps its unavailable-command meaning")
        }
    }
    assert!(delivered.is_empty());

    let mut sequence = CreationSequence::new();
    let foreign = core::iter::from_fn(|| sequence.issue())
        .take(proxies.len() + 1)
        .find(|creation| !proxies.contains(creation))
        .expect("the finite proxy roster leaves another creation available");
    let unexpected = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(995),
                phase: ProxyPhase::Activating,
                command: SearchCommand::Find("unowned proxy".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the unowned report becomes one diagnostic"));
    assert!(matches!(unexpected.become_, Step::Continue));
    let mut delivered = unexpected.sends.diagnostics.into_requests();
    match delivered.pop().expect("the unowned report is returned") {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyReported(report),
                },
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(report.child, foreign);
            match report.report {
                ProxyOutcome::Unavailable {
                    sender,
                    phase,
                    command,
                } => {
                    assert_eq!(sender, RuntimeAddr(995));
                    assert_eq!(phase, ProxyPhase::Activating);
                    assert_eq!(command, SearchCommand::Find("unowned proxy".to_owned()));
                }
                ProxyOutcome::Initial { .. }
                | ProxyOutcome::Replacement { .. }
                | ProxyOutcome::WorkerStopped { .. } => {
                    panic!("the returned report keeps its unavailable-command meaning")
                }
            }
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("an unowned proxy cannot identify a supervised role")
        }
    }
    assert!(delivered.is_empty());

    let (route, control, operation) = initial.into_parts();
    drop(control);
    let settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    996,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the unavailable report does not consume initial progress"));
    assert!(matches!(settled.become_, Step::Continue));

    let awaiting_outcome = fixed
        .on(ChildReport::new(
            proxies[0],
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(997),
                phase: ProxyPhase::Dormant,
                command: SearchCommand::Find("awaiting initial outcome".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the settled initial input still owns one live proxy"));
    assert!(matches!(awaiting_outcome.become_, Step::Continue));
    let mut delivered = awaiting_outcome.sends.diagnostics.into_requests();
    match delivered
        .pop()
        .expect("awaiting Search unavailability is published")
    {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(997));
            assert_eq!(unavailable.phase(), ProxyPhase::Dormant);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("awaiting initial outcome".to_owned())
            );
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("the awaiting role keeps its unavailable-command meaning")
        }
    }
    assert!(delivered.is_empty());
}

#[tokio::test]
async fn preparing_recovery_accepts_unavailable_from_its_live_proxy() {
    let diagnostics =
        EstablishedRecipient::<SearchRecoveryDiagnosticProtocol>::issued(Endpoint(995));
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, preparation, proxy, _) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::deliver_to(diagnostics),
    )
    .await;
    let unavailable = fixed
        .on(ChildReport::new(
            proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(996),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("during recovery".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("recovery retains the exact live StableProxy"));
    assert!(matches!(unavailable.become_, Step::Continue));
    assert_eq!(unavailable.sends.diagnostics.into_requests().len(), 1);
    drop(preparation);
}

#[tokio::test]
async fn admitted_replacement_accepts_unavailable_from_its_live_proxy() {
    let diagnostics =
        EstablishedRecipient::<SearchRecoveryDiagnosticProtocol>::issued(Endpoint(997));
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, proxy, _) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };
    let admitted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the admitted recovery emits one replacement"));
    assert_eq!(
        admitted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );

    let unavailable = fixed
        .on(ChildReport::new(
            proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(998),
                phase: ProxyPhase::Activating,
                command: SearchCommand::Find("during replacement".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("an admitted replacement retains its live proxy"));
    assert!(matches!(unavailable.become_, Step::Continue));
    let mut delivered = unavailable.sends.diagnostics.into_requests();
    assert_eq!(delivered.len(), 1);
    match delivered.remove(0) {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(998));
            assert_eq!(unavailable.phase(), ProxyPhase::Activating);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("during replacement".to_owned())
            );
        }
        DiagnosticAction::Deliver { .. } => {
            panic!("the live replacement proxy keeps the unavailable command")
        }
        DiagnosticAction::Terminal { .. } => {
            panic!("the configured diagnostic route remains available")
        }
    }

    let mut sequence = CreationSequence::new();
    let candidates = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
    ];
    let foreign = candidates
        .into_iter()
        .find(|candidate| *candidate != proxy)
        .expect("two distinct IDs include one not owned by the single proxy");
    let unexpected = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(999),
                phase: ProxyPhase::Activating,
                command: SearchCommand::Find("unowned proxy".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the unowned child report becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Continue));
    let mut delivered = unexpected.sends.diagnostics.into_requests();
    assert_eq!(delivered.len(), 1);
    match delivered.remove(0) {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyReported(report),
                },
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(report.child, foreign);
            match report.report {
                ProxyOutcome::Unavailable {
                    sender,
                    phase,
                    command,
                } => {
                    assert_eq!(sender, RuntimeAddr(999));
                    assert_eq!(phase, ProxyPhase::Activating);
                    assert_eq!(command, SearchCommand::Find("unowned proxy".to_owned()));
                }
                ProxyOutcome::Initial { .. }
                | ProxyOutcome::Replacement { .. }
                | ProxyOutcome::WorkerStopped { .. } => {
                    panic!("the unowned report keeps the unavailable command")
                }
            }
        }
        DiagnosticAction::Deliver { .. } => {
            panic!("an unowned child cannot become a worker-unavailable diagnostic")
        }
        DiagnosticAction::Terminal { .. } => {
            panic!("the configured diagnostic route remains available")
        }
    }
}

#[tokio::test]
async fn terminal_unavailable_diagnostic_stops_with_the_complete_command() {
    let (mut fixed, member) = ready_single_proxy_with_worker().await;
    let stopped = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(985),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("terminal command".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the exact unavailable command enters terminal custody"));

    assert!(matches!(stopped.become_, Step::Stop(_)));
    let NoSends = stopped.sends.lifecycle;
    match stopped
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal diagnostic owns the command")
    {
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(985));
            assert_eq!(unavailable.phase(), ProxyPhase::EmptyAfter);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("terminal command".to_owned())
            );
        }
        atomic::DiagnosticAction::Deliver { .. } => {
            panic!("terminal policy cannot fabricate a diagnostic route")
        }
        atomic::DiagnosticAction::Terminal { .. } => {
            panic!("the unavailable command keeps its diagnostic meaning")
        }
    }
}

#[test]
fn unavailable_report_before_proxy_creation_returns_complete() {
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(SearchRole::Search, []).expect("one role is non-empty"),
        ActivationPolicy::new(ONE_WORKER.get()).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("the initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one proxy creation"));
    let mut fixed = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .expect("one proxy creation is emitted");
    let proxy = creation.id();

    let unexpected = fixed
        .on(ChildReport::new(
            proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(986),
                phase: ProxyPhase::Creating,
                command: SearchCommand::Find("not created".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the premature command becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("an unsettled creation is not a live proxy"),
    };
    assert_eq!(returned.child, proxy);
    match returned.report {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(986));
            assert_eq!(phase, ProxyPhase::Creating);
            assert_eq!(command, SearchCommand::Find("not created".to_owned()));
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the complete pre-creation report returns unchanged")
        }
    }
}

#[tokio::test]
async fn shutdown_accepts_unavailable_until_the_exact_proxy_exit() {
    let diagnostics = EstablishedRecipient::<SearchDiagnosticProtocol>::issued(Endpoint(987));
    let (mut fixed, member) = ready_single_proxy_with_policies(
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;
    let shutting_down = fixed
        .receive(RuntimeAddr(988), FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("the online proxy begins shutdown"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one exact proxy shutdown is emitted")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts the uninterpreted shutdown")
        }
    };
    let mut sequence = CreationSequence::new();
    let foreign = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
    ]
    .into_iter()
    .find(|candidate| *candidate != member.proxy)
    .expect("two distinct IDs include one other than the stopping proxy");
    let foreign_report = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(988),
                phase: ProxyPhase::ShuttingDown,
                command: SearchCommand::Find("foreign during shutdown".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the foreign report becomes a routed diagnostic"));
    assert!(matches!(foreign_report.become_, Step::Continue));
    let returned = match foreign_report
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one routed diagnostic owns the foreign report")
    {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyReported(report),
                },
        } => {
            assert_eq!(route, diagnostics);
            report
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("a foreign proxy cannot borrow the stopping member's role")
        }
    };
    assert_eq!(returned.child, foreign);
    match returned.report {
        ProxyOutcome::Unavailable {
            sender,
            phase,
            command,
        } => {
            assert_eq!(sender, RuntimeAddr(988));
            assert_eq!(phase, ProxyPhase::ShuttingDown);
            assert_eq!(
                command,
                SearchCommand::Find("foreign during shutdown".to_owned())
            );
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the complete foreign unavailable report returns unchanged")
        }
    }
    let before_exit = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(989),
                phase: ProxyPhase::ShuttingDown,
                command: SearchCommand::Find("during shutdown".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the still-live proxy report is accepted"));
    assert!(matches!(before_exit.become_, Step::Continue));
    let mut delivered = before_exit.sends.diagnostics.into_requests();
    assert_eq!(delivered.len(), 1);
    match delivered.remove(0) {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(989));
            assert_eq!(unavailable.phase(), ProxyPhase::ShuttingDown);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("during shutdown".to_owned())
            );
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("the live shutdown proxy keeps its unavailable meaning")
        }
    }

    let exit_first = fixed
        .on(ChildStopped::new(
            member.proxy,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact exit is retained until shutdown settlement"));
    assert!(matches!(exit_first.become_, Step::Continue));
    let unexpected = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(990),
                phase: ProxyPhase::Stopped,
                command: SearchCommand::Find("after exit".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the post-exit command becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Continue));
    let returned = match unexpected
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one routed diagnostic owns the post-exit command")
    {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyReported(report),
                },
        } => {
            assert_eq!(route, diagnostics);
            report
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("an exited proxy cannot emit another command report")
        }
    };
    match returned.report {
        ProxyOutcome::Unavailable { command, .. } => {
            assert_eq!(command, SearchCommand::Find("after exit".to_owned()));
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. } => {
            panic!("the complete post-exit report returns unchanged")
        }
    }

    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let retired = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    991,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact settlement closes proxy shutdown"));
    assert!(matches!(retired.become_, Step::Stop(_)));
}

#[tokio::test]
async fn eligible_worker_stop_emits_one_exact_preparation_request() {
    let recovery = Recovery::transient(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, member) = ready_single_proxy_with_recovery(recovery).await;
    let worker_stop =
        ChildStopped::new(member.worker.creation(), Err(Crash::Failed), Instant::now());

    let recovering = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("transient recovery accepts the exact abnormal worker stop"));
    let mut requests = recovering
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs();
    let mut request = match requests.pop().expect("one preparation request is emitted") {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts an uninterpreted request"),
    };
    assert_eq!(request.source_and_role().1, &SearchRole::Search);
    assert_eq!(requests.len(), 0);

    let unexpected = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker.clone(),
                stopped: worker_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("the duplicate worker stop becomes a diagnostic"));
    let duplicate = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the recovery owner cannot accept the same stop twice"),
    };
    assert_eq!(duplicate.child, member.proxy);

    let shutting_down = fixed
        .receive(RuntimeAddr(940), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown drains the proxy while preparation is outside"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the live proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let shutdown_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(shutdown_settled.become_, Step::Continue));
    let waiting_for_source = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact proxy exits"));
    assert!(matches!(waiting_for_source.become_, Step::Continue));
    let stopped = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(request),
        ))
        .unwrap_or_else(|_| panic!("the exact late preparation return closes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

async fn one_replacement_operation() -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            EstablishedRecipient<SearchLifecycleProtocol>,
        >,
    >,
    ProxyOperation<Here, SearchWorker, SearchActivation>,
    atomic::WorkerAttempt,
) {
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(SearchRole::Search, [])
            .unwrap_or_else(|_| panic!("the single role is unique")),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        recovery,
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .publish_lifecycle(EstablishedRecipient::<SearchLifecycleProtocol>::issued(
        Endpoint(970),
    ))
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("the initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits one proxy birth"));
    let mut fixed = initialized.behavior;
    let dispatched = fixed
        .on(commit_proxy_births(initialized.actions.creates, 801))
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"));
    let initial = match dispatched
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one initial proxy operation is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted operation")
        }
    };
    let (proxy, control, operation) = initial.into_parts();
    let ready = ready_proxy_outcome(control).await;
    let worker = match &ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the proxy fixture reaches ready"),
    };
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                proxy,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));
    let online = fixed
        .on(ChildReport::new(proxy, ready))
        .unwrap_or_else(|_| panic!("the exact ready outcome opens the role"));
    assert!(matches!(online.become_, Step::Continue));
    let previous = worker.clone();
    let stopped = ChildStopped::new(worker.creation(), Ok(Exit::Normal), Instant::now());
    let preparing = fixed
        .on(ChildReport::new(
            proxy,
            ProxyOutcome::WorkerStopped { worker, stopped },
        ))
        .unwrap_or_else(|_| panic!("the exact stop starts preparation"));
    let request = match preparing
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one preparation request is emitted")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts an uninterpreted request"),
    };
    let submission =
        WorkerSubmission::activated(SearchWorker(SearchRole::Search), SearchActivation);
    let preparation = match request.accept(submission) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes in one preparation")
        }
    };

    let accepted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact preparation returns to its recovery owner"));
    assert!(matches!(accepted.become_, Step::Continue));
    let mut replacements = accepted.sends.proxy_operations.unattempted().into_inputs();
    let replacement = match replacements
        .pop()
        .expect("immediate recovery issues one replacement")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted replacement")
        }
    };
    assert_eq!(replacements.len(), 0);
    assert_eq!(replacement.creation(), proxy);
    (fixed, replacement, previous)
}

#[tokio::test]
async fn replacement_rejects_an_outcome_for_another_predecessor() {
    let (_, foreign_member) = ready_single_proxy_with_worker().await;
    let foreign = foreign_member.worker;
    let (mut fixed, replacement, previous) = one_replacement_operation().await;
    assert_ne!(foreign, previous);
    let (route, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    802,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt advances its recovery"));
    assert!(matches!(accepted.become_, Step::Continue));

    let unexpected = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces: foreign.clone(),
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("another predecessor's outcome becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete replacement outcome returns unchanged"),
    };
    assert_eq!(returned.child, route);
    match returned.report {
        ProxyOutcome::Replacement {
            outcome: ReplacementOutcome::WorkerAttemptsExhausted { replaces, .. },
        } => assert_eq!(replaces, foreign),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => {
            panic!("the returned report keeps the foreign replacement outcome")
        }
    }
}

#[tokio::test]
async fn replacement_rejects_its_outcome_from_another_proxy() {
    let (mut fixed, members, mut replacements) = admitted_one_for_all(THREE_WORKERS).await;
    let replacement = replacements.remove(0);
    let previous = members[0].worker.clone();
    let (route, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    803,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt advances its recovery"));
    assert!(matches!(accepted.become_, Step::Continue));

    let wrong_proxy = members[1].proxy;
    assert_ne!(wrong_proxy, route);
    let unexpected = fixed
        .on(ChildReport::new(
            wrong_proxy,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces: previous.clone(),
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("another proxy's outcome becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete wrong-proxy outcome returns unchanged"),
    };
    assert_eq!(returned.child, wrong_proxy);
    match returned.report {
        ProxyOutcome::Replacement {
            outcome: ReplacementOutcome::WorkerAttemptsExhausted { replaces, .. },
        } => assert_eq!(replaces, previous),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => {
            panic!("the returned report keeps the replacement outcome")
        }
    }
}

#[tokio::test]
async fn replacement_rejects_a_stop_for_another_predecessor() {
    let (mut fixed, request, members) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    let preparation = complete_coordinated_preparation(request);
    let issued = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the coordinated recovery is admitted"));
    let replacement = match issued
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .remove(0)
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the peer replacement is still uninterpreted")
        }
    };
    let previous = members[0].worker.clone();
    let foreign = members[2].worker.clone();
    assert_ne!(foreign, previous);
    let route = replacement.creation();
    let foreign_stop = ChildStopped::new(foreign.creation(), Ok(Exit::Normal), Instant::now());

    let unexpected = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::WorkerStopped {
                worker: foreign.clone(),
                stopped: foreign_stop,
            },
        ))
        .unwrap_or_else(|_| panic!("another predecessor's stop becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete worker stop returns unchanged"),
    };
    assert_eq!(returned.child, route);
    match returned.report {
        ProxyOutcome::WorkerStopped { worker, stopped } => {
            assert_eq!(worker, foreign);
            assert_eq!(stopped, foreign_stop);
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::Unavailable { .. } => {
            panic!("the returned report keeps the foreign worker stop")
        }
    }
}

#[tokio::test]
async fn accepted_replacement_receipt_keeps_shutdown_live_until_outcome_returns() {
    let (mut fixed, replacement, previous) = one_replacement_operation().await;
    let (replacement_route, replacement_control, replacement_operation) = replacement.into_parts();
    drop(replacement_control);
    let replacement_accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                replacement_route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    802,
                )),
                replacement_operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt advances its recovery"));
    assert!(matches!(replacement_accepted.become_, Step::Continue));

    let shutting_down = fixed
        .receive(RuntimeAddr(941), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the prepared replacement"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let waiting = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(waiting.become_, Step::Continue));
    let awaiting_replacement_outcome = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact stable proxy exits"));
    assert!(matches!(
        awaiting_replacement_outcome.become_,
        Step::Continue
    ));
    let stopped = fixed
        .on(ChildReport::new(
            replacement_route,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces: previous,
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact replacement outcome closes shutdown custody"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert!(stopped.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn retired_proxy_rejects_a_later_outcome_from_a_foreign_child() {
    let (mut fixed, replacement, previous) = one_replacement_operation().await;
    let (replacement_route, replacement_control, replacement_operation) = replacement.into_parts();
    drop(replacement_control);
    let replacement_accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                replacement_route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    802,
                )),
                replacement_operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt advances its recovery"));
    assert!(matches!(replacement_accepted.become_, Step::Continue));

    let shutting_down = fixed
        .receive(RuntimeAddr(941), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the prepared replacement"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts the uninterpreted shutdown")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let waiting = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(waiting.become_, Step::Continue));
    let awaiting_replacement_outcome = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact stable proxy exits"));
    assert!(matches!(
        awaiting_replacement_outcome.become_,
        Step::Continue
    ));

    let mut sequence = CreationSequence::new();
    let foreign = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
    ]
    .into_iter()
    .find(|candidate| *candidate != route)
    .expect("two distinct IDs include one other than the retired proxy");
    let unexpected = fixed
        .on(ChildReport::new(
            foreign,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces: previous.clone(),
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the foreign late outcome becomes a terminal diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::ProxyReported(report) => report,
        _ => panic!("the complete foreign late outcome returns unchanged"),
    };
    assert_eq!(returned.child, foreign);
    match returned.report {
        ProxyOutcome::Replacement {
            outcome:
                ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces,
                    worker,
                    activation,
                },
        } => {
            assert_eq!(replaces, previous);
            assert_eq!(worker, SearchWorker(SearchRole::Search));
            assert_eq!(activation, SearchActivation);
        }
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => {
            panic!("the rejected report retains its exact replacement outcome")
        }
    }
}

#[tokio::test]
async fn ready_replacement_restores_the_role_before_the_next_worker_stop() {
    let (mut fixed, replacement, previous) = one_replacement_operation().await;
    let successor = ready_single_proxy_with_worker().await.1.worker;
    let (route, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    802,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt advances its recovery"));
    assert!(matches!(accepted.become_, Step::Continue));

    let replaced = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::Resolved {
                    replaces: previous,
                    result: WorkerStartResult::Ready {
                        attempt: successor.clone(),
                        readiness: (),
                    },
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact ready replacement restores the role"));
    assert!(matches!(replaced.become_, Step::Continue));
    assert!(
        replaced
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let mut lifecycle = replaced.sends.lifecycle;
    assert_eq!(lifecycle.len(), 2);
    let worker_stopped = lifecycle.remove(0).message;
    let recovery = match worker_stopped.event() {
        FixedLifecycleEvent::WorkerStoppedAfterAdmission { role, recovery, .. } => {
            assert_eq!(role, &SearchRole::Search);
            recovery
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::Unavailable { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("worker stop is the first replacement lifecycle message")
        }
    };
    let restarted = lifecycle.remove(0).message;
    match restarted.event() {
        FixedLifecycleEvent::Restarted {
            role,
            recovery: restarted_recovery,
            ..
        } => {
            assert_eq!(role, &SearchRole::Search);
            assert_eq!(restarted_recovery, recovery);
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::Unavailable { .. }
        | FixedLifecycleEvent::MemberRetired { .. } => {
            panic!("restart is the second replacement lifecycle message")
        }
    }

    let successor_creation = successor.creation();
    let recovering_again = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::WorkerStopped {
                worker: successor,
                stopped: ChildStopped::new(successor_creation, Ok(Exit::Normal), Instant::now()),
            },
        ))
        .unwrap_or_else(|_| panic!("the successor is the role's current worker"));
    assert_eq!(
        recovering_again
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );
}

#[tokio::test]
async fn failed_replacement_outcome_enters_terminal_custody_complete() {
    let (mut fixed, replacement, previous) = one_replacement_operation().await;
    let expected = previous.clone();
    let (route, control, operation) = replacement.into_parts();
    drop(control);
    let accepted = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    802,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact replacement receipt advances its recovery"));
    assert!(matches!(accepted.become_, Step::Continue));

    let failed = fixed
        .on(ChildReport::new(
            route,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces: previous,
                    worker: SearchWorker(SearchRole::Search),
                    activation: SearchActivation,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact failed replacement enters terminal custody"));
    assert!(matches!(failed.become_, Step::Stop(_)));
    assert_eq!(
        failed
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );
    match failed
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal proxy-outcome diagnostic is emitted")
    {
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::ProxyOutcomeFailed(failure),
        } => match failure.outcome() {
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted { replaces, .. },
            } => assert_eq!(replaces, &expected),
            ProxyOutcome::Initial { .. }
            | ProxyOutcome::Replacement { .. }
            | ProxyOutcome::WorkerStopped { .. }
            | ProxyOutcome::Unavailable { .. } => {
                panic!("the exact failed replacement remains in the diagnostic")
            }
        },
        atomic::DiagnosticAction::Deliver { .. } => {
            panic!("terminal policy cannot fabricate a diagnostic route")
        }
        atomic::DiagnosticAction::Terminal {
            diagnostic:
                FixedDiagnostic::ProxyInputRejected(_)
                | FixedDiagnostic::WorkerPreparationFailed(_)
                | FixedDiagnostic::RecoveryDenied(_)
                | FixedDiagnostic::RestartScheduleFailed(_)
                | FixedDiagnostic::WorkerUnavailable(_)
                | FixedDiagnostic::UnexpectedInput { .. },
        } => panic!("the exact proxy outcome keeps its diagnostic kind"),
    }
}

#[tokio::test]
async fn rejected_replacement_input_enters_terminal_custody_complete() {
    let (mut fixed, replacement, _) = one_replacement_operation().await;
    let expected_creation = replacement.creation();
    let rejected = fixed
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: replacement,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the exact replacement rejection returns to its recovery"));
    assert!(matches!(rejected.become_, Step::Stop(_)));
    assert_eq!(
        rejected
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );
    let diagnostic = rejected
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal input-rejection diagnostic is emitted");
    match diagnostic {
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::ProxyInputRejected(failure),
        } => {
            assert_eq!(failure.role(), &SearchRole::Search);
            assert_eq!(failure.reason(), ChildInputReason::ClosedControlLane);
            assert_eq!(failure.operation().creation(), expected_creation);
        }
        atomic::DiagnosticAction::Deliver { .. } => {
            panic!("terminal policy cannot fabricate a diagnostic route")
        }
        atomic::DiagnosticAction::Terminal {
            diagnostic:
                FixedDiagnostic::ProxyOutcomeFailed(_)
                | FixedDiagnostic::WorkerPreparationFailed(_)
                | FixedDiagnostic::RecoveryDenied(_)
                | FixedDiagnostic::RestartScheduleFailed(_)
                | FixedDiagnostic::WorkerUnavailable(_)
                | FixedDiagnostic::UnexpectedInput { .. },
        } => panic!("the exact input rejection keeps its diagnostic kind"),
    }
}

#[tokio::test]
async fn zero_restart_limit_denies_prepared_recovery_without_replacement() {
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(0, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _, expected_stop) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
    .await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };

    let denied = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact prepared recovery reaches restart policy"));
    assert!(matches!(denied.become_, Step::Stop(_)));
    assert!(
        denied
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let diagnostic = denied
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("restart denial emits one terminal diagnostic");
    let atomic::DiagnosticAction::Terminal {
        diagnostic: FixedDiagnostic::RecoveryDenied(denial),
    } = diagnostic
    else {
        panic!("restart policy emits its exact terminal diagnostic")
    };
    assert_eq!(denial.role(), &SearchRole::Search);
    assert_eq!(denial.stopped(), &expected_stop);
    assert!(matches!(
        denial.reason(),
        RecoveryDenialReason::RestartLimitReached {
            active: 0,
            requested,
            maximum: 0,
        } if requested.get() == 1
    ));
}

#[tokio::test]
async fn coordinated_recovery_charges_every_selected_worker_to_the_restart_limit() {
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForAll,
        RestartLimit::new(2, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, members) = ready_three_role_roster_with_policies(
        recovery,
        THREE_WORKERS,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
    .await;
    let request = begin_recovery(&mut fixed, &members[0]);
    let preparation = complete_coordinated_preparation(request);

    let denied = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact preparation reaches restart admission"));
    assert!(matches!(denied.become_, Step::Stop(_)));
    assert!(denied.sends.proxy_operations.is_empty());
    let diagnostic = denied
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("restart admission emits one terminal diagnostic");
    let DiagnosticAction::Terminal {
        diagnostic: FixedDiagnostic::RecoveryDenied(denial),
    } = diagnostic
    else {
        panic!("the coordinated restart limit keeps its diagnostic meaning")
    };
    assert!(matches!(
        denial.reason(),
        RecoveryDenialReason::RestartLimitReached {
            active: 0,
            requested,
            maximum: 2,
        } if requested.get() == 3
    ));
}

#[tokio::test]
async fn routed_replacement_rejection_retires_only_the_failed_member() {
    let diagnostic_route = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>,
        >,
    >::issued(Endpoint(709));
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, proxy, _) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::RetireMember,
        DiagnosticDisposition::deliver_to(diagnostic_route),
    )
    .await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };

    let accepted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("diagnostic routing does not change recovery admission"));
    let replacement = match accepted
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one replacement is admitted")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts the replacement before execution")
        }
    };
    assert_eq!(replacement.creation(), proxy);
    assert!(accepted.sends.diagnostics.is_empty());

    let retired = fixed
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: replacement,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the exact rejection applies the configured reaction"));
    assert!(matches!(retired.become_, Step::Continue));
    let mut diagnostics = retired.sends.diagnostics.into_requests();
    match diagnostics
        .pop()
        .expect("one exact rejection diagnostic is routed")
    {
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyInputRejected(failure),
            ..
        } => {
            assert_eq!(failure.role(), &SearchRole::Search);
            assert_eq!(failure.reason(), ChildInputReason::ClosedControlLane);
        }
        atomic::DiagnosticAction::Terminal { .. } | atomic::DiagnosticAction::Deliver { .. } => {
            panic!("the exact routed proxy-input diagnostic is required")
        }
    }
    assert!(diagnostics.is_empty());
    let shutdown = match retired
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the failed member receives one proxy shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts shutdown before execution")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the failed member retains shutdown settlement"));
    assert!(matches!(settled.become_, Step::Continue));
    let proxy_retired = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the failed member retains exact proxy retirement"));
    assert!(matches!(proxy_retired.become_, Step::Continue));

    let stopped = fixed
        .receive(RuntimeAddr(945), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| {
            panic!("the remaining supervisor can stop without another proxy input")
        });
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert!(
        stopped
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
}

#[tokio::test]
async fn routed_restart_denial_retires_the_exact_trigger() {
    let diagnostic_route = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>,
        >,
    >::issued(Endpoint(710));
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(0, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, proxy, expected_stop) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::RetireMember,
        DiagnosticDisposition::deliver_to(diagnostic_route),
    )
    .await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };

    let denied = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("routed denial applies its topology reaction"));
    let mut diagnostics = denied.sends.diagnostics.into_requests();
    match diagnostics.pop().expect("one restart denial is delivered") {
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RecoveryDenied(denial),
            ..
        } => {
            assert_eq!(denial.role(), &SearchRole::Search);
            assert_eq!(denial.stopped(), &expected_stop);
            assert!(matches!(
                denial.reason(),
                RecoveryDenialReason::RestartLimitReached { maximum: 0, .. }
            ));
        }
        atomic::DiagnosticAction::Terminal { .. } | atomic::DiagnosticAction::Deliver { .. } => {
            panic!("the exact routed restart denial is required")
        }
    }
    assert_eq!(diagnostics.len(), 0);
    let retirement_creations = denied
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.creation(),
            SettledItem::Attempted(_) => {
                panic!("the test intercepts retirement before execution")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(retirement_creations, [proxy]);
}

#[tokio::test]
async fn routed_restart_denial_stops_the_complete_supervisor() {
    let diagnostic_route = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>,
        >,
    >::issued(Endpoint(711));
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(0, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _, expected_stop) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::deliver_to(diagnostic_route),
    )
    .await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };
    let denied = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("routed denial starts complete shutdown"));
    let mut diagnostics = denied.sends.diagnostics.into_requests();
    match diagnostics.pop().expect("one restart denial is delivered") {
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RecoveryDenied(denial),
            ..
        } => assert_eq!(denial.stopped(), &expected_stop),
        atomic::DiagnosticAction::Terminal { .. } | atomic::DiagnosticAction::Deliver { .. } => {
            panic!("the exact routed restart denial is required")
        }
    }
    assert!(diagnostics.is_empty());
    let shutdown = match denied
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the live proxy receives shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts shutdown before execution")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("shutdown settlement remains required"));
    assert!(matches!(settled.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("proxy exit closes complete shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn delayed_prepared_recovery_emits_exact_schedule_before_replacement() {
    let delay = Duration::from_secs(3);
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::constant(delay).expect("the delay is positive"),
    );
    let (mut fixed, request, proxy) = preparing_one_for_one_with_recovery(recovery).await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };

    let scheduled = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact prepared recovery commits one schedule"));
    assert!(
        scheduled
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let mut schedules = scheduled
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs();
    let schedule = match schedules
        .pop()
        .expect("delayed recovery emits one unsettled schedule")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the test intercepts the schedule before execution"),
    };
    assert_eq!(schedules.len(), 0);
    assert_eq!(schedule.id, TimerId(1));
    assert_eq!(schedule.generation, TimerGeneration(0));
    assert_eq!(schedule.after, delay);

    let accepted = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })),
        ))
        .unwrap_or_else(|_| panic!("the exact schedule acceptance starts timer waiting"));
    assert!(
        accepted
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    for elapsed in [
        TimerElapsed::new(TimerId(schedule.id.0 + 1), schedule.generation),
        TimerElapsed::new(schedule.id, TimerGeneration(schedule.generation.0 + 1)),
    ] {
        let unexpected = fixed
            .transition(FixedSupervisorEvent::RestartElapsed(elapsed))
            .unwrap_or_else(|_| panic!("the foreign timer becomes a diagnostic"));
        let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
            FixedSupervisorEvent::RestartElapsed(returned) => returned,
            _ => panic!("a foreign timer cannot release replacement work"),
        };
        assert_eq!(returned, elapsed);
    }

    let released = fixed
        .transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
            schedule.id,
            schedule.generation,
        )))
        .unwrap_or_else(|_| panic!("the exact timer releases replacement work"));
    let mut replacements = released.sends.proxy_operations.unattempted().into_inputs();
    let replacement = match replacements
        .pop()
        .expect("timer release issues one replacement")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted replacement")
        }
    };
    assert_eq!(replacement.creation(), proxy);
    assert_eq!(replacements.len(), 0);

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
            schedule.id,
            schedule.generation,
        )))
        .unwrap_or_else(|_| panic!("the duplicate timer becomes a diagnostic"));
    let duplicate = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartElapsed(elapsed) => elapsed,
        _ => panic!("one timer cannot release the recovery twice"),
    };
    assert_eq!(duplicate.id, schedule.id);
    assert_eq!(duplicate.generation, schedule.generation);
}

#[tokio::test]
async fn exact_restart_schedule_rejection_enters_terminal_custody() {
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::constant(Duration::from_secs(3)).expect("the delay is positive"),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };
    let admitted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the delayed recovery emits its schedule"));
    let schedule = match admitted
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one schedule is emitted")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the test intercepts the schedule before execution"),
    };

    let failed = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            }),
        ))
        .unwrap_or_else(|_| panic!("exact timer rejection follows configured terminal policy"));
    assert!(matches!(failed.become_, Step::Stop(_)));
    assert!(
        failed
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let mut diagnostics = failed.sends.diagnostics.into_requests();
    match diagnostics
        .pop()
        .expect("one exact schedule failure enters terminal custody")
    {
        atomic::DiagnosticAction::Terminal {
            diagnostic: FixedDiagnostic::RestartScheduleFailed(failure),
        } => {
            assert_eq!(failure.role(), &SearchRole::Search);
            assert_eq!(failure.request(), schedule);
            assert_eq!(failure.reason(), ScheduleAfterRejection::DeadlineOverflow);
        }
        atomic::DiagnosticAction::Deliver { .. } | atomic::DiagnosticAction::Terminal { .. } => {
            panic!("the exact restart schedule diagnostic is required")
        }
    }
    assert_eq!(diagnostics.len(), 0);

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
            schedule.id,
            schedule.generation,
        )))
        .unwrap_or_else(|_| panic!("the later timer becomes a diagnostic"));
    let timer = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartElapsed(timer) => timer,
        _ => panic!("rejected scheduling cannot later issue replacement work"),
    };
    assert_eq!(timer.id, schedule.id);
    assert_eq!(timer.generation, schedule.generation);
}

#[tokio::test]
async fn routed_restart_schedule_rejection_retires_only_the_trigger() {
    let diagnostic_route = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>,
        >,
    >::issued(Endpoint(712));
    let (mut fixed, schedule) = delayed_schedule_with_policies(
        FailureReaction::RetireMember,
        DiagnosticDisposition::deliver_to(diagnostic_route),
    )
    .await;
    let failed = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::QueueSequenceExhausted,
            }),
        ))
        .unwrap_or_else(|_| panic!("routed rejection starts exact member retirement"));
    assert!(matches!(failed.become_, Step::Continue));
    assert_eq!(failed.sends.diagnostics.into_requests().len(), 1);
    let shutdown = match failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the exact trigger proxy receives shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts retirement before execution")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("retirement settlement is accepted"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    assert!(operation_settled.sends.diagnostics.is_empty());
    let retired = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("exact proxy exit retires only the member"));
    assert!(matches!(retired.become_, Step::Continue));
}

#[tokio::test]
async fn routed_restart_schedule_rejection_stops_the_supervisor() {
    let diagnostic_route = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>,
        >,
    >::issued(Endpoint(713));
    let (mut fixed, schedule) = delayed_schedule_with_policies(
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::deliver_to(diagnostic_route),
    )
    .await;
    let failed = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::QueueGenerationExhausted,
            }),
        ))
        .unwrap_or_else(|_| panic!("routed rejection starts complete shutdown"));
    assert!(matches!(failed.become_, Step::Continue));
    assert_eq!(failed.sends.diagnostics.into_requests().len(), 1);
    let shutdown = match failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the live proxy receives shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts shutdown before execution")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("shutdown settlement is accepted"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    assert!(operation_settled.sends.diagnostics.is_empty());
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("exact proxy exit closes complete shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn non_rejection_schedule_results_leave_the_recovery_unchanged() {
    let (mut fixed, schedule) = delayed_schedule_with_policies(
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
    .await;
    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                id: TimerId(schedule.id.0 + 1),
                generation: schedule.generation,
            })),
        ))
        .unwrap_or_else(|_| panic!("the foreign receipt becomes a diagnostic"));
    let foreign = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartScheduleSettled(input) => input,
        _ => panic!("a foreign schedule receipt cannot advance recovery"),
    };
    assert!(matches!(
        foreign,
        SettledItem::Attempted(ItemSettlement::Accepted(_))
    ));

    for item in [
        ScheduleAfter::new(
            TimerId(schedule.id.0 + 1),
            schedule.generation,
            schedule.after,
        ),
        ScheduleAfter::new(
            schedule.id,
            TimerGeneration(schedule.generation.0 + 1),
            schedule.after,
        ),
    ] {
        let unexpected = fixed
            .transition(FixedSupervisorEvent::RestartScheduleSettled(
                SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason: ScheduleAfterRejection::DeadlineOverflow,
                }),
            ))
            .unwrap_or_else(|_| panic!("the foreign rejection becomes a diagnostic"));
        let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
            FixedSupervisorEvent::RestartScheduleSettled(returned) => returned,
            _ => panic!("a foreign schedule rejection cannot advance recovery"),
        };
        assert_eq!(
            returned,
            SettledItem::Attempted(ItemSettlement::Rejected {
                item,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            })
        );
    }

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Unattempted(schedule),
        ))
        .unwrap_or_else(|_| panic!("the unattempted request becomes a diagnostic"));
    let unattempted = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartScheduleSettled(input) => input,
        _ => panic!("an unattempted schedule cannot advance recovery"),
    };
    assert_eq!(unattempted, SettledItem::Unattempted(schedule));

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Corrupt {
                item: schedule,
                fault: InterpreterFault::CorruptTraversal,
            }),
        ))
        .unwrap_or_else(|_| panic!("the corrupt settlement becomes a diagnostic"));
    let corrupt = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartScheduleSettled(input) => input,
        _ => panic!("corrupt interpretation cannot advance recovery"),
    };
    assert!(matches!(
        corrupt,
        SettledItem::Attempted(ItemSettlement::Corrupt {
            item,
            fault: InterpreterFault::CorruptTraversal,
        }) if item == schedule
    ));

    let accepted = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })),
        ))
        .unwrap_or_else(|_| panic!("the exact receipt still advances unchanged recovery"));
    assert!(accepted.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn shutdown_waits_for_an_emitted_restart_schedule_settlement() {
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::constant(Duration::from_secs(3)).expect("the delay is positive"),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };
    let admitted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the delayed recovery emits its schedule"));
    let schedule = match admitted
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one schedule remains unsettled")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the test intercepts the schedule before execution"),
    };

    let shutting_down = fixed
        .receive(RuntimeAddr(946), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown adopts the delayed recovery"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one live proxy receives shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts the shutdown before execution")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let proxy_stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("proxy exit still waits for the emitted schedule"));
    assert!(matches!(proxy_stopped.become_, Step::Continue));

    let schedule_settled = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })),
        ))
        .unwrap_or_else(|_| panic!("shutdown retains the exact schedule settlement"));
    assert!(matches!(schedule_settled.become_, Step::Stop(_)));
}

#[tokio::test]
async fn late_preparation_and_proxy_exit_close_shutdown_in_either_order() {
    let (mut fixed, request, _) = preparing_one_for_one().await;
    let submission =
        WorkerSubmission::activated(SearchWorker(SearchRole::Search), SearchActivation);
    let preparation = match request.accept(submission) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes in one preparation")
        }
    };
    let shutting_down = fixed
        .receive(RuntimeAddr(942), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the outstanding preparation"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };

    let awaiting_proxy = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact late preparation is retained"));
    assert!(matches!(awaiting_proxy.become_, Step::Continue));
    assert_eq!(
        awaiting_proxy
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );

    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact stable proxy exit closes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn late_corrupt_preparation_remains_owned_until_proxy_exit() {
    let source_drops = Arc::new(AtomicUsize::new(0));
    let recovery = Recovery::permanent(
        TrackedWorkshop {
            drops: Arc::clone(&source_drops),
        },
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let shutting_down = fixed
        .receive(RuntimeAddr(943), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the outstanding preparation"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let retained = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Corrupt {
                item: request,
                fault: InterpreterFault::CorruptTraversal,
            }),
        ))
        .unwrap_or_else(|_| panic!("the exact corrupt result remains in shutdown custody"));
    assert!(matches!(retained.become_, Step::Continue));
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);

    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact stable proxy exit closes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    drop(fixed);
    assert_eq!(source_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn late_source_and_worker_rejections_remain_owned_through_shutdown() {
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut source_rejected, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let shutting_down = source_rejected
        .receive(RuntimeAddr(944), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the outstanding preparation"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = source_rejected
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let waiting_for_source = source_rejected
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact stable proxy exits"));
    assert!(matches!(waiting_for_source.become_, Step::Continue));
    let stopped = source_rejected
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: request,
                reason: WorkshopRejection::SourceUnavailable,
            }),
        ))
        .unwrap_or_else(|_| panic!("the exact source rejection closes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));

    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut worker_rejected, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let preparation = request.reject(WorkshopRejection::WorkerUnavailable);
    let shutting_down = worker_rejected
        .receive(RuntimeAddr(945), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown retains the worker rejection"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let retained = worker_rejected
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact worker rejection remains in shutdown custody"));
    assert!(matches!(retained.become_, Step::Continue));
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = worker_rejected
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact shutdown operation settles"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let stopped = worker_rejected
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact stable proxy exit closes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn foreign_late_preparation_cannot_close_another_shutdown() {
    let (mut owner, owner_request, _) = preparing_one_for_one().await;
    let (mut source, source_request, _) = preparing_one_for_one().await;
    let owner_shutdown = owner
        .receive(RuntimeAddr(946), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("owner shutdown retains its preparation"));
    let shutdown = match owner_shutdown
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the owner stable proxy receives one shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let unexpected = owner
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(source_request),
        ))
        .unwrap_or_else(|_| panic!("the foreign preparation becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::WorkerPreparationSettled(SettledItem::Unattempted(request)) => {
            request
        }
        _ => panic!("another recovery ticket cannot close this shutdown"),
    };

    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = owner
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact owner shutdown operation settles"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let still_waiting = owner
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact owner stable proxy exits"));
    assert!(matches!(still_waiting.become_, Step::Continue));

    let source_shutdown = source
        .receive(RuntimeAddr(947), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("source shutdown retains its preparation"));
    assert!(matches!(source_shutdown.become_, Step::Continue));
    let accepted_by_source = source
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(returned),
        ))
        .unwrap_or_else(|_| panic!("the unchanged request remains valid for its owner"));
    assert!(matches!(accepted_by_source.become_, Step::Continue));

    let stopped = owner
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(owner_request),
        ))
        .unwrap_or_else(|_| panic!("only the owner's exact request closes shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn terminal_source_rejection_stops_with_one_complete_diagnostic() {
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let stopped = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: request,
                reason: WorkshopRejection::SourceUnavailable,
            }),
        ))
        .unwrap_or_else(|_| panic!("the exact source rejection terminates the supervisor"));

    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(
        stopped
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
    let diagnostic = stopped
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal diagnostic is emitted");
    let atomic::DiagnosticAction::Terminal {
        diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
    } = diagnostic
    else {
        panic!("source rejection uses terminal preparation diagnostics")
    };
    assert_eq!(failure.role(), &SearchRole::Search);
    assert_eq!(failure.prepared().len(), 0);
    assert_eq!(failure.remaining_roles().count(), 1);
    assert!(matches!(
        failure.reason(),
        WorkerPreparationFailureReason::SourceRejected(WorkshopRejection::SourceUnavailable)
    ));
}

#[tokio::test]
async fn terminal_worker_rejection_keeps_the_exact_failed_role() {
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let preparation = request.reject(WorkshopRejection::WorkerUnavailable);
    let stopped = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact worker rejection terminates the supervisor"));
    let diagnostic = stopped
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal diagnostic is emitted");
    let atomic::DiagnosticAction::Terminal {
        diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
    } = diagnostic
    else {
        panic!("worker rejection uses terminal preparation diagnostics")
    };

    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(failure.prepared().len(), 0);
    assert_eq!(failure.remaining_roles().len(), 0);
    assert!(matches!(
        failure.reason(),
        WorkerPreparationFailureReason::WorkerRejected {
            role: SearchRole::Search,
            reason: WorkshopRejection::WorkerUnavailable,
        }
    ));
}

#[tokio::test]
async fn coordinated_worker_rejection_restores_peers_before_supervisor_shutdown() {
    let diagnostics = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, FallibleWorkshop>,
        >,
    >::issued(Endpoint(985));
    let (mut fixed, members) = ready_three_role_roster_with_policies(
        Recovery::permanent(
            FallibleWorkshop,
            Strategy::OneForAll,
            RestartLimit::new(3, Duration::from_secs(60)),
            RestartRelease::immediate(),
        ),
        THREE_WORKERS,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;

    let mut preparation = begin_recovery(&mut fixed, &members[1]);
    assert_eq!(preparation.source_and_role().1, &SearchRole::Search);
    let mut preparation = match preparation.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Continue(preparation) => preparation,
        ControlFlow::Break(_) => panic!("Index and Spellcheck remain selected"),
    };
    assert_eq!(preparation.source_and_role().1, &SearchRole::Index);
    let rejected = preparation.reject(WorkshopRejection::WorkerUnavailable);

    let shutdown = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(rejected)),
        ))
        .unwrap_or_else(|_| panic!("the coordinated rejection begins supervisor shutdown"));
    assert!(matches!(shutdown.become_, Step::Continue));
    assert!(shutdown.sends.worker_preparations.is_empty());
    assert!(shutdown.sends.restart_schedules.is_empty());
    assert!(shutdown.sends.lifecycle.is_empty());

    let mut emitted_diagnostics = shutdown.sends.diagnostics.into_requests();
    match emitted_diagnostics
        .pop()
        .expect("one preparation diagnostic is delivered")
    {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(failure.role(), &SearchRole::Index);
            let mut prepared = failure.prepared();
            assert_eq!(
                prepared.next().map(|(role, _)| role),
                Some(&SearchRole::Search)
            );
            assert!(prepared.next().is_none());
            assert!(matches!(
                failure.reason(),
                WorkerPreparationFailureReason::WorkerRejected {
                    role: SearchRole::Index,
                    reason: WorkshopRejection::WorkerUnavailable,
                }
            ));
            assert_eq!(
                failure.remaining_roles().collect::<Vec<_>>(),
                [&SearchRole::Spellcheck]
            );
        }
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("the exact coordinated rejection keeps its preparation meaning")
        }
    }
    assert!(emitted_diagnostics.is_empty());

    let stopped_proxies = shutdown
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(operation) => operation.creation(),
            SettledItem::Attempted(_) => {
                panic!("the test intercepts uninterpreted shutdown operations")
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        stopped_proxies,
        members
            .into_iter()
            .map(|member| member.proxy)
            .collect::<Vec<_>>()
    );
}

#[derive(Clone, Copy)]
enum PreparationReturn {
    SourceRejected,
    InterpreterCorrupt,
    Unattempted,
}

#[tokio::test]
async fn every_coordinated_preparation_return_preserves_selection_and_reaction() {
    let recoveries = [
        (Strategy::OneForAll, SearchRole::Search),
        (Strategy::OneForAll, SearchRole::Index),
        (Strategy::OneForAll, SearchRole::Spellcheck),
        (Strategy::RestForOne, SearchRole::Search),
        (Strategy::RestForOne, SearchRole::Index),
    ];
    let returns = [
        PreparationReturn::SourceRejected,
        PreparationReturn::InterpreterCorrupt,
        PreparationReturn::Unattempted,
    ];
    let reactions = [
        FailureReaction::RetireMember,
        FailureReaction::StopSupervisor,
    ];

    for (strategy, trigger_role) in recoveries {
        let selected_roles = expected_recovery_roles(strategy, trigger_role);
        for preparation_return in returns {
            for reaction in reactions {
                let diagnostics = EstablishedRecipient::<
                    MessageProtocol<
                        RuntimeAddr,
                        FixedDiagnostic<
                            SearchRole,
                            SearchWorker,
                            SearchActivation,
                            FallibleWorkshop,
                        >,
                    >,
                >::issued(Endpoint(986));
                let (mut fixed, members) = ready_three_role_roster_with_policies(
                    Recovery::permanent(
                        FallibleWorkshop,
                        strategy,
                        RestartLimit::new(3, Duration::from_secs(60)),
                        RestartRelease::immediate(),
                    ),
                    THREE_WORKERS,
                    reaction,
                    DiagnosticDisposition::deliver_to(diagnostics.clone()),
                )
                .await;
                let trigger_position = roster_position(trigger_role);
                let request = begin_recovery(&mut fixed, &members[trigger_position]);
                let returned = match preparation_return {
                    PreparationReturn::SourceRejected => {
                        SettledItem::Attempted(ItemSettlement::Rejected {
                            item: request,
                            reason: WorkshopRejection::SourceUnavailable,
                        })
                    }
                    PreparationReturn::InterpreterCorrupt => {
                        SettledItem::Attempted(ItemSettlement::Corrupt {
                            item: request,
                            fault: InterpreterFault::MissingCapability,
                        })
                    }
                    PreparationReturn::Unattempted => SettledItem::Unattempted(request),
                };
                let expected_shutdown = match reaction {
                    FailureReaction::RetireMember => vec![members[trigger_position].proxy],
                    FailureReaction::StopSupervisor => {
                        members.iter().map(|member| member.proxy).collect()
                    }
                };

                let failed = fixed
                    .transition(FixedSupervisorEvent::WorkerPreparationSettled(returned))
                    .unwrap_or_else(|_| {
                        panic!("each preparation return applies its configured reaction")
                    });
                assert!(matches!(failed.become_, Step::Continue));
                assert!(failed.creates.is_empty());
                assert!(failed.sends.proxy_observations.is_empty());
                assert!(failed.sends.worker_preparations.is_empty());
                assert!(failed.sends.restart_schedules.is_empty());
                assert!(failed.sends.lifecycle.is_empty());
                assert!(failed.sends.status_replies.into_deliveries().is_empty());
                assert!(failed.sends.capability_replies.into_deliveries().is_empty());

                let mut emitted_diagnostics = failed.sends.diagnostics.into_requests();
                match emitted_diagnostics
                    .pop()
                    .expect("one exact preparation failure is delivered")
                {
                    DiagnosticAction::Deliver {
                        route,
                        diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
                    } => {
                        assert_eq!(route, diagnostics);
                        assert_eq!(failure.role(), &trigger_role);
                        assert!(failure.prepared().next().is_none());
                        assert_eq!(
                            failure.remaining_roles().collect::<Vec<_>>(),
                            selected_roles.iter().collect::<Vec<_>>()
                        );
                        match (preparation_return, failure.reason()) {
                            (
                                PreparationReturn::SourceRejected,
                                WorkerPreparationFailureReason::SourceRejected(
                                    WorkshopRejection::SourceUnavailable,
                                ),
                            )
                            | (
                                PreparationReturn::InterpreterCorrupt,
                                WorkerPreparationFailureReason::InterpreterFault(
                                    InterpreterFault::MissingCapability,
                                ),
                            )
                            | (
                                PreparationReturn::Unattempted,
                                WorkerPreparationFailureReason::Unattempted,
                            ) => {}
                            _ => panic!("the diagnostic preserves the exact return class"),
                        }
                    }
                    DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
                        panic!("a preparation return keeps its failure meaning")
                    }
                }
                assert!(emitted_diagnostics.is_empty());

                let stopped_proxies = failed
                    .sends
                    .proxy_operations
                    .unattempted()
                    .into_inputs()
                    .into_iter()
                    .map(|settlement| match settlement {
                        SettledItem::Unattempted(operation) => operation.creation(),
                        SettledItem::Attempted(_) => {
                            panic!("the test intercepts uninterpreted shutdown operations")
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(stopped_proxies, expected_shutdown);

                for queried_role in SEARCH_ROLES {
                    let capability = fixed
                        .receive(
                            RuntimeAddr(987),
                            FixedCommand::capability(
                                queried_role,
                                Recipient::global(RuntimeAddr(988)),
                            ),
                        )
                        .unwrap_or_else(|_| panic!("the post-failure roster remains queryable"));
                    let capability =
                        logical_reply(capability.sends.capability_replies.into_deliveries());
                    match reaction {
                        FailureReaction::RetireMember if queried_role != trigger_role => {
                            assert!(matches!(
                                capability,
                                CapabilityResult::Ready { role, .. } if role == queried_role
                            ));
                        }
                        FailureReaction::RetireMember | FailureReaction::StopSupervisor => {
                            assert!(matches!(
                                capability,
                                CapabilityResult::Unavailable {
                                    role,
                                    phase: atomic::UnavailablePhase::Stopping,
                                } if role == queried_role
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn every_coordinated_worker_rejection_preserves_strategy_and_topology_reaction() {
    let recoveries = [
        (Strategy::OneForAll, SearchRole::Search),
        (Strategy::OneForAll, SearchRole::Index),
        (Strategy::OneForAll, SearchRole::Spellcheck),
        (Strategy::RestForOne, SearchRole::Search),
        (Strategy::RestForOne, SearchRole::Index),
    ];
    let reactions = [
        FailureReaction::RetireMember,
        FailureReaction::StopSupervisor,
    ];

    for (strategy, trigger_role) in recoveries {
        let selected_roles = expected_recovery_roles(strategy, trigger_role);
        for rejected_role in selected_roles.iter().copied() {
            for reaction in reactions {
                let diagnostics = EstablishedRecipient::<
                    MessageProtocol<
                        RuntimeAddr,
                        FixedDiagnostic<
                            SearchRole,
                            SearchWorker,
                            SearchActivation,
                            FallibleWorkshop,
                        >,
                    >,
                >::issued(Endpoint(987));
                let (mut fixed, members) = ready_three_role_roster_with_policies(
                    Recovery::permanent(
                        FallibleWorkshop,
                        strategy,
                        RestartLimit::new(3, Duration::from_secs(60)),
                        RestartRelease::immediate(),
                    ),
                    THREE_WORKERS,
                    reaction,
                    DiagnosticDisposition::deliver_to(diagnostics.clone()),
                )
                .await;
                let trigger_position = roster_position(trigger_role);
                let preparation = begin_recovery(&mut fixed, &members[trigger_position]);
                let preparation =
                    reject_selected_worker(preparation, &selected_roles, rejected_role);
                let expected_shutdown = match reaction {
                    FailureReaction::RetireMember => vec![members[trigger_position].proxy],
                    FailureReaction::StopSupervisor => {
                        members.iter().map(|member| member.proxy).collect()
                    }
                };

                let failed = fixed
                    .transition(FixedSupervisorEvent::WorkerPreparationSettled(
                        SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
                    ))
                    .unwrap_or_else(|_| {
                        panic!("every exact coordinated rejection applies its configured reaction")
                    });
                assert!(matches!(failed.become_, Step::Continue));
                assert!(failed.creates.is_empty());
                assert!(failed.sends.proxy_observations.is_empty());
                assert!(failed.sends.worker_preparations.is_empty());
                assert!(failed.sends.restart_schedules.is_empty());
                assert!(failed.sends.lifecycle.is_empty());

                let mut emitted_diagnostics = failed.sends.diagnostics.into_requests();
                match emitted_diagnostics
                    .pop()
                    .expect("one exact preparation failure is delivered")
                {
                    DiagnosticAction::Deliver {
                        route,
                        diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
                    } => {
                        assert_eq!(route, diagnostics);
                        assert_eq!(failure.role(), &trigger_role);
                        let rejected_position = selected_roles
                            .iter()
                            .position(|role| *role == rejected_role)
                            .expect("the rejected role remains in its selected roster");
                        let prepared = failure.prepared().collect::<Vec<_>>();
                        assert_eq!(prepared.len(), rejected_position);
                        for ((prepared_role, submission), expected_role) in
                            prepared.into_iter().zip(&selected_roles)
                        {
                            assert_eq!(prepared_role, expected_role);
                            assert_eq!(
                                submission,
                                &WorkerSubmission::activated(
                                    worker(expected_role),
                                    SearchActivation,
                                )
                            );
                        }
                        assert!(matches!(
                            failure.reason(),
                            WorkerPreparationFailureReason::WorkerRejected {
                                role,
                                reason: WorkshopRejection::WorkerUnavailable,
                            } if role == &rejected_role
                        ));
                        assert_eq!(
                            failure.remaining_roles().collect::<Vec<_>>(),
                            selected_roles[(rejected_position + 1)..]
                                .iter()
                                .collect::<Vec<_>>()
                        );
                    }
                    DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
                        panic!("a worker rejection keeps its preparation-failure meaning")
                    }
                }
                assert!(emitted_diagnostics.is_empty());

                let stopped_proxies = failed
                    .sends
                    .proxy_operations
                    .unattempted()
                    .into_inputs()
                    .into_iter()
                    .map(|settlement| match settlement {
                        SettledItem::Unattempted(operation) => operation.creation(),
                        SettledItem::Attempted(_) => {
                            panic!("the test intercepts uninterpreted shutdown operations")
                        }
                    })
                    .collect::<Vec<_>>();
                assert_eq!(stopped_proxies, expected_shutdown);
            }
        }
    }
}

#[tokio::test]
async fn terminal_corrupt_preparation_keeps_interpreter_fault_and_selected_role() {
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let stopped = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Corrupt {
                item: request,
                fault: InterpreterFault::MissingCapability,
            }),
        ))
        .unwrap_or_else(|_| panic!("the exact corrupt preparation terminates the supervisor"));
    let diagnostic = stopped
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal diagnostic is emitted");
    let atomic::DiagnosticAction::Terminal {
        diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
    } = diagnostic
    else {
        panic!("interpreter corruption uses terminal preparation diagnostics")
    };

    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(failure.prepared().len(), 0);
    assert_eq!(failure.remaining_roles().count(), 1);
    assert!(matches!(
        failure.reason(),
        WorkerPreparationFailureReason::InterpreterFault(InterpreterFault::MissingCapability)
    ));
}

#[tokio::test]
async fn terminal_unattempted_preparation_keeps_selected_role() {
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _) = preparing_one_for_one_with_recovery(recovery).await;
    let stopped = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(request),
        ))
        .unwrap_or_else(|_| panic!("the exact unattempted preparation terminates the supervisor"));
    let diagnostic = stopped
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one terminal diagnostic is emitted");
    let atomic::DiagnosticAction::Terminal {
        diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
    } = diagnostic
    else {
        panic!("an unattempted preparation uses terminal preparation diagnostics")
    };

    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(failure.prepared().len(), 0);
    assert_eq!(failure.remaining_roles().count(), 1);
    assert!(matches!(
        failure.reason(),
        WorkerPreparationFailureReason::Unattempted
    ));
}

#[tokio::test]
async fn routed_preparation_failure_retires_only_the_failed_member() {
    let diagnostics = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, FallibleWorkshop>,
        >,
    >::issued(Endpoint(911));
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, proxy, _) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::RetireMember,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;
    let failed = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: request,
                reason: WorkshopRejection::SourceUnavailable,
            }),
        ))
        .unwrap_or_else(|_| panic!("the exact rejection starts member retirement"));

    assert!(matches!(failed.become_, Step::Continue));
    let diagnostic = failed
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one routed diagnostic is emitted");
    match diagnostic {
        atomic::DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(failure.role(), &SearchRole::Search);
            assert!(matches!(
                failure.reason(),
                WorkerPreparationFailureReason::SourceRejected(
                    WorkshopRejection::SourceUnavailable
                )
            ));
        }
        atomic::DiagnosticAction::Terminal { .. } => {
            panic!("a routed diagnostic cannot become terminal custody")
        }
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyOutcomeFailed(_),
            ..
        } => panic!("a preparation rejection cannot become a proxy-outcome failure"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RecoveryDenied(_),
            ..
        } => panic!("a preparation rejection cannot become a recovery denial"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RestartScheduleFailed(_),
            ..
        } => panic!("a preparation rejection cannot become a schedule failure"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyInputRejected(_),
            ..
        } => panic!("a preparation rejection cannot become an input rejection"),
        atomic::DiagnosticAction::Deliver {
            diagnostic:
                FixedDiagnostic::WorkerUnavailable(_) | FixedDiagnostic::UnexpectedInput { .. },
            ..
        } => panic!("a preparation rejection cannot become an unavailable command"),
    }

    let unavailable = fixed
        .on(ChildReport::new(
            proxy,
            ProxyOutcome::Unavailable {
                sender: RuntimeAddr(910),
                phase: ProxyPhase::EmptyAfter,
                command: SearchCommand::Find("retiring proxy".to_owned()),
            },
        ))
        .unwrap_or_else(|_| panic!("the retiring member retains its exact proxy"));
    assert!(matches!(unavailable.become_, Step::Continue));
    let mut delivered = unavailable.sends.diagnostics.into_requests();
    assert_eq!(delivered.len(), 1);
    match delivered.remove(0) {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerUnavailable(unavailable),
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(unavailable.role(), &SearchRole::Search);
            assert_eq!(unavailable.sender(), &RuntimeAddr(910));
            assert_eq!(unavailable.phase(), ProxyPhase::EmptyAfter);
            assert_eq!(
                unavailable.command(),
                &SearchCommand::Find("retiring proxy".to_owned())
            );
        }
        DiagnosticAction::Deliver { .. } => {
            panic!("the retiring proxy keeps the unavailable command")
        }
        DiagnosticAction::Terminal { .. } => {
            panic!("the configured diagnostic route remains available")
        }
    }
    let shutdown = match failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("retiring the failed member emits one proxy shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let creation = shutdown.creation();

    let mut sequence = CreationSequence::new();
    let candidates = [
        sequence.issue().expect("the sequence has a first ID"),
        sequence.issue().expect("the sequence has a second ID"),
    ];
    let foreign = candidates
        .into_iter()
        .find(|candidate| *candidate != creation)
        .expect("two distinct IDs include one other than the retiring proxy");
    let foreign_stop = ChildStopped::new(foreign, Ok(Exit::Normal), Instant::now());
    let unexpected = fixed
        .on(foreign_stop)
        .unwrap_or_else(|_| panic!("another proxy's exit becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Continue));
    let mut delivered = unexpected.sends.diagnostics.into_requests();
    assert_eq!(delivered.len(), 1);
    match delivered.remove(0) {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyStopped(returned),
                },
        } => {
            assert_eq!(route, diagnostics);
            assert_eq!(returned, foreign_stop);
        }
        DiagnosticAction::Deliver { .. } => {
            panic!("the foreign exit keeps its unexpected-input meaning")
        }
        DiagnosticAction::Terminal { .. } => {
            panic!("the configured diagnostic route remains available")
        }
    }

    let exit_first = fixed
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the member retains an early exact proxy exit"));
    assert!(matches!(exit_first.become_, Step::Continue));
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let retired = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact settlement closes member retirement"));
    assert!(matches!(retired.become_, Step::Continue));

    let stopped = fixed
        .receive(RuntimeAddr(912), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("shutdown sees the retired member and restored source"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
    assert_eq!(
        stopped
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        0
    );
}

#[tokio::test]
async fn later_retiring_member_accepts_its_own_proxy_exit() {
    let diagnostics =
        EstablishedRecipient::<SearchRecoveryDiagnosticProtocol>::issued(Endpoint(914));
    let (mut fixed, members) = ready_three_role_roster_with_policies(
        Recovery::permanent(
            SearchWorkshop,
            Strategy::OneForOne,
            RestartLimit::new(3, Duration::from_secs(60)),
            RestartRelease::immediate(),
        ),
        THREE_WORKERS,
        FailureReaction::RetireMember,
        DiagnosticDisposition::deliver_to(diagnostics),
    )
    .await;

    let search_request = begin_recovery(&mut fixed, &members[0]);
    let search_failed = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(search_request),
        ))
        .unwrap_or_else(|_| panic!("the first preparation failure retires Search"));
    assert_eq!(search_failed.sends.diagnostics.into_requests().len(), 1);
    let search_shutdown = match search_failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("Search receives one proxy shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the test intercepts the Search shutdown"),
    };
    assert_eq!(search_shutdown.creation(), members[0].proxy);

    let index_request = begin_recovery(&mut fixed, &members[1]);
    let index_failed = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(index_request),
        ))
        .unwrap_or_else(|_| panic!("the second preparation failure retires Index"));
    assert_eq!(index_failed.sends.diagnostics.into_requests().len(), 1);
    let index_shutdown = match index_failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("Index receives one proxy shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the test intercepts the Index shutdown"),
    };
    assert_eq!(index_shutdown.creation(), members[1].proxy);

    let exit_first = fixed
        .on(ChildStopped::new(
            members[1].proxy,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("Index owns its exit despite earlier retiring Search"));
    assert!(matches!(exit_first.become_, Step::Continue));
    assert!(exit_first.sends.diagnostics.is_empty());
    assert!(exit_first.sends.lifecycle.is_empty());

    let (route, control, operation) = index_shutdown.into_parts();
    drop(control);
    let retired = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    802,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the Index shutdown receipt completes its retirement"));
    assert!(retired.sends.diagnostics.is_empty());
    let mut lifecycle = retired.sends.lifecycle;
    assert_eq!(lifecycle.len(), 1);
    match lifecycle.remove(0).message.event() {
        FixedLifecycleEvent::MemberRetired { role } => assert_eq!(role, &SearchRole::Index),
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::Unavailable { .. } => {
            panic!("the exact Index join retires only Index")
        }
    }
}

#[tokio::test]
async fn exact_proxy_retirement_publishes_the_topology_change_once() {
    let diagnostics = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, FallibleWorkshop>,
        >,
    >::issued(Endpoint(913));
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, member) = ready_single_proxy_with_lifecycle_policies(
        recovery,
        FailureReaction::RetireMember,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;
    let stopped = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), Instant::now());
    let preparing = fixed
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker,
                stopped,
            },
        ))
        .unwrap_or_else(|_| panic!("the exact worker stop starts preparation"));
    assert!(preparing.sends.lifecycle.is_empty());
    let request = match preparing
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one preparation request is emitted")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts the request"),
    };
    let failed = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: request,
                reason: WorkshopRejection::SourceUnavailable,
            }),
        ))
        .unwrap_or_else(|_| panic!("the routed failure starts member retirement"));
    assert!(failed.sends.lifecycle.is_empty());
    let shutdown = match failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("the failed member receives one proxy shutdown")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the test intercepts proxy shutdown"),
    };
    let exit_first = fixed
        .on(ChildStopped::new(
            shutdown.creation(),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the first exact input remains pending"));
    assert!(exit_first.sends.lifecycle.is_empty());

    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let retired = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the second exact input retires the member"));
    let mut lifecycle = retired.sends.lifecycle;
    assert_eq!(lifecycle.len(), 1);
    match lifecycle.remove(0).message.event() {
        FixedLifecycleEvent::MemberRetired { role } => {
            assert_eq!(role, &SearchRole::Search);
        }
        FixedLifecycleEvent::Started { .. }
        | FixedLifecycleEvent::Restarted { .. }
        | FixedLifecycleEvent::WorkerStoppedIneligible { .. }
        | FixedLifecycleEvent::WorkerStoppedAfterAdmission { .. }
        | FixedLifecycleEvent::Unavailable { .. } => {
            panic!("exact proxy retirement has one lifecycle meaning")
        }
    }

    let unexpected = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the duplicate exit becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Continue));
    match unexpected
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one routed diagnostic owns the duplicate exit")
    {
        DiagnosticAction::Deliver {
            route,
            diagnostic:
                FixedDiagnostic::UnexpectedInput {
                    input: FixedSupervisorEvent::ProxyStopped(_),
                },
        } => assert_eq!(route, diagnostics),
        DiagnosticAction::Deliver { .. } | DiagnosticAction::Terminal { .. } => {
            panic!("one exact proxy exit cannot retire the member twice")
        }
    }
}

#[tokio::test]
async fn routed_preparation_failure_stops_the_complete_supervisor() {
    let diagnostics = EstablishedRecipient::<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, FallibleWorkshop>,
        >,
    >::issued(Endpoint(921));
    let recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut fixed, request, _, _) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::deliver_to(diagnostics.clone()),
    )
    .await;
    let failed = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(request),
        ))
        .unwrap_or_else(|_| panic!("the exact failure starts complete supervisor shutdown"));

    assert!(matches!(failed.become_, Step::Continue));
    match failed
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .expect("one routed diagnostic is emitted")
    {
        atomic::DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic::WorkerPreparationFailed(failure),
        } => {
            assert_eq!(route, diagnostics);
            assert!(matches!(
                failure.reason(),
                WorkerPreparationFailureReason::Unattempted
            ));
        }
        atomic::DiagnosticAction::Terminal { .. } => {
            panic!("a routed diagnostic cannot become terminal custody")
        }
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyOutcomeFailed(_),
            ..
        } => panic!("a preparation failure cannot become a proxy-outcome failure"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RecoveryDenied(_),
            ..
        } => panic!("a preparation failure cannot become a recovery denial"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::RestartScheduleFailed(_),
            ..
        } => panic!("a preparation failure cannot become a schedule failure"),
        atomic::DiagnosticAction::Deliver {
            diagnostic: FixedDiagnostic::ProxyInputRejected(_),
            ..
        } => panic!("a preparation failure cannot become an input rejection"),
        atomic::DiagnosticAction::Deliver {
            diagnostic:
                FixedDiagnostic::WorkerUnavailable(_) | FixedDiagnostic::UnexpectedInput { .. },
            ..
        } => panic!("a preparation failure cannot become an unavailable command"),
    }
    let shutdown = match failed
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("complete shutdown emits one proxy operation")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let (route, control, operation) = shutdown.into_parts();
    drop(control);
    let operation_settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact operation settlement remains in fleet custody"));
    assert!(matches!(operation_settled.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("the exact proxy exit closes complete shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

async fn preparing_one_for_one() -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            Infallible,
            Infallible,
        >,
    >,
    PrepareWorkers<SearchWorkshop, SearchRole, SearchWorker, SearchActivation>,
    CreationId,
) {
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    preparing_one_for_one_with_recovery(recovery).await
}

async fn preparing_one_for_one_with_recovery<Source>(
    recovery: Recovery<Source>,
) -> (
    Active<
        FixedSupervisor<SearchRole, SearchWorker, SearchActivation, Source, Infallible, Infallible>,
    >,
    PrepareWorkers<Source, SearchRole, SearchWorker, SearchActivation>,
    CreationId,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
{
    let (fixed, request, proxy, _) = preparing_one_for_one_with_policies(
        recovery,
        FailureReaction::StopSupervisor,
        DiagnosticDisposition::terminate(),
    )
    .await;
    (fixed, request, proxy)
}

async fn preparing_one_for_one_with_policies<Source, DiagnosticRoute>(
    recovery: Recovery<Source>,
    failure_reaction: FailureReaction,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            Source,
            DiagnosticRoute,
            Infallible,
        >,
    >,
    PrepareWorkers<Source, SearchRole, SearchWorker, SearchActivation>,
    CreationId,
    ChildStopped<RuntimeAddr>,
)
where
    Source: WorkerSource<SearchRole, SearchWorker, SearchActivation>,
    DiagnosticRoute: atomic::DiagnosticRoute<FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, Source>>
        + Clone,
{
    let (mut fixed, member) = ready_single_proxy_with_policies(
        recovery,
        failure_reaction,
        ActorDrainPolicy::WaitForActorGraph,
        diagnostics,
    )
    .await;
    let proxy = member.proxy;
    let stopped = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), Instant::now());
    let preparing = fixed
        .on(ChildReport::new(
            proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker,
                stopped,
            },
        ))
        .unwrap_or_else(|_| panic!("the exact stop starts preparation"));
    let request = match preparing
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one preparation request is emitted")
    {
        SettledItem::Unattempted(request) => request,
        SettledItem::Attempted(_) => panic!("the test intercepts an uninterpreted request"),
    };
    (fixed, request, proxy, stopped)
}

async fn delayed_schedule_with_policies<DiagnosticRoute>(
    failure_reaction: FailureReaction,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> (
    Active<
        FixedSupervisor<
            SearchRole,
            SearchWorker,
            SearchActivation,
            SearchWorkshop,
            DiagnosticRoute,
            Infallible,
        >,
    >,
    ScheduleAfter,
)
where
    DiagnosticRoute: atomic::DiagnosticRoute<
            FixedDiagnostic<SearchRole, SearchWorker, SearchActivation, SearchWorkshop>,
        > + Clone,
{
    let recovery = Recovery::permanent(
        SearchWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::constant(Duration::from_secs(3)).expect("the delay is positive"),
    );
    let (mut fixed, request, _, _) =
        preparing_one_for_one_with_policies(recovery, failure_reaction, diagnostics).await;
    let preparation = match request.accept(WorkerSubmission::activated(
        SearchWorker(SearchRole::Search),
        SearchActivation,
    )) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes preparation")
        }
    };
    let admitted = fixed
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the delayed recovery emits its schedule"));
    let schedule = match admitted
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one schedule is emitted")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the test intercepts the schedule before execution"),
    };
    (fixed, schedule)
}

#[tokio::test]
async fn another_recoverys_preparation_returns_unchanged_to_its_owner() {
    let (mut owner, owner_request, _) = preparing_one_for_one().await;
    let (mut source, source_request, _) = preparing_one_for_one().await;
    let source_submission =
        WorkerSubmission::activated(SearchWorker(SearchRole::Search), SearchActivation);
    let source_preparation = match source_request.accept(source_submission) {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => {
            panic!("one selected role completes in one preparation")
        }
    };

    let unexpected = owner
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(source_preparation)),
        ))
        .unwrap_or_else(|_| panic!("the foreign preparation becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )) => preparation,
        _ => panic!("another recovery ticket cannot advance this owner"),
    };
    let source_accepted = source
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(returned)),
        ))
        .unwrap_or_else(|_| panic!("the unchanged preparation remains valid for its owner"));
    assert!(matches!(source_accepted.become_, Step::Continue));
    drop(owner_request);
}

#[tokio::test]
async fn another_recoverys_source_rejection_returns_unchanged_to_its_owner() {
    let owner_recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let source_recovery = Recovery::permanent(
        FallibleWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    );
    let (mut owner, owner_request, _) = preparing_one_for_one_with_recovery(owner_recovery).await;
    let (mut source, source_request, _) =
        preparing_one_for_one_with_recovery(source_recovery).await;

    let unexpected = owner
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: source_request,
                reason: WorkshopRejection::SourceUnavailable,
            }),
        ))
        .unwrap_or_else(|_| panic!("the foreign rejection becomes a diagnostic"));
    let returned = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Rejected { item, reason },
        )) => {
            assert_eq!(reason, WorkshopRejection::SourceUnavailable);
            (item, reason)
        }
        _ => panic!("another recovery ticket cannot terminate this owner"),
    };
    let (returned, reason) = returned;
    let source_stopped = source
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: returned,
                reason,
            }),
        ))
        .unwrap_or_else(|_| panic!("the unchanged rejection remains valid for its owner"));
    assert!(matches!(source_stopped.become_, Step::Stop(_)));

    let owner_stopped = owner
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: owner_request,
                reason: WorkshopRejection::SourceUnavailable,
            }),
        ))
        .unwrap_or_else(|_| panic!("the owner still accepts its exact source rejection"));
    assert!(matches!(owner_stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn ready_roster_shutdown_accepts_proxy_exit_before_operation_settlement() {
    let (mut fixed, _) = ready_single_proxy_with_worker().await;
    let shutting_down = fixed
        .receive(RuntimeAddr(910), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("a ready roster accepts shutdown"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one exact proxy shutdown is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let creation = shutdown.creation();
    let waiting = fixed
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("an early exact proxy exit remains pending"));
    assert!(matches!(waiting.become_, Step::Continue));

    let (route, control, shutdown) = shutdown.into_parts();
    drop(control);
    let stopped = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                shutdown,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact settlement closes fleet shutdown"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn ready_roster_shutdown_retires_after_rejection_and_exact_proxy_exit() {
    let (mut fixed, _) = ready_single_proxy_with_worker().await;
    let shutting_down = fixed
        .receive(RuntimeAddr(920), atomic::FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("a ready roster accepts shutdown"));
    let shutdown = match shutting_down
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one exact proxy shutdown is dispatched")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => {
            panic!("the test intercepts an uninterpreted shutdown")
        }
    };
    let creation = shutdown.creation();
    let rejected = fixed
        .on(SettledItem::Unattempted(shutdown))
        .unwrap_or_else(|_| panic!("the exact rejection remains in fleet custody"));
    assert!(matches!(rejected.become_, Step::Continue));
    let stopped = fixed
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact exit completes proxy retirement"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn rejected_actor_graph_deadline_transfers_the_unresolved_proxy() {
    let (mut fixed, worker) = ready_single_proxy_with_policies(
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(30),
        },
        DiagnosticDisposition::terminate(),
    )
    .await;
    drop(worker);
    let shutdown = fixed
        .receive(RuntimeAddr(930), FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("a ready roster starts its actor-graph deadline"));
    assert!(matches!(shutdown.become_, Step::Continue));
    assert_eq!(
        shutdown
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .len(),
        1
    );
    let schedule = match shutdown
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs()
        .pop()
        .expect("deadline policy emits one relative schedule")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the test intercepts the schedule"),
    };
    assert_eq!(schedule.id, TimerId(0));
    assert_eq!(schedule.generation, TimerGeneration(0));
    assert_eq!(schedule.after, Duration::from_secs(30));

    let repeated = fixed
        .receive(RuntimeAddr(931), FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("repeated shutdown retains the same deadline"));
    assert!(
        repeated
            .sends
            .restart_schedules
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    assert!(
        repeated
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .is_empty()
    );

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                id: TimerId(1),
                generation: schedule.generation,
            })),
        ))
        .unwrap_or_else(|_| panic!("the foreign receipt becomes a diagnostic"));
    let foreign = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartScheduleSettled(input) => input,
        _ => panic!("a foreign receipt cannot force actor transfer"),
    };
    assert!(matches!(
        foreign,
        SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
            id: TimerId(1),
            generation: TimerGeneration(0),
        }))
    ));

    let forced = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::QueueSequenceExhausted,
            }),
        ))
        .unwrap_or_else(|_| panic!("the exact rejection forces actor transfer"));
    assert!(matches!(forced.become_, Step::Stop(_)));
}

#[tokio::test]
async fn accepted_actor_graph_deadline_requires_its_exact_elapsed_timer() {
    let (mut fixed, worker) = ready_single_proxy_with_policies(
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(45),
        },
        DiagnosticDisposition::terminate(),
    )
    .await;
    drop(worker);
    let shutdown = fixed
        .receive(RuntimeAddr(940), FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("a ready roster starts its actor-graph deadline"));
    let schedule = match shutdown
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs()
        .pop()
        .expect("deadline policy emits one relative schedule")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the test intercepts the schedule"),
    };

    let waiting = fixed
        .transition(FixedSupervisorEvent::RestartScheduleSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            })),
        ))
        .unwrap_or_else(|_| panic!("the exact receipt starts deadline waiting"));
    assert!(matches!(waiting.become_, Step::Continue));

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
            TimerId(1),
            schedule.generation,
        )))
        .unwrap_or_else(|_| panic!("the foreign timer becomes a diagnostic"));
    let foreign = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartElapsed(elapsed) => elapsed,
        _ => panic!("a foreign timer cannot force actor transfer"),
    };
    assert_eq!(foreign.id, TimerId(1));
    assert_eq!(foreign.generation, schedule.generation);

    let forced = fixed
        .transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
            schedule.id,
            schedule.generation,
        )))
        .unwrap_or_else(|_| panic!("the exact elapsed timer forces actor transfer"));
    assert!(matches!(forced.become_, Step::Stop(_)));

    let unexpected = fixed
        .transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
            schedule.id,
            schedule.generation,
        )))
        .unwrap_or_else(|_| panic!("the duplicate timer becomes a diagnostic"));
    assert!(matches!(unexpected.become_, Step::Stop(_)));
    let duplicate = match terminal_unexpected(unexpected.sends.diagnostics.into_requests()) {
        FixedSupervisorEvent::RestartElapsed(elapsed) => elapsed,
        _ => panic!("one deadline cannot force actor transfer twice"),
    };
    assert_eq!(duplicate.id, schedule.id);
    assert_eq!(duplicate.generation, schedule.generation);
}

#[tokio::test]
async fn ready_management_queries_return_the_stable_proxy_without_mutating_the_roster() {
    let (mut fixed, _) = ready_single_proxy_with_worker().await;
    let status = fixed
        .receive(
            RuntimeAddr(950),
            FixedCommand::status(Recipient::global(RuntimeAddr(951))),
        )
        .unwrap_or_else(|_| panic!("a ready roster accepts a status query"));
    let status = logical_reply(status.sends.status_replies.into_deliveries());
    let proxy = match status.members.as_slice() {
        [MemberStatus::Ready { proxy }] => proxy.clone(),
        _ => panic!("one ready role produces one ready status"),
    };

    let capability = fixed
        .receive(
            RuntimeAddr(952),
            FixedCommand::capability(
                SearchRole::Search,
                EstablishedRecipient::issued(Endpoint(953)),
            ),
        )
        .unwrap_or_else(|_| panic!("status leaves the ready roster unchanged"));
    let capability = established_reply(capability.sends.capability_replies.into_deliveries());
    match capability {
        CapabilityResult::Ready {
            role: SearchRole::Search,
            proxy: returned,
        } => assert_eq!(returned, proxy),
        CapabilityResult::Ready { .. }
        | CapabilityResult::Unavailable { .. }
        | CapabilityResult::UnknownRole { .. } => {
            panic!("the ready role returns its stable proxy")
        }
    }

    let unknown = fixed
        .receive(
            RuntimeAddr(954),
            FixedCommand::capability(SearchRole::Index, Recipient::global(RuntimeAddr(955))),
        )
        .unwrap_or_else(|_| panic!("an unknown role is a successful read-only query"));
    let unknown = logical_reply(unknown.sends.capability_replies.into_deliveries());
    assert!(matches!(
        unknown,
        CapabilityResult::UnknownRole {
            submitted: SearchRole::Index
        }
    ));
}

#[test]
fn pending_proxy_births_are_observable_as_creating_proxy() {
    let initialized = fixed(
        initial_worker,
        roles(),
        ActivationPolicy::new(3).expect("three proxies may start together"),
        Recovery::temporary(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build::<SearchWorker, SearchActivation, Never>()
    .unwrap_or_else(|_| panic!("every initial worker prepares"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed initialization emits pending proxy births"));
    let mut fixed = initialized.behavior;
    let pending_creations = initialized.actions.creates;
    assert_eq!(pending_creations.len(), 3);

    let status = fixed
        .receive(
            RuntimeAddr(956),
            FixedCommand::status(Recipient::global(RuntimeAddr(957))),
        )
        .unwrap_or_else(|_| panic!("pending proxy births remain queryable"));
    let status = logical_reply(status.sends.status_replies.into_deliveries());
    assert!(matches!(
        status.members.as_slice(),
        [
            MemberStatus::CreatingProxy,
            MemberStatus::CreatingProxy,
            MemberStatus::CreatingProxy,
        ]
    ));

    let capability = fixed
        .receive(
            RuntimeAddr(958),
            FixedCommand::capability(SearchRole::Search, Recipient::global(RuntimeAddr(959))),
        )
        .unwrap_or_else(|_| panic!("pending proxy capability remains queryable"));
    let capability = logical_reply(capability.sends.capability_replies.into_deliveries());
    assert!(matches!(
        capability,
        CapabilityResult::Unavailable {
            role: SearchRole::Search,
            phase: atomic::UnavailablePhase::CreatingProxy,
        }
    ));
    assert_eq!(pending_creations.len(), 3);
}

#[tokio::test]
async fn draining_management_queries_distinguish_retired_and_stopping_roles() {
    let (mut fixed, _) = ready_three_role_roster(Strategy::OneForOne, THREE_WORKERS).await;
    let shutdown = fixed
        .receive(RuntimeAddr(960), FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("the ready roster starts draining"));
    let mut operations = shutdown.sends.proxy_operations.unattempted().into_inputs();
    assert_eq!(operations.len(), 3);
    let first = match operations.remove(0) {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the test intercepts the first shutdown"),
    };
    let (route, control, operation) = first.into_parts();
    drop(control);
    let settled = fixed
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<SearchWorker, SearchActivation>>::issued(Endpoint(
                    801,
                )),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the first shutdown settlement remains draining"));
    assert!(matches!(settled.become_, Step::Continue));
    let exited = fixed
        .on(ChildStopped::new(route, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|_| panic!("two live proxies keep the supervisor draining"));
    assert!(matches!(exited.become_, Step::Continue));

    let status = fixed
        .receive(
            RuntimeAddr(961),
            FixedCommand::status(Recipient::global(RuntimeAddr(962))),
        )
        .unwrap_or_else(|_| panic!("draining accepts status queries"));
    let status = logical_reply(status.sends.status_replies.into_deliveries());
    assert!(matches!(
        status.members.as_slice(),
        [
            MemberStatus::Retired,
            MemberStatus::Stopping,
            MemberStatus::Stopping
        ]
    ));

    let capability = fixed
        .receive(
            RuntimeAddr(963),
            FixedCommand::capability(SearchRole::Search, Recipient::global(RuntimeAddr(964))),
        )
        .unwrap_or_else(|_| panic!("draining accepts capability queries"));
    let capability = logical_reply(capability.sends.capability_replies.into_deliveries());
    assert!(matches!(
        capability,
        CapabilityResult::Unavailable {
            role: SearchRole::Search,
            phase: atomic::UnavailablePhase::Retired,
        }
    ));
}

#[test]
fn management_status_preserves_semantic_order_during_proxy_startup() {
    let (mut fixed, first_operation) = first_proxy_dispatched(701);
    let status = fixed
        .receive(
            RuntimeAddr(970),
            FixedCommand::status(Recipient::global(RuntimeAddr(971))),
        )
        .unwrap_or_else(|_| panic!("startup accepts status queries"));
    let status = logical_reply(status.sends.status_replies.into_deliveries());
    assert!(matches!(
        status.members.as_slice(),
        [
            MemberStatus::AwaitingProxy,
            MemberStatus::WaitingForActivation,
            MemberStatus::WaitingForActivation,
        ]
    ));
    drop(first_operation);
}

#[tokio::test]
async fn management_queries_project_coordinated_recovery_and_empty_roles() {
    let (mut recovering, request, _) =
        coordinated_preparation(Strategy::OneForAll, THREE_WORKERS, 1).await;
    let recovery_status = recovering
        .receive(
            RuntimeAddr(980),
            FixedCommand::status(Recipient::global(RuntimeAddr(981))),
        )
        .unwrap_or_else(|_| panic!("coordinated recovery accepts status queries"));
    let recovery_status = logical_reply(recovery_status.sends.status_replies.into_deliveries());
    assert!(matches!(
        recovery_status.members.as_slice(),
        [
            MemberStatus::Recovering,
            MemberStatus::Recovering,
            MemberStatus::Recovering,
        ]
    ));
    let recovery_capability = recovering
        .receive(
            RuntimeAddr(982),
            FixedCommand::capability(SearchRole::Index, Recipient::global(RuntimeAddr(983))),
        )
        .unwrap_or_else(|_| panic!("coordinated recovery accepts capability queries"));
    let recovery_capability = logical_reply(
        recovery_capability
            .sends
            .capability_replies
            .into_deliveries(),
    );
    assert!(matches!(
        recovery_capability,
        CapabilityResult::Unavailable {
            role: SearchRole::Index,
            phase: atomic::UnavailablePhase::Recovering,
        }
    ));
    drop(request);

    let (mut empty, member) = ready_single_proxy_with_worker().await;
    let stopped = ChildStopped::new(member.worker.creation(), Ok(Exit::Normal), Instant::now());
    let emptied = empty
        .on(ChildReport::new(
            member.proxy,
            ProxyOutcome::WorkerStopped {
                worker: member.worker,
                stopped,
            },
        ))
        .unwrap_or_else(|_| panic!("temporary recovery leaves the role empty"));
    assert!(matches!(emptied.become_, Step::Continue));
    assert!(
        emptied
            .sends
            .worker_preparations
            .unattempted()
            .into_inputs()
            .is_empty()
    );
    let empty_status = empty
        .receive(
            RuntimeAddr(984),
            FixedCommand::status(Recipient::global(RuntimeAddr(985))),
        )
        .unwrap_or_else(|_| panic!("an empty role remains queryable"));
    let empty_status = logical_reply(empty_status.sends.status_replies.into_deliveries());
    assert!(matches!(
        empty_status.members.as_slice(),
        [MemberStatus::Empty]
    ));
}

#[tokio::test]
async fn one_role_recovery_distinguishes_an_undeclared_capability_role() {
    let (mut preparing, preparation, _) = preparing_one_for_one().await;
    let known = preparing
        .receive(
            RuntimeAddr(986),
            FixedCommand::capability(SearchRole::Search, Recipient::global(RuntimeAddr(987))),
        )
        .unwrap_or_else(|_| panic!("preparation recognizes its declared role"));
    let known = logical_reply(known.sends.capability_replies.into_deliveries());
    assert!(matches!(
        known,
        CapabilityResult::Unavailable {
            role: SearchRole::Search,
            phase: atomic::UnavailablePhase::Recovering,
        }
    ));
    let unknown = preparing
        .receive(
            RuntimeAddr(988),
            FixedCommand::capability(SearchRole::Index, Recipient::global(RuntimeAddr(989))),
        )
        .unwrap_or_else(|_| panic!("preparation answers the undeclared role query"));
    assert!(unknown.sends.diagnostics.is_empty());
    let unknown = logical_reply(unknown.sends.capability_replies.into_deliveries());
    assert!(matches!(
        unknown,
        CapabilityResult::UnknownRole {
            submitted: SearchRole::Index,
        }
    ));
    drop(preparation);

    let (mut admitted, replacement, _) = one_replacement_operation().await;
    let known = admitted
        .receive(
            RuntimeAddr(990),
            FixedCommand::capability(SearchRole::Search, Recipient::global(RuntimeAddr(991))),
        )
        .unwrap_or_else(|_| panic!("admitted recovery recognizes its declared role"));
    let known = logical_reply(known.sends.capability_replies.into_deliveries());
    assert!(matches!(
        known,
        CapabilityResult::Unavailable {
            role: SearchRole::Search,
            phase: atomic::UnavailablePhase::Recovering,
        }
    ));
    let unknown = admitted
        .receive(
            RuntimeAddr(992),
            FixedCommand::capability(SearchRole::Index, Recipient::global(RuntimeAddr(993))),
        )
        .unwrap_or_else(|_| panic!("admitted recovery answers the undeclared role query"));
    assert!(unknown.sends.diagnostics.is_empty());
    let unknown = logical_reply(unknown.sends.capability_replies.into_deliveries());
    assert!(matches!(
        unknown,
        CapabilityResult::UnknownRole {
            submitted: SearchRole::Index,
        }
    ));
    drop(replacement);
}
