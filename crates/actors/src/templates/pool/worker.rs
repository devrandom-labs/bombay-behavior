//! One semantic role and its current direct-worker relationship.

use core::ops::ControlFlow;

use behavior::{
    Behavior, BehaviorAddr, BehaviorBase, ChildCreationOutcome, ChildCreationSettled, ChildHead,
    ChildNamespaceExhausted, CreateChild, CreationKind, CreationSettlement, Creations,
    CreationsSettled, EndpointAddress, EstablishedActor, EstablishedRecipient, Here, Ingress,
    InjectEvent, InterpreterFault, ItemSettlement, SettledItem,
};

use crate::{
    ChildStopped, EstablishedShutdownResolved, ShutdownEstablished, ShutdownId, ShutdownRequested,
    StopOnShutdown,
};

use super::super::restart::RecoveryCount;
use super::super::schedule::ScheduleKey;
use super::super::worker::{
    WorkerCreationOutcome, WorkerCreationSettlement, settle_worker_creation,
};
use super::super::worker::{
    WorkerPreparationExpectation, WorkerPreparationOutcome, WorkerStartupGrant,
    WorkerStartupOutcome,
};
use super::assignment::{AssignedJob, AssignmentShutdown};
use crate::templates::roster::RoleName;
use crate::worker::{
    InitializationAttempt, InitializeWorker, PrepareWorkers, PreparedWorker, StartWorker,
    StartupPermit, StartupPlan, WorkerAttempt, WorkerCreationRejection, WorkerInitializationReport,
    WorkerPreparation, WorkerSource, WorkerStartup, WorkerSubmission,
};

#[derive(Clone, Copy)]
pub(in crate::templates) enum CreationBatchRejection {
    NamespaceExhausted,
    InterpreterCorrupt(InterpreterFault),
}

impl CreationBatchRejection {
    pub(in crate::templates) fn worker<W>(self, worker: W) -> WorkerCreationRejection<W>
    where
        W: Behavior,
    {
        match self {
            Self::NamespaceExhausted => WorkerCreationRejection::NamespaceExhausted { worker },
            Self::InterpreterCorrupt(fault) => {
                WorkerCreationRejection::InterpreterCorrupt { worker, fault }
            }
        }
    }

    pub(in crate::templates) fn settlement<W>(
        self,
        creations: Creations<CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>>,
    ) -> CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>
    where
        W: Behavior + BehaviorBase,
        BehaviorAddr<W>: EndpointAddress,
        StopOnShutdown<W>:
            Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    {
        match self {
            Self::NamespaceExhausted => CreationsSettled::new(CreationSettlement::Rejected {
                creations,
                reason: ChildNamespaceExhausted,
            }),
            Self::InterpreterCorrupt(fault) => {
                CreationsSettled::new(CreationSettlement::Corrupt { creations, fault })
            }
        }
    }
}

#[expect(
    dead_code,
    reason = "the emitted diagnostic retains the exact preparation failure"
)]
pub(in crate::templates) enum WorkerPreparationError<WorkerRejection, SourceRejection> {
    WorkerRejected(WorkerRejection),
    SourceRejected(SourceRejection),
    InterpreterCorrupt(behavior::InterpreterFault),
    InterpretationSkipped,
    SourceStateCorrupt,
}

pub(in crate::templates) struct CurrentWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) attempt: WorkerAttempt,
    actor: EstablishedActor<StopOnShutdown<W>>,
}

impl<W> CurrentWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
{
    pub(in crate::templates) const fn new(
        attempt: WorkerAttempt,
        actor: EstablishedActor<StopOnShutdown<W>>,
    ) -> Self {
        Self { attempt, actor }
    }

    pub(in crate::templates) fn recipient(&self) -> EstablishedRecipient<W::Protocol> {
        self.actor.recipient()
    }

    pub(in crate::templates) fn shutdown_request(
        &self,
        id: ShutdownId,
    ) -> ShutdownEstablished<StopOnShutdown<W>, Here>
    where
        <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    {
        ShutdownEstablished::new(id, self.actor.clone(), Ingress::new())
    }
}

pub(in crate::templates) struct CreatingWorker<W, P>
where
    W: Behavior,
{
    pub(in crate::templates) attempt: WorkerAttempt,
    pub(in crate::templates) initialization: InitializationAttempt,
    pub(in crate::templates) startup: P,
    pub(in crate::templates) kind: behavior::CreationKind,
    pub(in crate::templates) stopped: Option<ChildStopped<BehaviorAddr<W>>>,
}

pub(in crate::templates) enum WorkerPhase<W, P, Assigned>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Initializing {
        initialization: InitializationAttempt,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    WaitingForStartup {
        permit: StartupPermit<W>,
        startup: P,
    },
    StartupRequested {
        attempt: WorkerStartupGrant<W, P>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Starting {
        attempt: WorkerStartupGrant<W, P>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Idle,
    Busy(Assigned),
    Stopping(ShutdownJoin<W::Protocol, BehaviorAddr<W>>),
}

pub(in crate::templates) struct Worker<W, P, Assigned>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) current: CurrentWorker<W>,
    pub(in crate::templates) phase: WorkerPhase<W, P, Assigned>,
}

pub(in crate::templates) enum RecoveringWorker<W, P>
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
        preparation: WorkerPreparationExpectation,
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

