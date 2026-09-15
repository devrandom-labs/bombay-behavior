use core::ops::ControlFlow;

use behavior::{
    Actions, Address, Behavior, BehaviorAddr, BehaviorBase, EndpointAddress, Here, InjectEvent,
    InterpreterRequests, ItemSettlement, Protocol, SendEffects, SettledItem,
};

use crate::{DiagnosticRoute, EstablishedShutdownResolved, ShutdownRequested, StopOnShutdown};

use behavior::ChildReport;

use crate::atomic::RoleName;
use crate::atomic::pool::assignment::{
    AssignmentReceipt, AssignmentReceiptOutcome, AssignmentRejectionOutcome, CorrelationMatch,
    CustomerJob, WorkerCompletionOutcome, WorkerExitOutcome,
};
use crate::atomic::pool::worker as direct_worker;
use crate::atomic::pool::worker::{Member, MemberState, ShutdownJoin, Worker, WorkerPhase};
use crate::atomic::pool::{
    AssignWorker, Assignment, BacklogCapacity, CompletesAssignments, Completion, CustomerDelivery,
};
use crate::atomic::restart::RecoveryCount;
use crate::atomic::{ActivationPlan, WorkerSource};

use super::protocol;
use super::role::RoleCell;
use super::{
    CustomerRoute, KeyedActions, KeyedAssignedReturnReason, KeyedCustomer, KeyedDiagnostic,
    KeyedEvent, KeyedOutcome, KeyedPool, KeyedPoolState, KeyedQueue, Operating,
};

