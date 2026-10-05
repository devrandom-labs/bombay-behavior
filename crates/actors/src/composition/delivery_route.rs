//! Static construction of one concrete delivery effect.

use behavior::{
    ActionItem, ActionItemResult, Delivery, EndpointAddress, EstablishedDelivery,
    EstablishedRecipient, InterpretItem, InterpretSends, Interpretation, InterpretationProgress,
    ItemSettlement, Own, Protocol, Recipient, RecipientAddress, SendEffects, SendInput,
    SendSettlements, SendsFor, SettledItem, SourceCustody, SourceProgress, SourceSettlementCustody,
};
use core::future::Future;

mod sealed {
    pub trait DeliveryRoute {}
}

/// A statically selected transferable destination capability.
///
/// The associated protocol prevents actor templates from repeating a separate
/// protocol parameter beside the route that already determines it. Logical
/// and established routes select different concrete send products without
/// weakening either capability.
///
/// The owner's address namespace is stated through the associated protocol:
/// `R: DeliveryRoute<Protocol: Protocol<Addr = BehaviorAddr<Owner>>>`.
/// A route for another protocol cannot be substituted merely because its
/// payload has the same Rust type:
///
/// ```compile_fail,E0271
/// type Expected = behavior::MessageProtocol<behavior::MailAddr, u8>;
/// struct Other;
/// impl behavior::Protocol for Other {
///     type Addr = behavior::MailAddr;
///     type Msg = u8;
/// }
/// fn require_expected<R: behavior_actors::DeliveryRoute<Protocol = Expected>>(_: R) {}
/// require_expected(behavior::Recipient::<Other>::global(behavior::MailAddr(1)));
/// ```
/// Creator-local child routes require the owning actor's birth algebra and
/// cannot be substituted for a transferable acquaintance. There is one route
/// interface for these transferable capabilities:
///
/// ```compile_fail,E0405
/// fn require<Owner, Route>()
/// where
///     Owner: behavior::Behavior,
///     Route: behavior_actors::DeliveryRouteFor<Owner>,
/// {}
/// ```
pub trait DeliveryRoute: sealed::DeliveryRoute + Sized {
    /// Protocol selected by this capability.
    type Protocol: Protocol;
    /// The concrete sends product produced by this route.
    type Sends: SendEffects;

    /// Consume the capability into one explicit ordered delivery product.
    fn deliver(self, message: <Self::Protocol as Protocol>::Msg) -> Self::Sends;
}

impl<P: Protocol> sealed::DeliveryRoute for Recipient<P> {}
impl<P: Protocol> DeliveryRoute for Recipient<P> {
    type Protocol = P;
    type Sends = Vec<Delivery<P>>;

