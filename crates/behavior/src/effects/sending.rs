//! Typed send effects, their composition contract, and event ownership.

use crate::{
    Behavior, BirthProtocol, BirthProtocolProduct, ChildDelivery, ChildInput, ComposedEvent,
    Delivery, EndpointAddress, EstablishedDelivery, InjectEvent, Inside, NoBirthProtocols,
    Protocol,
};
use core::future::Future;

/// Total settlement of one statically selected action item.
///
/// Rejection and dependency blocking retain the complete original item.
/// Interpreter corruption does the same. Accepted items are consumed and leave
/// only the capability's promised receipt. Whether an item was attempted at all
/// belongs to [`SettledItem`], so a concrete interpreter cannot fabricate that
/// product-level fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemSettlement<Item, Accepted, Rejection, Prerequisite> {
    /// The capability consumed the item and returned its promised receipt.
    Accepted(Accepted),
    /// The capability lawfully rejected the complete item.
    Rejected { item: Item, reason: Rejection },
    /// A declared prerequisite did not commit, so the item remains untouched.
    Blocked {
        item: Item,
        prerequisite: Prerequisite,
    },
    /// The interpreter violated its contract while the item remained owned.
    Corrupt { item: Item, fault: InterpreterFault },
}

/// Product-owned evidence that an action item was attempted or left untouched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettledItem<Item, Settlement> {
    /// The concrete interpreter returned the item's complete settlement.
    Attempted(Settlement),
    /// Product traversal stopped at an earlier corrupt item.
    Unattempted(Item),
}

/// Completion state of one total static product traversal.
///
/// Both variants own the complete settlement shape. `Corrupt` contains the
/// factual committed prefix, the exact corrupt item, and every later item as
/// [`SettledItem::Unattempted`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interpretation<Settlement> {
    Complete(Settlement),
    Corrupt(Settlement),
}

impl<Settlement> Interpretation<Settlement> {
    /// Transform the complete settlement shape without changing whether the
    /// interpreter completed or corrupted.
    #[must_use]
    pub fn map<Mapped>(self, map: impl FnOnce(Settlement) -> Mapped) -> Interpretation<Mapped> {
        match self {
            Self::Complete(settlement) => Interpretation::Complete(map(settlement)),
            Self::Corrupt(settlement) => Interpretation::Corrupt(map(settlement)),
        }
    }

    /// Recover the complete owned settlement shape.
    #[must_use]
    pub fn into_settlement(self) -> Settlement {
        match self {
            Self::Complete(settlement) | Self::Corrupt(settlement) => settlement,
        }
    }
}

/// Interpret two statically named send products in declared order.
///
/// A corrupt earlier product retains the complete later product as
/// unattempted. Expected rejection inside the earlier product does not stop
/// the later product because it remains a complete interpretation.
#[doc(hidden)]
pub async fn settle_in_order<Interpreter, RootEvent, Path, Earlier, Later>(
    earlier: Earlier,
    later: Later,
    interpreter: &mut Interpreter,
) -> Interpretation<(Earlier::Settlements, Later::Settlements)>
where
    Interpreter: Send,
    Earlier: InterpretSends<Interpreter, RootEvent, Path>,
    Later: InterpretSends<Interpreter, RootEvent, Path>,
{
    let earlier = match earlier.interpret(interpreter).await {
        Interpretation::Complete(earlier) => earlier,
        Interpretation::Corrupt(earlier) => {
            return Interpretation::Corrupt((earlier, later.unattempted()));
        }
    };
    later
        .interpret(interpreter)
        .await
        .map(|later| (earlier, later))
}

/// Read-only control-flow status of one complete retained settlement product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SettlementStatus {
    /// Every item was attempted and accepted.
    Accepted,
    /// At least one item was lawfully rejected or blocked and none corrupted.
    Rejected,
    /// Interpretation corrupted or left an unattempted suffix.
    Corrupt,
}

impl SettlementStatus {
    #[must_use]
    pub const fn combine(self, later: Self) -> Self {
        match (self, later) {
            (Self::Corrupt, _) | (_, Self::Corrupt) => Self::Corrupt,
            (Self::Rejected, _) | (_, Self::Rejected) => Self::Rejected,
            (Self::Accepted, Self::Accepted) => Self::Accepted,
        }
    }
}

/// Lossless status projection over an exact retained settlement value.
pub trait ClassifySettlement {
    /// Inspect every member without consuming or rewriting the settlement.
    fn settlement_status(&self) -> SettlementStatus;
}

impl<Item, Accepted, Rejection, Prerequisite> ClassifySettlement
    for ItemSettlement<Item, Accepted, Rejection, Prerequisite>
{
    fn settlement_status(&self) -> SettlementStatus {
        match self {
            Self::Accepted(_) => SettlementStatus::Accepted,
            Self::Rejected { .. } | Self::Blocked { .. } => SettlementStatus::Rejected,
            Self::Corrupt { .. } => SettlementStatus::Corrupt,
        }
    }
}

impl<Item, Settlement> ClassifySettlement for SettledItem<Item, Settlement>
where
    Settlement: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        match self {
            Self::Attempted(settlement) => settlement.settlement_status(),
            Self::Unattempted(_) => SettlementStatus::Corrupt,
        }
    }
}

impl<Settlement> ClassifySettlement for Interpretation<Settlement>
where
    Settlement: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        match self {
            Self::Complete(settlement) => settlement.settlement_status(),
            Self::Corrupt(_) => SettlementStatus::Corrupt,
        }
    }
}

