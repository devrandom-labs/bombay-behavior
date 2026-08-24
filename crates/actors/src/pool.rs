//! A bounded, FIFO worker pool expressed entirely as a pure behavior.
//!
//! Pool scheduling is a derived Bombay construction, not an actor-model
//! primitive. Runtime installation, delivery, and observation remain effects
//! for an interpreter; this module owns only their typed protocol and fold.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::DeliveryRoute;
use crate::supervision::{FixedFleetOwnership, OwnershipError};
use crate::{
    Actions, Address, Behavior, Births, ChildTopology, CommandSupervisionEvent, Crash,
    CreationRejection, Exit, FleetError, Never, Own, Protocol, ProxyCommand, ProxyParentIngress,
    ProxyWithParent, Recipient, RestartConfiguration, RestartPolicy, SendEffects, SendInput,
    SendLayer, Strategy, SupervisorSends, User, WorkerCreationResolved, WorkerStopped,
};
use behavior::{ChildRoute, MessageProtocol};

/// Caller-chosen identity used to correlate pool responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

/// Pool-owned correlation token for one exact dispatch attempt.
///
/// This is not an actor identity or evidence that delivery occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssignmentId(pub u64);

/// One assignment accepted by a worker behavior.
#[derive(Clone, PartialEq, Eq)]
pub struct PoolAssignment<Pool>
where
    Pool: crate::PoolAssignmentProtocol,
{
    pub assignment: AssignmentId,
    pub job: JobId,
    pub payload: Pool::Job,
    /// Stable worker slot completing this assignment.
    pub worker: <Pool::Addr as Address>::Nonce,
    /// Established destination for the completion protocol.
    pub complete_to: Recipient<Pool>,
}

impl<Pool> core::fmt::Debug for PoolAssignment<Pool>
where
    Pool: crate::PoolAssignmentProtocol,
    Pool::Job: core::fmt::Debug,
    <Pool::Addr as Address>::Nonce: core::fmt::Debug,
    Pool::Addr: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PoolAssignment")
            .field("assignment", &self.assignment)
            .field("job", &self.job)
            .field("payload", &self.payload)
            .field("worker", &self.worker)
            .field("complete_to", &self.complete_to)
            .finish()
    }
}

/// Messages accepted by a pool coordinator.
#[derive(Clone, PartialEq, Eq)]
pub enum PoolMessage<A, D, J, R, Route>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D>,
{
    Submit {
        job: JobId,
        payload: J,
        reply_to: Route,
    },
    Completed {
        worker: <D::Addr as Address>::Nonce,
        assignment: AssignmentId,
        result: R,
    },
}

/// Messages accepted by a key-persistent pool coordinator.
///
/// `Rebalance` is the only input that can change an established key binding.
/// It affects later submissions; jobs already accepted retain their selected
/// stable worker slot.
#[derive(Clone, PartialEq, Eq)]
pub enum KeyedPoolMessage<A, D, K, J, R, Route>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D>,
{
    Submit {
        key: K,
        job: JobId,
        payload: J,
        reply_to: Route,
    },
    Completed {
        worker: <D::Addr as Address>::Nonce,
        assignment: AssignmentId,
        result: R,
    },
    Rebalance {
        key: K,
        worker: <D::Addr as Address>::Nonce,
    },
}

/// Why a submitted job was not accepted by the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolRejection {
    BacklogFull,
    /// The key's selected stable slot is unknown or permanently retired.
    AffinityUnavailable,
    /// The pool has begun terminal shutdown and cannot accept ownership.
    ShuttingDown,
}

/// Why an accepted assignment ended without a worker completion.
#[derive(Clone, PartialEq, Eq)]
pub enum PoolInterruption<A: Address> {
    WorkerStopped {
        worker: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
    },
    NoRecoverableWorkers,
    /// The job's selected stable slot retired while the job was queued.
    AffinityRetired {
        worker: A::Nonce,
        reason: WorkerRetirement,
    },
    /// The pool returned accepted ownership before draining its worker proxies.
    PoolShutdown,
}

/// Complete response protocol for one submitted job.
#[derive(Clone, PartialEq, Eq)]
pub enum PoolResponse<J, R, A: Address> {
    Accepted {
        job: JobId,
    },
    Rejected {
        job: JobId,
        payload: J,
        reason: PoolRejection,
    },
    Completed {
        job: JobId,
        result: R,
    },
    Interrupted {
        job: JobId,
        payload: J,
        reason: PoolInterruption<A>,
    },
}

/// Bombay policy for an assigned job whose worker incarnation stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptionPolicy {
    /// End pool ownership and report the still-owned job to its submitter.
    Fail,
    /// Put the job at the front of the backlog for at-least-once assignment.
    Retry,
}

/// Admission, interruption, and replacement policy for a worker pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolConfiguration {
    /// Maximum jobs retained while every eligible worker is occupied.
    pub backlog_capacity: usize,
    /// Ownership policy when an assigned worker stops.
    pub interruption: InterruptionPolicy,
    /// Worker exits eligible for replacement.
    pub restart_policy: RestartPolicy,
    /// Maximum accepted replacements inside `restart_window`.
    pub maximum_restarts: u32,
    /// Inclusive restart-budget window.
    pub restart_window: Duration,
}

impl PoolConfiguration {
    /// Define the complete pool policy independently of its worker topology.
    #[must_use]
    pub const fn new(
        backlog_capacity: usize,
        interruption: InterruptionPolicy,
        restart_policy: RestartPolicy,
        maximum_restarts: u32,
        restart_window: Duration,
    ) -> Self {
        Self {
            backlog_capacity,
            interruption,
            restart_policy,
            maximum_restarts,
            restart_window,
        }
    }
}

/// Public, payload-free view of one stable worker slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerPhase {
    Installing,
    Idle,
    /// The pool has returned owned work and is draining this stable proxy.
    Stopping,
    Assigned {
        assignment: AssignmentId,
        job: JobId,
    },
    Retired {
        reason: WorkerRetirement,
    },
}

/// Why a stable worker slot is no longer eligible for dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerRetirement {
    CreationRejected(CreationRejection),
    ReplacementUnavailable,
}

/// Invalid static pool topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PoolConfigError<N> {
    /// No stable worker slot exists, so accepted ownership could never end.
    #[error("a worker pool requires at least one worker")]
    NoWorkers,
    /// Two configured positions selected the same stable worker nonce.
    #[error("two configured worker positions selected the same nonce")]
    DuplicateWorker(N),
    #[error("the configured worker nonce sequence is exhausted")]
    SequenceExhausted,
}

/// Pure, statically dispatched policy for a previously unseen affinity key.
pub trait AffinitySelector<K, N> {
    /// Select the stable worker nonce for a key that has no binding yet.
    fn select(&self, key: &K) -> N;
}

impl<K, N, F> AffinitySelector<K, N> for F
where
    F: Fn(&K) -> N,
{
    fn select(&self, key: &K) -> N {
        self(key)
    }
}

