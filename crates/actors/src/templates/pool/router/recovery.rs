//! Keyed-role recovery and direct-worker replacement.

use core::{num::NonZeroUsize, ops::ControlFlow};
use std::collections::BTreeMap;
use std::mem;

use behavior::{
    Actions, Address, Behavior, BehaviorAddr, BehaviorBase, ChildCreationSettled, CreateChild,
    CreationSettlement, Creations, CreationsSettled, EndpointAddress, InjectEvent,
    InterpreterRequests, Protocol, SendEffects,
};

use crate::{DiagnosticRoute, ScheduleAfter, ShutdownRequested, StopOnShutdown};

use super::job::KeyedCustomer;
use super::protocol;
use super::role::RoleCell;
use super::{
    CustomerDelivery, CustomerRoute, KeyedActions, KeyedAssignedReturnReason, KeyedDiagnostic,
    KeyedEvent, KeyedOperating, KeyedOutcome, KeyedPool, KeyedPoolState, KeyedQueue,
    KeyedQueuedReturnReason,
};
use crate::templates::pool::assignment::CustomerJob;
use crate::templates::pool::worker as direct_worker;
use crate::templates::pool::worker::{
    CreationBatchRejection, Member, MemberState, PreparedReplacement, WorkerCreationAdmission,
    WorkerPreparationError, WorkerPreparationStartFault, WorkerRecoveryPreparation,
    WorkerReplacementError, WorkerReplacementRelease,
};
use crate::templates::pool::{
    Assignment, CompletesAssignments, Interruption, PoolFailureReaction, PoolRecoveryState,
    WorkerRecoveryDecision,
};
use crate::templates::restart::{
    RecoveryCount, RecoveryRelease, RestartAdmission, RestartBudget, admit_restart,
};
use crate::templates::roster::RoleName;
use crate::templates::schedule::ScheduleKey;
use crate::templates::worker::{WorkerPreparationExpectation, stop_kind};
use crate::worker::{PrepareWorkers, WorkerPreparation, WorkerSource};
use crate::worker::{PreparedWorker, StartupPlan, WorkerAttempt, WorkerSubmission};

impl<Role, W, P, Key, Job, WorkerResult> KeyedOperating<Role, W, P, Key, Job, WorkerResult>
where
    Role: Eq,
    W: Behavior + BehaviorBase,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn preparation_start_position<Source>(
        &self,
        input: &behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Option<usize>
    where
        Role: Send + Sync,
        W: Send,
        Source: WorkerSource<Role, W, P>,
    {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Recovering(direct_worker::RecoveringWorker::Preparing {
                preparation: expected,
                ..
            }) => expected.accepts_issued(input),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn preparation_return_position<Source>(
        &self,
        returned: &WorkerPreparation<Source, Role, W, P>,
    ) -> Option<usize>
    where
        Role: Send + Sync,
        W: Send,
        Source: WorkerSource<Role, W, P>,
    {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Recovering(direct_worker::RecoveringWorker::Preparing {
                preparation: expected,
                ..
            }) => expected.accepts_return(returned),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn waiting_recovery_position(&self) -> Option<usize> {
        self.roles.iter().position(|cell| {
            matches!(
                &cell.member.state,
                MemberState::Recovering(direct_worker::RecoveringWorker::WaitingForSource { .. })
            )
        })
    }

    fn restart_schedule_position(
        &self,
        input: &behavior::ActionItemResult<ScheduleAfter>,
    ) -> Option<usize> {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Recovering(recovering) => recovering.accepts_restart_schedule(input),
            MemberState::Creating(_) | MemberState::Worker(_) | MemberState::Retired => false,
        })
    }

    fn restart_timer_position(&self, elapsed: &crate::TimerElapsed) -> Option<usize> {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Recovering(recovering) => recovering.accepts_restart_timer(elapsed),
            MemberState::Creating(_) | MemberState::Worker(_) | MemberState::Retired => false,
        })
    }
}

impl<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
    KeyedPool<Role, W, P, Source, Selector, Diagnostics, Key, Job, WorkerResult>
