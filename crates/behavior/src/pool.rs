//! A bounded, FIFO worker pool expressed entirely as a pure behavior.
//!
//! Pool scheduling is a derived Bombay construction, not an actor-model
//! primitive. Runtime installation, delivery, and observation remain effects
//! for an interpreter; this module owns only their typed protocol and fold.

use core::convert::Infallible;
use core::marker::PhantomData;
use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use crate::{
    Actions, Address, Behavior, Births, Crash, CreationRejection, Delivery, Exit, Never, Own,
    Proxy, ProxyCommand, Recipient, RestartPolicy, SendAlgebra, SendInput, Strategy,
    SupervisionEvent, Supervisor, SupervisorSends, User, WorkerCreationResolved, WorkerStopped,
    delegate_transition,
};

/// Caller-chosen identity used to correlate pool responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

/// Pool-owned correlation token for one exact dispatch attempt.
///
/// This is not an actor identity or evidence that delivery occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssignmentId(pub u64);

/// One assignment accepted by a worker behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolAssignment<J> {
    pub assignment: AssignmentId,
    pub job: JobId,
    pub payload: J,
}

/// Messages accepted by a pool coordinator.
#[derive(Clone, PartialEq, Eq)]
pub enum PoolMessage<A, D, J, R>
where
    A: Address,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
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
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
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

/// Public, payload-free view of one stable worker slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerPhase {
    Installing,
    Idle,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolConfigError<N> {
    /// No stable worker slot exists, so accepted ownership could never end.
    NoWorkers,
    /// Two configured positions selected the same stable worker nonce.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError<N> {
    UnknownWorker(N),
    CompletionForUnavailableWorker {
        worker: N,
        phase: WorkerPhase,
    },
    StaleCompletion {
        worker: N,
        expected: AssignmentId,
        received: AssignmentId,
    },
    WorkerStoppedWhileUnavailable {
        worker: N,
        phase: WorkerPhase,
    },
    CreationResolvedWhileUnavailable {
        worker: N,
        phase: WorkerPhase,
    },
    RebalanceToRetiredWorker {
        worker: N,
        reason: WorkerRetirement,
    },
}

struct AcceptedJob<A: Address, D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    id: JobId,
    payload: J,
    reply_to: Recipient<D>,
    interruption: Option<PoolInterruption<A>>,
    target: Option<A::Nonce>,
}

struct QueuedJob<A: Address, D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    accepted: AcceptedJob<A, D, J, R>,
    dispatch_payload: J,
}

enum SlotState<A: Address, D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    Installing,
    Idle,
    Assigned {
        assignment: AssignmentId,
        job: AcceptedJob<A, D, J, R>,
    },
    Retired {
        reason: WorkerRetirement,
    },
}

struct Slot<A: Address, D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>, J, R> {
    nonce: A::Nonce,
    state: SlotState<A, D, J, R>,
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

/// Concrete event sum for a [`KeyedWorkerPool`].
pub type KeyedPoolEvent<A, D, K, J, R> = SupervisionEvent<User<A, KeyedPoolMessage<A, D, K, J, R>>>;

/// Named pool-owned delivery lanes.
pub struct PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Addr = A, Ph = Never>,
{
    /// Admission and terminal responses addressed to submitters.
    pub responses: Vec<Delivery<D>>,
    /// Assignments addressed to the selected stable worker proxies.
    pub assignments: Vec<Delivery<Proxy<C>>>,
}

impl<A, D, J, R, C> SendAlgebra for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Addr = A, Ph = Never>,
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

impl<A, D, J, R, C> SendInput<Delivery<D>, Own> for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn emit(&mut self, input: Delivery<D>) {
        self.responses.push(input);
    }
}

impl<A, D, J, R, C> SendInput<Delivery<Proxy<C>>, Own> for PoolBehaviorSends<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn emit(&mut self, input: Delivery<Proxy<C>>) {
        self.assignments.push(input);
    }
}

