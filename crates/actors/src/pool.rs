//! A bounded, FIFO worker pool expressed entirely as a pure behavior.
//!
//! Pool scheduling is a derived Bombay construction, not an actor-model
//! primitive. Runtime installation, delivery, and observation remain effects
//! for an interpreter; this module owns only their typed protocol and fold.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::DeliveryRoute;
use crate::composition::RelayChildReports;
use crate::supervision::{FixedFleetOwnership, OwnershipError, OwnershipFold};
use crate::{
    Actions, Address, Behavior, Births, ChildTopology, Crash, CreationRejection, Exit, FleetError,
    Never, Own, Protocol, RestartConfiguration, RestartPolicy, RestartTiming, SendEffects,
    SendInput, Strategy, SupervisionEvent, SupervisorSends, User, WorkerCreationResolved,
    WorkerStopped,
};
use behavior::ChildRoute;
use behavior::{BehaviorLayer, ChildInputIngress};

/// Caller-chosen identity used to correlate pool responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

/// Pool-owned correlation token for one exact dispatch attempt.
///
/// This is not an actor identity or evidence that delivery occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssignmentId(pub u64);

/// One assignment accepted by a worker behavior.
///
/// A worker learns only the job value and its pool-owned correlation token. It
/// neither names nor receives a logical capability for the pool actor.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PoolAssignment<J> {
    pub assignment: AssignmentId,
    pub job: JobId,
    pub payload: J,
}

/// One worker-owned completion value reported through its established parent.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PoolCompletion<R> {
    pub assignment: AssignmentId,
    pub result: R,
}

/// Messages accepted by a pool coordinator.
#[derive(Clone, PartialEq, Eq)]
pub enum PoolMessage<A, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    Submit {
        job: JobId,
        payload: J,
        reply_to: Route,
    },
}