impl<Settlement> ClassifySettlement for Vec<Settlement>
where
    Settlement: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        let mut status = SettlementStatus::Accepted;
        for settlement in self {
            status = status.combine(settlement.settlement_status());
        }
        status
    }
}

impl ClassifySettlement for crate::Never {
    fn settlement_status(&self) -> SettlementStatus {
        match *self {}
    }
}

/// One concrete value that can occur in [`crate::Actions`].
///
/// The capability item owns its settlement vocabulary. Every conforming
/// interpreter therefore agrees on the exact accepted receipt, rejection,
/// and prerequisite types for that item. Interpreter corruption is the one
/// shared [`InterpreterFault`] sum.
///
/// A runtime cannot substitute a different rejection type for the same item:
///
/// ```compile_fail,E0053
/// struct Request;
/// struct RequiredRejection;
/// struct RuntimeRejection;
///
/// impl behavior::ActionItem for Request {
///     type Accepted = ();
///     type Rejection = RequiredRejection;
///     type Prerequisite = behavior::Never;
/// }
///
/// struct Runtime;
/// impl behavior::InterpretItem<Request, (), behavior::Here> for Runtime {
///     fn interpret_item(
///         &mut self,
///         item: Request,
///     ) -> impl core::future::Future<
///         Output = behavior::ItemSettlement<
///             Request,
///             (),
///             RuntimeRejection,
///             behavior::Never,
///         >,
///     > + Send {
///         async move {
///             behavior::ItemSettlement::Rejected { item, reason: RuntimeRejection }
///         }
///     }
/// }
/// ```
pub trait ActionItem: Sized + Send {
    type Accepted: Send;
    type Rejection: Send;
    type Prerequisite: Send;
}

/// Exact result of one action item within an interpreted action product.
///
/// The inner sum records the capability attempt. The outer sum preserves an
/// untouched item when an earlier corrupt item stopped ordered traversal.
pub type ActionItemResult<
    Item,
    Accepted = <Item as ActionItem>::Accepted,
    Rejection = <Item as ActionItem>::Rejection,
    Prerequisite = <Item as ActionItem>::Prerequisite,
> = SettledItem<Item, ItemSettlement<Item, Accepted, Rejection, Prerequisite>>;

/// A broken interpreter invariant, distinct from expected capability rejection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterpreterFault {
    MissingCapability,
    CorruptTraversal,
}

/// Exact reason one structural parent report was not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParentReportReason {
    ClosedParentControlLane,
}

/// Concrete interpreter ownership port for one exact typed action item.
///
/// `Item`, `RootEvent`, and `Path` select the capability statically. Distinct
/// delivery and runtime-request values remain distinct implementations without
/// an erased envelope, registry, or runtime lookup.
pub trait InterpretItem<Item, RootEvent, Path>: Send
where
    Item: ActionItem,
{
    /// Attempt exactly one item and return its complete settlement.
    fn interpret_item(
        &mut self,
        item: Item,
    ) -> impl Future<
        Output = ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
    > + Send;
}

/// Static settlement product determined solely by one concrete sends value.
///
/// Every action item declares its own accepted, rejection, and prerequisite
/// types. A containing product therefore has one settlement type under every
/// conforming interpreter. `unattempted` preserves every item untouched after
/// an earlier corrupt item stops traversal.
///
/// A runtime cannot select another settlement product for the same sends value:
///
/// ```compile_fail,E0271
/// struct RuntimeSettlement;
///
/// fn runtime_selected<Runtime>(
///     requests: behavior::InterpreterRequests<behavior::ReportToParent<u8>>,
///     runtime: &mut Runtime,
/// ) -> impl core::future::Future<
///     Output = behavior::Interpretation<RuntimeSettlement>,
/// > + Send
/// where
///     Runtime: behavior::InterpretItem<
///         behavior::ReportToParent<u8>,
///         (),
///         behavior::Here,
///     >,
/// {
///     <behavior::InterpreterRequests<behavior::ReportToParent<u8>> as
///         behavior::InterpretSends<Runtime, (), behavior::Here>>::interpret(
///             requests,
///             runtime,
///         )
/// }
/// ```
pub trait SendSettlements: Sized {
    /// Complete settlement product corresponding to `Self`.
    type Settlements: Send + ClassifySettlement;

    /// Preserve every item without granting interpretation authority.
    fn unattempted(self) -> Self::Settlements;
}

/// Static interpretation of one complete sends value at an absolute event path.
///
/// Implementations are monomorphized over `Interpreter`; there is no erased
/// envelope, runtime lane lookup, or downcast. `RootEvent` is the event type
/// ultimately enqueued for the actor and `Path` is the current send owner's
/// absolute position in it. Lawful rejection and blocking continue to later
/// independent items. Corruption stops interpretation but retains the exact
/// fault item and every untouched suffix value in the settlement shape.
pub trait InterpretSends<Interpreter, RootEvent, Path>: SendSettlements + Send {
    /// Interpret every value in this product in its declared stable order.
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send;
}

/// The lane owned by the current named send product.
pub enum Own {}

/// Static evidence that a sends type contains one request lane.
///
/// Implementations append the input exactly once to that lane and leave every
/// other lane unchanged. `Path` distinguishes repeated request types without
/// erasing their position or choosing a lane at runtime.
///
/// [`Own`] selects a named product's own semantic lane. [`SendLayer`] carries
/// wrapper-owned and inner effects as explicit named fields; request routing
/// remains a compile-time proof rather than a runtime lane lookup.
pub trait SendInput<Input, Path> {
    fn emit(&mut self, input: Input);
}

