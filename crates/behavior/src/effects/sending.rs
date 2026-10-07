//! Typed send effects, their composition contract, and event ownership.

use crate::{
    Behavior, BirthProtocol, BirthProtocolProduct, ChildDelivery, ChildInput, ComposedEvent,
    Delivery, EstablishedDelivery, InjectEvent, Inside, NoBirthProtocols, Protocol,
    RecipientAddress,
};
use core::future::{self, Future};
use std::vec;

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
/// ```compile_fail,E0308
/// struct Request;
/// struct RequiredRejection;
/// struct RuntimeRejection;
/// impl behavior::ActionItem for Request {
///     type Accepted = (); type Rejection = RequiredRejection; type Prerequisite = behavior::Never;
///     type Custody = (Option<Self>, Option<Self::Reply>);
///     type Input<'a> = &'a mut Option<Self> where Self: 'a;
///     type Reply = behavior::ItemSettlement<Self, (), RequiredRejection, behavior::Never>;
///     fn prepare_interpretation(progress: &mut Option<behavior::InterpretationProgress<Self, Self::Custody, Self::Reply>>) { behavior::prepare_item::<Self>(progress); }
///     fn interpretation_input<'a>(custody: &'a mut Self::Custody) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)> where Self: 'a {
///         let (input, received) = custody;
///         match (&*input, &*received) { (Some(_), None) => Some((input, received)), _ => None }
///     }
///     fn finish_interpretation(progress: &mut Option<behavior::InterpretationProgress<Self, Self::Custody, Self::Reply>>) { behavior::finish_item::<Self>(progress); }
/// }
/// struct Runtime;
/// impl behavior::InterpretItem<Request, (), behavior::Here> for Runtime {
///     fn interpret_item<'a>(&'a mut self, input: &'a mut Option<Request>, received: &'a mut Option<<Request as behavior::ActionItem>::Reply>) -> impl core::future::Future<Output = ()> + Send + 'a where Request: 'a {
///         async move {
///             if received.is_some() { return; }
///             let Some(item) = input.take() else { return; };
///             *received = Some(behavior::ItemSettlement::Rejected { item, reason: RuntimeRejection });
///         }
///     }
/// }
/// ```
pub trait ActionItem: Sized + Send {
    type Accepted: Send;
    type Rejection: Send;
    type Prerequisite: Send;
    type Custody;
    type Input<'a>
    where
        Self: 'a;
    type Reply;

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                Self::Custody,
                ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>,
            >,
        >,
    );
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a;
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                Self::Custody,
                ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>,
            >,
        >,
    );

    /// Keep an accepted value only while it still carries terminal custody.
    ///
    /// The default discharges and destroys the receipt. An implementation
    /// returning `Some` must return the same owned value on every later offer;
    /// it must not perform an effect or consume authority still promised by
    /// that value. This decision does not change settlement status.
    fn retain_accepted(_: Self::Accepted) -> Option<Self::Accepted> {
        None
    }
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
    fn interpret_item<'a>(
        &'a mut self,
        input: Item::Input<'a>,
        received: &'a mut Option<Item::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Item: 'a;
}

/// Caller-owned cold input, partial normal interpretation, or acquired result.
///
/// Partial custody retains all available typed facts across producer disposal.
/// This is not source admission, native failure, or an actor protocol value.
pub enum InterpretationProgress<Input, Custody, Settlement> {
    Original(Input),
    Interpreting(Custody),
    Completed(Interpretation<Settlement>),
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
    type Settlements: Send + ClassifySettlement;
    type SourceCustody: Send;
    type InterpretationCustody;

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    );
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    );
    // Replaces consuming unattempted(self), so custom traversal does not own
    // the parent's remaining sibling while other sibling facts stay acquired.
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    );
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
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send;
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
/// struct OpaqueSends;
/// impl behavior::SendEffects for OpaqueSends {
///     fn empty() -> Self { Self }
///     fn append(&mut self, _: Self) {}
/// }
/// impl<E> behavior::SendsFor<E> for OpaqueSends {}
/// struct Actor;
/// impl behavior::Protocol for Actor { type Addr = behavior::MailAddr; type Msg = (); }
/// impl behavior::Behavior for Actor {
///     type Protocol = Self;
///     type Event = behavior::User<behavior::MailAddr, ()>;
///     type Sends = OpaqueSends;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event)
///         -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// }
/// fn require_complete<B: behavior::LogicalHostRequirements>() {}
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
/// struct Request;
/// impl behavior::InterpreterRequest for Request {
///     type ReturnToEmitter = behavior::ReturnsToEmitter<u8, behavior::Here>;
///     type LogicalProtocols = behavior::NoBirthProtocols;
/// }
/// fn lawful<E, F: behavior::SendsFor<E>>() {}
/// type Inner = behavior::EventLayer<u8, behavior::User<behavior::MailAddr, ()>>;
/// type Outer = behavior::EventLayer<(), Inner>;
/// lawful::<Outer, behavior::InterpreterRequests<Request>>();
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
    type SourceCustody = (
        Option<ActionItemResult<Item>>,
        vec::IntoIter<ActionItemResult<Item>>,
        Option<Result<(), ActionItemResult<Item>>>,
    );
    type InterpretationCustody = Vec<
        Option<
            InterpretationProgress<
                Item,
                Item::Custody,
                ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
            >,
        >,
    >;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        if matches!(progress, Some(InterpretationProgress::Original(_))) {
            match progress.take() {
                Some(InterpretationProgress::Original(original)) => {
                    *progress = Some(InterpretationProgress::Interpreting(
                        original
                            .items
                            .into_iter()
                            .map(|item| Some(InterpretationProgress::Original(item)))
                            .collect(),
                    ));
                }
                retained => *progress = retained,
            }
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        let Some(InterpretationProgress::Interpreting(rows)) = progress else {
            return;
        };
        if let Some(received) = finish_item_rows::<Item>(rows) {
            *progress = Some(InterpretationProgress::Completed(match received {
                Interpretation::Complete(items) => {
                    Interpretation::Complete(SourceSettlements::new(items))
                }
                Interpretation::Corrupt(items) => {
                    Interpretation::Corrupt(SourceSettlements::new(items))
                }
            }));
        }
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                let items = original.items;
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    SourceSettlements::new(
                        items.into_iter().map(SettledItem::Unattempted).collect(),
                    ),
                )));
            }
            retained => *progress = retained,
        }
    }
}