where
    Role: Eq + Send + Sync,
    W: Behavior + BehaviorBase + Send,
    W::Protocol: Protocol<Msg = Assignment<Job>>,
    W::Sends: CompletesAssignments<WorkerResult = WorkerResult>,
    P: StartupPlan,
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
    pub(super) fn accept_creations(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
            CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
        ),
    > {
        let settlements = match workers.into_settlement() {
            CreationSettlement::Settled(settlements) => settlements,
            CreationSettlement::Rejected {
                creations,
                reason: _,
            } => {
                return self.reject_creation_batch(
                    operating,
                    creations,
                    CreationBatchRejection::NamespaceExhausted,
                );
            }
            CreationSettlement::Corrupt { creations, fault } => {
                return self.reject_creation_batch(
                    operating,
                    creations,
                    CreationBatchRejection::InterpreterCorrupt(fault),
                );
            }
        };
        let identities = settlements
            .iter()
            .map(direct_worker::creation_identity)
            .collect::<Option<Vec<_>>>();
        let Some(positions) =
            identities.and_then(|identities| operating.creation_positions(identities))
        else {
            return Err((
                operating,
                CreationsSettled::new(CreationSettlement::Settled(settlements)),
            ));
        };
        let mut received: BTreeMap<_, _> = positions.into_iter().zip(settlements).collect();
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        while let Some((position, settlement)) = received.pop_first() {
            let RoleCell {
                member,
                queue,
                capacity,
            } = operating.roles.remove(position);
            match member.admit_creation(ChildCreationSettled::new(settlement)) {
                WorkerCreationAdmission::Initializing { member, request } => {
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
                        .worker_initializations
                        .append(InterpreterRequests::one(request));
                }
                WorkerCreationAdmission::Rejected {
                    member,
                    role,
                    rejection,
                    startup,
                    stopped,
                } => {
                    let Member { recoveries, .. } = member;
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::WorkerCreationRejected {
                                role: role.clone(),
                                rejection,
                                startup,
                                stopped,
                            },
                        )),
                    ));
                    let (state, mut recovered) = self.retire_unrecovered_worker(
                        operating, position, role, recoveries, queue, capacity, actions,
                    );
                    match state {
                        KeyedPoolState::Operating(resumed) => {
                            operating = resumed;
                            actions = recovered;
                        }
                        state if received.is_empty() => return Ok((state, recovered)),
                        KeyedPoolState::Retiring { workers, deadline } => {
                            let remaining = CreationsSettled::new(CreationSettlement::Settled(
                                received.into_values().collect::<Creations<_>>(),
                            ));
                            let (state, settled) =
                                self.accept_worker_creations(workers, deadline, remaining);
                            recovered.sends.append(settled.sends);
                            recovered.creates.extend(settled.creates);
                            recovered.become_ = settled.become_;
                            return Ok((state, recovered));
                        }
                        state => {
                            let remaining = CreationsSettled::new(CreationSettlement::Settled(
                                received.into_values().collect::<Creations<_>>(),
                            ));
                            let (state, returned) =
                                self.diagnose(state, KeyedEvent::WorkerCreationsSettled(remaining));
                            recovered.sends.append(returned.sends);
                            return Ok((state, recovered));
                        }
                    }
                }
                WorkerCreationAdmission::Unrelated { member, creation } => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member,
                            queue,
                            capacity,
                        },
                    );
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
        Ok((KeyedPoolState::Operating(operating), actions))
    }

    fn reject_creation_batch(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        creations: Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
        rejection: CreationBatchRejection,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
            CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
        ),
    > {
        let Some(positions) = operating.creation_positions(
            creations
                .iter()
                .map(|creation| (creation.id(), creation.kind())),
        ) else {
            return Err((operating, rejection.settlement(creations)));
        };
        let mut received: BTreeMap<_, _> = positions.into_iter().zip(creations).collect();
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        while let Some((position, creation)) = received.pop_first() {
            let RoleCell {
                member:
                    Member {
                        role,
                        recoveries,
                        state,
                    },
                queue,
                capacity,
            } = operating.roles.remove(position);
            match state {
                MemberState::Creating(starting) => {
                    let (_, worker, _) = creation.into_parts();
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::WorkerCreationRejected {
                                role: role.clone(),
                                rejection: rejection.worker(worker.into_inner()),
                                startup: starting.startup,
                                stopped: starting.stopped,
                            },
                        )),
                    ));
                    let (state, mut recovered) = self.retire_unrecovered_worker(
                        operating, position, role, recoveries, queue, capacity, actions,
                    );
                    match state {
                        KeyedPoolState::Operating(resumed) => {
                            operating = resumed;
                            actions = recovered;
                        }
                        state if received.is_empty() => return Ok((state, recovered)),
                        KeyedPoolState::Retiring { workers, deadline } => {
                            let remaining = rejection
                                .settlement(received.into_values().collect::<Creations<_>>());
                            let (state, settled) =
                                self.accept_worker_creations(workers, deadline, remaining);
                            recovered.sends.append(settled.sends);
                            recovered.creates.extend(settled.creates);
                            recovered.become_ = settled.become_;
                            return Ok((state, recovered));
                        }
                        state => {
                            let remaining = rejection
                                .settlement(received.into_values().collect::<Creations<_>>());
                            let (state, returned) =
                                self.diagnose(state, KeyedEvent::WorkerCreationsSettled(remaining));
                            recovered.sends.append(returned.sends);
                            return Ok((state, recovered));
                        }
                    }
                }
                state => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member: Member {
                                role,
                                recoveries,
                                state,
                            },
                            queue,
                            capacity,
                        },
                    );
                    actions.sends.diagnostics.append(InterpreterRequests::one(
                        self.diagnostics.action(KeyedDiagnostic::from(
                            protocol::KeyedDiagnosticCause::Unexpected(
                                KeyedEvent::WorkerCreationsSettled(
                                    rejection.settlement(Creations::one(creation)),
                                ),
                            ),
                        )),
                    ));
                }
            }
        }
        Ok((KeyedPoolState::Operating(operating), actions))
    }

    fn retire_role(
        &self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        for (_, customer) in queue {
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
                        reason: KeyedQueuedReturnReason::RolePermanentlyUnavailable,
                    },
                )));
        }
        for (key, binding) in operating.bindings.retire_role(role.role()) {
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
        operating.roles.insert(
            position,
            RoleCell {
                member: Member {
                    role,
                    recoveries,
                    state: MemberState::Retired,
                },
                queue: BTreeMap::new(),
                capacity,
            },
        );
        (operating, actions)
    }

    fn request_preparation(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        source: Source,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        RoleCell<
            Role,
            W,
            P,
            Job,
            WorkerResult,
            CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>,
        >,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let (ticket, request) = PrepareWorkers::new(source, role.clone(), Vec::new());
        actions.sends.worker_preparations.send(request);
        (
            RoleCell {
                member: Member {
                    role,
                    recoveries,
                    state: MemberState::Recovering(direct_worker::RecoveringWorker::Preparing {
                        previous,
                        stopped,
                        preparation: WorkerPreparationExpectation::issued(ticket),
                    }),
                },
                queue,
                capacity,
            },
            actions,
        )
    }

    fn prepare_next_waiting(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let Some(position) = operating.waiting_recovery_position() else {
            return (operating, actions);
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        let Member {
            role,
            recoveries,
            state:
                MemberState::Recovering(direct_worker::RecoveringWorker::WaitingForSource {
                    previous,
                    stopped,
                }),
        } = member
        else {
            operating.roles.insert(
                position,
                RoleCell {
                    member,
                    queue,
                    capacity,
                },
            );
            return (operating, actions);
        };
        let Some(source) = self.recovery.claim_waiting_source() else {
            operating.roles.insert(
                position,
                RoleCell {
                    member: Member {
                        role,
                        recoveries,
                        state: MemberState::Recovering(
                            direct_worker::RecoveringWorker::WaitingForSource { previous, stopped },
                        ),
                    },
                    queue,
                    capacity,
                },
            );
            return (operating, actions);
        };
        let (role, actions) = Self::request_preparation(
            role, recoveries, previous, stopped, source, queue, capacity, actions,
        );
        operating.roles.insert(position, role);
        (operating, actions)
    }

    fn retire_unrecovered_worker(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
        actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match self.recovery.failure() {
            PoolFailureReaction::RetireRole => {
                let (operating, actions) = self.retire_role(
                    operating, position, role, recoveries, queue, capacity, actions,
                );
                let (operating, actions) = self.prepare_next_waiting(operating, actions);
                (KeyedPoolState::Operating(operating), actions)
            }
            PoolFailureReaction::StopPool => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Retired,
                        },
                        queue,
                        capacity,
                    },
                );
                self.begin_retirement(operating, actions)
            }
        }
    }

    fn reject_preparation(
        &mut self,
        operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        source: Source,
        error: WorkerPreparationError<Source::WorkerRejection, Source::SourceRejection>,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let (returned_source, error) = match self.recovery.restore_source(source) {
            Ok(()) => (None, error),
            Err(source) => (Some(source), WorkerPreparationError::SourceStateCorrupt),
        };
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions
            .sends
            .diagnostics
            .append(InterpreterRequests::one(self.diagnostics.action(
                KeyedDiagnostic::from(protocol::KeyedDiagnosticCause::WorkerPreparationFailed {
                    role: role.clone(),
                    previous,
                    stopped,
                    returned_source,
                    error,
                }),
            )));
        self.retire_unrecovered_worker(
            operating, position, role, recoveries, queue, capacity, actions,
        )
    }

    fn reject_replacement(
        &mut self,
        operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        submission: WorkerSubmission<W, P>,
        returned_source: Option<Source>,
        error: WorkerReplacementError,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        actions
            .sends
            .diagnostics
            .append(InterpreterRequests::one(self.diagnostics.action(
                KeyedDiagnostic::from(protocol::KeyedDiagnosticCause::WorkerReplacementFailed {
                    role: role.clone(),
                    previous,
                    stopped,
                    submission,
                    returned_source,
                    error,
                }),
            )));
        self.retire_unrecovered_worker(
            operating, position, role, recoveries, queue, capacity, actions,
        )
    }

    fn return_source_after_replacement_failure(
        &mut self,
        operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        source: Source,
        submission: WorkerSubmission<W, P>,
        error: WorkerReplacementError,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let (returned_source, error) = match self.recovery.restore_source(source) {
            Ok(()) => (None, error),
            Err(source) => (Some(source), WorkerReplacementError::SourceStateCorrupt),
        };
        self.reject_replacement(
            operating,
            position,
            role,
            recoveries,
            previous,
            stopped,
            submission,
            returned_source,
            error,
            queue,
            capacity,
            Actions::cont(),
        )
    }

    pub(super) fn recover_worker(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        interrupted: Option<
            CustomerJob<
                Job,
                KeyedCustomer<Role, CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>>,
            >,
        >,
        mut queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: crate::pool::BacklogCapacity,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        match self.recovery.decide(stop_kind(&stopped.outcome)) {
            WorkerRecoveryDecision::PrepareWorker(source) => {
                if let Some(customer) = interrupted {
                    match self.interruption {
                        Interruption::Fail => Self::return_assigned(
                            customer,
                            KeyedAssignedReturnReason::WorkerStopped,
                            &mut actions,
                        ),
                        Interruption::Retry => {
                            queue.insert(customer.admitted, customer);
                        }
                    }
                }
                let (role, actions) = Self::request_preparation(
                    role, recoveries, previous, stopped, source, queue, capacity, actions,
                );
                operating.roles.insert(position, role);
                (KeyedPoolState::Operating(operating), actions)
            }
            WorkerRecoveryDecision::WaitForSource => {
                if let Some(customer) = interrupted {
                    match self.interruption {
                        Interruption::Fail => Self::return_assigned(
                            customer,
                            KeyedAssignedReturnReason::WorkerStopped,
                            &mut actions,
                        ),
                        Interruption::Retry => {
                            queue.insert(customer.admitted, customer);
                        }
                    }
                }
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Recovering(
                                direct_worker::RecoveringWorker::WaitingForSource {
                                    previous,
                                    stopped,
                                },
                            ),
                        },
                        queue,
                        capacity,
                    },
                );
                (KeyedPoolState::Operating(operating), actions)
            }
            WorkerRecoveryDecision::RetireRole => {
                if let Some(customer) = interrupted {
                    let reason = match self.interruption {
                        Interruption::Fail => KeyedAssignedReturnReason::WorkerStopped,
                        Interruption::Retry => {
                            KeyedAssignedReturnReason::RolePermanentlyUnavailable
                        }
                    };
                    Self::return_assigned(customer, reason, &mut actions);
                }
                let (operating, actions) = self.retire_role(
                    operating, position, role, recoveries, queue, capacity, actions,
                );
                (KeyedPoolState::Operating(operating), actions)
            }
            WorkerRecoveryDecision::StopPool => {
                if let Some(customer) = interrupted {
                    let reason = match self.interruption {
                        Interruption::Fail => KeyedAssignedReturnReason::WorkerStopped,
                        Interruption::Retry => KeyedAssignedReturnReason::PoolShutdown,
                    };
                    Self::return_assigned(customer, reason, &mut actions);
                }
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Retired,
                        },
                        queue,
                        capacity,
                    },
                );
                self.begin_retirement(operating, actions)
            }
        }
    }

    pub(super) fn accept_worker_preparation_start(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        ),
    > {
        let Some(position) = operating.preparation_start_position(&input) else {
            return Err((operating, input));
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        match member.accept_preparation_start(input) {
            Ok(ControlFlow::Continue(member)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member,
                        queue,
                        capacity,
                    },
                );
                Ok((KeyedPoolState::Operating(operating), Actions::cont()))
            }
            Ok(ControlFlow::Break(failed)) => {
                let error = match failed.fault {
                    WorkerPreparationStartFault::InterpreterCorrupt(fault) => {
                        WorkerPreparationError::InterpreterCorrupt(fault)
                    }
                    WorkerPreparationStartFault::InterpretationSkipped => {
                        WorkerPreparationError::InterpretationSkipped
                    }
                };
                Ok(self.reject_preparation(
                    operating,
                    position,
                    failed.role,
                    failed.recoveries,
                    failed.previous,
                    failed.stopped,
                    failed.source,
                    error,
                    queue,
                    capacity,
                ))
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

    pub(super) fn accept_worker_preparation_return(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        input: WorkerPreparation<Source, Role, W, P>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
            WorkerPreparation<Source, Role, W, P>,
        ),
    > {
        let (limit, release) = match &self.recovery {
            PoolRecoveryState::Permanent { limit, release, .. }
            | PoolRecoveryState::Transient { limit, release, .. } => (*limit, *release),
            PoolRecoveryState::Temporary { .. } => return Err((operating, input)),
        };
        let Some(position) = operating.preparation_return_position(&input) else {
            return Err((operating, input));
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        let prepared = match member.accept_preparation_return(input) {
            Ok(WorkerRecoveryPreparation::Ready(prepared)) => prepared,
            Ok(WorkerRecoveryPreparation::Failed {
                role,
                recoveries,
                previous,
                stopped,
                source,
                error,
            }) => {
                return Ok(self.reject_preparation(
                    operating, position, role, recoveries, previous, stopped, source, error, queue,
                    capacity,
                ));
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
                return Err((operating, input));
            }
        };
        let PreparedReplacement {
            role,
            mut recoveries,
            previous,
            stopped,
            source,
            submission,
        } = prepared;
        let budget = mem::replace(&mut self.restarts, RestartBudget::empty());
        match admit_restart(
            &mut recoveries,
            budget,
            limit,
            release,
            stopped.at,
            NonZeroUsize::MIN,
        ) {
            RestartAdmission::Proposed(proposal) => {
                let release = match proposal.release() {
                    RecoveryRelease::Immediate => match self.creations.issue() {
                        Some(creation) => Ok(WorkerReplacementRelease::Now(creation)),
                        None => Err(WorkerReplacementError::WorkerCreationsExhausted),
                    },
                    RecoveryRelease::Delayed(delay) => match self.issue_restart_timer() {
                        Some(timer) => Ok(WorkerReplacementRelease::After { timer, delay }),
                        None => Err(WorkerReplacementError::RestartTimersExhausted),
                    },
                };
                let release = match release {
                    Ok(release) => release,
                    Err(error) => {
                        self.restarts = proposal.decline();
                        return Ok(self.return_source_after_replacement_failure(
                            operating, position, role, recoveries, previous, stopped, source,
                            submission, error, queue, capacity,
                        ));
                    }
                };
                if let Err(source) = self.recovery.restore_source(source) {
                    self.restarts = proposal.decline();
                    return Ok(self.reject_replacement(
                        operating,
                        position,
                        role,
                        recoveries,
                        previous,
                        stopped,
                        submission,
                        Some(source),
                        WorkerReplacementError::SourceStateCorrupt,
                        queue,
                        capacity,
                        Actions::cont(),
                    ));
                }
                self.restarts = proposal.accept();
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
                match release {
                    WorkerReplacementRelease::Now(creation) => {
                        let prepared = PreparedWorker { role, submission };
                        let (member, worker, observation) = Member::begin_replacement(
                            prepared,
                            recoveries,
                            previous.creation(),
                            creation,
                        );
                        operating.roles.insert(
                            position,
                            RoleCell {
                                member,
                                queue,
                                capacity,
                            },
                        );
                        actions.creates.extend([worker]);
                        actions
                            .sends
                            .worker_observations
                            .append(InterpreterRequests::one(observation));
                    }
                    WorkerReplacementRelease::After { timer, delay } => {
                        operating.roles.insert(
                            position,
                            RoleCell {
                                member: Member {
                                    role,
                                    recoveries,
                                    state: MemberState::Recovering(
                                        direct_worker::RecoveringWorker::Scheduling {
                                            previous,
                                            stopped,
                                            submission,
                                            timer,
                                        },
                                    ),
                                },
                                queue,
                                capacity,
                            },
                        );
                        actions.sends.restart_schedules.send(timer.after(delay));
                    }
                }
                let (operating, actions) = self.prepare_next_waiting(operating, actions);
                Ok((KeyedPoolState::Operating(operating), actions))
            }
            RestartAdmission::Denied { budget, reason } => {
                self.restarts = budget;
                Ok(self.return_source_after_replacement_failure(
                    operating,
                    position,
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    submission,
                    WorkerReplacementError::RestartDenied(reason),
                    queue,
                    capacity,
                ))
            }
        }
    }

    pub(super) fn accept_restart_schedule(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        input: behavior::ActionItemResult<ScheduleAfter>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
            behavior::ActionItemResult<ScheduleAfter>,
        ),
    > {
        let Some(position) = operating.restart_schedule_position(&input) else {
            return Err((operating, input));
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        let Member {
            role,
            recoveries,
            state,
        } = member;
        let MemberState::Recovering(worker) = state else {
            operating.roles.insert(
                position,
                RoleCell {
                    member: Member {
                        role,
                        recoveries,
                        state,
                    },
                    queue,
                    capacity,
                },
            );
            return Err((operating, input));
        };
        match worker.admit_restart_schedule(input) {
            Ok(ControlFlow::Continue(worker)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Recovering(worker),
                        },
                        queue,
                        capacity,
                    },
                );
                Ok((KeyedPoolState::Operating(operating), Actions::cont()))
            }
            Ok(ControlFlow::Break((replacement, settlement))) => {
                let direct_worker::PendingWorkerReplacement {
                    previous,
                    stopped,
                    submission,
                } = replacement;
                Ok(self.reject_replacement(
                    operating,
                    position,
                    role,
                    recoveries,
                    previous,
                    stopped,
                    submission,
                    None,
                    WorkerReplacementError::RestartScheduleReturned(settlement),
                    queue,
                    capacity,
                    Actions::cont(),
                ))
            }
            Err((worker, settlement)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Recovering(worker),
                        },
                        queue,
                        capacity,
                    },
                );
                Err((operating, settlement))
            }
        }
    }

    pub(super) fn accept_restart_timer(
        &mut self,
        mut operating: KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
        elapsed: crate::TimerElapsed,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            KeyedOperating<Role, W, P, Key, Job, WorkerResult>,
            crate::TimerElapsed,
        ),
    > {
        let Some(position) = operating.restart_timer_position(&elapsed) else {
            return Err((operating, elapsed));
        };
        let RoleCell {
            member,
            queue,
            capacity,
        } = operating.roles.remove(position);
        let Member {
            role,
            recoveries,
            state,
        } = member;
        let MemberState::Recovering(worker) = state else {
            operating.roles.insert(
                position,
                RoleCell {
                    member: Member {
                        role,
                        recoveries,
                        state,
                    },
                    queue,
                    capacity,
                },
            );
            return Err((operating, elapsed));
        };
        let replacement = match worker.admit_restart_timer(elapsed) {
            Ok(replacement) => replacement,
            Err((worker, elapsed)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Recovering(worker),
                        },
                        queue,
                        capacity,
                    },
                );
                return Err((operating, elapsed));
            }
        };
        let direct_worker::PendingWorkerReplacement {
            previous,
            stopped,
            submission,
        } = replacement;
        let Some(creation) = self.creations.issue() else {
            return Ok(self.reject_replacement(
                operating,
                position,
                role,
                recoveries,
                previous,
                stopped,
                submission,
                None,
                WorkerReplacementError::WorkerCreationsExhausted,
                queue,
                capacity,
                Actions::cont(),
            ));
        };
        let prepared = PreparedWorker { role, submission };
        let (member, worker, observation) =
            Member::begin_replacement(prepared, recoveries, previous.creation(), creation);
        operating.roles.insert(
            position,
            RoleCell {
                member,
                queue,
                capacity,
            },
        );
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions.creates.extend([worker]);
        actions
            .sends
            .worker_observations
            .append(InterpreterRequests::one(observation));
        Ok((KeyedPoolState::Operating(operating), actions))
    }

    fn issue_restart_timer(&mut self) -> Option<ScheduleKey> {
        ScheduleKey::issue(&mut self.next_restart_timer)
    }
}