/// Send effects emitted by a pure actor transition.
///
/// Values compose without interpreting them. This keeps communications and
/// interpreter requests inside the explicit [`crate::Actions`] boundary.
pub trait SendEffects: Sized {
    fn empty() -> Self;
    fn append(&mut self, other: Self);

    #[must_use]
    fn combine(mut self, other: Self) -> Self {
        self.append(other);
        self
    }

    /// Append one request to its statically selected semantic lane.
    fn send<Input, Path>(&mut self, input: Input)
    where
        Self: SendInput<Input, Path>,
    {
        <Self as SendInput<Input, Path>>::emit(self, input);
    }

    /// Build a send product containing one request in its selected lane.
    #[must_use]
    fn sending<Input, Path>(input: Input) -> Self
    where
        Self: SendInput<Input, Path>,
    {
        let mut sends = Self::empty();
        sends.send(input);
        sends
    }
}

/// Static projection of intentional logical destinations from one concrete
/// sends product.
///
/// Implementations mirror the product's [`InterpretSends`] traversal without
/// inspecting values. Only [`Delivery<P>`] contributes `P`; exact established
/// deliveries, creator-local child deliveries and inputs, and interpreter
/// requests contribute nothing. Named products append their field projections
/// in interpretation order, preserving repeated protocol occurrences.
///
/// Custom named sends products must implement this trait explicitly. There is
/// deliberately no blanket `Vec<T>` implementation that could silently treat
/// an unknown delivery representation as having no logical destination.
///
/// ```compile_fail,E0277
/// use behavior::{
///     Actions, Behavior, BehaviorActed, LogicalHostRequirements, MailAddr,
///     Never, NoBirths, Protocol, SendEffects, User,
/// };
/// struct OpaqueSends;
/// impl SendEffects for OpaqueSends {
///     fn empty() -> Self { Self }
///     fn append(&mut self, _: Self) {}
/// }
/// impl<E> behavior::SendsFor<E> for OpaqueSends {}
/// struct Actor;
/// impl Protocol for Actor { type Addr = MailAddr; type Msg = (); }
/// impl Behavior for Actor {
///     type Protocol = Self;
///     type Event = User<MailAddr, ()>;
///     type Sends = OpaqueSends;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event)
///         -> BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// fn require_complete<B: LogicalHostRequirements>() {}
/// require_complete::<Actor>();
/// ```
pub trait LogicalDeliveryProtocols: SendEffects {
    /// Ordered, duplicate-preserving logical protocol occurrences.
    type Protocols: BirthProtocolProduct;
}

/// Proof that send effects are lawful for one complete event type.
///
/// Ordinary communications are independent of `Event`. Interpreter requests
/// that return a local fact are not: their continuation must select an exact
/// member of `Event`. Composite products implement this trait structurally,
/// reindexing only their wrapped behavior effects through an outer event
/// injection.
///
/// An un-reindexed return to the emitter cannot be paired with an added outer event
/// layer:
///
/// ```compile_fail
/// use behavior::{SendsFor, EventLayer, Here, MailAddr, ReturnsToEmitter,
///     InterpreterRequest, InterpreterRequests, User};
/// struct Request;
/// impl InterpreterRequest for Request {
///     type ReturnToEmitter = ReturnsToEmitter<u8, Here>;
/// }
/// fn lawful<E, F: SendsFor<E>>() {}
/// type Inner = EventLayer<u8, User<MailAddr, ()>>;
/// type Outer = EventLayer<(), Inner>;
/// lawful::<Outer, InterpreterRequests<Request>>();
/// ```
pub trait SendsFor<Event>: SendEffects {}

/// One action whose exact interpretation result returns to its live source.
///
/// [`ActionItem`] determines the complete result. An implementation selects
/// only the actor receiving it, so an action cannot discard a rejected or
/// unattempted value or reinterpret its status. Bombay admits the exact input
/// through [`SourceAdmission`] or keeps it in lifecycle custody.
pub trait SourceAction: ActionItem {
    /// Actor-owned ingress selection for the result.
    type Source;
}

/// Ordered actions whose normalized results return to their emitting actor.
///
/// This product is interpreter-facing machinery. Aggregate builders expose
/// domain operations, not this structural lane.
pub struct SourceActions<Item> {
    items: Vec<Item>,
}

impl<Item> SourceActions<Item> {
    /// Number of requests retained in authored order.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Report whether this ordered request lane contains no items.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Consume the lane into its requests in authored order.
    #[must_use]
    pub fn into_items(self) -> Vec<Item> {
        self.items
    }
}

impl<Item> SendEffects for SourceActions<Item> {
    fn empty() -> Self {
        Self { items: Vec::new() }
    }

    fn append(&mut self, mut other: Self) {
        self.items.append(&mut other.items);
    }
}

impl<Item> SendInput<Item, Own> for SourceActions<Item> {
    fn emit(&mut self, input: Item) {
        self.items.push(input);
    }
}

impl<Item> LogicalDeliveryProtocols for SourceActions<Item> {
    type Protocols = crate::NoBirthProtocols;
}

impl<Event, Item> SendsFor<Event> for SourceActions<Item> where Item: SourceAction {}