impl<Interpreter, RootEvent, Path, Item> InterpretSends<Interpreter, RootEvent, Path>
    for SourceActions<Item>
where
    Interpreter: InterpretItem<Item, RootEvent, Path>,
    Item: SourceAction,
    Item::Custody: Send,
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
                match row.as_ref() {
                    Some(InterpretationProgress::Completed(Interpretation::Corrupt(_))) => break,
                    Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {
                        continue;
                    }
                    Some(InterpretationProgress::Original(_))
                    | Some(InterpretationProgress::Interpreting(_)) => {}
                    None => return,
                }
                settle_item::<Item, Interpreter, RootEvent, Path>(row, interpreter).await;
                Item::finish_interpretation(row);
                match row.as_ref() {
                    Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {}
                    _ => break,
                }
            }
            Self::finish_interpretation(progress);
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
///     fn admit_source(&mut self, input: &mut Option<Input>, reply: &mut Option<Result<(), Input>>)
///         -> impl core::future::Future<Output = ()> + Send
///     {
///         async move {
///             if let Some(input) = input.take() { *reply = Some(Err(input)); }
///         }
///     }
/// }
/// ```
pub trait SourceAdmission<RootEvent, Source, Input>: Send
where
    RootEvent: crate::EventIngress<Source, Input>,
{
    fn admit_source(
        &mut self,
        input: &mut Option<Input>,
        reply: &mut Option<Result<(), Input>>,
    ) -> impl Future<Output = ()> + Send;
}

/// Result of offering at most one ordered source settlement.
pub enum SourceCustody<Residual> {
    /// No source input remains in the complete residual product.
    Exhausted(Residual),
    /// No live-source input remains, but exact terminal custody is required.
    Retained(Residual),
    /// Exactly one source input transferred; the residual remains in custody.
    Admitted(Residual),
    /// Admission closed; the residual contains the current and untouched suffix.
    Closed(Residual),
}

impl<Residual> SourceCustody<Residual> {
    /// Transform the complete residual without changing its custody state.
    #[must_use]
    pub fn map<Mapped>(self, map: impl FnOnce(Residual) -> Mapped) -> SourceCustody<Mapped> {
        match self {
            Self::Exhausted(residual) => SourceCustody::Exhausted(map(residual)),
            Self::Retained(residual) => SourceCustody::Retained(map(residual)),
            Self::Admitted(residual) => SourceCustody::Admitted(map(residual)),
            Self::Closed(residual) => SourceCustody::Closed(map(residual)),
        }
    }
}

/// Static one-at-a-time source admission for one complete settlement product.
/// The actual source product before offering, its outside partial owner, or
/// its acquired complete normal result. Native causes remain runtime-owned.
pub enum SourceProgress<Settlement, Custody> {
    Original(Settlement),
    Offering(Custody),
    Completed(SourceCustody<Settlement>),
}

pub trait SourceSettlementCustody<Host, RootEvent>: Sized {
    type Custody: Send;

    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>);

    fn offer_next_to_source(
        custody: &mut Self::Custody,
        host: &mut Host,
    ) -> impl Future<Output = ()> + Send;

    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>);
}

pub(super) async fn offer_source_in_order<Host, RootEvent, Earlier, Later>(
    custody: &mut (
        Option<SourceProgress<Earlier, Earlier::Custody>>,
        Option<SourceProgress<Later, Later::Custody>>,
    ),
    host: &mut Host,
) where
    Host: Send,
    Earlier: SourceSettlementCustody<Host, RootEvent> + Send,
    Later: SourceSettlementCustody<Host, RootEvent> + Send,
{
    match &custody.0 {
        Some(SourceProgress::Original(_)) => Earlier::prepare_source(&mut custody.0),
        Some(SourceProgress::Offering(_) | SourceProgress::Completed(_)) | None => {}
    }
    if let Some(SourceProgress::Offering(earlier)) = &mut custody.0 {
        {
            Earlier::offer_next_to_source(earlier, host).await;
        }
        Earlier::finish_source(&mut custody.0);
    }
    match &custody.0 {
        Some(SourceProgress::Completed(
            SourceCustody::Exhausted(_) | SourceCustody::Retained(_),
        )) => {}
        Some(SourceProgress::Completed(SourceCustody::Admitted(_) | SourceCustody::Closed(_)))
        | Some(SourceProgress::Original(_) | SourceProgress::Offering(_))
        | None => return,
    }
    match &custody.1 {
        Some(SourceProgress::Original(_)) => Later::prepare_source(&mut custody.1),
        Some(SourceProgress::Offering(_) | SourceProgress::Completed(_)) | None => {}
    }
    if let Some(SourceProgress::Offering(later)) = &mut custody.1 {
        {
            Later::offer_next_to_source(later, host).await;
        }
        Later::finish_source(&mut custody.1);
    }
}

