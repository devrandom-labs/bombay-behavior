use core::ops::ControlFlow;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use behavior::{
    ActionItemResult, Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase,
    ChildCreationOutcome, ChildHead, ChildReport, CreationId, CreationSequence, CreationSettlement,
    CreationsSettled, EndpointAddress, EstablishedCreation, EstablishedRecipient,
    ExactDeliveryReason, InterpreterFault, InterpreterRequests, ItemSettlement, MessageProtocol,
    Never, NoBirths, Protocol, Recipient, ReportToParent, SettledItem, Step, User,
};
use behavior_actors::atomic::{
    ActivationPlan, ActivationPolicy, ActivationStartRejection, ActorDrainPolicy,
    AdmissionRejection, AssignWorker, AssignedReturnReason, Assignment, BacklogCapacity,
    BeginActivation, Completion, DiagnosticAction, DiagnosticDisposition, FifoCommand, FifoEvent,
    FifoOutcome, FifoOutcomeKind, FifoPool, ImmediateActivation, Interruption, JobId, OrderedRoles,
    PoolFailureReaction, PoolRecovery, PrepareWorkers, QueuedReturnReason, RestartLimit,
    RestartRelease, SubmissionId, WorkerInitializationFailure, WorkerInitializationOutcome,
    WorkerSource, WorkerSubmission, fifo,
};
use behavior_actors::{
    Activate as _, Active, ChildStopped, Crash, EstablishedShutdownResolved, Exit, ReplyDelivery,
    ScheduleAfter, ScheduleAfterRejection, ShutdownRejection, StopOnShutdown,
    SupervisionFailureReason, TimerElapsed, TimerId, TimerScheduled,
};

#[path = "fifo_pool/compile.rs"]
mod compile;
#[path = "fifo_pool/correlation.rs"]
mod correlation;
#[path = "fifo_pool/customer.rs"]
mod customer;
mod direct_pool_customer;
#[path = "fifo_pool/property.rs"]
mod property;
#[path = "fifo_pool/recovery.rs"]
mod recovery;
#[path = "fifo_pool/retirement.rs"]
mod retirement;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Search,
    Index,
}

#[derive(Debug, Eq, PartialEq)]
struct SearchWorker;

impl Protocol for SearchWorker {
    type Addr = RuntimeAddr;
    type Msg = Assignment<u8>;
}

impl BehaviorBase for SearchWorker {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for SearchWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Assignment<u8>>;
    type Sends = InterpreterRequests<ReportToParent<Completion<u16>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, assignment: Self::Event) -> BehaviorActed<Self> {
        let worker_result = u16::from(*assignment.message.payload());
        Ok(Actions::cont().with_send(assignment.message.complete(worker_result)))
    }
}

fn prepare_search_worker(
    _: &Role,
) -> Result<WorkerSubmission<SearchWorker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(SearchWorker))
}

#[derive(Debug, Eq, PartialEq)]
struct SearchSource;

impl WorkerSource<Role, SearchWorker, ImmediateActivation> for SearchSource {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchWorkerRejection {
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchSourceRejection {
    Closed,
}

#[derive(Debug, Eq, PartialEq)]
struct FallibleSearchSource;

impl WorkerSource<Role, SearchWorker, ImmediateActivation> for FallibleSearchSource {
    type WorkerRejection = SearchWorkerRejection;
    type SourceRejection = SearchSourceRejection;
}

struct TrackedSearchSource(Arc<AtomicUsize>);

impl Drop for TrackedSearchSource {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct TrackedSourceRejection(Arc<AtomicUsize>);

impl Drop for TrackedSourceRejection {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl WorkerSource<Role, SearchWorker, ImmediateActivation> for TrackedSearchSource {
    type WorkerRejection = Never;
    type SourceRejection = TrackedSourceRejection;
}

struct HeldActivation(Arc<AtomicUsize>);

impl Drop for HeldActivation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl WorkerSource<Role, SearchWorker, HeldActivation> for SearchSource {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

struct SearchActivationRejection(Arc<AtomicUsize>);

impl Drop for SearchActivationRejection {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct RejectedSearchActivation(Arc<AtomicUsize>);

impl ActivationPlan for RejectedSearchActivation {
    type Ready = Never;
    type Rejection = SearchActivationRejection;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Err(SearchActivationRejection(self.0)))
    }
}

impl ActivationPlan for HeldActivation {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        async move {
            drop(self);
            Ok(())
        }
    }
}

fn created_worker(
    creation: behavior::CreateChild<RuntimeAddr, StopOnShutdown<SearchWorker>>,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<SearchWorker>> {
    let (worker, _, kind) = creation.into_parts();
    let endpoint = Endpoint(40 + worker.get());
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::<StopOnShutdown<SearchWorker>, ChildHead>::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(endpoint),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

struct ReadySearchPool<Source>
where
    Source: WorkerSource<Role, SearchWorker, ImmediateActivation>,
{
    pool: Active<FifoPool<Role, SearchWorker, ImmediateActivation, Source, Infallible, u8, u16>>,
    workers: Vec<CreationId>,
}

async fn ready_search_pool<Source>(
    roles: OrderedRoles<Role>,
    backlog: BacklogCapacity,
    interruption: Interruption,
    recovery: PoolRecovery<Source>,
) -> ReadySearchPool<Source>
where
    Source: WorkerSource<Role, SearchWorker, ImmediateActivation>,
{
    ready_search_pool_with_drain(
        roles,
        backlog,
        interruption,
        recovery,
        ActorDrainPolicy::WaitForActorGraph,
    )
    .await
}

async fn ready_search_pool_with_drain<Source>(
    roles: OrderedRoles<Role>,
    backlog: BacklogCapacity,
    interruption: Interruption,
    recovery: PoolRecovery<Source>,
    actor_drain: ActorDrainPolicy,
) -> ReadySearchPool<Source>
where
    Source: WorkerSource<Role, SearchWorker, ImmediateActivation>,
{
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        recovery,
        backlog,
        interruption,
        actor_drain,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut workers = Vec::new();
    for creation in initialized.actions.creates {
        let committed = pool
            .on(created_worker(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = committed
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("created worker awaits initialization"));
        workers.push(initialization.worker().creation());
        let initialized_worker = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized_worker
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("initialized worker begins activation"));
        let activation_started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("activation start failed: {error}"));
        assert!(activation_started.sends.worker_assignments.is_empty());
        let worker_ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(worker_ready.sends.worker_assignments.is_empty());
    }
    ReadySearchPool { pool, workers }
}

fn pending_search_activation<P>(
    activation: P,
) -> (
    Active<FifoPool<Role, SearchWorker, P, Never, Infallible, u8, u16>>,
    CreationId,
    BeginActivation<SearchWorker, P>,
)
where
    P: ActivationPlan,
{
    pending_search_activation_with_drain(activation, ActorDrainPolicy::WaitForActorGraph)
}

fn pending_search_activation_with_drain<P>(
    activation: P,
    actor_drain: ActorDrainPolicy,
) -> (
    Active<FifoPool<Role, SearchWorker, P, Never, Infallible, u8, u16>>,
    CreationId,
    BeginActivation<SearchWorker, P>,
)
where
    P: ActivationPlan,
{
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let mut submission = Some(WorkerSubmission::activated(SearchWorker, activation));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(
                submission
                    .take()
                    .unwrap_or_else(|| panic!("one-role fixture prepares one worker")),
            )
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        actor_drain,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    (pool, worker, activation)
}

async fn pool_awaiting_worker_preparation<Source>(
    source: Source,
) -> (
    Active<FifoPool<Role, SearchWorker, ImmediateActivation, Source, Infallible, u8, u16>>,
    PrepareWorkers<Source, Role, SearchWorker, ImmediateActivation>,
)
where
    Source: WorkerSource<Role, SearchWorker, ImmediateActivation>,
{
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            source,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let stopped = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("recovering worker exit input failed: {error}"));
    let preparation = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown stops the remaining live worker"));
    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(settled.become_, Step::Continue));
    let workers_retired = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("remaining worker exit input failed: {error}"));
    assert!(matches!(workers_retired.become_, Step::Continue));
    (pool, preparation)
}

fn stop_search_worker<Source>(
    pool: &mut Active<
        FifoPool<Role, SearchWorker, ImmediateActivation, Source, Infallible, u8, u16>,
    >,
    worker: CreationId,
) -> PrepareWorkers<Source, Role, SearchWorker, ImmediateActivation>
where
    Source: WorkerSource<Role, SearchWorker, ImmediateActivation>,
{
    pool.on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"))
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"))
}

fn assert_search_pool_unavailable<P, Source>(
    pool: &mut Active<FifoPool<Role, SearchWorker, P, Source, Infallible, u8, u16>>,
    submission: u64,
) where
    P: ActivationPlan,
    Source: WorkerSource<Role, SearchWorker, P>,
{
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let rejected = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(submission), 41, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    assert!(rejected.sends.worker_assignments.is_empty());
    let mut outcomes = rejected.sends.customer_outcomes.into_deliveries();
    assert_eq!(outcomes.len(), 1);
    let outcome = match outcomes
        .pop()
        .unwrap_or_else(|| panic!("retired one-role pool rejects new work"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    let (_, payload, reason) = outcome
        .into_rejected()
        .unwrap_or_else(|_| panic!("the unavailable pool returns a rejection"));
    assert_eq!(payload, 41);
    assert_eq!(reason, AdmissionRejection::NoRecoverableWorkers);
}

async fn scheduling_search_replacement() -> (
    Active<FifoPool<Role, SearchWorker, ImmediateActivation, SearchSource, Infallible, u8, u16>>,
    ScheduleAfter,
) {
    scheduling_search_replacement_with_drain(ActorDrainPolicy::WaitForActorGraph).await
}

async fn scheduling_search_replacement_with_drain(
    actor_drain: ActorDrainPolicy,
) -> (
    Active<FifoPool<Role, SearchWorker, ImmediateActivation, SearchSource, Infallible, u8, u16>>,
    ScheduleAfter,
) {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool_with_drain(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive restart delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
        actor_drain,
    )
    .await;
    let request = stop_search_worker(
        &mut pool,
        workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists")),
    );
    let ControlFlow::Break(preparation) = request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let scheduling = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"));
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));
    (pool, schedule)
}

async fn draining_search_restart_schedule() -> (
    Active<FifoPool<Role, SearchWorker, ImmediateActivation, SearchSource, Infallible, u8, u16>>,
    ScheduleAfter,
) {
    let (mut pool, schedule) = scheduling_search_replacement().await;
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(draining.become_, Step::Continue));
    assert!(draining.sends.diagnostics.is_empty());
    (pool, schedule)
}

