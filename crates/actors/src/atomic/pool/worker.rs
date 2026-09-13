//! One semantic role and its current direct-worker relationship.

use core::ops::ControlFlow;

use behavior::{
    Behavior, BehaviorAddr, BehaviorBase, ChildCreationOutcome, ChildCreationSettled, ChildHead,
    CreateChild, CreationKind, EndpointAddress, EstablishedActor, EstablishedCreation,
    EstablishedRecipient, Here, Ingress, InjectEvent, ItemSettlement, SettledItem,
};

use crate::{
    ChildStopped, EstablishedShutdownResolved, ShutdownEstablished, ShutdownId, ShutdownRequested,
    StopOnShutdown,
};

use super::super::restart::RecoveryCount;
use super::super::schedule::ScheduleKey;
use super::super::worker::{
    ActivationAttempt, PreparationTicket, WorkerActivationOutcome, WorkerPreparationOutcome,
    preparation_result_accepts,
};
use super::super::worker::{
    WorkerCreationOutcome, WorkerCreationSettlement, settle_worker_creation,
};
use super::super::{
    ActivationPermit, ActivationPlan, BeginActivation, InitializationAttempt, InitializeWorker,
    PrepareWorkers, PreparedWorker, RoleName, WorkerActivation, WorkerAttempt,
    WorkerCreationRejection, WorkerInitializationReport, WorkerSource, WorkerSubmission,
};
use super::assignment::{AssignedJob, AssignmentShutdown};

#[expect(
    dead_code,
    reason = "the emitted diagnostic retains the exact preparation failure"
)]
pub(in crate::atomic) enum WorkerPreparationError<WorkerRejection, SourceRejection> {
    WorkerRejected(WorkerRejection),
    SourceRejected(SourceRejection),
    InterpreterCorrupt(behavior::InterpreterFault),
    InterpretationSkipped,
    SourceStateCorrupt,
}

pub(in crate::atomic) struct CurrentWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) attempt: WorkerAttempt,
    actor: EstablishedActor<StopOnShutdown<W>>,
}

impl<W> CurrentWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
{
    pub(in crate::atomic) const fn new(
        attempt: WorkerAttempt,
        actor: EstablishedActor<StopOnShutdown<W>>,
    ) -> Self {
        Self { attempt, actor }
    }

    pub(in crate::atomic) fn recipient(&self) -> EstablishedRecipient<W::Protocol> {
        self.actor.recipient()
    }

    pub(in crate::atomic) fn shutdown_request(
        &self,
        id: ShutdownId,
    ) -> ShutdownEstablished<StopOnShutdown<W>, Here>
    where
        <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    {
        ShutdownEstablished::new(id, self.actor.clone(), Ingress::new())
    }
}

pub(in crate::atomic) struct CreatingWorker<W, P>
where
    W: Behavior,
{
    pub(in crate::atomic) attempt: WorkerAttempt,
    pub(in crate::atomic) initialization: InitializationAttempt,
    pub(in crate::atomic) activation: P,
    pub(in crate::atomic) kind: behavior::CreationKind,
    pub(in crate::atomic) stopped: Option<ChildStopped<BehaviorAddr<W>>>,
}

pub(in crate::atomic) enum WorkerPhase<W, P, Assigned>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Initializing {
        initialization: InitializationAttempt,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    WaitingForActivation {
        permit: ActivationPermit<W>,
        activation: P,
    },
    ActivationDispatched {
        attempt: ActivationAttempt,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Activating {
        attempt: ActivationAttempt,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Idle,
    Busy(Assigned),
    Stopping(ShutdownJoin<W::Protocol, BehaviorAddr<W>>),
}

pub(in crate::atomic) struct Worker<W, P, Assigned>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) current: CurrentWorker<W>,
    pub(in crate::atomic) phase: WorkerPhase<W, P, Assigned>,
}

pub(in crate::atomic) enum RecoveringWorker<W, P>
where
    W: Behavior,
{
    WaitingForSource {
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
    Preparing {
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        preparation: PreparationTicket,
    },
    Scheduling {
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        submission: WorkerSubmission<W, P>,
        timer: ScheduleKey,
    },
    WaitingForTimer {
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        submission: WorkerSubmission<W, P>,
        timer: ScheduleKey,
    },
}

pub(in crate::atomic) struct PreparedReplacement<Role, W, P, Source>
where
    W: Behavior,
{
    pub(in crate::atomic) role: RoleName<Role>,
    pub(in crate::atomic) recoveries: RecoveryCount,
    pub(in crate::atomic) previous: WorkerAttempt,
    pub(in crate::atomic) stopped: ChildStopped<BehaviorAddr<W>>,
    pub(in crate::atomic) source: Source,
    pub(in crate::atomic) submission: WorkerSubmission<W, P>,
}

#[expect(
    dead_code,
    reason = "the emitted pool diagnostic retains the exact replacement failure"
)]
pub(in crate::atomic) enum WorkerReplacementError {
    PoolShutdown,
    RestartDenied(super::super::restart::RestartDenial),
    WorkerCreationsExhausted,
    RestartTimersExhausted,
    RestartScheduleReturned(behavior::ActionItemResult<crate::ScheduleAfter>),
    SourceStateCorrupt,
}

pub(in crate::atomic) enum WorkerReplacementRelease {
    Now(behavior::CreationId),
    After {
        timer: ScheduleKey,
        delay: core::time::Duration,
    },
}

pub(in crate::atomic) struct PendingWorkerReplacement<W, P>
where
    W: Behavior,
{
    pub(in crate::atomic) previous: WorkerAttempt,
    pub(in crate::atomic) stopped: ChildStopped<BehaviorAddr<W>>,
    pub(in crate::atomic) submission: WorkerSubmission<W, P>,
}

pub(in crate::atomic) enum WorkerRecoveryPreparation<Role, W, P, Source>
where
    Role: Send + Sync,
    W: Behavior + Send,
    BehaviorAddr<W>: EndpointAddress,
    Source: WorkerSource<Role, W, P>,
    P: ActivationPlan,
{
    Ready(PreparedReplacement<Role, W, P, Source>),
    Failed {
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        source: Source,
        error: WorkerPreparationError<Source::WorkerRejection, Source::SourceRejection>,
    },
}