/// Exact results for one [`SourceActions`] lane.
///
/// Closed admission retains values in this same product. There is no second
/// residual representation and no inverse conversion.
pub struct SourceSettlements<Item>
where
    Item: SourceAction,
{
    inputs: Vec<ActionItemResult<Item>>,
}

impl<Item> SourceSettlements<Item>
where
    Item: SourceAction,
{
    fn new(inputs: Vec<ActionItemResult<Item>>) -> Self {
        Self { inputs }
    }

    /// Transfer every retained input in authored order.
    #[must_use]
    pub fn into_inputs(self) -> Vec<ActionItemResult<Item>> {
        self.inputs
    }
}

impl<Item> ClassifySettlement for SourceSettlements<Item>
where
    Item: SourceAction,
{
    fn settlement_status(&self) -> SettlementStatus {
        self.inputs.settlement_status()
    }
}

impl<Item> SendSettlements for SourceActions<Item>
where
    Item: SourceAction,
{
    type Settlements = SourceSettlements<Item>;

    fn unattempted(self) -> Self::Settlements {
        SourceSettlements::new(
            self.items
                .into_iter()
                .map(SettledItem::Unattempted)
                .collect(),
        )
    }
}

impl<Interpreter, RootEvent, Path, Item> InterpretSends<Interpreter, RootEvent, Path>
    for SourceActions<Item>
where
    Interpreter: InterpretItem<Item, RootEvent, Path>,
    Item: SourceAction,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        async move {
            let mut items = self.items.into_iter();
            let mut inputs = Vec::with_capacity(items.len());
            while let Some(item) = items.next() {
                match settle_item::<Item, Interpreter, RootEvent, Path>(item, interpreter).await {
                    Interpretation::Complete(settlement) => {
                        inputs.push(settlement);
                    }
                    Interpretation::Corrupt(settlement) => {
                        inputs.push(settlement);
                        inputs.extend(items.map(SettledItem::Unattempted));
                        return Interpretation::Corrupt(SourceSettlements::new(inputs));
                    }
                }
            }
            Interpretation::Complete(SourceSettlements::new(inputs))
        }
    }
}

/// Bombay ownership port for one exact current-actor system input.
///
/// Success transfers the input to the existing actor loop. Closed admission
/// returns the unchanged input to the current lifecycle host.
///
/// An event without the exact source ingress cannot implement this port:
///
/// ```compile_fail,E0277
/// struct EventWithoutInput;
/// struct Owner;
/// struct Input;
/// struct Host;
/// impl behavior::SourceAdmission<EventWithoutInput, Owner, Input> for Host {
///     async fn admit_source(&mut self, _: Input) -> Result<(), Input> { Ok(()) }
/// }
/// ```
pub trait SourceAdmission<RootEvent, Source, Input>: Send
where
    RootEvent: crate::EventIngress<Source, Input>,
{
    /// Attempt one source input without discarding closed-control ownership.
    fn admit_source(&mut self, input: Input) -> impl Future<Output = Result<(), Input>> + Send;
}

/// Progress of one ordered source-settlement custody pass.
pub enum SourceCustody<Residual> {
    /// Source admission remains available after processing this residual.
    Open(Residual),
    /// Admission closed; the residual contains the current and untouched suffix.
    Closed(Residual),
}

/// Static ordered source admission for one complete settlement product.
pub trait SourceSettlementCustody<Host, RootEvent>: Sized {
    /// Offer source inputs in their declared order.
    fn offer_to_source(self, host: &mut Host) -> impl Future<Output = SourceCustody<Self>> + Send;
}

impl<Host, RootEvent, Item> SourceSettlementCustody<Host, RootEvent> for SourceSettlements<Item>
where
    Host: SourceAdmission<RootEvent, Item::Source, ActionItemResult<Item>>,
    RootEvent: crate::EventIngress<Item::Source, ActionItemResult<Item>>,
    Item: SourceAction,
{
    fn offer_to_source(self, host: &mut Host) -> impl Future<Output = SourceCustody<Self>> + Send {
        async move {
            let mut source_inputs = self.inputs.into_iter();
            let mut residual = Vec::new();
            while let Some(input) = source_inputs.next() {
                match host.admit_source(input).await {
                    Ok(()) => {}
                    Err(input) => {
                        residual.push(input);
                        residual.extend(source_inputs);
                        return SourceCustody::Closed(Self::new(residual));
                    }
                }
            }
            SourceCustody::Open(Self::new(residual))
        }
    }
}

impl<Host, RootEvent> SourceSettlementCustody<Host, RootEvent> for NoSends {
    fn offer_to_source(self, _: &mut Host) -> impl Future<Output = SourceCustody<Self>> + Send {
        async move { SourceCustody::Open(self) }
    }
}

impl<Host, RootEvent, Item> SourceSettlementCustody<Host, RootEvent> for Vec<ActionItemResult<Item>>
where
    Item: ActionItem,
{
    fn offer_to_source(self, _: &mut Host) -> impl Future<Output = SourceCustody<Self>> + Send {
        async move { SourceCustody::Open(self) }
    }
}

impl<Host, RootEvent> SourceSettlementCustody<Host, RootEvent> for Vec<crate::Never> {
    fn offer_to_source(self, _: &mut Host) -> impl Future<Output = SourceCustody<Self>> + Send {
        async move { SourceCustody::Open(self) }
    }
}

impl<Host, RootEvent, Owned, Inner> SourceSettlementCustody<Host, RootEvent>
    for SendLayer<Owned, Inner>
