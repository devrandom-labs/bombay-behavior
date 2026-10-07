use behavior::{Behavior, BehaviorAddr, EndpointAddress, InterpretationProgress};
use behavior_actors::atomic::{ActivationPlan, ProxyControlAdmission, ProxyOperation, StableProxy};

// Borrowed polling can repeat; the same original private authority cannot enter two owners.
fn reuse_original_proxy_operation<Source, Worker, Plan, Host>(
    operation: ProxyOperation<Source, Worker, Plan>,
    host: &mut Host,
) where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    Host: ProxyControlAdmission<Worker, Plan>,
{
    let mut progress = Some(InterpretationProgress::Original(operation));
    ProxyOperation::settle(&mut progress, host);
    let mut duplicate = Some(InterpretationProgress::Original(operation));
    ProxyOperation::settle(&mut duplicate, host);
}

fn main() {}