fn pending_creation_with_deadline(
    activation_drops: Arc<AtomicUsize>,
) -> (
    Active<FifoPool<Role, SearchWorker, HeldActivation, Never, Infallible, u8, u16>>,
    ScheduleAfter,
) {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let mut activation = Some(HeldActivation(activation_drops));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                activation
                    .take()
                    .unwrap_or_else(|| panic!("one-role fixture prepares one worker")),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        },
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creations = initialized.actions.creates;
    assert_eq!(creations.len(), 1);
    drop(creations);
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(draining.become_, Step::Continue));
    let deadline = draining
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded drain schedules one deadline"));
    (pool, deadline)
}

#[test]
fn canonical_fifo_domain_surface_exists_without_structural_types() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(4),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let _pool = initialized.behavior;
    let _: Option<JobId> = None;
    let _: Option<SubmissionId> = None;
    let _: Option<FifoCommand<RuntimeAddr, Role, u8, u16>> = None;
    let _: Option<FifoOutcome<Role, u8, u16>> = None;
}

#[test]
fn construction_rejection_returns_the_worker_roster() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum WorkerRole {
        Search,
        Index,
        Spellcheck,
    }

    let calls = Rc::new(RefCell::new(Vec::new()));
    let factory_calls = Rc::clone(&calls);
    let roles = OrderedRoles::new(
        WorkerRole::Search,
        [WorkerRole::Index, WorkerRole::Spellcheck],
    )
    .unwrap_or_else(|_| panic!("worker roles are unique"));
    let activation = ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid"));

    let mut rejection = match fifo(
        move |role: &WorkerRole| {
            factory_calls.borrow_mut().push(*role);
            match role {
                WorkerRole::Index => Err(SearchWorkerRejection::Unavailable),
                WorkerRole::Search | WorkerRole::Spellcheck => {
                    Ok(WorkerSubmission::immediate(SearchWorker))
                }
            }
        },
        roles,
        activation,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(4),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    ) {
        Ok(_) => panic!("index worker preparation must reject"),
        Err(rejection) => rejection,
    };

    assert_eq!(*calls.borrow(), [WorkerRole::Search, WorkerRole::Index]);
    assert_eq!(rejection.workers.prepared.len(), 1);
    assert_eq!(rejection.workers.prepared[0].role, WorkerRole::Search);
    assert_eq!(rejection.workers.role, WorkerRole::Index);
    assert_eq!(rejection.workers.reason, SearchWorkerRejection::Unavailable);
    assert_eq!(rejection.workers.remaining, [WorkerRole::Spellcheck]);
    let returned = (rejection.workers.factory)(&WorkerRole::Spellcheck);
    assert_eq!(returned, Ok(WorkerSubmission::immediate(SearchWorker)));
}

#[test]
fn initialization_stop_completes_drain_without_a_second_stop_observation() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let factory_drops = Arc::clone(&activation_drops);
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                HeldActivation(Arc::clone(&factory_drops)),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(draining.become_, Step::Continue));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"));
    let shutdown_settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(shutdown_settled.become_, Step::Continue));

    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let retired = pool
        .on(initialization.resolve(WorkerInitializationOutcome::Stopped(stopped)))
        .unwrap_or_else(|error| panic!("initialization stop failed: {error}"));

    assert!(matches!(retired.become_, Step::Stop(_)));
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert!(retired.sends.worker_activations.is_empty());
    assert!(retired.sends.worker_shutdowns.is_empty());
    assert!(retired.creates.is_empty());
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(retired);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn drain_advances_started_activation_without_restoring_eligibility() {
    let (mut pool, worker, activation) = pending_search_activation(ImmediateActivation);

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"));
    let awaiting_exit = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(awaiting_exit.become_, Step::Continue));

    let first_started = activation.started();
    let duplicate_started = activation.started();
    let started = pool
        .on(first_started)
        .unwrap_or_else(|error| panic!("late activation start failed: {error}"));
    assert!(started.sends.diagnostics.is_empty());
    assert!(started.sends.worker_assignments.is_empty());
    let duplicate = pool
        .on(duplicate_started)
        .unwrap_or_else(|error| panic!("duplicate activation start failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.worker_assignments.is_empty());
    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("late activation readiness failed: {error}"));
    assert_eq!(ready.sends.diagnostics.len(), 1);
    assert!(ready.sends.worker_assignments.is_empty());

    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert!(retired.sends.worker_assignments.is_empty());
}

#[tokio::test]
async fn foreign_drain_activation_cannot_advance_the_worker() {
    let (mut pool, worker, activation) = pending_search_activation(ImmediateActivation);
    let (_foreign_pool, _foreign_worker, foreign_activation) =
        pending_search_activation(ImmediateActivation);

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"));

    let foreign = pool
        .on(foreign_activation.started())
        .unwrap_or_else(|error| panic!("foreign activation input failed: {error}"));
    assert_eq!(foreign.sends.diagnostics.len(), 1);
    assert!(foreign.sends.worker_assignments.is_empty());

    let exact = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("exact activation input failed: {error}"));
    assert!(exact.sends.diagnostics.is_empty());
    assert!(exact.sends.worker_assignments.is_empty());

    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(matches!(stopped.become_, Step::Continue));
    let retired = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
}

#[tokio::test]
async fn drain_retains_activation_start_rejection_without_assigning_work() {
    let (mut pool, worker, activation) = pending_search_activation(ImmediateActivation);

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"));

    let rejected = pool
        .on(activation.start_rejected(ActivationStartRejection::OwnerStopped))
        .unwrap_or_else(|error| panic!("activation start rejection failed: {error}"));
    assert_eq!(rejected.sends.diagnostics.len(), 1);
    assert!(rejected.sends.worker_assignments.is_empty());

    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(settled.become_, Step::Continue));
    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
}

#[tokio::test]
async fn drain_retains_activation_plan_rejection_without_assigning_work() {
    let rejection_drops = Arc::new(AtomicUsize::new(0));
    let (mut pool, worker, activation) =
        pending_search_activation(RejectedSearchActivation(Arc::clone(&rejection_drops)));

    let started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(started.sends.diagnostics.is_empty());
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"));

    let rejected = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("activation rejection failed: {error}"));
    assert_eq!(rejected.sends.diagnostics.len(), 1);
    assert!(rejected.sends.worker_assignments.is_empty());
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 0);
    drop(rejected);
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 1);

    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(settled.become_, Step::Continue));
    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
}