impl<W, P> RecoveringWorker<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) fn accepts_restart_schedule(
        &self,
        input: &behavior::ActionItemResult<crate::ScheduleAfter>,
    ) -> bool {
        match self {
            Self::Scheduling { timer, .. } => timer.accepts_result(input),
            Self::WaitingForSource { .. }
            | Self::Preparing { .. }
            | Self::WaitingForTimer { .. } => false,
        }
    }

    pub(in crate::atomic) fn accepts_restart_timer(&self, elapsed: &crate::TimerElapsed) -> bool {
        match self {
            Self::WaitingForTimer { timer, .. } => {
                timer.id == elapsed.id && timer.generation == elapsed.generation
            }
            Self::WaitingForSource { .. } | Self::Preparing { .. } | Self::Scheduling { .. } => {
                false
            }
        }
    }

    pub(in crate::atomic) fn admit_restart_schedule(
        self,
        input: behavior::ActionItemResult<crate::ScheduleAfter>,
    ) -> Result<
        ControlFlow<
            (
                PendingWorkerReplacement<W, P>,
                behavior::ActionItemResult<crate::ScheduleAfter>,
            ),
            Self,
        >,
        (Self, behavior::ActionItemResult<crate::ScheduleAfter>),
    > {
        let Self::Scheduling {
            previous,
            stopped,
            submission,
            timer,
        } = self
        else {
            return Err((self, input));
        };
        let input = match timer.admit_result(input) {
            Ok(input) => input,
            Err(input) => {
                return Err((
                    Self::Scheduling {
                        previous,
                        stopped,
                        submission,
                        timer,
                    },
                    input,
                ));
            }
        };
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(_)) => {
                Ok(ControlFlow::Continue(Self::WaitingForTimer {
                    previous,
                    stopped,
                    submission,
                    timer,
                }))
            }
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match prerequisite {}
            }
            input @ (SettledItem::Attempted(
                ItemSettlement::Rejected { .. } | ItemSettlement::Corrupt { .. },
            )
            | SettledItem::Unattempted(_)) => Ok(ControlFlow::Break((
                PendingWorkerReplacement {
                    previous,
                    stopped,
                    submission,
                },
                input,
            ))),
        }
    }

    pub(in crate::atomic) fn admit_restart_timer(
        self,
        elapsed: crate::TimerElapsed,
    ) -> Result<PendingWorkerReplacement<W, P>, (RecoveringWorker<W, P>, crate::TimerElapsed)> {
        let Self::WaitingForTimer {
            previous,
            stopped,
            submission,
            timer,
        } = self
        else {
            return Err((self, elapsed));
        };
        match timer.admit_elapsed(elapsed) {
            Ok(_) => Ok(PendingWorkerReplacement {
                previous,
                stopped,
                submission,
            }),
            Err(elapsed) => Err((
                Self::WaitingForTimer {
                    previous,
                    stopped,
                    submission,
                    timer,
                },
                elapsed,
            )),
        }
    }

    pub(in crate::atomic) fn accept_preparation<Source, Role>(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        expected: PreparationTicket,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Result<
        WorkerRecoveryPreparation<Role, W, P, Source>,
        (
            RoleName<Role>,
            RecoveryCount,
            WorkerAttempt,
            ChildStopped<BehaviorAddr<W>>,
            PreparationTicket,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        ),
    >
    where
        Role: Send + Sync,
        W: Send,
        Source: WorkerSource<Role, W, P>,
    {
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)) => {
                let (ticket, outcome) = preparation.into_parts();
                match outcome {
                    WorkerPreparationOutcome::Prepared {
                        source,
                        mut members,
                    } => match members.pop() {
                        Some(prepared) => {
                            Ok(WorkerRecoveryPreparation::Ready(PreparedReplacement {
                                role,
                                recoveries,
                                previous,
                                stopped,
                                source,
                                submission: prepared.submission,
                            }))
                        }
                        prepared => {
                            if let Some(prepared) = prepared {
                                members.push(prepared);
                            }
                            Err((
                                role,
                                recoveries,
                                previous,
                                stopped,
                                expected,
                                SettledItem::Attempted(ItemSettlement::Accepted(
                                    super::super::WorkerPreparation::from_parts(
                                        ticket,
                                        WorkerPreparationOutcome::Prepared { source, members },
                                    ),
                                )),
                            ))
                        }
                    },
                    WorkerPreparationOutcome::WorkerRejected { source, reason, .. } => {
                        Ok(WorkerRecoveryPreparation::Failed {
                            role,
                            recoveries,
                            previous,
                            stopped,
                            source,
                            error: WorkerPreparationError::WorkerRejected(reason),
                        })
                    }
                }
            }
            SettledItem::Attempted(ItemSettlement::Rejected { item, reason }) => {
                let (source, _, _) = item.into_parts();
                Ok(WorkerRecoveryPreparation::Failed {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    error: WorkerPreparationError::SourceRejected(reason),
                })
            }
            SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }) => {
                let (source, _, _) = item.into_parts();
                Ok(WorkerRecoveryPreparation::Failed {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    error: WorkerPreparationError::InterpreterCorrupt(fault),
                })
            }
            SettledItem::Unattempted(item) => {
                let (source, _, _) = item.into_parts();
                Ok(WorkerRecoveryPreparation::Failed {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    error: WorkerPreparationError::InterpretationSkipped,
                })
            }
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match prerequisite {}
            }
        }
    }
}

pub(in crate::atomic) enum ShutdownJoin<P, A>
where
    P: behavior::Protocol,
    A: behavior::Address,
{
    AwaitingBoth(ShutdownId),
    AwaitingStop(EstablishedShutdownResolved<P>),
    AwaitingSettlement {
        request: ShutdownId,
        stopped: ChildStopped<A>,
    },
}

impl<P, A> ShutdownJoin<P, A>
where
    P: behavior::Protocol,
    A: behavior::Address,
{
    pub(in crate::atomic) const fn request(&self) -> ShutdownId {
        match self {
            Self::AwaitingBoth(request) | Self::AwaitingSettlement { request, .. } => *request,
            Self::AwaitingStop(settlement) => settlement.id(),
        }
    }

    pub(in crate::atomic) fn settled(
        self,
        settlement: EstablishedShutdownResolved<P>,
    ) -> Result<
        ControlFlow<(EstablishedShutdownResolved<P>, ChildStopped<A>), Self>,
        (Self, EstablishedShutdownResolved<P>),
    > {
        match self {
            Self::AwaitingBoth(expected) if settlement.id() == expected => {
                Ok(ControlFlow::Continue(Self::AwaitingStop(settlement)))
            }
            Self::AwaitingSettlement { request, stopped } if settlement.id() == request => {
                Ok(ControlFlow::Break((settlement, stopped)))
            }
            current => Err((current, settlement)),
        }
    }

    pub(in crate::atomic) fn stopped(
        self,
        stopped: ChildStopped<A>,
        worker: &WorkerAttempt,
    ) -> Result<
        ControlFlow<(EstablishedShutdownResolved<P>, ChildStopped<A>), Self>,
        (Self, ChildStopped<A>),
    > {
        if stopped.child != worker.creation() {
            return Err((self, stopped));
        }
        match self {
            Self::AwaitingBoth(request) => Ok(ControlFlow::Continue(Self::AwaitingSettlement {
                request,
                stopped,
            })),
            Self::AwaitingStop(settlement) => Ok(ControlFlow::Break((settlement, stopped))),
            current @ Self::AwaitingSettlement { .. } => Err((current, stopped)),
        }
    }
}

