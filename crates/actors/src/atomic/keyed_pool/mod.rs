//! Direct workers with per-role queues and generation-exact affinity.

use std::collections::BTreeMap;
use std::mem;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, Births,
    ChildCreationOutcome, ChildCreationSettled, ChildReport, CreateChild, CreationKind,
    CreationSequence, CreationSettlement, Creations, CreationsSettled, EndpointAddress,
    EstablishedCreation, Here, InitializationTurn, InjectEvent, InterpreterRequests,
    ItemSettlement, MessageProtocol, Never, Protocol, SendEffects, SettledItem, SourceActions,
    User,
};
use thiserror::Error;

use crate::{
    DeliveryRoute, DiagnosticAction, DiagnosticDisposition, DiagnosticRoute, ObserveChild,
    ReplyRoute, ScheduleAfter, ShutdownEstablished, ShutdownRequested, StopOnShutdown,
};

use super::pool::assignment::{AcceptedJobSequence, AssignedJob, AssignmentSequence, CustomerJob};
use super::pool::worker as direct_worker;
use super::pool::worker::{Member, MemberState, RetiringWorker, Worker, WorkerPhase};
use super::pool::{
    AssignWorker, Assignment, BacklogCapacity, CompletesAssignments, CustomerDelivery,
    Interruption, PoolRecovery, PoolRecoveryState, ShutdownSequence, SubmissionId,
};
use super::worker::{InitialWorkerRejection, prepare_initial_workers};
use super::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, BeginActivation, InitializeWorker,
    OrderedRoles, PrepareWorkers, PreparedWorker, WorkerActivation, WorkerAttempt,
    WorkerInitializationReport, WorkerSource, WorkerSubmission,
};

mod assignment;
mod binding;
mod event;
mod job;
mod protocol;
mod recovery;
mod requests;
mod role;
mod shutdown;

pub use binding::{
    BindingCapacity, BindingEvidence, BindingExpectation, BindingGeneration, BindingRequestId,
};
pub use event::KeyedEvent;
pub use protocol::{
    BindingCommand, BindingRejection, BindingReply, KeyedAdmissionRejection,
    KeyedAssignedReturnReason, KeyedCommand, KeyedDiagnostic, KeyedOutcome,
    KeyedQueuedReturnReason,
};
pub use requests::KeyedRequests;

use binding::{BindingReservationRejected, BindingTable, ReservedBinding};
use job::KeyedCustomer;
use protocol::{BindingChange, KeyedRequest};
use role::{AdmissionTarget, ManagementTarget, RoleCell};

type CustomerRoute<A, Key, Role, Job, WorkerResult> =
    ReplyRoute<MessageProtocol<A, KeyedOutcome<Key, Role, Job, WorkerResult>>>;

type KeyedQueue<Role, W, Key, Job, WorkerResult> = BTreeMap<
    super::pool::assignment::AdmissionOrdinal,
    CustomerJob<
        Job,
        KeyedCustomer<Role, CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>>,
    >,
>;

type ManagementRoute<A, Key, Role> = ReplyRoute<MessageProtocol<A, BindingReply<A, Key, Role>>>;

type KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> = Actions<
    BehaviorAddr<W>,
    Never,
    KeyedRequests<
        InterpreterRequests<ObserveChild<<W as Behavior>::Protocol, behavior::ChildHead>>,
        InterpreterRequests<InitializeWorker<W, P>>,
        InterpreterRequests<BeginActivation<W, P>>,
        InterpreterRequests<
            CustomerDelivery<
                MessageProtocol<BehaviorAddr<W>, KeyedOutcome<Key, Role, Job, WorkerResult>>,
            >,
        >,
        <ManagementRoute<BehaviorAddr<W>, Key, Role> as DeliveryRoute>::Sends,
        SourceActions<AssignWorker<<W as Behavior>::Protocol, Job>>,
        SourceActions<PrepareWorkers<Source, Role, W, P>>,
        SourceActions<ScheduleAfter>,
        InterpreterRequests<ShutdownEstablished<StopOnShutdown<W>, Here>>,
        InterpreterRequests<
            DiagnosticAction<
                Diagnostics,
                KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>,
            >,
        >,
    >,
    Births<StopOnShutdown<W>>,
>;

type KeyedRoleCell<Role, W, P, Key, Job, WorkerResult> = RoleCell<
    Role,
    W,
    P,
    Job,
    WorkerResult,
    CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>,
>;

struct Operating<Role, W, P, Key, Job, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    roles: Vec<KeyedRoleCell<Role, W, P, Key, Job, WorkerResult>>,
    bindings: BindingTable<Key, Role>,
}