pub(in crate::templates) struct PreparedReplacement<Role, W, P, Source>
where
    W: Behavior,
{
    pub(in crate::templates) role: RoleName<Role>,
    pub(in crate::templates) recoveries: RecoveryCount,
    pub(in crate::templates) previous: WorkerAttempt,
    pub(in crate::templates) stopped: ChildStopped<BehaviorAddr<W>>,
    pub(in crate::templates) source: Source,
    pub(in crate::templates) submission: WorkerSubmission<W, P>,
}

#[expect(
    dead_code,
    reason = "the emitted pool diagnostic retains the exact replacement failure"
)]
pub(in crate::templates) enum WorkerReplacementError {
    PoolShutdown,
    RestartDenied(super::super::restart::RestartDenial),
    WorkerCreationsExhausted,
    RestartTimersExhausted,
    RestartScheduleReturned(behavior::ActionItemResult<crate::ScheduleAfter>),
    SourceStateCorrupt,
}

pub(in crate::templates) enum WorkerReplacementRelease {
    Now(behavior::CreationId),
    After {
        timer: ScheduleKey,
        delay: core::time::Duration,
    },
}

pub(in crate::templates) struct PendingWorkerReplacement<W, P>
where
    W: Behavior,
{
    pub(in crate::templates) previous: WorkerAttempt,
    pub(in crate::templates) stopped: ChildStopped<BehaviorAddr<W>>,
    pub(in crate::templates) submission: WorkerSubmission<W, P>,
}

pub(in crate::templates) enum WorkerRecoveryPreparation<Role, W, P, Source>
where
    Role: Send + Sync,
    W: Behavior + Send,
    BehaviorAddr<W>: EndpointAddress,
    Source: WorkerSource<Role, W, P>,
    P: StartupPlan,
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

pub(in crate::templates) enum WorkerPreparationStartFault {
    InterpreterCorrupt(behavior::InterpreterFault),
    InterpretationSkipped,
}

pub(in crate::templates) struct FailedWorkerPreparation<Role, W, Source>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) role: RoleName<Role>,
    pub(in crate::templates) recoveries: RecoveryCount,
    pub(in crate::templates) previous: WorkerAttempt,
    pub(in crate::templates) stopped: ChildStopped<BehaviorAddr<W>>,
    pub(in crate::templates) source: Source,
    pub(in crate::templates) fault: WorkerPreparationStartFault,
}

impl<W, P> RecoveringWorker<W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) fn accepts_restart_schedule(
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

    pub(in crate::templates) fn accepts_restart_timer(
        &self,
        elapsed: &crate::TimerElapsed,
    ) -> bool {
        match self {
            Self::WaitingForTimer { timer, .. } => {
                timer.id == elapsed.id && timer.generation == elapsed.generation
            }
            Self::WaitingForSource { .. } | Self::Preparing { .. } | Self::Scheduling { .. } => {
                false
            }
        }
    }

    pub(in crate::templates) fn admit_restart_schedule(
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

    pub(in crate::templates) fn admit_restart_timer(
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

    pub(in crate::templates) fn accept_preparation_start<Source, Role>(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        expected: WorkerPreparationExpectation,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Result<
        ControlFlow<
            FailedWorkerPreparation<Role, W, Source>,
            (
                RoleName<Role>,
                RecoveryCount,
                WorkerAttempt,
                ChildStopped<BehaviorAddr<W>>,
                WorkerPreparationExpectation,
            ),
        >,
        (
            RoleName<Role>,
            RecoveryCount,
            WorkerAttempt,
            ChildStopped<BehaviorAddr<W>>,
            WorkerPreparationExpectation,
            behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
        ),
    >
    where
        Role: Send + Sync,
        W: Send,
        Source: WorkerSource<Role, W, P>,
    {
        if !expected.accepts_issued(&input) {
            return Err((role, recoveries, previous, stopped, expected, input));
        }
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(started)) => {
                match expected.accept_start(&started) {
                    Ok(expected) => Ok(ControlFlow::Continue((
                        role, recoveries, previous, stopped, expected,
                    ))),
                    Err(expected) => Err((
                        role,
                        recoveries,
                        previous,
                        stopped,
                        expected,
                        SettledItem::Attempted(ItemSettlement::Accepted(started)),
                    )),
                }
            }
            SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }) => {
                let (source, _, _) = item.into_parts();
                Ok(ControlFlow::Break(FailedWorkerPreparation {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    fault: WorkerPreparationStartFault::InterpreterCorrupt(fault),
                }))
            }
            SettledItem::Unattempted(item) => {
                let (source, _, _) = item.into_parts();
                Ok(ControlFlow::Break(FailedWorkerPreparation {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    fault: WorkerPreparationStartFault::InterpretationSkipped,
                }))
            }
            SettledItem::Attempted(ItemSettlement::Rejected { reason, .. }) => match reason {},
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match prerequisite {}
            }
        }
    }

    pub(in crate::templates) fn accept_preparation_return<Source, Role>(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        previous: WorkerAttempt,
        stopped: ChildStopped<BehaviorAddr<W>>,
        expected: WorkerPreparationExpectation,
        returned: WorkerPreparation<Source, Role, W, P>,
    ) -> Result<
        WorkerRecoveryPreparation<Role, W, P, Source>,
        (
            RoleName<Role>,
            RecoveryCount,
            WorkerAttempt,
            ChildStopped<BehaviorAddr<W>>,
            WorkerPreparationExpectation,
            WorkerPreparation<Source, Role, W, P>,
        ),
    >
    where
        Role: Send + Sync,
        W: Send,
        Source: WorkerSource<Role, W, P>,
    {
        if !expected.accepts_return(&returned) {
            return Err((role, recoveries, previous, stopped, expected, returned));
        }
        let (ticket, outcome) = returned.into_parts();
        match outcome {
            WorkerPreparationOutcome::Prepared {
                source,
                mut members,
            } => match members.pop() {
                Some(prepared) => Ok(WorkerRecoveryPreparation::Ready(PreparedReplacement {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    submission: prepared.submission,
                })),
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
                        WorkerPreparation::from_parts(
                            ticket,
                            WorkerPreparationOutcome::Prepared { source, members },
                        ),
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
            WorkerPreparationOutcome::SourceRejected { source, reason, .. } => {
                Ok(WorkerRecoveryPreparation::Failed {
                    role,
                    recoveries,
                    previous,
                    stopped,
                    source,
                    error: WorkerPreparationError::SourceRejected(reason),
                })
            }
        }
    }
}

