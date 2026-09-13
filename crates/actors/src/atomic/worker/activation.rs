//! Worker activation authorized by successful initialization.

use std::sync::Arc;

use core::marker::PhantomData;

use behavior::{
    ActionItem, Behavior, BehaviorAddr, EndpointAddress, EstablishedRecipient, Here,
    InterpreterRequest, Never, ReturnsToEmitter,
};

use super::initialization::{ActivationPermit, InitializationAttempt};
use super::{ActivationPlan, WorkerAttempt};

#[derive(Clone)]
pub(in super::super) struct ActivationAttempt {
    worker: WorkerAttempt,
    token: Arc<()>,
}

impl ActivationAttempt {
    fn issued(worker: &WorkerAttempt) -> Self {
        Self {
            worker: worker.clone(),
            token: Arc::new(()),
        }
    }
}

impl PartialEq for ActivationAttempt {
    fn eq(&self, other: &Self) -> bool {
        self.worker == other.worker && Arc::ptr_eq(&self.token, &other.token)
    }
}

impl Eq for ActivationAttempt {}

/// Exact reason activation work was not admitted by its owner.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivationStartRejection {
    OwnerStopped,
}

/// One activation request consuming an initialized worker permit and plan.
///
/// ```compile_fail,E0382
/// fn duplicate<W, P>(request: behavior_actors::atomic::BeginActivation<W, P>)
/// where
///     W: behavior::Behavior,
///     behavior::BehaviorAddr<W>: behavior::EndpointAddress,
///     P: behavior_actors::atomic::ActivationPlan,
/// {
///     let _accepted = request;
///     let _duplicate = request;
/// }
/// ```
#[doc(hidden)]
#[must_use = "activation work must be admitted or returned whole"]
pub struct BeginActivation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
{
    activation: ActivationAttempt,
    permit: ActivationPermit<W>,
    plan: P,
}

impl<W, P> BeginActivation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
{
    /// Consume the successful initialization authority and concrete plan.
    #[doc(hidden)]
    #[must_use]
    pub fn new(plan: P, permit: ActivationPermit<W>) -> Self {
        Self {
            activation: ActivationAttempt::issued(permit.worker_evidence()),
            permit,
            plan,
        }
    }

    /// Exact installed worker target carried by the permit.
    #[doc(hidden)]
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<W::Protocol> {
        self.permit.target()
    }

    /// Worker creation correlation authorized by the consumed permit.
    #[doc(hidden)]
    #[must_use]
    pub fn worker(&self) -> WorkerAttempt {
        self.permit.worker()
    }

    /// Initialization correlation authorized by the consumed permit.
    #[doc(hidden)]
    #[must_use]
    pub fn initialization(&self) -> InitializationAttempt {
        self.permit.initialization()
    }

    pub(in super::super) fn attempt(&self) -> ActivationAttempt {
        self.activation.clone()
    }

    /// Produce the input Bombay must admit before polling the plan.
    #[doc(hidden)]
    #[must_use]
    pub fn started(&self) -> WorkerActivation<W, P> {
        WorkerActivation {
            worker: self.worker(),
            activation: self.activation.clone(),
            outcome: WorkerActivationOutcome::Started,
            worker_type: PhantomData,
        }
    }

    /// Return a request that its owner could not admit.
    #[doc(hidden)]
    #[must_use]
    pub fn start_rejected(self, reason: ActivationStartRejection) -> WorkerActivation<W, P> {
        let worker = self.worker();
        let activation = self.activation.clone();
        WorkerActivation {
            worker,
            activation,
            outcome: WorkerActivationOutcome::StartRejected {
                request: self,
                reason,
            },
            worker_type: PhantomData,
        }
    }