/// Messages accepted by a key-persistent pool coordinator.
///
/// `Rebalance` is the only input that can change an established key binding.
/// It affects later submissions; jobs already accepted retain their selected
/// stable worker slot.
#[derive(Clone, PartialEq, Eq)]
pub enum KeyedPoolMessage<A, K, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    Submit {
        key: K,
        job: JobId,
        payload: J,
        reply_to: Route,
    },
    Rebalance {
        key: K,
        worker: A::Nonce,
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
    /// Timing of an admitted worker replacement.
    pub restart_timing: RestartTiming,
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
        restart_timing: RestartTiming,
    ) -> Self {
        Self {
            backlog_capacity,
            interruption,
            restart_policy,
            maximum_restarts,
            restart_window,
            restart_timing,
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
    #[error("a rejected worker-proxy creation contradicts its worker-creation fact")]
    ContradictoryProxyAndWorkerCreation {
        proxy: crate::CreationResolved<A>,
        worker: crate::WorkerCreationResolved<A::Nonce>,
    },
    #[error("a rejected worker-proxy creation contradicts its worker-stop fact")]
    ContradictoryProxyCreationAndWorkerStop {
        proxy: crate::CreationResolved<A>,
        worker: crate::WorkerStopped<A>,
    },
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::WorkerCreationResolved<A::Nonce>,
    },
    #[error("an exact delayed-restart timer contradicts the retained worker phase")]
    DelayedReplacementStateMismatch {
        event: crate::TimerElapsed,
        child: A::Nonce,
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
    #[error("the completion came from a stale worker incarnation")]
    StaleIncarnation {
        worker: N,
        expected: N,
        observed: N,
        assignment: AssignmentId,
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
pub enum PoolFailure<A: Address, J, R, K> {
    /// A lifecycle or ownership fact was rejected.
    #[error(transparent)]
    Infrastructure(#[from] PoolError<A>),
    /// A completion command was not accepted.
    #[error(transparent)]
    Completion(CompletionRejection<A::Nonce, R>),
    /// A proxy-unavailability fact did not match the assignment still owned by
    /// that stable slot.
    #[error("a returned assignment does not match the pool's current ownership")]
    UnexpectedAssignmentUnavailable(crate::ProxyUnavailable<A, PoolAssignment<J>>),
    /// A keyed affinity change was not accepted.
    #[error(transparent)]
    Rebalance(RebalanceRejection<A::Nonce, K>),
}

fn widen_pool_failure<A: Address, J, R, K>(
    failure: PoolFailure<A, J, R, Never>,
) -> PoolFailure<A, J, R, K> {
    match failure {
        PoolFailure::Infrastructure(error) => PoolFailure::Infrastructure(error),
        PoolFailure::Completion(error) => PoolFailure::Completion(error),
        PoolFailure::UnexpectedAssignmentUnavailable(error) => {
            PoolFailure::UnexpectedAssignmentUnavailable(error)
        }
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
        OwnershipError::ContradictoryStableAndWorkerCreation { proxy, worker } => {
            PoolError::ContradictoryProxyAndWorkerCreation { proxy, worker }
        }
        OwnershipError::ContradictoryStableCreationAndWorkerStop { proxy, worker } => {
            PoolError::ContradictoryProxyCreationAndWorkerStop { proxy, worker }
        }
        OwnershipError::WorkerCreationProvenanceMismatch { expected, observed } => {
            PoolError::WorkerCreationProvenanceMismatch { expected, observed }
        }
        OwnershipError::DelayedReplacementStateMismatch { event, child } => {
            PoolError::DelayedReplacementStateMismatch { event, child }
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
    Vacant,
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

struct ExpectedAssignmentReturn {
    assignment: AssignmentId,
    job: JobId,
}

struct Slot<A: Address, J, Route> {
    nonce: A::Nonce,
    state: SlotState<A, J, Route>,
    expected_returns: Vec<ExpectedAssignmentReturn>,
}

struct PlannedDispatch {
    slot_position: usize,
    job_position: usize,
}

enum Admission {
    Accepted,
    Rejected,
}

type WorkerCompletionFact<A, R> = behavior::ChildReport<A, PoolCompletion<R>>;
type PoolCompletionFact<A, R> = behavior::ChildReport<A, WorkerCompletionFact<A, R>>;

/// Pool-owned input algebra beneath the common supervision lifecycle layer.
pub enum WorkerPoolEvent<A, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    Command(User<A, PoolMessage<A, J, R, Route>>),
    AssignmentUnavailable(crate::ProxyUnavailable<A, PoolAssignment<J>>),
    Completion(PoolCompletionFact<A, R>),
}

impl<A, J, R, Route> behavior::UserEvent for WorkerPoolEvent<A, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    type Addr = A;
    type Message = PoolMessage<A, J, R, Route>;

    fn user(from: A, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }

    fn into_user(self) -> Result<User<A, Self::Message>, Self> {
        match self {
            Self::Command(event) => Ok(event),
            unavailable => Err(unavailable),
        }
    }
}

impl<A, J, R, Route, Stable>
    behavior::EventIngress<
        ChildRoute<Stable, behavior::ChildHead>,
        crate::ProxyUnavailable<A, PoolAssignment<J>>,
    > for WorkerPoolEvent<A, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
    Stable: Behavior<Protocol: Protocol<Addr = A>>,
{
    fn ingress(unavailable: crate::ProxyUnavailable<A, PoolAssignment<J>>) -> Self {
        Self::AssignmentUnavailable(unavailable)
    }
}

impl<A, J, R, Route, Stable>
    behavior::EventIngress<ChildRoute<Stable, behavior::ChildHead>, PoolCompletionFact<A, R>>
    for WorkerPoolEvent<A, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
    Stable: Behavior<Protocol: Protocol<Addr = A>>,
{
    fn ingress(completion: PoolCompletionFact<A, R>) -> Self {
        Self::Completion(completion)
    }
}

/// Keyed-pool input algebra beneath the common supervision lifecycle layer.
pub enum KeyedWorkerPoolEvent<A, K, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    Command(User<A, KeyedPoolMessage<A, K, J, R, Route>>),
    AssignmentUnavailable(crate::ProxyUnavailable<A, PoolAssignment<J>>),
    Completion(PoolCompletionFact<A, R>),
}

impl<A, K, J, R, Route> behavior::UserEvent for KeyedWorkerPoolEvent<A, K, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    type Addr = A;
    type Message = KeyedPoolMessage<A, K, J, R, Route>;

    fn user(from: A, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }

    fn into_user(self) -> Result<User<A, Self::Message>, Self> {
        match self {
            Self::Command(event) => Ok(event),
            unavailable => Err(unavailable),
        }
    }
}

impl<A, K, J, R, Route, Stable>
    behavior::EventIngress<
        ChildRoute<Stable, behavior::ChildHead>,
        crate::ProxyUnavailable<A, PoolAssignment<J>>,
    > for KeyedWorkerPoolEvent<A, K, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
    Stable: Behavior<Protocol: Protocol<Addr = A>>,
{
    fn ingress(unavailable: crate::ProxyUnavailable<A, PoolAssignment<J>>) -> Self {
        Self::AssignmentUnavailable(unavailable)
    }
}

impl<A, K, J, R, Route, Stable>
    behavior::EventIngress<ChildRoute<Stable, behavior::ChildHead>, PoolCompletionFact<A, R>>
    for KeyedWorkerPoolEvent<A, K, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
    Stable: Behavior<Protocol: Protocol<Addr = A>>,
{
    fn ingress(completion: PoolCompletionFact<A, R>) -> Self {
        Self::Completion(completion)
    }
}

/// Complete named effect product emitted by a worker pool.
///
/// Interpretation preserves the authored order: customer responses, worker
/// assignments, then fleet-supervision effects. The product exposes semantic
/// field names instead of the structural position of the pool coordinator
/// beneath its supervision law.
pub struct PoolSends<A, C, Stable, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    ResponseSends: SendEffects,
{
    /// Admission and terminal responses addressed to submitters.
    pub responses: ResponseSends,
    /// Assignments addressed to the selected stable worker proxies.
    pub assignments: Vec<behavior::ChildDelivery<C::Protocol, behavior::ChildHead>>,
    /// Observation, replacement, failure-report, timer, and shutdown effects
    /// owned by the pool's fixed-fleet supervision law.
    pub supervision: SupervisorSends<A, C, Stable>,
}

impl<A, C, Stable, ResponseSends> SendEffects for PoolSends<A, C, Stable, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    ResponseSends: SendEffects,
{
    fn empty() -> Self {
        Self {
            responses: ResponseSends::empty(),
            assignments: Vec::new(),
            supervision: SupervisorSends::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.responses.append(other.responses);
        self.assignments.append(&mut other.assignments);
        self.supervision.append(other.supervision);
    }
}

impl<Event, A, C, Stable, ResponseSends> behavior::SendsFor<SupervisionEvent<Event>>
    for PoolSends<A, C, Stable, ResponseSends>
where
    Event: behavior::UserEvent<Addr = A>,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    ResponseSends: behavior::SendsFor<Event>,
    SupervisorSends<A, C, Stable>: behavior::SendsFor<SupervisionEvent<Event>>,
{
}

impl<I, RootEvent, Path, A, C, Stable, ResponseSends> behavior::InterpretSends<I, RootEvent, Path>
    for PoolSends<A, C, Stable, ResponseSends>
where
    I: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    ResponseSends: SendEffects + behavior::InterpretSends<I, RootEvent, behavior::Inside<Path>>,
    Vec<behavior::ChildDelivery<C::Protocol, behavior::ChildHead>>:
        behavior::InterpretSends<I, RootEvent, behavior::Inside<Path>>,
    SupervisorSends<A, C, Stable>: behavior::InterpretSends<I, RootEvent, Path>,
    PoolSends<A, C, Stable, ResponseSends>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.responses, interpreter).await?;
            behavior::InterpretSends::interpret(self.assignments, interpreter).await?;
            behavior::InterpretSends::interpret(self.supervision, interpreter).await
        }
    }
}

