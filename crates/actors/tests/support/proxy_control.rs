use behavior::InterpretationProgress;
use behavior::{
    Behavior, BehaviorAddr, ChildInputReason, CreationId, EndpointAddress, EstablishedActor,
    ItemSettlement, Never,
};
use behavior_actors::atomic::{
    ActivationPlan, ProxyControl, ProxyControlAdmission, ProxyInputReceipt, ProxyOperation,
    StableProxy,
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
        assert!(
            self.admitted.is_none(),
            "one control is admitted per test host"
        );
        self.admitted = Some((creation, control));
        ItemSettlement::Accepted(
            self.proxy
                .take()
                .unwrap_or_else(|| panic!("one exact proxy actor is supplied")),
        )
    }
}

/// Settle an actual owner request, retaining its admitted private control for
/// the pure proxy transition exercised by the calling test.
pub fn admit_proxy_operation<Source, Worker, Plan>(
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
    let (actual_creation, control) = host
        .admitted
        .take()
        .unwrap_or_else(|| panic!("the admitted control remains owned by the test host"));
    assert_eq!(actual_creation, expected_creation);
    (expected_creation, control, receipt)
}