    fn deliver(self, message: P::Msg) -> Self::Sends {
        vec![Delivery::new(self, message)]
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
    type SourceCustody = Self::Settlements;
    type InterpretationCustody = Vec<
        ReplyDelivery<
            (
                Option<Delivery<P>>,
                Option<
                    ItemSettlement<
                        Delivery<P>,
                        <Delivery<P> as ActionItem>::Accepted,
                        <Delivery<P> as ActionItem>::Rejection,
                        <Delivery<P> as ActionItem>::Prerequisite,
                    >,
                >,
            ),
            (
                Option<EstablishedDelivery<P>>,
                Option<
                    ItemSettlement<
                        EstablishedDelivery<P>,
                        <EstablishedDelivery<P> as ActionItem>::Accepted,
                        <EstablishedDelivery<P> as ActionItem>::Rejection,
                        <EstablishedDelivery<P> as ActionItem>::Prerequisite,
                    >,
                >,
            ),
        >,
    >;

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        *progress = match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                Some(InterpretationProgress::Interpreting(
                    original
                        .deliveries
                        .into_iter()
                        .map(|delivery| match delivery {
                            ReplyDelivery::Logical(input) => {
                                ReplyDelivery::Logical((Some(input), None))
                            }
                            ReplyDelivery::Established(input) => {
                                ReplyDelivery::Established((Some(input), None))
                            }
                        })
                        .collect(),
                ))
            }
            retained => retained,
        };
    }

    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        let Some(InterpretationProgress::Interpreting(rows)) = progress.as_ref() else {
            return;
        };
        let corrupt = rows.iter().position(|row| match row {
            ReplyDelivery::Logical((_, received)) => {
                matches!(received, Some(ItemSettlement::Corrupt { .. }))
            }
            ReplyDelivery::Established((_, received)) => {
                matches!(received, Some(ItemSettlement::Corrupt { .. }))
            }
        });
        let complete = rows.iter().enumerate().all(|(index, row)| {
            let after_corrupt = corrupt.is_some_and(|corrupt| index > corrupt);
            match row {
                ReplyDelivery::Logical((input, received)) => {
                    if after_corrupt {
                        input.is_some() && received.is_none()
                    } else {
                        input.is_none() && received.is_some()
                    }
                }
                ReplyDelivery::Established((input, received)) => {
                    if after_corrupt {
                        input.is_some() && received.is_none()
                    } else {
                        input.is_none() && received.is_some()
                    }
                }
            }
        });
        if !complete {
            return;
        }
        let Some(InterpretationProgress::Interpreting(rows)) = progress.take() else {
            return;
        };
        let mut remaining = rows.into_iter();
        let mut settled = Vec::with_capacity(remaining.len());
        while let Some(row) = remaining.next() {
            match row {
                ReplyDelivery::Logical((None, Some(received))) => {
                    settled.push(ReplyDelivery::Logical(SettledItem::Attempted(received)))
                }
                ReplyDelivery::Logical((Some(input), None)) => {
                    settled.push(ReplyDelivery::Logical(SettledItem::Unattempted(input)))
                }
                ReplyDelivery::Established((None, Some(received))) => {
                    settled.push(ReplyDelivery::Established(SettledItem::Attempted(received)))
                }
                ReplyDelivery::Established((Some(input), None)) => {
                    settled.push(ReplyDelivery::Established(SettledItem::Unattempted(input)))
                }
                row => {
                    let restored = settled
                        .into_iter()
                        .map(|settled| match settled {
                            ReplyDelivery::Logical(SettledItem::Attempted(received)) => {
                                ReplyDelivery::Logical((None, Some(received)))
                            }
                            ReplyDelivery::Logical(SettledItem::Unattempted(input)) => {
                                ReplyDelivery::Logical((Some(input), None))
                            }
                            ReplyDelivery::Established(SettledItem::Attempted(received)) => {
                                ReplyDelivery::Established((None, Some(received)))
                            }
                            ReplyDelivery::Established(SettledItem::Unattempted(input)) => {
                                ReplyDelivery::Established((Some(input), None))
                            }
                        })
                        .chain(core::iter::once(row))
                        .chain(remaining)
                        .collect();
                    *progress = Some(InterpretationProgress::Interpreting(restored));
                    return;
                }
            }
        }
        let settled = ReplyDeliveries::new(settled);
        *progress = Some(InterpretationProgress::Completed(match corrupt {
            Some(_) => Interpretation::Corrupt(settled),
            None => Interpretation::Complete(settled),
        }));
    }

    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        *progress = match progress.take() {
            Some(InterpretationProgress::Original(original)) => Some(
                InterpretationProgress::Completed(Interpretation::Complete(ReplyDeliveries::new(
                    original
                        .deliveries
                        .into_iter()
                        .map(|delivery| match delivery {
                            ReplyDelivery::Logical(input) => {
                                ReplyDelivery::Logical(SettledItem::Unattempted(input))
                            }
                            ReplyDelivery::Established(input) => {
                                ReplyDelivery::Established(SettledItem::Unattempted(input))
                            }
                        })
                        .collect(),
                ))),
            ),
            retained => retained,
        };
    }
}

impl<Host, RootEvent, P> SourceSettlementCustody<Host, RootEvent>
    for ReplyDeliveries<ActionItemResult<Delivery<P>>, ActionItemResult<EstablishedDelivery<P>>>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    Delivery<P>: ActionItem,
    EstablishedDelivery<P>: ActionItem,
{
    type Custody = Self;

    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Completed(
                SourceCustody::Exhausted(original),
            )),
            retained => retained,
        };
    }

    fn offer_next_to_source(
        _: &mut Self::Custody,
        _: &mut Host,
    ) -> impl core::future::Future<Output = ()> + Send {
        core::future::ready(())
    }

    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original) | SourceProgress::Offering(original)) => Some(
                SourceProgress::Completed(SourceCustody::Exhausted(original)),
            ),
            retained => retained,
        };
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
    P::Addr: Send,
    P::Msg: Send,
    <P::Addr as RecipientAddress>::Established<P>: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            Self::prepare_interpretation(progress);
            let Some(InterpretationProgress::Interpreting(rows)) = progress else {
                return;
            };
            for row in rows {
                match row {
                    ReplyDelivery::Logical((input, received)) => {
                        match (input.as_ref(), received.as_ref()) {
                            (None, Some(ItemSettlement::Corrupt { .. })) => break,
                            (None, Some(_)) => continue,
                            (Some(_), None) => {}
                            _ => return,
                        }
                        <Interpreter as InterpretItem<Delivery<P>, RootEvent, Path>>::interpret_item(interpreter,input,received).await;
                        match (input.as_ref(), received.as_ref()) {
                            (None, Some(ItemSettlement::Corrupt { .. })) => break,
                            (None, Some(_)) => {}
                            _ => return,
                        }
                    }
                    ReplyDelivery::Established((input, received)) => {
                        match (input.as_ref(), received.as_ref()) {
                            (None, Some(ItemSettlement::Corrupt { .. })) => break,
                            (None, Some(_)) => continue,
                            (Some(_), None) => {}
                            _ => return,
                        }
                        <Interpreter as InterpretItem<EstablishedDelivery<P>, RootEvent, Path>>::interpret_item(interpreter,input,received).await;
                        match (input.as_ref(), received.as_ref()) {
                            (None, Some(ItemSettlement::Corrupt { .. })) => break,
                            (None, Some(_)) => {}
                            _ => return,
                        }
                    }
                }
            }
            Self::finish_interpretation(progress);
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
