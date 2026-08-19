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
    Address, BirthMode, Births, ChildChoice, ChildCons, ChildProduct, ChildRecipient, Children,
    ChildrenError, Create, CreationKind, Delivery, DeliveryTarget, DispatchBirth, InstallBirth,
    MailAddr, NoBirths, NoChildren, Recipient,
};
pub use effect::Effect;
pub use effects::{
    Acted, Actions, Become, InterpretDelivery, InterpretRequest, InterpretSends,
    InterpreterRequest, InterpreterRequests, NoReturnToEmitter, NoSends, Own, ReturnsToEmitter,
    SendEffects, SendInput, SendInterpreter, SendLayer, SendsFor,
};
pub use next::{Never, Step, Stopped};
pub use reducer::{ActionReducer, Effects, FoldFailure, Folded, fold_events};
pub use transition::{
    ActiveTurn, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, BehaviorMessage,
    InitializationTurn, MessageProtocol, Protocol, delegate_transition, initialize,
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
/// `SendsFor`, and `InterpretSends` implementations. A
/// `births = { lane: Child }` declaration generates `ActorChildren` as the
/// exact recursive `ChildChoice` produced by `Children` calls in declaration
/// order. The lane labels document each child role; creation remains an
/// authored `Children` value and is never performed by the macro.
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
/// impl InstallBirth<MailAddr, First, (), Never> for Incomplete {
///     async fn install_birth(&mut self, _: Create<MailAddr, First>) -> Result<(), Never> { Ok(()) }
/// }
/// fn require_complete<T: DispatchBirth<MailAddr, Incomplete, (), Never>>() {}
/// require_complete::<RootChildren>();
/// ```
pub use behavior_macros::behavior;