impl<Host, RootEvent, Earlier, Later> SourceSettlementCustody<Host, RootEvent> for (Earlier, Later)
where
    Host: Send,
    Earlier: SourceSettlementCustody<Host, RootEvent> + Send,
    Later: SourceSettlementCustody<Host, RootEvent> + Send,
{
    type Custody = (
        Option<SourceProgress<Earlier, Earlier::Custody>>,
        Option<SourceProgress<Later, Later::Custody>>,
    );

    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        let original = match progress.take() {
            Some(SourceProgress::Original(original)) => original,
            progress_ => {
                *progress = progress_;
                return;
            }
        };
        // No child method is called until BOTH original child owners are stored.
        *progress = Some(SourceProgress::Offering((
            Some(SourceProgress::Original(original.0)),
            Some(SourceProgress::Original(original.1)),
        )));
    }

    fn offer_next_to_source(
        custody: &mut Self::Custody,
        host: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        offer_source_in_order::<Host, RootEvent, Earlier, Later>(custody, host)
    }

    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        // Child finalization has already borrowed each child slot separately.
        // This total structural combination calls no child or host method.
        let completed = match progress.take() {
            Some(SourceProgress::Offering((
                Some(SourceProgress::Completed(SourceCustody::Admitted(earlier))),
                Some(SourceProgress::Original(later)),
            ))) => SourceCustody::Admitted((earlier, later)),
            Some(SourceProgress::Offering((
                Some(SourceProgress::Completed(SourceCustody::Closed(earlier))),
                Some(SourceProgress::Original(later)),
            ))) => SourceCustody::Closed((earlier, later)),
            Some(SourceProgress::Offering((
                Some(SourceProgress::Completed(SourceCustody::Exhausted(earlier))),
                Some(SourceProgress::Completed(later)),
            ))) => later.map(|later| (earlier, later)),
            Some(SourceProgress::Offering((
                Some(SourceProgress::Completed(SourceCustody::Retained(earlier))),
                Some(SourceProgress::Completed(later)),
            ))) => match later {
                SourceCustody::Exhausted(later) | SourceCustody::Retained(later) => {
                    SourceCustody::Retained((earlier, later))
                }
                SourceCustody::Admitted(later) => SourceCustody::Admitted((earlier, later)),
                SourceCustody::Closed(later) => SourceCustody::Closed((earlier, later)),
            },
            progress_ => {
                *progress = progress_;
                return;
            }
        };
        *progress = Some(SourceProgress::Completed(completed));
    }
}

impl<Host, RootEvent, Item> SourceSettlementCustody<Host, RootEvent> for SourceSettlements<Item>
where
    Host: SourceAdmission<RootEvent, Item::Source, ActionItemResult<Item>>,
    RootEvent: crate::EventIngress<Item::Source, ActionItemResult<Item>>,
    Item: SourceAction,
{
    type Custody = (
        Option<ActionItemResult<Item>>,
        vec::IntoIter<ActionItemResult<Item>>,
        Option<Result<(), ActionItemResult<Item>>>,
    );

    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        let original = match progress.take() {
            Some(SourceProgress::Original(original)) => original,
            progress_ => {
                *progress = progress_;
                return;
            }
        };
        if original.inputs.is_empty() {
            *progress = Some(SourceProgress::Completed(SourceCustody::Exhausted(
                original,
            )));
            return;
        }
        let mut remaining = original.inputs.into_iter();
        let input = remaining.next();
        *progress = Some(SourceProgress::Offering((input, remaining, None)));
    }

    fn offer_next_to_source(
        custody: &mut Self::Custody,
        host: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        async move {
            if custody.2.is_none() {
                host.admit_source(&mut custody.0, &mut custody.2).await;
            }
        }
    }

    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        let completed = match progress.take() {
            Some(SourceProgress::Offering((None, remaining, Some(Ok(()))))) => {
                SourceCustody::Admitted(Self::new(remaining.collect()))
            }
            Some(SourceProgress::Offering((None, remaining, Some(Err(input))))) => {
                let Some(capacity) = remaining.len().checked_add(1) else {
                    *progress = Some(SourceProgress::Offering((
                        None,
                        remaining,
                        Some(Err(input)),
                    )));
                    return;
                };
                let mut retained = Vec::with_capacity(capacity);
                retained.push(input);
                retained.extend(remaining);
                SourceCustody::Closed(Self::new(retained))
            }
            progress_ => {
                *progress = progress_;
                return;
            }
        };
        *progress = Some(SourceProgress::Completed(completed));
    }
}