#[tokio::test]
async fn shutdown_waits_for_exact_worker_preparation_without_restarting() {
    let (mut pool, preparation) = pool_awaiting_worker_preparation(SearchSource).await;

    let ControlFlow::Break(prepared) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let returned = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(prepared),
        )))
        .unwrap_or_else(|error| panic!("late worker preparation failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());
    assert!(returned.sends.worker_preparations.is_empty());
}

#[tokio::test]
async fn foreign_worker_preparation_cannot_release_the_exact_drain() {
    let (mut pool, exact) = pool_awaiting_worker_preparation(SearchSource).await;
    let (_foreign_pool, foreign) = pool_awaiting_worker_preparation(SearchSource).await;

    let unrelated = pool
        .transition(FifoEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(foreign),
        ))
        .unwrap_or_else(|error| panic!("foreign worker preparation failed: {error}"));
    assert!(matches!(unrelated.become_, Step::Continue));
    assert_eq!(unrelated.sends.diagnostics.len(), 1);
    assert!(unrelated.creates.is_empty());

    let returned = pool
        .transition(FifoEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(exact),
        ))
        .unwrap_or_else(|error| panic!("exact unattempted preparation failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());
}

#[tokio::test]
async fn worker_rejection_during_drain_returns_source_without_recovery() {
    let (mut pool, preparation) = pool_awaiting_worker_preparation(FallibleSearchSource).await;
    let rejected = preparation.reject(SearchWorkerRejection::Unavailable);

    let returned = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(rejected),
        )))
        .unwrap_or_else(|error| panic!("worker preparation rejection failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());
    assert!(returned.sends.worker_preparations.is_empty());
}

#[tokio::test]
async fn source_rejection_during_drain_preserves_affine_custody() {
    let source_drops = Arc::new(AtomicUsize::new(0));
    let reason_drops = Arc::new(AtomicUsize::new(0));
    let (mut pool, preparation) =
        pool_awaiting_worker_preparation(TrackedSearchSource(Arc::clone(&source_drops))).await;

    let returned = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: preparation,
                reason: TrackedSourceRejection(Arc::clone(&reason_drops)),
            },
        )))
        .unwrap_or_else(|error| panic!("worker source rejection failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    assert_eq!(reason_drops.load(Ordering::SeqCst), 0);

    drop(returned);
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    assert_eq!(reason_drops.load(Ordering::SeqCst), 1);
    drop(pool);
    assert_eq!(source_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn corrupt_worker_preparation_during_drain_cannot_restart() {
    let (mut pool, preparation) = pool_awaiting_worker_preparation(FallibleSearchSource).await;

    let returned = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Corrupt {
                item: preparation,
                fault: InterpreterFault::CorruptTraversal,
            },
        )))
        .unwrap_or_else(|error| panic!("corrupt worker preparation failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());
    assert!(returned.sends.worker_preparations.is_empty());
}

#[tokio::test]
async fn shutdown_waits_for_exact_restart_schedule_settlement() {
    let (mut pool, schedule) = draining_search_restart_schedule().await;
    let scheduled = TimerScheduled {
        id: schedule.id,
        generation: schedule.generation,
    };

    let returned = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("restart schedule settlement failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());

    let duplicate = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("duplicate restart schedule failed: {error}"));
    assert!(matches!(duplicate.become_, Step::Stop(_)));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.creates.is_empty());
}

#[tokio::test]
async fn rejected_restart_schedule_during_drain_returns_the_replacement() {
    let (mut pool, schedule) = draining_search_restart_schedule().await;

    let returned = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("restart schedule rejection failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
}

#[tokio::test]
async fn corrupt_restart_schedule_during_drain_returns_the_replacement() {
    let (mut pool, schedule) = draining_search_restart_schedule().await;

    let returned = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Corrupt {
                item: schedule,
                fault: InterpreterFault::CorruptTraversal,
            },
        )))
        .unwrap_or_else(|error| panic!("corrupt restart schedule failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
}

#[tokio::test]
async fn unattempted_restart_schedule_during_drain_returns_the_replacement() {
    let (mut pool, schedule) = draining_search_restart_schedule().await;

    let returned = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Unattempted(
            schedule,
        )))
        .unwrap_or_else(|error| panic!("unattempted restart schedule failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
}

#[tokio::test]
async fn foreign_restart_schedule_during_drain_cannot_return_the_replacement() {
    let (mut pool, schedule) = draining_search_restart_schedule().await;
    let foreign = TimerScheduled {
        id: TimerId(schedule.id.0 + 1),
        generation: schedule.generation,
    };

    let unrelated = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(foreign),
        )))
        .unwrap_or_else(|error| panic!("foreign restart schedule failed: {error}"));
    assert!(matches!(unrelated.become_, Step::Continue));
    assert_eq!(unrelated.sends.diagnostics.len(), 1);

    let returned = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("exact restart schedule failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert_eq!(returned.sends.diagnostics.len(), 1);
    assert!(returned.creates.is_empty());
}

#[tokio::test]
async fn shutdown_cancels_an_accepted_restart_timer_without_waiting_for_elapsed() {
    let (mut pool, schedule) = scheduling_search_replacement().await;
    let waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("restart schedule settlement failed: {error}"));
    assert!(matches!(waiting.become_, Step::Continue));

    let retired = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert!(retired.creates.is_empty());
    assert!(retired.sends.restart_schedules.is_empty());

    let stale = pool
        .on(TimerElapsed::new(schedule.id, schedule.generation))
        .unwrap_or_else(|error| panic!("stale restart timer input failed: {error}"));
    assert!(matches!(stale.become_, Step::Stop(_)));
    assert_eq!(stale.sends.diagnostics.len(), 1);
    assert!(stale.creates.is_empty());
}

#[tokio::test]
async fn cancelled_restart_keeps_the_uncommitted_activation_until_diagnostic_delivery() {
    let initial_drops = Arc::new(AtomicUsize::new(0));
    let replacement_drops = Arc::new(AtomicUsize::new(0));
    let mut initial = Some(HeldActivation(Arc::clone(&initial_drops)));
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                initial
                    .take()
                    .unwrap_or_else(|| panic!("one-role fixture prepares one worker")),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive restart delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let worker_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(worker_ready.sends.worker_assignments.is_empty());
    assert_eq!(initial_drops.load(Ordering::SeqCst), 1);

    let preparation = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"))
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let ControlFlow::Break(prepared) = preparation.accept(WorkerSubmission::activated(
        SearchWorker,
        HeldActivation(Arc::clone(&replacement_drops)),
    )) else {
        panic!("one selected role completes in one preparation")
    };
    let schedule = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(prepared),
        )))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"))
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));
    let timer_accepted = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("restart schedule settlement failed: {error}"));
    assert!(matches!(timer_accepted.become_, Step::Continue));
    assert!(timer_accepted.sends.diagnostics.is_empty());
    assert!(timer_accepted.creates.is_empty());

    let retired = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 0);
    drop(retired);
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn drain_deadline_cannot_consume_a_worker_restart_schedule() {
    let (mut pool, worker_schedule) =
        scheduling_search_replacement_with_drain(ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        })
        .await;
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let deadline_schedule = draining
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded drain schedules one deadline"));
    assert_ne!(worker_schedule.id, deadline_schedule.id);

    let deadline_accepted = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: deadline_schedule.id,
                generation: deadline_schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("drain deadline schedule failed: {error}"));
    assert!(matches!(deadline_accepted.become_, Step::Continue));
    assert!(deadline_accepted.sends.diagnostics.is_empty());

    let replacement_returned = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: worker_schedule.id,
                generation: worker_schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("worker restart schedule failed: {error}"));
    assert!(matches!(replacement_returned.become_, Step::Stop(_)));
    assert_eq!(replacement_returned.sends.diagnostics.len(), 1);
    assert!(replacement_returned.creates.is_empty());
}

#[tokio::test]
async fn orderly_retirement_releases_role_history_before_the_pool_is_dropped() {
    let role_owner = Arc::new(());
    let roles = OrderedRoles::new(Arc::clone(&role_owner), [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        |_: &Arc<()>| Ok::<_, Never>(WorkerSubmission::immediate(SearchWorker)),
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let worker_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(worker_ready.sends.worker_assignments.is_empty());

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"));
    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
    assert!(matches!(settled.become_, Step::Continue));
    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert_eq!(Arc::strong_count(&role_owner), 1);

    drop(pool);
    assert_eq!(Arc::strong_count(&role_owner), 1);
}

#[test]
fn rejected_drain_deadline_keeps_pending_creation_until_the_pool_is_dropped() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let (mut pool, deadline) = pending_creation_with_deadline(Arc::clone(&activation_drops));

    let retired = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: deadline,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("drain deadline rejection failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert!(retired.creates.is_empty());
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(retired);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(pool);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn elapsed_drain_deadline_keeps_pending_creation_until_the_pool_is_dropped() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let (mut pool, deadline) = pending_creation_with_deadline(Arc::clone(&activation_drops));
    let scheduled = TimerScheduled {
        id: deadline.id,
        generation: deadline.generation,
    };

    let waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("drain deadline schedule failed: {error}"));
    assert!(matches!(waiting.become_, Step::Continue));
    let retired = pool
        .on(TimerElapsed::new(deadline.id, deadline.generation))
        .unwrap_or_else(|error| panic!("drain deadline input failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert!(retired.creates.is_empty());
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(retired);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(pool);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn corrupt_and_unattempted_drain_deadlines_keep_pending_creation() {
    let corrupt_drops = Arc::new(AtomicUsize::new(0));
    let (mut corrupt_pool, corrupt_deadline) =
        pending_creation_with_deadline(Arc::clone(&corrupt_drops));
    let corrupt = corrupt_pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Corrupt {
                item: corrupt_deadline,
                fault: InterpreterFault::CorruptTraversal,
            },
        )))
        .unwrap_or_else(|error| panic!("corrupt drain deadline failed: {error}"));
    assert!(matches!(corrupt.become_, Step::Stop(_)));
    assert_eq!(corrupt_drops.load(Ordering::SeqCst), 0);
    drop(corrupt);
    drop(corrupt_pool);
    assert_eq!(corrupt_drops.load(Ordering::SeqCst), 1);

    let skipped_drops = Arc::new(AtomicUsize::new(0));
    let (mut skipped_pool, skipped_deadline) =
        pending_creation_with_deadline(Arc::clone(&skipped_drops));
    let skipped = skipped_pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Unattempted(
            skipped_deadline,
        )))
        .unwrap_or_else(|error| panic!("unattempted drain deadline failed: {error}"));
    assert!(matches!(skipped.become_, Step::Stop(_)));
    assert_eq!(skipped_drops.load(Ordering::SeqCst), 0);
    drop(skipped);
    drop(skipped_pool);
    assert_eq!(skipped_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn drain_deadline_requires_exact_schedule_and_elapsed_once() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let (mut pool, deadline) = pending_creation_with_deadline(Arc::clone(&activation_drops));
    let foreign_id = TimerId(deadline.id.0 + 1);

    let foreign_schedule = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: foreign_id,
                generation: deadline.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("foreign drain schedule failed: {error}"));
    assert!(matches!(foreign_schedule.become_, Step::Continue));
    assert_eq!(foreign_schedule.sends.diagnostics.len(), 1);

    let scheduled = TimerScheduled {
        id: deadline.id,
        generation: deadline.generation,
    };
    let waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("drain deadline schedule failed: {error}"));
    assert!(matches!(waiting.become_, Step::Continue));
    assert!(waiting.sends.diagnostics.is_empty());

    let duplicate_schedule = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("duplicate drain schedule failed: {error}"));
    assert!(matches!(duplicate_schedule.become_, Step::Continue));
    assert_eq!(duplicate_schedule.sends.diagnostics.len(), 1);

    let foreign_elapsed = pool
        .on(TimerElapsed::new(foreign_id, deadline.generation))
        .unwrap_or_else(|error| panic!("foreign drain deadline input failed: {error}"));
    assert!(matches!(foreign_elapsed.become_, Step::Continue));
    assert_eq!(foreign_elapsed.sends.diagnostics.len(), 1);

    let retired = pool
        .on(TimerElapsed::new(deadline.id, deadline.generation))
        .unwrap_or_else(|error| panic!("exact drain deadline input failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);

    let duplicate_elapsed = pool
        .on(TimerElapsed::new(deadline.id, deadline.generation))
        .unwrap_or_else(|error| panic!("duplicate drain deadline input failed: {error}"));
    assert!(matches!(duplicate_elapsed.become_, Step::Stop(_)));
    assert_eq!(duplicate_elapsed.sends.diagnostics.len(), 1);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(pool);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rejected_drain_deadline_cannot_settle_a_pending_worker_restart() {
    let (mut pool, worker_schedule) =
        scheduling_search_replacement_with_drain(ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        })
        .await;
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let deadline_schedule = draining
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded drain schedules one deadline"));
    assert_ne!(worker_schedule.id, deadline_schedule.id);

    let forced = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: deadline_schedule,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("drain deadline rejection failed: {error}"));
    assert!(matches!(forced.become_, Step::Stop(_)));
    assert!(forced.sends.diagnostics.is_empty());
    assert!(forced.creates.is_empty());

    let late_worker_schedule = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: worker_schedule.id,
                generation: worker_schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("late worker restart schedule failed: {error}"));
    assert!(matches!(late_worker_schedule.become_, Step::Stop(_)));
    assert_eq!(late_worker_schedule.sends.diagnostics.len(), 1);
    assert!(late_worker_schedule.creates.is_empty());
}

