fn settle_twice<Source, Worker, Plan, Host>(
    operation: behavior_actors::atomic::ProxyOperation<Source, Worker, Plan>,
    host: &mut Host,
)
where
    Worker: behavior::Behavior,
    Plan: behavior_actors::atomic::ActivationPlan,
    behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
    behavior_actors::atomic::StableProxy<Worker, Plan>:
        behavior::Behavior<Protocol = Worker::Protocol>,
    Host: behavior_actors::atomic::ProxyControlAdmission<Worker, Plan>,
{
    let _accepted = operation.settle(host);
    let _duplicate = operation.settle(host);
}

fn main() {}