impl<Host, RootEvent> SourceSettlementCustody<Host, RootEvent> for NoSends {
    type Custody = Self;
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Completed(
                SourceCustody::Exhausted(original),
            )),
            progress => progress,
        };
    }
    fn offer_next_to_source(
        _: &mut Self::Custody,
        _: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        future::ready(())
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

impl<Host, RootEvent, Item> SourceSettlementCustody<Host, RootEvent> for Vec<ActionItemResult<Item>>
where
    Item: ActionItem,
{
    type Custody = (Self, vec::IntoIter<ActionItemResult<Item>>);
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) if original.is_empty() => Some(
                SourceProgress::Completed(SourceCustody::Exhausted(original)),
            ),
            Some(SourceProgress::Original(original)) => {
                Some(SourceProgress::Offering((Vec::new(), original.into_iter())))
            }
            progress => progress,
        };
    }
    fn offer_next_to_source(
        custody: &mut Self::Custody,
        _: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        async move {
            for settlement in custody.1.by_ref() {
                let settlement = match settlement {
                    SettledItem::Attempted(ItemSettlement::Accepted(accepted)) => {
                        Item::retain_accepted(accepted).map(|accepted| {
                            SettledItem::Attempted(ItemSettlement::Accepted(accepted))
                        })
                    }
                    settlement => Some(settlement),
                };
                if let Some(settlement) = settlement {
                    custody.0.push(settlement);
                }
            }
        }
    }
    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Offering((retained, remaining)))
                if remaining.as_slice().is_empty() =>
            {
                if retained.is_empty() {
                    Some(SourceProgress::Completed(SourceCustody::Exhausted(
                        retained,
                    )))
                } else {
                    Some(SourceProgress::Completed(SourceCustody::Retained(retained)))
                }
            }
            progress => progress,
        };
    }
}

impl<Host, RootEvent> SourceSettlementCustody<Host, RootEvent> for Vec<crate::Never> {
    type Custody = Self;
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Completed(
                SourceCustody::Exhausted(original),
            )),
            progress => progress,
        };
    }
    fn offer_next_to_source(
        _: &mut Self::Custody,
        _: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        future::ready(())
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

impl<Host, RootEvent, Owned, Inner> SourceSettlementCustody<Host, RootEvent>
    for SendLayer<Owned, Inner>
where
    Host: Send,
    Owned: SourceSettlementCustody<Host, RootEvent> + Send,
    Inner: SourceSettlementCustody<Host, RootEvent> + Send,
{
    type Custody = <(Inner, Owned) as SourceSettlementCustody<Host, RootEvent>>::Custody;
    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Offering((
                Some(SourceProgress::Original(original.inner)),
                Some(SourceProgress::Original(original.owned)),
            ))),
            progress => progress,
        };
    }
    fn offer_next_to_source(
        custody: &mut Self::Custody,
        host: &mut Host,
    ) -> impl Future<Output = ()> + Send {
        offer_source_in_order::<Host, RootEvent, Inner, Owned>(custody, host)
    }
    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        let custody = match progress.take() {
            Some(SourceProgress::Offering(custody)) => custody,
            retained => {
                *progress = retained;
                return;
            }
        };
        // The concrete Core tuple finish is total and calls no child methods.
        let mut product = Some(SourceProgress::Offering(custody));
        <(Inner, Owned) as SourceSettlementCustody<Host, RootEvent>>::finish_source(&mut product);
        *progress = match product {
            Some(SourceProgress::Original((inner, owned))) => {
                Some(SourceProgress::Original(Self::new(owned, inner)))
            }
            Some(SourceProgress::Offering(custody)) => Some(SourceProgress::Offering(custody)),
            Some(SourceProgress::Completed(reply)) => Some(SourceProgress::Completed(
                reply.map(|(inner, owned)| Self::new(owned, inner)),
            )),
            None => None,
        };
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
/// emitter is wrapped. `LogicalProtocols` lists possible logical destinations
/// of the request in declaration order, independently of any value's selected
/// variant; exact and creator-local destinations contribute none.
pub trait InterpreterRequest {
    type ReturnToEmitter;
    type LogicalProtocols: BirthProtocolProduct;
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
    type LogicalProtocols = NoBirthProtocols;
}

impl<R> ActionItem for ReportToParent<R>
where
    R: Send,
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
        let (input, received) = custody;
        if input.is_some() && received.is_none() {
            Some((input, received))
        } else {
            None
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

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
    type SourceCustody = Self;
    type InterpretationCustody = Self;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    original,
                )))
            }
            retained => *progress = retained,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        Self::prepare_interpretation(progress);
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    original,
                )))
            }
            retained => *progress = retained,
        }
    }
}

impl ClassifySettlement for NoSends {
    fn settlement_status(&self) -> SettlementStatus {
        SettlementStatus::Accepted
    }
}

