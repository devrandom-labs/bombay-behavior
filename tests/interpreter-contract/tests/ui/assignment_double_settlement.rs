async fn settle_twice<P, Job, Host>(
    request: behavior_actors::atomic::AssignWorker<P, Job>,
    host: &mut Host,
)
where
    P: behavior::Protocol<Msg = behavior_actors::atomic::Assignment<Job>>,
    P::Addr: behavior::EndpointAddress,
    <P::Addr as behavior::EndpointAddress>::Established<P>: Send,
    Job: Send,
    Host: behavior::InterpretItem<behavior::EstablishedDelivery<P>, (), behavior::Here>,
{
    drop(request.settle(host).await);
    drop(request.settle(host).await);
}

fn main() {}
