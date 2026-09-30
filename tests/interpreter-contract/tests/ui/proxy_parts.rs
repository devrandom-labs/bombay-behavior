fn dismantle<Source, Worker, Plan>(
    operation: behavior_actors::atomic::ProxyOperation<Source, Worker, Plan>,
)
where
    Worker: behavior::Behavior,
    Plan: behavior_actors::atomic::ActivationPlan,
    behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
    behavior_actors::atomic::StableProxy<Worker, Plan>:
        behavior::Behavior<Protocol = Worker::Protocol>,
{
    let _ = operation.into_parts();
}

fn main() {}
