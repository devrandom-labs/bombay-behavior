fn dismantle_request<P, Job>(request: behavior_actors::atomic::AssignWorker<P, Job>)
where
    P: behavior::Protocol<Msg = behavior_actors::atomic::Assignment<Job>>,
    P::Addr: behavior::EndpointAddress,
{
    let _ = request.into_parts();
}

fn main() {}
