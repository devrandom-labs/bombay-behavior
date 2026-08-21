//! A bounded, FIFO worker pool expressed entirely as a pure behavior.
//!
//! Pool scheduling is a derived Bombay construction, not an actor-model
//! primitive. Runtime installation, delivery, and observation remain effects
//! for an interpreter; this module owns only their typed protocol and fold.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::supervision::{FixedFleetOwnership, OwnershipError, StableProxyChildRole};
use crate::{
    Actions, Address, Behavior, Births, ChildTopology, Crash, CreationRejection, Delivery, Exit,
    FleetError, Never, Own, Protocol, Proxy, ProxyCommand, ProxyParentIngress, ProxyWithParent,
    Recipient, RestartConfiguration, RestartPolicy, SendEffects, SendInput, SendLayer, Strategy,
    SupervisionEvent, SupervisorSends, User, WorkerCreationResolved, WorkerStopped,
};
use behavior::{ChildRoute, Here};

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
pub enum PoolMessage<A, D, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    Submit {
        job: JobId,
        payload: J,
        reply_to: Recipient<D>,
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
pub enum KeyedPoolMessage<A, D, K, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    Submit {
        key: K,
        job: JobId,
        payload: J,
        reply_to: Recipient<D>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PoolError<N> {
    #[error("the event names an unknown worker")]
    UnknownWorker(N),
    #[error("the worker nonce is already configured")]
    DuplicateWorker(N),
    #[error("a pool-owned sequence is exhausted")]
    SequenceExhausted,
    #[error("the worker factory rejected a configured index")]
    WorkerFactoryIndex { index: usize },
    #[error("a completion targeted a worker that cannot complete work")]
    CompletionForUnavailableWorker { worker: N, phase: WorkerPhase },
    #[error("the completion carries a stale assignment identifier")]
    StaleCompletion {
        worker: N,
        expected: AssignmentId,
        received: AssignmentId,
    },
    #[error("a stop observation targeted an unavailable worker")]
    WorkerStoppedWhileUnavailable { worker: N, phase: WorkerPhase },
    #[error("a creation result targeted a worker that is not installing")]
    CreationResolvedWhileUnavailable { worker: N, phase: WorkerPhase },
    #[error("an affinity rebalance targeted a retired worker")]
    RebalanceToRetiredWorker { worker: N, reason: WorkerRetirement },
    #[error("the pool is draining its owned worker proxies")]
    ShuttingDown,
    #[error("owned worker proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: N,
        reason: crate::ChildShutdownRejection,
    },
    #[error("worker-proxy creation provenance did not match the pending request")]
    ProxyCreationProvenanceMismatch {
        nonce: N,
        expected: crate::CreationKind<N>,
        observed: crate::CreationKind<N>,
    },
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        proxy: N,
        worker: N,
        expected: crate::CreationKind<N>,
        observed: crate::CreationKind<N>,
    },
}

struct AcceptedJob<A: Address, D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    id: JobId,
    payload: J,
    reply_to: Recipient<D>,
    interruption: Option<PoolInterruption<A>>,
    target: Option<A::Nonce>,
}

struct QueuedJob<A: Address, D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    accepted: AcceptedJob<A, D, J, R>,
    dispatch_payload: J,
}

enum SlotState<A: Address, D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    Installing,
    Idle,
    Stopping,
    Assigned {
        assignment: AssignmentId,
        job: AcceptedJob<A, D, J, R>,
    },
    Retired {
        reason: WorkerRetirement,
    },
}

struct Slot<A: Address, D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    nonce: A::Nonce,
    state: SlotState<A, D, J, R>,
}