/// Typed rejection of an event that cannot apply to the current pool state.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolError<A: Address> {
    #[error("the event names an unknown worker")]
    UnknownWorker(A::Nonce),
    #[error("the worker nonce is already configured")]
    DuplicateWorker(A::Nonce),
    #[error("a pool-owned sequence is exhausted")]
    SequenceExhausted,
    #[error("the worker factory rejected a configured index")]
    WorkerFactoryIndex { index: usize },
    #[error("a stop observation targeted an unavailable worker")]
    WorkerStoppedWhileUnavailable {
        observed: WorkerStopped<A>,
        phase: WorkerPhase,
    },
    #[error("a creation result targeted a worker that is not installing")]
    CreationResolvedWhileUnavailable {
        observed: WorkerCreationResolved<A::Nonce>,
        phase: WorkerPhase,
    },
    #[error("owned worker proxy shutdown was rejected")]
    ChildShutdownRejected(crate::ChildShutdownRejected<A::Nonce>),
    #[error("a creation result does not belong to a pending stable-child creation")]
    UnexpectedCreation(crate::CreationResolved<A>),
    #[error("a stable-child stop does not belong to the current ownership state")]
    UnexpectedChildStopped(crate::ChildStopped<A>),
    #[error("a worker stop does not belong to the current worker incarnation")]
    UnexpectedWorkerStopped(crate::WorkerStopped<A>),
    #[error("a worker creation result does not belong to a pending worker creation")]
    UnexpectedWorkerCreation(crate::WorkerCreationResolved<A::Nonce>),
    #[error("a child-shutdown rejection does not belong to an outstanding shutdown request")]
    UnexpectedChildShutdownRejection(crate::ChildShutdownRejected<A::Nonce>),
    #[error("worker-proxy creation provenance did not match the pending request")]
    ProxyCreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::CreationResolved<A>,
    },
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::WorkerCreationResolved<A::Nonce>,
    },
}

/// Rejected worker completion preserving the exact owned result.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompletionRejection<N, R> {
    #[error("a completion named an unknown worker")]
    UnknownWorker {
        worker: N,
        assignment: AssignmentId,
        result: R,
    },
    #[error("a completion targeted a worker that cannot complete work")]
    WorkerUnavailable {
        worker: N,
        assignment: AssignmentId,
        phase: WorkerPhase,
        result: R,
    },
    #[error("the completion carries a stale assignment identifier")]
    StaleAssignment {
        worker: N,
        expected: AssignmentId,
        received: AssignmentId,
        result: R,
    },
    #[error("the pool is draining its owned worker proxies")]
    ShuttingDown {
        worker: N,
        assignment: AssignmentId,
        result: R,
    },
}

/// Rejected affinity change preserving the exact owned key.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RebalanceRejection<N, K> {
    #[error("an affinity rebalance named an unknown worker")]
    UnknownWorker { key: K, worker: N },
    #[error("an affinity rebalance targeted a retired worker")]
    RetiredWorker {
        key: K,
        worker: N,
        reason: WorkerRetirement,
    },
    #[error("the pool is draining its owned worker proxies")]
    ShuttingDown { key: K, worker: N },
}

/// Complete worker-pool rejection surface.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolFailure<A: Address, R, K> {
    /// A lifecycle or ownership fact was rejected.
    #[error(transparent)]
    Infrastructure(#[from] PoolError<A>),
    /// A completion command was not accepted.
    #[error(transparent)]
    Completion(CompletionRejection<A::Nonce, R>),
    /// A keyed affinity change was not accepted.
    #[error(transparent)]
    Rebalance(RebalanceRejection<A::Nonce, K>),
}

fn widen_pool_failure<A: Address, R, K>(failure: PoolFailure<A, R, Never>) -> PoolFailure<A, R, K> {
    match failure {
        PoolFailure::Infrastructure(error) => PoolFailure::Infrastructure(error),
        PoolFailure::Completion(error) => PoolFailure::Completion(error),
        PoolFailure::Rebalance(rejection) => match rejection {
            RebalanceRejection::UnknownWorker { key, .. }
            | RebalanceRejection::RetiredWorker { key, .. }
            | RebalanceRejection::ShuttingDown { key, .. } => match key {},
        },
    }
}

fn map_pool_ownership_error<A: Address>(error: OwnershipError<A>) -> PoolError<A> {
    match error {
        OwnershipError::Fleet(FleetError::UnknownChild(nonce)) => PoolError::UnknownWorker(nonce),
        OwnershipError::Fleet(FleetError::DuplicateChild(nonce)) => {
            PoolError::DuplicateWorker(nonce)
        }
        OwnershipError::Fleet(FleetError::SequenceExhausted) => PoolError::SequenceExhausted,
        OwnershipError::FactoryIndex { index } => PoolError::WorkerFactoryIndex { index },
        OwnershipError::ChildShutdownRejected(event) => PoolError::ChildShutdownRejected(event),
        OwnershipError::UnexpectedCreation(event) => PoolError::UnexpectedCreation(event),
        OwnershipError::UnexpectedChildStopped(event) => PoolError::UnexpectedChildStopped(event),
        OwnershipError::UnexpectedWorkerStopped(event) => PoolError::UnexpectedWorkerStopped(event),
        OwnershipError::UnexpectedWorkerCreation(event) => {
            PoolError::UnexpectedWorkerCreation(event)
        }
        OwnershipError::UnexpectedChildShutdownRejection(event) => {
            PoolError::UnexpectedChildShutdownRejection(event)
        }
        OwnershipError::CreationProvenanceMismatch { expected, observed } => {
            PoolError::ProxyCreationProvenanceMismatch { expected, observed }
        }
        OwnershipError::WorkerCreationProvenanceMismatch { expected, observed } => {
            PoolError::WorkerCreationProvenanceMismatch { expected, observed }
        }
    }
}

struct AcceptedJob<A: Address, J, Route> {
    id: JobId,
    payload: J,
    reply_to: Route,
    interruption: Option<PoolInterruption<A>>,
    target: Option<A::Nonce>,
}

struct QueuedJob<A: Address, J, Route> {
    accepted: AcceptedJob<A, J, Route>,
    dispatch_payload: J,
}

enum SlotState<A: Address, J, Route> {
    Installing,
    Idle,
    Stopping,
    Assigned {
        assignment: AssignmentId,
        job: AcceptedJob<A, J, Route>,
    },
    CommandReturned {
        job: AcceptedJob<A, J, Route>,
        dispatch_payload: J,
    },
    Retired {
        reason: WorkerRetirement,
    },
}

struct Slot<A: Address, J, Route> {
    nonce: A::Nonce,
    state: SlotState<A, J, Route>,
}

impl<A: Address, J, Route> Slot<A, J, Route> {
    fn phase(&self) -> WorkerPhase {
        match &self.state {
            SlotState::Installing => WorkerPhase::Installing,
            SlotState::Idle => WorkerPhase::Idle,
            SlotState::Stopping => WorkerPhase::Stopping,
            SlotState::Assigned { assignment, job } => WorkerPhase::Assigned {
                assignment: *assignment,
                job: job.id,
            },
            SlotState::CommandReturned { .. } => WorkerPhase::Installing,
            SlotState::Retired { reason } => WorkerPhase::Retired { reason: *reason },
        }
    }
}

struct PlannedDispatch {
    slot_position: usize,
    job_position: usize,
}

enum Admission {
    Accepted,
    Rejected,
}

/// Complete event algebra of a shutdown-owning [`WorkerPool`].
pub type WorkerPoolEvent<A, D, J, R, Route> = CommandSupervisionEvent<
    User<A, PoolMessage<A, D, J, R, Route>>,
    PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
>;

/// Concrete event sum for a [`KeyedWorkerPool`].
pub type KeyedWorkerPoolEvent<A, D, K, J, R, Route> = CommandSupervisionEvent<
    User<A, KeyedPoolMessage<A, D, K, J, R, Route>>,
    PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
>;

/// Named pool-owned delivery lanes.
pub struct PoolBehaviorSends<A, C, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    ResponseSends: SendEffects,
{
    /// Admission and terminal responses addressed to submitters.
    pub responses: ResponseSends,
    /// Assignments addressed to the selected stable worker proxies.
    pub assignments: Vec<behavior::ChildDelivery<crate::Proxy<C>, behavior::ChildHead>>,
}