#[test]
fn forced_retirement_keeps_external_activation_owned_outside_the_pool() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let (mut pool, worker, activation) = pending_search_activation_with_drain(
        HeldActivation(Arc::clone(&activation_drops)),
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        },
    );

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    let deadline = draining
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded drain schedules one deadline"));
    let waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: deadline.id,
                generation: deadline.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("drain deadline schedule failed: {error}"));
    assert!(matches!(waiting.become_, Step::Continue));

    let forced = pool
        .on(TimerElapsed::new(deadline.id, deadline.generation))
        .unwrap_or_else(|error| panic!("drain deadline input failed: {error}"));
    assert!(matches!(forced.become_, Step::Stop(_)));
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);

    let late_stop = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("late worker exit input failed: {error}"));
    assert!(matches!(late_stop.become_, Step::Stop(_)));
    assert_eq!(late_stop.sends.diagnostics.len(), 1);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(pool);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(activation);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn forced_retirement_keeps_worker_source_owned_by_its_pending_request() {
    let source_drops = Arc::new(AtomicUsize::new(0));
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool_with_drain(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            TrackedSearchSource(Arc::clone(&source_drops)),
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        },
    )
    .await;
    let preparation = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"))
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdowns = draining.sends.worker_shutdowns;
    assert_eq!(shutdowns.len(), 1);
    let deadline = draining
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded drain schedules one deadline"));
    let forced = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: deadline,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("drain deadline rejection failed: {error}"));
    assert!(matches!(forced.become_, Step::Stop(_)));
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    drop(pool);
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    drop(preparation);
    assert_eq!(source_drops.load(Ordering::SeqCst), 1);
    drop(shutdowns);
}

#[tokio::test]
async fn forced_retirement_returns_queued_and_assigned_jobs_once() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        workers: _,
    } = ready_search_pool_with_drain(
        roles,
        BacklogCapacity::new(1),
        Interruption::Retry,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        },
    )
    .await;
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let first = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(51), 7, customer),
        )
        .unwrap_or_else(|error| panic!("first FIFO submission failed: {error}"));
    let assignment = first
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the first job"));
    let receipt = assignment.receipt();
    let second = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(52), 9, customer),
        )
        .unwrap_or_else(|error| panic!("second FIFO submission failed: {error}"));
    assert!(second.sends.worker_assignments.is_empty());

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let mut outcomes = draining.sends.customer_outcomes.into_deliveries();
    assert_eq!(outcomes.len(), 2);
    let first_outcome = match outcomes.remove(0) {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    let second_outcome = match outcomes.remove(0) {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(first_outcome.kind(), FifoOutcomeKind::ReturnedQueued);
    assert_eq!(second_outcome.kind(), FifoOutcomeKind::ReturnedAssigned);
    let deadline = draining
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded drain schedules one deadline"));
    let forced = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: deadline,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("drain deadline rejection failed: {error}"));
    assert!(matches!(forced.become_, Step::Stop(_)));

    let late_assignment = pool
        .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(receipt),
        )))
        .unwrap_or_else(|error| panic!("late assignment settlement failed: {error}"));
    assert!(matches!(late_assignment.become_, Step::Stop(_)));
    assert_eq!(late_assignment.sends.diagnostics.len(), 1);
    assert!(
        late_assignment
            .sends
            .customer_outcomes
            .as_slice()
            .is_empty()
    );
    drop(assignment);
}

#[test]
fn assignment_completion_source_shape_needs_no_pool_machinery() {
    fn worker_transition(
        worker: &mut SearchWorker,
        assignment: Assignment<u8>,
    ) -> BehaviorActed<SearchWorker> {
        let worker_result = u16::from(*assignment.payload());
        let _ = worker;
        Ok(Actions::cont().with_send(assignment.complete(worker_result)))
    }

    let _ = worker_transition;
    let _: Option<MessageProtocol<RuntimeAddr, Assignment<u8>>> = None;
}

#[tokio::test]
async fn stop_before_creation_settlement_never_advertises_the_worker() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let worker = creation.id();

    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("pre-creation worker stop failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());
    assert!(stopped.sends.worker_preparations.is_empty());

    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("committed worker returns its initialization outcome"));
    let retired = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));

    assert!(retired.sends.worker_activations.is_empty());
    assert!(retired.sends.worker_assignments.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 50);
}

#[tokio::test]
async fn stop_during_initialization_is_retained_once_until_initialization_returns() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();

    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let accepted = pool
        .on(stopped)
        .unwrap_or_else(|error| panic!("initializing worker stop failed: {error}"));
    assert!(accepted.sends.diagnostics.is_empty());
    assert!(accepted.sends.worker_preparations.is_empty());

    let duplicate = pool
        .on(stopped)
        .unwrap_or_else(|error| panic!("duplicate worker stop failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.worker_preparations.is_empty());

    let retired = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    assert!(retired.sends.worker_activations.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 51);
}

#[tokio::test]
async fn stop_while_waiting_for_activation_retires_the_exact_role() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut initializations = Vec::new();
    for creation in initialized.actions.creates {
        let committed = pool
            .on(created_worker(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        initializations.push(
            committed
                .sends
                .worker_initializations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("created worker awaits initialization")),
        );
    }
    let first = initializations.remove(0);
    let first_initialized = pool
        .on(first.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("first worker initialization failed: {error}"));
    assert_eq!(first_initialized.sends.worker_activations.len(), 1);

    let waiting = initializations.remove(0);
    let waiting_worker = waiting.worker().creation();
    let second_initialized = pool
        .on(waiting.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("second worker initialization failed: {error}"));
    assert!(second_initialized.sends.worker_activations.is_empty());

    let retired = pool
        .on(ChildStopped::new(
            waiting_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("waiting worker stop failed: {error}"));
    assert!(retired.sends.worker_activations.is_empty());
    assert!(retired.sends.worker_preparations.is_empty());
    let mut diagnostics = retired.sends.diagnostics.into_requests();
    let diagnostic = match diagnostics
        .pop()
        .unwrap_or_else(|| panic!("retiring a waiting worker retains its activation values"))
    {
        DiagnosticAction::Terminal { diagnostic } => diagnostic,
        DiagnosticAction::Deliver { .. } => panic!("the test selects terminal diagnostics"),
    };
    assert_eq!(diagnostic.role(), Some(&Role::Index));
}

#[tokio::test]
async fn stop_before_activation_start_releases_capacity_only_after_start_returns() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut initializations = Vec::new();
    for creation in initialized.actions.creates {
        let committed = pool
            .on(created_worker(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        initializations.push(
            committed
                .sends
                .worker_initializations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("created worker awaits initialization")),
        );
    }
    let first = initializations.remove(0);
    let first_worker = first.worker().creation();
    let first_initialized = pool
        .on(first.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("first worker initialization failed: {error}"));
    let activation = first_initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("first worker occupies activation capacity"));
    let second = initializations.remove(0);
    let second_initialized = pool
        .on(second.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("second worker initialization failed: {error}"));
    assert!(second_initialized.sends.worker_activations.is_empty());

    let stopped = ChildStopped::new(first_worker, Ok(Exit::Normal), Instant::now());
    let waiting = pool
        .on(stopped)
        .unwrap_or_else(|error| panic!("activation-dispatched worker stop failed: {error}"));
    assert!(waiting.sends.diagnostics.is_empty());
    assert!(waiting.sends.worker_activations.is_empty());

    let duplicate = pool
        .on(stopped)
        .unwrap_or_else(|error| panic!("duplicate activation worker stop failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.worker_activations.is_empty());

    let released = pool
        .on(activation.start_rejected(ActivationStartRejection::OwnerStopped))
        .unwrap_or_else(|error| panic!("activation rejection failed: {error}"));
    assert_eq!(released.sends.diagnostics.len(), 1);
    assert_eq!(released.sends.worker_activations.len(), 1);
}

#[tokio::test]
async fn stop_before_activation_start_remains_owned_until_late_readiness() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("activation-dispatched worker stop failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());
    assert!(stopped.sends.worker_preparations.is_empty());

    let started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("late activation start failed: {error}"));
    assert!(started.sends.diagnostics.is_empty());
    assert!(started.sends.worker_preparations.is_empty());

    let retired = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("late readiness failed: {error}"));
    assert!(retired.sends.worker_assignments.is_empty());
    assert!(retired.sends.worker_activations.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 52);
}

#[tokio::test]
async fn initialization_reported_stop_retires_without_a_separate_stop_input() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let retired = pool
        .on(
            initialization.resolve(WorkerInitializationOutcome::Stopped(ChildStopped::new(
                worker,
                Ok(Exit::Normal),
                Instant::now(),
            ))),
        )
        .unwrap_or_else(|error| panic!("initialization stop failed: {error}"));

    assert!(retired.sends.worker_activations.is_empty());
    assert!(retired.sends.worker_preparations.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 53);
}