pub(in crate::atomic) enum MemberState<W, P, Job, WorkerResult, Customer>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Creating(CreatingWorker<W, P>),
    Worker(Worker<W, P, AssignedJob<Job, Customer, WorkerResult, BehaviorAddr<W>>>),
    Recovering(RecoveringWorker<W, P>),
    Retired,
}

pub(in crate::atomic) struct Member<Role, W, P, Job, WorkerResult, Customer>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) role: RoleName<Role>,
    pub(in crate::atomic) recoveries: RecoveryCount,
    pub(in crate::atomic) state: MemberState<W, P, Job, WorkerResult, Customer>,
}

pub(in crate::atomic) enum WorkerStartupCustody<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    ActivationPlan(P),
    Initializing(InitializationAttempt),
    ActivationPermit {
        permit: ActivationPermit<W>,
        activation: P,
    },
    ActivationStart(ActivationAttempt),
    Activation(ActivationAttempt),
}

pub(in crate::atomic) enum WorkerShutdownStatus<P, A>
where
    P: behavior::Protocol,
    A: behavior::Address,
{
    NotRequested,
    Waiting(ShutdownJoin<P, A>),
}

pub(in crate::atomic) enum RetirementStatus<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    AwaitingCreation(CreatingWorker<W, P>),
    Established {
        current: CurrentWorker<W>,
        startup: Option<WorkerStartupCustody<W, P>>,
        shutdown: WorkerShutdownStatus<W::Protocol, BehaviorAddr<W>>,
    },
    AwaitingPreparation {
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        preparation: PreparationTicket,
    },
    AwaitingRestartSchedule {
        replacement: PendingWorkerReplacement<W, P>,
        timer: ScheduleKey,
    },
    Drained,
}

pub(in crate::atomic) struct RetiringWorker<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) role: RoleName<Role>,
    pub(in crate::atomic) state: RetirementStatus<W, P>,
}

pub(in crate::atomic) struct WorkerDeparture<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) role: RoleName<Role>,
    pub(in crate::atomic) startup: Option<WorkerStartupCustody<W, P>>,
    pub(in crate::atomic) shutdown: EstablishedShutdownResolved<W::Protocol>,
}

pub(in crate::atomic) enum WorkerCustody<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Running {
        role: RoleName<Role>,
        worker: CurrentWorker<W>,
        activation: P,
    },
    Returned {
        role: RoleName<Role>,
        rejection: WorkerCreationRejection<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Stopped {
        role: RoleName<Role>,
        worker: CurrentWorker<W>,
        activation: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

pub(in crate::atomic) enum RetiringWorkerInitialization<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Returned {
        workers: Vec<RetiringWorker<Role, W, P>>,
        report: WorkerInitializationReport<W, P>,
    },
    WorkerStopped {
        workers: Vec<RetiringWorker<Role, W, P>>,
        departure: Option<WorkerDeparture<Role, W, P>>,
        role: RoleName<Role>,
        activation: P,
    },
}

pub(in crate::atomic) enum RetiringWorkerActivation<Role, W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Started {
        workers: Vec<RetiringWorker<Role, W, P>>,
    },
    Returned {
        workers: Vec<RetiringWorker<Role, W, P>>,
        role: RoleName<Role>,
        worker: WorkerAttempt,
        activation: ActivationAttempt,
        outcome: WorkerActivationOutcome<W, P>,
    },
    Unrelated {
        workers: Vec<RetiringWorker<Role, W, P>>,
        activation: WorkerActivation<W, P>,
    },
}