impl<A, C, ResponseSends> SendEffects for PoolBehaviorSends<A, C, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    ResponseSends: SendEffects,
{
    fn empty() -> Self {
        Self {
            responses: ResponseSends::empty(),
            assignments: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.responses.append(other.responses);
        self.assignments.append(&mut other.assignments);
    }
}

impl<Event, A, C, ResponseSends> behavior::SendsFor<Event>
    for PoolBehaviorSends<A, C, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    ResponseSends: SendEffects,
{
}

impl<I, RootEvent, Path, A, C, ResponseSends> behavior::InterpretSends<I, RootEvent, Path>
    for PoolBehaviorSends<A, C, ResponseSends>
where
    I: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    ResponseSends: SendEffects + behavior::InterpretSends<I, RootEvent, Path>,
    Vec<behavior::ChildDelivery<crate::Proxy<C>, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, Path>,
    PoolBehaviorSends<A, C, ResponseSends>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.responses, interpreter).await?;
            behavior::InterpretSends::interpret(self.assignments, interpreter).await
        }
    }
}

impl<A, C, ResponseSends>
    SendInput<behavior::ChildDelivery<crate::Proxy<C>, behavior::ChildHead>, Own>
    for PoolBehaviorSends<A, C, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    ResponseSends: SendEffects,
{
    fn emit(&mut self, input: behavior::ChildDelivery<crate::Proxy<C>, behavior::ChildHead>) {
        self.assignments.push(input);
    }
}

/// Pool effects keep responses and assignments in named, independently
/// appendable lanes within the supervised behavior send product.
pub type PoolSends<A, C, Route, ParentPath> = SendLayer<
    SupervisorSends<A, C, ParentPath>,
    PoolBehaviorSends<A, C, <Route as crate::DeliveryRouteProtocol>::Sends>,
>;

/// Complete action type shared by FIFO and keyed worker pools.
pub type PoolActions<A, C, Route, ParentPath> =
    Actions<A, Never, PoolSends<A, C, Route, ParentPath>, Births<ProxyWithParent<C, ParentPath>>>;

enum PoolOwnershipEvent<A: Address> {
    WorkerStopped(WorkerStopped<A>),
    WorkerCreationResolved(WorkerCreationResolved<A::Nonce>),
    ChildStopped(crate::ChildStopped<A>),
    CreationResolved(crate::CreationResolved<A>),
    ShutdownRequested,
    ChildShutdownRejected(crate::ChildShutdownRejected<A::Nonce>),
}

/// A fixed, homogeneous, bounded FIFO worker pool.
///
/// Each configured nonce names one stable supervised proxy. Jobs are assigned
/// only after a successful worker-creation result makes that slot idle. The
/// retained state records an assignment before the corresponding delivery is
/// returned, and a completion must carry the exact assignment token.
///
/// Assignment-sequence exhaustion is a typed [`PoolError::SequenceExhausted`]
/// rejection selected before backlog or slot state is changed. As with any
/// generic Rust API, a user-provided `Clone` implementation can itself unwind;
/// the pool introduces no panic path of its own.
///
/// A worker with any other message protocol cannot form a pool:
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, PoolResponse, Protocol, User, WorkerPool, WorkerPoolProtocol};
///
/// struct Reply;
/// struct WrongWorker;
/// impl Protocol for Reply {
///     type Addr = MailAddr;
///     type Msg = PoolResponse<String, (), MailAddr>;
/// }
/// impl Behavior for Reply {
///     type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// impl Protocol for WrongWorker {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// impl Behavior for WrongWorker {
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<behavior::Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
///
/// type PoolProtocol = WorkerPoolProtocol<MailAddr, Reply, String, ()>;
/// // `WrongWorker::Protocol::Msg` is not `PoolAssignment<PoolProtocol>`.
/// let _: Option<WorkerPool<MailAddr, Reply, String, (), WrongWorker>> = None;
/// ```
struct PoolState<A: Address, D, J, R, C, Route, P, ParentPath>
where
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
    Route: DeliveryRoute<D>,
{
    supervisor: FixedFleetOwnership<A, C, ParentPath>,
    complete_to: Recipient<P>,
    slots: Vec<Slot<A, J, Route>>,
    backlog: VecDeque<QueuedJob<A, J, Route>>,
    backlog_capacity: usize,
    next_assignment: u64,
    interruption: InterruptionPolicy,
    response_contract: core::marker::PhantomData<fn(D, R)>,
}

impl<A, D, J, R, C, Route, P, ParentPath> PoolState<A, D, J, R, C, Route, P, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
    Route: DeliveryRoute<D>,
{
    /// Construct a pool after proving that every configured child route is
    /// unique.
    ///
    /// # Errors
    ///
    /// Returns [`PoolConfigError::NoWorkers`] for an empty topology or
    /// [`PoolConfigError::DuplicateWorker`] for the first repeated
    /// creator-local nonce. No behavior or creation request is produced.
    pub fn with_parent(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        complete_to: Recipient<P>,
        proxy_parent: ProxyParentIngress<A, ParentPath>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        let ChildTopology { nonces, build } = topology;
        let count = nonces.len();
        if count == 0 {
            return Err(PoolConfigError::NoWorkers);
        }
        let mut slots = Vec::with_capacity(count);
        for &nonce in &nonces {
            if slots
                .iter()
                .any(|slot: &Slot<A, J, Route>| slot.nonce == nonce)
            {
                return Err(PoolConfigError::DuplicateWorker(nonce));
            }
            slots.push(Slot {
                nonce,
                state: SlotState::Installing,
            });
        }
        Ok(Self {
            supervisor: FixedFleetOwnership::new(
                ChildTopology::new(nonces, build),
                RestartConfiguration::new(
                    Strategy::OneForOne,
                    configuration.restart_policy,
                    configuration.maximum_restarts,
                    configuration.restart_window,
                ),
                proxy_parent,
            )
            .map_err(|error| match error {
                FleetError::UnknownChild(nonce) | FleetError::DuplicateChild(nonce) => {
                    PoolConfigError::DuplicateWorker(nonce)
                }
                FleetError::SequenceExhausted => PoolConfigError::SequenceExhausted,
            })?,
            complete_to,
            slots,
            backlog: VecDeque::new(),
            backlog_capacity: configuration.backlog_capacity,
            next_assignment: 0,
            interruption: configuration.interruption,
            response_contract: core::marker::PhantomData,
        })
    }

    #[must_use]
    pub fn backlog_len(&self) -> usize {
        self.backlog.len()
    }

    #[must_use]
    pub fn worker_phase(&self, worker: A::Nonce) -> Option<WorkerPhase> {
        self.slots
            .iter()
            .find(|slot| slot.nonce == worker)
            .map(Slot::phase)
    }

    fn slot_position(&self, worker: A::Nonce) -> Result<usize, PoolError<A>> {
        self.slots
            .iter()
            .position(|slot| slot.nonce == worker)
            .ok_or(PoolError::UnknownWorker(worker))
    }

    fn initialize_actions(
        &mut self,
    ) -> Result<PoolActions<A, C, Route, ParentPath>, PoolFailure<A, R, Never>> {
        let actions = self
            .supervisor
            .initialize()
            .map_err(map_pool_ownership_error)?;
        Ok(Actions::new(
            SendLayer::new(actions.sends, PoolBehaviorSends::empty()),
            actions.creates,
            actions.become_,
        ))
    }
}

