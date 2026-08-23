//! Static construction of one concrete delivery effect.

use behavior::{
    Delivery, EndpointAddress, EstablishedDelivery, EstablishedRecipient, InterpretDelivery,
    InterpretEstablishedDelivery, InterpretSends, Own, Protocol, Recipient, SendEffects, SendInput,
    SendInterpreter, SendsFor,
};
use core::future::Future;

mod sealed {
    pub trait DeliveryRoute<P: behavior::Protocol> {}
    pub trait DeliveryRouteProtocol {}
}

/// The protocol carried by a concrete delivery capability.
///
/// This projection lets a stateful template retain a route without repeating
/// an independently selectable protocol parameter. Implementations remain
/// sealed to the exact route families supported by [`DeliveryRoute`].
pub trait DeliveryRouteProtocol: sealed::DeliveryRouteProtocol {
    type Protocol: Protocol;
    /// The concrete sends product produced by this route.
    type Sends: SendEffects;

    /// Consume the capability into one explicit ordered delivery product.
    fn deliver(self, message: <Self::Protocol as Protocol>::Msg) -> Self::Sends;
}

/// A statically selected destination capability for protocol `P`.
///
/// This is a Behavior Actors construction over existing Behavior delivery
/// effects. Each route selects one concrete ordered sends product. A template
/// generic over `Route` therefore preserves the supplied capability without
/// weakening an established endpoint to a logical name or requiring exact
/// endpoint support for a deliberately logical-only instantiation.
///
/// Creator-local child routes are deliberately excluded. A standalone
/// [`crate::MessageAdapterWithRoute`] declares [`behavior::NoBirths`] and
/// therefore cannot own the local child binding required to interpret a
/// [`behavior::ChildDelivery`]. Child forwarding belongs in a topology-owning
/// behavior whose birth algebra proves that occurrence.
///
/// A route for another protocol cannot be substituted merely because its
/// payload has the same Rust type:
///
/// ```compile_fail
/// use behavior::{MailAddr, MessageProtocol, Recipient};
/// use behavior_actors::DeliveryRoute;
/// type Expected = MessageProtocol<MailAddr, u8>;
/// struct Other;
/// impl behavior::Protocol for Other {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// fn require_expected<R: DeliveryRoute<Expected>>(_: R) {}
/// require_expected(Recipient::<Other>::global(MailAddr(1)));
/// ```
pub trait DeliveryRoute<P: Protocol>:
    sealed::DeliveryRoute<P> + DeliveryRouteProtocol<Protocol = P> + Sized
{
}

impl<P: Protocol> sealed::DeliveryRoute<P> for Recipient<P> {}
impl<P: Protocol> sealed::DeliveryRouteProtocol for Recipient<P> {}
impl<P: Protocol> DeliveryRouteProtocol for Recipient<P> {
    type Protocol = P;
    type Sends = Vec<Delivery<P>>;

    fn deliver(self, message: P::Msg) -> Self::Sends {
        vec![Delivery::new(self, message)]
    }
}
impl<P: Protocol> DeliveryRoute<P> for Recipient<P> {}

impl<P> sealed::DeliveryRoute<P> for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
impl<P> sealed::DeliveryRouteProtocol for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
impl<P> DeliveryRouteProtocol for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Protocol = P;
    type Sends = Vec<EstablishedDelivery<P>>;

    fn deliver(self, message: P::Msg) -> Self::Sends {
        vec![EstablishedDelivery::new(self, message)]
    }
}
impl<P> DeliveryRoute<P> for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}

/// One customer capability that truthfully retains logical or exact routing.
///
/// This closed sum is useful when one running actor must serve both stable
/// logical customers and exact external customers through the same protocol.
/// Selecting a variant does not resolve, weaken, or otherwise convert the
/// enclosed capability. Address types without an established endpoint family
/// continue to use `Recipient<P>` directly as their route parameter.
pub enum ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    /// Resolve the logical name when the delivery is interpreted.
    Logical(Recipient<P>),
    /// Deliver to this one installed incarnation without name resolution.
    Established(EstablishedRecipient<P>),
}

impl<P> Clone for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn clone(&self) -> Self {
        match self {
            Self::Logical(recipient) => Self::Logical(*recipient),
            Self::Established(recipient) => Self::Established(recipient.clone()),
        }
    }
}

impl<P> ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    /// Preserve one logical customer capability.
    #[must_use]
    pub const fn logical(recipient: Recipient<P>) -> Self {
        Self::Logical(recipient)
    }

    /// Preserve one exact installed customer capability.
    #[must_use]
    pub const fn established(recipient: EstablishedRecipient<P>) -> Self {
        Self::Established(recipient)
    }
}