impl<Interpreter, RootEvent, Path> InterpretSends<Interpreter, RootEvent, Path> for NoSends {
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        _: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            Self::prepare_interpretation(progress);
        }
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
    // Actual old associated SourceCustody body is retained unchanged.
    type SourceCustody = (
        Option<SourceProgress<Inner::Settlements, Inner::SourceCustody>>,
        Option<SourceProgress<Owned::Settlements, Owned::SourceCustody>>,
    );
    type InterpretationCustody = SendLayer<
        Option<InterpretationProgress<Owned, Owned::InterpretationCustody, Owned::Settlements>>,
        Option<InterpretationProgress<Inner, Inner::InterpretationCustody, Inner::Settlements>>,
    >;

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        if matches!(progress, Some(InterpretationProgress::Original(_))) {
            match progress.take() {
                Some(InterpretationProgress::Original(layer)) => {
                    *progress = Some(InterpretationProgress::Interpreting(SendLayer::new(
                        Some(InterpretationProgress::Original(layer.owned)),
                        Some(InterpretationProgress::Original(layer.inner)),
                    )));
                }
                other => *progress = other,
            }
        }
        // Both originals are installed. Each actual interpret operation
        // prepares only its own child; untouched siblings stay Original.
    }

    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        let Some(InterpretationProgress::Interpreting(layer)) = progress else {
            return;
        };
        // Total structural extraction only. Child preparation/finalization
        // borrows the outside child slots in the actual traversal below.
        if !matches!(
            (&layer.inner, &layer.owned),
            (
                Some(InterpretationProgress::Completed(_)),
                Some(InterpretationProgress::Completed(_))
            )
        ) {
            return;
        }
        match progress.take() {
            Some(InterpretationProgress::Interpreting(SendLayer {
                owned: Some(InterpretationProgress::Completed(owned)),
                inner: Some(InterpretationProgress::Completed(inner)),
            })) => {
                let combined = match (inner, owned) {
                    (Interpretation::Complete(inner), Interpretation::Complete(owned)) => {
                        Interpretation::Complete(SendLayer::new(owned, inner))
                    }
                    (
                        Interpretation::Complete(inner) | Interpretation::Corrupt(inner),
                        Interpretation::Corrupt(owned),
                    )
                    | (Interpretation::Corrupt(inner), Interpretation::Complete(owned)) => {
                        Interpretation::Corrupt(SendLayer::new(owned, inner))
                    }
                };
                *progress = Some(InterpretationProgress::Completed(combined));
            }
            other => *progress = other,
        }
    }

    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        // Install child originals but do not run prepare callbacks: these are
        // untouched children after another owner's actual corrupt fact.
        if matches!(progress, Some(InterpretationProgress::Original(_))) {
            match progress.take() {
                Some(InterpretationProgress::Original(layer)) => {
                    *progress = Some(InterpretationProgress::Interpreting(SendLayer::new(
                        Some(InterpretationProgress::Original(layer.owned)),
                        Some(InterpretationProgress::Original(layer.inner)),
                    )));
                }
                other => *progress = other,
            }
        }
        if let Some(InterpretationProgress::Interpreting(layer)) = progress {
            Inner::unattempted(&mut layer.inner);
            Owned::unattempted(&mut layer.owned);
        }
        Self::finish_interpretation(progress);
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
    Owned::InterpretationCustody: Send,
    Inner::InterpretationCustody: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            Self::prepare_interpretation(progress);
            let Some(InterpretationProgress::Interpreting(layer)) = progress else {
                return;
            };
            <Inner as InterpretSends<Interpreter, RootEvent, Inside<Path>>>::interpret(
                &mut layer.inner,
                interpreter,
            )
            .await;
            Inner::finish_interpretation(&mut layer.inner);
            match &layer.inner {
                Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {
                    <Owned as InterpretSends<Interpreter, RootEvent, Path>>::interpret(
                        &mut layer.owned,
                        interpreter,
                    )
                    .await;
                    Owned::finish_interpretation(&mut layer.owned);
                }
                Some(InterpretationProgress::Completed(Interpretation::Corrupt(_))) => {
                    Owned::unattempted(&mut layer.owned);
                }
                _ => return,
            }
            Self::finish_interpretation(progress);
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
    P::Addr: RecipientAddress,
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
    type SourceCustody = (Self::Settlements, vec::IntoIter<ActionItemResult<Item>>);
    type InterpretationCustody = Vec<
        Option<
            InterpretationProgress<
                Item,
                Item::Custody,
                ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
            >,
        >,
    >;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        prepare_items(progress);
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        finish_items(progress);
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        unattempted_items(progress);
    }
}

/// Prepare an ordinary request and its independent outside reply destination.
/// Construction calls no interpreter and transfers no containing sibling.
pub fn prepare_item<Item>(
    progress: &mut Option<
        InterpretationProgress<
            Item,
            Item::Custody,
            ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
        >,
    >,
) where
    Item: ActionItem<
        Custody = (
            Option<Item>,
            Option<
                ItemSettlement<
                    Item,
                    <Item as ActionItem>::Accepted,
                    <Item as ActionItem>::Rejection,
                    <Item as ActionItem>::Prerequisite,
                >,
            >,
        ),
    >,
{
    if !matches!(progress, Some(InterpretationProgress::Original(_))) {
        return;
    }
    match progress.take() {
        Some(InterpretationProgress::Original(item)) => {
            *progress = Some(InterpretationProgress::Interpreting((Some(item), None)));
        }
        retained => *progress = retained,
    }
}