type KernelSends<A, D, J, R, C> = PoolBehaviorSends<A, D, J, R, C>;

/// Pool effects keep responses and assignments in named, independently
/// appendable lanes within the supervised behavior send product.
pub type PoolSends<A, D, J, R, C> = SupervisorSends<A, KernelSends<A, D, J, R, C>, C>;

/// Complete action type returned by a [`WorkerPool`] transition.
pub type PoolActions<A, D, J, R, C> = Actions<A, Never, PoolSends<A, D, J, R, C>, Births<Proxy<C>>>;

#[allow(
    clippy::type_complexity,
    reason = "the marker retains the complete pool topology signature"
)]
struct PoolKernel<A: Address, D, J, R, C>(PhantomData<fn(A, D, J, R, C)>);

impl<A: Address, D, J, R, C> PoolKernel<A, D, J, R, C> {
    const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<A, D, J, R, C> Behavior for PoolKernel<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    type Addr = A;
    type Msg = PoolMessage<A, D, J, R>;
    type Event = User<A, PoolMessage<A, D, J, R>>;
    type Sends = KernelSends<A, D, J, R, C>;
    type Ph = Never;
    type Error = Infallible;
    type Birth = Births<C>;

    fn init(&mut self) -> crate::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, _event: Self::Event) -> crate::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

type PoolSupervisor<A, D, J, R, C> = Supervisor<PoolKernel<A, D, J, R, C>, C>;

/// A fixed, homogeneous, bounded FIFO worker pool.
///
/// Each configured nonce names one stable supervised proxy. Jobs are assigned
/// only after a successful worker-creation result makes that slot idle. The
/// retained state records an assignment before the corresponding delivery is
/// returned, and a completion must carry the exact assignment token.
///
/// # Panics
///
/// Admission or retry propagates a panic from the application payload's
/// `Clone` implementation before changing pool state. Dispatch panics at the
/// physical assignment-counter boundary before committing its dispatch plan;
/// the executor's poison-before-step contract makes that actor turn terminal
/// rather than exposing partial successor state. The final counter value is
/// deliberately reserved so every successful batch has a representable
/// successor counter. This is a Bombay implementation boundary, not an actor
/// model law.
///
/// A worker with any other message protocol cannot form a pool:
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, PoolResponse, User, WorkerPool};
///
/// struct Reply;
/// struct WrongWorker;
/// impl Behavior for Reply {
///     type Addr = MailAddr;
///     type Msg = PoolResponse<String, (), MailAddr>;
///     type Event = User<MailAddr, Self::Msg>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// impl Behavior for WrongWorker {
///     type Addr = MailAddr;
///     type Msg = u8;
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<behavior::Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self) -> behavior::BehaviorActed<Self> { unimplemented!() }
///     fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> { unimplemented!() }
/// }
///
/// // `WrongWorker::Msg` is not `PoolAssignment<String>`.
/// let _: Option<WorkerPool<MailAddr, Reply, String, (), WrongWorker>> = None;
/// ```
pub struct WorkerPool<A: Address, D, J, R, C>
where
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    supervisor: PoolSupervisor<A, D, J, R, C>,
    slots: Vec<Slot<A, D, J, R>>,
    backlog: VecDeque<QueuedJob<A, D, J, R>>,
    backlog_capacity: usize,
    next_assignment: u64,
    interruption: InterruptionPolicy,
}