impl<Role, W, P, Key, Job, WorkerResult> Operating<Role, W, P, Key, Job, WorkerResult>
where
    Role: Eq,
    W: Behavior + BehaviorBase,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn role_position(&self, role: &Role) -> Option<usize> {
        self.roles.iter().position(|cell| cell.role() == role)
    }

    fn binding_expectation(&self, key: &Key) -> BindingExpectation
    where
        Key: Ord,
    {
        match self.bindings.binding(key) {
            Some(evidence) => BindingExpectation::Exact(evidence.generation().clone()),
            None => BindingExpectation::Absent,
        }
    }

    fn creation_positions(
        &self,
        identities: impl IntoIterator<Item = (behavior::CreationId, CreationKind)>,
    ) -> Option<Vec<usize>> {
        direct_worker::ordered_creation_positions(
            self.roles
                .iter()
                .map(|cell| cell.member.expected_creation()),
            identities,
        )
    }

    fn worker_position(&self, worker: &WorkerAttempt) -> Option<usize> {
        self.roles
            .iter()
            .position(|cell| cell.member.worker_attempt() == Some(worker))
    }
}

enum AdmissionBinding<'table, Key, Role> {
    Retained {
        submitted_key: Key,
        evidence: BindingEvidence<Role>,
    },
    Reserved(ReservedBinding<'table, Key, Role>),
}

impl<Key, Role> AdmissionBinding<'_, Key, Role> {
    fn commit(self) -> BindingEvidence<Role> {
        match self {
            Self::Retained {
                submitted_key: _,
                evidence,
            } => evidence,
            Self::Reserved(binding) => binding.commit(),
        }
    }

    fn reject(self) -> Key {
        match self {
            Self::Retained { submitted_key, .. } => submitted_key,
            Self::Reserved(binding) => binding.reject(),
        }
    }
}

enum KeyedPoolState<Role, W, P, Key, Job, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Constructed(Vec<PreparedWorker<Role, W, P>>),
    Operating(Operating<Role, W, P, Key, Job, WorkerResult>),
    Retiring {
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: super::drain::ShutdownDeadline,
    },
    Stopped,
    ForcedRetirement {
        #[expect(
            dead_code,
            reason = "Bombay's retirement custodian receives every unresolved worker"
        )]
        workers: Vec<RetiringWorker<Role, W, P>>,
        #[expect(
            dead_code,
            reason = "Bombay's retirement custodian receives the exact retirement reason"
        )]
        cause: super::drain::ForcedRetirementCause,
    },
}

impl<Role, W, P, Key, Job, WorkerResult> KeyedPoolState<Role, W, P, Key, Job, WorkerResult>
where
    Key: Ord,
    W: Behavior + BehaviorBase,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
{
    fn start(
        self,
        creations: &mut CreationSequence,
        backlog: BacklogCapacity,
        binding_capacity: BindingCapacity,
    ) -> Result<
        (
            Self,
            Vec<ObserveChild<W::Protocol, behavior::ChildHead>>,
            Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
        ),
        (Self, KeyedError),
    > {
        let prepared = match self {
            Self::Constructed(prepared) => prepared,
            state => return Err((state, KeyedError::InitializationUnavailable)),
        };
        let mut ids = Vec::with_capacity(prepared.len());
        for _ in 0..prepared.len() {
            let Some(id) = creations.issue() else {
                return Err((
                    Self::Constructed(prepared),
                    KeyedError::WorkerCreationsExhausted,
                ));
            };
            ids.push(id);
        }

        let mut observations = Vec::with_capacity(prepared.len());
        let mut workers = Creations::empty();
        let roles = prepared
            .into_iter()
            .zip(ids)
            .map(|(prepared, creation)| {
                let (member, worker, observation) = Member::begin(prepared, creation);
                workers.extend([worker]);
                observations.push(observation);
                RoleCell::new(member, backlog)
            })
            .collect();

        Ok((
            Self::Operating(Operating {
                roles,
                bindings: BindingTable::new(binding_capacity),
            }),
            observations,
            workers,
        ))
    }
}

/// Controlled failure while starting or transitioning one keyed pool.
#[doc(hidden)]
#[derive(Debug, Error)]
pub enum KeyedError {
    /// Initialization was invoked after the prepared roster had already advanced.
    #[error("keyed pool initialization is no longer available")]
    InitializationUnavailable,
    /// The pool could not reserve an identifier for every initial worker.
    #[error("keyed pool worker creation identifiers are exhausted")]
    WorkerCreationsExhausted,
}

/// A direct-worker pool with one bounded queue per semantic role.
pub struct KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    state: KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
    selector: Selector,
    activation: ActivationPolicy,
    recovery: PoolRecoveryState<Source>,
    restarts: super::restart::RestartBudget,
    backlog: BacklogCapacity,
    binding_capacity: BindingCapacity,
    interruption: Interruption,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<Diagnostics>,
    creations: CreationSequence,
    jobs: AcceptedJobSequence,
    assignments: AssignmentSequence,
    shutdowns: ShutdownSequence,
    next_restart_timer: u64,
}