/// Finalize only an acquired reply after its original input was consumed.
/// Surviving input with a reply, or absent input without a reply, stays owned.
pub fn finish_item<Item>(
    progress: &mut Option<
        InterpretationProgress<
            Item,
            Item::Custody,
            ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
        >,
    >,
) where
    Item: ActionItem<
        Custody = (
            Option<Item>,
            Option<
                ItemSettlement<
                    Item,
                    <Item as ActionItem>::Accepted,
                    <Item as ActionItem>::Rejection,
                    <Item as ActionItem>::Prerequisite,
                >,
            >,
        ),
    >,
{
    if !matches!(
        progress,
        Some(InterpretationProgress::Interpreting((None, Some(_))))
    ) {
        return;
    }
    match progress.take() {
        Some(InterpretationProgress::Interpreting((None, Some(received)))) => {
            let interpretation = match received {
                received @ ItemSettlement::Corrupt { .. } => Interpretation::Corrupt(received),
                received => Interpretation::Complete(received),
            };
            *progress = Some(InterpretationProgress::Completed(interpretation));
        }
        retained => *progress = retained,
    }
}

/// Loan only the current lower input and its outside reply destination.
/// The producer returns unit; the caller finalizes after producer disposal.
/// Empty input or an occupied destination remains an incomplete owned fact.
pub fn settle_item<'a, Item, Interpreter, RootEvent, Path>(
    progress: &'a mut Option<
        InterpretationProgress<
            Item,
            Item::Custody,
            ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
        >,
    >,
    interpreter: &'a mut Interpreter,
) -> impl Future<Output = ()> + Send + 'a
where
    Item: ActionItem + 'a,
    Interpreter: InterpretItem<Item, RootEvent, Path> + 'a,
{
    Item::prepare_interpretation(progress);
    let attempt = match progress {
        Some(InterpretationProgress::Interpreting(custody)) => Item::interpretation_input(custody)
            .map(|(input, received)| {
                <Interpreter as InterpretItem<Item, RootEvent, Path>>::interpret_item(
                    interpreter,
                    input,
                    received,
                )
            }),
        _ => None,
    };
    // Only the already-declared Send host future is retained. No whole custody,
    // parent loan, metadata, or finalization callback is captured here.
    async move {
        if let Some(attempt) = attempt {
            attempt.await;
        }
    }
}

fn prepare_items<Item>(
    progress: &mut Option<
        InterpretationProgress<
            Vec<Item>,
            Vec<
                Option<
                    InterpretationProgress<
                        Item,
                        Item::Custody,
                        ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
                    >,
                >,
            >,
            Vec<ActionItemResult<Item>>,
        >,
    >,
) where
    Item: ActionItem,
{
    if !matches!(progress, Some(InterpretationProgress::Original(_))) {
        return;
    }
    match progress.take() {
        Some(InterpretationProgress::Original(items)) => {
            let rows = items
                .into_iter()
                .map(|item| Some(InterpretationProgress::Original(item)))
                .collect();
            *progress = Some(InterpretationProgress::Interpreting(rows));
        }
        retained => *progress = retained,
    }
}

fn finish_item_rows<Item>(
    rows: &mut Vec<
        Option<
            InterpretationProgress<
                Item,
                Item::Custody,
                ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
            >,
        >,
    >,
) -> Option<Interpretation<Vec<ActionItemResult<Item>>>>
where
    Item: ActionItem,
{
    for row in rows.iter_mut() {
        if matches!(row, Some(InterpretationProgress::Interpreting(_))) {
            Item::finish_interpretation(row);
        }
    }
    let corrupt = rows.iter().position(|row| {
        matches!(
            row,
            Some(InterpretationProgress::Completed(Interpretation::Corrupt(
                _
            )))
        )
    });
    let complete = rows.iter().enumerate().all(|(index, row)| {
        if corrupt.is_some_and(|corrupt| index > corrupt) {
            matches!(row, Some(InterpretationProgress::Original(_)))
        } else {
            matches!(row, Some(InterpretationProgress::Completed(_)))
        }
    });
    if !complete {
        return None;
    }
    let mut remaining = core::mem::take(rows).into_iter();
    let mut received_rows = Vec::with_capacity(remaining.len());
    while let Some(row) = remaining.next() {
        match row {
            row @ Some(InterpretationProgress::Original(_))
            | row @ Some(InterpretationProgress::Completed(_)) => received_rows.push(row),
            row => {
                *rows = received_rows
                    .into_iter()
                    .chain(core::iter::once(row))
                    .chain(remaining)
                    .collect();
                return None;
            }
        }
    }
    let settled = received_rows
        .into_iter()
        .filter_map(|row| match row {
            Some(InterpretationProgress::Original(item)) => Some(SettledItem::Unattempted(item)),
            Some(InterpretationProgress::Completed(
                Interpretation::Complete(received) | Interpretation::Corrupt(received),
            )) => Some(SettledItem::Attempted(received)),
            _ => None,
        })
        .collect();
    Some(match corrupt {
        Some(_) => Interpretation::Corrupt(settled),
        None => Interpretation::Complete(settled),
    })
}

