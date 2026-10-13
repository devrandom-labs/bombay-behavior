fn dismantle<Source, Worker, Plan>(
    operation: behavior_actors::proxy::ProxyOperation<Source, Worker, Plan>,
)
where
    Worker: behavior::Behavior,
    Plan: behavior_actors::worker::StartupPlan,
    behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
    behavior_actors::proxy::StableProxy<Worker, Plan>:
        behavior::Behavior<Protocol = Worker::Protocol>,
{
    let _ = operation.into_parts();
}

fn main() {}
