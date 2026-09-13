use core::ops::ControlFlow;
use std::collections::BTreeMap;

use behavior::{
    Actions, Address, Behavior, BehaviorAddr, BehaviorBase, ChildCreationSettled, ChildReport,
    CreationSettlement, CreationsSettled, EndpointAddress, InjectEvent, InterpreterRequests,
    Protocol, SendEffects,
};

use crate::{DiagnosticRoute, EstablishedShutdownResolved, ShutdownRequested, StopOnShutdown};

use super::super::drain::{ForcedRetirementCause, ShutdownDeadline};
use super::super::pool::assignment::AssignmentShutdown;
use super::super::pool::worker as direct_worker;
use super::super::pool::worker::{
    RetirementReturns, RetiringWorker, RetiringWorkerActivation, RetiringWorkerInitialization,
    WorkerCustody, WorkerDeparture, WorkerPreparationError, WorkerRecoveryPreparation,
    WorkerReplacementError, WorkerStartupCustody,
};
use super::super::pool::{Assignment, CompletesAssignments};
use super::super::worker::WorkerCreationSettlement;
use super::super::{ActivationPlan, WorkerCreationRejection, WorkerSource};
use super::{
    CustomerDelivery, CustomerJob, KeyedActions, KeyedAssignedReturnReason, KeyedCustomer,
    KeyedDiagnostic, KeyedEvent, KeyedOutcome, KeyedPool, KeyedPoolState, KeyedQueuedReturnReason,
    Operating, protocol,
};

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
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, behavior::Here>,
    Key: Ord + Send,
    Job: Clone + Send,
    WorkerResult: Send,
{
    pub(super) fn begin_retirement(
        &mut self,
        operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let Operating {
            roles,
            mut bindings,
        } = operating;
        for (key, binding) in bindings.drain() {
            actions
                .sends
                .diagnostics
                .append(InterpreterRequests::one(self.diagnostics.action(
                    KeyedDiagnostic::from(protocol::KeyedDiagnosticCause::BindingRemoved {
                        key,
                        binding,
                    }),
                )));
        }

        let required = roles
            .iter()
            .filter_map(|role| role.member.shutdown_target())
            .count();
        let reserved = self.shutdowns.reserve(required);
        let mut shutdowns = reserved.map(|ids| ids.into_iter());
        let mut workers = Vec::with_capacity(roles.len());

        for role in roles {
            for (_, customer) in role.queue {
                let CustomerJob {
                    id,
                    admitted: _,
                    payload,
                    customer,
                } = customer;
                actions
                    .sends
                    .customer_outcomes
                    .append(InterpreterRequests::one(CustomerDelivery::outcome(
                        customer.route,
                        KeyedOutcome::ReturnedQueued {
                            job: id,
                            binding: customer.binding,
                            payload,
                            reason: KeyedQueuedReturnReason::PoolShutdown,
                        },
                    )));
            }

            let role_name = role.member.role_name();
            let shutdown = match (&mut shutdowns, role.member.shutdown_target()) {
                (Some(shutdowns), Some(_)) => shutdowns.next(),
                (Some(_) | None, None) | (None, Some(_)) => None,
            };
            let retiring = role.member.shutdown(shutdown);
            if let Some(request) = retiring.request {
                actions
                    .sends
                    .worker_shutdowns
                    .append(InterpreterRequests::one(request));
            }
            match retiring.custody {
                RetirementReturns::NoTransfer => {}
                RetirementReturns::Assignment(AssignmentShutdown {
                    customer,
                    worker,
                    stopped: _,
                    completion,
                }) => {
                    let KeyedCustomer { binding, route } = customer.customer;
                    actions
                        .sends
                        .customer_outcomes
                        .append(InterpreterRequests::one(CustomerDelivery::outcome(
                            route,
                            KeyedOutcome::ReturnedAssigned {
                                job: customer.id,
                                binding,
                                payload: customer.payload,
                                reason: KeyedAssignedReturnReason::PoolShutdown,
                            },
                        )));
                    if let Some(completion) = completion {
                        actions.sends.diagnostics.append(InterpreterRequests::one(
                            self.diagnostics.action(KeyedDiagnostic::from(
                                protocol::KeyedDiagnosticCause::Unexpected(
                                    KeyedEvent::WorkerCompleted(ChildReport::new(
                                        worker.creation(),
                                        completion,
                                    )),
                                ),
                            )),
                        ));
                    }
                }
                RetirementReturns::Replacement(replacement) => {
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::WorkerReplacementCancelled {
                                role: role_name,
                                replacement,
                            },
                        )),
                    ));
                }
            }
            workers.push(retiring.member);
        }

        let Some(_) = shutdowns else {
            actions.become_ = behavior::Step::Stop(behavior::Stopped);
            return (
                KeyedPoolState::ForcedRetirement {
                    workers,
                    cause: ForcedRetirementCause::WorkerShutdownIdsExhausted,
                },
                actions,
            );
        };
        let (deadline, schedule) = ShutdownDeadline::begin(self.actor_drain);
        self.continue_retirement(workers, deadline, schedule, actions)
    }

    fn retain_retired_worker(
        &self,
        worker: WorkerDeparture<Role, W, P>,
        actions: &mut KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let WorkerDeparture {
            role,
            startup,
            shutdown,
        } = worker;
        match startup {
            Some(WorkerStartupCustody::ActivationPlan(activation)) => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::UnusedActivation {
                            role: role.clone(),
                            permit: None,
                            activation,
                        },
                    )),
                ));
            }
            Some(WorkerStartupCustody::ActivationPermit { permit, activation }) => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::UnusedActivation {
                            role: role.clone(),
                            permit: Some(permit),
                            activation,
                        },
                    )),
                ));
            }
            Some(
                WorkerStartupCustody::Initializing(_)
                | WorkerStartupCustody::ActivationStart(_)
                | WorkerStartupCustody::Activation(_),
            )
            | None => {}
        }
        match shutdown {
            EstablishedShutdownResolved::Accepted { .. } => {}
            shutdown @ EstablishedShutdownResolved::Rejected { .. } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::WorkerShutdownRejected { role, shutdown },
                    )),
                ));
            }
        }
    }

    fn retain_worker(
        &mut self,
        custody: WorkerCustody<Role, W, P>,
        workers: &mut Vec<RetiringWorker<Role, W, P>>,
        actions: &mut KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> Option<ForcedRetirementCause> {
        match custody {
            WorkerCustody::Running {
                role,
                worker,
                activation,
            } => {
                let reserved = self.shutdowns.reserve(1);
                let shutdown = reserved.and_then(|mut shutdowns| shutdowns.pop());
                let exhausted = match shutdown {
                    Some(_) => None,
                    None => Some(ForcedRetirementCause::WorkerShutdownIdsExhausted),
                };
                let (worker, request) = direct_worker::retire_worker(
                    role,
                    worker,
                    Some(WorkerStartupCustody::ActivationPlan(activation)),
                    shutdown,
                );
                workers.push(worker);
                if let Some(request) = request {
                    actions
                        .sends
                        .worker_shutdowns
                        .append(InterpreterRequests::one(request));
                }
                exhausted
            }
            WorkerCustody::Returned {
                role,
                rejection,
                activation,
                stopped,
            } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::WorkerReturned {
                            role,
                            rejection,
                            activation,
                            stopped,
                        },
                    )),
                ));
                None
            }
            WorkerCustody::Stopped {
                role,
                worker,
                activation,
                stopped,
            } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::StoppedWorkerEstablished {
                            role,
                            worker,
                            activation,
                            stopped,
                        },
                    )),
                ));
                None
            }
        }
    }

    pub(super) fn accept_worker_creations(
        &mut self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        creations: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match creations.into_settlement() {
            CreationSettlement::Settled(settlements) => {
                let identities = settlements
                    .iter()
                    .map(direct_worker::creation_identity)
                    .collect::<Option<Vec<_>>>();
                let positions = identities.and_then(|identities| {
                    direct_worker::ordered_creation_positions(
                        workers.iter().map(RetiringWorker::expected_creation),
                        identities,
                    )
                });
                let Some(positions) = positions else {
                    return self.diagnose(
                        KeyedPoolState::Retiring { workers, deadline },
                        KeyedEvent::WorkerCreationsSettled(CreationsSettled::new(
                            CreationSettlement::Settled(settlements),
                        )),
                    );
                };
                let received: BTreeMap<_, _> = positions.into_iter().zip(settlements).collect();
                self.accept_correlated_creations(workers, deadline, received)
            }
            CreationSettlement::Rejected {
                creations,
                reason: _,
            } => {
                let identities = creations
                    .iter()
                    .map(|creation| (creation.id(), creation.kind()));
                let positions = direct_worker::ordered_creation_positions(
                    workers.iter().map(RetiringWorker::expected_creation),
                    identities,
                );
                let Some(positions) = positions else {
                    return self.diagnose(
                        KeyedPoolState::Retiring { workers, deadline },
                        KeyedEvent::WorkerCreationsSettled(CreationsSettled::new(
                            CreationSettlement::Rejected {
                                creations,
                                reason: behavior::ChildNamespaceExhausted,
                            },
                        )),
                    );
                };
                let returned = positions
                    .into_iter()
                    .zip(creations)
                    .map(|(position, creation)| {
                        let (id, worker, kind) = creation.into_parts();
                        (
                            position,
                            (
                                id,
                                kind,
                                WorkerCreationRejection::NamespaceExhausted {
                                    worker: worker.into_inner(),
                                },
                            ),
                        )
                    });
                self.accept_correlated_creation_rejections(workers, deadline, returned.collect())
            }
            CreationSettlement::Corrupt { creations, fault } => {
                let identities = creations
                    .iter()
                    .map(|creation| (creation.id(), creation.kind()));
                let positions = direct_worker::ordered_creation_positions(
                    workers.iter().map(RetiringWorker::expected_creation),
                    identities,
                );
                let Some(positions) = positions else {
                    return self.diagnose(
                        KeyedPoolState::Retiring { workers, deadline },
                        KeyedEvent::WorkerCreationsSettled(CreationsSettled::new(
                            CreationSettlement::Corrupt { creations, fault },
                        )),
                    );
                };
                let returned = positions
                    .into_iter()
                    .zip(creations)
                    .map(|(position, creation)| {
                        let (id, worker, kind) = creation.into_parts();
                        (
                            position,
                            (
                                id,
                                kind,
                                WorkerCreationRejection::InterpreterCorrupt {
                                    worker: worker.into_inner(),
                                    fault,
                                },
                            ),
                        )
                    });
                self.accept_correlated_creation_rejections(workers, deadline, returned.collect())
            }
        }
    }

    fn accept_correlated_creations(
        &mut self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        mut received: BTreeMap<usize, WorkerCreationSettlement<W>>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let mut remaining = Vec::with_capacity(workers.len());
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        let mut forced = None;
        for (position, worker) in workers.into_iter().enumerate() {
            let Some(settlement) = received.remove(&position) else {
                remaining.push(worker);
                continue;
            };
            match worker.accept_creation(ChildCreationSettled::new(settlement)) {
                Ok(custody) => {
                    let exhausted = self.retain_worker(custody, &mut remaining, &mut actions);
                    forced = forced.or(exhausted);
                }
                Err((worker, creation)) => {
                    remaining.push(worker);
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::Unexpected(
                                KeyedEvent::WorkerCreationsSettled(CreationsSettled::new(
                                    CreationSettlement::Settled(behavior::Creations::one(
                                        creation.into_settlement(),
                                    )),
                                )),
                            ),
                        )),
                    ));
                }
            }
        }
        match forced {
            Some(cause) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    KeyedPoolState::ForcedRetirement {
                        workers: remaining,
                        cause,
                    },
                    actions,
                )
            }
            None => self.continue_retirement(remaining, deadline, None, actions),
        }
    }

    fn accept_correlated_creation_rejections(
        &mut self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        mut returned: BTreeMap<
            usize,
            (
                behavior::CreationId,
                behavior::CreationKind,
                WorkerCreationRejection<W>,
            ),
        >,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let mut remaining = Vec::with_capacity(workers.len());
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        let mut forced = None;
        for (position, worker) in workers.into_iter().enumerate() {
            let Some((id, kind, rejection)) = returned.remove(&position) else {
                remaining.push(worker);
                continue;
            };
            match worker.accept_creation_rejection(id, kind, rejection) {
                Ok(custody) => {
                    let exhausted = self.retain_worker(custody, &mut remaining, &mut actions);
                    forced = forced.or(exhausted);
                }
                Err((worker, id, kind, rejection)) => {
                    remaining.push(worker);
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::UnmatchedWorkerReturn {
                                id,
                                kind,
                                rejection,
                            },
                        )),
                    ));
                }
            }
        }
        match forced {
            Some(cause) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    KeyedPoolState::ForcedRetirement {
                        workers: remaining,
                        cause,
                    },
                    actions,
                )
            }
            None => self.continue_retirement(remaining, deadline, None, actions),
        }
    }

    fn continue_retirement(
        &self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        schedule: Option<crate::ScheduleAfter>,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        if let Some(schedule) = schedule {
            actions.sends.restart_schedules.send(schedule);
        }
        match deadline {
            ShutdownDeadline::NotScheduled(settlement) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    KeyedPoolState::ForcedRetirement {
                        workers,
                        cause: ForcedRetirementCause::DeadlineNotScheduled(settlement),
                    },
                    actions,
                )
            }
            ShutdownDeadline::Elapsed(elapsed) => {
                actions.become_ = behavior::Step::Stop(behavior::Stopped);
                (
                    KeyedPoolState::ForcedRetirement {
                        workers,
                        cause: ForcedRetirementCause::DeadlineElapsed(elapsed),
                    },
                    actions,
                )
            }
            deadline @ (ShutdownDeadline::Unlimited
            | ShutdownDeadline::Scheduling(_)
            | ShutdownDeadline::Waiting(_)) => {
                match workers.iter().find_map(RetiringWorker::waiting) {
                    Some(_) => (KeyedPoolState::Retiring { workers, deadline }, actions),
                    None => {
                        actions.become_ = behavior::Step::Stop(behavior::Stopped);
                        (KeyedPoolState::Stopped, actions)
                    }
                }
            }
        }
    }

    pub(super) fn accept_worker_exit(
        &self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_exit(workers, stopped) {
            Ok(ControlFlow::Continue(workers)) => (
                KeyedPoolState::Retiring { workers, deadline },
                Actions::cont(),
            ),
            Ok(ControlFlow::Break((workers, worker))) => {
                let mut actions = Actions::cont();
                self.retain_retired_worker(worker, &mut actions);
                self.continue_retirement(workers, deadline, None, actions)
            }
            Err((workers, stopped)) => self.diagnose(
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerStopped(stopped),
            ),
        }
    }

    pub(super) fn accept_worker_shutdown(
        &self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        settlement: EstablishedShutdownResolved<W::Protocol>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_shutdown(workers, settlement) {
            Ok(ControlFlow::Continue(workers)) => (
                KeyedPoolState::Retiring { workers, deadline },
                Actions::cont(),
            ),
            Ok(ControlFlow::Break((workers, worker))) => {
                let mut actions = Actions::cont();
                self.retain_retired_worker(worker, &mut actions);
                self.continue_retirement(workers, deadline, None, actions)
            }
            Err((workers, settlement)) => self.diagnose(
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerShutdownSettled(settlement),
            ),
        }
    }

    pub(super) fn accept_worker_initialization(
        &self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        input: super::super::WorkerInitializationReport<W, P>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_initialization(workers, input) {
            RetiringWorkerInitialization::Returned { workers, report } => self.diagnose(
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerInitialization(report),
            ),
            RetiringWorkerInitialization::WorkerStopped {
                workers,
                departure,
                role,
                activation,
            } => {
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
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::UnusedActivation {
                            role,
                            permit: None,
                            activation,
                        },
                    )),
                ));
                if let Some(departure) = departure {
                    self.retain_retired_worker(departure, &mut actions);
                }
                self.continue_retirement(workers, deadline, None, actions)
            }
        }
    }

    pub(super) fn accept_worker_activation(
        &self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        input: super::super::WorkerActivation<W, P>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match direct_worker::accept_worker_activation(workers, input) {
            RetiringWorkerActivation::Started { workers } => {
                self.continue_retirement(workers, deadline, None, Actions::cont())
            }
            RetiringWorkerActivation::Returned {
                workers,
                role,
                worker,
                activation,
                outcome,
            } => {
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
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::WorkerActivationReturned {
                            role,
                            worker,
                            activation,
                            outcome,
                        },
                    )),
                ));
                self.continue_retirement(workers, deadline, None, actions)
            }
            RetiringWorkerActivation::Unrelated {
                workers,
                activation,
            } => self.diagnose(
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::WorkerActivationReported(activation),
            ),
        }
    }

    pub(super) fn accept_retired_preparation(
        &mut self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        deadline: ShutdownDeadline,
        input: behavior::ActionItemResult<super::super::PrepareWorkers<Source, Role, W, P>>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let (workers, returned) =
            match direct_worker::accept_retiring_worker_preparation(workers, input) {
                Ok(returned) => returned,
                Err((workers, input)) => {
                    return self.diagnose(
                        KeyedPoolState::Retiring { workers, deadline },
                        KeyedEvent::WorkerPreparationSettled(input),
                    );
                }
            };
        let diagnostic = match returned {
            WorkerRecoveryPreparation::Ready(direct_worker::PreparedReplacement {
                role,
                recoveries: _,
                previous,
                stopped,
                source,
                submission,
            }) => {
                let (returned_source, error) = match self.recovery.restore_source(source) {
                    Ok(()) => (None, WorkerReplacementError::PoolShutdown),
                    Err(source) => (Some(source), WorkerReplacementError::SourceStateCorrupt),
                };
                protocol::KeyedDiagnosticCause::WorkerReplacementFailed {
                    role,
                    previous,
                    stopped,
                    submission,
                    returned_source,
                    error,
                }
            }
            WorkerRecoveryPreparation::Failed {
                role,
                recoveries: _,
                previous,
                stopped,
                source,
                error,
            } => {
                let (returned_source, error) = match self.recovery.restore_source(source) {
                    Ok(()) => (None, error),
                    Err(source) => (Some(source), WorkerPreparationError::SourceStateCorrupt),
                };
                protocol::KeyedDiagnosticCause::WorkerPreparationFailed {
                    role,
                    previous,
                    stopped,
                    returned_source,
                    error,
                }
            }
        };
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions.sends.diagnostics.append(InterpreterRequests::one(
            self.diagnostics.action(KeyedDiagnostic::from(diagnostic)),
        ));
        self.continue_retirement(workers, deadline, None, actions)
    }

    pub(super) fn accept_retirement_schedule(
        &self,
        mut workers: Vec<RetiringWorker<Role, W, P>>,
        mut deadline: ShutdownDeadline,
        settlement: behavior::ActionItemResult<crate::ScheduleAfter>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let replacement = workers.iter().position(|worker| match &worker.state {
            direct_worker::RetirementStatus::AwaitingRestartSchedule { timer, .. } => {
                timer.accepts_result(&settlement)
            }
            direct_worker::RetirementStatus::AwaitingCreation(_)
            | direct_worker::RetirementStatus::Established { .. }
            | direct_worker::RetirementStatus::AwaitingPreparation { .. }
            | direct_worker::RetirementStatus::Drained => false,
        });
        if let Some(position) = replacement {
            let RetiringWorker { role, state } = workers.remove(position);
            let direct_worker::RetirementStatus::AwaitingRestartSchedule {
                replacement,
                timer: _,
            } = state
            else {
                workers.insert(position, RetiringWorker { role, state });
                return self.diagnose(
                    KeyedPoolState::Retiring { workers, deadline },
                    KeyedEvent::RestartScheduleSettled(settlement),
                );
            };
            let direct_worker::PendingWorkerReplacement {
                previous,
                stopped,
                submission,
            } = replacement;
            workers.insert(
                position,
                RetiringWorker {
                    role: role.clone(),
                    state: direct_worker::RetirementStatus::Drained,
                },
            );
            let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
                Actions::cont();
            actions
                .sends
                .diagnostics
                .append(InterpreterRequests::one(self.diagnostics.action(
                    KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::WorkerReplacementFailed {
                            role,
                            previous,
                            stopped,
                            submission,
                            returned_source: None,
                            error: WorkerReplacementError::RestartScheduleReturned(settlement),
                        },
                    ),
                )));
            return self.continue_retirement(workers, deadline, None, actions);
        }
        match deadline.accept_schedule(settlement) {
            Ok(()) => self.continue_retirement(workers, deadline, None, Actions::cont()),
            Err(settlement) => self.diagnose(
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::RestartScheduleSettled(settlement),
            ),
        }
    }

    pub(super) fn accept_deadline(
        &self,
        workers: Vec<RetiringWorker<Role, W, P>>,
        mut deadline: ShutdownDeadline,
        elapsed: crate::TimerElapsed,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match deadline.accept_elapsed(elapsed) {
            Ok(()) => self.continue_retirement(workers, deadline, None, Actions::cont()),
            Err(elapsed) => self.diagnose(
                KeyedPoolState::Retiring { workers, deadline },
                KeyedEvent::RestartElapsed(elapsed),
            ),
        }
    }
}