impl<A, D, J, R, C, Route, P, ParentPath> PoolState<A, D, J, R, C, Route, P, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
    Route: DeliveryRoute<D> + Clone,
{
    fn supervisor_transition(
        &mut self,
        event: PoolOwnershipEvent<A>,
    ) -> Result<PoolActions<A, C, Route, ParentPath>, PoolError<A>> {
        let fold = match event {
            PoolOwnershipEvent::WorkerStopped(event) => self.supervisor.worker_stopped(event),
            PoolOwnershipEvent::WorkerCreationResolved(event) => {
                self.supervisor.worker_creation_resolved(event)
            }
            PoolOwnershipEvent::ChildStopped(event) => self.supervisor.child_stopped(event),
            PoolOwnershipEvent::CreationResolved(event) => self.supervisor.creation_resolved(event),
            PoolOwnershipEvent::ShutdownRequested => Ok(self.supervisor.shutdown()),
            PoolOwnershipEvent::ChildShutdownRejected(event) => {
                self.supervisor.child_shutdown_rejected(event)
            }
        }
        .map_err(map_pool_ownership_error)?;
        Ok(Actions::new(
            SendLayer::new(fold.actions.sends, PoolBehaviorSends::empty()),
            fold.actions.creates,
            fold.actions.become_,
        ))
    }

    fn submit(
        &mut self,
        job: JobId,
        payload: J,
        reply_to: Route,
        actions: &mut PoolActions<A, C, Route, ParentPath>,
    ) {
        let can_dispatch = self
            .slots
            .iter()
            .any(|slot| matches!(slot.state, SlotState::Idle));
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
                .inner
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::BacklogFull,
                }));
            return;
        }
        let dispatch_payload = payload.clone();
        self.backlog.push_back(QueuedJob {
            accepted: AcceptedJob {
                id: job,
                payload,
                reply_to: reply_to.clone(),
                interruption: None,
                target: None,
            },
            dispatch_payload,
        });
        actions
            .sends
            .inner
            .responses
            .append(reply_to.deliver(PoolResponse::Accepted { job }));
    }

    fn submit_to(
        &mut self,
        target: A::Nonce,
        job: JobId,
        payload: J,
        reply_to: Route,
        actions: &mut PoolActions<A, C, Route, ParentPath>,
    ) -> Admission {
        let Some(slot) = self.slots.iter().find(|slot| slot.nonce == target) else {
            actions
                .sends
                .inner
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::AffinityUnavailable,
                }));
            return Admission::Rejected;
        };
        if matches!(slot.state, SlotState::Retired { .. }) {
            actions
                .sends
                .inner
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::AffinityUnavailable,
                }));
            return Admission::Rejected;
        }
        let can_dispatch = matches!(slot.state, SlotState::Idle);
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
                .inner
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::BacklogFull,
                }));
            return Admission::Rejected;
        }
        let dispatch_payload = payload.clone();
        self.backlog.push_back(QueuedJob {
            accepted: AcceptedJob {
                id: job,
                payload,
                reply_to: reply_to.clone(),
                interruption: None,
                target: Some(target),
            },
            dispatch_payload,
        });
        actions
            .sends
            .inner
            .responses
            .append(reply_to.deliver(PoolResponse::Accepted { job }));
        Admission::Accepted
    }

    fn complete(
        &mut self,
        worker: A::Nonce,
        assignment: AssignmentId,
        result: R,
        actions: &mut PoolActions<A, C, Route, ParentPath>,
    ) -> Result<(), CompletionRejection<A::Nonce, R>> {
        let Some(position) = self.slots.iter().position(|slot| slot.nonce == worker) else {
            return Err(CompletionRejection::UnknownWorker {
                worker,
                assignment,
                result,
            });
        };
        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Idle);
        let job = match previous {
            SlotState::Assigned {
                assignment: expected,
                job,
            } if expected == assignment => job,
            SlotState::Assigned {
                assignment: expected,
                job,
            } => {
                self.slots[position].state = SlotState::Assigned {
                    assignment: expected,
                    job,
                };
                return Err(CompletionRejection::StaleAssignment {
                    worker,
                    expected,
                    received: assignment,
                    result,
                });
            }
            state => {
                let phase = match &state {
                    SlotState::Installing => WorkerPhase::Installing,
                    SlotState::Idle => WorkerPhase::Idle,
                    SlotState::Stopping => WorkerPhase::Stopping,
                    SlotState::Assigned { assignment, job } => WorkerPhase::Assigned {
                        assignment: *assignment,
                        job: job.id,
                    },
                    SlotState::CommandReturned { .. } => WorkerPhase::Installing,
                    SlotState::Retired { reason } => WorkerPhase::Retired { reason: *reason },
                };
                self.slots[position].state = state;
                return Err(CompletionRejection::WorkerUnavailable {
                    worker,
                    assignment,
                    phase,
                    result,
                });
            }
        };
        actions
            .sends
            .inner
            .responses
            .append(job.reply_to.deliver(PoolResponse::Completed {
                job: job.id,
                result,
            }));
        Ok(())
    }

    fn worker_stopped(
        &mut self,
        stopped: &WorkerStopped<A>,
        responses: &mut Route::Sends,
    ) -> Result<(), PoolError<A>> {
        self.validate_worker_stopped(stopped)?;
        let position = self.slot_position(stopped.proxy)?;
        if self.interruption == InterruptionPolicy::Retry {
            // Cloning a user payload can execute arbitrary `Clone` code. Prepare it
            // before committing the slot transition so unwinding cannot leave the
            // worker marked as installing while its accepted job is lost.
            let dispatch_payload = match &self.slots[position].state {
                SlotState::Assigned { job, .. } => Some(job.payload.clone()),
                SlotState::CommandReturned { .. } | SlotState::Idle => None,
                SlotState::Installing | SlotState::Stopping | SlotState::Retired { .. } => {
                    return Ok(());
                }
            };
            let previous =
                core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
            match (previous, dispatch_payload) {
                (
                    SlotState::Assigned {
                        assignment: _,
                        mut job,
                    },
                    Some(dispatch_payload),
                ) => {
                    job.interruption = Some(PoolInterruption::WorkerStopped {
                        worker: stopped.proxy,
                        outcome: stopped.outcome,
                    });
                    self.backlog.push_front(QueuedJob {
                        accepted: job,
                        dispatch_payload,
                    });
                }
                (
                    SlotState::CommandReturned {
                        mut job,
                        dispatch_payload,
                        ..
                    },
                    None,
                ) => {
                    job.interruption = Some(PoolInterruption::WorkerStopped {
                        worker: stopped.proxy,
                        outcome: stopped.outcome,
                    });
                    self.backlog.push_front(QueuedJob {
                        accepted: job,
                        dispatch_payload,
                    });
                }
                (SlotState::Idle, None) => {}
                (state, _) => self.slots[position].state = state,
            }
            return Ok(());
        }

        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
        match previous {
            SlotState::Assigned { assignment: _, job } => {
                responses.append(job.reply_to.deliver(PoolResponse::Interrupted {
                    job: job.id,
                    payload: job.payload,
                    reason: PoolInterruption::WorkerStopped {
                        worker: stopped.proxy,
                        outcome: stopped.outcome,
                    },
                }));
            }
            SlotState::CommandReturned { job, .. } => {
                responses.append(job.reply_to.deliver(PoolResponse::Interrupted {
                    job: job.id,
                    payload: job.payload,
                    reason: PoolInterruption::WorkerStopped {
                        worker: stopped.proxy,
                        outcome: stopped.outcome,
                    },
                }));
            }
            SlotState::Idle => {}
            state => self.slots[position].state = state,
        }
        Ok(())
    }

    fn validate_worker_stopped(&self, stopped: &WorkerStopped<A>) -> Result<(), PoolError<A>> {
        let position = self.slot_position(stopped.proxy)?;
        match self.slots[position].state {
            SlotState::Idle | SlotState::Assigned { .. } | SlotState::CommandReturned { .. } => {
                Ok(())
            }
            _ => Err(PoolError::WorkerStoppedWhileUnavailable {
                observed: stopped.clone(),
                phase: self.slots[position].phase(),
            }),
        }
    }

    fn command_unavailable(&mut self, returned: crate::ProxyUnavailable<A, PoolAssignment<P>>) {
        let PoolAssignment {
            assignment,
            job,
            payload,
            worker,
            ..
        } = returned.command;
        let Some(position) = self.slots.iter().position(|slot| slot.nonce == worker) else {
            return;
        };
        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
        self.slots[position].state = match previous {
            SlotState::Assigned {
                assignment: expected,
                job: accepted,
            } if expected == assignment && accepted.id == job => SlotState::CommandReturned {
                job: accepted,
                dispatch_payload: payload,
            },
            state => state,
        };
    }

    fn fail_backlog_if_irrecoverable(
        &mut self,
        actions: &mut PoolActions<A, C, Route, ParentPath>,
    ) {
        if self
            .slots
            .iter()
            .any(|slot| !matches!(slot.state, SlotState::Retired { .. }))
        {
            return;
        }
        for queued in self.backlog.drain(..) {
            let job = queued.accepted;
            actions.sends.inner.responses.append(
                job.reply_to.deliver(PoolResponse::Interrupted {
                    job: job.id,
                    payload: job.payload,
                    reason: job
                        .interruption
                        .unwrap_or(PoolInterruption::NoRecoverableWorkers),
                }),
            );
        }
    }

    fn fail_jobs_for_retired_slot(
        &mut self,
        worker: A::Nonce,
        reason: WorkerRetirement,
        actions: &mut PoolActions<A, C, Route, ParentPath>,
    ) {
        let mut retained = VecDeque::with_capacity(self.backlog.len());
        while let Some(queued) = self.backlog.pop_front() {
            if queued.accepted.target == Some(worker) {
                let job = queued.accepted;
                actions.sends.inner.responses.append(
                    job.reply_to.deliver(PoolResponse::Interrupted {
                        job: job.id,
                        payload: job.payload,
                        reason: job
                            .interruption
                            .unwrap_or(PoolInterruption::AffinityRetired { worker, reason }),
                    }),
                );
            } else {
                retained.push_back(queued);
            }
        }
        self.backlog = retained;
    }

    fn retire_slot(&mut self, position: usize, reason: WorkerRetirement) {
        self.slots[position].state = SlotState::Retired { reason };
    }

    fn interrupt_all_for_shutdown(&mut self) -> Route::Sends {
        let mut responses = Route::Sends::empty();
        for slot in &mut self.slots {
            let previous = core::mem::replace(&mut slot.state, SlotState::Stopping);
            match previous {
                SlotState::Assigned { job, .. } => {
                    responses.append(job.reply_to.deliver(PoolResponse::Interrupted {
                        job: job.id,
                        payload: job.payload,
                        reason: PoolInterruption::PoolShutdown,
                    }))
                }
                SlotState::CommandReturned { job, .. } => {
                    responses.append(job.reply_to.deliver(PoolResponse::Interrupted {
                        job: job.id,
                        payload: job.payload,
                        reason: PoolInterruption::PoolShutdown,
                    }))
                }
                retired @ SlotState::Retired { .. } => slot.state = retired,
                SlotState::Installing | SlotState::Idle | SlotState::Stopping => {}
            }
        }
        for queued in self.backlog.drain(..) {
            let job = queued.accepted;
            responses.append(job.reply_to.deliver(PoolResponse::Interrupted {
                job: job.id,
                payload: job.payload,
                reason: PoolInterruption::PoolShutdown,
            }));
        }
        responses
    }

    fn creation_resolved(
        &mut self,
        resolved: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<(), PoolError<A>> {
        self.validate_creation_resolved(resolved)?;
        let position = self.slot_position(resolved.proxy)?;
        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
        self.slots[position].state = match (previous, resolved.result) {
            (SlotState::Installing, Ok(())) => SlotState::Idle,
            (SlotState::Installing, Err(rejection)) => SlotState::Retired {
                reason: WorkerRetirement::CreationRejected(rejection),
            },
            (state, _) => state,
        };
        Ok(())
    }

    fn validate_creation_resolved(
        &self,
        resolved: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<(), PoolError<A>> {
        let position = self.slot_position(resolved.proxy)?;
        let phase = self.slots[position].phase();
        if matches!(self.slots[position].state, SlotState::Installing) {
            Ok(())
        } else {
            Err(PoolError::CreationResolvedWhileUnavailable {
                observed: *resolved,
                phase,
            })
        }
    }

    fn dispatch(
        &mut self,
        actions: &mut PoolActions<A, C, Route, ParentPath>,
    ) -> Result<(), PoolError<A>> {
        let mut selected_jobs = Vec::new();
        let mut plan = Vec::new();
        for (slot_position, slot) in self.slots.iter().enumerate() {
            if !matches!(slot.state, SlotState::Idle) {
                continue;
            }
            let Some(job_position) = self.backlog.iter().enumerate().find_map(|(position, job)| {
                (!selected_jobs.contains(&position)
                    && job
                        .accepted
                        .target
                        .is_none_or(|target| target == slot.nonce))
                .then_some(position)
            }) else {
                continue;
            };
            selected_jobs.push(job_position);
            plan.push(PlannedDispatch {
                slot_position,
                job_position,
            });
        }
        let count = u64::try_from(plan.len()).map_err(|_| PoolError::SequenceExhausted)?;
        let next_assignment = self
            .next_assignment
            .checked_add(count)
            .ok_or(PoolError::SequenceExhausted)?;

        let mut selected_by_position = BTreeMap::new();
        for planned in plan {
            selected_by_position.insert(planned.job_position, planned.slot_position);
        }
        let mut selected_by_slot: Vec<Option<QueuedJob<A, J, Route>>> =
            std::iter::repeat_with(|| None)
                .take(self.slots.len())
                .collect();
        let mut remaining = VecDeque::new();
        for (position, queued) in self.backlog.drain(..).enumerate() {
            if let Some(slot_position) = selected_by_position.remove(&position) {
                selected_by_slot[slot_position] = Some(queued);
            } else {
                remaining.push_back(queued);
            }
        }
        self.backlog = remaining;

        for (assignment, (slot_position, queued)) in (self.next_assignment..next_assignment).zip(
            selected_by_slot
                .into_iter()
                .enumerate()
                .filter_map(|(slot_position, queued)| queued.map(|queued| (slot_position, queued))),
        ) {
            let payload = queued.dispatch_payload;
            let job = queued.accepted;
            let assignment = AssignmentId(assignment);
            let nonce = self.slots[slot_position].nonce;
            let route =
                ChildRoute::<ProxyWithParent<C, ParentPath>, behavior::ChildHead>::new(nonce);
            let job_id = job.id;
            self.slots[slot_position].state = SlotState::Assigned { assignment, job };
            actions
                .sends
                .inner
                .send::<behavior::ChildDelivery<crate::Proxy<C>, behavior::ChildHead>, Own>(
                    behavior::ChildDelivery::at(
                        route,
                        ProxyCommand::Forward {
                            command: PoolAssignment {
                                assignment,
                                job: job_id,
                                payload,
                                worker: nonce,
                                complete_to: self.complete_to,
                            },
                            unavailable_to: Recipient::<
                                MessageProtocol<A, crate::ProxyUnavailable<A, PoolAssignment<P>>>,
                            >::global(
                                self.complete_to.address()
                            ),
                        },
                    ),
                );
        }
        self.next_assignment = next_assignment;
        Ok(())
    }
}

/// Public FIFO worker-pool behavior with its completion protocol fixed by the
/// pool's own message signature.
pub struct WorkerPoolWithParent<A: Address, D, J, R, C, Route, ParentPath>
where
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
        >,
{
    core: PoolState<A, D, J, R, C, Route, crate::WorkerPoolProtocol<A, D, J, R, Route>, ParentPath>,
}

/// A FIFO worker pool whose proxy reports target its direct event layer.
pub type WorkerPool<A, D, J, R, C, Route> =
    WorkerPoolWithParent<A, D, J, R, C, Route, behavior::Here>;

impl<A, D, J, R, C, Route, ParentPath> crate::BehaviorBase
    for WorkerPoolWithParent<A, D, J, R, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
        >,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, D, J, R, C, Route, ParentPath> WorkerPoolWithParent<A, D, J, R, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
        >,
{
    /// Construct a pool whose completion destination implements this exact
    /// pool protocol.
    pub fn with_parent(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        complete_to: Recipient<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
        proxy_parent: ProxyParentIngress<A, ParentPath>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        PoolState::with_parent(topology, configuration, complete_to, proxy_parent)
            .map(|core| Self { core })
    }

    #[must_use]
    pub fn backlog_len(&self) -> usize {
        self.core.backlog_len()
    }

    #[must_use]
    pub fn worker_phase(&self, worker: A::Nonce) -> Option<WorkerPhase> {
        self.core.worker_phase(worker)
    }
}

impl<A, D, J, R, C, Route> WorkerPoolWithParent<A, D, J, R, C, Route, behavior::Here>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
        >,
{
    /// Construct a pool whose proxy reports target the pool's direct event layer.
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        complete_to: Recipient<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Self::with_parent(
            topology,
            configuration,
            complete_to,
            ProxyParentIngress::new(),
        )
    }
}