impl<A, D, J, R, C> WorkerPool<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    /// Construct a pool after proving that every configured child route is
    /// unique.
    ///
    /// # Errors
    ///
    /// Returns [`PoolConfigError::NoWorkers`] for an empty topology or
    /// [`PoolConfigError::DuplicateWorker`] for the first repeated
    /// creator-local nonce. No behavior or creation request is produced.
    #[allow(
        clippy::too_many_arguments,
        reason = "the arguments expose the complete pool policy"
    )]
    pub fn new(
        nonces: fn(usize) -> A::Nonce,
        count: usize,
        build: fn(usize) -> C,
        backlog_capacity: usize,
        interruption: InterruptionPolicy,
        restart_policy: RestartPolicy,
        max_restarts: u32,
        restart_window: Duration,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        if count == 0 {
            return Err(PoolConfigError::NoWorkers);
        }
        let mut slots = Vec::with_capacity(count);
        for index in 0..count {
            let nonce = nonces(index);
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
            supervisor: Supervisor::new(
                PoolKernel::new(),
                nonces,
                count,
                build,
                Strategy::OneForOne,
                restart_policy,
                max_restarts,
                restart_window,
            ),
            slots,
            backlog: VecDeque::new(),
            backlog_capacity,
            next_assignment: 0,
            interruption,
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
            .map(|slot| match &slot.state {
                SlotState::Installing => WorkerPhase::Installing,
                SlotState::Idle => WorkerPhase::Idle,
                SlotState::Assigned { assignment, job } => WorkerPhase::Assigned {
                    assignment: *assignment,
                    job: job.id,
                },
                SlotState::Retired { reason } => WorkerPhase::Retired { reason: *reason },
            })
    }

    fn slot_position(&self, worker: A::Nonce) -> Result<usize, PoolError<A::Nonce>> {
        self.slots
            .iter()
            .position(|slot| slot.nonce == worker)
            .ok_or(PoolError::UnknownWorker(worker))
    }
}

