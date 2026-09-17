//! Pure, typed actor-behavior primitives.
//!
//! [`Protocol`] is stable public destination identity (`Addr` plus `Msg`). A
//! [`Behavior`] separately owns state and folds its complete [`Behavior::Event`]
//! algebra into exactly [`Actions`]: sends, fresh creations, and its next
//! behavior or termination. A protocol is not a behavior, and `Behavior` is not
//! a `Protocol` supertrait. Higher capabilities extend internal event and
//! effect algebras while transparent wrappers preserve [`Behavior::Protocol`].
//!
//! Finite mailbox execution belongs to a runtime or test driver, not to this
//! one-turn behavior algebra.
//!
//! ```compile_fail
//! use behavior::ActionReducer;
//! ```
//!
//! Capability-restricted action products use the existing `Actions` algebra;
//! there is no second convenience wrapper with an overlapping contract.
//!
//! ```compile_fail
//! fn accepts_duplicate(_: behavior::Effect<u8>) {}
//! ```

// The `#[behavior]` expansion emits `::behavior::…` paths; this alias lets the
// expansion resolve inside this crate too.
extern crate self as behavior;

mod actor;
mod effects;
mod next;
mod transition;
mod user_event;

pub use actor::{
    Address, AllocationRejection, BirthMode, BirthNodeAppend, BirthNodeAt, BirthNodeLogicalHosts,
    BirthNodeProtocols, BirthProtocol, BirthProtocolAt, BirthProtocolHead, BirthProtocolProduct,
    BirthProtocolTail, BirthProtocols, Births, ChildChoice, ChildCons, ChildCreationOutcome,
    ChildCreationProduct, ChildCreationSettled, ChildDelivery, ChildDeliveryReason, ChildHead,
    ChildInput, ChildInputReason, ChildNamespaceExhausted, ChildOccurrence, ChildOccurrenceProduct,
    ChildOccurrenceProductAt, ChildOccurrenceResolution, ChildOccurrenceShape, ChildOccurrences,
    ChildPosition, ChildProduct, ChildReport, ChildRole, ChildTail, Children, CreateChild,
    CreationCorrelation, CreationId, CreationKind, CreationRejection, CreationSequence, Creations,
    DeclaredChildOccurrence, Delivery, DispatchBirth, EndpointAddress, EstablishChild,
    EstablishedActor, EstablishedCreation, EstablishedDelivery, EstablishedRecipient,
    ExactDeliveryReason, InterpretEstablished, LogicalDeliveryReason, MailAddr, NoBirthProtocols,
    NoBirths, NoChildren, Recipient, ResolveChildOccurrence, ResolvedChild, ResolvedChildPosition,
    RetirementBirths, RoleChild, RoleProtocol, RoutedCreation, StructuralChildOccurrence,
};
pub use effects::{
    Acted, ActionItem, ActionItemResult, ActionSettlement, ActionSettlements, Actions, AppendSend,
    Become, BehaviorSettlements, ClassifySettlement, CreationEvent, CreationSettlement,
    CreationSettlements, CreationsSettled, InterpretCreations, InterpretItem, InterpretSends,
    Interpretation, InterpreterFault, InterpreterRequest, InterpreterRequests, ItemSettlement,
    LogicalDeliveryProtocols, NoReturnToEmitter, NoSends, Own, ParentReportReason, ReportToParent,
    RetirementCreationSettlement, ReturnsToEmitter, SendEffects, SendInput, SendLayer,
    SendSettlements, SendsFor, SettledItem, SettlementStatus, SourceAction, SourceActions,
    SourceAdmission, SourceCustody, SourceSettlementCustody, SourceSettlements, settle_in_order,
    settle_item,
};
pub use next::{Never, Step, Stopped};
pub use transition::{
    ActiveTurn, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, BehaviorLayer,
    BehaviorMessage, InitializationTurn, LogicalHostRequirements, MessageProtocol, Protocol,
    delegate_transition, initialize,
};
pub use user_event::{
    ChildInputIngress, ComposedEvent, EventIngress, EventLayer, Here, Ingress, InjectEvent, Inside,
    RecoverEvent, User, UserEvent,
};

