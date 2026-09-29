use core::future::{Future, ready};
use core::task::{Context, Poll, Waker};

use behavior::{
    EndpointAddress, EstablishedDelivery, ExactDeliveryReason, Here, InterpretItem, ItemSettlement,
    Never, Protocol,
};
use behavior_actors::atomic::{AssignWorker, Assignment, AssignmentReceipt};

enum Admission {
    Accept,
    Reject,
}

/// Test interpreter that actually takes an accepted worker delivery or returns
/// the same delivery on a closed endpoint.
pub struct AssignmentDeliveryHost<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    admission: Admission,
    accepted: Vec<EstablishedDelivery<P>>,
    rejections: usize,
}

impl<P> AssignmentDeliveryHost<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub fn accepting() -> Self {
        Self {
            admission: Admission::Accept,
            accepted: Vec::new(),
            rejections: 0,
        }
    }

    pub fn rejecting() -> Self {
        Self {
            admission: Admission::Reject,
            accepted: Vec::new(),
            rejections: 0,
        }
    }

    pub fn into_accepted(self) -> Vec<EstablishedDelivery<P>> {
        self.accepted
    }

    pub fn rejections(&self) -> usize {
        self.rejections
    }
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
                self.accepted.push(delivery);
                ready(ItemSettlement::Accepted(()))
            }
            Admission::Reject => {
                self.rejections += 1;
                ready(ItemSettlement::Rejected {
                    item: delivery,
                    reason: ExactDeliveryReason::ClosedRecipient,
                })
            }
        }
    }
}

/// Admit one actual exact delivery through the ready test interpreter and
/// return the owner-issued receipt with the delivery captured by that host.
pub fn accepted_assignment<P, Job>(
    request: AssignWorker<P, Job>,
) -> (AssignmentReceipt, EstablishedDelivery<P>)
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
{
    let mut host = AssignmentDeliveryHost::<P>::accepting();
    let settlement = {
        let future = request.settle(&mut host);
        let mut future = core::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(settlement) => settlement,
            Poll::Pending => panic!("the ready exact-delivery interpreter must settle immediately"),
        }
    };
    let ItemSettlement::Accepted(receipt) = settlement else {
        panic!("the accepting exact-delivery interpreter must admit its request");
    };
    let [delivery]: [EstablishedDelivery<P>; 1] = host
        .into_accepted()
        .try_into()
        .unwrap_or_else(|_| panic!("one actual exact delivery is admitted"));
    (receipt, delivery)
}