pub(in crate::templates) enum ShutdownJoin<P, A>
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
    pub(in crate::templates) const fn request(&self) -> ShutdownId {
        match self {
            Self::AwaitingBoth(request) | Self::AwaitingSettlement { request, .. } => *request,
            Self::AwaitingStop(settlement) => settlement.id(),
        }
    }

    pub(in crate::templates) fn settled(
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

    pub(in crate::templates) fn stopped(
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

pub(in crate::templates) enum MemberState<W, P, Job, WorkerResult, Customer>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Creating(CreatingWorker<W, P>),
    Worker(Worker<W, P, AssignedJob<Job, Customer, WorkerResult, BehaviorAddr<W>>>),
    Recovering(RecoveringWorker<W, P>),
    Retired,
}

pub(in crate::templates) struct Member<Role, W, P, Job, WorkerResult, Customer>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) role: RoleName<Role>,
    pub(in crate::templates) recoveries: RecoveryCount,
    pub(in crate::templates) state: MemberState<W, P, Job, WorkerResult, Customer>,
}

pub(in crate::templates) enum WorkerStartupCustody<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    StartupPlan(P),
    Initializing(InitializationAttempt),
    StartupPermit {
        permit: StartupPermit<W>,
        startup: P,
    },
    StartupRequested(WorkerStartupGrant<W, P>),
    Starting(WorkerStartupGrant<W, P>),
}

pub(in crate::templates) enum WorkerShutdownStatus<P, A>
where
    P: behavior::Protocol,
    A: behavior::Address,
{
    NotRequested,
    Waiting(ShutdownJoin<P, A>),
}

pub(in crate::templates) enum RetirementStatus<W, P>
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
        preparation: WorkerPreparationExpectation,
    },
    AwaitingRestartSchedule {
        replacement: PendingWorkerReplacement<W, P>,
        timer: ScheduleKey,
    },
    Drained,
}

pub(in crate::templates) struct RetiringWorker<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) role: RoleName<Role>,
    pub(in crate::templates) state: RetirementStatus<W, P>,
}

pub(in crate::templates) struct WorkerDeparture<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) role: RoleName<Role>,
    pub(in crate::templates) startup: Option<WorkerStartupCustody<W, P>>,
    pub(in crate::templates) shutdown: EstablishedShutdownResolved<W::Protocol>,
}

pub(in crate::templates) enum WorkerCustody<Role, W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Running {
        role: RoleName<Role>,
        worker: CurrentWorker<W>,
        startup: P,
    },
    Rejected {
        role: RoleName<Role>,
        rejection: WorkerCreationRejection<W>,
        startup: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Stopped {
        role: RoleName<Role>,
        worker: CurrentWorker<W>,
        startup: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

pub(in crate::templates) enum RetiringWorkerInitialization<Role, W, P>
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
        startup: P,
    },
}