#[tokio::test]
async fn initialization_rejection_after_stop_retires_without_losing_the_plan() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let factory_drops = Arc::clone(&activation_drops);
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                HeldActivation(Arc::clone(&factory_drops)),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("initializing worker stop failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());

    let retired = pool
        .on(
            initialization.resolve(WorkerInitializationOutcome::EffectsRejected(
                WorkerInitializationFailure::EffectsRejected,
            )),
        )
        .unwrap_or_else(|error| panic!("initialization rejection failed: {error}"));
    assert!(retired.sends.worker_activations.is_empty());
    assert!(retired.sends.worker_preparations.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    assert_search_pool_unavailable(&mut pool, 54);
    drop(retired);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn activation_rejection_after_stop_never_restores_worker_eligibility() {
    let rejection_drops = Arc::new(AtomicUsize::new(0));
    let factory_drops = Arc::clone(&rejection_drops);
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                RejectedSearchActivation(Arc::clone(&factory_drops)),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(started.sends.diagnostics.is_empty());
    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("activating worker stop failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());

    let retired = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("activation rejection input failed: {error}"));
    assert!(retired.sends.worker_assignments.is_empty());
    assert!(retired.sends.worker_activations.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 0);
    drop(retired);
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn permanent_pre_ready_stop_requests_replacement_only_after_initialization_returns() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("initializing worker stop failed: {error}"));
    assert!(stopped.sends.worker_preparations.is_empty());

    let recovering = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    assert!(recovering.sends.worker_activations.is_empty());
    assert_eq!(recovering.sends.worker_preparations.len(), 1);
    assert_eq!(recovering.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn foreign_stop_during_initialization_cannot_retire_the_worker() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let mut foreign_creations = CreationSequence::new();
    let _first = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("first test creation exists"));
    let foreign = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("second test creation exists"));
    let rejected = pool
        .on(ChildStopped::new(foreign, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("foreign worker stop failed: {error}"));
    assert_eq!(rejected.sends.diagnostics.len(), 1);

    let activated = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    assert_eq!(activated.sends.worker_activations.len(), 1);
    assert!(activated.sends.worker_preparations.is_empty());
}

#[tokio::test]
async fn initialization_failure_stops_the_worker_before_permanent_recovery() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let factory_drops = Arc::clone(&activation_drops);
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                HeldActivation(Arc::clone(&factory_drops)),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();

    let failed = pool
        .on(
            initialization.resolve(WorkerInitializationOutcome::EffectsRejected(
                WorkerInitializationFailure::InterpreterCorrupt,
            )),
        )
        .unwrap_or_else(|error| panic!("initialization failure input failed: {error}"));
    assert!(failed.sends.worker_activations.is_empty());
    assert!(failed.sends.worker_preparations.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    let shutdown = failed
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialization failure stops its installed worker"));

    let waiting = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("failed worker exit input failed: {error}"));
    assert!(waiting.sends.worker_preparations.is_empty());
    let recovering = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("failed worker shutdown settlement failed: {error}"));
    assert_eq!(recovering.sends.worker_preparations.len(), 1);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(failed.sends.diagnostics);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn activation_failure_releases_capacity_but_waits_for_worker_exit() {
    let rejection_drops = Arc::new(AtomicUsize::new(0));
    let factory_drops = Arc::clone(&rejection_drops);
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                RejectedSearchActivation(Arc::clone(&factory_drops)),
            ))
        },
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut initializations = Vec::new();
    for creation in initialized.actions.creates {
        let committed = pool
            .on(created_worker(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        initializations.push(
            committed
                .sends
                .worker_initializations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("created worker awaits initialization")),
        );
    }
    let first = initializations.remove(0);
    let first_worker = first.worker().creation();
    let first_initialized = pool
        .on(first.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("first worker initialization failed: {error}"));
    let activation = first_initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("first worker occupies activation capacity"));
    let second = initializations.remove(0);
    let second_initialized = pool
        .on(second.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("second worker initialization failed: {error}"));
    assert!(second_initialized.sends.worker_activations.is_empty());
    let started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(started.sends.diagnostics.is_empty());

    let failed = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("activation failure input failed: {error}"));
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_eq!(failed.sends.worker_activations.len(), 1);
    assert!(failed.sends.worker_preparations.is_empty());
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 0);
    let shutdown = failed
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("activation failure stops its installed worker"));

    let waiting = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("failed worker shutdown settlement failed: {error}"));
    assert!(waiting.sends.worker_preparations.is_empty());
    let retired = pool
        .on(ChildStopped::new(
            first_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("failed worker exit input failed: {error}"));
    assert!(retired.sends.worker_preparations.is_empty());
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 0);
    drop(failed.sends.diagnostics);
    assert_eq!(rejection_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn activation_start_rejection_stops_the_installed_worker() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));

    let failed = pool
        .on(activation.start_rejected(ActivationStartRejection::OwnerStopped))
        .unwrap_or_else(|error| panic!("activation start rejection failed: {error}"));
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert!(failed.sends.worker_activations.is_empty());
    assert!(failed.sends.worker_preparations.is_empty());
    let shutdown = failed
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("start rejection stops the installed worker"));
    let waiting = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("failed worker exit input failed: {error}"));
    assert!(waiting.sends.worker_preparations.is_empty());
    let retired = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("failed worker shutdown settlement failed: {error}"));
    assert!(retired.sends.worker_preparations.is_empty());
    assert_search_pool_unavailable(&mut pool, 55);
}

