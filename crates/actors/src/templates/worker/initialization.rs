//! Worker initialization and its exact returned input shared by atomic owners.

use std::sync::Arc;

use core::marker::PhantomData;

use behavior::{
    ActionItem, Behavior, BehaviorAddr, EndpointAddress, EstablishedRecipient, Here,
    InterpretationProgress, InterpreterRequest, ItemSettlement, Never, ReturnsToEmitter,
    finish_item, prepare_item,
};

use crate::ChildStopped;

use super::WorkerAttempt;

/// Non-reused correlation for one worker's initialization settlement.
#[derive(Clone)]
pub struct InitializationAttempt {
    worker: WorkerAttempt,
    token: Arc<()>,
}

impl InitializationAttempt {
    pub(in crate::templates) fn issued(worker: &WorkerAttempt) -> Self {
        Self {
            worker: worker.clone(),
            token: Arc::new(()),
        }
    }
}

impl core::fmt::Debug for InitializationAttempt {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("InitializationAttempt")
            .field(&self.worker)
            .finish()
    }
}

impl PartialEq for InitializationAttempt {
    fn eq(&self, other: &Self) -> bool {
        self.worker == other.worker && Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for InitializationAttempt {}

/// One-shot authority to begin startup for one initialized worker.
///
/// ```
/// fn accept<W>(permit: behavior_actors::worker::StartupPermit<W>)
/// where
///     W: behavior::Behavior,
///     behavior::BehaviorAddr<W>: behavior::EndpointAddress,
/// {
///     let _accepted = permit;
/// }
/// ```
///
/// ```compile_fail,E0382
/// fn duplicate<W>(permit: behavior_actors::worker::StartupPermit<W>)
/// where
///     W: behavior::Behavior,
///     behavior::BehaviorAddr<W>: behavior::EndpointAddress,
/// {
///     let _accepted = permit;
///     let _duplicate = permit;
/// }
/// ```
#[must_use = "startup authority must be consumed or retained"]
pub struct StartupPermit<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    worker: WorkerAttempt,
    initialization: InitializationAttempt,
    target: EstablishedRecipient<W::Protocol>,
    worker_type: PhantomData<fn() -> W>,
}

impl<W> StartupPermit<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    /// Worker correlation authorized by this permit.
    #[must_use]
    pub fn worker(&self) -> WorkerAttempt {
        self.worker.clone()
    }

    /// Initialization correlation authorized by this permit.
    #[must_use]
    pub fn initialization(&self) -> InitializationAttempt {
        self.initialization.clone()
    }

    /// Exact installed worker authorized by this permit.
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<W::Protocol> {
        self.target.clone()
    }

    pub(in super::super) fn worker_evidence(&self) -> &WorkerAttempt {
        &self.worker
    }
}

/// One child-host request retaining the exact startup plan.
///
/// ```compile_fail,E0382
/// fn duplicate<W, P>(request: behavior_actors::worker::InitializeWorker<W, P>)
/// where
///     W: behavior::Behavior,
///     behavior::BehaviorAddr<W>: behavior::EndpointAddress,
/// {
///     let _accepted = request;
///     let _duplicate = request;
/// }
/// ```
#[must_use = "worker initialization custody must settle or transfer outward"]
pub struct InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    worker: WorkerAttempt,
    initialization: InitializationAttempt,
    target: EstablishedRecipient<W::Protocol>,
    startup: P,
}

impl<W, P> InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) fn new(
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        target: EstablishedRecipient<W::Protocol>,
        startup: P,
    ) -> Self {
        Self {
            worker,
            initialization,
            target,
            startup,
        }
    }

    /// Exact installed-worker target retained by this request.
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<W::Protocol> {
        self.target.clone()
    }

    /// Worker-creation correlation retained by the proxy.
    #[must_use]
    pub fn worker(&self) -> WorkerAttempt {
        self.worker.clone()
    }

    /// Initialization correlation retained by the proxy.
    #[must_use]
    pub fn initialization(&self) -> InitializationAttempt {
        self.initialization.clone()
    }

    /// Reunite the host result with the affine startup plan.
    #[must_use]
    pub fn resolve(
        self,
        result: WorkerInitializationOutcome<W>,
    ) -> WorkerInitializationReport<W, P> {
        let Self {
            worker,
            initialization,
            target,
            startup,
        } = self;
        match result {
            WorkerInitializationOutcome::ReadyForStartup => {
                WorkerInitializationReport::ReadyForStartup {
                    worker: worker.clone(),
                    initialization: initialization.clone(),
                    startup,
                    permit: StartupPermit {
                        worker,
                        initialization,
                        target,
                        worker_type: PhantomData,
                    },
                }
            }
            WorkerInitializationOutcome::EffectsRejected(failure) => {
                WorkerInitializationReport::EffectsRejected {
                    worker,
                    initialization,
                    startup,
                    failure,
                }
            }
            WorkerInitializationOutcome::Stopped(stopped) => WorkerInitializationReport::Stopped {
                worker,
                initialization,
                startup,
                stopped,
            },
        }
    }
}

impl<W, P> InterpreterRequest for InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    type ReturnToEmitter = ReturnsToEmitter<WorkerInitializationReport<W, P>, Here>;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<W, P> ActionItem for InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    EstablishedRecipient<W::Protocol>: Send,
    P: Send,
{
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;

    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }

    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        match custody {
            (input @ Some(_), received @ None) => Some((input, received)),
            _ => None,
        }
    }

    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

/// Result selected by the child host after total initialization settlement.
pub enum WorkerInitializationOutcome<W>
where
    W: Behavior,
{
    ReadyForStartup,
    EffectsRejected(WorkerInitializationFailure),
    Stopped(ChildStopped<BehaviorAddr<W>>),
}

/// Exact initialization result returned to the worker owner.
pub enum WorkerInitializationReport<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    ReadyForStartup {
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        startup: P,
        permit: StartupPermit<W>,
    },
    EffectsRejected {
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        startup: P,
        failure: WorkerInitializationFailure,
    },
    Stopped {
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        startup: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

impl<W, P> WorkerInitializationReport<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::templates) fn worker(&self) -> &WorkerAttempt {
        match self {
            Self::ReadyForStartup { worker, .. }
            | Self::EffectsRejected { worker, .. }
            | Self::Stopped { worker, .. } => worker,
        }
    }

    pub(in crate::templates) fn initialization(&self) -> &InitializationAttempt {
        match self {
            Self::ReadyForStartup { initialization, .. }
            | Self::EffectsRejected { initialization, .. }
            | Self::Stopped { initialization, .. } => initialization,
        }
    }
}

/// Semantic classification returned after Bombay retains a failed worker
/// initialization settlement in runtime retirement custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerInitializationFailure {
    /// At least one initialization item was rejected or dependency-blocked.
    EffectsRejected,
    /// Interpretation corrupted or left an unattempted suffix.
    InterpreterCorrupt,
}