pub(in crate::templates) enum RetiringWorkerStartup<Role, W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    Started {
        workers: Vec<RetiringWorker<Role, W, P>>,
    },
    Returned {
        workers: Vec<RetiringWorker<Role, W, P>>,
        role: RoleName<Role>,
        worker: WorkerAttempt,
        startup: WorkerStartupGrant<W, P>,
        outcome: WorkerStartupOutcome<W, P>,
    },
    Unrelated {
        workers: Vec<RetiringWorker<Role, W, P>>,
        startup: WorkerStartup<W, P>,
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

    fn startup(&self) -> Option<&WorkerStartupGrant<W, P>> {
        match &self.state {
            RetirementStatus::Established {
                startup:
                    Some(
                        WorkerStartupCustody::StartupRequested(startup)
                        | WorkerStartupCustody::Starting(startup),
                    ),
                ..
            } => Some(startup),
            _ => None,
        }
    }

    pub(in crate::templates) fn expected_creation(
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

    pub(in crate::templates) fn waiting(&self) -> Option<&RetirementStatus<W, P>> {
        match &self.state {
            state @ (RetirementStatus::AwaitingCreation(_)
            | RetirementStatus::Established { .. }
            | RetirementStatus::AwaitingPreparation { .. }
            | RetirementStatus::AwaitingRestartSchedule { .. }) => Some(state),
            RetirementStatus::Drained => None,
        }
    }

    pub(in crate::templates) fn accept_exit(
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

    pub(in crate::templates) fn accept_creation(
        self,
        creation: ChildCreationSettled<StopOnShutdown<W>, ChildHead>,
    ) -> Result<WorkerCustody<Role, W, P>, (Self, ChildCreationSettled<StopOnShutdown<W>, ChildHead>)>
    where
        W: BehaviorBase,
        P: StartupPlan,
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
                        startup: worker.startup,
                        stopped,
                    }),
                    None => Ok(WorkerCustody::Running {
                        role,
                        worker: current,
                        startup: worker.startup,
                    }),
                }
            }
            WorkerCreationOutcome::Rejected(rejection) => Ok(WorkerCustody::Rejected {
                role,
                rejection,
                startup: worker.startup,
                stopped: worker.stopped,
            }),
        }
    }

    pub(in crate::templates) fn accept_creation_rejection(
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
        Ok(WorkerCustody::Rejected {
            role,
            rejection,
            startup: worker.startup,
            stopped: worker.stopped,
        })
    }

    pub(in crate::templates) fn accept_shutdown_settlement(
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

pub(in crate::templates) fn accept_retiring_worker_preparation_start<Role, W, P, Source>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
) -> Result<
    ControlFlow<
        (
            Vec<RetiringWorker<Role, W, P>>,
            FailedWorkerPreparation<Role, W, Source>,
        ),
        Vec<RetiringWorker<Role, W, P>>,
    >,
    (
        Vec<RetiringWorker<Role, W, P>>,
        behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ),