where
    Host: Send,
    Owned: SourceSettlementCustody<Host, RootEvent> + Send,
    Inner: SourceSettlementCustody<Host, RootEvent> + Send,
{
    fn offer_to_source(self, host: &mut Host) -> impl Future<Output = SourceCustody<Self>> + Send {
        async move {
            match self.inner.offer_to_source(host).await {
                SourceCustody::Open(inner) => match self.owned.offer_to_source(host).await {
                    SourceCustody::Open(owned) => SourceCustody::Open(SendLayer::new(owned, inner)),
                    SourceCustody::Closed(owned) => {
                        SourceCustody::Closed(SendLayer::new(owned, inner))
                    }
                },
                SourceCustody::Closed(inner) => {
                    SourceCustody::Closed(SendLayer::new(self.owned, inner))
                }
            }
        }
    }
}

/// An interpreter request that produces no later fact for the emitting actor.
pub enum NoReturnToEmitter {}

/// An interpreter request whose later `Input` returns to the emitting actor at
/// `Path`, relative to the effect lane that owns the request.
pub struct ReturnsToEmitter<Input, Path>(core::marker::PhantomData<fn(Input, Path)>);

/// Local-return contract declared by one interpreter-facing request.
pub trait ReturnToEmitterFor<Event> {}

impl<Event> ReturnToEmitterFor<Event> for NoReturnToEmitter {}

impl<Event, Input, Path> ReturnToEmitterFor<Event> for ReturnsToEmitter<Input, Path> where
    Event: InjectEvent<Input, Path>
{
}

/// Declares only the continuation returning to the actor that emitted this
/// interpreter request. Destinations owned by a child, parent, ancestor, or
/// established actor are separate capabilities and are not reindexed when the
/// emitter is wrapped.
pub trait InterpreterRequest {
    type ReturnToEmitter;
}

/// Transfer one owned report to the emitter's established parent.
///
/// The request is an ordinary typed send effect. Its interpreter uses the
/// already-established creator/child relationship, attaches the emitter's
/// exact creator-local nonce, and injects a [`crate::ChildReport`] into the
/// parent's closed event algebra. It performs no address or protocol lookup.
/// A root behavior has no parent capability and therefore cannot interpret
/// this request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportToParent<R> {
    /// Complete report value transferred to the parent.
    pub report: R,
}

impl<R> ReportToParent<R> {
    /// Construct one structural parent report.
    #[must_use]
    pub const fn new(report: R) -> Self {
        Self { report }
    }

    /// Recover ownership of the complete report value.
    #[must_use]
    pub fn into_inner(self) -> R {
        self.report
    }
}

impl<R> InterpreterRequest for ReportToParent<R> {
    type ReturnToEmitter = NoReturnToEmitter;
}

impl<R> ActionItem for ReportToParent<R>
where
    R: Send,
{
    type Accepted = ();
    type Rejection = ParentReportReason;
    type Prerequisite = crate::Never;
}

/// Send effects containing no communications or interpreter requests.
///
/// A behavior layer that adds an event lane but emits nothing of its own uses
/// this named value rather than an ambiguous `()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSends;

impl SendEffects for NoSends {
    fn empty() -> Self {
        Self
    }

    fn append(&mut self, _: Self) {}
}

impl LogicalDeliveryProtocols for NoSends {
    type Protocols = NoBirthProtocols;
}

impl<Event> SendsFor<Event> for NoSends {}

impl SendSettlements for NoSends {
    type Settlements = Self;

    fn unattempted(self) -> Self::Settlements {
        self
    }
}

impl ClassifySettlement for NoSends {
    fn settlement_status(&self) -> SettlementStatus {
        SettlementStatus::Accepted
    }
}

impl<Interpreter, RootEvent, Path> InterpretSends<Interpreter, RootEvent, Path> for NoSends {
    fn interpret(
        self,
        _: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        async move { Interpretation::Complete(self) }
    }
}

/// Send effects introduced by one wrapper around inner send effects.
///
/// `owned` contains effects introduced by the current behavior layer and
/// `inner` contains effects of the wrapped interaction. The structural
/// [`SendsFor`] implementation is the composition law:
///
/// ```text
/// Event'   = OwnedEvent + InnerEvent
/// Effects' = OwnedEffects × InnerEffects
/// ```
///
/// Owned return to the emitters target `Event'`; inner return to the emitters target
/// `InnerEvent` and are therefore lifted through the same `Inner` injection.
/// Established actor, child, and ancestor destinations are unaffected because
/// they are not return to the emitters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendLayer<Owned, Inner> {
    pub owned: Owned,
    pub inner: Inner,
}

impl<Owned, Inner> SendLayer<Owned, Inner> {
    #[must_use]
    pub const fn new(owned: Owned, inner: Inner) -> Self {
        Self { owned, inner }
    }
}

impl<Owned: SendEffects, Inner: SendEffects> SendEffects for SendLayer<Owned, Inner> {
    fn empty() -> Self {
        Self::new(Owned::empty(), Inner::empty())
    }

    fn append(&mut self, other: Self) {
        self.owned.append(other.owned);
        self.inner.append(other.inner);
    }
}

impl<Owned, Inner> LogicalDeliveryProtocols for SendLayer<Owned, Inner>
where
    Owned: LogicalDeliveryProtocols,
    Inner: LogicalDeliveryProtocols,
{
    // Interpretation preserves authored inner-to-outer wrapper order.
    type Protocols = <Inner::Protocols as BirthProtocolProduct>::Append<Owned::Protocols>;
}

