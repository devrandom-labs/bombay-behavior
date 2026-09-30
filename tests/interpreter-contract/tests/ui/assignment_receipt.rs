fn steal_receipt<P, Job>(
    request: behavior_actors::atomic::AssignWorker<P, Job>,
) -> behavior_actors::atomic::AssignmentReceipt
where
    P: behavior::Protocol<Msg = behavior_actors::atomic::Assignment<Job>>,
    P::Addr: behavior::EndpointAddress,
{
    request.receipt()
}

fn main() {}
