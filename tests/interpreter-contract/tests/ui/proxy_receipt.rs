fn assemble<Worker, Plan>(
    creation: behavior::CreationId,
    proxy: behavior::EstablishedActor<behavior_actors::atomic::StableProxy<Worker, Plan>>,
    operation: behavior_actors::atomic::ProxyOperationId,
) -> behavior_actors::atomic::ProxyInputReceipt<Worker, Plan>
where
    Worker: behavior::Behavior,
    Plan: behavior_actors::atomic::ActivationPlan,
    behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
    behavior_actors::atomic::StableProxy<Worker, Plan>:
        behavior::Behavior<Protocol = Worker::Protocol>,
{
    behavior_actors::atomic::ProxyInputReceipt::new(creation, proxy, operation)
}

fn main() {}