impl<Event, OwnedEffects, InnerEffects> SendsFor<Event> for SendLayer<OwnedEffects, InnerEffects>
where
    Event: ComposedEvent,
    OwnedEffects: SendsFor<Event>,
    InnerEffects: SendsFor<Event::Inner>,
{
}

impl<Owned, Inner> SendSettlements for SendLayer<Owned, Inner>
where
    Owned: SendSettlements,
    Inner: SendSettlements,
{
    type Settlements = SendLayer<Owned::Settlements, Inner::Settlements>;

    fn unattempted(self) -> Self::Settlements {
        SendLayer::new(self.owned.unattempted(), self.inner.unattempted())
    }
}

impl<Owned, Inner> ClassifySettlement for SendLayer<Owned, Inner>
where
    Owned: ClassifySettlement,
    Inner: ClassifySettlement,
{
    fn settlement_status(&self) -> SettlementStatus {
        self.inner
            .settlement_status()
            .combine(self.owned.settlement_status())
    }
}

impl<Input, Path, Owned, Inner> SendInput<Input, Path> for SendLayer<Owned, Inner>
where
    Owned: SendInput<Input, Path>,
{
    fn emit(&mut self, input: Input) {
        self.owned.emit(input);
    }
}

impl<Interpreter, RootEvent, Path, Owned, Inner> InterpretSends<Interpreter, RootEvent, Path>
    for SendLayer<Owned, Inner>
where
    Interpreter: Send,
    Owned: InterpretSends<Interpreter, RootEvent, Path> + Send,
    Inner: InterpretSends<Interpreter, RootEvent, Inside<Path>> + Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        async move {
            let inner = <Inner as InterpretSends<Interpreter, RootEvent, Inside<Path>>>::interpret(
                self.inner,
                interpreter,
            )
            .await;
            let inner = match inner {
                Interpretation::Complete(inner) => inner,
                Interpretation::Corrupt(inner) => {
                    return Interpretation::Corrupt(SendLayer::new(
                        self.owned.unattempted(),
                        inner,
                    ));
                }
            };
            match <Owned as InterpretSends<Interpreter, RootEvent, Path>>::interpret(
                self.owned,
                interpreter,
            )
            .await
            {
                Interpretation::Complete(owned) => {
                    Interpretation::Complete(SendLayer::new(owned, inner))
                }
                Interpretation::Corrupt(owned) => {
                    Interpretation::Corrupt(SendLayer::new(owned, inner))
                }
            }
        }
    }
}

impl<T> SendEffects for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }

    fn append(&mut self, mut other: Self) {
        Vec::append(self, &mut other);
    }
}

impl<P: Protocol> LogicalDeliveryProtocols for Vec<Delivery<P>> {
    type Protocols = BirthProtocol<P, NoBirthProtocols>;
}

impl<P: Protocol, Occurrence> LogicalDeliveryProtocols for Vec<ChildDelivery<P, Occurrence>> {
    type Protocols = NoBirthProtocols;
}

impl<Child, Source, Input, Occurrence> LogicalDeliveryProtocols
    for Vec<ChildInput<Child, Source, Input, Occurrence>>
where
    Child: Behavior,
{
    type Protocols = NoBirthProtocols;
}

impl<P> LogicalDeliveryProtocols for Vec<EstablishedDelivery<P>>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Protocols = NoBirthProtocols;
}

impl LogicalDeliveryProtocols for Vec<crate::Never> {
    type Protocols = NoBirthProtocols;
}

impl<Event, T> SendsFor<Event> for Vec<T> {}

impl<Item> SendSettlements for Vec<Item>
where
    Item: ActionItem,
{
    type Settlements = Vec<ActionItemResult<Item>>;

    fn unattempted(self) -> Self::Settlements {
        unattempted_items(self)
    }
}

/// Interpret exactly one item and attach its product-owned attempt evidence.
///
/// This is the indivisible form of [`InterpretSends`]. It marks the enclosing
/// interpretation corrupt exactly when the concrete item result is corrupt;
/// lawful rejection and blocking remain complete outcomes.
pub async fn settle_item<Item, Interpreter, RootEvent, Path>(
    item: Item,
    interpreter: &mut Interpreter,
) -> Interpretation<ActionItemResult<Item>>
where
    Item: ActionItem,
    Interpreter: InterpretItem<Item, RootEvent, Path>,
{
    let settlement =
        <Interpreter as InterpretItem<Item, RootEvent, Path>>::interpret_item(interpreter, item)
            .await;
    match settlement {
        corrupt @ ItemSettlement::Corrupt { .. } => {
            Interpretation::Corrupt(SettledItem::Attempted(corrupt))
        }
        settled => Interpretation::Complete(SettledItem::Attempted(settled)),
    }
}

async fn interpret_items<Item, Interpreter, RootEvent, Path>(
    items: Vec<Item>,
    interpreter: &mut Interpreter,
) -> Interpretation<Vec<ActionItemResult<Item>>>
where
    Item: ActionItem,
    Interpreter: InterpretItem<Item, RootEvent, Path>,
{
    let mut items = items.into_iter();
    let mut settlements = Vec::with_capacity(items.len());
    while let Some(item) = items.next() {
        match settle_item::<Item, Interpreter, RootEvent, Path>(item, interpreter).await {
            Interpretation::Complete(settlement) => settlements.push(settlement),
            Interpretation::Corrupt(settlement) => {
                settlements.push(settlement);
                settlements.extend(items.map(SettledItem::Unattempted));
                return Interpretation::Corrupt(settlements);
            }
        }
    }
    Interpretation::Complete(settlements)
}

