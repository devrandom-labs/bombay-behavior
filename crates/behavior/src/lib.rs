//! Pure, typed actor-behavior primitives.
//!
//! [`Protocol`] is stable public destination identity (`Addr` plus `Msg`). A
//! [`Behavior`] separately owns state and folds its complete [`Behavior::Event`]
//! algebra into exactly [`Actions`]: sends, fresh creations, and its next
//! behavior or termination. A protocol is not a behavior, and `Behavior` is not
//! a `Protocol` supertrait. Higher capabilities extend internal event and
//! effect algebras while transparent wrappers preserve [`Behavior::Protocol`].

// The `#[behavior]` expansion emits `::behavior::…` paths; this alias lets the
// expansion resolve inside this crate too.
extern crate self as behavior;

mod actor;
mod effect;
mod effects;
mod next;
mod reducer;
mod transition;
mod user_event;

pub use actor::{
    Address, AllocationRejection, BirthMode, BirthNodeMapper, BirthNodeProtocols, BirthProtocol,
    BirthProtocolAt, BirthProtocolHead, BirthProtocolProduct, BirthProtocolTail, BirthProtocols,
    Births, ChildChoice, ChildCons, ChildDelivery, ChildHead, ChildOccurrence,
    ChildOccurrenceResolution, ChildPosition, ChildProduct, ChildRole, ChildRoute, ChildTail,
    Children, ChildrenError, Create, CreationKind, CreationRejection, DeclaredChildOccurrence,
    Delivery, DispatchBirth, DispatchBirthAt, EndpointAddress, EstablishedActor,
    EstablishedCreation, EstablishedDelivery, EstablishedRecipient, FoldBirthNode, FoldedBirthNode,
    InstallBirth, InterpretEstablished, MailAddr, NoBirthProtocols, NoBirths, NoChildren,
    Recipient, ResolveChildOccurrence, ResolvedChild, ResolvedChildPosition, RoleChild,
    RoleProtocol, StructuralChildOccurrence,
};
pub use effect::Effect;
pub use effects::{
    Acted, Actions, AppendSend, Become, InterpretChildDelivery, InterpretDelivery,
    InterpretEstablishedDelivery, InterpretRequest, InterpretSends, InterpreterRequest,
    InterpreterRequests, NoReturnToEmitter, NoSends, Own, ReturnsToEmitter, SendEffects, SendInput,
    SendInterpreter, SendLayer, SendsFor,
};
pub use next::{Never, Step, Stopped};
pub use reducer::{ActionReducer, Effects, FoldFailure, Folded, fold_events};
pub use transition::{
    ActiveTurn, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, BehaviorLayer,
    BehaviorMessage, InitializationTurn, MessageProtocol, Protocol, delegate_transition,
    initialize,
};
pub use user_event::{
    ComposedEvent, EventLayer, Here, Ingress, InjectEvent, Inside, User, UserEvent,
};

