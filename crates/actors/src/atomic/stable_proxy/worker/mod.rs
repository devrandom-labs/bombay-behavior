//! Exact current worker ownership and correlation.

use core::ops::ControlFlow;

use behavior::{
    Address, Behavior, BehaviorAddr, ChildCreationOutcome, CreateChild, CreationId, CreationKind,
    CreationSequence, CreationSettlement, CreationsSettled, EndpointAddress, EstablishedActor,
    Here, Ingress, InjectEvent, ItemSettlement, SettledItem,
};

use crate::{
    ChildStopped, EstablishedShutdownResolved, ShutdownEstablished, ShutdownId, ShutdownRequested,
    StopOnShutdown,
};

use crate::atomic::worker::{
    WorkerCreationOutcome, WorkerCreationSettlement, settle_worker_creation,
};
use crate::atomic::{
    ActivationPlan, InitializationAttempt, WorkerAttempt, WorkerCreationRejection,
    WorkerInitializationReport,
};

use super::ProxyDrain;

#[cfg(test)]
mod direct_worker_admission_contract {
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    use crate::ChildStopped;

    use super::{CurrentWorker, PendingWorker, WorkerCreationSettlement};
    use crate::atomic::{ActivationPlan, WorkerInitializationReport};

    #[expect(dead_code, reason = "compile contract for exact worker input")]
    fn current_stop<W>(
        worker: &CurrentWorker<W>,
        input: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<ChildStopped<BehaviorAddr<W>>, ChildStopped<BehaviorAddr<W>>>
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
    {
        worker.admit_stop(input)
    }

    #[expect(dead_code, reason = "compile contract for exact worker input")]
    fn current_initialization<W, P>(
        worker: &CurrentWorker<W>,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<WorkerInitializationReport<W, P>, WorkerInitializationReport<W, P>>
    where
        W: Behavior,
        P: ActivationPlan,
        BehaviorAddr<W>: EndpointAddress,
    {
        worker.admit_initialization(input)
    }

    #[expect(dead_code, reason = "compile contract for exact worker input")]
    fn pending_stop<W, P>(
        worker: &PendingWorker<P>,
        input: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<ChildStopped<BehaviorAddr<W>>, ChildStopped<BehaviorAddr<W>>>
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
    {
        worker.admit_stop(input)
    }

    #[expect(dead_code, reason = "compile contract for exact worker input")]
    fn pending_creation<W, P>(
        worker: &PendingWorker<P>,
        input: WorkerCreationSettlement<W>,
    ) -> Result<WorkerCreationSettlement<W>, WorkerCreationSettlement<W>>
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
    {
        worker.admit_creation(input)
    }
}

/// Complete result of one owner-submitted worker start.
pub enum WorkerStartResult<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    StopOnShutdown<W>: Behavior<Protocol = W::Protocol, Ph = W::Ph, Birth = W::Birth>,
{
    /// Worker creation rejected before a worker was committed.
    CreationRejected {
        rejection: WorkerCreationRejection<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    /// The committed worker became ready and may receive service commands.
    Ready {
        attempt: WorkerAttempt,
        readiness: P::Ready,
    },
    /// A committed worker could not become available.
    Unavailable {
        attempt: WorkerAttempt,
        drain: ProxyDrain<W, P>,
    },
}

pub(in super::super) struct PendingWorker<P> {
    attempt: WorkerAttempt,
    initialization: InitializationAttempt,
    kind: CreationKind,
    activation: P,
}

impl<P> PendingWorker<P> {
    pub(in super::super) fn birth<W>(
        creations: &mut CreationSequence,
        worker: W,
        activation: P,
    ) -> Result<
        (
            Self,
            CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
        ),
        (W, P),
    >
    where
        W: Behavior,
    {
        let Some(creation) = creations.issue() else {
            return Err((worker, activation));
        };
        let request = CreateChild::birth(creation, StopOnShutdown::new(worker));
        Ok(Self::await_creation(activation, request))
    }

    pub(in super::super) fn replacement<W>(
        creations: &mut CreationSequence,
        previous: CreationId,
        worker: W,
        activation: P,
    ) -> Result<
        (
            Self,
            CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
        ),
        (W, P),
    >
    where
        W: Behavior,
    {
        let Some(creation) = creations.issue() else {
            return Err((worker, activation));
        };
        let request = CreateChild::replacement(creation, previous, StopOnShutdown::new(worker));
        Ok(Self::await_creation(activation, request))
    }

    fn await_creation<W>(
        activation: P,
        request: CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
    ) -> (
        Self,
        CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
    )
    where
        W: Behavior,
    {
        let creation = request.id();
        let kind = request.kind();
        let attempt = WorkerAttempt::issued(creation);
        let initialization = InitializationAttempt::issued(&attempt);
        (
            Self {
                attempt,
                initialization,
                kind,
                activation,
            },
            request,
        )
    }

    pub(in super::super) const fn creation(&self) -> CreationId {
        self.attempt.creation()
    }

    pub(in super::super) fn cancel<W>(
        self,
        creation: CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
    ) -> (WorkerAttempt, W, P)
    where
        W: Behavior,
    {
        let (_, worker, _) = creation.into_parts();
        (self.attempt, worker.into_inner(), self.activation)
    }

    pub(in super::super) fn admit_stop<A>(
        &self,
        stopped: ChildStopped<A>,
    ) -> Result<ChildStopped<A>, ChildStopped<A>>
    where
        A: Address,
    {
        if stopped.child == self.attempt.creation() {
            Ok(stopped)
        } else {
            Err(stopped)
        }
    }

    fn admit_creation<W>(
        &self,
        settlement: WorkerCreationSettlement<W>,
    ) -> Result<WorkerCreationSettlement<W>, WorkerCreationSettlement<W>>
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
    {
        let received = match &settlement {
            SettledItem::Unattempted(creation) => (creation.id(), creation.kind()),
            SettledItem::Attempted(ItemSettlement::Rejected { item, .. })
            | SettledItem::Attempted(ItemSettlement::Blocked { item, .. })
            | SettledItem::Attempted(ItemSettlement::Corrupt { item, .. }) => {
                (item.id(), item.kind())
            }
            SettledItem::Attempted(ItemSettlement::Accepted(creation)) => match creation {
                ChildCreationOutcome::Established { established } => {
                    (established.id(), established.kind())
                }
                ChildCreationOutcome::InitializationRejected { creation, .. }
                | ChildCreationOutcome::HostRejected { creation, .. } => {
                    (creation.id(), creation.kind())
                }
            },
        };
        if received == (self.attempt.creation(), self.kind) {
            Ok(settlement)
        } else {
            Err(settlement)
        }
    }
}

enum WorkerShutdown<P>
where
    P: behavior::Protocol,
{
    Requested(ShutdownId),
    Settled(EstablishedShutdownResolved<P>),
}

pub(in super::super) struct WorkerStopping<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    worker: CurrentWorker<W>,
    shutdown: WorkerShutdown<W::Protocol>,
    stopped: Option<ChildStopped<BehaviorAddr<W>>>,
}

pub(in super::super) struct StoppedWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in super::super) worker: CurrentWorker<W>,
    pub(in super::super) shutdown: EstablishedShutdownResolved<W::Protocol>,
    pub(in super::super) stopped: ChildStopped<BehaviorAddr<W>>,
}

impl<W> WorkerStopping<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in super::super) const fn worker(&self) -> &CurrentWorker<W> {
        &self.worker
    }

    pub(in super::super) const fn stopped(&self) -> Option<&ChildStopped<BehaviorAddr<W>>> {
        self.stopped.as_ref()
    }

    pub(in super::super) fn begin(
        worker: CurrentWorker<W>,
    ) -> (Self, ShutdownEstablished<StopOnShutdown<W>, Here>)
    where
        StopOnShutdown<W>: Behavior<Protocol = W::Protocol>,
        <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    {
        let shutdown = ShutdownEstablished::new(
            ShutdownId(worker.attempt.creation().get()),
            worker.actor.clone(),
            Ingress::new(),
        );
        (
            Self {
                worker,
                shutdown: WorkerShutdown::Requested(shutdown.id),
                stopped: None,
            },
            shutdown,
        )
    }

    pub(in super::super) fn shutdown_resolved(
        self,
        input: EstablishedShutdownResolved<W::Protocol>,
    ) -> Result<ControlFlow<StoppedWorker<W>, Self>, (Self, EstablishedShutdownResolved<W::Protocol>)>
    {
        let Self {
            worker,
            shutdown,
            stopped,
        } = self;
        match shutdown {
            WorkerShutdown::Requested(expected) if input.id() == expected => Ok(Self {
                worker,
                shutdown: WorkerShutdown::Settled(input),
                stopped,
            }
            .complete()),
            shutdown => Err((
                Self {
                    worker,
                    shutdown,
                    stopped,
                },
                input,
            )),
        }
    }

    pub(in super::super) fn worker_stopped(
        self,
        input: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<ControlFlow<StoppedWorker<W>, Self>, (Self, ChildStopped<BehaviorAddr<W>>)> {
        let Self {
            worker,
            shutdown,
            stopped,
        } = self;
        match stopped {
            None => match worker.admit_stop(input) {
                Ok(stopped) => Ok(Self {
                    worker,
                    shutdown,
                    stopped: Some(stopped),
                }
                .complete()),
                Err(input) => Err((
                    Self {
                        worker,
                        shutdown,
                        stopped: None,
                    },
                    input,
                )),
            },
            Some(stopped) => Err((
                Self {
                    worker,
                    shutdown,
                    stopped: Some(stopped),
                },
                input,
            )),
        }
    }

    pub(in super::super) fn into_stopped_before_shutdown(
        self,
    ) -> Result<(CurrentWorker<W>, ShutdownId, ChildStopped<BehaviorAddr<W>>), Self> {
        match (self.shutdown, self.stopped) {
            (WorkerShutdown::Requested(shutdown), Some(stopped)) => {
                Ok((self.worker, shutdown, stopped))
            }
            (shutdown, stopped) => Err(Self {
                worker: self.worker,
                shutdown,
                stopped,
            }),
        }
    }

    fn complete(self) -> ControlFlow<StoppedWorker<W>, Self> {
        match (self.shutdown, self.stopped) {
            (WorkerShutdown::Settled(shutdown), Some(stopped)) => {
                ControlFlow::Break(StoppedWorker {
                    worker: self.worker,
                    shutdown,
                    stopped,
                })
            }
            (shutdown, stopped) => ControlFlow::Continue(Self {
                worker: self.worker,
                shutdown,
                stopped,
            }),
        }
    }
}

