//! Current state owned by one retained keyed service.

use core::num::NonZeroU64;

use behavior::{Behavior, BehaviorAddr, CreationId, EndpointAddress, EstablishedActor};

use crate::atomic::stable_proxy::ProxyOperationWitness;
use crate::{
    ActivationPlan, ChildStopped, ProxyOperation, ProxyOperationId, StableProxy, WorkerChange,
    WorkerSubmission,
};

use super::{DynamicStatus, WorkerChangeInterruption};

mod retirement;

pub(super) use retirement::{
    ProxyInputCustody, ProxyInputPurpose, ProxyRetirement, ProxyShutdown, RetiringWork,
};

pub(super) enum ServiceAvailability<Ready> {
    Ready {
        worker: crate::WorkerAttempt,
        readiness: Ready,
    },
    Empty {
        previous: crate::WorkerAttempt,
    },
}

pub(super) enum WorkerChangeDisposition {
    Committed,
    Cancelled,
}

pub(super) enum DynamicEntryPhase<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    CreatingProxy {
        submission: WorkerSubmission<Worker, Plan>,
    },
    ShuttingDownProxyCreation,
    DrainingProxyCreationStop,
    WaitingForActivation {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        submission: WorkerSubmission<Worker, Plan>,
    },
    WaitingForProxyInput {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        witness: ProxyOperationWitness,
    },
    WaitingForProxyOutcome {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        operation: ProxyOperationId,
    },
    CancellingProxyCreation,
    StoppingProxyCreation {
        start_operation: NonZeroU64,
        submission: crate::WorkerSubmission<Worker, Plan>,
    },
    Available {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        service: ServiceAvailability<Plan::Ready>,
        latest: WorkerChangeDisposition,
    },
    ReplacementQueued {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        service: ServiceAvailability<Plan::Ready>,
        submission: WorkerSubmission<Worker, Plan>,
    },
    ReplacementAwaitingReceipt {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        service: ServiceAvailability<Plan::Ready>,
        witness: ProxyOperationWitness,
    },
    ReplacementAwaitingOutcome {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        service: ServiceAvailability<Plan::Ready>,
        operation: ProxyOperationId,
    },
    Stopping {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        restorable: Option<ServiceAvailability<Plan::Ready>>,
        shutdown: ProxyShutdown,
        stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
    },
    Retiring(ProxyRetirement<Worker, Plan>),
}