impl<A, C, Stable, ResponseSends>
    SendInput<behavior::ChildDelivery<C::Protocol, behavior::ChildHead>, Own>
    for PoolSends<A, C, Stable, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    ResponseSends: SendEffects,
{
    fn emit(&mut self, input: behavior::ChildDelivery<C::Protocol, behavior::ChildHead>) {
        self.assignments.push(input);
    }
}

/// Complete action type shared by FIFO and keyed worker pools.
type PoolActions<A, C, Route, Stable> =
    Actions<A, Never, PoolSends<A, C, Stable, <Route as DeliveryRoute>::Sends>, Births<Stable>>;

enum PoolOwnershipEvent<A: Address> {
    WorkerCreationResolved(WorkerCreationResolved<A::Nonce>),
    ChildStopped(crate::ChildStopped<A>),
    CreationResolved(crate::CreationResolved<A>),
    TimerElapsed(crate::TimerElapsed),
    ShutdownRequested,
    ChildShutdownRejected(crate::ChildShutdownRejected<A::Nonce>),
}

type PoolStable<C, L, R> = RelayChildReports<<L as BehaviorLayer<C>>::Output, C, PoolCompletion<R>>;

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
/// ```compile_fail,E0271
/// use behavior_actors::{Actions, Behavior, BehaviorActed, MailAddr, Never, NoBirths,
///     PoolResponse, Protocol, Proxy, Recipient, User, WorkerPool};
///
/// struct Reply;
/// struct WrongWorker;
/// impl Protocol for Reply {
///     type Addr = MailAddr;
///     type Msg = PoolResponse<String, (), MailAddr>;
/// }
/// impl Protocol for WrongWorker {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// impl Behavior for WrongWorker {
///     type Protocol = Self;
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior_actors::ActiveTurn, _: Self::Event)
///         -> BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
///
/// type WrongPool = WorkerPool<
///     MailAddr,
///     String,
///     (),
///     WrongWorker,
///     Recipient<Reply>,
///     fn(WrongWorker) -> Proxy<WrongWorker>,
/// >;
/// fn require_behavior<B: Behavior>() {}
/// // The only unsatisfied pool bound is
/// // `WrongWorker::Protocol::Msg = PoolAssignment<String>`.
/// require_behavior::<WrongPool>();
/// ```
struct PoolState<A: Address, J, R, C, Route, L>
where
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    supervisor: FixedFleetOwnership<A, C, PoolStable<C, L, R>>,
    layer: L,
    slots: Vec<Slot<A, J, Route>>,
    backlog: VecDeque<QueuedJob<A, J, Route>>,
    backlog_capacity: usize,
    next_assignment: u64,
    interruption: InterruptionPolicy,
    response_contract: core::marker::PhantomData<fn() -> R>,
}

impl<A, J, R, C, Route, L> PoolState<A, J, R, C, Route, L>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    /// Construct a pool after proving that every configured child route is
    /// unique.
    ///
    /// # Errors
    ///
    /// Returns [`PoolConfigError::NoWorkers`] for an empty topology or
    /// [`PoolConfigError::DuplicateWorker`] for the first repeated
    /// creator-local nonce. No behavior or creation request is produced.
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        layer: L,
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
                state: SlotState::Vacant,
                expected_returns: Vec::new(),
            });
        }
        Ok(Self {
            supervisor: FixedFleetOwnership::new(
                ChildTopology::new(nonces, build),
                RestartConfiguration {
                    strategy: Strategy::OneForOne,
                    policy: configuration.restart_policy,
                    maximum: configuration.maximum_restarts,
                    window: configuration.restart_window,
                    timing: configuration.restart_timing,
                },
            )
            .map_err(|error| match error {
                FleetError::UnknownChild(nonce) | FleetError::DuplicateChild(nonce) => {
                    PoolConfigError::DuplicateWorker(nonce)
                }
            })?,
            layer,
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
            .position(|slot| slot.nonce == worker)
            .map(|position| self.phase_at(position))
    }

    fn phase_at(&self, position: usize) -> WorkerPhase {
        let slot = &self.slots[position];
        if self.supervisor.is_shutting_down() {
            return match &slot.state {
                SlotState::Retired { reason } => WorkerPhase::Retired { reason: *reason },
                _ => WorkerPhase::Stopping,
            };
        }
        match &slot.state {
            SlotState::Assigned { assignment, job } => WorkerPhase::Assigned {
                assignment: *assignment,
                job: job.id,
            },
            SlotState::CommandReturned { .. } => WorkerPhase::Installing,
            SlotState::Retired { reason } => WorkerPhase::Retired { reason: *reason },
            SlotState::Vacant => {
                if self.supervisor.worker_routable(slot.nonce).unwrap_or(false) {
                    WorkerPhase::Idle
                } else {
                    WorkerPhase::Installing
                }
            }
        }
    }

    fn is_dispatchable(&self, position: usize) -> bool {
        let slot = &self.slots[position];
        matches!(slot.state, SlotState::Vacant)
            && self.supervisor.worker_routable(slot.nonce).unwrap_or(false)
    }

    fn slot_position(&self, worker: A::Nonce) -> Result<usize, PoolError<A>> {
        self.slots
            .iter()
            .position(|slot| slot.nonce == worker)
            .ok_or(PoolError::UnknownWorker(worker))
    }

    fn initialize_actions(
        &mut self,
    ) -> Result<PoolActions<A, C, Route, PoolStable<C, L, R>>, PoolFailure<A, J, R, Never>> {
        let Self {
            supervisor, layer, ..
        } = self;
        let stable = |worker| RelayChildReports::new(layer.layer(worker));
        let actions = supervisor
            .initialize(&stable)
            .map_err(map_pool_ownership_error)?;
        Ok(Actions::new(
            PoolSends {
                responses: Route::Sends::empty(),
                assignments: Vec::new(),
                supervision: actions.sends,
            },
            actions.creates,
            actions.become_,
        ))
    }
}

