use core::future::{Future, ready};
use core::task::{Context, Poll, Waker};

use behavior_actors::atomic::{AssignWorker, Assignment, AssignmentReceipt};
use behavior_core::{
    EndpointAddress, EstablishedDelivery, ExactDeliveryReason, Here, InterpretItem, ItemSettlement,
    Never, Protocol,
};

enum Admission {
    Accept,
    Reject,
}

struct AssignmentDeliveryHost<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    admission: Admission,
    accepted: Option<EstablishedDelivery<P>>,
}

impl<P, Job> InterpretItem<EstablishedDelivery<P>, (), Here> for AssignmentDeliveryHost<P>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
{
    fn interpret_item(
        &mut self,
        delivery: EstablishedDelivery<P>,
    ) -> impl Future<Output = ItemSettlement<EstablishedDelivery<P>, (), ExactDeliveryReason, Never>>
    + Send {
        match self.admission {
            Admission::Accept => {
                let previous = self.accepted.replace(delivery);
                assert!(previous.is_none());
                ready(ItemSettlement::Accepted(()))
            }
            Admission::Reject => ready(ItemSettlement::Rejected {
                item: delivery,
                reason: ExactDeliveryReason::ClosedRecipient,
            }),
        }
    }
}

fn settle_now<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(settlement) => settlement,
        Poll::Pending => panic!("the exact-delivery fuzz interpreter must settle immediately"),
    }
}

pub(super) fn accept_assignment<P, Job>(
    request: AssignWorker<P, Job>,
) -> (AssignmentReceipt, EstablishedDelivery<P>)
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
{
    let mut host = AssignmentDeliveryHost {
        admission: Admission::Accept,
        accepted: None,
    };
    let ItemSettlement::Accepted(receipt) = settle_now(request.settle(&mut host)) else {
        panic!("accepting exact-delivery interpreter returns one owner receipt");
    };
    let delivery = host
        .accepted
        .expect("accepting interpreter takes the exact delivery");
    (receipt, delivery)
}

pub(super) fn reject_assignment<P, Job>(
    request: AssignWorker<P, Job>,
) -> ItemSettlement<AssignWorker<P, Job>, AssignmentReceipt, ExactDeliveryReason, Never>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
{
    let mut host = AssignmentDeliveryHost {
        admission: Admission::Reject,
        accepted: None,
    };
    let settlement = settle_now(request.settle(&mut host));
    assert!(host.accepted.is_none());
    assert!(matches!(&settlement, ItemSettlement::Rejected { .. }));
    settlement
}
