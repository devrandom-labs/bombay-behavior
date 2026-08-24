//! Static construction of one concrete delivery effect.

use behavior::{
    Behavior, BehaviorAddr, ChildDelivery, ChildRoute, Delivery, EndpointAddress,
    EstablishedDelivery, EstablishedRecipient, InterpretDelivery, InterpretEstablishedDelivery,
    InterpretSends, Own, Protocol, Recipient, ResolveChildOccurrence, SendEffects, SendInput,
    SendInterpreter, SendsFor,
};
use core::future::Future;

mod sealed {
    pub trait DeliveryRoute {}
    pub trait DeliveryRouteFor<Owner: behavior::Behavior> {}
}

/// A statically selected transferable destination capability.
///
/// The associated protocol prevents actor templates from repeating a separate
/// protocol parameter beside the route that already determines it. Logical
/// and established routes select different concrete send products without
/// weakening either capability.
pub trait DeliveryRoute: sealed::DeliveryRoute + Sized {
    /// Protocol selected by this capability.
    type Protocol: Protocol;
    /// The concrete sends product produced by this route.
    type Sends: SendEffects;

    /// Consume the capability into one explicit ordered delivery product.
    fn deliver(self, message: <Self::Protocol as Protocol>::Msg) -> Self::Sends;
}

/// Creator-local child routes are deliberately excluded. A standalone
/// [`crate::MessageAdapterWithRoute`] declares [`behavior::NoBirths`] and
/// therefore cannot own the local child binding required to interpret a
/// [`behavior::ChildDelivery`]. Child forwarding belongs in a topology-owning
/// behavior whose birth algebra proves that occurrence.
///
/// A route for another protocol cannot be substituted merely because its
/// payload has the same Rust type:
///
/// ```compile_fail,E0277
/// use behavior::{MailAddr, MessageProtocol, Recipient};
/// use behavior_actors::DeliveryRoute;
/// type Expected = MessageProtocol<MailAddr, u8>;
/// struct Other;
/// impl behavior::Protocol for Other {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// fn require_expected<R: DeliveryRoute<Protocol = Expected>>(_: R) {}
/// require_expected(Recipient::<Other>::global(MailAddr(1)));
/// ```
/// A delivery capability interpreted in the namespace of one emitting owner.
///
/// Logical and established recipients are transferable acquaintances and are
/// therefore valid for any owner in the same address namespace. A
/// [`ChildRoute`] is valid only when `Owner` statically resolves that exact
/// direct occurrence from its own [`Behavior::Birth`] algebra. The route is
/// not made transferable and no runtime ownership check is introduced.
///
/// This contract constructs one concrete send product. It performs no
/// delivery and introduces no effect beyond the existing logical,
/// established, or creator-local delivery values.
///
/// A child route cannot be emitted by an owner with no matching birth:
///
/// ```compile_fail,E0277
/// use behavior::{ChildHead, ChildRoute, MailAddr, MessageProtocol, Recipient};
/// use behavior_actors::{Cache, CacheResult, DeliveryRouteFor};
/// type Reply = MessageProtocol<MailAddr, CacheResult<(), ()>>;
/// type Owner = Cache<MailAddr, (), (), Recipient<Reply>>;
/// fn require<R: DeliveryRouteFor<Owner>>(_: R) {}
/// require(ChildRoute::<Owner, ChildHead>::new(1));
/// ```
pub trait DeliveryRouteFor<Owner: Behavior>: sealed::DeliveryRouteFor<Owner> + Sized {
    /// Protocol selected by this owner-scoped capability.
    type Protocol: Protocol<Addr = BehaviorAddr<Owner>>;
    /// Concrete send product selected by this owner-scoped capability.
    type Sends: SendEffects;

    /// Consume the capability after proving it belongs to `Owner`.
    fn deliver_for(self, message: <Self::Protocol as Protocol>::Msg) -> Self::Sends;
}

impl<P: Protocol> sealed::DeliveryRoute for Recipient<P> {}
impl<P: Protocol> DeliveryRoute for Recipient<P> {
    type Protocol = P;
    type Sends = Vec<Delivery<P>>;

    fn deliver(self, message: P::Msg) -> Self::Sends {
        vec![Delivery::new(self, message)]
    }
}

impl<Owner, P> sealed::DeliveryRouteFor<Owner> for Recipient<P>
where
    Owner: Behavior,
    P: Protocol<Addr = BehaviorAddr<Owner>>,
{
}
impl<Owner, P> DeliveryRouteFor<Owner> for Recipient<P>
where
    Owner: Behavior,
    P: Protocol<Addr = BehaviorAddr<Owner>>,
{
    type Protocol = P;
    type Sends = Vec<Delivery<P>>;

    fn deliver_for(self, message: P::Msg) -> Self::Sends {
        DeliveryRoute::deliver(self, message)
    }
}

impl<P> sealed::DeliveryRoute for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
impl<P> DeliveryRoute for EstablishedRecipient<P>
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

impl<Owner, P> sealed::DeliveryRouteFor<Owner> for EstablishedRecipient<P>
where
    Owner: Behavior,
    P: Protocol<Addr = BehaviorAddr<Owner>>,
    P::Addr: EndpointAddress,
{
}
impl<Owner, P> DeliveryRouteFor<Owner> for EstablishedRecipient<P>
where
    Owner: Behavior,
    P: Protocol<Addr = BehaviorAddr<Owner>>,
    P::Addr: EndpointAddress,
{
    type Protocol = P;
    type Sends = Vec<EstablishedDelivery<P>>;

    fn deliver_for(self, message: P::Msg) -> Self::Sends {
        DeliveryRoute::deliver(self, message)
    }
}

impl<Owner, C, Occurrence> sealed::DeliveryRouteFor<Owner> for ChildRoute<C, Occurrence>
where
    Owner: ResolveChildOccurrence<Occurrence, Child = C>,
    C: Behavior,
    C::Protocol: Protocol<Addr = BehaviorAddr<Owner>>,
{
}
impl<Owner, C, Occurrence> DeliveryRouteFor<Owner> for ChildRoute<C, Occurrence>
where
    Owner: ResolveChildOccurrence<Occurrence, Child = C>,
    C: Behavior,
    C::Protocol: Protocol<Addr = BehaviorAddr<Owner>>,
{
    type Protocol = C::Protocol;
    type Sends = Vec<ChildDelivery<C::Protocol, Occurrence>>;

    fn deliver_for(self, message: <C::Protocol as Protocol>::Msg) -> Self::Sends {
        vec![ChildDelivery::at(self, message)]
    }
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

impl<P> sealed::DeliveryRoute for ReplyRoute<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
}
impl<P> DeliveryRoute for ReplyRoute<P>
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

impl<Owner, P> sealed::DeliveryRouteFor<Owner> for ReplyRoute<P>
where
    Owner: Behavior,
    P: Protocol<Addr = BehaviorAddr<Owner>>,
    P::Addr: EndpointAddress,
{
}
impl<Owner, P> DeliveryRouteFor<Owner> for ReplyRoute<P>
where
    Owner: Behavior,
    P: Protocol<Addr = BehaviorAddr<Owner>>,
    P::Addr: EndpointAddress,
{
    type Protocol = P;
    type Sends = ReplyDeliveries<P>;

    fn deliver_for(self, message: P::Msg) -> Self::Sends {
        DeliveryRoute::deliver(self, message)
    }
}