#[tokio::test]
async fn delivery_and_completion_orders_complete_once() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let worker_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(worker_ready.sends.worker_assignments.is_empty());

    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(9), 7, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    assert_eq!(submitted.sends.customer_outcomes.as_slice().len(), 1);
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
    let receipt = assignment.receipt();
    let (_, assignment, _) = assignment.into_parts();

    let completion = ChildReport::new(worker, assignment.complete(21).into_inner());
    let waiting = pool
        .on(completion)
        .unwrap_or_else(|error| panic!("completion input failed: {error}"));
    assert!(waiting.sends.customer_outcomes.as_slice().is_empty());
    assert!(waiting.sends.diagnostics.is_empty());

    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
    let completed = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("assignment settlement failed: {error}"));
    assert!(completed.sends.diagnostics.is_empty());
    let mut deliveries = completed.sends.customer_outcomes.into_deliveries();
    assert_eq!(deliveries.len(), 1);
    let outcome = match deliveries
        .pop()
        .unwrap_or_else(|| panic!("completion emits one customer outcome"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(outcome.kind(), FifoOutcomeKind::Completed);
    assert_eq!(outcome.role(), Some(&Role::Search));
    assert_eq!(outcome.worker_result(), Some(&21));

    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(10), 8, customer),
        )
        .unwrap_or_else(|error| panic!("second FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("released worker receives the next job"));
    let receipt = assignment.receipt();
    let (_, assignment, _) = assignment.into_parts();
    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
    let waiting = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("second assignment settlement failed: {error}"));
    assert!(waiting.sends.customer_outcomes.as_slice().is_empty());
    assert!(waiting.sends.diagnostics.is_empty());

    let completed = pool
        .on(ChildReport::new(
            worker,
            assignment.complete(34).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("second completion failed: {error}"));
    assert!(completed.sends.diagnostics.is_empty());
    let mut deliveries = completed.sends.customer_outcomes.into_deliveries();
    assert_eq!(deliveries.len(), 1);
    let outcome = match deliveries
        .pop()
        .unwrap_or_else(|| panic!("second completion emits one customer outcome"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(outcome.kind(), FifoOutcomeKind::Completed);
    assert_eq!(outcome.worker_result(), Some(&34));

    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(11), 9, customer),
        )
        .unwrap_or_else(|error| panic!("third FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("released worker receives the third job"));
    let receipt = assignment.receipt();
    let (_, assignment, _) = assignment.into_parts();
    let mut foreign_creations = CreationSequence::new();
    let _first = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("first test creation exists"));
    let foreign = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("second test creation exists"));
    let foreign_completion = pool
        .on(ChildReport::new(
            foreign,
            assignment.complete(55).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("foreign completion input failed: {error}"));
    assert!(
        foreign_completion
            .sends
            .customer_outcomes
            .as_slice()
            .is_empty()
    );
    assert_eq!(foreign_completion.sends.diagnostics.len(), 1);

    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
    let waiting = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("third assignment settlement failed: {error}"));
    assert!(waiting.sends.customer_outcomes.as_slice().is_empty());
    assert!(waiting.sends.diagnostics.is_empty());

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let mut deliveries = draining.sends.customer_outcomes.into_deliveries();
    assert_eq!(deliveries.len(), 1);
    let outcome = match deliveries
        .pop()
        .unwrap_or_else(|| panic!("shutdown returns the still-assigned job"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(outcome.kind(), FifoOutcomeKind::ReturnedAssigned);
    let (_, payload, reason) = outcome
        .into_returned_assigned()
        .unwrap_or_else(|_| panic!("shutdown returns an assigned outcome"));
    assert_eq!(payload, 9);
    assert_eq!(reason, AssignedReturnReason::PoolShutdown);
    drop(foreign_completion);
}

#[tokio::test]
async fn rejected_assignment_is_requeued_and_its_worker_is_quarantined() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let worker_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(worker_ready.sends.worker_assignments.is_empty());

    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(20), 13, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
    let rejected: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let requeued = pool
        .transition(FifoEvent::AssignmentSettled(rejected))
        .unwrap_or_else(|error| panic!("assignment rejection failed: {error}"));

    assert!(requeued.sends.diagnostics.is_empty());
    assert!(requeued.sends.customer_outcomes.as_slice().is_empty());
    assert!(requeued.sends.worker_assignments.is_empty());
    let shutdown = requeued
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("rejected delivery quarantines its worker"));

    let awaiting_settlement = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("quarantined worker exit failed: {error}"));
    assert!(awaiting_settlement.sends.diagnostics.is_empty());
    assert!(
        awaiting_settlement
            .sends
            .customer_outcomes
            .as_slice()
            .is_empty()
    );
    let retired = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("quarantine shutdown settlement failed: {error}"));
    assert!(retired.sends.diagnostics.is_empty());
    let mut retirement_outcomes = retired.sends.customer_outcomes.into_deliveries();
    assert_eq!(retirement_outcomes.len(), 1);
    let returned = match retirement_outcomes
        .pop()
        .unwrap_or_else(|| panic!("temporary retirement returns the requeued job"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    let (_, payload, reason) = returned
        .into_returned_queued()
        .unwrap_or_else(|_| panic!("temporary retirement returns a queued outcome"));
    assert_eq!(payload, 13);
    assert_eq!(reason, QueuedReturnReason::NoRecoverableWorkers);

    let duplicate = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("duplicate quarantined exit failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.customer_outcomes.as_slice().is_empty());

    let next = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(21), 17, customer),
        )
        .unwrap_or_else(|error| panic!("second FIFO submission failed: {error}"));
    assert!(next.sends.worker_assignments.is_empty());
    let mut next_outcomes = next.sends.customer_outcomes.into_deliveries();
    assert_eq!(next_outcomes.len(), 1);
    let rejected = match next_outcomes
        .pop()
        .unwrap_or_else(|| panic!("retired pool rejects the second submission"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(rejected.kind(), FifoOutcomeKind::Rejected);
    let (_, payload, reason) = rejected
        .into_rejected()
        .unwrap_or_else(|_| panic!("retired pool returns a rejected outcome"));
    assert_eq!(payload, 17);
    assert_eq!(reason, AdmissionRejection::NoRecoverableWorkers);

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(draining.sends.customer_outcomes.as_slice().is_empty());
}

#[tokio::test]
async fn rejected_quarantine_shutdown_waits_for_worker_exit() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let worker_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(worker_ready.sends.worker_assignments.is_empty());

    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(22), 19, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
    let rejected: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let requeued = pool
        .transition(FifoEvent::AssignmentSettled(rejected))
        .unwrap_or_else(|error| panic!("assignment rejection failed: {error}"));
    let shutdown = requeued
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("rejected delivery quarantines its worker"));

    let awaiting_exit = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::rejected(
            shutdown.id,
            ShutdownRejection::AlreadyStopping,
        ))
        .unwrap_or_else(|error| panic!("quarantine shutdown rejection failed: {error}"));
    assert!(awaiting_exit.sends.diagnostics.is_empty());
    assert!(awaiting_exit.sends.customer_outcomes.as_slice().is_empty());

    let mut foreign_creations = CreationSequence::new();
    let _first = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("first test creation exists"));
    let foreign = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("second test creation exists"));
    let foreign_exit = pool
        .on(ChildStopped::new(foreign, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("foreign worker exit input failed: {error}"));
    assert_eq!(foreign_exit.sends.diagnostics.len(), 1);

    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("quarantined worker exit failed: {error}"));
    assert_eq!(retired.sends.diagnostics.len(), 1);
    let mut retirement_outcomes = retired.sends.customer_outcomes.into_deliveries();
    assert_eq!(retirement_outcomes.len(), 1);
    let returned = match retirement_outcomes
        .pop()
        .unwrap_or_else(|| panic!("temporary retirement returns the requeued job"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    let (_, payload, reason) = returned
        .into_returned_queued()
        .unwrap_or_else(|_| panic!("temporary retirement returns a queued outcome"));
    assert_eq!(payload, 19);
    assert_eq!(reason, QueuedReturnReason::NoRecoverableWorkers);

    let duplicate = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("duplicate shutdown settlement failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.customer_outcomes.as_slice().is_empty());

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(draining.sends.customer_outcomes.as_slice().is_empty());
}

#[tokio::test]
async fn accepted_quarantine_shutdown_still_waits_for_worker_exit() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(1),
        Interruption::Retry,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let worker = workers
        .pop()
        .unwrap_or_else(|| panic!("the ready worker exists"));
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(23), 31, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
    let rejected: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let requeued = pool
        .transition(FifoEvent::AssignmentSettled(rejected))
        .unwrap_or_else(|error| panic!("assignment rejection failed: {error}"));
    let shutdown = requeued
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("rejected delivery quarantines its worker"));

    let awaiting_exit = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("quarantine shutdown settlement failed: {error}"));
    assert!(awaiting_exit.sends.diagnostics.is_empty());
    assert!(awaiting_exit.sends.customer_outcomes.as_slice().is_empty());

    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("quarantined worker exit failed: {error}"));
    assert!(retired.sends.diagnostics.is_empty());
    let mut retirement_outcomes = retired.sends.customer_outcomes.into_deliveries();
    assert_eq!(retirement_outcomes.len(), 1);
    let returned = match retirement_outcomes
        .pop()
        .unwrap_or_else(|| panic!("temporary retirement returns the requeued job"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    let (_, payload, reason) = returned
        .into_returned_queued()
        .unwrap_or_else(|_| panic!("temporary retirement returns a queued outcome"));
    assert_eq!(payload, 31);
    assert_eq!(reason, QueuedReturnReason::NoRecoverableWorkers);

    let duplicate = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("duplicate shutdown settlement failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.customer_outcomes.as_slice().is_empty());
}

#[tokio::test]
async fn temporary_retirement_reassigns_retry_to_a_surviving_worker() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(1),
        Interruption::Retry,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let stopped_worker = workers.remove(0);
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(24), 37, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("first ready worker receives the accepted job"));
    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(assignment.receipt()));
    let waiting = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("assignment settlement failed: {error}"));
    assert!(waiting.sends.customer_outcomes.as_slice().is_empty());

    let retired = pool
        .on(ChildStopped::new(
            stopped_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(retired.sends.diagnostics.is_empty());
    assert!(retired.sends.customer_outcomes.as_slice().is_empty());
    let reassigned = retired
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("surviving worker receives the retried job"));
    let (_, reassigned, _) = reassigned.into_parts();
    assert_eq!(*reassigned.payload(), 37);
    drop(assignment);
}

#[tokio::test]
async fn temporary_stop_pool_shuts_down_only_surviving_workers() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::temporary(PoolFailureReaction::StopPool),
    )
    .await;
    let stopped_worker = workers.remove(0);

    let stopping = pool
        .on(ChildStopped::new(
            stopped_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(matches!(stopping.become_, Step::Continue));
    assert_eq!(stopping.sends.worker_shutdowns.len(), 1);
    assert!(stopping.sends.customer_outcomes.as_slice().is_empty());
}

#[tokio::test]
async fn permanent_worker_stop_emits_one_affine_source_request() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let worker = workers
        .pop()
        .unwrap_or_else(|| panic!("the ready worker exists"));

    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());
    let mut requests = stopped.sends.worker_preparations.into_items();
    assert_eq!(requests.len(), 1);
    let mut request = requests
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let (source, role) = request.source_and_role();
    assert_eq!(source, &mut SearchSource);
    assert_eq!(role, &Role::Search);
}

#[tokio::test]
async fn transient_normal_stop_retires_without_preparation() {
    for outcome in [Ok(Exit::Normal), Ok(Exit::Collected)] {
        let roles = OrderedRoles::new(Role::Search, [])
            .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
        let ReadySearchPool {
            mut pool,
            mut workers,
        } = ready_search_pool(
            roles,
            BacklogCapacity::new(1),
            Interruption::Fail,
            PoolRecovery::transient(
                SearchSource,
                RestartLimit::new(3, Duration::from_secs(10)),
                RestartRelease::immediate(),
                PoolFailureReaction::RetireRole,
            ),
        )
        .await;
        let worker = workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists"));

        let retired = pool
            .on(ChildStopped::new(worker, outcome, Instant::now()))
            .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
        assert!(retired.sends.worker_preparations.is_empty());
        let customer =
            Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
                RuntimeAddr(88),
            );
        let rejected = pool
            .receive(
                RuntimeAddr(7),
                FifoCommand::submit(SubmissionId::new(25), 41, customer),
            )
            .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
        let mut outcomes = rejected.sends.customer_outcomes.into_deliveries();
        let outcome = match outcomes
            .pop()
            .unwrap_or_else(|| panic!("a retired one-role pool rejects new work"))
        {
            ReplyDelivery::Logical(delivery) => delivery.message,
            ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
        };
        let (_, payload, reason) = outcome
            .into_rejected()
            .unwrap_or_else(|_| panic!("the unavailable pool returns a rejection"));
        assert_eq!(payload, 41);
        assert_eq!(reason, AdmissionRejection::NoRecoverableWorkers);
    }
}