fn finish_items<Item>(
    progress: &mut Option<
        InterpretationProgress<
            Vec<Item>,
            Vec<
                Option<
                    InterpretationProgress<
                        Item,
                        Item::Custody,
                        ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
                    >,
                >,
            >,
            Vec<ActionItemResult<Item>>,
        >,
    >,
) where
    Item: ActionItem,
{
    let Some(InterpretationProgress::Interpreting(rows)) = progress else {
        return;
    };
    if let Some(received) = finish_item_rows(rows) {
        *progress = Some(InterpretationProgress::Completed(received));
    }
}

async fn interpret_items<Item, Interpreter, RootEvent, Path>(
    progress: &mut Option<
        InterpretationProgress<
            Vec<Item>,
            Vec<
                Option<
                    InterpretationProgress<
                        Item,
                        Item::Custody,
                        ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
                    >,
                >,
            >,
            Vec<ActionItemResult<Item>>,
        >,
    >,
    interpreter: &mut Interpreter,
) where
    Item: ActionItem,
    Item::Custody: Send,
    Interpreter: InterpretItem<Item, RootEvent, Path>,
{
    prepare_items(progress);
    let Some(InterpretationProgress::Interpreting(rows)) = progress else {
        return;
    };
    for row in rows {
        match row.as_ref() {
            Some(InterpretationProgress::Completed(Interpretation::Corrupt(_))) => return,
            Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => continue,
            Some(InterpretationProgress::Original(_))
            | Some(InterpretationProgress::Interpreting(_)) => {}
            None => return,
        }
        settle_item::<Item, Interpreter, RootEvent, Path>(row, interpreter).await;
        Item::finish_interpretation(row);
        match row.as_ref() {
            Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {}
            _ => return,
        }
    }
}

fn unattempted_items<Item>(
    progress: &mut Option<
        InterpretationProgress<
            Vec<Item>,
            Vec<
                Option<
                    InterpretationProgress<
                        Item,
                        Item::Custody,
                        ItemSettlement<Item, Item::Accepted, Item::Rejection, Item::Prerequisite>,
                    >,
                >,
            >,
            Vec<ActionItemResult<Item>>,
        >,
    >,
) where
    Item: ActionItem,
{
    if !matches!(progress, Some(InterpretationProgress::Original(_))) {
        return;
    }
    match progress.take() {
        Some(InterpretationProgress::Original(items)) => {
            let untouched = items.into_iter().map(SettledItem::Unattempted).collect();
            *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                untouched,
            )));
        }
        other => *progress = other,
    }
}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<Delivery<P>>
where
    Interpreter: InterpretItem<Delivery<P>, RootEvent, Path>,
    P: Protocol,
    P::Addr: Send,
    P::Msg: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            interpret_items(progress, interpreter).await;
            Self::finish_interpretation(progress);
        }
    }
}

impl<Interpreter, RootEvent, Path, P, Occurrence> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<ChildDelivery<P, Occurrence>>
where
    Interpreter: InterpretItem<ChildDelivery<P, Occurrence>, RootEvent, Path>,
    P: Protocol,
    P::Msg: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            interpret_items(progress, interpreter).await;
            Self::finish_interpretation(progress);
        }
    }
}

impl<Interpreter, RootEvent, Path, Child, Source, Input, Occurrence>
    InterpretSends<Interpreter, RootEvent, Path>
    for Vec<ChildInput<Child, Source, Input, Occurrence>>
where
    Interpreter: InterpretItem<ChildInput<Child, Source, Input, Occurrence>, RootEvent, Path>,
    Child: Behavior,
    Child::Event: crate::ChildInputIngress<Source, Input>,
    Input: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            interpret_items(progress, interpreter).await;
            Self::finish_interpretation(progress);
        }
    }
}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<EstablishedDelivery<P>>
where
    Interpreter: InterpretItem<EstablishedDelivery<P>, RootEvent, Path>,
    P: Protocol,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: Send,
    P::Msg: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            interpret_items(progress, interpreter).await;
            Self::finish_interpretation(progress);
        }
    }
}

impl SendSettlements for Vec<crate::Never> {
    type Settlements = Self;
    type SourceCustody = Self;
    type InterpretationCustody = Self;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    original,
                )))
            }
            retained => *progress = retained,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        Self::prepare_interpretation(progress);
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    original,
                )))
            }
            retained => *progress = retained,
        }
    }
}

impl<Interpreter, RootEvent, Path> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<crate::Never>
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        _: &mut Interpreter,
    ) -> impl Future<Output = ()> + Send {
        async move {
            Self::prepare_interpretation(progress);
        }
    }
}

impl<T> SendInput<T, Own> for Vec<T> {
    fn emit(&mut self, input: T) {
        self.push(input);
    }
}

/// Requests interpreted by the runtime local to the emitting actor.
///
/// The request itself is interpreted by the runtime local to the emitting
/// actor. A request may carry a separate typed logical or exact destination;
/// its [`InterpreterRequest::LogicalProtocols`] reports any possible logical
/// destination. This lane keeps ordinary deliveries and interpreter operations
/// statically distinct.
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

