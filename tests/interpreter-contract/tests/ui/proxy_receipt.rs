fn assemble<Worker, Plan>()
where
    Worker: behavior::Behavior,
    Plan: behavior_actors::atomic::ActivationPlan,
    behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
    behavior_actors::atomic::StableProxy<Worker, Plan>:
        behavior::Behavior<Protocol = Worker::Protocol>,
{
    let _ = behavior_actors::atomic::ProxyInputReceipt::<Worker, Plan>::new;
}

fn main() {}