fn unattempted_items<Item>(items: Vec<Item>) -> Vec<ActionItemResult<Item>>
where
    Item: ActionItem,
{
    items.into_iter().map(SettledItem::Unattempted).collect()
}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<Delivery<P>>
where
    Interpreter: InterpretItem<Delivery<P>, RootEvent, Path>,
    P: Protocol,
    Delivery<P>: ActionItem,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        interpret_items(self, interpreter)
    }
}

impl<Interpreter, RootEvent, Path, P, Occurrence> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<ChildDelivery<P, Occurrence>>
where
    Interpreter: InterpretItem<ChildDelivery<P, Occurrence>, RootEvent, Path>,
    P: Protocol,
    ChildDelivery<P, Occurrence>: ActionItem,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        interpret_items(self, interpreter)
    }
}

impl<Interpreter, RootEvent, Path, Child, Source, Input, Occurrence>
    InterpretSends<Interpreter, RootEvent, Path>
    for Vec<ChildInput<Child, Source, Input, Occurrence>>
where
    Interpreter: InterpretItem<ChildInput<Child, Source, Input, Occurrence>, RootEvent, Path>,
    Child: Behavior,
    Child::Event: crate::ChildInputIngress<Source, Input>,
    ChildInput<Child, Source, Input, Occurrence>: ActionItem,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        interpret_items(self, interpreter)
    }
}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<EstablishedDelivery<P>>
where
    Interpreter: InterpretItem<EstablishedDelivery<P>, RootEvent, Path>,
    P: Protocol,
    P::Addr: EndpointAddress,
    EstablishedDelivery<P>: ActionItem,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        interpret_items(self, interpreter)
    }
}

impl SendSettlements for Vec<crate::Never> {
    type Settlements = Self;

    fn unattempted(self) -> Self::Settlements {
        self
    }
}

impl<Interpreter, RootEvent, Path> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<crate::Never>
{
    fn interpret(
        self,
        _: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        async move { Interpretation::Complete(self) }
    }
}

impl<T> SendInput<T, Own> for Vec<T> {
    fn emit(&mut self, input: T) {
        self.push(input);
    }
}

/// Requests interpreted by the runtime local to the emitting actor.
///
/// Unlike [`crate::Delivery`], a interpreter request has no actor address. Its
/// recipient is definitionally the interpreter of the actor whose transition
/// emitted it. This distinct send lane lets interpreters route ordinary
/// deliveries and interpreter requests with disjoint static implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpreterRequests<M> {
    requests: Vec<M>,
}

impl<M> InterpreterRequests<M> {
    #[must_use]
    pub fn new(requests: Vec<M>) -> Self {
        Self { requests }
    }
    #[must_use]
    pub fn one(request: M) -> Self {
        Self::new(vec![request])
    }
    #[must_use]
    pub fn as_slice(&self) -> &[M] {
        &self.requests
    }
    pub fn iter(&self) -> core::slice::Iter<'_, M> {
        self.requests.iter()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
    pub fn extend(&mut self, requests: impl IntoIterator<Item = M>) {
        self.requests.extend(requests);
    }
    #[must_use]
    pub fn into_requests(self) -> Vec<M> {
        self.requests
    }
}

impl<M> core::ops::Index<usize> for InterpreterRequests<M> {
    type Output = M;
    fn index(&self, index: usize) -> &Self::Output {
        &self.requests[index]
    }
}

impl<M> IntoIterator for InterpreterRequests<M> {
    type Item = M;
    type IntoIter = std::vec::IntoIter<M>;
    fn into_iter(self) -> Self::IntoIter {
        self.requests.into_iter()
    }
}

impl<'a, M> IntoIterator for &'a InterpreterRequests<M> {
    type Item = &'a M;
    type IntoIter = core::slice::Iter<'a, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.requests.iter()
    }
}

impl<M> SendEffects for InterpreterRequests<M> {
    fn empty() -> Self {
        Self::new(Vec::new())
    }
    fn append(&mut self, mut other: Self) {
        self.requests.append(&mut other.requests);
    }
}

impl<M> LogicalDeliveryProtocols for InterpreterRequests<M> {
    type Protocols = NoBirthProtocols;
}

impl<Event, M> SendsFor<Event> for InterpreterRequests<M>
where
    M: InterpreterRequest,
    M::ReturnToEmitter: ReturnToEmitterFor<Event>,
{
}

impl<M> SendInput<M, Own> for InterpreterRequests<M> {
    fn emit(&mut self, input: M) {
        self.requests.push(input);
    }
}

impl<Request> SendSettlements for InterpreterRequests<Request>
where
    Request: ActionItem,
{
    type Settlements = Vec<ActionItemResult<Request>>;

    fn unattempted(self) -> Self::Settlements {
        unattempted_items(self.requests)
    }
}

impl<Interpreter, RootEvent, Path, Request> InterpretSends<Interpreter, RootEvent, Path>
    for InterpreterRequests<Request>
