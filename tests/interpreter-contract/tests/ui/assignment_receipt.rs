fn steal_receipt<P, Job>(
    request: behavior_actors::pool::AssignWorker<P, Job>,
) -> behavior_actors::pool::AssignmentReceipt
where
    P: behavior::Protocol<Msg = behavior_actors::pool::Assignment<Job>>,
    P::Addr: behavior::EndpointAddress,
{
    request.receipt()
}

fn main() {}