#[tokio::test]
async fn transient_abnormal_stop_emits_one_affine_source_request() {
    let outcomes = [
        Ok(Exit::LinkDied(RuntimeAddr(99))),
        Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildStopped,
        )),
        Err(Crash::Failed),
        Err(Crash::EnvironmentFailed),
        Err(Crash::Panicked),
        Err(Crash::Cancelled),
    ];
    for outcome in outcomes {
        let roles = OrderedRoles::new(Role::Search, [])
            .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
        let ReadySearchPool {
            mut pool,
            mut workers,
        } = ready_search_pool(
            roles,
            BacklogCapacity::new(0),
            Interruption::Fail,
            PoolRecovery::transient(
                SearchSource,
                RestartLimit::new(3, Duration::from_secs(10)),
                RestartRelease::immediate(),
                PoolFailureReaction::RetireRole,
            ),
        )
        .await;
        let worker = workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists"));

        let stopped = pool
            .on(ChildStopped::new(worker, outcome, Instant::now()))
            .unwrap_or_else(|error| panic!("worker crash input failed: {error}"));
        assert_eq!(stopped.sends.worker_preparations.len(), 1);
    }
}

#[tokio::test]
async fn concurrent_stops_do_not_duplicate_the_affine_source() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;

    let first = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("first worker exit input failed: {error}"));
    assert_eq!(first.sends.worker_preparations.len(), 1);
    let second = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("second worker exit input failed: {error}"));
    assert!(second.sends.worker_preparations.is_empty());
    assert!(second.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn accepted_preparation_starts_one_immediate_replacement() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let stopped = pool
        .on(ChildStopped::new(
            workers
                .pop()
                .unwrap_or_else(|| panic!("the ready worker exists")),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    let request = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let ControlFlow::Break(preparation) = request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let settled = SettledItem::Attempted(ItemSettlement::Accepted(preparation));

    let restarting = pool
        .transition(FifoEvent::WorkerPreparationSettled(settled))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"));
    assert!(restarting.sends.diagnostics.is_empty());
    assert_eq!(restarting.creates.len(), 1);
    assert!(restarting.sends.worker_preparations.is_empty());
}

#[tokio::test]
async fn returned_source_prepares_the_first_waiting_role() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let first = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("first worker exit input failed: {error}"));
    let first_request = first
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("first stop owns the source request"));
    let waiting = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("second worker exit input failed: {error}"));
    assert!(waiting.sends.worker_preparations.is_empty());
    let ControlFlow::Break(preparation) =
        first_request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };

    let restarting = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"));
    assert!(restarting.sends.diagnostics.is_empty());
    assert_eq!(restarting.creates.len(), 1);
    let mut requests = restarting.sends.worker_preparations.into_items();
    assert_eq!(requests.len(), 1);
    let mut request = requests
        .pop()
        .unwrap_or_else(|| panic!("returned source serves one waiting role"));
    let (source, role) = request.source_and_role();
    assert_eq!(source, &mut SearchSource);
    assert_eq!(role, &Role::Index);
}

#[tokio::test]
async fn delayed_replacement_waits_for_schedule_and_timer() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive restart delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let stopped = pool
        .on(ChildStopped::new(
            workers
                .pop()
                .unwrap_or_else(|| panic!("the ready worker exists")),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    let request = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let ControlFlow::Break(preparation) = request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let scheduling = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"));
    assert!(scheduling.creates.is_empty());
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));
    assert_eq!(schedule.after, Duration::from_secs(1));

    let waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("restart schedule settlement failed: {error}"));
    assert!(waiting.creates.is_empty());
    assert!(waiting.sends.diagnostics.is_empty());
    let restarting = pool
        .on(TimerElapsed::new(schedule.id, schedule.generation))
        .unwrap_or_else(|error| panic!("restart timer input failed: {error}"));
    assert_eq!(restarting.creates.len(), 1);
    assert!(restarting.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn restart_limit_denial_retires_the_role_without_creation() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(0, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let stopped = pool
        .on(ChildStopped::new(
            workers
                .pop()
                .unwrap_or_else(|| panic!("the ready worker exists")),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    let request = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let ControlFlow::Break(preparation) = request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let denied = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"));
    assert!(denied.creates.is_empty());
    assert_eq!(denied.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn restart_limit_keeps_a_charge_at_the_inclusive_cutoff() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(1, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let first_stop = Instant::now();
    let first = pool
        .on(ChildStopped::new(
            workers
                .pop()
                .unwrap_or_else(|| panic!("the ready worker exists")),
            Ok(Exit::Normal),
            first_stop,
        ))
        .unwrap_or_else(|error| panic!("first worker exit input failed: {error}"));
    let first_request = first
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the first stop prepares one replacement"));
    let ControlFlow::Break(first_preparation) =
        first_request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let first_replacement = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(first_preparation),
        )))
        .unwrap_or_else(|error| panic!("first preparation settlement failed: {error}"));
    let committed = pool
        .on(created_worker(
            first_replacement
                .creates
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("the first replacement is created")),
        ))
        .unwrap_or_else(|error| panic!("replacement creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the replacement awaits initialization"));
    let replacement = initialization.worker().creation();
    let initialized = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("replacement initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the replacement begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("replacement activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let replacement_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("replacement readiness failed: {error}"));
    assert!(replacement_ready.sends.worker_assignments.is_empty());

    let cutoff_stop = first_stop + Duration::from_secs(10);
    let second = pool
        .on(ChildStopped::new(
            replacement,
            Ok(Exit::Normal),
            cutoff_stop,
        ))
        .unwrap_or_else(|error| panic!("replacement exit input failed: {error}"));
    let second_request = second
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the second stop prepares one replacement"));
    let ControlFlow::Break(second_preparation) =
        second_request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let denied = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(second_preparation),
        )))
        .unwrap_or_else(|error| panic!("second preparation settlement failed: {error}"));
    assert!(denied.creates.is_empty());
    assert_eq!(denied.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn rejected_restart_schedule_retires_without_creation() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive restart delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let stopped = pool
        .on(ChildStopped::new(
            workers
                .pop()
                .unwrap_or_else(|| panic!("the ready worker exists")),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    let request = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("permanent stop prepares one replacement"));
    let ControlFlow::Break(preparation) = request.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let scheduling = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation settlement failed: {error}"));
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));
    let rejected = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("restart schedule rejection failed: {error}"));
    assert!(rejected.creates.is_empty());
    assert_eq!(rejected.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn rejected_preparation_restores_source_for_next_waiting_role() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            FallibleSearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let request = stop_search_worker(&mut pool, workers.remove(0));
    let waiting = pool
        .on(ChildStopped::new(
            workers.remove(0),
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("second worker exit input failed: {error}"));
    assert!(waiting.sends.worker_preparations.is_empty());

    let failed = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: request,
                reason: SearchSourceRejection::Closed,
            },
        )))
        .unwrap_or_else(|error| panic!("worker preparation rejection failed: {error}"));
    assert!(failed.creates.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    let mut requests = failed.sends.worker_preparations.into_items();
    assert_eq!(requests.len(), 1);
    let mut request = requests
        .pop()
        .unwrap_or_else(|| panic!("returned source serves the waiting role"));
    let (source, role) = request.source_and_role();
    assert_eq!(source, &mut FallibleSearchSource);
    assert_eq!(role, &Role::Index);
}

#[tokio::test]
async fn rejected_preparation_preserves_source_and_reason_custody() {
    let source_drops = Arc::new(AtomicUsize::new(0));
    let reason_drops = Arc::new(AtomicUsize::new(0));
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            TrackedSearchSource(Arc::clone(&source_drops)),
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let request = stop_search_worker(
        &mut pool,
        workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists")),
    );

    let failed = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: request,
                reason: TrackedSourceRejection(Arc::clone(&reason_drops)),
            },
        )))
        .unwrap_or_else(|error| panic!("worker preparation rejection failed: {error}"));
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    assert_eq!(reason_drops.load(Ordering::SeqCst), 0);

    drop(failed);
    assert_eq!(source_drops.load(Ordering::SeqCst), 0);
    assert_eq!(reason_drops.load(Ordering::SeqCst), 1);
    drop(pool);
    assert_eq!(source_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn worker_preparation_rejection_retires_the_role() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            FallibleSearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let request = stop_search_worker(
        &mut pool,
        workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists")),
    );
    let preparation = request.reject(SearchWorkerRejection::Unavailable);

    let failed = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation failure input failed: {error}"));
    assert!(failed.creates.is_empty());
    assert!(failed.sends.worker_preparations.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 31);
}

#[tokio::test]
async fn preparation_failure_can_stop_the_pool_and_drain_surviving_workers() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            FallibleSearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::StopPool,
        ),
    )
    .await;
    let request = stop_search_worker(&mut pool, workers.remove(0));
    let preparation = request.reject(SearchWorkerRejection::Unavailable);

    let draining = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(preparation),
        )))
        .unwrap_or_else(|error| panic!("worker preparation failure input failed: {error}"));
    assert!(draining.creates.is_empty());
    assert!(draining.sends.worker_preparations.is_empty());
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    assert_eq!(draining.sends.diagnostics.len(), 1);
    assert!(matches!(draining.become_, Step::Continue));
}