impl<Role, W, P, Key, Job, WorkerResult> Operating<Role, W, P, Key, Job, WorkerResult>
where
    Role: Eq,
    W: Behavior + BehaviorBase,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    fn worker_id_position(&self, worker: behavior::CreationId) -> Option<usize> {
        self.roles.iter().position(|cell| {
            cell.member
                .worker_attempt()
                .is_some_and(|attempt| attempt.creation() == worker)
        })
    }

    fn assignment_receipt_position(&self, receipt: &AssignmentReceipt) -> Option<usize> {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Worker(Worker {
                phase: WorkerPhase::Busy(assignment),
                ..
            }) => matches!(assignment.compare_receipt(receipt), CorrelationMatch::Exact),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn completion_position(
        &self,
        child: behavior::CreationId,
        completion: &Completion<WorkerResult>,
    ) -> Option<usize> {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Worker(Worker {
                current,
                phase: WorkerPhase::Busy(assignment),
            }) if current.attempt.creation() == child => matches!(
                assignment.compare_completion(completion),
                CorrelationMatch::Exact
            ),
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
        })
    }

    fn worker_shutdown_position(&self, request: crate::ShutdownId) -> Option<usize> {
        self.roles.iter().position(|cell| match &cell.member.state {
            MemberState::Worker(Worker {
                phase: WorkerPhase::Stopping(shutdown),
                ..
            }) => shutdown.request() == request,
            MemberState::Creating(_)
            | MemberState::Worker(_)
            | MemberState::Recovering(_)
            | MemberState::Retired => false,
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
    pub(super) fn return_assigned(
        customer: CustomerJob<
            Job,
            KeyedCustomer<Role, CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>>,
        >,
        reason: KeyedAssignedReturnReason,
        actions: &mut KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
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
                KeyedOutcome::ReturnedAssigned {
                    job: id,
                    binding,
                    payload,
                    reason,
                },
            )));
    }

    fn complete_assignment(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        customer: CustomerJob<
            Job,
            KeyedCustomer<Role, CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>>,
        >,
        worker_result: WorkerResult,
        stopped: Option<crate::ChildStopped<BehaviorAddr<W>>>,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: BacklogCapacity,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let CustomerJob {
            id,
            admitted: _,
            payload: _,
            customer: KeyedCustomer { binding, route },
        } = customer;
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        actions.sends.customer_outcomes = InterpreterRequests::one(CustomerDelivery::outcome(
            route,
            KeyedOutcome::Completed {
                job: id,
                binding,
                worker_result,
            },
        ));
        match stopped {
            None => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Idle,
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                self.fill_role(&mut operating, position, &mut actions);
                (KeyedPoolState::Operating(operating), actions)
            }
            Some(stopped) => self.recover_worker(
                operating,
                position,
                role,
                recoveries,
                current.attempt,
                stopped,
                None,
                queue,
                capacity,
                actions,
            ),
        }
    }

    fn interrupt_assignment(
        &mut self,
        operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        customer: CustomerJob<
            Job,
            KeyedCustomer<Role, CustomerRoute<BehaviorAddr<W>, Key, Role, Job, WorkerResult>>,
        >,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        late_completion: Option<Completion<WorkerResult>>,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: BacklogCapacity,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        if let Some(completion) = late_completion {
            actions
                .sends
                .diagnostics
                .append(InterpreterRequests::one(self.diagnostics.action(
                    KeyedDiagnostic::from(protocol::KeyedDiagnosticCause::Unexpected(
                        KeyedEvent::WorkerCompleted(ChildReport::new(
                            current.attempt.creation(),
                            completion,
                        )),
                    )),
                )));
        }
        self.recover_worker(
            operating,
            position,
            role,
            recoveries,
            current.attempt,
            stopped,
            Some(customer),
            queue,
            capacity,
            actions,
        )
    }

    fn quarantine_worker(
        &self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        shutdown: crate::ShutdownId,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: BacklogCapacity,
        mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) -> (
        Operating<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        actions
            .sends
            .worker_shutdowns
            .append(InterpreterRequests::one(current.shutdown_request(shutdown)));
        operating.roles.insert(
            position,
            RoleCell {
                member: Member {
                    role,
                    recoveries,
                    state: MemberState::Worker(Worker {
                        current,
                        phase: WorkerPhase::Stopping(ShutdownJoin::AwaitingBoth(shutdown)),
                    }),
                },
                queue,
                capacity,
            },
        );
        (operating, actions)
    }

    pub(super) fn reject_assignment_delivery(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        item: AssignWorker<W::Protocol, Job>,
        reason: behavior::ExactDeliveryReason,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let receipt = item.receipt();
        let Some(position) = operating.assignment_receipt_position(&receipt) else {
            return self.diagnose(
                KeyedPoolState::Operating(operating),
                KeyedEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason,
                })),
            );
        };
        let Some(shutdown) = self
            .shutdowns
            .reserve(1)
            .and_then(|reserved| reserved.into_iter().next())
        else {
            let (state, actions) = self.diagnose(
                KeyedPoolState::Operating(operating),
                KeyedEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason,
                })),
            );
            return match state {
                KeyedPoolState::Operating(operating) => self.begin_retirement(operating, actions),
                state => (state, actions),
            };
        };
        let (target, returned, receipt) = item.into_parts();
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
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Busy(assignment),
        }) = state
        else {
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
            return self.diagnose(
                KeyedPoolState::Operating(operating),
                KeyedEvent::AssignmentSettled(SettledItem::Attempted(ItemSettlement::Rejected {
                    item: AssignWorker::returned(target, returned, receipt),
                    reason,
                })),
            );
        };
        let rejected = match assignment.accept_rejection(returned) {
            Ok(rejected) => rejected,
            Err((assignment, returned)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Busy(assignment),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                return self.diagnose(
                    KeyedPoolState::Operating(operating),
                    KeyedEvent::AssignmentSettled(SettledItem::Attempted(
                        ItemSettlement::Rejected {
                            item: AssignWorker::returned(target, returned, receipt),
                            reason,
                        },
                    )),
                );
            }
        };
        match rejected {
            AssignmentRejectionOutcome::JobReturned { customer, stopped } => {
                let mut queue = queue;
                queue.insert(customer.admitted, customer);
                let actions = Actions::cont();
                match stopped {
                    None => {
                        let (operating, actions) = self.quarantine_worker(
                            operating, position, role, recoveries, current, shutdown, queue,
                            capacity, actions,
                        );
                        (KeyedPoolState::Operating(operating), actions)
                    }
                    Some(stopped) => self.recover_worker(
                        operating,
                        position,
                        role,
                        recoveries,
                        current.attempt,
                        stopped,
                        None,
                        queue,
                        capacity,
                        actions,
                    ),
                }
            }
            AssignmentRejectionOutcome::ConflictingCompletion {
                customer,
                assignment,
                completion,
                stopped,
            } => {
                let mut actions = Actions::cont();
                Self::return_assigned(
                    customer,
                    KeyedAssignedReturnReason::ContradictoryAssignmentSettlement,
                    &mut actions,
                );
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::Unexpected(KeyedEvent::AssignmentSettled(
                            SettledItem::Attempted(ItemSettlement::Rejected {
                                item: AssignWorker::returned(target, assignment, receipt),
                                reason,
                            }),
                        )),
                    )),
                ));
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerCompleted(
                            ChildReport::new(current.attempt.creation(), completion),
                        )),
                    )),
                ));
                match stopped {
                    None => {
                        let (operating, actions) = self.quarantine_worker(
                            operating, position, role, recoveries, current, shutdown, queue,
                            capacity, actions,
                        );
                        self.begin_retirement(operating, actions)
                    }
                    Some(stopped) => {
                        let (state, actions) = self.recover_worker(
                            operating,
                            position,
                            role,
                            recoveries,
                            current.attempt,
                            stopped,
                            None,
                            queue,
                            capacity,
                            actions,
                        );
                        match state {
                            KeyedPoolState::Operating(operating) => {
                                self.begin_retirement(operating, actions)
                            }
                            state => (state, actions),
                        }
                    }
                }
            }
        }
    }

    pub(super) fn accept_assignment_receipt(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        receipt: AssignmentReceipt,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            AssignmentReceipt,
        ),
    > {
        let Some(position) = operating.assignment_receipt_position(&receipt) else {
            return Err((operating, receipt));
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
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Busy(assignment),
        }) = state
        else {
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
            return Err((operating, receipt));
        };
        match assignment.accept_receipt(receipt) {
            Ok(AssignmentReceiptOutcome::AwaitingCompletion(assignment)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Busy(assignment),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                Ok((KeyedPoolState::Operating(operating), Actions::cont()))
            }
            Ok(AssignmentReceiptOutcome::JobCompleted {
                customer,
                result,
                stopped,
            }) => Ok(self.complete_assignment(
                operating, position, role, recoveries, current, customer, result, stopped, queue,
                capacity,
            )),
            Ok(AssignmentReceiptOutcome::JobInterrupted {
                customer,
                stopped,
                late_completion,
            }) => Ok(self.interrupt_assignment(
                operating,
                position,
                role,
                recoveries,
                current,
                customer,
                stopped,
                late_completion,
                queue,
                capacity,
            )),
            Err((assignment, receipt)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Busy(assignment),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                Err((operating, receipt))
            }
        }
    }

    pub(super) fn accept_completion(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        child: behavior::CreationId,
        completion: Completion<WorkerResult>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            ChildReport<Completion<WorkerResult>>,
        ),
    > {
        let Some(position) = operating.completion_position(child, &completion) else {
            return Err((operating, ChildReport::new(child, completion)));
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
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Busy(assignment),
        }) = state
        else {
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
            return Err((operating, ChildReport::new(child, completion)));
        };
        match assignment.accept_completion(completion) {
            Ok(WorkerCompletionOutcome::AwaitingReceipt(assignment)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Busy(assignment),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                Ok((KeyedPoolState::Operating(operating), Actions::cont()))
            }
            Ok(WorkerCompletionOutcome::JobCompleted {
                customer,
                result,
                stopped,
            }) => Ok(self.complete_assignment(
                operating, position, role, recoveries, current, customer, result, stopped, queue,
                capacity,
            )),
            Err((assignment, completion)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Busy(assignment),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                Err((operating, ChildReport::new(child, completion)))
            }
        }
    }

    pub(super) fn accept_worker_stop(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            crate::ChildStopped<BehaviorAddr<W>>,
        ),
    > {
        let Some(position) = operating.worker_id_position(stopped.child) else {
            return Err((operating, stopped));
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
        let MemberState::Worker(Worker { current, phase }) = state else {
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
            return Err((operating, stopped));
        };
        match phase {
            WorkerPhase::Idle => Ok(self.recover_worker(
                operating,
                position,
                role,
                recoveries,
                current.attempt,
                stopped,
                None,
                queue,
                capacity,
                Actions::cont(),
            )),
            WorkerPhase::Busy(assignment) => match assignment.accept_worker_exit(stopped) {
                Ok(WorkerExitOutcome::AwaitingReceipt(assignment)) => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member: Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Busy(assignment),
                                }),
                            },
                            queue,
                            capacity,
                        },
                    );
                    Ok((KeyedPoolState::Operating(operating), Actions::cont()))
                }
                Ok(WorkerExitOutcome::JobInterrupted {
                    customer,
                    stopped,
                    late_completion,
                }) => Ok(self.interrupt_assignment(
                    operating,
                    position,
                    role,
                    recoveries,
                    current,
                    customer,
                    stopped,
                    late_completion,
                    queue,
                    capacity,
                )),
                Err((assignment, stopped)) => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member: Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Busy(assignment),
                                }),
                            },
                            queue,
                            capacity,
                        },
                    );
                    Err((operating, stopped))
                }
            },
            WorkerPhase::Stopping(shutdown) => match shutdown.stopped(stopped, &current.attempt) {
                Ok(ControlFlow::Continue(shutdown)) => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member: Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Stopping(shutdown),
                                }),
                            },
                            queue,
                            capacity,
                        },
                    );
                    Ok((KeyedPoolState::Operating(operating), Actions::cont()))
                }
                Ok(ControlFlow::Break((shutdown, stopped))) => Ok(self.recover_quarantined_worker(
                    operating, position, role, recoveries, current, shutdown, stopped, queue,
                    capacity,
                )),
                Err((shutdown, stopped)) => {
                    operating.roles.insert(
                        position,
                        RoleCell {
                            member: Member {
                                role,
                                recoveries,
                                state: MemberState::Worker(Worker {
                                    current,
                                    phase: WorkerPhase::Stopping(shutdown),
                                }),
                            },
                            queue,
                            capacity,
                        },
                    );
                    Err((operating, stopped))
                }
            },
            phase => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker { current, phase }),
                        },
                        queue,
                        capacity,
                    },
                );
                Err((operating, stopped))
            }
        }
    }

    fn recover_quarantined_worker(
        &mut self,
        operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        position: usize,
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        current: direct_worker::CurrentWorker<W>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
        stopped: crate::ChildStopped<BehaviorAddr<W>>,
        queue: KeyedQueue<Role, W, Key, Job, WorkerResult>,
        capacity: BacklogCapacity,
    ) -> (
        KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
        KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
    ) {
        let mut actions: KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult> =
            Actions::cont();
        match shutdown {
            EstablishedShutdownResolved::Accepted { .. } => {}
            shutdown @ EstablishedShutdownResolved::Rejected { .. } => {
                actions.sends.diagnostics.append(InterpreterRequests::one(
                    self.diagnostics.action(KeyedDiagnostic::from(
                        protocol::KeyedDiagnosticCause::WorkerShutdownRejected {
                            role: role.clone(),
                            shutdown,
                        },
                    )),
                ));
            }
        }
        self.recover_worker(
            operating,
            position,
            role,
            recoveries,
            current.attempt,
            stopped,
            None,
            queue,
            capacity,
            actions,
        )
    }

    pub(super) fn accept_quarantine_shutdown(
        &mut self,
        mut operating: Operating<Role, W, P, Key, Job, WorkerResult>,
        settlement: EstablishedShutdownResolved<W::Protocol>,
    ) -> Result<
        (
            KeyedPoolState<Role, W, P, Key, Job, WorkerResult>,
            KeyedActions<Role, W, P, Source, Diagnostics, Key, Job, WorkerResult>,
        ),
        (
            Operating<Role, W, P, Key, Job, WorkerResult>,
            EstablishedShutdownResolved<W::Protocol>,
        ),
    > {
        let Some(position) = operating.worker_shutdown_position(settlement.id()) else {
            return Err((operating, settlement));
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
        let MemberState::Worker(Worker {
            current,
            phase: WorkerPhase::Stopping(shutdown),
        }) = state
        else {
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
            return Err((operating, settlement));
        };
        match shutdown.settled(settlement) {
            Ok(ControlFlow::Continue(shutdown)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Stopping(shutdown),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                Ok((KeyedPoolState::Operating(operating), Actions::cont()))
            }
            Ok(ControlFlow::Break((shutdown, stopped))) => Ok(self.recover_quarantined_worker(
                operating, position, role, recoveries, current, shutdown, stopped, queue, capacity,
            )),
            Err((shutdown, settlement)) => {
                operating.roles.insert(
                    position,
                    RoleCell {
                        member: Member {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::Stopping(shutdown),
                            }),
                        },
                        queue,
                        capacity,
                    },
                );
                Err((operating, settlement))
            }
        }
    }
}
