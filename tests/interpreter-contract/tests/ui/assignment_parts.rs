fn dismantle_request<P, Job>(request: behavior_actors::pool::AssignWorker<P, Job>)
where
    P: behavior::Protocol<Msg = behavior_actors::pool::Assignment<Job>>,
    P::Addr: behavior::EndpointAddress,
{
    let _ = request.into_parts();
}

fn main() {}