#[tokio::test]
async fn corrupt_preparation_retires_the_role_without_losing_its_source() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            FallibleSearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let request = stop_search_worker(
        &mut pool,
        workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists")),
    );

    let failed = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Corrupt {
                item: request,
                fault: InterpreterFault::CorruptTraversal,
            },
        )))
        .unwrap_or_else(|error| panic!("corrupt preparation input failed: {error}"));
    assert!(failed.creates.is_empty());
    assert!(failed.sends.worker_preparations.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 32);
}

#[tokio::test]
async fn unattempted_preparation_retires_the_role_without_losing_its_source() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            FallibleSearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;
    let request = stop_search_worker(
        &mut pool,
        workers
            .pop()
            .unwrap_or_else(|| panic!("the ready worker exists")),
    );

    let failed = pool
        .transition(FifoEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(request),
        ))
        .unwrap_or_else(|error| panic!("unattempted preparation input failed: {error}"));
    assert!(failed.creates.is_empty());
    assert!(failed.sends.worker_preparations.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 33);
}

#[tokio::test]
async fn foreign_preparation_cannot_advance_another_pool() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let recovery = || {
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        )
    };
    let ReadySearchPool {
        pool: mut first_pool,
        workers: mut first_workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        recovery(),
    )
    .await;
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let ReadySearchPool {
        pool: mut second_pool,
        workers: mut second_workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        recovery(),
    )
    .await;
    let foreign = stop_search_worker(
        &mut first_pool,
        first_workers
            .pop()
            .unwrap_or_else(|| panic!("the first worker exists")),
    );
    let expected = stop_search_worker(
        &mut second_pool,
        second_workers
            .pop()
            .unwrap_or_else(|| panic!("the second worker exists")),
    );
    let ControlFlow::Break(foreign) = foreign.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };

    let rejected = second_pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(foreign),
        )))
        .unwrap_or_else(|error| panic!("foreign preparation input failed: {error}"));
    assert!(rejected.creates.is_empty());
    assert!(rejected.sends.worker_preparations.is_empty());
    assert_eq!(rejected.sends.diagnostics.len(), 1);

    let ControlFlow::Break(expected) = expected.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let accepted = second_pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(expected),
        )))
        .unwrap_or_else(|error| panic!("exact preparation input failed: {error}"));
    assert_eq!(accepted.creates.len(), 1);
    assert!(accepted.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn corrupt_restart_schedule_retires_without_creation() {
    let (mut pool, schedule) = scheduling_search_replacement().await;

    let failed = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Corrupt {
                item: schedule,
                fault: InterpreterFault::CorruptTraversal,
            },
        )))
        .unwrap_or_else(|error| panic!("corrupt restart schedule failed: {error}"));
    assert!(failed.creates.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 34);
}

#[tokio::test]
async fn unattempted_restart_schedule_retires_without_creation() {
    let (mut pool, schedule) = scheduling_search_replacement().await;

    let failed = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Unattempted(
            schedule,
        )))
        .unwrap_or_else(|error| panic!("unattempted restart schedule failed: {error}"));
    assert!(failed.creates.is_empty());
    assert_eq!(failed.sends.diagnostics.len(), 1);
    assert_search_pool_unavailable(&mut pool, 35);
}

#[tokio::test]
async fn restart_schedule_and_timer_require_exact_correlation_once() {
    let (mut pool, schedule) = scheduling_search_replacement().await;
    let scheduled = TimerScheduled {
        id: schedule.id,
        generation: schedule.generation,
    };
    let foreign_scheduled = TimerScheduled {
        id: TimerId(schedule.id.0 + 1),
        generation: schedule.generation,
    };

    let foreign = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(foreign_scheduled),
        )))
        .unwrap_or_else(|error| panic!("foreign restart schedule input failed: {error}"));
    assert!(foreign.creates.is_empty());
    assert_eq!(foreign.sends.diagnostics.len(), 1);

    let waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("exact restart schedule input failed: {error}"));
    assert!(waiting.creates.is_empty());
    assert!(waiting.sends.diagnostics.is_empty());

    let duplicate_schedule = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(scheduled),
        )))
        .unwrap_or_else(|error| panic!("duplicate restart schedule input failed: {error}"));
    assert!(duplicate_schedule.creates.is_empty());
    assert_eq!(duplicate_schedule.sends.diagnostics.len(), 1);

    let foreign_timer = pool
        .on(TimerElapsed::new(
            TimerId(schedule.id.0 + 1),
            schedule.generation,
        ))
        .unwrap_or_else(|error| panic!("foreign restart timer input failed: {error}"));
    assert!(foreign_timer.creates.is_empty());
    assert_eq!(foreign_timer.sends.diagnostics.len(), 1);

    let restarted = pool
        .on(TimerElapsed::new(schedule.id, schedule.generation))
        .unwrap_or_else(|error| panic!("exact restart timer input failed: {error}"));
    assert_eq!(restarted.creates.len(), 1);
    assert!(restarted.sends.diagnostics.is_empty());

    let duplicate_timer = pool
        .on(TimerElapsed::new(schedule.id, schedule.generation))
        .unwrap_or_else(|error| panic!("duplicate restart timer input failed: {error}"));
    assert!(duplicate_timer.creates.is_empty());
    assert_eq!(duplicate_timer.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn worker_exit_waits_for_delivery_then_retries_the_same_job() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let worker_ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(worker_ready.sends.worker_assignments.is_empty());

    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(30), 23, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
    let receipt = assignment.receipt();

    let mut foreign_creations = CreationSequence::new();
    let _first = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("first test creation exists"));
    let foreign = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("second test creation exists"));
    let foreign_exit = pool
        .on(ChildStopped::new(foreign, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("foreign worker exit input failed: {error}"));
    assert_eq!(foreign_exit.sends.diagnostics.len(), 1);
    assert!(foreign_exit.sends.customer_outcomes.as_slice().is_empty());

    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());
    assert!(stopped.sends.customer_outcomes.as_slice().is_empty());
    assert!(stopped.sends.worker_assignments.is_empty());

    let duplicate = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("duplicate worker exit input failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.customer_outcomes.as_slice().is_empty());

    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
    let interrupted = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("assignment settlement failed: {error}"));
    assert!(interrupted.sends.diagnostics.is_empty());
    let mut outcomes = interrupted.sends.customer_outcomes.into_deliveries();
    assert_eq!(outcomes.len(), 1);
    let returned = match outcomes
        .pop()
        .unwrap_or_else(|| panic!("the final temporary worker returns the retried job"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(returned.kind(), FifoOutcomeKind::ReturnedQueued);
    let (_, payload, reason) = returned
        .into_returned_queued()
        .unwrap_or_else(|_| panic!("temporary retirement returns the retried job as queued"));
    assert_eq!(payload, 23);
    assert_eq!(reason, QueuedReturnReason::NoRecoverableWorkers);
    assert!(interrupted.sends.worker_assignments.is_empty());

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(draining.sends.customer_outcomes.as_slice().is_empty());
    drop(assignment);
}

#[tokio::test]
async fn worker_exit_after_delivery_returns_the_job_under_fail_policy() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut workers = Vec::new();
    for creation in initialized.actions.creates {
        let committed = pool
            .on(created_worker(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = committed
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("created worker awaits initialization"));
        workers.push(initialization.worker().creation());
        let initialized_worker = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized_worker
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("initialized worker begins activation"));
        let activation_started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("activation start failed: {error}"));
        assert!(activation_started.sends.worker_assignments.is_empty());
        let worker_ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(worker_ready.sends.worker_assignments.is_empty());
    }
    assert_eq!(workers.len(), 2);
    let worker = workers
        .pop()
        .unwrap_or_else(|| panic!("the second worker exists"));
    let idle_worker = workers
        .pop()
        .unwrap_or_else(|| panic!("the first worker exists"));
    let idle_stopped = pool
        .on(ChildStopped::new(
            idle_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("idle worker exit input failed: {error}"));
    assert!(idle_stopped.sends.diagnostics.is_empty());
    assert!(idle_stopped.sends.customer_outcomes.as_slice().is_empty());

    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(40), 29, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
    let receipt = assignment.receipt();
    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
    let waiting = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("assignment settlement failed: {error}"));
    assert!(waiting.sends.customer_outcomes.as_slice().is_empty());

    let interrupted = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
    assert!(interrupted.sends.diagnostics.is_empty());
    assert!(interrupted.sends.worker_assignments.is_empty());
    let mut outcomes = interrupted.sends.customer_outcomes.into_deliveries();
    assert_eq!(outcomes.len(), 1);
    let returned = match outcomes
        .pop()
        .unwrap_or_else(|| panic!("Fail returns the interrupted job"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(returned.kind(), FifoOutcomeKind::ReturnedAssigned);
    let (_, payload, reason) = returned
        .into_returned_assigned()
        .unwrap_or_else(|_| panic!("Fail returns one assigned outcome"));
    assert_eq!(payload, 29);
    assert_eq!(reason, AssignedReturnReason::WorkerStopped);

    let duplicate = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("duplicate worker exit input failed: {error}"));
    assert_eq!(duplicate.sends.diagnostics.len(), 1);
    assert!(duplicate.sends.customer_outcomes.as_slice().is_empty());
    drop(assignment);
}