>
where
    Role: Send + Sync,
    W: Behavior + Send,
    BehaviorAddr<W>: EndpointAddress,
    P: StartupPlan,
    Source: WorkerSource<Role, W, P>,
{
    let Some(position) = workers.iter().position(|worker| match &worker.state {
        RetirementStatus::AwaitingPreparation { preparation, .. } => {
            preparation.accepts_issued(&input)
        }
        RetirementStatus::AwaitingCreation(_)
        | RetirementStatus::Established { .. }
        | RetirementStatus::AwaitingRestartSchedule { .. }
        | RetirementStatus::Drained => false,
    }) else {
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
    match RecoveringWorker::<W, P>::accept_preparation_start(
        role,
        recoveries,
        previous,
        stopped,
        preparation,
        input,
    ) {
        Ok(ControlFlow::Continue((role, recoveries, previous, stopped, preparation))) => {
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
            Ok(ControlFlow::Continue(workers))
        }
        Ok(ControlFlow::Break(failed)) => {
            workers.insert(
                position,
                RetiringWorker {
                    role: drained_role,
                    state: RetirementStatus::Drained,
                },
            );
            Ok(ControlFlow::Break((workers, failed)))
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

pub(in crate::templates) fn accept_retiring_worker_preparation_return<Role, W, P, Source>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    returned: WorkerPreparation<Source, Role, W, P>,
) -> Result<
    (
        Vec<RetiringWorker<Role, W, P>>,
        WorkerRecoveryPreparation<Role, W, P, Source>,
    ),
    (
        Vec<RetiringWorker<Role, W, P>>,
        WorkerPreparation<Source, Role, W, P>,
    ),
>
where
    Role: Send + Sync,
    W: Behavior + Send,
    BehaviorAddr<W>: EndpointAddress,
    P: StartupPlan,
    Source: WorkerSource<Role, W, P>,
{
    let Some(position) =
        workers
            .iter()
            .enumerate()
            .find_map(|(position, worker)| match &worker.state {
                RetirementStatus::AwaitingPreparation { preparation, .. }
                    if preparation.accepts_return(&returned) =>
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
        return Err((workers, returned));
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
        return Err((workers, returned));
    };
    let drained_role = role.clone();
    match RecoveringWorker::<W, P>::accept_preparation_return(
        role,
        recoveries,
        previous,
        stopped,
        preparation,
        returned,
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
        Err((role, recoveries, previous, stopped, preparation, returned)) => {
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
            Err((workers, returned))
        }
    }
}

pub(in crate::templates) fn accept_worker_initialization<Role, W, P>(
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
            startup,
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
                        startup,
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
                            startup,
                            stopped,
                        },
                    };
                }
            }
            RetiringWorkerInitialization::WorkerStopped {
                workers,
                departure: None,
                role: result_role,
                startup,
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

pub(in crate::templates) fn accept_worker_activation<Role, W, P>(
    mut workers: Vec<RetiringWorker<Role, W, P>>,
    input: WorkerStartup<W, P>,
) -> RetiringWorkerStartup<Role, W, P>
where
    W: Behavior,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    let Some(position) = workers
        .iter()
        .position(|worker| worker.startup() == Some(input.attempt()))
    else {
        return RetiringWorkerStartup::Unrelated {
            workers,
            startup: input,
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
        return RetiringWorkerStartup::Unrelated {
            workers,
            startup: input,
        };
    };
    match startup {
        Some(WorkerStartupCustody::StartupRequested(_)) => {
            let (worker, startup, outcome) = input.into_parts();
            match outcome {
                WorkerStartupOutcome::Started => {
                    workers.insert(
                        position,
                        RetiringWorker {
                            role,
                            state: RetirementStatus::Established {
                                current,
                                startup: Some(WorkerStartupCustody::Starting(startup)),
                                shutdown,
                            },
                        },
                    );
                    RetiringWorkerStartup::Started { workers }
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
                    RetiringWorkerStartup::Returned {
                        workers,
                        role,
                        worker,
                        startup,
                        outcome,
                    }
                }
            }
        }
        Some(WorkerStartupCustody::Starting(expected)) => {
            let (worker, grant, outcome) = input.into_parts();
            let startup = match &outcome {
                WorkerStartupOutcome::Started => Some(WorkerStartupCustody::Starting(expected)),
                WorkerStartupOutcome::StartRejected { .. }
                | WorkerStartupOutcome::Ready(_)
                | WorkerStartupOutcome::Rejected(_) => None,
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
            RetiringWorkerStartup::Returned {
                workers,
                role,
                worker,
                startup: grant,
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
            RetiringWorkerStartup::Unrelated {
                workers,
                startup: input,
            }
        }
    }
}

pub(in crate::templates) fn accept_worker_exit<Role, W, P>(
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

pub(in crate::templates) fn accept_worker_shutdown<Role, W, P>(
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

pub(in crate::templates) struct MemberRetirement<Role, W, P, Job, Customer, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) member: RetiringWorker<Role, W, P>,
    pub(in crate::templates) request: Option<ShutdownEstablished<StopOnShutdown<W>, Here>>,
    pub(in crate::templates) custody: RetirementReturns<W, P, Job, Customer, WorkerResult>,
}

pub(in crate::templates) enum RetirementReturns<W, P, Job, Customer, WorkerResult>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    NoTransfer,
    Assignment(AssignmentShutdown<Job, Customer, WorkerResult, BehaviorAddr<W>>),
    Replacement(PendingWorkerReplacement<W, P>),
}

pub(in crate::templates) enum WorkerCreationAdmission<Role, W, P, Job, WorkerResult, Customer>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Initializing {
        member: Member<Role, W, P, Job, WorkerResult, Customer>,
        request: InitializeWorker<W, P>,
    },
    Rejected {
        member: Member<Role, W, P, Job, WorkerResult, Customer>,
        role: RoleName<Role>,
        rejection: WorkerCreationRejection<W>,
        startup: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Unrelated {
        member: Member<Role, W, P, Job, WorkerResult, Customer>,
        creation: ChildCreationSettled<StopOnShutdown<W>, ChildHead>,
    },
}

impl<Role, W, P, Job, WorkerResult, Customer> Member<Role, W, P, Job, WorkerResult, Customer>
where
    W: Behavior + behavior::BehaviorBase,
    P: StartupPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
{
    pub(in crate::templates) fn role_name(&self) -> RoleName<Role> {
        self.role.clone()
    }

    pub(in crate::templates) fn worker_attempt(&self) -> Option<&WorkerAttempt> {
        match &self.state {
            MemberState::Worker(worker) => Some(&worker.current.attempt),
            MemberState::Creating(_) | MemberState::Recovering(_) | MemberState::Retired => None,
        }
    }

    pub(in crate::templates) fn authorize_activation(
        self,
    ) -> Result<(Self, StartWorker<W, P>), Self> {
        let Self {
            role,
            recoveries,
            state,
        } = self;
        match state {
            MemberState::Worker(Worker {
                current,
                phase: WorkerPhase::WaitingForStartup { permit, startup },
            }) => {
                let request = StartWorker::new(startup, permit);
                let attempt = request.attempt();
                Ok((
                    Self {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::StartupRequested {
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

    pub(in crate::templates) fn accept_preparation_start<Source>(
        self,
        input: behavior::ActionItemResult<PrepareWorkers<Source, Role, W, P>>,
    ) -> Result<
        ControlFlow<FailedWorkerPreparation<Role, W, Source>, Self>,
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
            }) => match RecoveringWorker::accept_preparation_start(
                role, recoveries, previous, stopped, expected, input,
            ) {
                Ok(ControlFlow::Continue((role, recoveries, previous, stopped, preparation))) => {
                    Ok(ControlFlow::Continue(Self {
                        role,
                        recoveries,
                        state: MemberState::Recovering(RecoveringWorker::Preparing {
                            previous,
                            stopped,
                            preparation,
                        }),
                    }))
                }
                Ok(ControlFlow::Break(failed)) => Ok(ControlFlow::Break(failed)),
                Err((role, recoveries, previous, stopped, preparation, input)) => Err((
                    Self {
                        role,
                        recoveries,
                        state: MemberState::Recovering(RecoveringWorker::Preparing {
                            previous,
                            stopped,
                            preparation,
                        }),
                    },
                    input,
                )),
            },
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

    pub(in crate::templates) fn accept_preparation_return<Source>(
        self,
        returned: WorkerPreparation<Source, Role, W, P>,
    ) -> Result<
        WorkerRecoveryPreparation<Role, W, P, Source>,
        (Self, WorkerPreparation<Source, Role, W, P>),
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
                preparation,
            }) => match RecoveringWorker::accept_preparation_return(
                role,
                recoveries,
                previous,
                stopped,
                preparation,
                returned,
            ) {
                Ok(accepted) => Ok(accepted),
                Err((role, recoveries, previous, stopped, preparation, returned)) => Err((
                    Self {
                        role,
                        recoveries,
                        state: MemberState::Recovering(RecoveringWorker::Preparing {
                            previous,
                            stopped,
                            preparation,
                        }),
                    },
                    returned,
                )),
            },
            state => Err((
                Self {
                    role,
                    recoveries,
                    state,
                },
                returned,
            )),
        }
    }

    pub(in crate::templates) fn expected_creation(
        &self,
    ) -> Option<(behavior::CreationId, CreationKind)> {
        match &self.state {
            MemberState::Creating(worker) => Some((worker.attempt.creation(), worker.kind)),
            MemberState::Worker(_) | MemberState::Recovering(_) | MemberState::Retired => None,
        }
    }

    pub(in crate::templates) fn shutdown_target(&self) -> Option<&CurrentWorker<W>> {
        let MemberState::Worker(worker) = &self.state else {
            return None;
        };
        match &worker.phase {
            WorkerPhase::Initializing { stopped: None, .. }
            | WorkerPhase::WaitingForStartup { .. }
            | WorkerPhase::StartupRequested { stopped: None, .. }
            | WorkerPhase::Starting { stopped: None, .. }
            | WorkerPhase::Idle => Some(&worker.current),
            WorkerPhase::Busy(assignment) => match assignment.observed_stop() {
                Some(_) => None,
                None => Some(&worker.current),
            },
            WorkerPhase::Initializing {
                stopped: Some(_), ..
            }
            | WorkerPhase::StartupRequested {
                stopped: Some(_), ..
            }
            | WorkerPhase::Starting {
                stopped: Some(_), ..
            }
            | WorkerPhase::Stopping(_) => None,
        }
    }

    pub(in crate::templates) fn begin(
        prepared: PreparedWorker<Role, W, P>,
        creation: behavior::CreationId,
    ) -> (
        Self,
        CreateChild<BehaviorAddr<W>, StopOnShutdown<W>>,
        crate::ObserveChild<W::Protocol, behavior::ChildHead>,
    ) {
        let PreparedWorker { role, submission } = prepared;
        let WorkerSubmission { worker, startup } = submission;
        let request = CreateChild::birth(creation, StopOnShutdown::new(worker));
        Self::start(RoleName::new(role), RecoveryCount::new(), startup, request)
    }

    pub(in crate::templates) fn begin_replacement(
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
        let WorkerSubmission { worker, startup } = submission;
        let request = CreateChild::replacement(creation, previous, StopOnShutdown::new(worker));
        Self::start(role, recoveries, startup, request)
    }

    fn start(
        role: RoleName<Role>,
        recoveries: RecoveryCount,
        startup: P,
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
                    startup,
                    kind,
                    stopped: None,
                }),
            },
            request,
            crate::ObserveChild::new(creation),
        )
    }

    pub(in crate::templates) fn admit_creation(
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
            WorkerCreationOutcome::Rejected(rejection) => WorkerCreationAdmission::Rejected {
                member: Self {
                    role: role.clone(),
                    recoveries,
                    state: MemberState::Retired,
                },
                role,
                rejection,
                startup: starting.startup,
                stopped: starting.stopped,
            },
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
            starting.startup,
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

    pub(in crate::templates) fn admit_initialization(
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
            WorkerInitializationReport::ReadyForStartup {
                worker: _,
                initialization: _,
                startup,
                permit,
            } => Ok(Self {
                role,
                recoveries,
                state: MemberState::Worker(Worker {
                    current,
                    phase: WorkerPhase::WaitingForStartup { permit, startup },
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

    pub(in crate::templates) fn admit_activation(
        self,
        input: WorkerStartup<W, P>,
    ) -> Result<Self, (Self, WorkerStartup<W, P>)> {
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
            WorkerPhase::StartupRequested { attempt, stopped } if &attempt == input.attempt() => {
                match input.into_started() {
                    Ok(()) => Ok(Self {
                        role,
                        recoveries,
                        state: MemberState::Worker(Worker {
                            current,
                            phase: WorkerPhase::Starting { attempt, stopped },
                        }),
                    }),
                    Err(input) => Err((
                        Self {
                            role,
                            recoveries,
                            state: MemberState::Worker(Worker {
                                current,
                                phase: WorkerPhase::StartupRequested { attempt, stopped },
                            }),
                        },
                        input,
                    )),
                }
            }
            WorkerPhase::Starting {
                attempt,
                stopped: None,
            } if &attempt == input.attempt() => match input.into_ready() {
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
                            phase: WorkerPhase::Starting {
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

    pub(in crate::templates) fn shutdown(
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
                WorkerPhase::WaitingForStartup { permit, startup } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::StartupPermit { permit, startup }),
                        shutdown,
                    );
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::StartupRequested {
                    attempt,
                    stopped: None,
                } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::StartupRequested(attempt)),
                        shutdown,
                    );
                    MemberRetirement {
                        member,
                        request,
                        custody: RetirementReturns::NoTransfer,
                    }
                }
                WorkerPhase::Starting {
                    attempt,
                    stopped: None,
                } => {
                    let (member, request) = retire_worker(
                        role,
                        current,
                        Some(WorkerStartupCustody::Starting(attempt)),
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
                | WorkerPhase::StartupRequested {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Starting {
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

pub(in crate::templates) fn retire_worker<Role, W, P>(
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

pub(in crate::templates) fn creation_identity<W>(
    settlement: &WorkerCreationSettlement<W>,
) -> Option<(behavior::CreationId, CreationKind)>
where
    W: Behavior + BehaviorBase,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
{
    match settlement {
        SettledItem::Attempted(ItemSettlement::Accepted(created)) => match created {
            ChildCreationOutcome::Established(child) => Some((child.id(), child.kind())),
            ChildCreationOutcome::Unavailable { id, kind } => Some((*id, *kind)),
            ChildCreationOutcome::InitializationRejected { creation, .. }
            | ChildCreationOutcome::InitializationPanicked { creation }
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

pub(in crate::templates) fn ordered_creation_positions(
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

#[cfg(test)]
mod activation_correlation {
    use std::sync::Arc;

    use behavior::{
        ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase, CreationSequence,
        EndpointAddress, EstablishedActor, EstablishedRecipient, MessageProtocol, Never, NoBirths,
        NoSends, Protocol, User,
    };

    use super::{CurrentWorker, Member, MemberState, RecoveryCount, RoleName, Worker, WorkerPhase};
    use crate::StopOnShutdown;
    use crate::worker::{
        ImmediateStartup, InitializationAttempt, InitializeWorker, StartWorker, WorkerAttempt,
        WorkerInitializationOutcome, WorkerInitializationReport,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct StartupAddress(u64);

    impl Address for StartupAddress {
        type Nonce = u64;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct StartupEndpoint(u64);

    impl EndpointAddress for StartupAddress {
        type Established<P>
            = StartupEndpoint
        where
            P: Protocol<Addr = Self>;
        type Installed<B>
            = StartupEndpoint
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>;

        fn recipient<B>(installed: &Self::Installed<B>) -> Self::Established<B::Protocol>
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>,
        {
            *installed
        }
    }

    struct StartupWorker;

    impl BehaviorBase for StartupWorker {
        type Base = Self;
        fn base(&self) -> &Self::Base {
            self
        }
    }

    impl Behavior for StartupWorker {
        type Protocol = MessageProtocol<StartupAddress, Never>;
        type Event = User<StartupAddress, Never>;
        type Sends = NoSends;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    fn initialized_startup(
        worker: WorkerAttempt,
        target: EstablishedRecipient<MessageProtocol<StartupAddress, Never>>,
    ) -> StartWorker<StartupWorker, ImmediateStartup> {
        let initialization = InitializationAttempt::issued(&worker);
        let request = InitializeWorker::new(
            worker.clone(),
            initialization.clone(),
            target,
            ImmediateStartup,
        );
        let report = request.resolve(WorkerInitializationOutcome::ReadyForStartup);
        let WorkerInitializationReport::ReadyForStartup {
            worker: returned_worker,
            initialization: returned_initialization,
            startup,
            permit,
        } = report
        else {
            panic!("the original ready initialization returns its permit")
        };
        assert_eq!(returned_worker, worker);
        assert_eq!(returned_initialization, initialization);
        StartWorker::new(startup, permit)
    }

    #[test]
    fn live_member_returns_a_same_worker_started_from_another_attempt() {
        let mut creations = CreationSequence::new();
        let creation = creations
            .issue()
            .expect("one real creation correlation is available");
        let worker = WorkerAttempt::issued(creation);
        let actor = EstablishedActor::<StopOnShutdown<StartupWorker>>::issued(StartupEndpoint(19));
        let expected_actor = actor.clone();
        let original = initialized_startup(worker.clone(), actor.recipient());
        let foreign = initialized_startup(worker.clone(), actor.recipient());
        let expected_attempt = original.attempt();
        let foreign_attempt = foreign.attempt();
        assert!(expected_attempt != foreign_attempt);
        let role = RoleName::new(23_u64);
        let expected_role = role.clone().into_role();
        let member: Member<u64, StartupWorker, ImmediateStartup, Never, Never, Never> = Member {
            role,
            recoveries: RecoveryCount::default(),
            state: MemberState::Worker(Worker {
                current: CurrentWorker::new(worker.clone(), actor),
                phase: WorkerPhase::StartupRequested {
                    attempt: expected_attempt.clone(),
                    stopped: None,
                },
            }),
        };
        let input = foreign.started();
        let admission = member.admit_activation(input);
        drop(original);
        drop(foreign);
        let Err((returned_member, returned_input)) = admission else {
            panic!("same worker identity cannot substitute a different startup attempt")
        };
        let Member {
            role,
            recoveries,
            state,
        } = returned_member;
        let MemberState::Worker(Worker { current, phase }) = state else {
            panic!("rejected startup preserves the complete worker state")
        };
        let WorkerPhase::StartupRequested { attempt, stopped } = phase else {
            panic!("foreign Started must not advance the original dispatched phase")
        };
        let returned_role = role.into_role();
        assert!(Arc::ptr_eq(&returned_role, &expected_role));
        assert_eq!(*returned_role, 23);
        assert_eq!(recoveries, RecoveryCount::default());
        assert_eq!(current.attempt, worker);
        assert_eq!(current.actor, expected_actor);
        assert!(attempt == expected_attempt);
        assert!(stopped.is_none());
        assert_eq!(returned_input.worker(), worker);
        assert!(returned_input.attempt() == &foreign_attempt);
        let started = returned_input.into_started();
        assert!(started.is_ok());
    }

    #[tokio::test]
    async fn live_member_returns_a_same_worker_ready_from_another_attempt() {
        let mut creations = CreationSequence::new();
        let creation = creations
            .issue()
            .expect("one real creation correlation is available");
        let worker = WorkerAttempt::issued(creation);
        let actor = EstablishedActor::<StopOnShutdown<StartupWorker>>::issued(StartupEndpoint(29));
        let expected_actor = actor.clone();
        let original = initialized_startup(worker.clone(), actor.recipient());
        let foreign = initialized_startup(worker.clone(), actor.recipient());
        let expected_attempt = original.attempt();
        let foreign_attempt = foreign.attempt();
        assert!(expected_attempt != foreign_attempt);
        let role = RoleName::new(31_u64);
        let expected_role = role.clone().into_role();
        let member: Member<u64, StartupWorker, ImmediateStartup, Never, Never, Never> = Member {
            role,
            recoveries: RecoveryCount::default(),
            state: MemberState::Worker(Worker {
                current: CurrentWorker::new(worker.clone(), actor),
                phase: WorkerPhase::Starting {
                    attempt: expected_attempt.clone(),
                    stopped: None,
                },
            }),
        };
        let input = foreign.start().await;
        let admission = member.admit_activation(input);
        drop(original);
        let Err((returned_member, returned_input)) = admission else {
            panic!("same worker identity cannot substitute a different startup attempt")
        };
        let Member {
            role,
            recoveries,
            state,
        } = returned_member;
        let MemberState::Worker(Worker { current, phase }) = state else {
            panic!("rejected startup preserves the complete worker state")
        };
        let WorkerPhase::Starting { attempt, stopped } = phase else {
            panic!("foreign Ready must not make the original startup idle")
        };
        let returned_role = role.into_role();
        assert!(Arc::ptr_eq(&returned_role, &expected_role));
        assert_eq!(*returned_role, 31);
        assert_eq!(recoveries, RecoveryCount::default());
        assert_eq!(current.attempt, worker);
        assert_eq!(current.actor, expected_actor);
        assert!(attempt == expected_attempt);
        assert!(stopped.is_none());
        assert_eq!(returned_input.worker(), worker);
        assert!(returned_input.attempt() == &foreign_attempt);
        let ready = returned_input.into_ready();
        assert!(matches!(ready, Ok(())));
    }
}

#[cfg(test)]
pub(in crate::templates) mod activation_pool_correlation {
    use super::CurrentWorker;
    use crate::StopOnShutdown;
    use crate::pool::{Assignment, Completion};
    use crate::worker::{
        ImmediateStartup, InitializationAttempt, InitializeWorker, StartWorker, WorkerAttempt,
        WorkerInitializationOutcome, WorkerInitializationReport,
    };
    use behavior::{
        Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase, EndpointAddress,
        EstablishedActor, EstablishedRecipient, InterpreterRequests, MessageProtocol, Never,
        NoBirths, Protocol, ReportToParent, User,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(in crate::templates) struct StartupAddress(pub(in crate::templates) u64);
    impl Address for StartupAddress {
        type Nonce = u64;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(in crate::templates) struct StartupEndpoint(pub(in crate::templates) u64);
    impl EndpointAddress for StartupAddress {
        type Established<P>
            = StartupEndpoint
        where
            P: Protocol<Addr = Self>;
        type Installed<B>
            = StartupEndpoint
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>;
        fn recipient<B>(installed: &Self::Installed<B>) -> Self::Established<B::Protocol>
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>,
        {
            *installed
        }
    }

    pub(in crate::templates) struct StartupWorker;
    impl BehaviorBase for StartupWorker {
        type Base = Self;
        fn base(&self) -> &Self::Base {
            self
        }
    }
    impl Behavior for StartupWorker {
        type Protocol = MessageProtocol<StartupAddress, Assignment<u8>>;
        type Event = User<StartupAddress, Assignment<u8>>;
        type Sends = InterpreterRequests<ReportToParent<Completion<u16>>>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            let result = u16::from(*event.message.payload());
            Ok(Actions::cont().with_send(event.message.complete(result)))
        }
    }

    pub(in crate::templates) fn initialized_startup(
        worker: WorkerAttempt,
        target: EstablishedRecipient<MessageProtocol<StartupAddress, Assignment<u8>>>,
    ) -> StartWorker<StartupWorker, ImmediateStartup> {
        let initialization = InitializationAttempt::issued(&worker);
        let report = InitializeWorker::new(
            worker.clone(),
            initialization.clone(),
            target,
            ImmediateStartup,
        )
        .resolve(WorkerInitializationOutcome::ReadyForStartup);
        let WorkerInitializationReport::ReadyForStartup {
            worker: returned_worker,
            initialization: returned_initialization,
            startup,
            permit,
        } = report
        else {
            panic!("original ready initialization returns its exact permit")
        };
        assert_eq!(returned_worker, worker);
        assert_eq!(returned_initialization, initialization);
        StartWorker::new(startup, permit)
    }

    pub(in crate::templates) fn assert_current_worker(
        current: &CurrentWorker<StartupWorker>,
        worker: &WorkerAttempt,
        actor: &EstablishedActor<StopOnShutdown<StartupWorker>>,
    ) {
        assert_eq!(&current.attempt, worker);
        assert_eq!(&current.actor, actor);
    }
}