/// Generate the nominal protocol, closed effect products, and exact `Behavior`
/// wiring for an inherent impl. `addr` and `message` are required. Omitting
/// `sends`, `births`, or `error` selects the capability-free `NoSends`,
/// `NoBirths`, or `Never` type respectively.
///
/// A `sends = { lane: Product }` declaration generates `ActorSends`, one
/// distinct `ActorSendsLane` selector per field, and structural `SendEffects`,
/// `SendsFor`, [`SendSettlements`], and `InterpretSends` implementations. The
/// doc-hidden `ActorSettlements` product keeps the same semantic field names
/// and has one runtime-independent type. The macro also generates an
/// `ActorActions` extension trait with one fluent `send_lane` method per named
/// lane. Each method delegates to [`AppendSend`], changing only the send leg
/// while preserving creations and the exact next-behavior verdict. A
/// `births = { lane: Child }` declaration generates `ActorChildren` as the
/// exact closed child algebra. One child remains its direct concrete type;
/// multiple alternatives form `ChildChoice` in declaration order. The macro
/// also generates one nominal role per declaration and an `ActorChild`
/// namespace for those roles. Every role implements both [`ChildRole`] for its
/// authored parent and [`ChildOccurrence`] for sealed resolution against that
/// parent or a topology-transparent wrapper. Creation remains an authored
/// [`Creations`] or [`Children`] value and is never performed by the macro.
///
/// Invalid receivers are rejected at compile time.
/// A birth-owning generated actor must also select the exact creation-settlement
/// disposition; there is no implicit discard or generated no-op receiver.
///
/// ```compile_fail
/// struct Child;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl Child {
///     fn receive(
///         &mut self,
///         _: behavior::MailAddr,
///         message: behavior::Never,
///     ) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct MissingCreationSettlementDisposition;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = behavior::Never,
///     births = { child: Child },
/// )]
/// impl MissingCreationSettlementDisposition {
///     fn receive(
///         &mut self,
///         _: behavior::MailAddr,
///         message: behavior::Never,
///     ) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// ```
///
/// ```compile_fail
/// use behavior::{Actions, BehaviorActed, MailAddr};
///
/// struct Invalid;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = u8,
/// )]
/// impl Invalid {
///     fn init(&self) -> BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
/// ```
///
/// Missing receive methods are rejected by the macro itself:
///
/// ```compile_fail
/// use behavior::MailAddr;
/// struct Missing;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = u8,
/// )]
/// impl Missing {
/// }
/// ```
///
/// Async behavior methods cannot introduce an erased or alternate execution
/// path:
///
/// ```compile_fail
/// use behavior::{Actions, BehaviorActed, MailAddr};
/// struct Async;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = u8,
/// )]
/// impl Async {
///     async fn init(&mut self) -> BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
/// ```
///
/// Undeclared send lanes have no selector and cannot be emitted:
///
/// ```compile_fail
/// use behavior::{Actions, BehaviorActed, MailAddr, SendEffects};
/// struct Sender;
/// #[behavior::behavior(addr = MailAddr, message = (), sends = { replies: Vec<u8> })]
/// impl Sender {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         let mut sends = SenderSends::empty();
///         sends.send::<_, SenderSendsUndeclared>(1);
///         Ok(Actions::send(sends))
///     }
/// }
/// ```
///
/// Generated lane methods accept only inputs supported by that lane:
///
/// ```compile_fail
/// use behavior::{Actions, BehaviorActed, MailAddr};
/// struct Sender;
/// #[behavior::behavior(addr = MailAddr, message = (), sends = { replies: Vec<u8> })]
/// impl Sender {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         Ok(Actions::cont().send_replies("not a u8"))
///     }
/// }
/// ```
///
/// A child absent from the declared closed birth product cannot be created:
///
/// ```compile_fail
/// use behavior::{Actions, BehaviorActed, CreationSequence, Creations, CreateChild, MailAddr};
/// struct Declared;
/// #[behavior::behavior(addr = MailAddr, message = behavior::Never)]
/// impl Declared {
///     fn receive(&mut self, _: MailAddr, message: behavior::Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Other;
/// #[behavior::behavior(addr = MailAddr, message = behavior::Never)]
/// impl Other {
///     fn receive(&mut self, _: MailAddr, message: behavior::Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Root { creations: CreationSequence }
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = (),
///     births = { declared: Declared },
///     creation_settlements = retain_for_retirement,
/// )]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         let id = self.creations.issue().expect("fixture creation ID");
///         Ok(Actions::create(Creations::one(CreateChild::birth(id, Other))))
///     }
/// }
/// ```
///
/// Every generated send lane remains a separate interpreter obligation:
///
/// ```compile_fail
/// use behavior::{BehaviorActed, Delivery, InterpretItem, InterpretSends, ItemSettlement,
///     MailAddr, MessageProtocol, Never};
/// struct Root;
/// type AuditProtocol = MessageProtocol<MailAddr, u8>;
/// type MetricsProtocol = MessageProtocol<MailAddr, u16>;
/// #[behavior::behavior(addr = MailAddr, message = (), sends = {
///     audit: Vec<Delivery<AuditProtocol>>,
///     metrics: Vec<Delivery<MetricsProtocol>>,
/// })]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// struct Incomplete;
/// impl<RootEvent, Path> InterpretItem<Delivery<AuditProtocol>, RootEvent, Path> for Incomplete {
///     fn interpret_item(&mut self, _: Delivery<AuditProtocol>) -> impl core::future::Future<
///         Output = ItemSettlement<Delivery<AuditProtocol>, (), Never, Never>,
///     > + Send {
///         async { ItemSettlement::Accepted(()) }
///     }
/// }
/// fn require_complete()
/// where
///     RootSends: InterpretSends<Incomplete, behavior::User<MailAddr, ()>, behavior::Here>,
/// {}
/// ```
///
/// Generated child products likewise require one [`EstablishChild`]
/// implementation for every declared alternative. [`DispatchBirth`] owns the
/// compile-denial example so this crate overview does not duplicate it.
///
/// Two declared roles remain distinct even when they use the same behavior:
///
/// ```compile_fail
/// use behavior::{Behavior, BehaviorActed, ChildDelivery, CreationSequence, MailAddr, Never};
/// struct Worker;
/// #[behavior::behavior(addr = MailAddr, message = ())]
/// impl Worker {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// struct Root;
/// #[behavior::behavior(addr = MailAddr, message = Never, births = {
///     primary: Worker,
///     backup: Worker,
/// }, creation_settlements = retain_for_retirement)]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// fn requires_primary(
///     _: ChildDelivery<<Worker as Behavior>::Protocol, RootChildrenPrimary>,
/// ) {}
/// let mut sequence = CreationSequence::new();
/// let id = sequence.issue().expect("fixture creation ID");
/// let backup = ChildDelivery::<<Worker as Behavior>::Protocol, RootChildrenBackup>::after(id, ());
/// requires_primary(backup);
/// ```
///
/// Named topology selectors accept only the child declared for that parent
/// role, which lets an application builder remain entirely static:
///
/// ```compile_fail
/// use behavior::{Behavior, BehaviorActed, ChildRole, MailAddr, Never};
/// struct Worker;
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl Worker {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Query;
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl Query {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Root;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = Never,
///     births = { workers: Worker },
///     creation_settlements = retain_for_retirement,
/// )]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// fn child<Parent, Role>(_: Role, _: Role::Child)
/// where
///     Parent: Behavior,
///     Role: ChildRole<Parent>,
/// {}
/// child::<Root, _>(RootChild::Workers, Query);
/// ```
pub use behavior_macros::behavior;
