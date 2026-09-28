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
//! ```compile_fail,E0405
//! fn requires_reducer<T: behavior::ActionReducer>() {}
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
///
/// struct Invalid;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = u8,
/// )]
/// impl Invalid {
///     fn init(&self) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
///     fn receive(&mut self, _: behavior::MailAddr, _: u8) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// ```
///
/// Missing receive methods are rejected by the macro itself:
///
/// ```compile_fail
/// struct Missing;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
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
/// struct Async;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = u8,
/// )]
/// impl Async {
///     async fn init(&mut self) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
///     fn receive(&mut self, _: behavior::MailAddr, _: u8) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// ```
///
/// Undeclared send lanes have no selector and cannot be emitted:
///
/// ```compile_fail
/// struct Sender;
/// #[behavior::behavior(addr = behavior::MailAddr, message = (), sends = { replies: Vec<u8> })]
/// impl Sender {
///     fn receive(&mut self, _: behavior::MailAddr, _: ()) -> behavior::BehaviorActed<Self> {
///         let mut sends: SenderSends = behavior::SendEffects::empty();
///         behavior::SendEffects::send::<_, SenderSendsUndeclared>(&mut sends, 1);
///         Ok(behavior::Actions::send(sends))
///     }
/// }
/// ```
///
/// Generated lane methods accept only inputs supported by that lane:
///
/// ```compile_fail
/// struct Sender;
/// #[behavior::behavior(addr = behavior::MailAddr, message = (), sends = { replies: Vec<u8> })]
/// impl Sender {
///     fn receive(&mut self, _: behavior::MailAddr, _: ()) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont().send_replies("not a u8"))
///     }
/// }
/// ```
///
/// A child absent from the declared closed birth product cannot be created:
///
/// ```compile_fail
/// struct Declared;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl Declared {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Other;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl Other {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Root { creations: behavior::CreationSequence }
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = (),
///     births = { declared: Declared },
///     creation_settlements = retain_for_retirement,
/// )]
/// impl Root {
///     fn receive(&mut self, _: behavior::MailAddr, _: ()) -> behavior::BehaviorActed<Self> {
///         let id = self.creations.issue().expect("fixture creation ID");
///         Ok(behavior::Actions::create(behavior::Creations::one(behavior::CreateChild::birth(id, Other))))
///     }
/// }
/// ```
///
/// Every generated send lane remains a separate interpreter obligation:
///
/// ```compile_fail
/// struct Root;
/// type AuditProtocol = behavior::MessageProtocol<behavior::MailAddr, u8>;
/// type MetricsProtocol = behavior::MessageProtocol<behavior::MailAddr, u16>;
/// #[behavior::behavior(addr = behavior::MailAddr, message = (), sends = {
///     audit: Vec<behavior::Delivery<AuditProtocol>>,
///     metrics: Vec<behavior::Delivery<MetricsProtocol>>,
/// })]
/// impl Root {
///     fn receive(&mut self, _: behavior::MailAddr, _: ()) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// struct Incomplete;
/// impl<RootEvent, Path> behavior::InterpretItem<behavior::Delivery<AuditProtocol>, RootEvent, Path> for Incomplete {
///     fn interpret_item(&mut self, _: behavior::Delivery<AuditProtocol>) -> impl core::future::Future<
///         Output = behavior::ItemSettlement<behavior::Delivery<AuditProtocol>, (), behavior::Never, behavior::Never>,
///     > + Send {
///         async { behavior::ItemSettlement::Accepted(()) }
///     }
/// }
/// fn require_complete()
/// where
///     RootSends: behavior::InterpretSends<Incomplete, behavior::User<behavior::MailAddr, ()>, behavior::Here>,
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
/// struct Worker;
/// #[behavior::behavior(addr = behavior::MailAddr, message = ())]
/// impl Worker {
///     fn receive(&mut self, _: behavior::MailAddr, _: ()) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// struct Root;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never, births = {
///     primary: Worker,
///     backup: Worker,
/// }, creation_settlements = retain_for_retirement)]
/// impl Root {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// fn requires_primary(
///     _: behavior::ChildDelivery<<Worker as behavior::Behavior>::Protocol, RootChildrenPrimary>,
/// ) {}
/// let mut sequence = behavior::CreationSequence::new();
/// let id = sequence.issue().expect("fixture creation ID");
/// let backup = behavior::ChildDelivery::<<Worker as behavior::Behavior>::Protocol, RootChildrenBackup>::after(id, ());
/// requires_primary(backup);
/// ```
///
/// Named topology selectors accept only the child declared for that parent
/// role, which lets an application builder remain entirely static:
///
/// ```compile_fail
/// struct Worker;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl Worker {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Query;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl Query {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Root;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = behavior::Never,
///     births = { workers: Worker },
///     creation_settlements = retain_for_retirement,
/// )]
/// impl Root {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// fn child<Parent, Role>(_: Role, _: Role::Child)
/// where
///     Parent: behavior::Behavior,
///     Role: behavior::ChildRole<Parent>,
/// {}
/// child::<Root, _>(RootChild::Workers, Query);
/// ```
pub use behavior_macros::behavior;
