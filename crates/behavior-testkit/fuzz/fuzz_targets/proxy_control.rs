//! Exact proxy-control admission for interpreter-facing fuzz fixtures.

use behavior_actors::atomic::{
    ActivationPlan, ProxyControl, ProxyControlAdmission, ProxyInputReceipt, ProxyOperation,
    StableProxy,
};
use behavior_core::{
    Behavior, BehaviorAddr, ChildInputReason, CreationId, EndpointAddress, EstablishedActor,
    InterpretationProgress, ItemSettlement, Never,
};

struct ProxyControlHost<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    proxy: Option<EstablishedActor<StableProxy<Worker, Plan>>>,
    admitted: Option<(CreationId, ProxyControl<Worker, Plan>)>,
}

impl<Worker, Plan> ProxyControlAdmission<Worker, Plan> for ProxyControlHost<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn admit_proxy_control(
        &mut self,
        creation: CreationId,
        control: ProxyControl<Worker, Plan>,
    ) -> ItemSettlement<
        ProxyControl<Worker, Plan>,
        EstablishedActor<StableProxy<Worker, Plan>>,
        ChildInputReason,
        Never,
    > {
        assert!(self.admitted.is_none(), "one control per admission");
        self.admitted = Some((creation, control));
        ItemSettlement::Accepted(self.proxy.take().expect("one exact proxy actor"))
    }
}

pub(crate) fn admit_proxy_operation<Source, Worker, Plan>(
    operation: ProxyOperation<Source, Worker, Plan>,
    proxy: EstablishedActor<StableProxy<Worker, Plan>>,
) -> (
    CreationId,
    ProxyControl<Worker, Plan>,
    ProxyInputReceipt<Worker, Plan>,
)
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let expected_creation = operation.creation();
    let mut host = ProxyControlHost {
        proxy: Some(proxy),
        admitted: None,
    };
    let ItemSettlement::Accepted(receipt) = ({
        let mut progress = Some(InterpretationProgress::Original(operation));
        ProxyOperation::settle(&mut progress, &mut host);
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual proxy host must return its complete settlement");
        };
        settlement.into_settlement()
    }) else {
        panic!("the exact proxy control is admitted");
    };
    let (actual_creation, control) = host.admitted.take().expect("the admitted control");
    assert_eq!(actual_creation, expected_creation);
    (expected_creation, control, receipt)
}
