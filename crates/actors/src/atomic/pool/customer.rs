//! Customer outcome delivery with exact rejected-submission custody.

use behavior::{
    ActionItem, Delivery, EndpointAddress, EstablishedDelivery, ExactDeliveryReason,
    InterpretationProgress, InterpreterRequest, ItemSettlement, LogicalDeliveryReason,
    NoReturnToEmitter, Protocol, finish_item, prepare_item,
};

use crate::{ReplyDelivery, ReplyRoute};

/// One customer outcome and every capability required if delivery rejects.
///
/// Ordinary accepted and terminal outcomes consume their sole customer route
/// as the delivery target. A synchronously rejected keyed submission instead
/// uses a cloned target and retains the original customer route in this action.
/// A rejecting interpreter therefore returns both capabilities together.
#[must_use = "customer delivery must be interpreted or retained"]
pub enum CustomerDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    /// One outcome targeting a logical customer.
    Logical {
        /// Concrete logical delivery.
        delivery: Delivery<P>,
    },
    /// One outcome targeting an exact customer.
    Established {
        /// Concrete exact delivery.
        delivery: EstablishedDelivery<P>,
    },
    /// One rejected submission targeting a clone of a logical customer.
    RejectedLogical {
        /// Concrete delivery using the cloned target.
        delivery: Delivery<P>,
        /// Original unchanged customer route.
        customer: ReplyRoute<P>,
    },
    /// One rejected submission targeting a clone of an exact customer.
    RejectedEstablished {
        /// Concrete delivery using the cloned target.
        delivery: EstablishedDelivery<P>,
        /// Original unchanged customer route.
        customer: ReplyRoute<P>,
    },
}

impl<P> CustomerDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub(in crate::atomic) fn outcome(route: ReplyRoute<P>, outcome: P::Msg) -> Self {
        match route {
            ReplyRoute::Logical(to) => Self::Logical {
                delivery: Delivery::new(to, outcome),
            },
            ReplyRoute::Established(to) => Self::Established {
                delivery: EstablishedDelivery::new(to, outcome),
            },
        }
    }

    pub(in crate::atomic) fn rejected(route: ReplyRoute<P>, outcome: P::Msg) -> Self {
        match route.clone() {
            ReplyRoute::Logical(to) => Self::RejectedLogical {
                delivery: Delivery::new(to, outcome),
                customer: route,
            },
            ReplyRoute::Established(to) => Self::RejectedEstablished {
                delivery: EstablishedDelivery::new(to, outcome),
                customer: route,
            },
        }
    }
}

impl<P> InterpreterRequest for CustomerDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type ReturnToEmitter = NoReturnToEmitter;
    type LogicalProtocols = behavior::BirthProtocol<P, behavior::NoBirthProtocols>;
}

impl<P> ActionItem for CustomerDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress + Send,
    P::Msg: Send,
    <P::Addr as EndpointAddress>::Established<P>: Send,
{
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;

    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }

    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        match custody {
            (input @ Some(_), received @ None) => Some((input, received)),
            _ => None,
        }
    }

    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = ();
    type Rejection = ReplyDelivery<LogicalDeliveryReason, ExactDeliveryReason>;
    type Prerequisite = behavior::Never;
}