impl<Worker, Plan> DynamicEntryPhase<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn status(&self) -> DynamicStatus<Worker::Protocol> {
        match self {
            Self::CreatingProxy { .. } => DynamicStatus::CreatingProxy,
            Self::ShuttingDownProxyCreation | Self::DrainingProxyCreationStop => {
                DynamicStatus::Draining
            }
            Self::WaitingForActivation { .. } => DynamicStatus::WaitingForActivation,
            Self::WaitingForProxyInput { .. } | Self::WaitingForProxyOutcome { .. } => {
                DynamicStatus::AwaitingProxy
            }
            Self::CancellingProxyCreation { .. } => DynamicStatus::Cancelling,
            Self::StoppingProxyCreation { .. } => DynamicStatus::Stopping,
            Self::Available { proxy, service, .. } => match service {
                ServiceAvailability::Ready { .. } => DynamicStatus::Ready {
                    proxy: proxy.recipient(),
                },
                ServiceAvailability::Empty { .. } => DynamicStatus::Empty,
            },
            Self::ReplacementQueued { .. }
            | Self::ReplacementAwaitingReceipt { .. }
            | Self::ReplacementAwaitingOutcome { .. } => DynamicStatus::Replacing,
            Self::Stopping { .. } => DynamicStatus::Stopping,
            Self::Retiring(_) => DynamicStatus::Retiring,
        }
    }

    pub(super) fn shutdown(
        self,
        creation: CreationId,
        operation: NonZeroU64,
    ) -> (
        Self,
        Option<ProxyOperation<behavior::Here, Worker, Plan>>,
        Option<(
            NonZeroU64,
            WorkerChangeInterruption,
            WorkerSubmission<Worker, Plan>,
        )>,
    ) {
        match self {
            Self::CreatingProxy { submission } => (
                Self::ShuttingDownProxyCreation,
                None,
                Some((
                    operation,
                    WorkerChangeInterruption::SupervisorShutdown {
                        change: WorkerChange::Start,
                    },
                    submission,
                )),
            ),
            Self::StoppingProxyCreation {
                start_operation,
                submission,
            } => (
                Self::DrainingProxyCreationStop,
                None,
                Some((
                    start_operation,
                    WorkerChangeInterruption::ExplicitStop {
                        operation: operation.get(),
                    },
                    submission,
                )),
            ),
            phase => {
                let (creation, proxy, work, interruption) = match phase {
                    Self::WaitingForActivation { proxy, submission } => (
                        creation,
                        proxy,
                        RetiringWork::SupervisorShutdown(None),
                        Some((
                            operation,
                            WorkerChangeInterruption::SupervisorShutdown {
                                change: WorkerChange::Start,
                            },
                            submission,
                        )),
                    ),
                    Self::WaitingForProxyInput { proxy, witness } => (
                        creation,
                        proxy,
                        RetiringWork::Transferred {
                            purpose: ProxyInputPurpose::SupervisorStart(operation),
                            input: ProxyInputCustody::AwaitingSettlement(witness),
                        },
                        None,
                    ),
                    Self::WaitingForProxyOutcome {
                        proxy,
                        operation: proxy_operation,
                    } => (
                        creation,
                        proxy,
                        RetiringWork::Transferred {
                            purpose: ProxyInputPurpose::SupervisorStart(operation),
                            input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                        },
                        None,
                    ),
                    Self::Available {
                        proxy,
                        service,
                        latest: _,
                    } => (
                        creation,
                        proxy,
                        RetiringWork::SupervisorShutdown(Some(service)),
                        None,
                    ),
                    Self::ReplacementQueued {
                        proxy,
                        service,
                        submission,
                    } => (
                        creation,
                        proxy,
                        RetiringWork::SupervisorShutdown(Some(service)),
                        Some((
                            operation,
                            WorkerChangeInterruption::SupervisorShutdown {
                                change: WorkerChange::Replacement,
                            },
                            submission,
                        )),
                    ),
                    Self::ReplacementAwaitingReceipt {
                        proxy,
                        service,
                        witness,
                    } => (
                        creation,
                        proxy,
                        RetiringWork::Transferred {
                            purpose: ProxyInputPurpose::SupervisorReplacement {
                                operation,
                                service,
                            },
                            input: ProxyInputCustody::AwaitingSettlement(witness),
                        },
                        None,
                    ),
                    Self::ReplacementAwaitingOutcome {
                        proxy,
                        service,
                        operation: proxy_operation,
                    } => (
                        creation,
                        proxy,
                        RetiringWork::Transferred {
                            purpose: ProxyInputPurpose::SupervisorReplacement {
                                operation,
                                service,
                            },
                            input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                        },
                        None,
                    ),
                    phase => return (phase, None, None),
                };
                let (retirement, shutdown) = ProxyRetirement::begin(creation, proxy, work);
                (Self::Retiring(retirement), Some(shutdown), interruption)
            }
        }
    }
}

pub(super) struct DynamicEntry<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) generation: NonZeroU64,
    pub(super) operation: NonZeroU64,
    pub(super) creation: CreationId,
    pub(super) phase: DynamicEntryPhase<Worker, Plan>,
}

#[cfg(test)]
mod tests {
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    use super::DynamicEntryPhase;
    use crate::{ActivationPlan, StableProxy};

    #[expect(
        dead_code,
        reason = "compile-only proof of explicit proxy-creation drain phases"
    )]
    fn proxy_creation_drain_names_its_current_cause<Worker, Plan>(
        phase: DynamicEntryPhase<Worker, Plan>,
    ) where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        match phase {
            DynamicEntryPhase::ShuttingDownProxyCreation => {}
            DynamicEntryPhase::DrainingProxyCreationStop => {}
            _ => {}
        }
    }
}