impl<Role, W, P> RetiringWorker<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    fn initialization(&self) -> Option<&InitializationAttempt> {
        match &self.state {
            RetirementStatus::Established {
                startup: Some(WorkerStartupCustody::Initializing(initialization)),
                ..
            } => Some(initialization),
            _ => None,
        }
    }

    fn activation(&self) -> Option<&ActivationAttempt> {
        match &self.state {
            RetirementStatus::Established {
                startup:
                    Some(
                        WorkerStartupCustody::ActivationStart(activation)
                        | WorkerStartupCustody::Activation(activation),
                    ),
                ..
            } => Some(activation),
            _ => None,
        }
    }

    pub(in crate::atomic) fn expected_creation(
        &self,
    ) -> Option<(behavior::CreationId, CreationKind)> {
        match &self.state {
            RetirementStatus::AwaitingCreation(worker) => {
                Some((worker.attempt.creation(), worker.kind))
            }
            RetirementStatus::Established { .. }
            | RetirementStatus::AwaitingPreparation { .. }
            | RetirementStatus::AwaitingRestartSchedule { .. }
            | RetirementStatus::Drained => None,
        }
    }

    pub(in crate::atomic) fn waiting(&self) -> Option<&RetirementStatus<W, P>> {
        match &self.state {
            state @ (RetirementStatus::AwaitingCreation(_)
            | RetirementStatus::Established { .. }
            | RetirementStatus::AwaitingPreparation { .. }
            | RetirementStatus::AwaitingRestartSchedule { .. }) => Some(state),
            RetirementStatus::Drained => None,
        }
    }

    pub(in crate::atomic) fn accept_exit(
        self,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<ControlFlow<WorkerDeparture<Role, W, P>, Self>, (Self, ChildStopped<BehaviorAddr<W>>)>
    {
        let Self { role, state } = self;
        match state {
            RetirementStatus::AwaitingCreation(mut worker)
                if worker.attempt.creation() == stopped.child =>
            {
                match worker.stopped {
                    None => {
                        worker.stopped = Some(stopped);
                        Ok(ControlFlow::Continue(Self {
                            role,
                            state: RetirementStatus::AwaitingCreation(worker),
                        }))
                    }
                    Some(_) => Err((
                        Self {
                            role,
                            state: RetirementStatus::AwaitingCreation(worker),
                        },
                        stopped,
                    )),
                }
            }
            RetirementStatus::Established {
                current,
                startup,
                shutdown: WorkerShutdownStatus::Waiting(join),
            } => match join.stopped(stopped, &current.attempt) {
                Ok(ControlFlow::Continue(join)) => Ok(ControlFlow::Continue(Self {
                    role,
                    state: RetirementStatus::Established {
                        current,
                        startup,
                        shutdown: WorkerShutdownStatus::Waiting(join),
                    },
                })),
                Ok(ControlFlow::Break((shutdown, _))) => Ok(ControlFlow::Break(WorkerDeparture {
                    role,
                    startup,
                    shutdown,
                })),
                Err((join, stopped)) => Err((
                    Self {
                        role,
                        state: RetirementStatus::Established {
                            current,
                            startup,
                            shutdown: WorkerShutdownStatus::Waiting(join),
                        },
                    },
                    stopped,
                )),
            },
            state => Err((Self { role, state }, stopped)),
        }
    }

    pub(in crate::atomic) fn accept_creation(
        self,
        creation: ChildCreationSettled<StopOnShutdown<W>, ChildHead>,
    ) -> Result<WorkerCustody<Role, W, P>, (Self, ChildCreationSettled<StopOnShutdown<W>, ChildHead>)>
    where
        W: BehaviorBase,
        P: ActivationPlan,
        StopOnShutdown<W>:
            Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
        <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    {
        let Self { role, state } = self;
        let RetirementStatus::AwaitingCreation(worker) = state else {
            return Err((Self { role, state }, creation));
        };
        let settlement = creation.into_settlement();
        if creation_identity(&settlement) != Some((worker.attempt.creation(), worker.kind)) {
            return Err((
                Self {
                    role,
                    state: RetirementStatus::AwaitingCreation(worker),
                },
                ChildCreationSettled::new(settlement),
            ));
        }
        match settle_worker_creation(settlement) {
            WorkerCreationOutcome::Established(actor) => {
                let current = CurrentWorker::new(worker.attempt, actor);
                match worker.stopped {
                    Some(stopped) => Ok(WorkerCustody::Stopped {
                        role,
                        worker: current,
                        activation: worker.activation,
                        stopped,
                    }),
                    None => Ok(WorkerCustody::Running {
                        role,
                        worker: current,
                        activation: worker.activation,
                    }),
                }
            }
            WorkerCreationOutcome::Rejected(rejection) => Ok(WorkerCustody::Returned {
                role,
                rejection,
                activation: worker.activation,
                stopped: worker.stopped,
            }),
            WorkerCreationOutcome::InvalidSettlement(settlement) => Err((
                Self {
                    role,
                    state: RetirementStatus::AwaitingCreation(worker),
                },
                ChildCreationSettled::new(settlement),
            )),
        }
    }

    pub(in crate::atomic) fn accept_creation_rejection(
        self,
        id: behavior::CreationId,
        kind: CreationKind,
        rejection: WorkerCreationRejection<W>,
    ) -> Result<
        WorkerCustody<Role, W, P>,
        (
            Self,
            behavior::CreationId,
            CreationKind,
            WorkerCreationRejection<W>,
        ),
    > {
        let Self { role, state } = self;
        let RetirementStatus::AwaitingCreation(worker) = state else {
            return Err((Self { role, state }, id, kind, rejection));
        };
        if (id, kind) != (worker.attempt.creation(), worker.kind) {
            return Err((
                Self {
                    role,
                    state: RetirementStatus::AwaitingCreation(worker),
                },
                id,
                kind,
                rejection,
            ));
        }
        Ok(WorkerCustody::Returned {
            role,
            rejection,
            activation: worker.activation,
            stopped: worker.stopped,
        })
    }

    pub(in crate::atomic) fn accept_shutdown_settlement(
        self,
        settlement: EstablishedShutdownResolved<W::Protocol>,
    ) -> Result<
        ControlFlow<WorkerDeparture<Role, W, P>, Self>,
        (Self, EstablishedShutdownResolved<W::Protocol>),
    > {
        let Self { role, state } = self;
        let RetirementStatus::Established {
            current,
            startup,
            shutdown: WorkerShutdownStatus::Waiting(join),
        } = state
        else {
            return Err((Self { role, state }, settlement));
        };
        match join.settled(settlement) {
            Ok(ControlFlow::Continue(join)) => Ok(ControlFlow::Continue(Self {
                role,
                state: RetirementStatus::Established {
                    current,
                    startup,
                    shutdown: WorkerShutdownStatus::Waiting(join),
                },
            })),
            Ok(ControlFlow::Break((shutdown, _))) => Ok(ControlFlow::Break(WorkerDeparture {
                role,
                startup,
                shutdown,
            })),
            Err((join, settlement)) => Err((
                Self {
                    role,
                    state: RetirementStatus::Established {
                        current,
                        startup,
                        shutdown: WorkerShutdownStatus::Waiting(join),
                    },
                },
                settlement,
            )),
        }
    }
}

pub(in crate::atomic) fn accept_retiring_worker_preparation<Role, W, P, Source>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
) -> Result<
    (
        Vec<RetiringWorker<Role, W, P>>,
        WorkerRecoveryPreparation<Role, W, P, Source>,
    ),
    (
        Vec<RetiringWorker<Role, W, P>>,
        behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ),
>
where
    Role: Send + Sync,
    W: Behavior + Send,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
    Source: WorkerSource<Role, W, P>,
{
    let Some(position) =
        workers
            .iter()
            .enumerate()
            .find_map(|(position, worker)| match &worker.state {
                RetirementStatus::AwaitingPreparation { preparation, .. }
                    if preparation_result_accepts(&input, preparation) =>
                {
                    Some(position)
                }
                RetirementStatus::AwaitingCreation(_)
                | RetirementStatus::Established { .. }
                | RetirementStatus::AwaitingPreparation { .. }
                | RetirementStatus::AwaitingRestartSchedule { .. }
                | RetirementStatus::Drained => None,
            })
    else {
        return Err((workers, input));
    };
    let RetiringWorker { role, state } = workers.remove(position);
    let RetirementStatus::AwaitingPreparation {
        recoveries,
        previous,
        stopped,
        preparation,
    } = state
    else {
        workers.insert(position, RetiringWorker { role, state });
        return Err((workers, input));
    };
    let drained_role = role.clone();
    match RecoveringWorker::<W, P>::accept_preparation(
        role,
        recoveries,
        previous,
        stopped,
        preparation,
        input,
    ) {
        Ok(preparation) => {
            workers.insert(
                position,
                RetiringWorker {
                    role: drained_role,
                    state: RetirementStatus::Drained,
                },
            );
            Ok((workers, preparation))
        }
        Err((role, recoveries, previous, stopped, preparation, input)) => {
            workers.insert(
                position,
                RetiringWorker {
                    role,
                    state: RetirementStatus::AwaitingPreparation {
                        recoveries,
                        previous,
                        stopped,
                        preparation,
                    },
                },
            );
            Err((workers, input))
        }
    }
}