    /// Consume the plan and produce its exact ready or rejected input.
    #[doc(hidden)]
    pub async fn activate(self) -> WorkerActivation<W, P> {
        let Self {
            activation,
            permit,
            plan,
        } = self;
        let worker = permit.worker();
        match plan.activate().await {
            Ok(readiness) => WorkerActivation {
                worker,
                activation,
                outcome: WorkerActivationOutcome::Ready(readiness),
                worker_type: PhantomData,
            },
            Err(rejection) => WorkerActivation {
                worker,
                activation,
                outcome: WorkerActivationOutcome::Rejected(rejection),
                worker_type: PhantomData,
            },
        }
    }
}

impl<W, P> InterpreterRequest for BeginActivation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
{
    type ReturnToEmitter = ReturnsToEmitter<WorkerActivation<W, P>, Here>;
}

impl<W, P> ActionItem for BeginActivation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    EstablishedRecipient<W::Protocol>: Send,
    P: ActivationPlan,
{
    type Accepted = ();
    type Rejection = ActivationStartRejection;
    type Prerequisite = Never;
}

pub(in crate::atomic) enum WorkerActivationOutcome<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
{
    Started,
    StartRejected {
        request: BeginActivation<W, P>,
        reason: ActivationStartRejection,
    },
    Ready(P::Ready),
    Rejected(P::Rejection),
}

/// Exact input produced by one accepted worker activation request.
#[doc(hidden)]
#[must_use = "worker activation must be admitted or transferred outward"]
pub struct WorkerActivation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
{
    worker: WorkerAttempt,
    activation: ActivationAttempt,
    outcome: WorkerActivationOutcome<W, P>,
    worker_type: PhantomData<fn() -> W>,
}

impl<W, P> WorkerActivation<W, P>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
    P: ActivationPlan,
{
    /// Worker creation correlation carried by this input.
    #[doc(hidden)]
    #[must_use]
    pub fn worker(&self) -> WorkerAttempt {
        self.worker.clone()
    }

    pub(in super::super) const fn attempt(&self) -> &ActivationAttempt {
        &self.activation
    }

    pub(in crate::atomic) fn into_parts(
        self,
    ) -> (
        WorkerAttempt,
        ActivationAttempt,
        WorkerActivationOutcome<W, P>,
    ) {
        (self.worker, self.activation, self.outcome)
    }

    pub(in crate::atomic) fn into_started(self) -> Result<(), Self> {
        let Self {
            worker,
            activation,
            outcome,
            worker_type,
        } = self;
        match outcome {
            WorkerActivationOutcome::Started => Ok(()),
            outcome => Err(Self {
                worker,
                activation,
                outcome,
                worker_type,
            }),
        }
    }

    pub(in super::super) fn into_start_rejection(
        self,
    ) -> Result<(BeginActivation<W, P>, ActivationStartRejection), Self> {
        let Self {
            worker,
            activation,
            outcome,
            worker_type,
        } = self;
        match outcome {
            WorkerActivationOutcome::StartRejected { request, reason } => Ok((request, reason)),
            outcome => Err(Self {
                worker,
                activation,
                outcome,
                worker_type,
            }),
        }
    }

    /// Consume readiness only when this is the ready alternative.
    #[doc(hidden)]
    pub fn into_ready(self) -> Result<P::Ready, Self> {
        let Self {
            worker,
            activation,
            outcome,
            worker_type,
        } = self;
        match outcome {
            WorkerActivationOutcome::Ready(readiness) => Ok(readiness),
            outcome => Err(Self {
                worker,
                activation,
                outcome,
                worker_type,
            }),
        }
    }

    /// Consume the application rejection only when this is the rejected alternative.
    #[doc(hidden)]
    pub fn into_rejection(self) -> Result<P::Rejection, Self> {
        let Self {
            worker,
            activation,
            outcome,
            worker_type,
        } = self;
        match outcome {
            WorkerActivationOutcome::Rejected(rejection) => Ok(rejection),
            outcome => Err(Self {
                worker,
                activation,
                outcome,
                worker_type,
            }),
        }
    }
}
