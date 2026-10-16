fn assemble<Worker, Plan>()
where
    Worker: behavior::Behavior,
    Plan: behavior_actors::worker::StartupPlan,
    behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
    behavior_actors::proxy::StableProxy<Worker, Plan>:
        behavior::Behavior<Protocol = Worker::Protocol>,
{
    let _ = behavior_actors::proxy::ProxyInputReceipt::<Worker, Plan>::new;
}

fn main() {}