pub(in super::super) struct CurrentWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in super::super) attempt: WorkerAttempt,
    pub(in super::super) initialization: InitializationAttempt,
    pub(in super::super) actor: EstablishedActor<StopOnShutdown<W>>,
}

impl<W> CurrentWorker<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in super::super) fn admit_stop(
        &self,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<ChildStopped<BehaviorAddr<W>>, ChildStopped<BehaviorAddr<W>>> {
        if stopped.child == self.attempt.creation() {
            Ok(stopped)
        } else {
            Err(stopped)
        }
    }

    pub(in super::super) fn admit_initialization<P>(
        &self,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<WorkerInitializationReport<W, P>, WorkerInitializationReport<W, P>>
    where
        P: ActivationPlan,
    {
        match (input.worker(), input.initialization()) {
            (worker, initialization)
                if worker == &self.attempt && initialization == &self.initialization =>
            {
                Ok(input)
            }
            _ => Err(input),
        }
    }
}

pub(in super::super) enum WorkerCreation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    Initializing {
        worker: CurrentWorker<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Rejected {
        rejection: WorkerCreationRejection<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    },
    Unexpected {
        worker: PendingWorker<P>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    },
}

impl<P> PendingWorker<P> {
    pub(in super::super) fn created<W>(
        self,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    ) -> WorkerCreation<W, P>
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
        StopOnShutdown<W>:
            Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    {
        match workers.into_settlement() {
            CreationSettlement::Rejected { creations, reason } => {
                let mut creations: Vec<_> = creations.into_iter().collect();
                if creations.len() != 1 {
                    return WorkerCreation::Unexpected {
                        worker: self,
                        stopped,
                        workers: CreationsSettled::new(CreationSettlement::Rejected {
                            creations: creations.into_iter().collect(),
                            reason,
                        }),
                    };
                }
                let Some(creation) = creations.pop() else {
                    return WorkerCreation::Unexpected {
                        worker: self,
                        stopped,
                        workers: CreationsSettled::new(CreationSettlement::Rejected {
                            creations: behavior::Creations::empty(),
                            reason,
                        }),
                    };
                };
                if (creation.id(), creation.kind()) != (self.creation(), self.kind) {
                    return WorkerCreation::Unexpected {
                        worker: self,
                        stopped,
                        workers: CreationsSettled::new(CreationSettlement::Rejected {
                            creations: behavior::Creations::one(creation),
                            reason,
                        }),
                    };
                }
                let (_, worker, _) = creation.into_parts();
                WorkerCreation::Rejected {
                    rejection: WorkerCreationRejection::NamespaceExhausted {
                        worker: worker.into_inner(),
                    },
                    activation: self.activation,
                    stopped,
                }
            }
            CreationSettlement::Settled(settlements) => {
                let mut settlements: Vec<_> = settlements.into_iter().collect();
                if settlements.len() != 1 {
                    return WorkerCreation::Unexpected {
                        worker: self,
                        stopped,
                        workers: CreationsSettled::new(CreationSettlement::Settled(
                            settlements.into_iter().collect(),
                        )),
                    };
                }
                let Some(settlement) = settlements.pop() else {
                    return WorkerCreation::Unexpected {
                        worker: self,
                        stopped,
                        workers: CreationsSettled::new(CreationSettlement::Settled(
                            behavior::Creations::empty(),
                        )),
                    };
                };
                self.created_one(settlement, stopped)
            }
            settlement @ CreationSettlement::Corrupt { .. } => WorkerCreation::Unexpected {
                worker: self,
                stopped,
                workers: CreationsSettled::new(settlement),
            },
        }
    }

    fn created_one<W>(
        self,
        worker: WorkerCreationSettlement<W>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    ) -> WorkerCreation<W, P>
    where
        W: Behavior,
        BehaviorAddr<W>: EndpointAddress,
        StopOnShutdown<W>:
            Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    {
        match self.admit_creation(worker) {
            Ok(settlement) => match settle_worker_creation(settlement) {
                WorkerCreationOutcome::Established(actor) => WorkerCreation::Initializing {
                    worker: CurrentWorker {
                        attempt: self.attempt,
                        initialization: self.initialization,
                        actor,
                    },
                    activation: self.activation,
                    stopped,
                },
                WorkerCreationOutcome::Rejected(rejection) => WorkerCreation::Rejected {
                    rejection,
                    activation: self.activation,
                    stopped,
                },
                WorkerCreationOutcome::InvalidSettlement(settlement) => {
                    WorkerCreation::Unexpected {
                        worker: self,
                        stopped,
                        workers: CreationsSettled::new(CreationSettlement::Settled(
                            behavior::Creations::one(settlement),
                        )),
                    }
                }
            },
            Err(settlement) => WorkerCreation::Unexpected {
                worker: self,
                stopped,
                workers: CreationsSettled::new(CreationSettlement::Settled(
                    behavior::Creations::one(settlement),
                )),
            },
        }
    }
}