/// Complete custody when initial keyed-worker preparation rejects.
pub struct KeyedConstructionRejected<Factory, Selector, Role, W, P, Rejection, Source, Diagnostics>
{
    /// Complete initial-worker rejection.
    pub workers: InitialWorkerRejection<Factory, Role, W, P, Rejection>,
    /// Complete key selector.
    pub selector: Selector,
    /// Complete activation policy.
    pub activation: ActivationPolicy,
    /// Complete recovery policy.
    pub recovery: PoolRecovery<Source>,
    /// Complete per-role waiting-job capacity.
    pub backlog: BacklogCapacity,
    /// Complete retained-binding capacity.
    pub binding_capacity: BindingCapacity,
    /// Complete interruption policy.
    pub interruption: Interruption,
    /// Complete actor-graph drain policy.
    pub actor_drain: ActorDrainPolicy,
    /// Complete diagnostic disposition.
    pub diagnostics: DiagnosticDisposition<Diagnostics>,
}

/// Construct one keyed pool after preparing every declared worker in order.
#[expect(
    clippy::too_many_arguments,
    reason = "every argument is one explicit KeyedPool policy pending the recorded builder comparison"
)]
pub fn keyed<
    Factory,
    Selector,
    Role,
    W,
    P,
    Rejection,
    Source,
    Diagnostics,
    Key,
    Job,
    WorkerResult,
>(
    factory: Factory,
    roles: OrderedRoles<Role>,
    selector: Selector,
    activation: ActivationPolicy,
    recovery: PoolRecovery<Source>,
    backlog: BacklogCapacity,
    binding_capacity: BindingCapacity,
    interruption: Interruption,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<Diagnostics>,
) -> Result<
    KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>,
    KeyedConstructionRejected<Factory, Selector, Role, W, P, Rejection, Source, Diagnostics>,
>
where
    Factory: FnMut(&Role) -> core::result::Result<WorkerSubmission<W, P>, Rejection>,
    Selector: Fn(&Key) -> Role,
    Role: Eq,
    W: Behavior,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    BehaviorAddr<W>: EndpointAddress,
{
    let prepared = match prepare_initial_workers(factory, roles) {
        Ok(prepared) => prepared,
        Err(workers) => {
            return Err(KeyedConstructionRejected {
                workers,
                selector,
                activation,
                recovery,
                backlog,
                binding_capacity,
                interruption,
                actor_drain,
                diagnostics,
            });
        }
    };
    Ok(KeyedPool {
        state: KeyedPoolState::Constructed(prepared),
        selector,
        activation,
        recovery: recovery.into(),
        restarts: super::restart::RestartBudget::empty(),
        backlog,
        binding_capacity,
        interruption,
        actor_drain,
        diagnostics,
        creations: CreationSequence::new(),
        jobs: AcceptedJobSequence::new(),
        assignments: AssignmentSequence::new(),
        shutdowns: ShutdownSequence::new(),
        next_restart_timer: 1,
    })
}

impl<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult> BehaviorBase
    for KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
where
    Role: Send + Sync,
    W: Behavior + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    BehaviorAddr<W>: EndpointAddress,
    Job: Send,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
    KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