pub(in crate::atomic) fn accept_worker_initialization<Role, W, P>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    report: WorkerInitializationReport<W, P>,
) -> RetiringWorkerInitialization<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    let Some(position) = workers
        .iter()
        .position(|worker| worker.initialization() == Some(report.initialization()))
    else {
        return RetiringWorkerInitialization::Returned { workers, report };
    };
    let RetiringWorker { role, state } = workers.remove(position);
    let RetirementStatus::Established {
        current,
        startup: Some(WorkerStartupCustody::Initializing(expected)),
        shutdown: WorkerShutdownStatus::Waiting(join),
    } = state
    else {
        workers.insert(position, RetiringWorker { role, state });
        return RetiringWorkerInitialization::Returned { workers, report };
    };
    match report {
        WorkerInitializationReport::Stopped {
            worker,
            initialization,
            activation,
            stopped,
        } if stopped.child == current.attempt.creation() => {
            let result_role = role.clone();
            match join.stopped(stopped, &current.attempt) {
                Ok(ControlFlow::Continue(join)) => workers.insert(
                    position,
                    RetiringWorker {
                        role,
                        state: RetirementStatus::Established {
                            current,
                            startup: None,
                            shutdown: WorkerShutdownStatus::Waiting(join),
                        },
                    },
                ),
                Ok(ControlFlow::Break((shutdown, _))) => {
                    return RetiringWorkerInitialization::WorkerStopped {
                        workers,
                        departure: Some(WorkerDeparture {
                            role,
                            startup: None,
                            shutdown,
                        }),
                        role: result_role,
                        activation,
                    };
                }
                Err((join, stopped)) => {
                    workers.insert(
                        position,
                        RetiringWorker {
                            role,
                            state: RetirementStatus::Established {
                                current,
                                startup: Some(WorkerStartupCustody::Initializing(expected)),
                                shutdown: WorkerShutdownStatus::Waiting(join),
                            },
                        },
                    );
                    return RetiringWorkerInitialization::Returned {
                        workers,
                        report: WorkerInitializationReport::Stopped {
                            worker,
                            initialization,
                            activation,
                            stopped,
                        },
                    };
                }
            }
            RetiringWorkerInitialization::WorkerStopped {
                workers,
                departure: None,
                role: result_role,
                activation,
            }
        }
        report @ WorkerInitializationReport::Stopped { .. } => {
            workers.insert(
                position,
                RetiringWorker {
                    role,
                    state: RetirementStatus::Established {
                        current,
                        startup: Some(WorkerStartupCustody::Initializing(expected)),
                        shutdown: WorkerShutdownStatus::Waiting(join),
                    },
                },
            );
            RetiringWorkerInitialization::Returned { workers, report }
        }
        report => {
            workers.insert(
                position,
                RetiringWorker {
                    role,
                    state: RetirementStatus::Established {
                        current,
                        startup: None,
                        shutdown: WorkerShutdownStatus::Waiting(join),
                    },
                },
            );
            RetiringWorkerInitialization::Returned { workers, report }
        }
    }
}

pub(in crate::atomic) fn accept_worker_activation<Role, W, P>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    input: WorkerActivation<W, P>,
) -> RetiringWorkerActivation<Role, W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    let Some(position) = workers
        .iter()
        .position(|worker| worker.activation() == Some(input.attempt()))
    else {
        return RetiringWorkerActivation::Unrelated {
            workers,
            activation: input,
        };
    };
    let RetiringWorker { role, state } = workers.remove(position);
    let RetirementStatus::Established {
        current,
        startup,
        shutdown,
    } = state
    else {
        workers.insert(position, RetiringWorker { role, state });
        return RetiringWorkerActivation::Unrelated {
            workers,
            activation: input,
        };
    };
    match startup {
        Some(WorkerStartupCustody::ActivationStart(_)) => {
            let (worker, activation, outcome) = input.into_parts();
            match outcome {
                WorkerActivationOutcome::Started => {
                    workers.insert(
                        position,
                        RetiringWorker {
                            role,
                            state: RetirementStatus::Established {
                                current,
                                startup: Some(WorkerStartupCustody::Activation(activation)),
                                shutdown,
                            },
                        },
                    );
                    RetiringWorkerActivation::Started { workers }
                }
                outcome => {
                    workers.insert(
                        position,
                        RetiringWorker {
                            role: role.clone(),
                            state: RetirementStatus::Established {
                                current,
                                startup: None,
                                shutdown,
                            },
                        },
                    );
                    RetiringWorkerActivation::Returned {
                        workers,
                        role,
                        worker,
                        activation,
                        outcome,
                    }
                }
            }
        }
        Some(WorkerStartupCustody::Activation(expected)) => {
            let (worker, activation, outcome) = input.into_parts();
            let startup = match &outcome {
                WorkerActivationOutcome::Started => {
                    Some(WorkerStartupCustody::Activation(expected))
                }
                WorkerActivationOutcome::StartRejected { .. }
                | WorkerActivationOutcome::Ready(_)
                | WorkerActivationOutcome::Rejected(_) => None,
            };
            workers.insert(
                position,
                RetiringWorker {
                    role: role.clone(),
                    state: RetirementStatus::Established {
                        current,
                        startup,
                        shutdown,
                    },
                },
            );
            RetiringWorkerActivation::Returned {
                workers,
                role,
                worker,
                activation,
                outcome,
            }
        }
        startup => {
            workers.insert(
                position,
                RetiringWorker {
                    role,
                    state: RetirementStatus::Established {
                        current,
                        startup,
                        shutdown,
                    },
                },
            );
            RetiringWorkerActivation::Unrelated {
                workers,
                activation: input,
            }
        }
    }
}

pub(in crate::atomic) fn accept_worker_exit<Role, W, P>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    mut stopped: ChildStopped<BehaviorAddr<W>>,
) -> Result<
    ControlFlow<
        (Vec<RetiringWorker<Role, W, P>>, WorkerDeparture<Role, W, P>),
        Vec<RetiringWorker<Role, W, P>>,
    >,
    (
        Vec<RetiringWorker<Role, W, P>>,
        ChildStopped<BehaviorAddr<W>>,
    ),
>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    for position in 0..workers.len() {
        match workers.remove(position).accept_exit(stopped) {
            Ok(ControlFlow::Continue(worker)) => {
                workers.insert(position, worker);
                return Ok(ControlFlow::Continue(workers));
            }
            Ok(ControlFlow::Break(worker)) => {
                return Ok(ControlFlow::Break((workers, worker)));
            }
            Err((worker, returned)) => {
                stopped = returned;
                workers.insert(position, worker);
            }
        }
    }
    Err((workers, stopped))
}

pub(in crate::atomic) fn accept_worker_shutdown<Role, W, P>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    mut settlement: EstablishedShutdownResolved<W::Protocol>,
) -> Result<
    ControlFlow<
        (Vec<RetiringWorker<Role, W, P>>, WorkerDeparture<Role, W, P>),
        Vec<RetiringWorker<Role, W, P>>,
    >,
    (
        Vec<RetiringWorker<Role, W, P>>,
        EstablishedShutdownResolved<W::Protocol>,
    ),
