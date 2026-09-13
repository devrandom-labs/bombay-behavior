//! Customer outcome delivery with exact rejected-submission custody.

use behavior::{
    ActionItem, Delivery, EndpointAddress, EstablishedDelivery, ExactDeliveryReason,
    InterpreterRequest, LogicalDeliveryReason, NoReturnToEmitter, Protocol,
};

use crate::{ReplyDelivery, ReplyRoute};

/// One customer outcome and every capability required if delivery rejects.
///
/// Ordinary accepted and terminal outcomes consume their sole customer route
/// as the delivery target. A synchronously rejected keyed submission instead
/// uses a cloned target and retains the original customer route in this action.
/// A rejecting interpreter therefore returns both capabilities together.
#[doc(hidden)]
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
}

impl<P> ActionItem for CustomerDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress + Send,
    P::Msg: Send,
    <P::Addr as EndpointAddress>::Established<P>: Send,
{
    type Accepted = ();
    type Rejection = ReplyDelivery<LogicalDeliveryReason, ExactDeliveryReason>;
    type Prerequisite = behavior::Never;
}