where
    Role: Eq + Send + Sync,
    W: Behavior + BehaviorBase + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    Selector: Fn(&Key) -> Role,
    Diagnostics:
        DiagnosticRoute<KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>> + Clone,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Send,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    Key: Ord + Send,
    Job: Clone + Send,
    WorkerResult: Send,
{
    fn reject_submission(
        &self,
        operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        submission: SubmissionId,
        key: Key,
        payload: Job,
        customer: CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>,
        reason: KeyedAdmissionRejection,
    ) -> (
        Operating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions.sends.customer_outcomes = InterpreterRequests::one(CustomerDelivery::rejected(
            customer,
            KeyedOutcome::Rejected {
                submission,
                key,
                payload,
                reason,
            },
        ));
        (operating, actions)
    }

    fn accept_submission(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        submission: SubmissionId,
        key: Key,
        payload: Job,
        customer: CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>,
    ) -> (
        Operating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let (position, proposed) = match operating.bindings.binding(&key) {
            Some(evidence) => {
                let evidence = evidence.clone();
                let Some(position) = operating.role_position(evidence.role()) else {
                    return self.reject_submission(
                        operating,
                        submission,
                        key,
                        payload,
                        customer,
                        KeyedAdmissionRejection::RoleUnavailable,
                    );
                };
                (
                    position,
                    AdmissionBinding::Retained {
                        submitted_key: key,
                        evidence,
                    },
                )
            }
            None => {
                let selected = (self.selector)(&key);
                let Some(position) = operating.role_position(&selected) else {
                    return self.reject_submission(
                        operating,
                        submission,
                        key,
                        payload,
                        customer,
                        KeyedAdmissionRejection::UnknownSelectedRole,
                    );
                };
                let role = operating.roles[position].member.role_name();
                let proposed = match operating.bindings.reserve(key, role.clone()) {
                    Ok(binding) => AdmissionBinding::Reserved(binding),
                    Err(BindingReservationRejected::CapacityExhausted(key)) => {
                        return self.reject_submission(
                            operating,
                            submission,
                            key,
                            payload,
                            customer,
                            KeyedAdmissionRejection::BindingCapacityExhausted,
                        );
                    }
                    Err(BindingReservationRejected::GenerationsExhausted(key)) => {
                        return self.reject_submission(
                            operating,
                            submission,
                            key,
                            payload,
                            customer,
                            KeyedAdmissionRejection::BindingGenerationExhausted,
                        );
                    }
                    Err(BindingReservationRejected::AlreadyBound(key)) => {
                        return self.reject_submission(
                            operating,
                            submission,
                            key,
                            payload,
                            customer,
                            KeyedAdmissionRejection::RoleUnavailable,
                        );
                    }
                };
                (position, proposed)
            }
        };

        let target = operating.roles[position].admission_target();
        let rejection = match target {
            AdmissionTarget::QueueFull => Some(KeyedAdmissionRejection::RoleBacklogFull),
            AdmissionTarget::Unavailable => Some(KeyedAdmissionRejection::RoleUnavailable),
            AdmissionTarget::Assign | AdmissionTarget::Queue => None,
        };
        if let Some(reason) = rejection {
            let key = proposed.reject();
            return self.reject_submission(operating, submission, key, payload, customer, reason);
        }

        let Some((job, admitted)) = self.jobs.issue() else {
            let key = proposed.reject();
            return self.reject_submission(
                operating,
                submission,
                key,
                payload,
                customer,
                KeyedAdmissionRejection::JobCorrelationExhausted,
            );
        };
        let customer_target = customer.clone();

        match target {
            AdmissionTarget::Assign => {
                let member = &mut operating.roles[position].member;
                let super::pool::worker::MemberState::Worker(Worker { current, phase }) =
                    &mut member.state
                else {
                    let key = proposed.reject();
                    return self.reject_submission(
                        operating,
                        submission,
                        key,
                        payload,
                        customer,
                        KeyedAdmissionRejection::RoleUnavailable,
                    );
                };
                let WorkerPhase::Idle = phase else {
                    let key = proposed.reject();
                    return self.reject_submission(
                        operating,
                        submission,
                        key,
                        payload,
                        customer,
                        KeyedAdmissionRejection::RoleUnavailable,
                    );
                };
                let Some((correlation, assignment)) =
                    self.assignments.assign(&current.attempt, payload.clone())
                else {
                    let key = proposed.reject();
                    return self.reject_submission(
                        operating,
                        submission,
                        key,
                        payload,
                        customer,
                        KeyedAdmissionRejection::AssignmentCorrelationExhausted,
                    );
                };
                let request = AssignWorker::new(current.recipient(), &correlation, assignment);
                let binding = proposed.commit();
                let obligation = CustomerJob {
                    id: job,
                    admitted,
                    payload,
                    customer: KeyedCustomer {
                        binding: binding.clone(),
                        route: customer,
                    },
                };
                *phase = WorkerPhase::Busy(AssignedJob::new(obligation, correlation));
                let mut actions: KeyedActions<
                    Role,
                    W,
                    P,
                    Source,
                    Diagnostics,
                    Key,
                    Job,
                    WorkerResult,
                > = Actions::cont();
                actions.sends.customer_outcomes =
                    InterpreterRequests::one(CustomerDelivery::outcome(
                        customer_target,
                        KeyedOutcome::Accepted {
                            submission,
                            job,
                            binding,
                        },
                    ));
                actions.sends.worker_assignments.send(request);
                (operating, actions)
            }
            AdmissionTarget::Queue => {
                let binding = proposed.commit();
                let obligation = CustomerJob {
                    id: job,
                    admitted,
                    payload,
                    customer: KeyedCustomer {
                        binding: binding.clone(),
                        route: customer,
                    },
                };
                operating.roles[position].queue.insert(admitted, obligation);
                let mut actions: KeyedActions<
                    Role,
                    W,
                    P,
                    Source,
                    Diagnostics,
                    Key,
                    Job,
                    WorkerResult,
                > = Actions::cont();
                actions.sends.customer_outcomes =
                    InterpreterRequests::one(CustomerDelivery::outcome(
                        customer_target,
                        KeyedOutcome::Accepted {
                            submission,
                            job,
                            binding,
                        },
                    ));
                (operating, actions)
            }
            AdmissionTarget::QueueFull | AdmissionTarget::Unavailable => {
                let key = proposed.reject();
                self.reject_submission(
                    operating,
                    submission,
                    key,
                    payload,
                    customer,
                    KeyedAdmissionRejection::RoleUnavailable,
                )
            }
        }
    }

    fn fill_role(
        &mut self,
        operating: &mut Operating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        actions: &mut KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let Some(cell) = operating.roles.get_mut(position) else {
            return;
        };
        let MemberState::Worker(Worker {
            current,
            phase: phase @ WorkerPhase::Idle,
        }) = &mut cell.member.state
        else {
            return;
        };
        let Some((_, customer)) = cell.queue.pop_first() else {
            return;
        };
        let Some((correlation, assignment)) = self
            .assignments
            .assign(&current.attempt, customer.payload.clone())
        else {
            let CustomerJob {
                id,
                admitted: _,
                payload,
                customer: KeyedCustomer { binding, route },
            } = customer;
            actions
                .sends
                .customer_outcomes
                .append(InterpreterRequests::one(CustomerDelivery::outcome(
                    route,
                    KeyedOutcome::ReturnedQueued {
                        job: id,
                        binding,
                        payload,
                        reason: KeyedQueuedReturnReason::AssignmentCorrelationExhausted,
                    },
                )));
            return;
        };
        let request = AssignWorker::new(current.recipient(), &correlation, assignment);
        *phase = WorkerPhase::Busy(AssignedJob::new(customer, correlation));
        actions.sends.worker_assignments.send(request);
    }

    fn authorize_waiting(
        &mut self,
        operating: &mut Operating<Role, W, P, Key, Job, WorkerResult>,
        actions: &mut KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let occupied = operating
            .roles
            .iter()
            .filter_map(|cell| match &cell.member.state {
                MemberState::Worker(Worker {
                    phase:
                        WorkerPhase::ActivationDispatched { .. }
                        | WorkerPhase::Activating { .. },
                    ..
                }) => Some(()),
                MemberState::Creating(_)
                | MemberState::Worker(_)
                | MemberState::Recovering(_)
                | MemberState::Retired => None,
            })
            .count();
        let available = self.activation.maximum().saturating_sub(occupied);
        for _ in 0..available {
            let position = operating
                .roles
                .iter()
                .enumerate()
                .find_map(|(position, cell)| match &cell.member.state {
                    MemberState::Worker(Worker {
                        phase: WorkerPhase::WaitingForActivation { .. },
                        ..
                    }) => Some(position),
                    MemberState::Creating(_)
                    | MemberState::Worker(_)
                    | MemberState::Recovering(_)
                    | MemberState::Retired => None,
                });
            let Some(position) = position else {
                return;
            };
            let RoleCell {
                member,
                queue,
                capacity,
            } = operating.roles.remove(position);
            match member.authorize_activation() {
                Ok((member, request)) => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member,
                            queue,
                            capacity,
                        },
                    );
                    actions
                        .sends
                        .worker_activations
                        .append(InterpreterRequests::one(request));
                }
                Err(member) => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member,
                            queue,
                            capacity,
                        },
                    );
                    return;
                }
            }
        }
    }

    fn accept_creations(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> Result<
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
        ),
    > {
        let settlements = match workers.into_settlement() {
            CreationSettlement::Settled(settlements) => settlements,
            settlement => {
                return Err((operating, CreationsSettled::new(settlement)));
            }
        };
        let identities = settlements
            .iter()
            .map(|settlement| match settlement {
                SettledItem::Attempted(ItemSettlement::Accepted(
                    ChildCreationOutcome::Established {
                        established: EstablishedCreation::Installed { id, kind, .. },
                    },
                )) => Some((*id, *kind)),
                SettledItem::Attempted(
                    ItemSettlement::Accepted(
                        ChildCreationOutcome::Established {
                            established: EstablishedCreation::Rejected { .. },
                        }
                        | ChildCreationOutcome::InitializationRejected { .. }
                        | ChildCreationOutcome::HostRejected { .. },
                    )
                    | ItemSettlement::Rejected { .. }
                    | ItemSettlement::Corrupt { .. }
                    | ItemSettlement::Blocked { .. },
                )
                | SettledItem::Unattempted(_) => None,
            })
            .collect::<Option<Vec<_>>>();
        let Some(positions) =
            identities.and_then(|identities| operating.creation_positions(identities))
        else {
            return Err((
                operating,
                CreationsSettled::new(CreationSettlement::Settled(settlements)),
            ));
        };
        let mut returned: BTreeMap<_, _> = positions.into_iter().zip(settlements).collect();
        let mut roles = Vec::with_capacity(operating.roles.len());
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        for (position, cell) in operating.roles.into_iter().enumerate() {
            let Some(settlement) = returned.remove(&position) else {
                roles.push(cell);
                continue;
            };
            let RoleCell {
                member,
                queue,
                capacity,
            } = cell;
            match member.admit_successful_creation(ChildCreationSettled::new(settlement)) {
                Ok((member, request)) => {
                    roles.push(RoleCell {
                        member,
                        queue,
                        capacity,
                    });
                    actions
                        .sends
                        .worker_initializations
                        .append(InterpreterRequests::one(request));
                }
                Err((member, creation)) => {
                    roles.push(RoleCell {
                        member,
                        queue,
                        capacity,
                    });
                    let returned = CreationsSettled::new(CreationSettlement::Settled(
                        Creations::one(creation.into_settlement()),
                    ));
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::Unexpected(
                                KeyedEvent::WorkerCreationsSettled(returned),
                            ),
                        )),
                    ));
                }
            }
        }
        operating.roles = roles;
        Ok((operating, actions))
    }

    fn accept_initialization(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            WorkerInitializationReport<W, P>,
        ),
    > {
        let Some(position) = operating.worker_position(input.worker()) else {
            return Err((operating, input));
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        match member.admit_initialization(input) {
            Ok(member) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member,
                        queue,
                        capacity,
                    },
                );
                let mut actions = Actions::cont();
                self.authorize_waiting(&mut operating, &mut actions);
                Ok((operating, actions))
            }
            Err((member, input)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member,
                        queue,
                        capacity,
                    },
                );
                Err((operating, input))
            }
        }
    }

    fn accept_activation(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        input: WorkerActivation<W, P>,
    ) -> Result<
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            WorkerActivation<W, P>,
        ),
    > {
        let Some(position) = operating.worker_position(&input.worker()) else {
            return Err((operating, input));
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        match member.admit_activation(input) {
            Ok(member) => {
                let mut actions = Actions::cont();
                match &member.state {
                    MemberState::Worker(Worker {
                        phase: WorkerPhase::Idle,
                        ..
                    }) => {
                        operating.roles.insert(
                            position,
                            RoleCell {
                                member,
                                queue,
                                capacity,
                            },
                        );
                        self.fill_role(&mut operating, position, &mut actions);
                    }
                    MemberState::Creating(_)
                    | MemberState::Worker(_)
                    | MemberState::Recovering(_)
                    | MemberState::Retired => {
                        operating.roles.insert(
                            position,
                            RoleCell {
                                member,
                                queue,
                                capacity,
                            },
                        );
                    }
                }
                self.authorize_waiting(&mut operating, &mut actions);
                Ok((operating, actions))
            }
            Err((member, input)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member,
                        queue,
                        capacity,
                    },
                );
                Err((operating, input))
            }
        }
    }

    fn reject_binding(
        &self,
        operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        command: BindingCommand<BehaviorAddr<W>, Key, Role>,
        reason: BindingRejection,
    ) -> (
        Operating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let target = command.reply().clone();
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions.sends.binding_replies = target.deliver(BindingReply::Rejected { command, reason });
        (operating, actions)
    }

    fn accept_binding(
        &self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        command: BindingCommand<BehaviorAddr<W>, Key, Role>,
    ) -> (
        Operating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let actual = operating.binding_expectation(command.key());
        if command.expectation() != &actual {
            return self.reject_binding(
                operating,
                command,
                BindingRejection::StaleExpectation { actual },
            );
        }

        match command.change() {
            BindingChange::Rebalance(target) => {
                let Some(position) = operating.role_position(target) else {
                    return self.reject_binding(
                        operating,
                        command,
                        BindingRejection::UnknownTarget,
                    );
                };
                if let ManagementTarget::Unavailable = operating.roles[position].management_target()
                {
                    return self.reject_binding(
                        operating,
                        command,
                        BindingRejection::TargetUnavailable,
                    );
                }
                let target = operating.roles[position].member.role_name();
                match actual {
                    BindingExpectation::Absent => {
                        let (request, key, expected, change, reply) = command.into_parts();
                        match operating.bindings.reserve(key, target) {
                            Ok(binding) => {
                                let current = binding.commit();
                                let mut actions: KeyedActions<
                                    Role,
                                    W,
                                    P,
                                    Source,
                                    Diagnostics,
                                    Key,
                                    Job,
                                    WorkerResult,
                                > = Actions::cont();
                                actions.sends.binding_replies =
                                    reply.deliver(BindingReply::Bound { request, current });
                                (operating, actions)
                            }
                            Err(rejection) => {
                                let (key, reason) = match rejection {
                                    BindingReservationRejected::AlreadyBound(key) => {
                                        let actual = operating.binding_expectation(&key);
                                        (key, BindingRejection::StaleExpectation { actual })
                                    }
                                    BindingReservationRejected::CapacityExhausted(key) => {
                                        (key, BindingRejection::BindingCapacityExhausted)
                                    }
                                    BindingReservationRejected::GenerationsExhausted(key) => {
                                        (key, BindingRejection::GenerationExhausted)
                                    }
                                };
                                let command = BindingCommand::from_parts(
                                    request, key, expected, change, reply,
                                );
                                self.reject_binding(operating, command, reason)
                            }
                        }
                    }
                    BindingExpectation::Exact(_) => {
                        let Some(current) = operating.bindings.binding(command.key()).cloned()
                        else {
                            return self.reject_binding(
                                operating,
                                command,
                                BindingRejection::StaleExpectation {
                                    actual: BindingExpectation::Absent,
                                },
                            );
                        };
                        if current.role() == target.role() {
                            let (request, _, _, _, reply) = command.into_parts();
                            let mut actions: KeyedActions<
                                Role,
                                W,
                                P,
                                Source,
                                Diagnostics,
                                Key,
                                Job,
                                WorkerResult,
                            > = Actions::cont();
                            actions.sends.binding_replies =
                                reply.deliver(BindingReply::Unchanged { request, current });
                            return (operating, actions);
                        }
                        let Some((prior, current)) = operating
                            .bindings
                            .occupied(command.key())
                            .and_then(|binding| binding.rebind(target))
                        else {
                            return self.reject_binding(
                                operating,
                                command,
                                BindingRejection::GenerationExhausted,
                            );
                        };
                        let (request, _, _, _, reply) = command.into_parts();
                        let mut actions: KeyedActions<
                            Role,
                            W,
                            P,
                            Source,
                            Diagnostics,
                            Key,
                            Job,
                            WorkerResult,
                        > = Actions::cont();
                        actions.sends.binding_replies = reply.deliver(BindingReply::Rebalanced {
                            request,
                            prior,
                            current,
                        });
                        (operating, actions)
                    }
                }
            }
            BindingChange::Unbind => match actual {
                BindingExpectation::Absent => {
                    let target = command.reply().clone();
                    let mut actions: KeyedActions<
                        Role,
                        W,
                        P,
                        Source,
                        Diagnostics,
                        Key,
                        Job,
                        WorkerResult,
                    > = Actions::cont();
                    actions.sends.binding_replies =
                        target.deliver(BindingReply::AlreadyUnbound { command });
                    (operating, actions)
                }
                BindingExpectation::Exact(_) => {
                    let Some((key, removed)) = operating.bindings.remove(command.key()) else {
                        return self.reject_binding(
                            operating,
                            command,
                            BindingRejection::StaleExpectation {
                                actual: BindingExpectation::Absent,
                            },
                        );
                    };
                    let (request, _, _, _, reply) = command.into_parts();
                    let mut actions: KeyedActions<
                        Role,
                        W,
                        P,
                        Source,
                        Diagnostics,
                        Key,
                        Job,
                        WorkerResult,
                    > = Actions::cont();
                    actions.sends.binding_replies = reply.deliver(BindingReply::Unbound {
                        request,
                        key,
                        removed,
                    });
                    (operating, actions)
                }
            },
        }
    }

    fn diagnose(
        &self,
        state: KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        input: KeyedEvent<
            Role,
            W,
            P,
            Key,
            Job,
            WorkerResult,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        >,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions.sends.diagnostics = InterpreterRequests::one(self.diagnostics.action(
            KeyedDiagnostic::from(protocol::KeyedDiagnosticCause::Unexpected(input)),
        ));
        (state, actions)
    }
}