>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    for position in 0..workers.len() {
        match workers
            .remove(position)
            .accept_shutdown_settlement(settlement)
        {
            Ok(ControlFlow::Continue(worker)) => {
                workers.insert(position, worker);
                return Ok(ControlFlow::Continue(workers));
            }
            Ok(ControlFlow::Break(worker)) => {
                return Ok(ControlFlow::Break((workers, worker)));
            }
            Err((worker, returned)) => {
                settlement = returned;
                workers.insert(position, worker);
            }
        }
    }
    Err((workers, settlement))
}

pub(in crate::atomic) struct MemberRetirement<Role, W, P, Job, Customer, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) member: RetiringWorker<Role, W, P>,
    pub(in crate::atomic) request: Option<ShutdownEstablished<StopOnShutdown<W>, Here>>,
    pub(in crate::atomic) custody: RetirementReturns<W, P, Job, Customer, WorkerResult>,
}

pub(in crate::atomic) enum RetirementReturns<W, P, Job, Customer, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    NoTransfer,
    Assignment(AssignmentShutdown<Job, Customer, WorkerResult, BehaviorAddr<W>>),
    Replacement(PendingWorkerReplacement<W, P>),
}

pub(in crate::atomic) enum WorkerCreationAdmission<Role, W, P, Job, WorkerResult, Customer>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Initializing {
        member: Member<Role, W, P, Job, WorkerResult, Customer>,
        request: InitializeWorker<W, P>,
    },
    Returned {
        member: Member<Role, W, P, Job, WorkerResult, Customer>,
        role: RoleName<Role>,
        rejection: WorkerCreationRejection<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Unrelated {
        member: Member<Role, W, P, Job, WorkerResult, Customer>,
        creation: ChildCreationSettled<StopOnShutdown<W>, ChildHead>,
    },
}

