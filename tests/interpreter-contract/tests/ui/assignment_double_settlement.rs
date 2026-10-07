use behavior::{
    EndpointAddress, EstablishedDelivery, Here, InterpretItem, InterpretationProgress, Protocol,
};
use behavior_actors::atomic::{AssignWorker, Assignment};

// Borrowed polling can repeat; the same original move-only request cannot enter two owners.
async fn reuse_original_assignment<P, Job, Host>(request: AssignWorker<P, Job>, host: &mut Host)
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
    Host: InterpretItem<EstablishedDelivery<P>, (), Here>,
{
    let mut progress = Some(InterpretationProgress::Original(request));
    AssignWorker::<P, Job>::settle::<_, (), Here>(&mut progress, host).await;
    let mut duplicate = Some(InterpretationProgress::Original(request));
    AssignWorker::<P, Job>::settle::<_, (), Here>(&mut duplicate, host).await;
}

fn main() {}