impl<P> From<Recipient<P>> for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn from(recipient: Recipient<P>) -> Self {
        Self::logical(recipient)
    }
}

impl<P> From<EstablishedRecipient<P>> for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn from(recipient: EstablishedRecipient<P>) -> Self {
        Self::established(recipient)
    }
}

/// One delivery whose logical-versus-exact capability remains explicit.
pub enum ReplyDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    /// One logical delivery requiring name resolution.
    Logical(Delivery<P>),
    /// One exact delivery to a retained installed incarnation.
    Established(EstablishedDelivery<P>),
}

impl<P> Clone for ReplyDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    P::Msg: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Logical(delivery) => Self::Logical(delivery.clone()),
            Self::Established(delivery) => Self::Established(delivery.clone()),
        }
    }
}

impl<P> core::fmt::Debug for ReplyDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: core::fmt::Debug,
    EstablishedDelivery<P>: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Logical(delivery) => formatter.debug_tuple("Logical").field(delivery).finish(),
            Self::Established(delivery) => formatter
                .debug_tuple("Established")
                .field(delivery)
                .finish(),
        }
    }
}

impl<P> PartialEq for ReplyDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: PartialEq,
    EstablishedDelivery<P>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Logical(left), Self::Logical(right)) => left == right,
            (Self::Established(left), Self::Established(right)) => left == right,
            (Self::Logical(_), Self::Established(_)) | (Self::Established(_), Self::Logical(_)) => {
                false
            }
        }
    }
}

impl<P> Eq for ReplyDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: Eq,
    EstablishedDelivery<P>: Eq,
{
}

/// Ordered customer-delivery lane preserving each route alternative.
pub struct ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    deliveries: Vec<ReplyDelivery<P>>,
}

impl<P> ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    /// Construct one ordered customer-delivery lane.
    #[must_use]
    pub fn new(deliveries: Vec<ReplyDelivery<P>>) -> Self {
        Self { deliveries }
    }

    /// Inspect the complete delivery order without exposing endpoint internals.
    #[must_use]
    pub fn as_slice(&self) -> &[ReplyDelivery<P>] {
        &self.deliveries
    }

    /// Consume the lane into its complete ordered alternatives.
    #[must_use]
    pub fn into_deliveries(self) -> Vec<ReplyDelivery<P>> {
        self.deliveries
    }
}

impl<P> Clone for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    P::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.deliveries.clone())
    }
}

impl<P> core::fmt::Debug for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    ReplyDelivery<P>: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_list().entries(&self.deliveries).finish()
    }
}

impl<P> PartialEq for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    ReplyDelivery<P>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.deliveries == other.deliveries
    }
}

impl<P> Eq for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    ReplyDelivery<P>: Eq,
{
}

impl<P> SendEffects for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn empty() -> Self {
        Self::new(Vec::new())
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
    }
}

impl<Event, P> SendsFor<Event> for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}

impl<P> SendInput<ReplyDelivery<P>, Own> for ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn emit(&mut self, input: ReplyDelivery<P>) {
        self.deliveries.push(input);
    }
}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for ReplyDeliveries<P>
where
    Interpreter: SendInterpreter + InterpretDelivery<P> + InterpretEstablishedDelivery<P>,
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: Send,
    EstablishedDelivery<P>: Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            for delivery in self.deliveries {
                match delivery {
                    ReplyDelivery::Logical(delivery) => {
                        <Vec<Delivery<P>> as InterpretSends<Interpreter, RootEvent, Path>>::interpret(
                            vec![delivery],
                            interpreter,
                        )
                        .await?;
                    }
                    ReplyDelivery::Established(delivery) => {
                        <Vec<EstablishedDelivery<P>> as InterpretSends<
                            Interpreter,
                            RootEvent,
                            Path,
                        >>::interpret(vec![delivery], interpreter)
                        .await?;
                    }
                }
            }
            Ok(())
        }
    }
}

impl<P> sealed::DeliveryRoute<P> for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
impl<P> sealed::DeliveryRouteProtocol for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
impl<P> DeliveryRouteProtocol for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Protocol = P;
    type Sends = ReplyDeliveries<P>;

    fn deliver(self, message: P::Msg) -> Self::Sends {
        let delivery = match self {
            Self::Logical(recipient) => ReplyDelivery::Logical(Delivery::new(recipient, message)),
            Self::Established(recipient) => {
                ReplyDelivery::Established(EstablishedDelivery::new(recipient, message))
            }
        };
        ReplyDeliveries::new(vec![delivery])
    }
}
impl<P> DeliveryRoute<P> for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