impl<M: InterpreterRequest> LogicalDeliveryProtocols for InterpreterRequests<M> {
    type Protocols = M::LogicalProtocols;
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
    type SourceCustody = (Self::Settlements, vec::IntoIter<ActionItemResult<Request>>);
    type InterpretationCustody = Vec<
        Option<
            InterpretationProgress<
                Request,
                Request::Custody,
                ItemSettlement<
                    Request,
                    Request::Accepted,
                    Request::Rejection,
                    Request::Prerequisite,
                >,
            >,
        >,
    >;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        if matches!(progress, Some(InterpretationProgress::Original(_))) {
            match progress.take() {
                Some(InterpretationProgress::Original(original)) => {
                    *progress = Some(InterpretationProgress::Interpreting(
                        original
                            .requests
                            .into_iter()
                            .map(|item| Some(InterpretationProgress::Original(item)))
                            .collect(),
                    ));
                }
                retained => *progress = retained,
            }
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        let Some(InterpretationProgress::Interpreting(rows)) = progress else {
            return;
        };
        if let Some(received) = finish_item_rows::<Request>(rows) {
            *progress = Some(InterpretationProgress::Completed(received));
        }
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                let items = original.requests;
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    items.into_iter().map(SettledItem::Unattempted).collect(),
                )));
            }
            retained => *progress = retained,
        }
    }
}

impl<Interpreter, RootEvent, Path, Request> InterpretSends<Interpreter, RootEvent, Path>
    for InterpreterRequests<Request>
where
    Interpreter: InterpretItem<Request, RootEvent, Path>,
    Request: ActionItem,
    Request::Custody: Send,
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
                match row.as_ref() {
                    Some(InterpretationProgress::Completed(Interpretation::Corrupt(_))) => break,
                    Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {
                        continue;
                    }
                    Some(InterpretationProgress::Original(_))
                    | Some(InterpretationProgress::Interpreting(_)) => {}
                    None => return,
                }
                settle_item::<Request, Interpreter, RootEvent, Path>(row, interpreter).await;
                Request::finish_interpretation(row);
                match row.as_ref() {
                    Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {}
                    _ => break,
                }
            }
            Self::finish_interpretation(progress);
        }
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
        let borrowed = (&services).into_iter().copied().collect::<Vec<_>>();
        assert_eq!(borrowed, [2, 4, 5, 3]);
        let owned = services.into_iter().collect::<Vec<_>>();
        assert_eq!(owned, [2, 4, 5, 3]);

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
        let items = prefix.into_items();
        assert_eq!(items, [1, 2, 3, 4]);

        let requests = InterpreterRequests::new(vec![5, 6, 7]);
        assert_eq!(requests.len(), 3);
    }

    struct Returning;

    impl InterpreterRequest for Returning {
        type ReturnToEmitter = ReturnsToEmitter<u8, crate::Here>;
        type LogicalProtocols = NoBirthProtocols;
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
            let (input, received) = custody;
            match (&*input, &*received) {
                (Some(_), None) => Some((input, received)),
                _ => None,
            }
        }
        fn finish_interpretation(
            progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
        ) {
            finish_item::<Self>(progress);
        }

        type Accepted = ();
        type Rejection = crate::Never;
        type Prerequisite = crate::Never;
    }

    impl ActionItem for u16 {
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
            let (input, received) = custody;
            match (&*input, &*received) {
                (Some(_), None) => Some((input, received)),
                _ => None,
            }
        }
        fn finish_interpretation(
            progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
        ) {
            finish_item::<Self>(progress);
        }

        type Accepted = ();
        type Rejection = crate::Never;
        type Prerequisite = crate::Never;
    }

    impl InterpretItem<u8, TraceEvent, crate::Inside<crate::Here>> for Trace {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<u8>,
            received: &'a mut Option<<u8 as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            u8: 'a,
        {
            async move {
                if received.is_some() {
                    return;
                }
                let Some(request) = input.take() else {
                    return;
                };
                self.0.push(Seen::Inner(request));
                *received = Some(ItemSettlement::Accepted(()));
            }
        }
    }

    impl InterpretItem<u16, TraceEvent, crate::Here> for Trace {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<u16>,
            received: &'a mut Option<<u16 as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            u16: 'a,
        {
            async move {
                if received.is_some() {
                    return;
                }
                let Some(request) = input.take() else {
                    return;
                };
                self.0.push(Seen::Outer(request));
                *received = Some(ItemSettlement::Accepted(()));
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

        let mut progress = Some(InterpretationProgress::Original(effects));
        <_ as InterpretSends<_, TraceEvent, crate::Here>>::interpret(&mut progress, &mut trace)
            .await;
        let Some(InterpretationProgress::Completed(Interpretation::Complete(settlement))) =
            progress
        else {
            panic!("the exact structural trace must complete every lane");
        };
        assert_eq!(
            settlement.owned,
            [
                SettledItem::Attempted(ItemSettlement::Accepted(())),
                SettledItem::Attempted(ItemSettlement::Accepted(()))
            ]
        );
        assert_eq!(
            settlement.inner.owned,
            [SettledItem::Attempted(ItemSettlement::Accepted(()))]
        );
        let NoSends = settlement.inner.inner;

        assert_eq!(trace.0, [Seen::Inner(2), Seen::Outer(3), Seen::Outer(5)]);
    }
}