impl<A, D, J, R> Slot<A, D, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    fn phase(&self) -> WorkerPhase {
        match &self.state {
            SlotState::Installing => WorkerPhase::Installing,
            SlotState::Idle => WorkerPhase::Idle,
            SlotState::Stopping => WorkerPhase::Stopping,
            SlotState::Assigned { assignment, job } => WorkerPhase::Assigned {
                assignment: *assignment,
                job: job.id,
            },
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

/// The pool's concrete event sum, including existing supervision facts.
pub type PoolEvent<A, D, J, R> = SupervisionEvent<User<A, PoolMessage<A, D, J, R>>>;

/// Complete event algebra of a shutdown-owning [`WorkerPool`].
pub type WorkerPoolEvent<A, D, J, R> = PoolEvent<A, D, J, R>;

/// Concrete event sum for a [`KeyedWorkerPool`].
pub type KeyedPoolEvent<A, D, K, J, R> = SupervisionEvent<User<A, KeyedPoolMessage<A, D, K, J, R>>>;

/// Named pool-owned delivery lanes.
pub struct PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    /// Admission and terminal responses addressed to submitters.
    pub responses: Vec<Delivery<D>>,
    /// Assignments addressed to the selected stable worker proxies.
    pub assignments: Vec<Delivery<Proxy<C>>>,
}

impl<A, D, J, R, C> SendEffects for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn empty() -> Self {
        Self {
            responses: Vec::new(),
            assignments: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.responses.append(&mut other.responses);
        self.assignments.append(&mut other.assignments);
    }
}

impl<Event, A, D, J, R, C> behavior::SendsFor<Event> for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
}

impl<I, RootEvent, Path, A, D, J, R, C> behavior::InterpretSends<I, RootEvent, Path>
    for PoolBehaviorSends<A, D, J, R, C>
where
    I: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Vec<Delivery<D>>: behavior::InterpretSends<I, RootEvent, Path>,
    Vec<Delivery<Proxy<C>>>: behavior::InterpretSends<I, RootEvent, Path>,
    PoolBehaviorSends<A, D, J, R, C>: Send,
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

impl<A, D, J, R, C> SendInput<Delivery<D>, Own> for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: Delivery<D>) {
        self.responses.push(input);
    }
}

impl<A, D, J, R, C> SendInput<Delivery<Proxy<C>>, Own> for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: Delivery<Proxy<C>>) {
        self.assignments.push(input);
    }
}

/// Pool effects keep responses and assignments in named, independently
/// appendable lanes within the supervised behavior send product.
pub type PoolSends<A, D, J, R, C, ParentPath = Here> =
    SendLayer<SupervisorSends<A, C, ParentPath>, PoolBehaviorSends<A, D, J, R, C>>;

/// Named effect product for a worker pool that owns orderly proxy shutdown.
pub type WorkerPoolSends<A, D, J, R, C, ParentPath = Here> = PoolSends<A, D, J, R, C, ParentPath>;

/// Complete action type returned by a shutdown-owning [`WorkerPool`].
pub type WorkerPoolActions<A, D, J, R, C, ParentPath = Here> = Actions<
    A,
    Never,
    WorkerPoolSends<A, D, J, R, C, ParentPath>,
    Births<ProxyWithParent<C, ParentPath>>,
>;

/// Complete action type returned by a [`WorkerPool`] transition.
pub type PoolActions<A, D, J, R, C, ParentPath = behavior::Here> =
    Actions<A, Never, PoolSends<A, D, J, R, C, ParentPath>, Births<ProxyWithParent<C, ParentPath>>>;

type PoolTransition<A, D, J, R, C, ParentPath = behavior::Here> =
    Result<PoolActions<A, D, J, R, C, ParentPath>, PoolError<<A as Address>::Nonce>>;

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
struct PoolCore<A: Address, D, J, R, C, P, ParentPath = behavior::Here>
where
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
{
    supervisor: FixedFleetOwnership<A, C, ParentPath>,
    complete_to: Recipient<P>,
    slots: Vec<Slot<A, D, J, R>>,
    backlog: VecDeque<QueuedJob<A, D, J, R>>,
    backlog_capacity: usize,
    next_assignment: u64,
    interruption: InterruptionPolicy,
}

impl<A, D, J, R, C, P, ParentPath> PoolCore<A, D, J, R, C, P, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
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
                .any(|slot: &Slot<A, D, J, R>| slot.nonce == nonce)
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

    fn slot_position(&self, worker: A::Nonce) -> Result<usize, PoolError<A::Nonce>> {
        self.slots
            .iter()
            .position(|slot| slot.nonce == worker)
            .ok_or(PoolError::UnknownWorker(worker))
    }
}

