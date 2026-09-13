//! Static construction of one concrete delivery effect.

use behavior::{
    ActionItem, ActionItemResult, Behavior, BehaviorAddr, Delivery, EndpointAddress,
    EstablishedDelivery, EstablishedRecipient, InterpretItem, InterpretSends, Interpretation, Own,
    Protocol, Recipient, SendEffects, SendInput, SendSettlements, SendsFor, SettledItem,
    settle_item,
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
/// therefore valid for any owner in the same address namespace. Creator-local
/// child communication is deliberately excluded because its creation ID is
/// meaningful only with the owner's statically selected child occurrence.
///
/// This contract constructs one concrete send product. It performs no
/// delivery and introduces no effect beyond the existing logical,
/// established, or creator-local delivery values.
///
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
pub enum ReplyDelivery<Logical, Established> {
    /// One logical delivery requiring name resolution.
    Logical(Logical),
    /// One exact delivery to a retained installed incarnation.
    Established(Established),
}

impl<Logical, Established> behavior::ClassifySettlement for ReplyDelivery<Logical, Established>
where
    Logical: behavior::ClassifySettlement,
    Established: behavior::ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        match self {
            Self::Logical(delivery) => delivery.settlement_status(),
            Self::Established(delivery) => delivery.settlement_status(),
        }
    }
}

impl<Logical: Clone, Established: Clone> Clone for ReplyDelivery<Logical, Established> {
    fn clone(&self) -> Self {
        match self {
            Self::Logical(delivery) => Self::Logical(delivery.clone()),
            Self::Established(delivery) => Self::Established(delivery.clone()),
        }
    }
}

impl<Logical, Established> core::fmt::Debug for ReplyDelivery<Logical, Established>
where
    Logical: core::fmt::Debug,
    Established: core::fmt::Debug,
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

impl<Logical, Established> PartialEq for ReplyDelivery<Logical, Established>
where
    Logical: PartialEq,
    Established: PartialEq,
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

impl<Logical: Eq, Established: Eq> Eq for ReplyDelivery<Logical, Established> {}

/// Ordered customer-delivery lane preserving each route alternative.
pub struct ReplyDeliveries<Logical, Established> {
    deliveries: Vec<ReplyDelivery<Logical, Established>>,
}

impl<Logical, Established> ReplyDeliveries<Logical, Established> {
    /// Construct one ordered customer-delivery lane.
    #[must_use]
    pub fn new(deliveries: Vec<ReplyDelivery<Logical, Established>>) -> Self {
        Self { deliveries }
    }

    /// Inspect the complete delivery order without exposing endpoint internals.
    #[must_use]
    pub fn as_slice(&self) -> &[ReplyDelivery<Logical, Established>] {
        &self.deliveries
    }

    /// Consume the lane into its complete ordered alternatives.
    #[must_use]
    pub fn into_deliveries(self) -> Vec<ReplyDelivery<Logical, Established>> {
        self.deliveries
    }
}

impl<Logical: Clone, Established: Clone> Clone for ReplyDeliveries<Logical, Established> {
    fn clone(&self) -> Self {
        Self::new(self.deliveries.clone())
    }
}

impl<Logical, Established> core::fmt::Debug for ReplyDeliveries<Logical, Established>
where
    ReplyDelivery<Logical, Established>: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_list().entries(&self.deliveries).finish()
    }
}

impl<Logical, Established> PartialEq for ReplyDeliveries<Logical, Established>
where
    ReplyDelivery<Logical, Established>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.deliveries == other.deliveries
    }
}

impl<Logical, Established> Eq for ReplyDeliveries<Logical, Established> where
    ReplyDelivery<Logical, Established>: Eq
{
}

impl<Logical, Established> SendEffects for ReplyDeliveries<Logical, Established> {
    fn empty() -> Self {
        Self::new(Vec::new())
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
    }
}

impl<Logical, Established> behavior::ClassifySettlement for ReplyDeliveries<Logical, Established>
where
    Logical: behavior::ClassifySettlement,
    Established: behavior::ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        self.deliveries.settlement_status()
    }
}

impl<Event, Logical, Established> SendsFor<Event> for ReplyDeliveries<Logical, Established> {}

impl<Logical, Established> SendInput<ReplyDelivery<Logical, Established>, Own>
    for ReplyDeliveries<Logical, Established>
{
    fn emit(&mut self, input: ReplyDelivery<Logical, Established>) {
        self.deliveries.push(input);
    }
}

impl<P> SendSettlements for ReplyDeliveries<Delivery<P>, EstablishedDelivery<P>>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: ActionItem,
    EstablishedDelivery<P>: ActionItem,
{
    type Settlements =
        ReplyDeliveries<ActionItemResult<Delivery<P>>, ActionItemResult<EstablishedDelivery<P>>>;

    fn unattempted(self) -> Self::Settlements {
        ReplyDeliveries::new(
            self.deliveries
                .into_iter()
                .map(|delivery| match delivery {
                    ReplyDelivery::Logical(delivery) => {
                        ReplyDelivery::Logical(SettledItem::Unattempted(delivery))
                    }
                    ReplyDelivery::Established(delivery) => {
                        ReplyDelivery::Established(SettledItem::Unattempted(delivery))
                    }
                })
                .collect(),
        )
    }
}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for ReplyDeliveries<Delivery<P>, EstablishedDelivery<P>>
where
    Interpreter: InterpretItem<Delivery<P>, RootEvent, Path>
        + InterpretItem<EstablishedDelivery<P>, RootEvent, Path>
        + Send,
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: ActionItem,
    EstablishedDelivery<P>: ActionItem,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        async move {
            let mut source = self.deliveries.into_iter();
            let mut settlements = Vec::with_capacity(source.len());
            while let Some(delivery) = source.next() {
                let settlement = match delivery {
                    ReplyDelivery::Logical(delivery) => {
                        settle_item::<Delivery<P>, Interpreter, RootEvent, Path>(
                            delivery,
                            interpreter,
                        )
                        .await
                        .map(ReplyDelivery::Logical)
                    }
                    ReplyDelivery::Established(delivery) => {
                        settle_item::<EstablishedDelivery<P>, Interpreter, RootEvent, Path>(
                            delivery,
                            interpreter,
                        )
                        .await
                        .map(ReplyDelivery::Established)
                    }
                };
                match settlement {
                    Interpretation::Complete(settlement) => settlements.push(settlement),
                    Interpretation::Corrupt(settlement) => {
                        settlements.push(settlement);
                        settlements.extend(source.map(|delivery| match delivery {
                            ReplyDelivery::Logical(delivery) => {
                                ReplyDelivery::Logical(SettledItem::Unattempted(delivery))
                            }
                            ReplyDelivery::Established(delivery) => {
                                ReplyDelivery::Established(SettledItem::Unattempted(delivery))
                            }
                        }));
                        return Interpretation::Corrupt(ReplyDeliveries::new(settlements));
                    }
                }
            }
            Interpretation::Complete(ReplyDeliveries::new(settlements))
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
    type Sends = ReplyDeliveries<Delivery<P>, EstablishedDelivery<P>>;

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
    type Sends = ReplyDeliveries<Delivery<P>, EstablishedDelivery<P>>;

    fn deliver_for(self, message: P::Msg) -> Self::Sends {
        DeliveryRoute::deliver(self, message)
    }
}