where
    Interpreter: InterpretItem<Request, RootEvent, Path>,
    Request: ActionItem,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Interpretation<Self::Settlements>> + Send {
        interpret_items(self.requests, interpreter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_accumulation_obeys_identity_and_associativity() {
        let values = vec![1, 2];
        assert_eq!(Vec::new().combine(values.clone()), values);
        assert_eq!(values.clone().combine(Vec::new()), values);

        let left = vec![1].combine(vec![2]).combine(vec![3]);
        let right = vec![1].combine(vec![2].combine(vec![3]));
        assert_eq!(left, right);
    }

    #[test]
    fn vector_and_service_lanes_emit_and_iterate_in_order() {
        assert!(<Vec<u8> as SendEffects>::empty().is_empty());
        let mut vector = Vec::new();
        <Vec<u8> as SendInput<u8, Own>>::emit(&mut vector, 1);
        assert_eq!(vector, [1]);

        let mut services = InterpreterRequests::one(2);
        services.extend([4, 5]);
        <InterpreterRequests<u8> as SendInput<u8, Own>>::emit(&mut services, 3);
        assert!(!services.is_empty());
        assert_eq!(services.as_slice(), [2, 4, 5, 3]);
        assert_eq!(
            (&services).into_iter().copied().collect::<Vec<_>>(),
            [2, 4, 5, 3]
        );
        assert_eq!(services.into_iter().collect::<Vec<_>>(), [2, 4, 5, 3]);

        let requests = InterpreterRequests::new(vec![4, 5]).into_requests();
        assert_eq!(requests, [4, 5]);
    }

    #[test]
    fn source_and_interpreter_lanes_report_complete_ordered_contents() {
        let empty = <SourceActions<u8> as SendEffects>::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let mut prefix = <SourceActions<u8> as SendEffects>::empty();
        <SourceActions<u8> as SendInput<u8, Own>>::emit(&mut prefix, 1);
        <SourceActions<u8> as SendInput<u8, Own>>::emit(&mut prefix, 2);
        let mut suffix = <SourceActions<u8> as SendEffects>::empty();
        <SourceActions<u8> as SendInput<u8, Own>>::emit(&mut suffix, 3);
        <SourceActions<u8> as SendInput<u8, Own>>::emit(&mut suffix, 4);
        prefix.append(suffix);
        assert!(!prefix.is_empty());
        assert_eq!(prefix.len(), 4);
        assert_eq!(prefix.into_items(), [1, 2, 3, 4]);

        let requests = InterpreterRequests::new(vec![5, 6, 7]);
        assert_eq!(requests.len(), 3);
    }

    struct Returning;

    impl InterpreterRequest for Returning {
        type ReturnToEmitter = ReturnsToEmitter<u8, crate::Here>;
    }

    fn lawful<Event, Effects: SendsFor<Event>>() {}

    #[test]
    fn local_return_proofs_compose_through_exact_event_layers() {
        type Inner = crate::EventLayer<u8, crate::User<crate::MailAddr, ()>>;
        type Outer = crate::EventLayer<(), Inner>;

        lawful::<Inner, InterpreterRequests<Returning>>();
        lawful::<Outer, SendLayer<NoSends, InterpreterRequests<Returning>>>();
    }

    #[test]
    fn send_layer_emits_into_its_designated_owned_lane() {
        let mut effects = SendLayer::new(Vec::<u8>::new(), Vec::<u16>::new());
        <SendLayer<Vec<u8>, Vec<u16>> as SendInput<u8, Own>>::emit(&mut effects, 7);
        assert_eq!(effects.owned, [7]);
        assert!(effects.inner.is_empty());
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Seen {
        Inner(u8),
        Outer(u16),
    }

    struct Trace(Vec<Seen>);

    type TraceEvent =
        crate::EventLayer<u16, crate::EventLayer<u8, crate::User<crate::MailAddr, ()>>>;

    impl ActionItem for u8 {
        type Accepted = ();
        type Rejection = crate::Never;
        type Prerequisite = crate::Never;
    }

    impl ActionItem for u16 {
        type Accepted = ();
        type Rejection = crate::Never;
        type Prerequisite = crate::Never;
    }

    impl InterpretItem<u8, TraceEvent, crate::Inside<crate::Here>> for Trace {
        fn interpret_item(
            &mut self,
            request: u8,
        ) -> impl Future<
            Output = ItemSettlement<
                u8,
                <u8 as ActionItem>::Accepted,
                <u8 as ActionItem>::Rejection,
                <u8 as ActionItem>::Prerequisite,
            >,
        > + Send {
            async move {
                self.0.push(Seen::Inner(request));
                ItemSettlement::Accepted(())
            }
        }
    }

    impl InterpretItem<u16, TraceEvent, crate::Here> for Trace {
        fn interpret_item(
            &mut self,
            request: u16,
        ) -> impl Future<
            Output = ItemSettlement<
                u16,
                <u16 as ActionItem>::Accepted,
                <u16 as ActionItem>::Rejection,
                <u16 as ActionItem>::Prerequisite,
            >,
        > + Send {
            async move {
                self.0.push(Seen::Outer(request));
                ItemSettlement::Accepted(())
            }
        }
    }

    #[tokio::test]
    async fn structural_interpretation_visits_every_lane_inner_to_outer() {
        let effects = SendLayer::new(
            InterpreterRequests::new(vec![3_u16, 5]),
            SendLayer::new(InterpreterRequests::one(2_u8), NoSends),
        );
        let mut trace = Trace(Vec::new());

        <_ as InterpretSends<_, TraceEvent, crate::Here>>::interpret(effects, &mut trace).await;

        assert_eq!(trace.0, [Seen::Inner(2), Seen::Outer(3), Seen::Outer(5)]);
    }
}