impl<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult> Behavior
    for KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
where
    Role: Eq + Send + Sync,
    W: Behavior + BehaviorBase + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
    Selector: Fn(&Key) -> Role + Send,
    Diagnostics:
        DiagnosticRoute<KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>> + Clone,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Send,
    <BehaviorAddr<W> as EndpointAddress>::Established<W::Protocol>: Send,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    Key: Ord + Send,
    Job: Clone + Send,
    WorkerResult: Send,
{
    type Protocol = MessageProtocol<
        BehaviorAddr<W>,
        KeyedCommand<BehaviorAddr<W>, Key, Role, Job, WorkerResult>,
    >;
    type Event = KeyedEvent<
        Role,
        W,
        P,
        Key,
        Job,
        WorkerResult,
        behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    >;
    type Sends = KeyedRequests<
        InterpreterRequests<ObserveChild<W::Protocol, behavior::ChildHead>>,
        InterpreterRequests<InitializeWorker<W, P>>,
        InterpreterRequests<BeginActivation<W, P>>,
        InterpreterRequests<
            CustomerDelivery<
                MessageProtocol<BehaviorAddr<W>, KeyedOutcome<Key, Role, Job, WorkerResult>>,
            >,
        >,
        <ManagementRoute<BehaviorAddr<W>, Key, Role> as DeliveryRoute>::Sends,
        SourceActions<AssignWorker<W::Protocol, Job>>,
        SourceActions<PrepareWorkers<Source, Role, W, P>>,
        SourceActions<ScheduleAfter>,
        InterpreterRequests<ShutdownEstablished<StopOnShutdown<W>, Here>>,
        InterpreterRequests<
            DiagnosticAction<
                Diagnostics,
                KeyedDiagnostic<Role, W, P, Source, Key, Job, WorkerResult>,
            >,
        >,
    >;
    type Ph = Never;
    type Error = KeyedError;
    type Birth = Births<StopOnShutdown<W>>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        let state = mem::replace(&mut self.state, KeyedPoolState::Stopped);
        match state.start(&mut self.creations, self.backlog, self.binding_capacity) {
            Ok((state, observations, workers)) => {
                self.state = state;
                let mut sends = KeyedRequests::empty();
                sends.worker_observations = InterpreterRequests::new(observations);
                Ok(Actions::new(sends, workers, behavior::Step::Continue))
            }
            Err((state, error)) => {
                self.state = state;
                Err(error)
            }
        }
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        let state = mem::replace(&mut self.state, KeyedPoolState::Stopped);
        let (state, actions) = match (state, input) {
            (state, KeyedEvent::Command(User { from, message })) => {
                match (state, message.into_request()) {
                    (
                        KeyedPoolState::Operating(operating),
                        KeyedRequest::Submit {
                            submission,
                            key,
                            payload,
                            customer,
                        },
                    ) => {
                        let (operating, actions) =
                            self.accept_submission(operating, submission, key, payload, customer);
                        (KeyedPoolState::Operating(operating), actions)
                    }
                    (KeyedPoolState::Operating(operating), KeyedRequest::Binding(command)) => {
                        let (operating, actions) = self.accept_binding(operating, command);
                        (KeyedPoolState::Operating(operating), actions)
                    }
                    (KeyedPoolState::Operating(operating), KeyedRequest::Shutdown) => {
                        self.begin_retirement(operating, Actions::cont())
                    }
                    (state, request) => self.diagnose(
                        state,
                        KeyedEvent::Command(User::new(from, KeyedCommand::from_request(request))),
                    ),
                }
            }
            (KeyedPoolState::Operating(operating), KeyedEvent::WorkerCreationsSettled(workers)) => {
                match self.accept_creations(operating, workers) {
                    Ok((operating, actions)) => (KeyedPoolState::Operating(operating), actions),
                    Err((operating, workers)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::WorkerCreationsSettled(workers),
                    ),
                }
            }
            (KeyedPoolState::Operating(operating), KeyedEvent::WorkerInitialization(input)) => {
                match self.accept_initialization(operating, input) {
                    Ok((operating, actions)) => (KeyedPoolState::Operating(operating), actions),
                    Err((operating, input)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::WorkerInitialization(input),
                    ),
                }
            }
            (KeyedPoolState::Operating(operating), KeyedEvent::WorkerActivationReported(input)) => {
                match self.accept_activation(operating, input) {
                    Ok((operating, actions)) => (KeyedPoolState::Operating(operating), actions),
                    Err((operating, input)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::WorkerActivationReported(input),
                    ),
                }
            }
            (
                KeyedPoolState::Operating(operating),
                KeyedEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Accepted(
                    receipt,
                ))),
            ) => match self.accept_assignment_receipt(operating, receipt) {
                Ok(result) => result,
                Err((operating, receipt)) => self.diagnose(
                    KeyedPoolState::Operating(operating),
                    KeyedEvent::AssignmentSettled(SettledItem::Attempted(
                        ItemSettlement::Accepted(receipt),
                    )),
                ),
            },
            (
                KeyedPoolState::Operating(operating),
                KeyedEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason,
                })),
            ) => self.reject_assignment_delivery(operating, item, reason),
            (
                KeyedPoolState::Operating(operating),
                KeyedEvent::WorkerCompleted(ChildReport { child, report }),
            ) => match self.accept_completion(operating, child, report) {
                Ok(result) => result,
                Err((operating, completion)) => self.diagnose(
                    KeyedPoolState::Operating(operating),
                    KeyedEvent::WorkerCompleted(completion),
                ),
            },
            (KeyedPoolState::Operating(operating), KeyedEvent::WorkerStopped(stopped)) => {
                match self.accept_worker_stop(operating, stopped) {
                    Ok(result) => result,
                    Err((operating, stopped)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::WorkerStopped(stopped),
                    ),
                }
            }
            (
                KeyedPoolState::Operating(operating),
                KeyedEvent::WorkerShutdownSettled(settlement),
            ) => match self.accept_quarantine_shutdown(operating, settlement) {
                Ok(result) => result,
                Err((operating, settlement)) => self.diagnose(
                    KeyedPoolState::Operating(operating),
                    KeyedEvent::WorkerShutdownSettled(settlement),
                ),
            },
            (KeyedPoolState::Operating(operating), KeyedEvent::WorkerPreparationSettled(input)) => {
                match self.accept_worker_preparation(operating, input) {
                    Ok(result) => result,
                    Err((operating, input)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::WorkerPreparationSettled(input),
                    ),
                }
            }
            (KeyedPoolState::Operating(operating), KeyedEvent::RestartScheduleSettled(input)) => {
                match self.accept_restart_schedule(operating, input) {
                    Ok(result) => result,
                    Err((operating, input)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::RestartScheduleSettled(input),
                    ),
                }
            }
            (KeyedPoolState::Operating(operating), KeyedEvent::RestartElapsed(elapsed)) => {
                match self.accept_restart_timer(operating, elapsed) {
                    Ok(result) => result,
                    Err((operating, elapsed)) => self.diagnose(
                        KeyedPoolState::Operating(operating),
                        KeyedEvent::RestartElapsed(elapsed),
                    ),
                }
            }
            (KeyedPoolState::Operating(operating), KeyedEvent::Shutdown(_)) => {
                self.begin_retirement(operating, Actions::cont())
            }
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerCreationsSettled(creations),
            ) => self.accept_worker_creations(workers, deadline, creations),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerStopped(stopped),
            ) => self.accept_worker_exit(workers, deadline, stopped),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerShutdownSettled(settlement),
            ) => self.accept_worker_shutdown(workers, deadline, settlement),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerInitialization(input),
            ) => self.accept_worker_initialization(workers, deadline, input),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerActivationReported(input),
            ) => self.accept_worker_activation(workers, deadline, input),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerPreparationSettled(input),
            ) => self.accept_retired_preparation(workers, deadline, input),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::RestartScheduleSettled(settlement),
            ) => self.accept_retirement_schedule(workers, deadline, settlement),
            (
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::RestartElapsed(elapsed),
            ) => self.accept_deadline(workers, deadline, elapsed),
            (state, input) => self.diagnose(state, input),
        };
        self.state = state;
        Ok(actions)
    }
}