impl<Role, W, P, Job, WorkerResult, Customer> Member<Role, W, P, Job, WorkerResult, Customer>
where
    W: Behavior + crate::BehaviorBase,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
{
    pub(in crate::atomic) fn role_name(&self) -> RoleName<Role> {
        self.role.clone()
    }

    pub(in crate::atomic) fn worker_attempt(&self) -> Option<&WorkerAttempt> {
        match &self.state {
            MemberState::Worker(worker) => Some(&worker.current.attempt),
            MemberState::Creating(_) | MemberState::Recovering(_) | MemberState::Retired => None,
        }
    }

    pub(in crate::atomic) fn authorize_activation(
        self,
    ) -> Result<(Self, BeginActivation<W, P>), Self> {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        match state {
            MemberState::Worker(Worker {
                current,
                phase: WorkerPhase::WaitingForActivation { permit, activation },
            }) => {
                let request = BeginActivation::new(activation, permit);
                let attempt = request.attempt();
                Ok((
                    Self {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::ActivationDispatched {
                                attempt,
                                stopped: None,
                            },
                        }),
                    },
                    request,
                ))
            }
            state => Err(Self {
                role,
                recoveries,
                state,
            }),
        }
    }

    pub(in crate::atomic) fn accept_preparation<Source>(
        self,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Result<
        WorkerRecoveryPreparation<Role, W, P, Source>,
        (
            Self,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        ),
    >
    where
        Role: Eq + Send + Sync,
        W: Send,
        Source: WorkerSource<Role, W, P>,
    {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        match state {
            MemberState::Recovering(RecoveringWorker::Preparing {
                previous,
                stopped,
                preparation: expected,
            }) => {
                match RecoveringWorker::accept_preparation(
                    role, recoveries, previous, stopped, expected, input,
                ) {
                    Ok(accepted) => Ok(accepted),
                    Err((role, recoveries, previous, stopped, expected, input)) => Err((
                        Self {
                            role,
                            recoveries,
                            state: MemberState::Recovering(RecoveringWorker::Preparing {
                                previous,
                                stopped,
                                preparation: expected,
                            }),
                        },
                        input,
                    )),
                }
            }
            state => Err((
                Self {
                    role,
                    recoveries,
                    state,
                },
                input,
            )),
        }
    }

    pub(in crate::atomic) fn expected_creation(
        &self,
    ) -> Option<(behavior::CreationId, CreationKind)> {
        match &self.state {
            MemberState::Creating(worker) => Some((worker.attempt.creation(), worker.kind)),
            MemberState::Worker(_) | MemberState::Recovering(_) | MemberState::Retired => None,
        }
    }

    pub(in crate::atomic) fn shutdown_target(&self) -> Option<&CurrentWorker<W>> {
        let MemberState::Worker(worker) = &self.state else {
            return None;
        };
        match &worker.phase {
            WorkerPhase::Initializing { stopped: None, .. }
            | WorkerPhase::WaitingForActivation { .. }
            | WorkerPhase::ActivationDispatched { stopped: None, .. }
            | WorkerPhase::Activating { stopped: None, .. }
            | WorkerPhase::Idle => Some(&worker.current),
            WorkerPhase::Busy(assignment) => match assignment.observed_stop() {
                Some(_) => None,
                None => Some(&worker.current),
            },
            WorkerPhase::Initializing {
                stopped: Some(_), ..
            }
            | WorkerPhase::ActivationDispatched {
                stopped: Some(_), ..
            }
            | WorkerPhase::Activating {
                stopped: Some(_), ..
            }
            | WorkerPhase::Stopping(_) => None,
        }
    }

    pub(in crate::atomic) fn begin(
        prepared: PreparedWorker<Role, W, P>,
        creation: behavior::CreationId,
    ) -> (
        Self,
        CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>,
        crate::ObserveChild<W::Protocol, behavior::ChildHead>,
    ) {
        let PreparedWorker { role, submission } = prepared;
        let WorkerSubmission { worker, activation } = submission;
        let request = CreateChild::birth(creation, StopOnShutdown::new(worker));
        Self::start(
            RoleName::new(role),
            RecoveryCount::new(),
            activation,
            request,
        )
    }

    pub(in crate::atomic) fn begin_replacement(
        prepared: PreparedWorker<RoleName<Role>, W, P>,
        recoveries: RecoveryCount,
        previous: behavior::CreationId,
        creation: behavior::CreationId,
    ) -> (
        Self,
        CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>,
        crate::ObserveChild<W::Protocol, behavior::ChildHead>,
    ) {
        let PreparedWorker { role, submission } = prepared;
        let WorkerSubmission { worker, activation } = submission;
        let request = CreateChild::replacement(creation, previous, StopOnShutdown::new(worker));
        Self::start(role, recoveries, activation, request)
    }

    fn start(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        activation: P,
        request: CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> (
        Self,
        CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>,
        crate::ObserveChild<W::Protocol, behavior::ChildHead>,
    ) {
        let creation = request.id();
        let kind = request.kind();
        let attempt = WorkerAttempt::issued(creation);
        let initialization = InitializationAttempt::issued(&attempt);
        (
            Self {
                role,
                recoveries,
                state: MemberState::Creating(CreatingWorker {
                    attempt,
                    initialization,
                    activation,
                    kind,
                    stopped: None,
                }),
            },
            request,
            crate::ObserveChild::new(creation),
        )
    }

    pub(in crate::atomic) fn admit_creation(
        self,
        creation: ChildCreationSettled<StopOnShutdown<W>, ChildHead>,
    ) -> WorkerCreationAdmission<Role, W, P, Job, WorkerResult, Customer> {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        let MemberState::Creating(starting) = state else {
            return WorkerCreationAdmission::Unrelated {
                member: Self {
                    role,
                    recoveries,
                    state,
                },
                creation,
            };
        };
        let settlement = creation.into_settlement();
        if creation_identity(&settlement) != Some((starting.attempt.creation(), starting.kind)) {
            return WorkerCreationAdmission::Unrelated {
                member: Self {
                    role,
                    recoveries,
                    state: MemberState::Creating(starting),
                },
                creation: ChildCreationSettled::new(settlement),
            };
        }

        match settle_worker_creation(settlement) {
            WorkerCreationOutcome::Established(actor) => {
                let (member, request) =
                    Self::begin_initialization(role, recoveries, starting, actor);
                WorkerCreationAdmission::Initializing { member, request }
            }
            WorkerCreationOutcome::Rejected(rejection) => WorkerCreationAdmission::Returned {
                member: Self {
                    role: role.clone(),
                    recoveries,
                    state: MemberState::Retired,
                },
                role,
                rejection,
                activation: starting.activation,
                stopped: starting.stopped,
            },
            WorkerCreationOutcome::InvalidSettlement(settlement) => {
                WorkerCreationAdmission::Unrelated {
                    member: Self {
                        role,
                        recoveries,
                        state: MemberState::Creating(starting),
                    },
                    creation: ChildCreationSettled::new(settlement),
                }
            }
        }
    }

    pub(in crate::atomic) fn admit_successful_creation(
        self,
        creation: ChildCreationSettled<StopOnShutdown<W>, ChildHead>,
    ) -> Result<
        (Self, InitializeWorker<W, P>),
        (Self, ChildCreationSettled<StopOnShutdown<W>, ChildHead>),
    > {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        let MemberState::Creating(starting) = state else {
            return Err((
                Self {
                    role,
                    recoveries,
                    state,
                },
                creation,
            ));
        };
        let settlement = creation.into_settlement();
        match settlement {
            SettledItem::Attempted(ItemSettlement::Accepted(created)) => {
                match &created {
                    ChildCreationOutcome::Established {
                        established: EstablishedCreation::Installed { .. },
                    } => {}
                    _ => {
                        return Err((
                            Self {
                                role,
                                recoveries,
                                state: MemberState::Creating(starting),
                            },
                            ChildCreationSettled::new(SettledItem::Attempted(
                                ItemSettlement::Accepted(created),
                            )),
                        ));
                    }
                }
                match created.into_actor() {
                    Ok(actor) => Ok(Self::begin_initialization(
                        role, recoveries, starting, actor,
                    )),
                    Err(created) => Err((
                        Self {
                            role,
                            recoveries,
                            state: MemberState::Creating(starting),
                        },
                        ChildCreationSettled::new(SettledItem::Attempted(
                            ItemSettlement::Accepted(created),
                        )),
                    )),
                }
            }
            settlement => Err((
                Self {
                    role,
                    recoveries,
                    state: MemberState::Creating(starting),
                },
                ChildCreationSettled::new(settlement),
            )),
        }
    }

    fn begin_initialization(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        starting: CreatingWorker<W, P>,
        actor: EstablishedActor<StopOnShutdown<W>>,
    ) -> (Self, InitializeWorker<W, P>) {
        let current = CurrentWorker::new(starting.attempt.clone(), actor);
        let request = InitializeWorker::new(
            starting.attempt,
            starting.initialization.clone(),
            current.recipient(),
            starting.activation,
        );
        (
            Self {
                role,
                recoveries,
                state: MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::Initializing {
                        initialization: starting.initialization,
                        stopped: starting.stopped,
                    },
                }),
            },
            request,
        )
    }

    pub(in crate::atomic) fn admit_initialization(
        self,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<Self, (Self, WorkerInitializationReport<W, P>)> {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        let MemberState::Worker(Worker {
            current,
            phase:
                WorkerPhase::Initializing {
                    initialization,
                    stopped: None,
                },
        }) = state
        else {
            return Err((
                Self {
                    role,
                    recoveries,
                    state,
                },
                input,
            ));
        };
        match input {
            WorkerInitializationReport::ReadyForActivation {
                worker: _,
                initialization: _,
                activation,
                permit,
            } => Ok(Self {
                role,
                recoveries,
                state: MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::WaitingForActivation { permit, activation },
                }),
            }),
            input => Err((
                Self {
                    role,
                    recoveries,
                    state: MemberState::Worker(Worker {
                        current,
                        phase: WorkerPhase::Initializing {
                            initialization,
                            stopped: None,
                        },
                    }),
                },
                input,
            )),
        }
    }

    pub(in crate::atomic) fn admit_activation(
        self,
        input: WorkerActivation<W, P>,
    ) -> Result<Self, (Self, WorkerActivation<W, P>)> {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        let MemberState::Worker(Worker { current, phase }) = state else {
            return Err((
                Self {
                    role,
                    recoveries,
                    state,
                },
                input,
            ));
        };
        if current.attempt != input.worker() {
            return Err((
                Self {
                    role,
                    recoveries,
                    state: MemberState::Worker(Worker { current, phase }),
                },
                input,
            ));
        }
        match phase {
            WorkerPhase::ActivationDispatched { attempt, stopped } => match input.into_started() {
                Ok(()) => Ok(Self {
                    role,
                    recoveries,
                    state: MemberState::Worker(Worker {
                        current,
                        phase: WorkerPhase::Activating { attempt, stopped },
                    }),
                }),
                Err(input) => Err((
                    Self {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::ActivationDispatched { attempt, stopped },
                        }),
                    },
                    input,
                )),
            },
            WorkerPhase::Activating {
                attempt,
                stopped: None,
            } => match input.into_ready() {
                Ok(_) => Ok(Self {
                    role,
                    recoveries,
                    state: MemberState::Worker(Worker {
                        current,
                        phase: WorkerPhase::Idle,
                    }),
                }),
                Err(input) => Err((
                    Self {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Activating {
                                attempt,
                                stopped: None,
                            },
                        }),
                    },
                    input,
                )),
            },
            phase => Err((
                Self {
                    role,
                    recoveries,
                    state: MemberState::Worker(Worker { current, phase }),
                },
                input,
            )),
        }
    }

    pub(in crate::atomic) fn shutdown(
        self,
        shutdown: Option<ShutdownId>,
    ) -> MemberRetirement<Role, W, P, Job, Customer, WorkerResult>
    where
        <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        match state {
            MemberState::Creating(worker) => MemberRetirement {
                member: RetiringWorker {
                    role,
                    state: RetirementStatus::AwaitingCreation(worker),
                },
                request: None,
                custody: RetirementReturns::NoTransfer,
            },
            MemberState::Worker(Worker { current, phase }) => match phase {
                WorkerPhase::Initializing {
                    initialization,
                    stopped: None,
                } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::Initializing(initialization)),
                        shutdown,
                    );
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::WaitingForActivation { permit, activation } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::ActivationPermit { permit, activation }),
                        shutdown,
                    );
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::ActivationDispatched {
                    attempt,
                    stopped: None,
                } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::ActivationStart(attempt)),
                        shutdown,
                    );
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::Activating {
                    attempt,
                    stopped: None,
                } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::Activation(attempt)),
                        shutdown,
                    );
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::Idle => {
                    let (member, request) = retire_worker(role, current, None, shutdown);
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::Busy(assignment) => {
                    let assignment = assignment.shutdown();
                    match assignment.stopped {
                        Some(_) => MemberRetirement {
                            member: RetiringWorker {
                                role,
                                state: RetirementStatus::Drained,
                            },
                            request: None,
                            custody: RetirementReturns::Assignment(assignment),
                        },
                        None => {
                            let (member, request) = retire_worker(role, current, None, shutdown);
                            MemberRetirement {
                                member,
                                request,
                                custody: RetirementReturns::Assignment(assignment),
                            }
                        }
                    }
                }
                WorkerPhase::Stopping(join @ ShutdownJoin::AwaitingBoth(_))
                | WorkerPhase::Stopping(join @ ShutdownJoin::AwaitingStop(_)) => MemberRetirement {
                    member: RetiringWorker {
                        role,
                        state: RetirementStatus::Established {
                            current,
                            startup: None,
                            shutdown: WorkerShutdownStatus::Waiting(join),
                        },
                    },
                    request: None,
                    custody: RetirementReturns::NoTransfer,
                },
                WorkerPhase::Initializing {
                    stopped: Some(_), ..
                }
                | WorkerPhase::ActivationDispatched {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Activating {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Stopping(ShutdownJoin::AwaitingSettlement { .. }) => {
                    MemberRetirement {
                        member: RetiringWorker {
                            role,
                            state: RetirementStatus::Drained,
                        },
                        request: None,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
            },
            MemberState::Recovering(RecoveringWorker::Preparing {
                previous,
                stopped,
                preparation,
            }) => MemberRetirement {
                member: RetiringWorker {
                    role,
                    state: RetirementStatus::AwaitingPreparation {
                        recoveries,
                        previous,
                        stopped,
                        preparation,
                    },
                },
                request: None,
                custody: RetirementReturns::NoTransfer,
            },
            MemberState::Recovering(RecoveringWorker::Scheduling {
                previous,
                stopped,
                submission,
                timer,
            }) => MemberRetirement {
                member: RetiringWorker {
                    role,
                    state: RetirementStatus::AwaitingRestartSchedule {
                        replacement: PendingWorkerReplacement {
                            previous,
                            stopped,
                            submission,
                        },
                        timer,
                    },
                },
                request: None,
                custody: RetirementReturns::NoTransfer,
            },
            MemberState::Recovering(RecoveringWorker::WaitingForTimer {
                previous,
                stopped,
                submission,
                timer: _,
            }) => MemberRetirement {
                member: RetiringWorker {
                    role,
                    state: RetirementStatus::Drained,
                },
                request: None,
                custody: RetirementReturns::Replacement(PendingWorkerReplacement {
                    previous,
                    stopped,
                    submission,
                }),
            },
            MemberState::Recovering(RecoveringWorker::WaitingForSource { .. })
            | MemberState::Retired => MemberRetirement {
                member: RetiringWorker {
                    role,
                    state: RetirementStatus::Drained,
                },
                request: None,
                custody: RetirementReturns::NoTransfer,
            },
        }
    }
}

pub(in crate::atomic) fn retire_worker<Role, W, P>(
    role: RoleName<Role>,
    current: CurrentWorker<W>,
    startup: Option<WorkerStartupCustody<W, P>>,
    shutdown: Option<ShutdownId>,
) -> (
    RetiringWorker<Role, W, P>,
    Option<ShutdownEstablished<StopOnShutdown<W>, Here>>,
)
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
{
    match shutdown {
        Some(shutdown) => {
            let request = current.shutdown_request(shutdown);
            (
                RetiringWorker {
                    role,
                    state: RetirementStatus::Established {
                        current,
                        startup,
                        shutdown: WorkerShutdownStatus::Waiting(ShutdownJoin::AwaitingBoth(
                            shutdown,
                        )),
                    },
                },
                Some(request),
            )
        }
        None => (
            RetiringWorker {
                role,
                state: RetirementStatus::Established {
                    current,
                    startup,
                    shutdown: WorkerShutdownStatus::NotRequested,
                },
            },
            None,
        ),
    }
}

pub(in crate::atomic) fn creation_identity<W>(
    settlement: &WorkerCreationSettlement<W>,
) -> Option<(behavior::CreationId, CreationKind)>
where
    W: Behavior + BehaviorBase,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
{
    match settlement {
        SettledItem::Attempted(ItemSettlement::Accepted(created)) => match created {
            ChildCreationOutcome::Established {
                established: EstablishedCreation::Installed { id, kind, .. },
            } => Some((*id, *kind)),
            ChildCreationOutcome::Established {
                established: EstablishedCreation::Rejected { .. },
            } => None,
            ChildCreationOutcome::InitializationRejected { creation, .. }
            | ChildCreationOutcome::HostRejected { creation, .. } => {
                Some((creation.id(), creation.kind()))
            }
        },
        SettledItem::Attempted(ItemSettlement::Rejected { item, .. })
        | SettledItem::Attempted(ItemSettlement::Corrupt { item, .. })
        | SettledItem::Unattempted(item) => Some((item.id(), item.kind())),
        SettledItem::Attempted(ItemSettlement::Blocked { .. }) => None,
    }
}

pub(in crate::atomic) fn ordered_creation_positions(
    expected: impl IntoIterator<Item = Option<(behavior::CreationId, CreationKind)>>,
    identities: impl IntoIterator<Item = (behavior::CreationId, CreationKind)>,
) -> Option<Vec<usize>> {
    let expected: Vec<_> = expected.into_iter().collect();
    let mut positions = Vec::new();
    for identity in identities {
        let position = expected
            .iter()
            .position(|expected| *expected == Some(identity))?;
        match positions.last() {
            Some(previous) if position <= *previous => return None,
            Some(_) | None => positions.push(position),
        }
    }
    match positions.first() {
        Some(_) => Some(positions),
        None => None,
    }
}