impl<A, D, J, R, C, Route, ParentPath> Behavior
    for WorkerPoolWithParent<A, D, J, R, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R, Route>>,
        >,
{
    type Protocol = crate::WorkerPoolProtocol<A, D, J, R, Route>;
    type Event = WorkerPoolEvent<A, D, J, R, Route>;
    type Sends = PoolSends<A, C, Route, ParentPath>;
    type Ph = Never;
    type Error = PoolFailure<A, R, Never>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.core.initialize_actions()
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            CommandSupervisionEvent::Behavior(User {
                message:
                    PoolMessage::Submit {
                        job,
                        payload,
                        reply_to,
                    },
                ..
            }) => {
                if self.core.supervisor.is_shutting_down() {
                    let mut actions: PoolActions<A, C, Route, ParentPath> = Actions::cont();
                    actions.sends.inner.responses.append(reply_to.deliver(
                        PoolResponse::Rejected {
                            job,
                            payload,
                            reason: PoolRejection::ShuttingDown,
                        },
                    ));
                    return Ok(actions);
                }
                let mut actions = Actions::cont();
                self.core.submit(job, payload, reply_to, &mut actions);
                self.core.dispatch(&mut actions)?;
                Ok(actions)
            }
            CommandSupervisionEvent::Behavior(User {
                message:
                    PoolMessage::Completed {
                        worker,
                        assignment,
                        result,
                    },
                ..
            }) => {
                if self.core.supervisor.is_shutting_down() {
                    return Err(PoolFailure::Completion(CompletionRejection::ShuttingDown {
                        worker,
                        assignment,
                        result,
                    }));
                }
                let mut actions = Actions::cont();
                self.core
                    .complete(worker, assignment, result, &mut actions)
                    .map_err(PoolFailure::Completion)?;
                self.core.dispatch(&mut actions)?;
                Ok(actions)
            }
            CommandSupervisionEvent::WorkerStopped(stopped) => {
                if self.core.supervisor.is_shutting_down() {
                    return Ok(self
                        .core
                        .supervisor_transition(PoolOwnershipEvent::WorkerStopped(stopped))?);
                }
                self.core
                    .supervisor
                    .validate_worker_stopped(&stopped)
                    .map_err(map_pool_ownership_error)?;
                self.core.validate_worker_stopped(&stopped)?;
                let proxy = stopped.proxy;
                let mut responses = Route::Sends::empty();
                self.core.worker_stopped(&stopped, &mut responses)?;
                let mut actions = self
                    .core
                    .supervisor_transition(PoolOwnershipEvent::WorkerStopped(stopped))?;
                actions.sends.inner.responses.append(responses);
                let replacement_requested = actions
                    .sends
                    .owned
                    .replacement_commands
                    .iter()
                    .any(|delivery| delivery.nonce == proxy);
                if !replacement_requested {
                    let position = self.core.slot_position(proxy)?;
                    let reason = WorkerRetirement::ReplacementUnavailable;
                    self.core.retire_slot(position, reason);
                    self.core
                        .fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.core.dispatch(&mut actions)?;
                self.core.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            CommandSupervisionEvent::WorkerCreationResolved(resolved) => {
                if self.core.supervisor.is_shutting_down() {
                    return Ok(self.core.supervisor_transition(
                        PoolOwnershipEvent::WorkerCreationResolved(resolved),
                    )?);
                }
                self.core
                    .supervisor
                    .validate_worker_creation_resolved(&resolved)
                    .map_err(map_pool_ownership_error)?;
                self.core.validate_creation_resolved(&resolved)?;
                let proxy = resolved.proxy;
                self.core.creation_resolved(&resolved)?;
                let mut actions = self
                    .core
                    .supervisor_transition(PoolOwnershipEvent::WorkerCreationResolved(resolved))?;
                if let Some(WorkerPhase::Retired { reason }) = self.core.worker_phase(proxy) {
                    self.core
                        .fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.core.dispatch(&mut actions)?;
                self.core.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            CommandSupervisionEvent::ChildStopped(stopped) => Ok(self
                .core
                .supervisor_transition(PoolOwnershipEvent::ChildStopped(stopped))?),
            CommandSupervisionEvent::CreationResolved(resolved) => Ok(self
                .core
                .supervisor_transition(PoolOwnershipEvent::CreationResolved(resolved))?),
            CommandSupervisionEvent::ShutdownRequested(_) => {
                let responses = self.core.interrupt_all_for_shutdown();
                let mut actions = self
                    .core
                    .supervisor_transition(PoolOwnershipEvent::ShutdownRequested)?;
                actions.sends.inner.responses.append(responses);
                Ok(actions)
            }
            CommandSupervisionEvent::CommandUnavailable(User { message, .. }) => {
                self.core.command_unavailable(message);
                let mut actions = Actions::cont();
                if !self.core.supervisor.is_shutting_down() {
                    self.core.dispatch(&mut actions)?;
                    self.core.fail_backlog_if_irrecoverable(&mut actions);
                }
                Ok(actions)
            }
            CommandSupervisionEvent::ChildShutdownRejected(rejected) => Ok(self
                .core
                .supervisor_transition(PoolOwnershipEvent::ChildShutdownRejected(rejected))?),
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::items_after_test_module,
    reason = "the local fixed-pool regression sits beside that implementation"
)]
mod tests {
    use super::*;
    use crate::{MailAddr, NoBirths, WorkerPoolProtocol};

    struct TestReply;

    impl behavior::Protocol for TestReply {
        type Addr = MailAddr;
        type Msg = PoolResponse<u8, (), MailAddr>;
    }

    impl Behavior for TestReply {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[derive(Clone, Copy)]
    struct TestWorker;

    impl behavior::Protocol for TestWorker {
        type Addr = MailAddr;
        type Msg =
            PoolAssignment<WorkerPoolProtocol<MailAddr, TestReply, u8, (), Recipient<TestReply>>>;
    }

    impl Behavior for TestWorker {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn test_worker(_: usize) -> TestWorker {
        TestWorker
    }

    #[test]
    fn path_aware_pool_binds_both_proxy_reports_to_the_supplied_parent_ingress() {
        type ParentPath = behavior::Inside<behavior::Here>;
        let parent = ProxyParentIngress::<MailAddr, behavior::Here>::new().inside();
        let mut pool = WorkerPoolWithParent::<
            MailAddr,
            TestReply,
            u8,
            (),
            TestWorker,
            Recipient<TestReply>,
            ParentPath,
        >::with_parent(
            ChildTopology::indexed(|_| 1, 1, |_| Some(TestWorker)),
            PoolConfiguration::new(
                0,
                InterruptionPolicy::Fail,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
            ),
            Recipient::global(MailAddr(92)),
            parent,
        )
        .unwrap();

        let initialized = behavior::initialize(&mut pool).unwrap();
        let mut proxy = initialized.creates.into_iter().next().unwrap().child;
        let proxy_initialized = behavior::initialize(&mut proxy).unwrap();
        let report = behavior::delegate_transition(
            &mut proxy,
            crate::ProxyEvent::CreationResolved(crate::CreationResolved::birth(0, MailAddr(10))),
        )
        .unwrap();

        assert_eq!(proxy_initialized.sends.stopped_reports.len(), 0);
        assert_eq!(report.sends.creation_reports[0].ingress, parent.creation);
        let stopped = behavior::delegate_transition(
            &mut proxy,
            crate::ProxyEvent::ChildStopped(crate::ChildStopped::new(
                0,
                Ok(Exit::Normal),
                std::time::Instant::now(),
            )),
        )
        .unwrap();
        assert_eq!(stopped.sends.stopped_reports[0].ingress, parent.stopped);
    }

    #[test]
    fn one_dispatch_batch_preserves_fifo_jobs_across_index_removal() {
        let mut pool = WorkerPoolWithParent::with_parent(
            ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                2,
                |index| Some(test_worker(index)),
            ),
            PoolConfiguration::new(
                3,
                InterruptionPolicy::Fail,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
            ),
            Recipient::global(MailAddr(92)),
            ProxyParentIngress::new(),
        )
        .unwrap();
        behavior::initialize(&mut pool).unwrap();

        for job in 1..=3 {
            behavior::delegate_transition(
                &mut pool,
                CommandSupervisionEvent::Behavior(User::new(
                    MailAddr(90),
                    PoolMessage::Submit {
                        job: JobId(job),
                        payload: u8::try_from(job).unwrap(),
                        reply_to: Recipient::global(MailAddr(91)),
                    },
                )),
            )
            .unwrap();
        }
        pool.core.slots[0].state = SlotState::Idle;
        pool.core.slots[1].state = SlotState::Idle;

        let mut actions: PoolActions<MailAddr, TestWorker, Recipient<TestReply>, behavior::Here> =
            Actions::cont();
        pool.core.dispatch(&mut actions).unwrap();

        let assignments = &actions.sends.inner.assignments;
        assert_eq!(assignments.len(), 2);
        for (index, expected_job) in [JobId(1), JobId(2)].into_iter().enumerate() {
            assert_eq!(assignments[index].nonce, u64::try_from(index).unwrap());
            let ProxyCommand::Forward {
                command: assignment,
                ..
            } = &assignments[index].message
            else {
                panic!("pool dispatches with Forward");
            };
            assert_eq!(
                assignment.assignment,
                AssignmentId(u64::try_from(index).unwrap())
            );
            assert_eq!(assignment.job, expected_job);
        }
        assert_eq!(pool.core.backlog.len(), 1);
        assert_eq!(pool.core.backlog[0].accepted.id, JobId(3));
    }
}

/// A worker pool whose admitted keys remain bound to stable worker slots.
///
/// The selector chooses a stable proxy nonce only when a key is first
/// admitted. Replacement incarnations remain behind that proxy, so they do
/// not alter affinity. [`KeyedPoolMessage::Rebalance`] is the sole transition
/// that changes an established binding, and jobs accepted before it retain
/// their original target.
///
/// Keys must have a concrete equality relation; a key type without `Eq` cannot
/// form an affinity table:
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, KeyedWorkerPool, KeyedWorkerPoolProtocol, MailAddr, Never, NoBirths, PoolResponse, Protocol, User};
/// struct NonKey(f64);
/// struct Reply;
/// struct Worker;
/// impl Protocol for Reply {
///     type Addr = MailAddr;
///     type Msg = PoolResponse<u8, (), MailAddr>;
/// }
/// impl Behavior for Reply {
///     type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// type PoolProtocol = KeyedWorkerPoolProtocol<MailAddr, Reply, NonKey, u8, ()>;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = behavior::PoolAssignment<PoolProtocol>,
///     sends = Vec<Never>,
///     births = NoBirths,
///     error = Never,
/// )]
/// impl Worker {
///     fn init(&mut self, _: crate::InitializationTurn) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: behavior::PoolAssignment<PoolProtocol>) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
/// }
/// let _: Option<KeyedWorkerPool<MailAddr, Reply, NonKey, u8, (), Worker, fn(&NonKey) -> u64>> = None;
/// ```
pub struct KeyedWorkerPoolWithParent<A: Address, D, K, J, R, C, Route, S, ParentPath>
where
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    pool: PoolState<
        A,
        D,
        J,
        R,
        C,
        Route,
        crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>,
        ParentPath,
    >,
    bindings: Vec<(K, A::Nonce)>,
    selector: S,
}

/// A keyed pool whose proxies report to the pool's direct event layer.
pub type KeyedWorkerPool<A, D, K, J, R, C, Route, S> =
    KeyedWorkerPoolWithParent<A, D, K, J, R, C, Route, S, behavior::Here>;

impl<A, D, K, J, R, C, Route, S, ParentPath> crate::BehaviorBase
    for KeyedWorkerPoolWithParent<A, D, K, J, R, C, Route, S, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, D, K, J, R, C, Route, S, ParentPath>
    KeyedWorkerPoolWithParent<A, D, K, J, R, C, Route, S, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    /// Construct a key-persistent pool over the same fixed supervised slots as
    /// [`WorkerPool`]. The selector is pure and is consulted once per
    /// previously unseen key. It chooses behavior policy; runtime route
    /// resolution remains outside this type.
    ///
    /// # Errors
    ///
    /// Returns [`PoolConfigError::NoWorkers`] for an empty topology or
    /// [`PoolConfigError::DuplicateWorker`] for a repeated stable nonce.
    pub fn with_parent(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        selector: S,
        complete_to: Recipient<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
        proxy_parent: ProxyParentIngress<A, ParentPath>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Ok(Self {
            pool: PoolState::with_parent(topology, configuration, complete_to, proxy_parent)?,
            bindings: Vec::new(),
            selector,
        })
    }

    /// Return the stable slot currently bound to `key`.
    #[must_use]
    pub fn affinity(&self, key: &K) -> Option<A::Nonce> {
        self.bindings
            .iter()
            .find_map(|(bound, worker)| (bound == key).then_some(*worker))
    }

    #[must_use]
    pub fn backlog_len(&self) -> usize {
        self.pool.backlog_len()
    }

    #[must_use]
    pub fn worker_phase(&self, worker: A::Nonce) -> Option<WorkerPhase> {
        self.pool.worker_phase(worker)
    }

    fn rebalance(
        &mut self,
        key: K,
        worker: A::Nonce,
    ) -> Result<(), RebalanceRejection<A::Nonce, K>> {
        let Some(position) = self.pool.slots.iter().position(|slot| slot.nonce == worker) else {
            return Err(RebalanceRejection::UnknownWorker { key, worker });
        };
        if let SlotState::Retired { reason } = self.pool.slots[position].state {
            return Err(RebalanceRejection::RetiredWorker {
                key,
                worker,
                reason,
            });
        }
        if let Some((_, bound)) = self.bindings.iter_mut().find(|(bound, _)| *bound == key) {
            *bound = worker;
        } else {
            self.bindings.push((key, worker));
        }
        Ok(())
    }
}

impl<A, D, K, J, R, C, Route, S>
    KeyedWorkerPoolWithParent<A, D, K, J, R, C, Route, S, behavior::Here>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        selector: S,
        complete_to: Recipient<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Self::with_parent(
            topology,
            configuration,
            selector,
            complete_to,
            ProxyParentIngress::new(),
        )
    }
}