impl<A, D, J, R, C, P> PoolCore<A, D, J, R, C, P, behavior::Here>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
{
    fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        complete_to: Recipient<P>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Self::with_parent(
            topology,
            configuration,
            complete_to,
            ProxyParentIngress::new(),
        )
    }
}

impl<A, D, J, R, C, P, ParentPath> PoolCore<A, D, J, R, C, P, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
{
    fn supervisor_transition(
        &mut self,
        event: PoolOwnershipEvent<A>,
    ) -> PoolTransition<A, D, J, R, C, ParentPath> {
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
        .map_err(|error| match error {
            OwnershipError::Fleet(FleetError::UnknownChild(nonce)) => {
                PoolError::UnknownWorker(nonce)
            }
            OwnershipError::Fleet(FleetError::DuplicateChild(nonce)) => {
                PoolError::DuplicateWorker(nonce)
            }
            OwnershipError::Fleet(FleetError::SequenceExhausted) => PoolError::SequenceExhausted,
            OwnershipError::FactoryIndex { index } => PoolError::WorkerFactoryIndex { index },
            OwnershipError::ChildShutdownRejected { nonce, reason } => {
                PoolError::ChildShutdownRejected { nonce, reason }
            }
            OwnershipError::CreationProvenanceMismatch {
                nonce,
                expected,
                observed,
            } => PoolError::ProxyCreationProvenanceMismatch {
                nonce,
                expected,
                observed,
            },
            OwnershipError::WorkerCreationProvenanceMismatch {
                proxy,
                worker,
                expected,
                observed,
            } => PoolError::WorkerCreationProvenanceMismatch {
                proxy,
                worker,
                expected,
                observed,
            },
        })?;
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
        reply_to: Recipient<D>,
        actions: &mut PoolActions<A, D, J, R, C, ParentPath>,
    ) {
        let can_dispatch = self
            .slots
            .iter()
            .any(|slot| matches!(slot.state, SlotState::Idle));
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions.sends.inner.send::<Delivery<D>, Own>(Delivery::new(
                reply_to,
                PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::BacklogFull,
                },
            ));
            return;
        }
        let dispatch_payload = payload.clone();
        self.backlog.push_back(QueuedJob {
            accepted: AcceptedJob {
                id: job,
                payload,
                reply_to,
                interruption: None,
                target: None,
            },
            dispatch_payload,
        });
        actions
            .sends
            .inner
            .send::<_, Own>(Delivery::new(reply_to, PoolResponse::Accepted { job }));
    }

    fn submit_to(
        &mut self,
        target: A::Nonce,
        job: JobId,
        payload: J,
        reply_to: Recipient<D>,
        actions: &mut PoolActions<A, D, J, R, C, ParentPath>,
    ) -> Admission {
        let Some(slot) = self.slots.iter().find(|slot| slot.nonce == target) else {
            actions.sends.inner.send::<Delivery<D>, Own>(Delivery::new(
                reply_to,
                PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::AffinityUnavailable,
                },
            ));
            return Admission::Rejected;
        };
        if matches!(slot.state, SlotState::Retired { .. }) {
            actions.sends.inner.send::<_, Own>(Delivery::new(
                reply_to,
                PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::AffinityUnavailable,
                },
            ));
            return Admission::Rejected;
        }
        let can_dispatch = matches!(slot.state, SlotState::Idle);
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions.sends.inner.send::<_, Own>(Delivery::new(
                reply_to,
                PoolResponse::Rejected {
                    job,
                    payload,
                    reason: PoolRejection::BacklogFull,
                },
            ));
            return Admission::Rejected;
        }
        let dispatch_payload = payload.clone();
        self.backlog.push_back(QueuedJob {
            accepted: AcceptedJob {
                id: job,
                payload,
                reply_to,
                interruption: None,
                target: Some(target),
            },
            dispatch_payload,
        });
        actions
            .sends
            .inner
            .send::<_, Own>(Delivery::new(reply_to, PoolResponse::Accepted { job }));
        Admission::Accepted
    }

    fn complete(
        &mut self,
        worker: A::Nonce,
        assignment: AssignmentId,
        result: R,
        actions: &mut PoolActions<A, D, J, R, C, ParentPath>,
    ) -> Result<(), PoolError<A::Nonce>> {
        let position = self.slot_position(worker)?;
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
                return Err(PoolError::StaleCompletion {
                    worker,
                    expected,
                    received: assignment,
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
                    SlotState::Retired { reason } => WorkerPhase::Retired { reason: *reason },
                };
                self.slots[position].state = state;
                return Err(PoolError::CompletionForUnavailableWorker { worker, phase });
            }
        };
        actions.sends.inner.send::<_, Own>(Delivery::new(
            job.reply_to,
            PoolResponse::Completed {
                job: job.id,
                result,
            },
        ));
        Ok(())
    }

    fn worker_stopped(
        &mut self,
        stopped: &WorkerStopped<A>,
        responses: &mut Vec<Delivery<D>>,
    ) -> Result<(), PoolError<A::Nonce>> {
        let position = self.slot_position(stopped.proxy)?;
        let phase = self.slots[position].phase();
        if matches!(
            phase,
            WorkerPhase::Installing | WorkerPhase::Stopping | WorkerPhase::Retired { .. }
        ) {
            return Err(PoolError::WorkerStoppedWhileUnavailable {
                worker: stopped.proxy,
                phase,
            });
        }
        if self.interruption == InterruptionPolicy::Retry {
            // Cloning a user payload can execute arbitrary `Clone` code. Prepare it
            // before committing the slot transition so unwinding cannot leave the
            // worker marked as installing while its accepted job is lost.
            let dispatch_payload = match &self.slots[position].state {
                SlotState::Assigned { job, .. } => Some(job.payload.clone()),
                SlotState::Idle => None,
                SlotState::Installing | SlotState::Stopping | SlotState::Retired { .. } => {
                    return Ok(());
                }
            };
            let previous =
                core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
            if let (SlotState::Assigned { mut job, .. }, Some(dispatch_payload)) =
                (previous, dispatch_payload)
            {
                job.interruption = Some(PoolInterruption::WorkerStopped {
                    worker: stopped.proxy,
                    outcome: stopped.outcome,
                });
                self.backlog.push_front(QueuedJob {
                    accepted: job,
                    dispatch_payload,
                });
            }
            return Ok(());
        }

        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
        if let SlotState::Assigned { job, .. } = previous {
            responses.push(Delivery::new(
                job.reply_to,
                PoolResponse::Interrupted {
                    job: job.id,
                    payload: job.payload,
                    reason: PoolInterruption::WorkerStopped {
                        worker: stopped.proxy,
                        outcome: stopped.outcome,
                    },
                },
            ));
        }
        Ok(())
    }

    fn fail_backlog_if_irrecoverable(
        &mut self,
        actions: &mut PoolActions<A, D, J, R, C, ParentPath>,
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
            actions.sends.inner.send::<_, Own>(Delivery::new(
                job.reply_to,
                PoolResponse::Interrupted {
                    job: job.id,
                    payload: job.payload,
                    reason: job
                        .interruption
                        .unwrap_or(PoolInterruption::NoRecoverableWorkers),
                },
            ));
        }
    }

    fn fail_jobs_for_retired_slot(
        &mut self,
        worker: A::Nonce,
        reason: WorkerRetirement,
        actions: &mut PoolActions<A, D, J, R, C, ParentPath>,
    ) {
        let mut retained = VecDeque::with_capacity(self.backlog.len());
        while let Some(queued) = self.backlog.pop_front() {
            if queued.accepted.target == Some(worker) {
                let job = queued.accepted;
                actions.sends.inner.send::<_, Own>(Delivery::new(
                    job.reply_to,
                    PoolResponse::Interrupted {
                        job: job.id,
                        payload: job.payload,
                        reason: job
                            .interruption
                            .unwrap_or(PoolInterruption::AffinityRetired { worker, reason }),
                    },
                ));
            } else {
                retained.push_back(queued);
            }
        }
        self.backlog = retained;
    }

    fn interrupt_all_for_shutdown(&mut self) -> Vec<Delivery<D>> {
        let mut responses = Vec::new();
        for slot in &mut self.slots {
            let previous = core::mem::replace(&mut slot.state, SlotState::Stopping);
            match previous {
                SlotState::Assigned { job, .. } => responses.push(Delivery::new(
                    job.reply_to,
                    PoolResponse::Interrupted {
                        job: job.id,
                        payload: job.payload,
                        reason: PoolInterruption::PoolShutdown,
                    },
                )),
                retired @ SlotState::Retired { .. } => slot.state = retired,
                SlotState::Installing | SlotState::Idle | SlotState::Stopping => {}
            }
        }
        for queued in self.backlog.drain(..) {
            let job = queued.accepted;
            responses.push(Delivery::new(
                job.reply_to,
                PoolResponse::Interrupted {
                    job: job.id,
                    payload: job.payload,
                    reason: PoolInterruption::PoolShutdown,
                },
            ));
        }
        responses
    }

    fn creation_resolved(
        &mut self,
        resolved: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<(), PoolError<A::Nonce>> {
        let position = self.slot_position(resolved.proxy)?;
        let phase = self.slots[position].phase();
        if !matches!(phase, WorkerPhase::Installing) {
            return Err(PoolError::CreationResolvedWhileUnavailable {
                worker: resolved.proxy,
                phase,
            });
        }
        self.slots[position].state = match resolved.result {
            Ok(()) => SlotState::Idle,
            Err(rejection) => SlotState::Retired {
                reason: WorkerRetirement::CreationRejected(rejection),
            },
        };
        Ok(())
    }

    fn dispatch(
        &mut self,
        actions: &mut PoolActions<A, D, J, R, C, ParentPath>,
    ) -> Result<(), PoolError<A::Nonce>> {
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
        let mut selected_by_slot: Vec<Option<QueuedJob<A, D, J, R>>> =
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
                ChildRoute::<ProxyWithParent<C, ParentPath>, StableProxyChildRole>::new(nonce);
            let job_id = job.id;
            self.slots[slot_position].state = SlotState::Assigned { assignment, job };
            actions
                .sends
                .inner
                .send::<Delivery<Proxy<C>>, Own>(Delivery::local_child(
                    route.recipient(),
                    ProxyCommand::Forward(PoolAssignment {
                        assignment,
                        job: job_id,
                        payload,
                        worker: nonce,
                        complete_to: self.complete_to,
                    }),
                ));
        }
        self.next_assignment = next_assignment;
        Ok(())
    }
}

