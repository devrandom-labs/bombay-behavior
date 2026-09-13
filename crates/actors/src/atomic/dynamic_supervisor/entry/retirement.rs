//! Proxy retirement and cancelled worker-change custody.

use core::num::NonZeroU64;

use behavior::{
    Behavior, BehaviorAddr, CreationId, EndpointAddress, EstablishedActor, ItemSettlement,
    SettledItem,
};

use crate::atomic::stable_proxy::ProxyOperationWitness;
use crate::{
    ActivationPlan, ChildStopped, InterruptedWorker, ProxyInputResult, ProxyOperation,
    ProxyOperationId, ProxyOutcome, StableProxy, WorkerChange,
};

use super::ServiceAvailability;

pub(in super::super) enum ProxyShutdown {
    AwaitingSettlement(ProxyOperationWitness),
    Accepted(ProxyOperationId),
    Rejected,
}

pub(in super::super) enum RetiringWork<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    StartFailed,
    UnexpectedWorkerStopped,
    InterruptedStart,
    SupervisorShutdown(Option<ServiceAvailability<Plan::Ready>>),
    CancelledWorkerReturned(WorkerChange),
    Transferred {
        purpose: ProxyInputPurpose<Plan::Ready>,
        input: ProxyInputCustody<Worker, Plan>,
    },
}

pub(in super::super) enum ProxyInputPurpose<Ready> {
    Cancellation(WorkerChange),
    InterruptedStart(NonZeroU64),
    SupervisorStart(NonZeroU64),
    SupervisorReplacement {
        operation: NonZeroU64,
        service: ServiceAvailability<Ready>,
    },
}

pub(in super::super) enum ProxyInputCustody<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    AwaitingSettlement(ProxyOperationWitness),
    AwaitingOutcome(ProxyOperationId),
    InputRejected,
    ProxyReported(ProxyOutcome<Worker, Plan>),
}

impl<Worker, Plan> ProxyInputCustody<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn into_interrupted_worker(
        self,
    ) -> Result<InterruptedWorker<Worker, Plan>, Self> {
        match self {
            Self::InputRejected => Ok(InterruptedWorker::ProxyInputRejected),
            Self::ProxyReported(outcome) => Ok(InterruptedWorker::ProxyReported(outcome)),
            input => Err(input),
        }
    }
}

impl<Worker, Plan> RetiringWork<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn settle_input(
        self,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        (Self, Option<ProxyInputResult<behavior::Here, Worker, Plan>>),
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        let Self::Transferred {
            purpose,
            input: ProxyInputCustody::AwaitingSettlement(witness),
        } = self
        else {
            return Err((self, input));
        };
        match witness.admit(input) {
            Ok(SettledItem::Attempted(ItemSettlement::Accepted(receipt))) => {
                let (_, _, operation) = receipt.into_parts();
                Ok((
                    Self::Transferred {
                        purpose,
                        input: ProxyInputCustody::AwaitingOutcome(operation),
                    },
                    None,
                ))
            }
            Ok(input) => Ok((
                Self::Transferred {
                    purpose,
                    input: ProxyInputCustody::InputRejected,
                },
                Some(input),
            )),
            Err((witness, input)) => Err((
                Self::Transferred {
                    purpose,
                    input: ProxyInputCustody::AwaitingSettlement(witness),
                },
                input,
            )),
        }
    }
}

pub(in super::super) struct ProxyRetirement<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    pub(in super::super) shutdown: ProxyShutdown,
    pub(in super::super) work: RetiringWork<Worker, Plan>,
    pub(in super::super) stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
}

impl<Worker, Plan> ProxyRetirement<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn begin(
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        work: RetiringWork<Worker, Plan>,
    ) -> (Self, ProxyOperation<behavior::Here, Worker, Plan>) {
        let (witness, operation) = ProxyOperation::shutdown(creation);
        (
            Self {
                proxy,
                shutdown: ProxyShutdown::AwaitingSettlement(witness),
                work,
                stopped: None,
            },
            operation,
        )
    }
}

#[cfg(test)]
mod tests {
    use behavior::{Behavior, BehaviorAddr, EndpointAddress};

    use super::{ProxyInputCustody, RetiringWork};
    use crate::{ActivationPlan, ProxyInputResult, StableProxy};

    #[expect(
        dead_code,
        reason = "compile-only proof of direct proxy-input custody outcomes"
    )]
    fn proxy_input_custody_names_each_current_outcome<Worker, Plan>(
        custody: ProxyInputCustody<Worker, Plan>,
    ) where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        match custody {
            ProxyInputCustody::AwaitingSettlement(_) => {}
            ProxyInputCustody::AwaitingOutcome(_) => {}
            ProxyInputCustody::InputRejected => {}
            ProxyInputCustody::ProxyReported(_) => {}
        }
    }

    #[expect(
        dead_code,
        reason = "compile-only proof that entry selection owns input correlation"
    )]
    fn retirement_input_is_already_correlated<Worker, Plan>(
        work: RetiringWork<Worker, Plan>,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) where
        Worker: Behavior,
        Plan: ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        match work.settle_input(input) {
            Ok(_) | Err(_) => {}
        }
    }
}
