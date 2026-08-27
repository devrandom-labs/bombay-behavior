//! Shared fixed-fleet ownership state machine.

use super::{FleetError, RestartBudget};
use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ObserveChild, ObserveCreation,
    ReplacementRequested, ScheduleAfter, ShutdownChild, TimerElapsed, TimerGeneration, TimerId,
    WorkerCreationResolved, WorkerStopped,
};
use crate::supervision::adapter::{
    ChildTopology, RestartConfiguration, RestartTiming, SupervisorSends,
};
use crate::supervision::{Backoff, RestartPolicy, SupervisionFailure, SupervisionLifecycle};
use crate::{CreationKind, Exit, ReportSupervisionFailure};
use behavior::{
    Actions, Address, Behavior, BehaviorLayer, Births, ChildInput, ChildInputIngress, ChildRoute,
    InterpreterRequests, Never, SendEffects, Step,
};

type OwnedActions<A, C, Stable> = Actions<A, Never, SupervisorSends<A, C, Stable>, Births<Stable>>;

pub(crate) struct OwnershipFold<A, C, Stable>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    pub actions: OwnedActions<A, C, Stable>,
    pub failure: Option<SupervisionFailure<A>>,
}

impl<A, C, Stable> OwnershipFold<A, C, Stable>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    fn actions(actions: OwnedActions<A, C, Stable>) -> Self {
        Self {
            actions,
            failure: None,
        }
    }
    fn failed(actions: OwnedActions<A, C, Stable>, failure: SupervisionFailure<A>) -> Self {
        Self {
            actions,
            failure: Some(failure),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum OwnershipError<A: Address> {
    #[error(transparent)]
    Fleet(#[from] FleetError<A::Nonce>),
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected(ChildShutdownRejected<A::Nonce>),
    #[error("a creation result does not belong to a pending stable-child creation")]
    UnexpectedCreation(CreationResolved<A>),
    #[error("a stable-child stop does not belong to the current ownership state")]
    UnexpectedChildStopped(ChildStopped<A>),
    #[error("a worker stop does not belong to the current worker incarnation")]
    UnexpectedWorkerStopped(WorkerStopped<A>),
    #[error("a worker creation result does not belong to a pending worker creation")]
    UnexpectedWorkerCreation(WorkerCreationResolved<A::Nonce>),
    #[error("a child-shutdown rejection does not belong to an outstanding shutdown request")]
    UnexpectedChildShutdownRejection(ChildShutdownRejected<A::Nonce>),
    #[error("stable-child creation provenance did not match the pending request")]
    CreationProvenanceMismatch {
        expected: CreationKind<A::Nonce>,
        observed: CreationResolved<A>,
    },
    #[error("a rejected stable-proxy creation contradicts its worker-creation fact")]
    ContradictoryStableAndWorkerCreation {
        proxy: CreationResolved<A>,
        worker: WorkerCreationResolved<A::Nonce>,
    },
    #[error("a rejected stable-proxy creation contradicts its worker-stop fact")]
    ContradictoryStableCreationAndWorkerStop {
        proxy: CreationResolved<A>,
        worker: WorkerStopped<A>,
    },
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        expected: CreationKind<A::Nonce>,
        observed: WorkerCreationResolved<A::Nonce>,
    },
    #[error("an exact delayed-restart timer contradicts the retained worker phase")]
    DelayedReplacementStateMismatch {
        event: TimerElapsed,
        child: A::Nonce,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Installation<N> {
    Pending {
        kind: CreationKind<N>,
    },
    Established,
    ShutdownRequested,
    Rejected {
        kind: CreationKind<N>,
        rejection: crate::CreationRejection,
    },
    Stopped,
}

enum WorkerInstallation<A: Address, Request> {
    AwaitingInitial,
    Running {
        resolved: WorkerCreationResolved<A::Nonce>,
    },
    ReplacementAccepted {
        subject: ReplacementSubject<A>,
        gate: ReplacementGate,
        request: Request,
    },
    ReplacementIssuedToRunning {
        resolved: WorkerCreationResolved<A::Nonce>,
    },
    AwaitingReplacement {
        stopped: WorkerStopped<A>,
    },
    WorkerCreationRejected {
        resolved: WorkerCreationResolved<A::Nonce>,
    },
    RetiredAfterStop {
        stopped: WorkerStopped<A>,
    },
    RetiredWithoutWorker,
}

impl<A: Address, Request> WorkerInstallation<A, Request> {
    const fn is_restartable(&self) -> bool {
        !matches!(
            self,
            Self::WorkerCreationRejected { .. }
                | Self::RetiredAfterStop { .. }
                | Self::RetiredWithoutWorker
        )
    }
}

struct OwnedSlot<A: Address, Request> {
    installation: Installation<A::Nonce>,
    worker: WorkerInstallation<A, Request>,
}

enum ReplacementSubject<A: Address> {
    InitialPending,
    Running {
        resolved: WorkerCreationResolved<A::Nonce>,
    },
    Stopped {
        stopped: WorkerStopped<A>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReplacementGate {
    Open,
    Timer(TimerElapsed),
}

enum Shutdown<N> {
    Running,
    Draining { awaiting: Vec<N> },
}

pub(crate) struct PreparedWorkerStop<A, C, Stable>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    decision: PreparedWorkerStopDecision<A, C, Stable>,
}

enum PreparedWorkerStopDecision<A, C, Stable>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    Retire {
        stopped: WorkerStopped<A>,
    },
    Accepted {
        trigger: WorkerStopped<A>,
        replacing: Vec<WorkerCreationResolved<A::Nonce>>,
        awaiting_initial: Vec<A::Nonce>,
        restart: RestartAdmission<A::Nonce>,
        updates: Vec<(A::Nonce, WorkerInstallation<A, ReplacementInput<C, Stable>>)>,
        inputs: Vec<ReplacementInput<C, Stable>>,
        schedule: Option<ScheduleAfter>,
    },
    Failed {
        stopped: WorkerStopped<A>,
        failure: SupervisionFailure<A>,
        restart: Option<RestartAdmission<A::Nonce>>,
    },
}

impl<A, C, Stable> PreparedWorkerStop<A, C, Stable>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    pub(crate) fn lifecycle(&self) -> SupervisionLifecycle<A> {
        match &self.decision {
            PreparedWorkerStopDecision::Retire { stopped } => {
                SupervisionLifecycle::RetiredAfterStop {
                    stopped: stopped.clone(),
                }
            }
            PreparedWorkerStopDecision::Accepted {
                trigger,
                replacing,
                awaiting_initial,
                ..
            } => SupervisionLifecycle::ReplacementStarted {
                trigger: trigger.clone(),
                replacing: replacing.clone(),
                awaiting_initial: awaiting_initial.clone(),
            },
            PreparedWorkerStopDecision::Failed { failure, .. } => {
                SupervisionLifecycle::Retired { failure: *failure }
            }
        }
    }
}

type ReplacementInput<C, Stable> =
    ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>;

#[derive(Clone)]
struct DelayCounter<N> {
    trigger: N,
    attempt: u32,
    generation: u64,
}

#[derive(Clone)]
struct PendingRestart<N> {
    elapsed: TimerElapsed,
    members: Vec<N>,
}

#[derive(Clone, Copy)]
enum TimerSequence {
    Next(u64),
    Exhausted,
}

#[derive(Clone)]
enum RestartAdmission<N> {
    Immediate {
        budget: RestartBudget,
    },
    Delayed {
        budget: RestartBudget,
        policy: Backoff,
        timers: TimerSequence,
        counters: Vec<DelayCounter<N>>,
        pending: Vec<PendingRestart<N>>,
    },
}

enum AdmittedRestart {
    Immediate,
    Delayed {
        elapsed: TimerElapsed,
        schedule: ScheduleAfter,
    },
}

impl AdmittedRestart {
    const fn gate(&self) -> ReplacementGate {
        match self {
            Self::Immediate => ReplacementGate::Open,
            Self::Delayed { elapsed, .. } => ReplacementGate::Timer(*elapsed),
        }
    }

    fn schedule(self) -> Option<ScheduleAfter> {
        match self {
            Self::Immediate => None,
            Self::Delayed { schedule, .. } => Some(schedule),
        }
    }
}

impl<N> RestartAdmission<N>
where
    N: Copy + Eq,
{
    const fn new(timing: RestartTiming, maximum: u32, window: std::time::Duration) -> Self {
        match timing {
            RestartTiming::Immediate => Self::Immediate {
                budget: RestartBudget::new(maximum, window),
            },
            RestartTiming::Delayed(policy) => Self::Delayed {
                budget: RestartBudget::new(maximum, window),
                policy,
                timers: TimerSequence::Next(0),
                counters: Vec::new(),
                pending: Vec::new(),
            },
        }
    }

    fn admit(
        &mut self,
        trigger: N,
        at: std::time::Instant,
        members: Vec<N>,
    ) -> Result<AdmittedRestart, crate::RestartDenial> {
        match self {
            Self::Immediate { budget } => {
                budget.admit(at, members.len())?;
                Ok(AdmittedRestart::Immediate)
            }
            Self::Delayed {
                budget,
                policy,
                timers,
                counters,
                pending,
            } => {
                let TimerSequence::Next(timer) = timers else {
                    return Err(crate::RestartDenial::TimerIdentityExhausted);
                };
                let current = counters.iter().find(|counter| counter.trigger == trigger);
                let attempt = current
                    .map_or(Some(1), |counter| counter.attempt.checked_add(1))
                    .ok_or(crate::RestartDenial::AttemptSequenceExhausted)?;
                let generation = current
                    .map_or(Some(0), |counter| counter.generation.checked_add(1))
                    .ok_or(crate::RestartDenial::TimerGenerationExhausted)?;
                let delay = policy
                    .delay(attempt)
                    .map_err(crate::RestartDenial::BackoffExhausted)?;
                budget.admit(at, members.len())?;

                let id = TimerId(*timer);
                let generation = TimerGeneration(generation);
                *timers = timer
                    .checked_add(1)
                    .map_or(TimerSequence::Exhausted, TimerSequence::Next);
                if let Some(counter) = counters
                    .iter_mut()
                    .find(|counter| counter.trigger == trigger)
                {
                    counter.attempt = attempt;
                    counter.generation = generation.0;
                } else {
                    counters.push(DelayCounter {
                        trigger,
                        attempt,
                        generation: generation.0,
                    });
                }
                let elapsed = TimerElapsed::new(id, generation);
                pending.push(PendingRestart { elapsed, members });
                Ok(AdmittedRestart::Delayed {
                    elapsed,
                    schedule: ScheduleAfter::new(id, generation, delay),
                })
            }
        }
    }

    fn members(&self, elapsed: TimerElapsed) -> Option<Vec<N>> {
        let Self::Delayed { pending, .. } = self else {
            return None;
        };
        pending
            .iter()
            .find(|batch| batch.elapsed == elapsed)
            .map(|batch| batch.members.clone())
    }

    fn release(&mut self, elapsed: TimerElapsed) -> bool {
        let Self::Delayed { pending, .. } = self else {
            return false;
        };
        let Some(position) = pending.iter().position(|batch| batch.elapsed == elapsed) else {
            return false;
        };
        pending.remove(position);
        true
    }

    fn remove_member(&mut self, elapsed: TimerElapsed, member: N) {
        if let Self::Delayed { pending, .. } = self
            && let Some(batch) = pending.iter_mut().find(|batch| batch.elapsed == elapsed)
        {
            batch.members.retain(|candidate| *candidate != member);
        }
    }

    fn cancel(&mut self) {
        if let Self::Delayed { pending, .. } = self {
            pending.clear();
        }
    }

    fn pending(&self) -> usize {
        match self {
            Self::Immediate { .. } => 0,
            Self::Delayed { pending, .. } => pending.len(),
        }
    }

    fn admitted(&self) -> usize {
        match self {
            Self::Immediate { budget } | Self::Delayed { budget, .. } => budget.admitted(),
        }
    }
}

/// The sole owner of fixed topology, installation, restart and drain state.
pub(crate) struct FixedFleetOwnership<A, C, Stable>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    build: fn(usize) -> Option<C>,
    strategy: crate::Strategy,
    policy: RestartPolicy,
    restart: RestartAdmission<A::Nonce>,
    slots: Vec<(A::Nonce, OwnedSlot<A, ReplacementInput<C, Stable>>)>,
    shutdown: Shutdown<A::Nonce>,
}

impl<A, C, Stable> FixedFleetOwnership<A, C, Stable>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        restart: RestartConfiguration,
    ) -> Result<Self, FleetError<A::Nonce>> {
        let mut slots = Vec::with_capacity(topology.nonces.len());
        for nonce in topology.nonces {
            if slots.iter().any(
                |(configured, _): &(A::Nonce, OwnedSlot<A, ReplacementInput<C, Stable>>)| {
                    *configured == nonce
                },
            ) {
                return Err(FleetError::DuplicateChild(nonce));
            }
            slots.push((
                nonce,
                OwnedSlot {
                    installation: Installation::Pending {
                        kind: CreationKind::Birth,
                    },
                    worker: WorkerInstallation::AwaitingInitial,
                },
            ));
        }
        Ok(Self {
            build: topology.build,
            strategy: restart.strategy,
            policy: restart.policy,
            restart: RestartAdmission::new(restart.timing, restart.maximum, restart.window),
            slots,
            shutdown: Shutdown::Running,
        })
    }

    pub fn is_restartable(&self, nonce: A::Nonce) -> Result<bool, FleetError<A::Nonce>> {
        self.worker(nonce)
            .map(WorkerInstallation::is_restartable)
            .ok_or(FleetError::UnknownChild(nonce))
    }

    pub fn is_established(&self, nonce: A::Nonce) -> Result<bool, FleetError<A::Nonce>> {
        self.slots
            .iter()
            .find_map(|(owned, slot)| {
                (*owned == nonce).then_some(matches!(
                    slot.installation,
                    Installation::Established | Installation::ShutdownRequested
                ))
            })
            .ok_or(FleetError::UnknownChild(nonce))
    }
    pub fn worker_routable(&self, nonce: A::Nonce) -> Result<bool, FleetError<A::Nonce>> {
        self.worker(nonce)
            .map(|worker| matches!(worker, WorkerInstallation::Running { .. }))
            .ok_or(FleetError::UnknownChild(nonce))
    }
    pub(crate) fn worker_incarnation(
        &self,
        nonce: A::Nonce,
    ) -> Result<Option<A::Nonce>, FleetError<A::Nonce>> {
        self.worker(nonce)
            .map(|worker| match worker {
                WorkerInstallation::Running { resolved }
                | WorkerInstallation::ReplacementIssuedToRunning { resolved } => {
                    Some(resolved.worker)
                }
                WorkerInstallation::ReplacementAccepted {
                    subject: ReplacementSubject::Running { resolved },
                    ..
                } => Some(resolved.worker),
                _ => None,
            })
            .ok_or(FleetError::UnknownChild(nonce))
    }
    pub fn child_count(&self) -> usize {
        self.slots.len()
    }
    pub fn restarts_in_window(&self) -> usize {
        self.restart.admitted()
    }
    pub fn pending_restarts(&self) -> usize {
        self.restart.pending()
    }
    pub fn is_shutting_down(&self) -> bool {
        matches!(self.shutdown, Shutdown::Draining { .. })
    }

    fn worker(
        &self,
        proxy: A::Nonce,
    ) -> Option<&WorkerInstallation<A, ReplacementInput<C, Stable>>> {
        self.slots
            .iter()
            .find_map(|(nonce, slot)| (*nonce == proxy).then_some(&slot.worker))
    }

    fn set_worker(
        &mut self,
        proxy: A::Nonce,
        next: WorkerInstallation<A, ReplacementInput<C, Stable>>,
    ) {
        if let Some((_, slot)) = self.slots.iter_mut().find(|(nonce, _)| *nonce == proxy) {
            slot.worker = next;
        }
    }

    fn take_worker(
        &mut self,
        proxy: A::Nonce,
    ) -> Option<(
        usize,
        Installation<A::Nonce>,
        WorkerInstallation<A, ReplacementInput<C, Stable>>,
    )> {
        self.slots
            .iter()
            .position(|(nonce, _)| *nonce == proxy)
            .map(|position| {
                let (_, slot) = self.slots.remove(position);
                (position, slot.installation, slot.worker)
            })
    }

    fn restore_worker(
        &mut self,
        position: usize,
        proxy: A::Nonce,
        installation: Installation<A::Nonce>,
        state: WorkerInstallation<A, ReplacementInput<C, Stable>>,
    ) {
        self.slots.insert(
            position,
            (
                proxy,
                OwnedSlot {
                    installation,
                    worker: state,
                },
            ),
        );
    }

    fn retire_worker(&mut self, proxy: A::Nonce) {
        let Some((position, installation, current)) = self.take_worker(proxy) else {
            return;
        };
        if let WorkerInstallation::ReplacementAccepted {
            gate: ReplacementGate::Timer(elapsed),
            ..
        } = &current
        {
            self.restart.remove_member(*elapsed, proxy);
        }
        self.restore_worker(
            position,
            proxy,
            installation,
            WorkerInstallation::RetiredWithoutWorker,
        );
    }

    pub fn initialize<L>(
        &mut self,
        layer: &L,
    ) -> Result<OwnedActions<A, C, Stable>, OwnershipError<A>>
    where
        L: BehaviorLayer<C, Output = Stable>,
    {
        let nonces: Vec<_> = self.slots.iter().map(|(nonce, _)| *nonce).collect();
        let children = nonces
            .iter()
            .copied()
            .enumerate()
            .map(|(index, nonce)| {
                (self.build)(index)
                    .map(|child| (nonce, child))
                    .ok_or(OwnershipError::FactoryIndex { index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sends = SupervisorSends::empty();
        sends.child_observations = InterpreterRequests::new(
            nonces
                .iter()
                .copied()
                .map(|nonce| {
                    ObserveChild::at(ChildRoute::<Stable, behavior::ChildHead>::new(nonce))
                })
                .collect(),
        );
        sends.creation_observations = InterpreterRequests::new(
            nonces
                .iter()
                .copied()
                .map(|nonce| {
                    ObserveCreation::at(ChildRoute::<Stable, behavior::ChildHead>::new(nonce))
                })
                .collect(),
        );
        Ok(Actions::new(
            sends,
            children
                .into_iter()
                .map(|(nonce, child)| {
                    ChildRoute::<Stable, behavior::ChildHead>::new(nonce).birth(layer.layer(child))
                })
                .collect(),
            Step::Continue,
        ))
    }

    pub(crate) fn prepare_worker_stopped(
        &self,
        event: WorkerStopped<A>,
    ) -> Result<Option<PreparedWorkerStop<A, C, Stable>>, OwnershipError<A>> {
        self.validate_worker_stopped(&event)?;
        if self.is_shutting_down()
            || matches!(
                self.worker(event.proxy),
                Some(
                    WorkerInstallation::ReplacementAccepted { .. }
                        | WorkerInstallation::ReplacementIssuedToRunning { .. }
                )
            )
        {
            return Ok(None);
        }
        let eligible = match self.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => {
                !matches!(&event.outcome, Ok(Exit::Normal | Exit::Collected))
            }
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            return Ok(Some(PreparedWorkerStop {
                decision: PreparedWorkerStopDecision::Retire { stopped: event },
            }));
        }
        let failed = self
            .slots
            .iter()
            .position(|(nonce, _)| *nonce == event.proxy)
            .ok_or(FleetError::UnknownChild(event.proxy))?;
        let candidates = self
            .slots
            .iter()
            .enumerate()
            .filter(|(index, (_, slot))| match self.strategy {
                crate::Strategy::OneForOne => *index == failed,
                crate::Strategy::OneForAll => slot.worker.is_restartable(),
                crate::Strategy::RestForOne => *index >= failed && slot.worker.is_restartable(),
            })
            .map(|(index, (nonce, _))| (index, *nonce))
            .filter(|(_, nonce)| {
                *nonce == event.proxy
                    || matches!(
                        self.worker(*nonce),
                        Some(
                            WorkerInstallation::AwaitingInitial
                                | WorkerInstallation::Running { .. }
                        )
                    )
            })
            .collect::<Vec<_>>();
        let replacements = candidates
            .iter()
            .map(|(index, nonce)| {
                (self.build)(*index)
                    .map(|child| (*nonce, child))
                    .ok_or((*nonce, *index))
            })
            .collect::<Result<Vec<_>, _>>();
        let replacements = match replacements {
            Ok(replacements) => replacements,
            Err((child, index)) => {
                let failure = SupervisionFailure::worker_factory_rejected(child, index);
                return Ok(Some(PreparedWorkerStop {
                    decision: PreparedWorkerStopDecision::Failed {
                        stopped: event,
                        failure,
                        restart: None,
                    },
                }));
            }
        };
        let members = candidates.iter().map(|(_, nonce)| *nonce).collect();
        let mut restart = self.restart.clone();
        let admitted = match restart.admit(event.proxy, event.at, members) {
            Ok(admitted) => admitted,
            Err(reason) => {
                let failure =
                    SupervisionFailure::restart_denied(event.proxy, event.outcome, reason);
                return Ok(Some(PreparedWorkerStop {
                    decision: PreparedWorkerStopDecision::Failed {
                        stopped: event,
                        failure,
                        restart: Some(restart),
                    },
                }));
            }
        };
        let gate = admitted.gate();
        let mut inputs = Vec::new();
        let mut updates = Vec::new();
        let mut replacing = Vec::new();
        let mut awaiting_initial = Vec::new();
        for ((nonce, child), (_, candidate)) in replacements.into_iter().zip(candidates) {
            let subject = if candidate == event.proxy {
                ReplacementSubject::Stopped {
                    stopped: event.clone(),
                }
            } else {
                match self.worker(candidate) {
                    Some(WorkerInstallation::AwaitingInitial) => {
                        awaiting_initial.push(candidate);
                        ReplacementSubject::InitialPending
                    }
                    Some(WorkerInstallation::Running { resolved }) => {
                        replacing.push(*resolved);
                        ReplacementSubject::Running {
                            resolved: *resolved,
                        }
                    }
                    _ => continue,
                }
            };
            let route = ChildRoute::<Stable, behavior::ChildHead>::new(nonce);
            let request = ChildInput::at(route, ReplacementRequested::new(child));
            let next = match (gate, subject) {
                (ReplacementGate::Open, ReplacementSubject::Stopped { stopped }) => {
                    inputs.push(request);
                    WorkerInstallation::AwaitingReplacement { stopped }
                }
                (ReplacementGate::Open, ReplacementSubject::Running { resolved }) => {
                    inputs.push(request);
                    WorkerInstallation::ReplacementIssuedToRunning { resolved }
                }
                (gate, subject) => WorkerInstallation::ReplacementAccepted {
                    subject,
                    gate,
                    request,
                },
            };
            updates.push((candidate, next));
        }
        Ok(Some(PreparedWorkerStop {
            decision: PreparedWorkerStopDecision::Accepted {
                trigger: event,
                replacing,
                awaiting_initial,
                restart,
                updates,
                inputs,
                schedule: admitted.schedule(),
            },
        }))
    }

    pub(crate) fn validate_worker_stopped(
        &self,
        event: &WorkerStopped<A>,
    ) -> Result<(), OwnershipError<A>> {
        if self.worker(event.proxy).is_none() {
            return Err(OwnershipError::UnexpectedWorkerStopped(event.clone()));
        }
        let belongs = match self.worker(event.proxy) {
            Some(WorkerInstallation::AwaitingInitial) => true,
            Some(WorkerInstallation::Running { resolved })
            | Some(WorkerInstallation::ReplacementIssuedToRunning { resolved }) => {
                resolved.worker == event.worker
            }
            Some(WorkerInstallation::ReplacementAccepted { subject, .. }) => match subject {
                ReplacementSubject::InitialPending => true,
                ReplacementSubject::Running { resolved } => resolved.worker == event.worker,
                ReplacementSubject::Stopped { .. } => false,
            },
            Some(
                WorkerInstallation::AwaitingReplacement { .. }
                | WorkerInstallation::WorkerCreationRejected { .. }
                | WorkerInstallation::RetiredAfterStop { .. }
                | WorkerInstallation::RetiredWithoutWorker,
            )
            | None => false,
        };
        if belongs {
            Ok(())
        } else {
            Err(OwnershipError::UnexpectedWorkerStopped(event.clone()))
        }
    }

    pub(crate) fn commit_worker_stopped(
        &mut self,
        prepared: PreparedWorkerStop<A, C, Stable>,
    ) -> OwnershipFold<A, C, Stable> {
        match prepared.decision {
            PreparedWorkerStopDecision::Retire { stopped } => {
                self.set_worker(
                    stopped.proxy,
                    WorkerInstallation::RetiredAfterStop { stopped },
                );
                OwnershipFold::actions(Actions::cont())
            }
            PreparedWorkerStopDecision::Accepted {
                restart,
                updates,
                inputs,
                schedule,
                ..
            } => {
                self.restart = restart;
                for (proxy, next) in updates {
                    self.set_worker(proxy, next);
                }
                let mut sends = SupervisorSends::empty();
                sends.replacement_inputs.extend(inputs);
                if let Some(schedule) = schedule {
                    sends.schedules.send(schedule);
                }
                OwnershipFold::actions(Actions::send(sends))
            }
            PreparedWorkerStopDecision::Failed {
                stopped,
                failure,
                restart,
            } => {
                if let Some(restart) = restart {
                    self.restart = restart;
                }
                self.set_worker(
                    stopped.proxy,
                    WorkerInstallation::RetiredAfterStop { stopped },
                );
                let mut sends = SupervisorSends::empty();
                sends
                    .failure_reports
                    .send(ReportSupervisionFailure::new(failure));
                OwnershipFold::failed(Actions::send(sends), failure)
            }
        }
    }

    pub fn worker_stopped(
        &mut self,
        event: WorkerStopped<A>,
    ) -> Result<OwnershipFold<A, C, Stable>, OwnershipError<A>> {
        if let Some(prepared) = self.prepare_worker_stopped(event.clone())? {
            return Ok(self.commit_worker_stopped(prepared));
        }
        if self.is_shutting_down() {
            self.set_worker(
                event.proxy,
                WorkerInstallation::RetiredAfterStop {
                    stopped: event.clone(),
                },
            );
            return Ok(OwnershipFold::actions(Actions::cont()));
        }
        if matches!(
            self.worker(event.proxy),
            Some(
                WorkerInstallation::ReplacementAccepted { .. }
                    | WorkerInstallation::ReplacementIssuedToRunning { .. }
            )
        ) {
            let Some((position, installation, current)) = self.take_worker(event.proxy) else {
                return Err(OwnershipError::UnexpectedWorkerStopped(event));
            };
            let (next, input) = match current {
                WorkerInstallation::ReplacementAccepted { gate, request, .. } => match gate {
                    ReplacementGate::Open => (
                        WorkerInstallation::AwaitingReplacement {
                            stopped: event.clone(),
                        },
                        Some(request),
                    ),
                    gate @ ReplacementGate::Timer(_) => (
                        WorkerInstallation::ReplacementAccepted {
                            subject: ReplacementSubject::Stopped {
                                stopped: event.clone(),
                            },
                            gate,
                            request,
                        },
                        None,
                    ),
                },
                WorkerInstallation::ReplacementIssuedToRunning { .. } => (
                    WorkerInstallation::AwaitingReplacement {
                        stopped: event.clone(),
                    },
                    None,
                ),
                state => {
                    self.restore_worker(position, event.proxy, installation, state);
                    return Err(OwnershipError::UnexpectedWorkerStopped(event));
                }
            };
            self.restore_worker(position, event.proxy, installation, next);
            let mut sends = SupervisorSends::empty();
            if let Some(input) = input {
                sends.replacement_inputs.push(input);
            }
            return Ok(OwnershipFold::actions(Actions::send(sends)));
        }
        Err(OwnershipError::UnexpectedWorkerStopped(event))
    }

    pub fn child_stopped(
        &mut self,
        event: ChildStopped<A>,
    ) -> Result<OwnershipFold<A, C, Stable>, OwnershipError<A>> {
        if let Shutdown::Draining { awaiting } = &mut self.shutdown {
            if awaiting.contains(&event.nonce) {
                awaiting.retain(|n| *n != event.nonce);
                let complete = awaiting.is_empty();
                if let Some((_, slot)) = self
                    .slots
                    .iter_mut()
                    .find(|(nonce, _)| *nonce == event.nonce)
                {
                    slot.installation = Installation::Stopped;
                }
                self.retire_worker(event.nonce);
                return Ok(OwnershipFold::actions(if complete {
                    Actions::stop()
                } else {
                    Actions::cont()
                }));
            }
            return Err(OwnershipError::UnexpectedChildStopped(event));
        }
        if self.worker(event.nonce).is_none() {
            return Err(OwnershipError::UnexpectedChildStopped(event));
        }
        let accepts_stop = self.slots.iter().any(|(nonce, slot)| {
            *nonce == event.nonce
                && matches!(
                    slot.installation,
                    Installation::Pending { .. } | Installation::Established
                )
        });
        if !accepts_stop {
            return Err(OwnershipError::UnexpectedChildStopped(event));
        }
        if let Some((_, slot)) = self
            .slots
            .iter_mut()
            .find(|(nonce, _)| *nonce == event.nonce)
        {
            slot.installation = Installation::Stopped;
        }
        self.retire_worker(event.nonce);
        let failure = SupervisionFailure::stable_child_stopped(event.nonce, event.outcome);
        let mut sends = SupervisorSends::empty();
        sends
            .failure_reports
            .send(ReportSupervisionFailure::new(failure));
        Ok(OwnershipFold::failed(Actions::send(sends), failure))
    }

    pub(crate) fn child_stopped_lifecycle(
        &self,
        event: &ChildStopped<A>,
    ) -> Result<Option<SupervisionLifecycle<A>>, OwnershipError<A>> {
        if let Shutdown::Draining { awaiting } = &self.shutdown {
            return if awaiting.contains(&event.nonce) {
                Ok(None)
            } else {
                Err(OwnershipError::UnexpectedChildStopped(event.clone()))
            };
        }
        if self.worker(event.nonce).is_none() {
            return Err(OwnershipError::UnexpectedChildStopped(event.clone()));
        }
        let accepts_stop = self.slots.iter().any(|(nonce, slot)| {
            *nonce == event.nonce
                && matches!(
                    slot.installation,
                    Installation::Pending { .. } | Installation::Established
                )
        });
        if !accepts_stop {
            return Err(OwnershipError::UnexpectedChildStopped(event.clone()));
        }
        Ok(Some(SupervisionLifecycle::Retired {
            failure: SupervisionFailure::stable_child_stopped(event.nonce, event.outcome),
        }))
    }

    fn validate_creation_resolved(
        &self,
        event: &CreationResolved<A>,
    ) -> Result<usize, OwnershipError<A>> {
        let Some(position) = self.slots.iter().position(|(n, _)| *n == event.nonce) else {
            return Err(OwnershipError::UnexpectedCreation(*event));
        };
        let Installation::Pending { kind: expected } = self.slots[position].1.installation else {
            return Err(OwnershipError::UnexpectedCreation(*event));
        };
        if event.kind != expected {
            return Err(OwnershipError::CreationProvenanceMismatch {
                expected,
                observed: *event,
            });
        }
        if event.result.is_err() {
            match self.worker(event.nonce) {
                Some(WorkerInstallation::Running { resolved })
                | Some(WorkerInstallation::ReplacementIssuedToRunning { resolved })
                | Some(WorkerInstallation::WorkerCreationRejected { resolved })
                | Some(WorkerInstallation::ReplacementAccepted {
                    subject: ReplacementSubject::Running { resolved },
                    ..
                }) => {
                    return Err(OwnershipError::ContradictoryStableAndWorkerCreation {
                        proxy: *event,
                        worker: *resolved,
                    });
                }
                Some(WorkerInstallation::AwaitingReplacement { stopped })
                | Some(WorkerInstallation::RetiredAfterStop { stopped })
                | Some(WorkerInstallation::ReplacementAccepted {
                    subject: ReplacementSubject::Stopped { stopped },
                    ..
                }) => {
                    return Err(OwnershipError::ContradictoryStableCreationAndWorkerStop {
                        proxy: *event,
                        worker: stopped.clone(),
                    });
                }
                _ => {}
            }
        }
        Ok(position)
    }

    pub(crate) fn creation_lifecycle(
        &self,
        event: &CreationResolved<A>,
    ) -> Result<Option<SupervisionLifecycle<A>>, OwnershipError<A>> {
        self.validate_creation_resolved(event)?;
        if self.is_shutting_down() {
            return Ok(None);
        }
        match event.result {
            Err(rejection) => {
                let failure = SupervisionFailure::stable_child_creation_rejected(
                    event.nonce,
                    event.kind,
                    rejection,
                );
                Ok(Some(SupervisionLifecycle::Retired { failure }))
            }
            Ok(_) => Ok(match self.worker(event.nonce) {
                Some(WorkerInstallation::Running { resolved }) => {
                    Some(SupervisionLifecycle::Ready {
                        proxy: event.nonce,
                        worker: resolved.worker,
                        kind: resolved.kind,
                    })
                }
                _ => None,
            }),
        }
    }

    pub fn creation_resolved(
        &mut self,
        event: CreationResolved<A>,
    ) -> Result<OwnershipFold<A, C, Stable>, OwnershipError<A>> {
        let position = self.validate_creation_resolved(&event)?;
        self.slots[position].1.installation = match event.result {
            Ok(_) => Installation::Established,
            Err(rejection) => Installation::Rejected {
                kind: event.kind,
                rejection,
            },
        };
        let failure = event.result.err().map(|rejection| {
            self.retire_worker(event.nonce);
            SupervisionFailure::stable_child_creation_rejected(event.nonce, event.kind, rejection)
        });
        let mut sends = SupervisorSends::empty();
        if let Some(failure) = failure {
            sends
                .failure_reports
                .send(ReportSupervisionFailure::new(failure));
        }
        if let Shutdown::Draining { awaiting } = &mut self.shutdown {
            if let Some(failure) = failure {
                awaiting.retain(|n| *n != event.nonce);
                let actions = if awaiting.is_empty() {
                    Actions::new(sends, Vec::new(), Step::Stop(behavior::Stopped))
                } else {
                    Actions::send(sends)
                };
                return Ok(OwnershipFold::failed(actions, failure));
            }
            self.slots[position].1.installation = Installation::ShutdownRequested;
            sends.shutdowns.send(ShutdownChild::at(
                ChildRoute::<Stable, behavior::ChildHead>::new(event.nonce),
            ));
            return Ok(OwnershipFold::actions(Actions::send(sends)));
        }
        Ok(match failure {
            Some(failure) => OwnershipFold::failed(Actions::send(sends), failure),
            None => OwnershipFold::actions(Actions::cont()),
        })
    }

    pub(crate) fn worker_creation_lifecycle(
        &self,
        event: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<Option<SupervisionLifecycle<A>>, OwnershipError<A>> {
        let expected = self.validate_worker_creation_resolved(event)?;
        if self.is_shutting_down() {
            return Ok(None);
        }
        match event.result {
            Err(rejection) => {
                let failure = SupervisionFailure::worker_creation_rejected(
                    event.proxy,
                    event.worker,
                    expected,
                    rejection,
                );
                Ok(Some(SupervisionLifecycle::Retired { failure }))
            }
            Ok(()) => {
                let established = self.slots.iter().any(|(proxy, slot)| {
                    *proxy == event.proxy && slot.installation == Installation::Established
                });
                Ok((established
                    && matches!(
                        self.worker(event.proxy),
                        Some(
                            WorkerInstallation::AwaitingInitial
                                | WorkerInstallation::AwaitingReplacement { .. }
                        )
                    ))
                .then_some(SupervisionLifecycle::Ready {
                    proxy: event.proxy,
                    worker: event.worker,
                    kind: event.kind,
                }))
            }
        }
    }

    pub fn worker_creation_resolved(
        &mut self,
        event: WorkerCreationResolved<A::Nonce>,
    ) -> Result<OwnershipFold<A, C, Stable>, OwnershipError<A>> {
        let expected = self.validate_worker_creation_resolved(&event)?;
        let Some((position, installation, current)) = self.take_worker(event.proxy) else {
            return Err(OwnershipError::UnexpectedWorkerCreation(event));
        };
        let mut sends = SupervisorSends::empty();
        let shutting_down = self.is_shutting_down();
        let next = match (current, event.result) {
            (WorkerInstallation::AwaitingInitial, Ok(()))
            | (WorkerInstallation::AwaitingReplacement { .. }, Ok(())) => {
                WorkerInstallation::Running { resolved: event }
            }
            (
                WorkerInstallation::ReplacementAccepted {
                    subject: ReplacementSubject::InitialPending,
                    gate: ReplacementGate::Open,
                    request,
                },
                Ok(()),
            ) if !shutting_down => {
                sends.replacement_inputs.push(request);
                WorkerInstallation::ReplacementIssuedToRunning { resolved: event }
            }
            (
                WorkerInstallation::ReplacementAccepted {
                    subject: ReplacementSubject::InitialPending,
                    gate,
                    request,
                },
                Ok(()),
            ) if !shutting_down => WorkerInstallation::ReplacementAccepted {
                subject: ReplacementSubject::Running { resolved: event },
                gate,
                request,
            },
            (WorkerInstallation::ReplacementAccepted { .. }, Ok(())) => {
                WorkerInstallation::Running { resolved: event }
            }
            (state, Err(rejection)) => {
                if let WorkerInstallation::ReplacementAccepted {
                    gate: ReplacementGate::Timer(elapsed),
                    ..
                } = &state
                {
                    self.restart.remove_member(*elapsed, event.proxy);
                }
                let failure = SupervisionFailure::worker_creation_rejected(
                    event.proxy,
                    event.worker,
                    expected,
                    rejection,
                );
                sends
                    .failure_reports
                    .send(ReportSupervisionFailure::new(failure));
                self.restore_worker(
                    position,
                    event.proxy,
                    installation,
                    WorkerInstallation::WorkerCreationRejected { resolved: event },
                );
                return Ok(OwnershipFold::failed(Actions::send(sends), failure));
            }
            (state, Ok(())) => {
                self.restore_worker(position, event.proxy, installation, state);
                return Err(OwnershipError::UnexpectedWorkerCreation(event));
            }
        };
        self.restore_worker(position, event.proxy, installation, next);
        Ok(OwnershipFold::actions(Actions::send(sends)))
    }

    pub fn timer_elapsed(
        &mut self,
        event: TimerElapsed,
    ) -> Result<OwnershipFold<A, C, Stable>, OwnershipError<A>> {
        let Some(members) = self.restart.members(event) else {
            return Ok(OwnershipFold::actions(Actions::cont()));
        };
        for child in &members {
            if !matches!(
                self.worker(*child),
                Some(WorkerInstallation::ReplacementAccepted {
                    gate: ReplacementGate::Timer(expected),
                    ..
                }) if *expected == event
            ) {
                return Err(OwnershipError::DelayedReplacementStateMismatch {
                    event,
                    child: *child,
                });
            }
        }
        if !self.restart.release(event) {
            return Ok(OwnershipFold::actions(Actions::cont()));
        }
        let mut sends = SupervisorSends::empty();
        for child in members {
            let Some((position, installation, current)) = self.take_worker(child) else {
                return Err(OwnershipError::DelayedReplacementStateMismatch { event, child });
            };
            let (subject, expected, request) = match current {
                WorkerInstallation::ReplacementAccepted {
                    subject,
                    gate: ReplacementGate::Timer(expected),
                    request,
                } => (subject, expected, request),
                state => {
                    self.restore_worker(position, child, installation, state);
                    return Err(OwnershipError::DelayedReplacementStateMismatch { event, child });
                }
            };
            if expected != event {
                self.restore_worker(
                    position,
                    child,
                    installation,
                    WorkerInstallation::ReplacementAccepted {
                        subject,
                        gate: ReplacementGate::Timer(expected),
                        request,
                    },
                );
                return Err(OwnershipError::DelayedReplacementStateMismatch { event, child });
            }
            let next = match subject {
                ReplacementSubject::InitialPending => WorkerInstallation::ReplacementAccepted {
                    subject,
                    gate: ReplacementGate::Open,
                    request,
                },
                ReplacementSubject::Running { resolved } => {
                    sends.replacement_inputs.push(request);
                    WorkerInstallation::ReplacementIssuedToRunning { resolved }
                }
                ReplacementSubject::Stopped { stopped } => {
                    sends.replacement_inputs.push(request);
                    WorkerInstallation::AwaitingReplacement { stopped }
                }
            };
            self.restore_worker(position, child, installation, next);
        }
        Ok(OwnershipFold::actions(Actions::send(sends)))
    }

    pub(crate) fn validate_worker_creation_resolved(
        &self,
        event: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<CreationKind<A::Nonce>, OwnershipError<A>> {
        if self.worker(event.proxy).is_none() {
            return Err(OwnershipError::UnexpectedWorkerCreation(*event));
        }
        let expected = match self.worker(event.proxy) {
            Some(WorkerInstallation::AwaitingInitial)
            | Some(WorkerInstallation::ReplacementAccepted {
                subject: ReplacementSubject::InitialPending,
                ..
            }) => CreationKind::Birth,
            Some(WorkerInstallation::AwaitingReplacement { stopped }) => {
                CreationKind::ReplacementIncarnation {
                    replaces: stopped.worker,
                }
            }
            _ => return Err(OwnershipError::UnexpectedWorkerCreation(*event)),
        };
        if event.kind != expected {
            return Err(OwnershipError::WorkerCreationProvenanceMismatch {
                expected,
                observed: *event,
            });
        }
        Ok(expected)
    }

    pub fn shutdown(&mut self) -> OwnershipFold<A, C, Stable> {
        if self.is_shutting_down() {
            return OwnershipFold::actions(Actions::cont());
        }
        self.restart.cancel();
        let awaiting = self
            .slots
            .iter()
            .filter_map(|(n, slot)| {
                matches!(
                    slot.installation,
                    Installation::Pending { .. } | Installation::Established
                )
                .then_some(*n)
            })
            .collect::<Vec<_>>();
        if awaiting.is_empty() {
            return OwnershipFold::actions(Actions::stop());
        }
        let mut sends = SupervisorSends::empty();
        for (nonce, slot) in &mut self.slots {
            if slot.installation == Installation::Established {
                slot.installation = Installation::ShutdownRequested;
                sends.shutdowns.send(ShutdownChild::at(
                    ChildRoute::<Stable, behavior::ChildHead>::new(*nonce),
                ));
            }
        }
        self.shutdown = Shutdown::Draining { awaiting };
        OwnershipFold::actions(Actions::send(sends))
    }

    pub(crate) fn shutdown_lifecycle(&self) -> Option<SupervisionLifecycle<A>> {
        if self.is_shutting_down() {
            return None;
        }
        let proxies = self
            .slots
            .iter()
            .filter_map(|(nonce, slot)| {
                matches!(
                    slot.installation,
                    Installation::Pending { .. } | Installation::Established
                )
                .then_some(*nonce)
            })
            .collect();
        Some(SupervisionLifecycle::ShuttingDown { proxies })
    }

    pub fn child_shutdown_rejected(
        &self,
        event: ChildShutdownRejected<A::Nonce>,
    ) -> Result<OwnershipFold<A, C, Stable>, OwnershipError<A>> {
        if matches!(&self.shutdown, Shutdown::Draining { awaiting } if awaiting.contains(&event.nonce))
        {
            Err(OwnershipError::ChildShutdownRejected(event))
        } else {
            Err(OwnershipError::UnexpectedChildShutdownRejection(event))
        }
    }
}
