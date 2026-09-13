//! Worker initialization and its exact returned input shared by atomic owners.

use std::sync::Arc;

use core::marker::PhantomData;

use behavior::{
    ActionItem, Behavior, BehaviorAddr, EndpointAddress, EstablishedRecipient, Here,
    InterpreterRequest, Never, ReturnsToEmitter,
};

use crate::ChildStopped;

use super::WorkerAttempt;

/// Non-reused correlation for one worker's initialization settlement.
#[doc(hidden)]
#[derive(Clone)]
pub struct InitializationAttempt {
    worker: WorkerAttempt,
    token: Arc<()>,
}

impl InitializationAttempt {
    pub(in crate::atomic) fn issued(worker: &WorkerAttempt) -> Self {
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

/// One-shot authority to begin activation for one initialized worker.
///
/// ```compile_fail,E0382
/// fn duplicate<W>(permit: behavior_actors::atomic::ActivationPermit<W>) {
///     let _accepted = permit;
///     let _duplicate = permit;
/// }
/// ```
#[doc(hidden)]
#[must_use = "activation authority must be consumed or retained"]
pub struct ActivationPermit<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    worker: WorkerAttempt,
    initialization: InitializationAttempt,
    target: EstablishedRecipient<W::Protocol>,
    worker_type: PhantomData<fn() -> W>,
}

impl<W> ActivationPermit<W>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    /// Worker correlation authorized by this permit.
    #[doc(hidden)]
    #[must_use]
    pub fn worker(&self) -> WorkerAttempt {
        self.worker.clone()
    }

    /// Initialization correlation authorized by this permit.
    #[doc(hidden)]
    #[must_use]
    pub fn initialization(&self) -> InitializationAttempt {
        self.initialization.clone()
    }

    /// Exact installed worker authorized by this permit.
    #[doc(hidden)]
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<W::Protocol> {
        self.target.clone()
    }

    pub(in super::super) fn worker_evidence(&self) -> &WorkerAttempt {
        &self.worker
    }
}

/// One child-host request retaining the exact activation plan.
///
/// ```compile_fail,E0382
/// fn duplicate<W, P>(request: behavior_actors::atomic::InitializeWorker<W, P>)
/// where
///     W: behavior::Behavior,
///     behavior::BehaviorAddr<W>: behavior::EndpointAddress,
/// {
///     let _accepted = request;
///     let _duplicate = request;
/// }
/// ```
#[doc(hidden)]
#[must_use = "worker initialization custody must settle or transfer outward"]
pub struct InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    worker: WorkerAttempt,
    initialization: InitializationAttempt,
    target: EstablishedRecipient<W::Protocol>,
    activation: P,
}

impl<W, P> InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) fn new(
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        target: EstablishedRecipient<W::Protocol>,
        activation: P,
    ) -> Self {
        Self {
            worker,
            initialization,
            target,
            activation,
        }
    }

    /// Exact installed-worker target retained by this request.
    #[doc(hidden)]
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<W::Protocol> {
        self.target.clone()
    }

    /// Worker-creation correlation retained by the proxy.
    #[doc(hidden)]
    #[must_use]
    pub fn worker(&self) -> WorkerAttempt {
        self.worker.clone()
    }

    /// Initialization correlation retained by the proxy.
    #[doc(hidden)]
    #[must_use]
    pub fn initialization(&self) -> InitializationAttempt {
        self.initialization.clone()
    }

    /// Reunite the host result with the affine activation plan.
    #[doc(hidden)]
    #[must_use]
    pub fn resolve(
        self,
        result: WorkerInitializationOutcome<W>,
    ) -> WorkerInitializationReport<W, P> {
        let Self {
            worker,
            initialization,
            target,
            activation,
        } = self;
        match result {
            WorkerInitializationOutcome::ReadyForActivation => {
                WorkerInitializationReport::ReadyForActivation {
                    worker: worker.clone(),
                    initialization: initialization.clone(),
                    activation,
                    permit: ActivationPermit {
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
                    activation,
                    failure,
                }
            }
            WorkerInitializationOutcome::Stopped(stopped) => WorkerInitializationReport::Stopped {
                worker,
                initialization,
                activation,
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
}

impl<W, P> ActionItem for InitializeWorker<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    EstablishedRecipient<W::Protocol>: Send,
    P: Send,
{
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

/// Result selected by the child host after total initialization settlement.
#[doc(hidden)]
pub enum WorkerInitializationOutcome<W>
where
    W: Behavior,
{
    ReadyForActivation,
    EffectsRejected(WorkerInitializationFailure),
    Stopped(ChildStopped<BehaviorAddr<W>>),
}

/// Exact initialization result returned to the worker owner.
#[doc(hidden)]
pub enum WorkerInitializationReport<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    ReadyForActivation {
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        activation: P,
        permit: ActivationPermit<W>,
    },
    EffectsRejected {
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        activation: P,
        failure: WorkerInitializationFailure,
    },
    Stopped {
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        activation: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    },
}

impl<W, P> WorkerInitializationReport<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(in crate::atomic) fn worker(&self) -> &WorkerAttempt {
        match self {
            Self::ReadyForActivation { worker, .. }
            | Self::EffectsRejected { worker, .. }
            | Self::Stopped { worker, .. } => worker,
        }
    }

    pub(in crate::atomic) fn initialization(&self) -> &InitializationAttempt {
        match self {
            Self::ReadyForActivation { initialization, .. }
            | Self::EffectsRejected { initialization, .. }
            | Self::Stopped { initialization, .. } => initialization,
        }
    }
}

/// Semantic classification returned after Bombay retains a failed worker
/// initialization settlement in runtime retirement custody.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerInitializationFailure {
    /// At least one initialization item was rejected or dependency-blocked.
    EffectsRejected,
    /// Interpretation corrupted or left an unattempted suffix.
    InterpreterCorrupt,
}