impl<A, D, J, R, C, P, ParentPath> behavior::Protocol for PoolCore<A, D, J, R, C, P, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
{
    type Addr = A;
    type Msg = PoolMessage<A, D, J, R>;
}

impl<A, D, J, R, C, P, ParentPath> Behavior for PoolCore<A, D, J, R, C, P, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    P: crate::PoolAssignmentProtocol<Addr = A, Job = J>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A, Msg = PoolAssignment<P>>,
{
    type Protocol = Self;
    type Event = PoolEvent<A, D, J, R>;
    type Sends = PoolSends<A, D, J, R, C, ParentPath>;
    type Ph = Never;
    type Error = PoolError<A::Nonce>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        let actions = self.supervisor.initialize().map_err(|error| match error {
            OwnershipError::Fleet(FleetError::UnknownChild(nonce)) => {
                PoolError::UnknownWorker(nonce)
            }
            OwnershipError::Fleet(FleetError::DuplicateChild(nonce)) => {
                PoolError::DuplicateWorker(nonce)
            }
            OwnershipError::Fleet(FleetError::SequenceExhausted) => PoolError::SequenceExhausted,
            OwnershipError::FactoryIndex { index } => PoolError::WorkerFactoryIndex { index },
            OwnershipError::ChildShutdownRejected { nonce, reason } => {
                PoolError::ChildShutdownRejected { nonce, reason }
            }
            OwnershipError::CreationProvenanceMismatch {
                nonce,
                expected,
                observed,
            } => PoolError::ProxyCreationProvenanceMismatch {
                nonce,
                expected,
                observed,
            },
            OwnershipError::WorkerCreationProvenanceMismatch {
                proxy,
                worker,
                expected,
                observed,
            } => PoolError::WorkerCreationProvenanceMismatch {
                proxy,
                worker,
                expected,
                observed,
            },
        })?;
        Ok(Actions::new(
            SendLayer::new(actions.sends, PoolBehaviorSends::empty()),
            actions.creates,
            actions.become_,
        ))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Behavior(User {
                message:
                    PoolMessage::Submit {
                        job,
                        payload,
                        reply_to,
                    },
                ..
            }) => {
                if self.supervisor.is_shutting_down() {
                    let mut actions: PoolActions<A, D, J, R, C, ParentPath> = Actions::cont();
                    actions.sends.inner.responses.push(Delivery::new(
                        reply_to,
                        PoolResponse::Rejected {
                            job,
                            payload,
                            reason: PoolRejection::ShuttingDown,
                        },
                    ));
                    return Ok(actions);
                }
                let mut actions = Actions::cont();
                self.submit(job, payload, reply_to, &mut actions);
                self.dispatch(&mut actions)?;
                Ok(actions)
            }
            SupervisionEvent::Behavior(User {
                message:
                    PoolMessage::Completed {
                        worker,
                        assignment,
                        result,
                    },
                ..
            }) => {
                if self.supervisor.is_shutting_down() {
                    return Err(PoolError::ShuttingDown);
                }
                let mut actions = Actions::cont();
                self.complete(worker, assignment, result, &mut actions)?;
                self.dispatch(&mut actions)?;
                Ok(actions)
            }
            SupervisionEvent::WorkerStopped(stopped) => {
                if self.supervisor.is_shutting_down() {
                    return self.supervisor_transition(PoolOwnershipEvent::WorkerStopped(stopped));
                }
                let proxy = stopped.proxy;
                let mut responses = Vec::new();
                self.worker_stopped(&stopped, &mut responses)?;
                let mut actions =
                    self.supervisor_transition(PoolOwnershipEvent::WorkerStopped(stopped))?;
                actions.sends.inner.responses.extend(responses);
                let replacement_requested = actions
                    .sends
                    .owned
                    .replacement_commands
                    .iter()
                    .any(|delivery| delivery.to.is_local_child(proxy));
                if !replacement_requested {
                    let position = self.slot_position(proxy)?;
                    let reason = WorkerRetirement::ReplacementUnavailable;
                    self.slots[position].state = SlotState::Retired { reason };
                    self.fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.dispatch(&mut actions)?;
                self.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::WorkerCreationResolved(resolved) => {
                if self.supervisor.is_shutting_down() {
                    return self.supervisor_transition(PoolOwnershipEvent::WorkerCreationResolved(
                        resolved,
                    ));
                }
                let proxy = resolved.proxy;
                self.creation_resolved(&resolved)?;
                let mut actions = self
                    .supervisor_transition(PoolOwnershipEvent::WorkerCreationResolved(resolved))?;
                if let Some(WorkerPhase::Retired { reason }) = self.worker_phase(proxy) {
                    self.fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.dispatch(&mut actions)?;
                self.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::ChildStopped(stopped) => {
                self.supervisor_transition(PoolOwnershipEvent::ChildStopped(stopped))
            }
            SupervisionEvent::CreationResolved(resolved) => {
                self.supervisor_transition(PoolOwnershipEvent::CreationResolved(resolved))
            }
            SupervisionEvent::ShutdownRequested(_) => {
                let responses = self.interrupt_all_for_shutdown();
                let mut actions =
                    self.supervisor_transition(PoolOwnershipEvent::ShutdownRequested)?;
                actions.sends.inner.responses.extend(responses);
                Ok(actions)
            }
            SupervisionEvent::ChildShutdownRejected(rejected) => {
                self.supervisor_transition(PoolOwnershipEvent::ChildShutdownRejected(rejected))
            }
        }
    }
}

/// Public FIFO worker-pool behavior with its completion protocol fixed by the
/// pool's own message signature.
pub struct WorkerPoolWithParent<A: Address, D, J, R, C, ParentPath>
where
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol:
        crate::Protocol<Addr = A, Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R>>>,
{
    core: PoolCore<A, D, J, R, C, crate::WorkerPoolProtocol<A, D, J, R>, ParentPath>,
}

/// A FIFO worker pool whose proxy reports target its direct event layer.
pub type WorkerPool<A, D, J, R, C> = WorkerPoolWithParent<A, D, J, R, C, behavior::Here>;

impl<A, D, J, R, C, ParentPath> WorkerPoolWithParent<A, D, J, R, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol:
        crate::Protocol<Addr = A, Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R>>>,
{
    /// Construct a pool whose completion destination implements this exact
    /// pool protocol.
    pub fn with_parent(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        complete_to: Recipient<crate::WorkerPoolProtocol<A, D, J, R>>,
        proxy_parent: ProxyParentIngress<A, ParentPath>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        PoolCore::with_parent(topology, configuration, complete_to, proxy_parent)
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

impl<A, D, J, R, C> WorkerPoolWithParent<A, D, J, R, C, behavior::Here>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Ph = Never>,
    C::Protocol:
        crate::Protocol<Addr = A, Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R>>>,
{
    /// Construct a pool whose proxy reports target the pool's direct event layer.
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        complete_to: Recipient<crate::WorkerPoolProtocol<A, D, J, R>>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Self::with_parent(
            topology,
            configuration,
            complete_to,
            ProxyParentIngress::new(),
        )
    }
}

impl<A, D, J, R, C, ParentPath> Behavior for WorkerPoolWithParent<A, D, J, R, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol:
        crate::Protocol<Addr = A, Msg = PoolAssignment<crate::WorkerPoolProtocol<A, D, J, R>>>,
{
    type Protocol = crate::WorkerPoolProtocol<A, D, J, R>;
    type Event = WorkerPoolEvent<A, D, J, R>;
    type Sends = WorkerPoolSends<A, D, J, R, C, ParentPath>;
    type Ph = Never;
    type Error = PoolError<A::Nonce>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(&mut self, turn: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.core.init(turn)
    }

    fn transition(
        &mut self,
        turn: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        self.core.transition(turn, event)
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
        type Msg = PoolAssignment<WorkerPoolProtocol<MailAddr, TestReply, u8, ()>>;
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
        let mut pool = PoolCore::new(
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
        )
        .unwrap();
        behavior::initialize(&mut pool).unwrap();

        for job in 1..=3 {
            behavior::delegate_transition(
                &mut pool,
                SupervisionEvent::Behavior(User::new(
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
        pool.slots[0].state = SlotState::Idle;
        pool.slots[1].state = SlotState::Idle;

        let mut actions: PoolActions<MailAddr, TestReply, u8, (), TestWorker> = Actions::cont();
        pool.dispatch(&mut actions).unwrap();

        let assignments = &actions.sends.inner.assignments;
        assert_eq!(assignments.len(), 2);
        for (index, expected_job) in [JobId(1), JobId(2)].into_iter().enumerate() {
            assert!(
                assignments[index]
                    .to
                    .is_local_child(u64::try_from(index).unwrap())
            );
            let ProxyCommand::Forward(assignment) = &assignments[index].message else {
                panic!("pool dispatches with Forward");
            };
            assert_eq!(
                assignment.assignment,
                AssignmentId(u64::try_from(index).unwrap())
            );
            assert_eq!(assignment.job, expected_job);
        }
        assert_eq!(pool.backlog.len(), 1);
        assert_eq!(pool.backlog[0].accepted.id, JobId(3));
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
pub struct KeyedWorkerPool<A: Address, D, K, J, R, C, S>
where
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    pool: PoolCore<A, D, J, R, C, crate::KeyedWorkerPoolProtocol<A, D, K, J, R>>,
    bindings: Vec<(K, A::Nonce)>,
    selector: S,
}

impl<A, D, K, J, R, C, S> KeyedWorkerPool<A, D, K, J, R, C, S>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    K: Eq,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R>>,
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
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        configuration: PoolConfiguration,
        selector: S,
        complete_to: Recipient<crate::KeyedWorkerPoolProtocol<A, D, K, J, R>>,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Ok(Self {
            pool: PoolCore::new(topology, configuration, complete_to)?,
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

    fn rebalance(&mut self, key: K, worker: A::Nonce) -> Result<(), PoolError<A::Nonce>> {
        let position = self.pool.slot_position(worker)?;
        if let SlotState::Retired { reason } = self.pool.slots[position].state {
            return Err(PoolError::RebalanceToRetiredWorker { worker, reason });
        }
        if let Some((_, bound)) = self.bindings.iter_mut().find(|(bound, _)| *bound == key) {
            *bound = worker;
        } else {
            self.bindings.push((key, worker));
        }
        Ok(())
    }
}

impl<A, D, K, J, R, C, S> Behavior for KeyedWorkerPool<A, D, K, J, R, C, S>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    K: Eq,
    J: Clone,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<
            Addr = A,
            Msg = PoolAssignment<crate::KeyedWorkerPoolProtocol<A, D, K, J, R>>,
        >,
    S: AffinitySelector<K, A::Nonce>,
{
    type Protocol = crate::KeyedWorkerPoolProtocol<A, D, K, J, R>;
    type Event = KeyedPoolEvent<A, D, K, J, R>;
    type Sends = PoolSends<A, D, J, R, C>;
    type Ph = Never;
    type Error = PoolError<A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        behavior::initialize(&mut self.pool)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Behavior(User {
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
                    let mut actions: PoolActions<A, D, J, R, C> = Actions::cont();
                    actions.sends.inner.responses.push(Delivery::new(
                        reply_to,
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
            SupervisionEvent::Behavior(User {
                message:
                    KeyedPoolMessage::Completed {
                        worker,
                        assignment,
                        result,
                    },
                ..
            }) => {
                if self.pool.supervisor.is_shutting_down() {
                    return Err(PoolError::ShuttingDown);
                }
                let mut actions = Actions::cont();
                self.pool
                    .complete(worker, assignment, result, &mut actions)?;
                self.pool.dispatch(&mut actions)?;
                Ok(actions)
            }
            SupervisionEvent::Behavior(User {
                message: KeyedPoolMessage::Rebalance { key, worker },
                ..
            }) => {
                if self.pool.supervisor.is_shutting_down() {
                    return Err(PoolError::ShuttingDown);
                }
                self.rebalance(key, worker)?;
                Ok(Actions::cont())
            }
            SupervisionEvent::WorkerStopped(stopped) => behavior::delegate_transition(
                &mut self.pool,
                SupervisionEvent::WorkerStopped(stopped),
            ),
            SupervisionEvent::WorkerCreationResolved(resolved) => behavior::delegate_transition(
                &mut self.pool,
                SupervisionEvent::WorkerCreationResolved(resolved),
            ),
            SupervisionEvent::ChildStopped(stopped) => behavior::delegate_transition(
                &mut self.pool,
                SupervisionEvent::ChildStopped(stopped),
            ),
            SupervisionEvent::CreationResolved(resolved) => behavior::delegate_transition(
                &mut self.pool,
                SupervisionEvent::CreationResolved(resolved),
            ),
            SupervisionEvent::ShutdownRequested(requested) => behavior::delegate_transition(
                &mut self.pool,
                SupervisionEvent::ShutdownRequested(requested),
            ),
            SupervisionEvent::ChildShutdownRejected(rejected) => behavior::delegate_transition(
                &mut self.pool,
                SupervisionEvent::ChildShutdownRejected(rejected),
            ),
        }
    }
}