impl<A, D, K, J, R, C, Route, S, ParentPath> Behavior
    for KeyedWorkerPoolWithParent<A, D, K, J, R, C, Route, S, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    K: Eq,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    type Protocol = crate::KeyedWorkerPoolProtocol<A, D, K, J, R, Route>;
    type Event = KeyedWorkerPoolEvent<A, D, K, J, R, Route>;
    type Sends = PoolSends<A, C, Route, ParentPath>;
    type Ph = Never;
    type Error = PoolFailure<A, R, K>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.pool.initialize_actions().map_err(widen_pool_failure)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            CommandSupervisionEvent::Behavior(User {
                message:
                    KeyedPoolMessage::Submit {
                        key,
                        job,
                        payload,
                        reply_to,
                    },
                ..
            }) => {
                if self.pool.supervisor.is_shutting_down() {
                    let mut actions: PoolActions<A, C, Route, ParentPath> = Actions::cont();
                    actions.sends.inner.responses.append(reply_to.deliver(
                        PoolResponse::Rejected {
                            job,
                            payload,
                            reason: PoolRejection::ShuttingDown,
                        },
                    ));
                    return Ok(actions);
                }
                let existing = self.affinity(&key);
                let target = existing.unwrap_or_else(|| self.selector.select(&key));
                let mut actions = Actions::cont();
                let admission = self
                    .pool
                    .submit_to(target, job, payload, reply_to, &mut actions);
                match (admission, existing) {
                    (Admission::Accepted, None) => self.bindings.push((key, target)),
                    (Admission::Accepted | Admission::Rejected, Some(_))
                    | (Admission::Rejected, None) => {}
                }
                self.pool.dispatch(&mut actions)?;
                Ok(actions)
            }
            CommandSupervisionEvent::Behavior(User {
                message:
                    KeyedPoolMessage::Completed {
                        worker,
                        assignment,
                        result,
                    },
                ..
            }) => {
                if self.pool.supervisor.is_shutting_down() {
                    return Err(PoolFailure::Completion(CompletionRejection::ShuttingDown {
                        worker,
                        assignment,
                        result,
                    }));
                }
                let mut actions = Actions::cont();
                self.pool
                    .complete(worker, assignment, result, &mut actions)
                    .map_err(PoolFailure::Completion)?;
                self.pool.dispatch(&mut actions)?;
                Ok(actions)
            }
            CommandSupervisionEvent::Behavior(User {
                message: KeyedPoolMessage::Rebalance { key, worker },
                ..
            }) => {
                if self.pool.supervisor.is_shutting_down() {
                    return Err(PoolFailure::Rebalance(RebalanceRejection::ShuttingDown {
                        key,
                        worker,
                    }));
                }
                self.rebalance(key, worker)
                    .map_err(PoolFailure::Rebalance)?;
                Ok(Actions::cont())
            }
            CommandSupervisionEvent::WorkerStopped(stopped) => {
                if self.pool.supervisor.is_shutting_down() {
                    return Ok(self
                        .pool
                        .supervisor_transition(PoolOwnershipEvent::WorkerStopped(stopped))?);
                }
                self.pool
                    .supervisor
                    .validate_worker_stopped(&stopped)
                    .map_err(map_pool_ownership_error)?;
                self.pool.validate_worker_stopped(&stopped)?;
                let proxy = stopped.proxy;
                let mut responses = Route::Sends::empty();
                self.pool.worker_stopped(&stopped, &mut responses)?;
                let mut actions = self
                    .pool
                    .supervisor_transition(PoolOwnershipEvent::WorkerStopped(stopped))?;
                actions.sends.inner.responses.append(responses);
                let replacement_requested = actions
                    .sends
                    .owned
                    .replacement_commands
                    .iter()
                    .any(|delivery| delivery.nonce == proxy);
                if !replacement_requested {
                    let position = self.pool.slot_position(proxy)?;
                    let reason = WorkerRetirement::ReplacementUnavailable;
                    self.pool.retire_slot(position, reason);
                    self.pool
                        .fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.pool.dispatch(&mut actions)?;
                self.pool.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            CommandSupervisionEvent::WorkerCreationResolved(resolved) => {
                if self.pool.supervisor.is_shutting_down() {
                    return Ok(self.pool.supervisor_transition(
                        PoolOwnershipEvent::WorkerCreationResolved(resolved),
                    )?);
                }
                self.pool
                    .supervisor
                    .validate_worker_creation_resolved(&resolved)
                    .map_err(map_pool_ownership_error)?;
                self.pool.validate_creation_resolved(&resolved)?;
                let proxy = resolved.proxy;
                self.pool.creation_resolved(&resolved)?;
                let mut actions = self
                    .pool
                    .supervisor_transition(PoolOwnershipEvent::WorkerCreationResolved(resolved))?;
                if let Some(WorkerPhase::Retired { reason }) = self.pool.worker_phase(proxy) {
                    self.pool
                        .fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.pool.dispatch(&mut actions)?;
                self.pool.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            CommandSupervisionEvent::ChildStopped(stopped) => Ok(self
                .pool
                .supervisor_transition(PoolOwnershipEvent::ChildStopped(stopped))?),
            CommandSupervisionEvent::CreationResolved(resolved) => Ok(self
                .pool
                .supervisor_transition(PoolOwnershipEvent::CreationResolved(resolved))?),
            CommandSupervisionEvent::ShutdownRequested(_) => {
                let responses = self.pool.interrupt_all_for_shutdown();
                let mut actions = self
                    .pool
                    .supervisor_transition(PoolOwnershipEvent::ShutdownRequested)?;
                actions.sends.inner.responses.append(responses);
                Ok(actions)
            }
            CommandSupervisionEvent::CommandUnavailable(User { message, .. }) => {
                self.pool.command_unavailable(message);
                let mut actions = Actions::cont();
                if !self.pool.supervisor.is_shutting_down() {
                    self.pool.dispatch(&mut actions)?;
                    self.pool.fail_backlog_if_irrecoverable(&mut actions);
                }
                Ok(actions)
            }
            CommandSupervisionEvent::ChildShutdownRejected(rejected) => Ok(self
                .pool
                .supervisor_transition(PoolOwnershipEvent::ChildShutdownRejected(rejected))?),
        }
    }
}