impl<A, J, R, C, Route, L> PoolState<A, J, R, C, Route, L>
where
    A: Address,
    A::Nonce: From<u64>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
{
    fn supervisor_transition(
        &mut self,
        event: PoolOwnershipEvent<A>,
    ) -> Result<PoolActions<A, C, Route, PoolStable<C, L, R>>, PoolError<A>> {
        let fold = match event {
            PoolOwnershipEvent::WorkerCreationResolved(event) => {
                self.supervisor.worker_creation_resolved(event)
            }
            PoolOwnershipEvent::ChildStopped(event) => self.supervisor.child_stopped(event),
            PoolOwnershipEvent::CreationResolved(event) => self.supervisor.creation_resolved(event),
            PoolOwnershipEvent::TimerElapsed(event) => self.supervisor.timer_elapsed(event),
            PoolOwnershipEvent::ShutdownRequested => Ok(self.supervisor.shutdown()),
            PoolOwnershipEvent::ChildShutdownRejected(event) => {
                self.supervisor.child_shutdown_rejected(event)
            }
        }
        .map_err(map_pool_ownership_error)?;
        Ok(self.wrap_ownership(fold))
    }

    fn wrap_ownership(
        &self,
        fold: OwnershipFold<A, C, PoolStable<C, L, R>>,
    ) -> PoolActions<A, C, Route, PoolStable<C, L, R>> {
        Actions::new(
            PoolSends {
                responses: Route::Sends::empty(),
                assignments: Vec::new(),
                supervision: fold.actions.sends,
            },
            fold.actions.creates,
            fold.actions.become_,
        )
    }

    fn supervisor_worker_stopped(
        &mut self,
        event: WorkerStopped<A>,
    ) -> Result<OwnershipFold<A, C, PoolStable<C, L, R>>, PoolError<A>> {
        self.supervisor
            .worker_stopped(event)
            .map_err(map_pool_ownership_error)
    }

    fn submit(
        &mut self,
        job: JobId,
        payload: J,
        reply_to: Route,
        actions: &mut PoolActions<A, C, Route, PoolStable<C, L, R>>,
    ) {
        let can_dispatch = self
            .slots
            .iter()
            .enumerate()
            .any(|(position, _)| self.is_dispatchable(position));
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
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
            .responses
            .append(reply_to.deliver(PoolResponse::Accepted { job }));
    }

    fn submit_to(
        &mut self,
        target: A::Nonce,
        job: JobId,
        payload: J,
        reply_to: Route,
        actions: &mut PoolActions<A, C, Route, PoolStable<C, L, R>>,
    ) -> Admission {
        let Some(position) = self.slots.iter().position(|slot| slot.nonce == target) else {
            actions
                .sends
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::AffinityUnavailable,
                }));
            return Admission::Rejected;
        };
        if matches!(self.slots[position].state, SlotState::Retired { .. }) {
            actions
                .sends
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::AffinityUnavailable,
                }));
            return Admission::Rejected;
        }
        let can_dispatch = self.is_dispatchable(position);
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
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
            .responses
            .append(reply_to.deliver(PoolResponse::Accepted { job }));
        Admission::Accepted
    }

    fn complete(
        &mut self,
        worker: A::Nonce,
        incarnation: A::Nonce,
        assignment: AssignmentId,
        result: R,
        actions: &mut PoolActions<A, C, Route, PoolStable<C, L, R>>,
    ) -> Result<(), CompletionRejection<A::Nonce, R>> {
        let Some(position) = self.slots.iter().position(|slot| slot.nonce == worker) else {
            return Err(CompletionRejection::UnknownWorker {
                worker,
                assignment,
                result,
            });
        };
        let expected_incarnation = match self.supervisor.worker_incarnation(worker) {
            Ok(Some(expected)) => expected,
            Ok(None) => {
                return Err(CompletionRejection::WorkerUnavailable {
                    worker,
                    assignment,
                    phase: self.phase_at(position),
                    result,
                });
            }
            Err(_) => {
                return Err(CompletionRejection::UnknownWorker {
                    worker,
                    assignment,
                    result,
                });
            }
        };
        if expected_incarnation != incarnation {
            return Err(CompletionRejection::StaleIncarnation {
                worker,
                expected: expected_incarnation,
                observed: incarnation,
                assignment,
                result,
            });
        }
        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Vacant);
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
                self.slots[position].state = state;
                return Err(CompletionRejection::WorkerUnavailable {
                    worker,
                    assignment,
                    phase: self.phase_at(position),
                    result,
                });
            }
        };
        actions
            .sends
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
                SlotState::CommandReturned { .. } | SlotState::Vacant => None,
                SlotState::Retired { .. } => return Ok(()),
            };
            let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Vacant);
            match (previous, dispatch_payload) {
                (
                    SlotState::Assigned {
                        assignment,
                        mut job,
                    },
                    Some(dispatch_payload),
                ) => {
                    self.slots[position]
                        .expected_returns
                        .push(ExpectedAssignmentReturn {
                            assignment,
                            job: job.id,
                        });
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
                (SlotState::Vacant, None) => {}
                (state, _) => self.slots[position].state = state,
            }
            return Ok(());
        }

        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Vacant);
        match previous {
            SlotState::Assigned { assignment, job } => {
                self.slots[position]
                    .expected_returns
                    .push(ExpectedAssignmentReturn {
                        assignment,
                        job: job.id,
                    });
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
            SlotState::Vacant => {}
            state => self.slots[position].state = state,
        }
        Ok(())
    }

    fn validate_worker_stopped(&self, stopped: &WorkerStopped<A>) -> Result<(), PoolError<A>> {
        let position = self.slot_position(stopped.proxy)?;
        match self.slots[position].state {
            SlotState::Vacant | SlotState::Assigned { .. } | SlotState::CommandReturned { .. } => {
                Ok(())
            }
            _ => Err(PoolError::WorkerStoppedWhileUnavailable {
                observed: stopped.clone(),
                phase: self.phase_at(position),
            }),
        }
    }

    fn command_unavailable(
        &mut self,
        returned: crate::ProxyUnavailable<A, PoolAssignment<J>>,
    ) -> Result<(), crate::ProxyUnavailable<A, PoolAssignment<J>>> {
        let worker = returned.proxy;
        let Some(position) = self.slots.iter().position(|slot| slot.nonce == worker) else {
            return Err(returned);
        };
        let crate::ProxyUnavailable {
            proxy,
            from,
            phase,
            command:
                PoolAssignment {
                    assignment,
                    job,
                    payload,
                },
        } = returned;
        if let Some(expected) = self.slots[position]
            .expected_returns
            .iter()
            .position(|expected| expected.assignment == assignment && expected.job == job)
        {
            self.slots[position].expected_returns.remove(expected);
            return Ok(());
        }
        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Vacant);
        match previous {
            SlotState::Assigned {
                assignment: expected,
                job: accepted,
            } if expected == assignment && accepted.id == job => {
                self.slots[position].state = SlotState::CommandReturned {
                    job: accepted,
                    dispatch_payload: payload,
                };
                Ok(())
            }
            state => {
                self.slots[position].state = state;
                Err(crate::ProxyUnavailable {
                    proxy,
                    from,
                    phase,
                    command: PoolAssignment {
                        assignment,
                        job,
                        payload,
                    },
                })
            }
        }
    }

    fn fail_backlog_if_irrecoverable(
        &mut self,
        actions: &mut PoolActions<A, C, Route, PoolStable<C, L, R>>,
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
            actions.sends.responses.append(
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
        actions: &mut PoolActions<A, C, Route, PoolStable<C, L, R>>,
    ) {
        let mut retained = VecDeque::with_capacity(self.backlog.len());
        while let Some(queued) = self.backlog.pop_front() {
            if queued.accepted.target == Some(worker) {
                let job = queued.accepted;
                actions.sends.responses.append(
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
            let previous = core::mem::replace(&mut slot.state, SlotState::Vacant);
            match previous {
                SlotState::Assigned { assignment, job } => {
                    slot.expected_returns.push(ExpectedAssignmentReturn {
                        assignment,
                        job: job.id,
                    });
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
                SlotState::Vacant => {}
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

    fn retain_worker_creation_result(
        &mut self,
        position: usize,
        result: Result<(), CreationRejection>,
    ) {
        if let Err(rejection) = result {
            self.slots[position].state = SlotState::Retired {
                reason: WorkerRetirement::CreationRejected(rejection),
            };
        }
    }

    fn dispatch(
        &mut self,
        actions: &mut PoolActions<A, C, Route, PoolStable<C, L, R>>,
    ) -> Result<(), PoolError<A>> {
        let mut selected_jobs = Vec::new();
        let mut plan = Vec::new();
        for (slot_position, slot) in self.slots.iter().enumerate() {
            if !self.is_dispatchable(slot_position) {
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
            let route = ChildRoute::<PoolStable<C, L, R>, behavior::ChildHead>::new(nonce);
            let job_id = job.id;
            self.slots[slot_position].state = SlotState::Assigned { assignment, job };
            actions
                .sends
                .send::<behavior::ChildDelivery<C::Protocol, behavior::ChildHead>, Own>(
                    behavior::ChildDelivery::at(
                        route,
                        PoolAssignment {
                            assignment,
                            job: job_id,
                            payload,
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
pub struct WorkerPool<A: Address, J, R, C, Route, L>
where
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    core: PoolState<A, J, R, C, Route, L>,
}

impl<A, J, R, C, Route, L> crate::BehaviorBase for WorkerPool<A, J, R, C, Route, L>
where
    A: Address,
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, J, R, C, Route, L> WorkerPool<A, J, R, C, Route, L>
where
    A: Address,
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    /// Construct a pool whose completion destination implements this exact
    /// pool protocol.
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        layer: L,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        PoolState::new(topology, configuration, layer).map(|core| Self { core })
    }

    #[must_use]
    pub fn backlog_len(&self) -> usize {
        self.core.backlog_len()
    }

    #[must_use]
    pub fn worker_phase(&self, worker: A::Nonce) -> Option<WorkerPhase> {
        self.core.worker_phase(worker)
    }

    fn is_shutting_down(&self) -> bool {
        self.core.supervisor.is_shutting_down()
    }

    fn submit_to(
        &mut self,
        worker: A::Nonce,
        job: JobId,
        payload: J,
        reply_to: Route,
    ) -> Result<
        (Admission, PoolActions<A, C, Route, PoolStable<C, L, R>>),
        PoolFailure<A, J, R, Never>,
    >
    where
        J: Clone,
    {
        let mut actions: PoolActions<A, C, Route, PoolStable<C, L, R>> = Actions::cont();
        if self.is_shutting_down() {
            actions
                .sends
                .responses
                .append(reply_to.deliver(PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::ShuttingDown,
                }));
            return Ok((Admission::Rejected, actions));
        }
        let admission = self
            .core
            .submit_to(worker, job, payload, reply_to, &mut actions);
        self.core.dispatch(&mut actions)?;
        Ok((admission, actions))
    }
}

impl<A, J, R, C, Route, L> Behavior for WorkerPool<A, J, R, C, Route, L>
where
    A: Address,
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    Route::Sends: behavior::SendsFor<WorkerPoolEvent<A, J, R, Route>>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    type Protocol = crate::WorkerPoolProtocol<A, J, R, Route>;
    type Event = SupervisionEvent<WorkerPoolEvent<A, J, R, Route>>;
    type Sends = PoolSends<A, C, PoolStable<C, L, R>, Route::Sends>;
    type Ph = Never;
    type Error = PoolFailure<A, J, R, Never>;
    type Birth = Births<PoolStable<C, L, R>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.core.initialize_actions()
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Behavior(WorkerPoolEvent::Command(User {
                message:
                    PoolMessage::Submit {
                        job,
                        payload,
                        reply_to,
                    },
                ..
            })) => {
                if self.core.supervisor.is_shutting_down() {
                    let mut actions: PoolActions<A, C, Route, PoolStable<C, L, R>> =
                        Actions::cont();
                    actions
                        .sends
                        .responses
                        .append(reply_to.deliver(PoolResponse::Rejected {
                            job,
                            payload,
                            reason: PoolRejection::ShuttingDown,
                        }));
                    return Ok(actions);
                }
                let mut actions = Actions::cont();
                self.core.submit(job, payload, reply_to, &mut actions);
                self.core.dispatch(&mut actions)?;
                Ok(actions)
            }
            SupervisionEvent::Behavior(WorkerPoolEvent::Completion(behavior::ChildReport {
                child: worker,
                report:
                    behavior::ChildReport {
                        child: incarnation,
                        report: PoolCompletion { assignment, result },
                    },
            })) => {
                if self.core.supervisor.is_shutting_down() {
                    return Err(PoolFailure::Completion(CompletionRejection::ShuttingDown {
                        worker,
                        assignment,
                        result,
                    }));
                }
                let mut actions = Actions::cont();
                self.core
                    .complete(worker, incarnation, assignment, result, &mut actions)
                    .map_err(PoolFailure::Completion)?;
                self.core.dispatch(&mut actions)?;
                Ok(actions)
            }
            SupervisionEvent::WorkerStopped(stopped) => {
                if self.core.supervisor.is_shutting_down() {
                    let fold = self.core.supervisor_worker_stopped(stopped)?;
                    return Ok(self.core.wrap_ownership(fold));
                }
                self.core
                    .supervisor
                    .validate_worker_stopped(&stopped)
                    .map_err(map_pool_ownership_error)?;
                self.core.validate_worker_stopped(&stopped)?;
                let proxy = stopped.proxy;
                let mut responses = Route::Sends::empty();
                self.core.worker_stopped(&stopped, &mut responses)?;
                let ownership = self.core.supervisor_worker_stopped(stopped)?;
                let replacement_accepted = self
                    .core
                    .supervisor
                    .is_restartable(proxy)
                    .map_err(OwnershipError::from)
                    .map_err(map_pool_ownership_error)?;
                let mut actions = self.core.wrap_ownership(ownership);
                actions.sends.responses.append(responses);
                if !replacement_accepted {
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
            SupervisionEvent::WorkerCreationResolved(resolved) => {
                if self.core.supervisor.is_shutting_down() {
                    return Ok(self.core.supervisor_transition(
                        PoolOwnershipEvent::WorkerCreationResolved(resolved),
                    )?);
                }
                self.core
                    .supervisor
                    .validate_worker_creation_resolved(&resolved)
                    .map_err(map_pool_ownership_error)?;
                let proxy = resolved.proxy;
                let position = self.core.slot_position(proxy)?;
                let result = resolved.result;
                let mut actions = self
                    .core
                    .supervisor_transition(PoolOwnershipEvent::WorkerCreationResolved(resolved))?;
                self.core.retain_worker_creation_result(position, result);
                if let Some(WorkerPhase::Retired { reason }) = self.core.worker_phase(proxy) {
                    self.core
                        .fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.core.dispatch(&mut actions)?;
                self.core.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::ChildStopped(stopped) => Ok(self
                .core
                .supervisor_transition(PoolOwnershipEvent::ChildStopped(stopped))?),
            SupervisionEvent::CreationResolved(resolved) => Ok(self
                .core
                .supervisor_transition(PoolOwnershipEvent::CreationResolved(resolved))?),
            SupervisionEvent::TimerElapsed(elapsed) => Ok(self
                .core
                .supervisor_transition(PoolOwnershipEvent::TimerElapsed(elapsed))?),
            SupervisionEvent::ShutdownRequested(_) => {
                let responses = self.core.interrupt_all_for_shutdown();
                let mut actions = self
                    .core
                    .supervisor_transition(PoolOwnershipEvent::ShutdownRequested)?;
                actions.sends.responses.append(responses);
                Ok(actions)
            }
            SupervisionEvent::Behavior(WorkerPoolEvent::AssignmentUnavailable(message)) => {
                self.core
                    .command_unavailable(message)
                    .map_err(PoolFailure::UnexpectedAssignmentUnavailable)?;
                let mut actions = Actions::cont();
                if !self.core.supervisor.is_shutting_down() {
                    self.core.dispatch(&mut actions)?;
                    self.core.fail_backlog_if_irrecoverable(&mut actions);
                }
                Ok(actions)
            }
            SupervisionEvent::ChildShutdownRejected(rejected) => Ok(self
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
    use crate::{MailAddr, NoBirths, Recipient};

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
        type Msg = PoolAssignment<u8>;
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
    fn one_dispatch_batch_preserves_fifo_jobs_across_index_removal() {
        let mut pool: WorkerPool<MailAddr, u8, (), TestWorker, Recipient<TestReply>, _> =
            WorkerPool::new(
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
                    crate::RestartTiming::Immediate,
                ),
                |worker: TestWorker| crate::Proxy::new(worker),
            )
            .unwrap();
        let initialized = behavior::initialize(&mut pool).unwrap();
        assert!(initialized.sends.responses.is_empty());
        assert!(initialized.sends.assignments.is_empty());
        assert_eq!(initialized.sends.supervision.child_observations.len(), 2);
        assert_eq!(initialized.sends.supervision.creation_observations.len(), 2);
        assert!(initialized.sends.supervision.replacement_inputs.is_empty());
        assert!(initialized.sends.supervision.failure_reports.is_empty());
        assert!(initialized.sends.supervision.schedules.is_empty());
        assert!(initialized.sends.supervision.shutdowns.is_empty());
        assert_eq!(initialized.creates.len(), 2);
        assert!(matches!(initialized.become_, behavior::Step::Continue));

        for proxy in 0..2 {
            pool.core
                .supervisor
                .worker_creation_resolved(WorkerCreationResolved::new(
                    proxy,
                    proxy + 10,
                    crate::CreationKind::Birth,
                    Ok(()),
                ))
                .unwrap();
        }

        for job in 1..=3 {
            let mut ignored = Actions::cont();
            pool.core.submit(
                JobId(job),
                u8::try_from(job).unwrap(),
                Recipient::<TestReply>::global(MailAddr(91)),
                &mut ignored,
            );
        }

        let mut actions = Actions::cont();
        pool.core.dispatch(&mut actions).unwrap();

        let assignments = &actions.sends.assignments;
        assert_eq!(assignments.len(), 2);
        for (index, expected_job) in [JobId(1), JobId(2)].into_iter().enumerate() {
            assert_eq!(assignments[index].nonce, u64::try_from(index).unwrap());
            let assignment = &assignments[index].message;
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
/// ```compile_fail,E0277
/// use behavior_actors::{Actions, Behavior, BehaviorActed, KeyedWorkerPool, MailAddr,
///     Never, NoBirths, PoolAssignment, PoolResponse, Protocol, Proxy, Recipient, User};
/// struct NonKey(f64);
/// struct Reply;
/// struct Worker;
/// impl Protocol for Reply {
///     type Addr = MailAddr;
///     type Msg = PoolResponse<u8, (), MailAddr>;
/// }
/// impl Protocol for Worker {
///     type Addr = MailAddr;
///     type Msg = PoolAssignment<u8>;
/// }
/// impl Behavior for Worker {
///     type Protocol = Self;
///     type Event = User<MailAddr, PoolAssignment<u8>>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior_actors::ActiveTurn, _: Self::Event)
///         -> BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// type WrongKeyPool = KeyedWorkerPool<
///     MailAddr,
///     NonKey,
///     u8,
///     (),
///     Worker,
///     Recipient<Reply>,
///     fn(&NonKey) -> u64,
///     fn(Worker) -> Proxy<Worker>,
/// >;
/// fn require_behavior<B: Behavior>() {}
/// // The only unsatisfied keyed-pool bound is `NonKey: Eq`.
/// require_behavior::<WrongKeyPool>();
/// ```
pub struct KeyedWorkerPool<A: Address, K, J, R, C, Route, S, L>
where
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    S: AffinitySelector<K, A::Nonce>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    pool: WorkerPool<A, J, R, C, Route, L>,
    bindings: Vec<(K, A::Nonce)>,
    selector: S,
}

impl<A, K, J, R, C, Route, S, L> crate::BehaviorBase for KeyedWorkerPool<A, K, J, R, C, Route, S, L>
where
    A: Address,
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    S: AffinitySelector<K, A::Nonce>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, K, J, R, C, Route, S, L> KeyedWorkerPool<A, K, J, R, C, Route, S, L>
where
    A: Address,
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    S: AffinitySelector<K, A::Nonce>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
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
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        selector: S,
        layer: L,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Ok(Self {
            pool: WorkerPool::new(topology, configuration, layer)?,
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
        match self.pool.worker_phase(worker) {
            None => return Err(RebalanceRejection::UnknownWorker { key, worker }),
            Some(WorkerPhase::Retired { reason }) => {
                return Err(RebalanceRejection::RetiredWorker {
                    key,
                    worker,
                    reason,
                });
            }
            Some(
                WorkerPhase::Installing
                | WorkerPhase::Idle
                | WorkerPhase::Stopping
                | WorkerPhase::Assigned { .. },
            ) => {}
        }
        if let Some((_, bound)) = self.bindings.iter_mut().find(|(bound, _)| *bound == key) {
            *bound = worker;
        } else {
            self.bindings.push((key, worker));
        }
        Ok(())
    }
}

impl<A, K, J, R, C, Route, S, L> Behavior for KeyedWorkerPool<A, K, J, R, C, Route, S, L>
where
    A: Address,
    A::Nonce: From<u64>,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>> + Clone,
    Route::Sends: behavior::SendsFor<KeyedWorkerPoolEvent<A, K, J, R, Route>>,
    Route::Sends: behavior::SendsFor<WorkerPoolEvent<A, J, R, Route>>,
    K: Eq,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<J>>,
    S: AffinitySelector<K, A::Nonce>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <PoolStable<C, L, R> as Behavior>::Event: ChildInputIngress<C, crate::ReplacementRequested<C>>,
{
    type Protocol = crate::KeyedWorkerPoolProtocol<A, K, J, R, Route>;
    type Event = SupervisionEvent<KeyedWorkerPoolEvent<A, K, J, R, Route>>;
    type Sends = PoolSends<A, C, PoolStable<C, L, R>, Route::Sends>;
    type Ph = Never;
    type Error = PoolFailure<A, J, R, K>;
    type Birth = Births<PoolStable<C, L, R>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        behavior::initialize(&mut self.pool).map_err(widen_pool_failure)
    }

    fn transition(
        &mut self,
        turn: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Behavior(KeyedWorkerPoolEvent::Command(User {
                message:
                    KeyedPoolMessage::Submit {
                        key,
                        job,
                        payload,
                        reply_to,
                    },
                ..
            })) => {
                let existing = self.affinity(&key);
                let target = existing.unwrap_or_else(|| self.selector.select(&key));
                let (admission, actions) = self
                    .pool
                    .submit_to(target, job, payload, reply_to)
                    .map_err(widen_pool_failure)?;
                match (admission, existing) {
                    (Admission::Accepted, None) => self.bindings.push((key, target)),
                    (Admission::Accepted | Admission::Rejected, Some(_))
                    | (Admission::Rejected, None) => {}
                }
                Ok(actions)
            }
            SupervisionEvent::Behavior(KeyedWorkerPoolEvent::Completion(completion)) => self
                .pool
                .transition(
                    turn,
                    SupervisionEvent::Behavior(WorkerPoolEvent::Completion(completion)),
                )
                .map_err(widen_pool_failure),
            SupervisionEvent::Behavior(KeyedWorkerPoolEvent::Command(User {
                message: KeyedPoolMessage::Rebalance { key, worker },
                ..
            })) => {
                if self.pool.is_shutting_down() {
                    return Err(PoolFailure::Rebalance(RebalanceRejection::ShuttingDown {
                        key,
                        worker,
                    }));
                }
                self.rebalance(key, worker)
                    .map_err(PoolFailure::Rebalance)?;
                Ok(Actions::cont())
            }
            SupervisionEvent::WorkerStopped(stopped) => self
                .pool
                .transition(turn, SupervisionEvent::WorkerStopped(stopped))
                .map_err(widen_pool_failure),
            SupervisionEvent::WorkerCreationResolved(resolved) => self
                .pool
                .transition(turn, SupervisionEvent::WorkerCreationResolved(resolved))
                .map_err(widen_pool_failure),
            SupervisionEvent::ChildStopped(stopped) => self
                .pool
                .transition(turn, SupervisionEvent::ChildStopped(stopped))
                .map_err(widen_pool_failure),
            SupervisionEvent::CreationResolved(resolved) => self
                .pool
                .transition(turn, SupervisionEvent::CreationResolved(resolved))
                .map_err(widen_pool_failure),
            SupervisionEvent::TimerElapsed(elapsed) => self
                .pool
                .transition(turn, SupervisionEvent::TimerElapsed(elapsed))
                .map_err(widen_pool_failure),
            SupervisionEvent::ShutdownRequested(shutdown) => self
                .pool
                .transition(turn, SupervisionEvent::ShutdownRequested(shutdown))
                .map_err(widen_pool_failure),
            SupervisionEvent::Behavior(KeyedWorkerPoolEvent::AssignmentUnavailable(message)) => {
                self.pool
                    .transition(
                        turn,
                        SupervisionEvent::Behavior(WorkerPoolEvent::AssignmentUnavailable(message)),
                    )
                    .map_err(widen_pool_failure)
            }
            SupervisionEvent::ChildShutdownRejected(rejected) => self
                .pool
                .transition(turn, SupervisionEvent::ChildShutdownRejected(rejected))
                .map_err(widen_pool_failure),
        }
    }
}

/// Owner-defined construction of pools whose commands carry logical customer routes.
///
/// The customer protocol is the only type Rust cannot infer from a pool's
/// configuration: it appears in each later `reply_to`, not in the constructor
/// values. Calling `Customer::worker_pool(...)` or
/// `Customer::keyed_worker_pool(...)` names that protocol once and fixes the
/// truthful route law to [`behavior::Recipient<Customer>`]. Worker, job,
/// result, selector, and behavior-layer types remain inferred. Exact or mixed
/// customer routes continue to use the general pool constructors directly.
///
/// This trait constructs the existing [`WorkerPool`] and [`KeyedWorkerPool`]
/// folds. It stores no witness and introduces no alternate pool behavior.
pub trait PoolCustomer: Protocol + Sized {
    /// Construct a FIFO pool using this protocol as its logical reply protocol.
    fn worker_pool<J, R, C, L>(
        topology: ChildTopology<<<Self as Protocol>::Addr as Address>::Nonce, C>,
        configuration: PoolConfiguration,
        layer: L,
    ) -> Result<
        WorkerPool<<Self as Protocol>::Addr, J, R, C, behavior::Recipient<Self>, L>,
        PoolConfigError<<<Self as Protocol>::Addr as Address>::Nonce>,
    >
    where
        Self: Protocol<Msg = PoolResponse<J, R, <Self as Protocol>::Addr>>,
        <<Self as Protocol>::Addr as Address>::Nonce: From<u64>,
        C: Behavior<Ph = Never>,
        C::Protocol: crate::Protocol<Addr = <Self as Protocol>::Addr, Msg = PoolAssignment<J>>,
        L: BehaviorLayer<C>,
        L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
        <PoolStable<C, L, R> as Behavior>::Event:
            ChildInputIngress<C, crate::ReplacementRequested<C>>,
    {
        WorkerPool::new(topology, configuration, layer)
    }

    /// Construct a keyed pool using this protocol as its logical reply protocol.
    fn keyed_worker_pool<K, J, R, C, S, L>(
        topology: ChildTopology<<<Self as Protocol>::Addr as Address>::Nonce, C>,
        configuration: PoolConfiguration,
        selector: S,
        layer: L,
    ) -> Result<
        KeyedWorkerPool<<Self as Protocol>::Addr, K, J, R, C, behavior::Recipient<Self>, S, L>,
        PoolConfigError<<<Self as Protocol>::Addr as Address>::Nonce>,
    >
    where
        Self: Protocol<Msg = PoolResponse<J, R, <Self as Protocol>::Addr>>,
        <<Self as Protocol>::Addr as Address>::Nonce: From<u64>,
        K: Eq,
        C: Behavior<Ph = Never>,
        C::Protocol: crate::Protocol<Addr = <Self as Protocol>::Addr, Msg = PoolAssignment<J>>,
        S: AffinitySelector<K, <<Self as Protocol>::Addr as Address>::Nonce>,
        L: BehaviorLayer<C>,
        L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
        <PoolStable<C, L, R> as Behavior>::Event:
            ChildInputIngress<C, crate::ReplacementRequested<C>>,
    {
        KeyedWorkerPool::new(topology, configuration, selector, layer)
    }
}

impl<P: Protocol> PoolCustomer for P {}