impl<A, D, J, R, C> WorkerPool<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    J: Clone,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    fn supervisor_transition(
        &mut self,
        event: PoolEvent<A, D, J, R>,
    ) -> PoolActions<A, D, J, R, C> {
        match delegate_transition(&mut self.supervisor, event) {
            Ok(actions) => actions,
            Err(never) => match never {},
        }
    }

    fn submit(
        &mut self,
        job: JobId,
        payload: J,
        reply_to: Recipient<D>,
        actions: &mut PoolActions<A, D, J, R, C>,
    ) {
        let can_dispatch = self
            .slots
            .iter()
            .any(|slot| matches!(slot.state, SlotState::Idle));
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
                .behavior
                .send::<Delivery<D>, Own>(Delivery::new(
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
            .behavior
            .send::<_, Own>(Delivery::new(reply_to, PoolResponse::Accepted { job }));
    }

    fn submit_to(
        &mut self,
        target: A::Nonce,
        job: JobId,
        payload: J,
        reply_to: Recipient<D>,
        actions: &mut PoolActions<A, D, J, R, C>,
    ) -> Admission {
        let Some(slot) = self.slots.iter().find(|slot| slot.nonce == target) else {
            actions
                .sends
                .behavior
                .send::<Delivery<D>, Own>(Delivery::new(
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
            actions.sends.behavior.send::<_, Own>(Delivery::new(
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
            actions.sends.behavior.send::<_, Own>(Delivery::new(
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
            .behavior
            .send::<_, Own>(Delivery::new(reply_to, PoolResponse::Accepted { job }));
        Admission::Accepted
    }

    fn complete(
        &mut self,
        worker: A::Nonce,
        assignment: AssignmentId,
        result: R,
        actions: &mut PoolActions<A, D, J, R, C>,
    ) -> Result<(), PoolError<A::Nonce>> {
        let position = self.slot_position(worker)?;
        let phase = self
            .worker_phase(worker)
            .expect("position proves the slot exists");
        let SlotState::Assigned {
            assignment: expected,
            ..
        } = &self.slots[position].state
        else {
            return Err(PoolError::CompletionForUnavailableWorker { worker, phase });
        };
        if *expected != assignment {
            return Err(PoolError::StaleCompletion {
                worker,
                expected: *expected,
                received: assignment,
            });
        }
        let SlotState::Assigned { job, .. } =
            core::mem::replace(&mut self.slots[position].state, SlotState::Idle)
        else {
            unreachable!("the state was proven assigned")
        };
        actions.sends.behavior.send::<_, Own>(Delivery::new(
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
        let phase = self
            .worker_phase(stopped.proxy)
            .expect("position proves the slot exists");
        if matches!(phase, WorkerPhase::Installing | WorkerPhase::Retired { .. }) {
            return Err(PoolError::WorkerStoppedWhileUnavailable {
                worker: stopped.proxy,
                phase,
            });
        }
        if self.interruption == InterruptionPolicy::Retry {
            if let SlotState::Assigned { job, .. } = &self.slots[position].state {
                let dispatch_payload = job.payload.clone();
                let SlotState::Assigned { mut job, .. } =
                    core::mem::replace(&mut self.slots[position].state, SlotState::Installing)
                else {
                    unreachable!("the assigned state was matched before committing retry")
                };
                job.interruption = Some(PoolInterruption::WorkerStopped {
                    worker: stopped.proxy,
                    outcome: stopped.outcome,
                });
                self.backlog.push_front(QueuedJob {
                    accepted: job,
                    dispatch_payload,
                });
            } else {
                self.slots[position].state = SlotState::Installing;
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

    fn fail_backlog_if_irrecoverable(&mut self, actions: &mut PoolActions<A, D, J, R, C>) {
        if self
            .slots
            .iter()
            .any(|slot| !matches!(slot.state, SlotState::Retired { .. }))
        {
            return;
        }
        for queued in self.backlog.drain(..) {
            let job = queued.accepted;
            actions.sends.behavior.send::<_, Own>(Delivery::new(
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
        actions: &mut PoolActions<A, D, J, R, C>,
    ) {
        let mut retained = VecDeque::with_capacity(self.backlog.len());
        while let Some(queued) = self.backlog.pop_front() {
            if queued.accepted.target == Some(worker) {
                let job = queued.accepted;
                actions.sends.behavior.send::<_, Own>(Delivery::new(
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

    fn creation_resolved(
        &mut self,
        resolved: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<(), PoolError<A::Nonce>> {
        let position = self.slot_position(resolved.proxy)?;
        let phase = self
            .worker_phase(resolved.proxy)
            .expect("position proves the slot exists");
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

    fn dispatch(&mut self, actions: &mut PoolActions<A, D, J, R, C>) {
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
        let count =
            u64::try_from(plan.len()).expect("a pool cannot contain more than u64::MAX slots");
        let next_assignment = self
            .next_assignment
            .checked_add(count)
            .expect("pool assignment identifiers exhausted");

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

        for (offset, (slot_position, queued)) in selected_by_slot
            .into_iter()
            .enumerate()
            .filter_map(|(slot_position, queued)| queued.map(|queued| (slot_position, queued)))
            .enumerate()
        {
            let payload = queued.dispatch_payload;
            let job = queued.accepted;
            let assignment = AssignmentId(
                self.next_assignment
                    + u64::try_from(offset).expect("offset is bounded by the checked plan length"),
            );
            let nonce = self.slots[slot_position].nonce;
            let job_id = job.id;
            self.slots[slot_position].state = SlotState::Assigned { assignment, job };
            actions
                .sends
                .behavior
                .send::<Delivery<Proxy<C>>, Own>(Delivery::new(
                    Recipient::child(nonce),
                    ProxyCommand::Forward(PoolAssignment {
                        assignment,
                        job: job_id,
                        payload,
                    }),
                ));
        }
        self.next_assignment = next_assignment;
    }
}

impl<A, D, J, R, C> Behavior for WorkerPool<A, D, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    J: Clone,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    type Addr = A;
    type Msg = PoolMessage<A, D, J, R>;
    type Event = PoolEvent<A, D, J, R>;
    type Sends = PoolSends<A, D, J, R, C>;
    type Ph = Never;
    type Error = PoolError<A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self) -> crate::BehaviorActed<Self> {
        match self.supervisor.init() {
            Ok(actions) => Ok(actions),
            Err(never) => match never {},
        }
    }

    fn transition(&mut self, event: Self::Event) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Inner(User {
                message:
                    PoolMessage::Submit {
                        job,
                        payload,
                        reply_to,
                    },
                ..
            }) => {
                let mut actions = Actions::cont();
                self.submit(job, payload, reply_to, &mut actions);
                self.dispatch(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::Inner(User {
                message:
                    PoolMessage::Completed {
                        worker,
                        assignment,
                        result,
                    },
                ..
            }) => {
                let mut actions = Actions::cont();
                self.complete(worker, assignment, result, &mut actions)?;
                self.dispatch(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::WorkerStopped(stopped) => {
                let proxy = stopped.proxy;
                let mut responses = Vec::new();
                self.worker_stopped(&stopped, &mut responses)?;
                let mut actions =
                    self.supervisor_transition(SupervisionEvent::WorkerStopped(stopped));
                actions.sends.behavior.responses.extend(responses);
                let replacement_requested = actions
                    .sends
                    .replacement_commands
                    .iter()
                    .any(|delivery| delivery.to.is_child(proxy));
                if !replacement_requested {
                    let position = self.slot_position(proxy)?;
                    let reason = WorkerRetirement::ReplacementUnavailable;
                    self.slots[position].state = SlotState::Retired { reason };
                    self.fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.dispatch(&mut actions);
                self.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::WorkerCreationResolved(resolved) => {
                let proxy = resolved.proxy;
                self.creation_resolved(&resolved)?;
                let mut actions =
                    self.supervisor_transition(SupervisionEvent::WorkerCreationResolved(resolved));
                if let Some(WorkerPhase::Retired { reason }) = self.worker_phase(proxy) {
                    self.fail_jobs_for_retired_slot(proxy, reason, &mut actions);
                }
                self.dispatch(&mut actions);
                self.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::ChildStopped(stopped) => {
                Ok(self.supervisor_transition(SupervisionEvent::ChildStopped(stopped)))
            }
            SupervisionEvent::CreationResolved(resolved) => {
                Ok(self.supervisor_transition(SupervisionEvent::CreationResolved(resolved)))
            }
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
    use crate::{MailAddr, NoBirths};

    struct TestReply;

    impl Behavior for TestReply {
        type Addr = MailAddr;
        type Msg = PoolResponse<u8, (), MailAddr>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn init(&mut self) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }

        fn transition(&mut self, _: Self::Event) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[derive(Clone, Copy)]
    struct TestWorker;

    impl Behavior for TestWorker {
        type Addr = MailAddr;
        type Msg = PoolAssignment<u8>;
        type Event = User<MailAddr, PoolAssignment<u8>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn init(&mut self) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }

        fn transition(&mut self, _: Self::Event) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn test_worker(_: usize) -> TestWorker {
        TestWorker
    }

    #[test]
    fn one_dispatch_batch_preserves_fifo_jobs_across_index_removal() {
        let mut pool = WorkerPool::new(
            |index| u64::try_from(index).unwrap(),
            2,
            test_worker,
            3,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        )
        .unwrap();
        pool.init().unwrap();

        for job in 1..=3 {
            pool.transition(SupervisionEvent::Inner(User::new(
                MailAddr(90),
                PoolMessage::Submit {
                    job: JobId(job),
                    payload: u8::try_from(job).unwrap(),
                    reply_to: Recipient::global(MailAddr(91)),
                },
            )))
            .unwrap();
        }
        pool.slots[0].state = SlotState::Idle;
        pool.slots[1].state = SlotState::Idle;

        let mut actions: PoolActions<MailAddr, TestReply, u8, (), TestWorker> = Actions::cont();
        pool.dispatch(&mut actions);

        let assignments = &actions.sends.behavior.assignments;
        assert_eq!(assignments.len(), 2);
        for (index, expected_job) in [JobId(1), JobId(2)].into_iter().enumerate() {
            assert!(
                assignments[index]
                    .to
                    .is_child(u64::try_from(index).unwrap())
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
/// use behavior::{Actions, Behavior, KeyedWorkerPool, MailAddr, Never, NoBirths, PoolResponse, User};
/// struct NonKey(f64);
/// struct Reply;
/// struct Worker;
/// impl Behavior for Reply {
///     type Addr = MailAddr;
///     type Msg = PoolResponse<u8, (), MailAddr>;
///     type Event = User<MailAddr, Self::Msg>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = behavior::PoolAssignment<u8>,
///     sends = Vec<Never>,
///     births = NoBirths,
///     error = Never,
/// )]
/// impl Worker {
///     fn init(&mut self) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: behavior::PoolAssignment<u8>) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
/// }
/// let _: Option<KeyedWorkerPool<MailAddr, Reply, NonKey, u8, (), Worker, fn(&NonKey) -> u64>> = None;
/// ```
pub struct KeyedWorkerPool<A: Address, D, K, J, R, C, S>
where
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    K: Eq,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
    S: AffinitySelector<K, A::Nonce>,
{
    pool: WorkerPool<A, D, J, R, C>,
    bindings: Vec<(K, A::Nonce)>,
    selector: S,
}

impl<A, D, K, J, R, C, S> KeyedWorkerPool<A, D, K, J, R, C, S>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    K: Eq,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
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
    #[allow(
        clippy::too_many_arguments,
        reason = "the arguments expose the complete pool and affinity policy"
    )]
    pub fn new(
        nonces: fn(usize) -> A::Nonce,
        count: usize,
        build: fn(usize) -> C,
        backlog_capacity: usize,
        interruption: InterruptionPolicy,
        restart_policy: RestartPolicy,
        max_restarts: u32,
        restart_window: Duration,
        selector: S,
    ) -> Result<Self, PoolConfigError<A::Nonce>> {
        Ok(Self {
            pool: WorkerPool::new(
                nonces,
                count,
                build,
                backlog_capacity,
                interruption,
                restart_policy,
                max_restarts,
                restart_window,
            )?,
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
    D: Behavior<Addr = A, Msg = PoolResponse<J, R, A>>,
    K: Eq,
    J: Clone,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
    S: AffinitySelector<K, A::Nonce>,
{
    type Addr = A;
    type Msg = KeyedPoolMessage<A, D, K, J, R>;
    type Event = KeyedPoolEvent<A, D, K, J, R>;
    type Sends = PoolSends<A, D, J, R, C>;
    type Ph = Never;
    type Error = PoolError<A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self) -> crate::BehaviorActed<Self> {
        self.pool.init()
    }

    fn transition(&mut self, event: Self::Event) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Inner(User {
                message:
                    KeyedPoolMessage::Submit {
                        key,
                        job,
                        payload,
                        reply_to,
                    },
                ..
            }) => {
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
                self.pool.dispatch(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::Inner(User {
                message:
                    KeyedPoolMessage::Completed {
                        worker,
                        assignment,
                        result,
                    },
                ..
            }) => {
                let mut actions = Actions::cont();
                self.pool
                    .complete(worker, assignment, result, &mut actions)?;
                self.pool.dispatch(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::Inner(User {
                message: KeyedPoolMessage::Rebalance { key, worker },
                ..
            }) => {
                self.rebalance(key, worker)?;
                Ok(Actions::cont())
            }
            SupervisionEvent::WorkerStopped(stopped) => self
                .pool
                .transition(SupervisionEvent::WorkerStopped(stopped)),
            SupervisionEvent::WorkerCreationResolved(resolved) => self
                .pool
                .transition(SupervisionEvent::WorkerCreationResolved(resolved)),
            SupervisionEvent::ChildStopped(stopped) => self
                .pool
                .transition(SupervisionEvent::ChildStopped(stopped)),
            SupervisionEvent::CreationResolved(resolved) => self
                .pool
                .transition(SupervisionEvent::CreationResolved(resolved)),
        }
    }
}
