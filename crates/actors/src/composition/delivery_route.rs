//! Static construction of one concrete delivery effect.

use behavior::{
    Delivery, EndpointAddress, EstablishedDelivery, EstablishedRecipient, Protocol, Recipient,
};

mod sealed {
    pub trait DeliveryRoute<P: behavior::Protocol> {}
}

/// A statically selected destination capability for protocol `P`.
///
/// This is a Behavior Actors construction over existing Behavior delivery
/// effects. It does not interpret communication or combine logical
/// and exact resolution into a runtime choice. Each concrete route has exactly
/// one associated effect type, so an interpreter needs only the authority
/// selected by the template's route parameter.
///
/// Creator-local child routes are deliberately excluded. A standalone
/// [`crate::MessageAdapterWithRoute`] declares [`behavior::NoBirths`] and
/// therefore cannot own the local child binding required to interpret a
/// [`behavior::ChildDelivery`]. Child forwarding belongs in a topology-owning
/// behavior whose birth algebra proves that occurrence.
pub trait DeliveryRoute<P: Protocol>: sealed::DeliveryRoute<P> + Sized {
    /// The one concrete effect produced by this route.
    type Effect;

    /// Consume the capability into one explicit delivery effect.
    fn deliver(self, message: P::Msg) -> Self::Effect;
}

impl<P: Protocol> sealed::DeliveryRoute<P> for Recipient<P> {}

impl<P: Protocol> DeliveryRoute<P> for Recipient<P> {
    type Effect = Delivery<P>;

    fn deliver(self, message: P::Msg) -> Self::Effect {
        Delivery::new(self, message)
    }
}

impl<P> sealed::DeliveryRoute<P> for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}

impl<P> DeliveryRoute<P> for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Effect = EstablishedDelivery<P>;

    fn deliver(self, message: P::Msg) -> Self::Effect {
        EstablishedDelivery::new(self, message)
    }
}
