//! A bounded, FIFO worker pool expressed entirely as a pure behavior.
//!
//! Pool scheduling is a derived Bombay construction, not an actor-model
//! primitive. Runtime installation, delivery, and observation remain effects
//! for an interpreter; this module owns only their typed protocol and fold.

use core::convert::Infallible;
use core::marker::PhantomData;
use std::collections::VecDeque;
use std::time::Duration;

use crate::{
    Actions, Address, Behavior, Births, Crash, CreationRejection, Delivery, Exit, Never, Own,
    Proxy, ProxyCommand, Recipient, RestartPolicy, SendAlgebra, SendProduct, Strategy,
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
pub enum PoolMessage<A: Address, J, R> {
    Submit {
        job: JobId,
        payload: J,
        reply_to: Recipient<A, PoolResponse<J, R, A>>,
    },
    Completed {
        worker: A::Nonce,
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
pub enum KeyedPoolMessage<A: Address, K, J, R> {
    Submit {
        key: K,
        job: JobId,
        payload: J,
        reply_to: Recipient<A, PoolResponse<J, R, A>>,
    },
    Completed {
        worker: A::Nonce,
        assignment: AssignmentId,
        result: R,
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
}

/// Why an accepted assignment ended without a worker completion.
#[derive(Clone, PartialEq, Eq)]
pub enum PoolInterruption<A: Address> {
    WorkerStopped {
        worker: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
    },
    NoRecoverableWorkers,
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
    DuplicateWorker(N),
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
    AssignmentSequenceExhausted,
}

struct AcceptedJob<A: Address, J, R> {
    id: JobId,
    payload: J,
    reply_to: Recipient<A, PoolResponse<J, R, A>>,
    interruption: Option<PoolInterruption<A>>,
    target: Option<A::Nonce>,
}

enum SlotState<A: Address, J, R> {
    Installing,
    Idle,
    Assigned {
        assignment: AssignmentId,
        job: AcceptedJob<A, J, R>,
    },
    Retired {
        reason: WorkerRetirement,
    },
}

struct Slot<A: Address, J, R> {
    nonce: A::Nonce,
    state: SlotState<A, J, R>,
}

/// The pool's concrete event sum, including existing supervision facts.
pub type PoolEvent<A, J, R> = SupervisionEvent<User<A, PoolMessage<A, J, R>>>;

/// Concrete event sum for a [`KeyedWorkerPool`].
pub type KeyedPoolEvent<A, K, J, R> = SupervisionEvent<User<A, KeyedPoolMessage<A, K, J, R>>>;

type KernelSends<A, J, R, C> =
    SendProduct<Vec<Delivery<A, PoolResponse<J, R, A>>>, Vec<Delivery<A, ProxyCommand<C>>>>;

/// Pool effects use existing recursively interpreted products: responses and
/// assignments remain distinct within the supervised behavior send lane.
pub type PoolSends<A, J, R, C> = SupervisorSends<A, KernelSends<A, J, R, C>, C>;

/// Complete action type returned by a [`WorkerPool`] transition.
pub type PoolActions<A, J, R, C> = Actions<A, Never, PoolSends<A, J, R, C>, Births<Proxy<C>>>;

struct PoolKernel<A: Address, J, R, C>(PhantomData<fn(A, J, R, C)>);

impl<A: Address, J, R, C> PoolKernel<A, J, R, C> {
    const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<A, J, R, C> Behavior for PoolKernel<A, J, R, C>
where
    A: Address,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    type Addr = A;
    type Msg = PoolMessage<A, J, R>;
    type Event = User<A, PoolMessage<A, J, R>>;
    type Sends = KernelSends<A, J, R, C>;
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

type PoolSupervisor<A, J, R, C> = Supervisor<PoolKernel<A, J, R, C>, C>;

/// A fixed, homogeneous, bounded FIFO worker pool.
///
/// Each configured nonce names one stable supervised proxy. Jobs are assigned
/// only after a successful worker-creation result makes that slot idle. The
/// retained state records an assignment before the corresponding delivery is
/// returned, and a completion must carry the exact assignment token.
///
/// A worker with any other message protocol cannot form a pool:
///
/// ```compile_fail
/// use behavior::{Behavior, MailAddr, Never, NoBirths, PoolAssignment, User, WorkerPool};
///
/// struct WrongWorker;
/// impl Behavior for WrongWorker {
///     type Addr = MailAddr;
///     type Msg = u8;
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<behavior::Delivery<MailAddr, Never>>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self) -> behavior::BehaviorActed<Self> { unimplemented!() }
///     fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> { unimplemented!() }
/// }
///
/// let _: Option<WorkerPool<MailAddr, String, (), WrongWorker>> = None;
/// ```
pub struct WorkerPool<A: Address, J, R, C>
where
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    supervisor: PoolSupervisor<A, J, R, C>,
    slots: Vec<Slot<A, J, R>>,
    backlog: VecDeque<AcceptedJob<A, J, R>>,
    backlog_capacity: usize,
    next_assignment: u64,
    interruption: InterruptionPolicy,
}

impl<A, J, R, C> WorkerPool<A, J, R, C>
where
    A: Address,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    /// Construct a pool after proving that every configured child route is
    /// unique.
    ///
    /// # Errors
    ///
    /// Returns [`PoolConfigError::DuplicateWorker`] for the first repeated
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
        let mut slots = Vec::with_capacity(count);
        for index in 0..count {
            let nonce = nonces(index);
            if slots.iter().any(|slot: &Slot<A, J, R>| slot.nonce == nonce) {
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

impl<A, J, R, C> WorkerPool<A, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    J: Clone,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    fn supervisor_transition(&mut self, event: PoolEvent<A, J, R>) -> PoolActions<A, J, R, C> {
        match delegate_transition(&mut self.supervisor, event) {
            Ok(actions) => actions,
            Err(never) => match never {},
        }
    }

    fn submit(
        &mut self,
        job: JobId,
        payload: J,
        reply_to: Recipient<A, PoolResponse<J, R, A>>,
        actions: &mut PoolActions<A, J, R, C>,
    ) {
        let can_dispatch = self
            .slots
            .iter()
            .any(|slot| matches!(slot.state, SlotState::Idle));
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
                .behavior
                .send::<_, crate::Inner<Own>>(Delivery::new(
                    reply_to,
                    PoolResponse::Rejected {
                        job,
                        payload,
                        reason: PoolRejection::BacklogFull,
                    },
                ));
            return;
        }
        self.backlog.push_back(AcceptedJob {
            id: job,
            payload,
            reply_to,
            interruption: None,
            target: None,
        });
        actions
            .sends
            .behavior
            .send::<_, crate::Inner<Own>>(Delivery::new(reply_to, PoolResponse::Accepted { job }));
    }

    fn submit_to(
        &mut self,
        target: A::Nonce,
        job: JobId,
        payload: J,
        reply_to: Recipient<A, PoolResponse<J, R, A>>,
        actions: &mut PoolActions<A, J, R, C>,
    ) -> bool {
        let Some(slot) = self.slots.iter().find(|slot| slot.nonce == target) else {
            actions
                .sends
                .behavior
                .send::<_, crate::Inner<Own>>(Delivery::new(
                    reply_to,
                    PoolResponse::Rejected {
                        job,
                        payload,
                        reason: PoolRejection::AffinityUnavailable,
                    },
                ));
            return false;
        };
        if matches!(slot.state, SlotState::Retired { .. }) {
            actions
                .sends
                .behavior
                .send::<_, crate::Inner<Own>>(Delivery::new(
                    reply_to,
                    PoolResponse::Rejected {
                        job,
                        payload,
                        reason: PoolRejection::AffinityUnavailable,
                    },
                ));
            return false;
        }
        let can_dispatch = matches!(slot.state, SlotState::Idle);
        if !can_dispatch && self.backlog.len() == self.backlog_capacity {
            actions
                .sends
                .behavior
                .send::<_, crate::Inner<Own>>(Delivery::new(
                    reply_to,
                    PoolResponse::Rejected {
                        job,
                        payload,
                        reason: PoolRejection::BacklogFull,
                    },
                ));
            return false;
        }
        self.backlog.push_back(AcceptedJob {
            id: job,
            payload,
            reply_to,
            interruption: None,
            target: Some(target),
        });
        actions
            .sends
            .behavior
            .send::<_, crate::Inner<Own>>(Delivery::new(reply_to, PoolResponse::Accepted { job }));
        true
    }

    fn complete(
        &mut self,
        worker: A::Nonce,
        assignment: AssignmentId,
        result: R,
        actions: &mut PoolActions<A, J, R, C>,
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
        actions
            .sends
            .behavior
            .send::<_, crate::Inner<Own>>(Delivery::new(
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
        responses: &mut Vec<Delivery<A, PoolResponse<J, R, A>>>,
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
        let previous = core::mem::replace(&mut self.slots[position].state, SlotState::Installing);
        if let SlotState::Assigned { job, .. } = previous {
            let interruption = PoolInterruption::WorkerStopped {
                worker: stopped.proxy,
                outcome: stopped.outcome.clone(),
            };
            match self.interruption {
                InterruptionPolicy::Fail => responses.push(Delivery::new(
                    job.reply_to,
                    PoolResponse::Interrupted {
                        job: job.id,
                        payload: job.payload,
                        reason: interruption,
                    },
                )),
                InterruptionPolicy::Retry => self.backlog.push_front(AcceptedJob {
                    interruption: Some(interruption),
                    ..job
                }),
            }
        }
        Ok(())
    }

    fn fail_backlog_if_irrecoverable(&mut self, actions: &mut PoolActions<A, J, R, C>) {
        if self
            .slots
            .iter()
            .any(|slot| !matches!(slot.state, SlotState::Retired { .. }))
        {
            return;
        }
        for job in self.backlog.drain(..) {
            actions
                .sends
                .behavior
                .send::<_, crate::Inner<Own>>(Delivery::new(
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

    fn dispatch(
        &mut self,
        actions: &mut PoolActions<A, J, R, C>,
    ) -> Result<(), PoolError<A::Nonce>> {
        for slot in &mut self.slots {
            if !matches!(slot.state, SlotState::Idle) || self.backlog.is_empty() {
                continue;
            }
            let Some(job_position) = self
                .backlog
                .iter()
                .position(|job| job.target.is_none_or(|target| target == slot.nonce))
            else {
                continue;
            };
            let next = self
                .next_assignment
                .checked_add(1)
                .ok_or(PoolError::AssignmentSequenceExhausted)?;
            let assignment = AssignmentId(self.next_assignment);
            self.next_assignment = next;
            let job = self
                .backlog
                .remove(job_position)
                .expect("the eligible backlog position was found");
            let delivery = Delivery::new(
                Recipient::child(slot.nonce),
                ProxyCommand::Forward(PoolAssignment {
                    assignment,
                    job: job.id,
                    payload: job.payload.clone(),
                }),
            );
            slot.state = SlotState::Assigned { assignment, job };
            actions.sends.behavior.send::<_, Own>(delivery);
        }
        Ok(())
    }
}

impl<A, J, R, C> Behavior for WorkerPool<A, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    J: Clone,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    type Addr = A;
    type Msg = PoolMessage<A, J, R>;
    type Event = PoolEvent<A, J, R>;
    type Sends = PoolSends<A, J, R, C>;
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
                self.dispatch(&mut actions)?;
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
                self.dispatch(&mut actions)?;
                Ok(actions)
            }
            SupervisionEvent::WorkerStopped(stopped) => {
                let proxy = stopped.proxy;
                let mut responses = Vec::new();
                self.worker_stopped(&stopped, &mut responses)?;
                let mut actions =
                    self.supervisor_transition(SupervisionEvent::WorkerStopped(stopped));
                actions.sends.behavior.inner.extend(responses);
                let replacement_requested = actions
                    .sends
                    .replacement_commands
                    .iter()
                    .any(|delivery| delivery.to.route() == crate::Route::Child(proxy));
                if !replacement_requested {
                    let position = self.slot_position(proxy)?;
                    self.slots[position].state = SlotState::Retired {
                        reason: WorkerRetirement::ReplacementUnavailable,
                    };
                }
                self.dispatch(&mut actions)?;
                self.fail_backlog_if_irrecoverable(&mut actions);
                Ok(actions)
            }
            SupervisionEvent::WorkerCreationResolved(resolved) => {
                self.creation_resolved(&resolved)?;
                let mut actions =
                    self.supervisor_transition(SupervisionEvent::WorkerCreationResolved(resolved));
                self.dispatch(&mut actions)?;
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

/// A worker pool whose admitted keys remain bound to stable worker slots.
///
/// The routing function selects a stable proxy nonce only when a key is first
/// admitted. Replacement incarnations remain behind that proxy, so they do
/// not alter affinity. [`KeyedPoolMessage::Rebalance`] is the sole transition
/// that changes an established binding, and jobs accepted before it retain
/// their original target.
///
/// Keys must have a concrete equality relation; a key type without `Eq` cannot
/// form an affinity table:
///
/// ```compile_fail
/// use behavior::{Actions, Delivery, KeyedWorkerPool, MailAddr, Never, NoBirths};
/// struct NonKey(f64);
/// struct Worker;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = behavior::PoolAssignment<u8>,
///     sends = Vec<Delivery<MailAddr, Never>>,
///     births = NoBirths,
///     error = Never,
/// )]
/// impl Worker {
///     fn init(&mut self) -> behavior::Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: behavior::PoolAssignment<u8>) -> behavior::Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
/// }
/// let _: Option<KeyedWorkerPool<MailAddr, NonKey, u8, (), Worker>> = None;
/// ```
pub struct KeyedWorkerPool<A: Address, K, J, R, C>
where
    K: Eq,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    pool: WorkerPool<A, J, R, C>,
    bindings: Vec<(K, A::Nonce)>,
    select: fn(&K) -> A::Nonce,
}

impl<A, K, J, R, C> KeyedWorkerPool<A, K, J, R, C>
where
    A: Address,
    K: Eq,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    /// Construct a key-persistent pool over the same fixed supervised slots as
    /// [`WorkerPool`]. The selector is pure and is consulted once per
    /// previously unseen key. It chooses behavior policy; runtime route
    /// resolution remains outside this type.
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
        select: fn(&K) -> A::Nonce,
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
            select,
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

impl<A, K, J, R, C> Behavior for KeyedWorkerPool<A, K, J, R, C>
where
    A: Address,
    A::Nonce: From<u64>,
    K: Clone + Eq,
    J: Clone,
    C: Behavior<Addr = A, Msg = PoolAssignment<J>, Ph = Never>,
{
    type Addr = A;
    type Msg = KeyedPoolMessage<A, K, J, R>;
    type Event = KeyedPoolEvent<A, K, J, R>;
    type Sends = PoolSends<A, J, R, C>;
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
                let target = existing.unwrap_or_else(|| (self.select)(&key));
                let mut actions = Actions::cont();
                let accepted = self
                    .pool
                    .submit_to(target, job, payload, reply_to, &mut actions);
                if accepted && existing.is_none() {
                    self.bindings.push((key, target));
                }
                self.pool.dispatch(&mut actions)?;
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
                self.pool.dispatch(&mut actions)?;
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