/// Generate the nominal protocol, closed effect products, and exact `Behavior`
/// wiring for an inherent impl. `addr` and `message` are required. Omitting
/// `sends`, `births`, or `error` selects the capability-free `NoSends`,
/// `NoBirths`, or `Never` type respectively.
///
/// A `sends = { lane: Product }` declaration generates `ActorSends`, one
/// distinct `ActorSendsLane` selector per field, and structural `SendEffects`,
/// `SendsFor`, and `InterpretSends` implementations. It also generates an
/// `ActorActions` extension trait with one fluent `send_lane` method per named
/// lane. Each method delegates to [`AppendSend`], changing only the send leg
/// while preserving creations and the exact next-behavior verdict. A
/// `births = { lane: Child }` declaration generates `ActorChildren` as the
/// exact recursive `ChildChoice` produced by `Children` calls in declaration
/// order. It also generates `ActorChildrenRoutes`, containing one nominally
/// distinct [`ChildRoute`] per declared role. Every role implements both
/// [`ChildRole`] for its authored parent and [`ChildOccurrence`] for sealed
/// resolution against that parent or a topology-transparent wrapper. A route
/// is the single typed source for staging that role's creation and constructing
/// its creator-local [`ChildDelivery`]. Creation remains an authored
/// [`Children`] value and is never performed by the macro.
///
/// Invalid receivers are rejected at compile time.
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
/// use behavior::{Actions, BehaviorActed, Create, MailAddr};
/// struct Declared;
/// struct Other;
/// struct Root;
/// #[behavior::behavior(addr = MailAddr, message = (), births = { declared: Declared })]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         Ok(Actions::create(vec![Create::birth(1, Other)]))
///     }
/// }
/// ```
///
/// Every generated send lane remains a separate interpreter obligation:
///
/// ```compile_fail
/// use behavior::{BehaviorActed, Delivery, InterpretDelivery, InterpretSends, MailAddr,
///     MessageProtocol, Recipient, SendInterpreter};
/// struct Root;
/// type First = MessageProtocol<MailAddr, u8>;
/// type Second = MessageProtocol<MailAddr, u16>;
/// #[behavior::behavior(addr = MailAddr, message = (), sends = {
///     first: Vec<Delivery<First>>,
///     second: Vec<Delivery<Second>>,
/// })]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// struct Incomplete;
/// impl SendInterpreter for Incomplete { type Error = (); }
/// impl InterpretDelivery<First> for Incomplete {
///     fn interpret_delivery(&mut self, _: Delivery<First>) -> impl core::future::Future<Output = Result<(), ()>> + Send {
///         async { Ok(()) }
///     }
/// }
/// fn require_complete()
/// where
///     RootSends: InterpretSends<Incomplete, behavior::User<MailAddr, ()>, behavior::Here>,
/// {}
/// ```
///
/// Generated child products likewise require an installer for every declared
/// alternative:
///
/// ```compile_fail
/// use behavior::{BehaviorActed, Create, DispatchBirth, InstallBirth, MailAddr, Never};
/// struct First;
/// struct Second;
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl First {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> { match message {} }
/// }
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl Second {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> { match message {} }
/// }
/// struct Root;
/// #[behavior::behavior(addr = MailAddr, message = (), births = {
///     first: First,
///     second: Second,
/// })]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, _: ()) -> BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// struct Incomplete;
/// impl InstallBirth<behavior::ChildHead, First, (), Never> for Incomplete {
///     async fn install_birth(&mut self, _: Create<MailAddr, First>) -> Result<(), Never> { Ok(()) }
/// }
/// fn require_complete<T: DispatchBirth<MailAddr, Incomplete, (), Never>>() {}
/// require_complete::<RootChildren>();
/// ```
///
/// A generated child route accepts only its declared behavior:
///
/// ```compile_fail
/// use behavior::{BehaviorActed, Children, MailAddr, Never};
/// struct Declared;
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl Declared {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Other;
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl Other {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Root;
/// #[behavior::behavior(addr = MailAddr, message = Never, births = { worker: Declared })]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// let routes = RootChildrenRoutes::new(1);
/// let _ = Children::<MailAddr>::new().child_at(routes.worker, Other);
/// ```
///
/// Two declared roles remain distinct even when they use the same behavior:
///
/// ```compile_fail
/// use behavior::{BehaviorActed, ChildRoute, MailAddr, Never};
/// struct Worker;
/// #[behavior::behavior(addr = MailAddr, message = Never)]
/// impl Worker {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Root;
/// #[behavior::behavior(addr = MailAddr, message = Never, births = {
///     primary: Worker,
///     backup: Worker,
/// })]
/// impl Root {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// fn requires_primary(_: ChildRoute<Worker, RootChildrenPrimary>) {}
/// let routes = RootChildrenRoutes::new(1, 2);
/// requires_primary(routes.backup);
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
/// #[behavior::behavior(addr = MailAddr, message = Never, births = { workers: Worker })]
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
