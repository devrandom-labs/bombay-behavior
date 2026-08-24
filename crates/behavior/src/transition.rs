//! Pure behavior folds from one typed event to explicit transition actions.

use crate::actor::{Address, BirthMode};
use crate::effects::{Acted, Actions, SendEffects};
use crate::user_event::UserEvent;

/// Reusable zero-state protocol identity for messages `M` addressed by `A`.
///
/// This value has no runtime operations. It lets an actor template publish its
/// communication signature independently of the concrete state/fold type that
/// currently implements that signature.
pub struct MessageProtocol<A: Address, M>(core::marker::PhantomData<fn(A, M)>);

impl<A: Address, M> Protocol for MessageProtocol<A, M> {
    type Addr = A;
    type Msg = M;
}

/// The statically known address and message signature of an actor protocol.
///
/// This signature is deliberately independent of [`Behavior`]'s transition
/// algebra. A [`Recipient`](crate::Recipient) or [`Delivery`](crate::Delivery)
/// needs to prove only which address namespace and message type it names; it
/// must not recursively prove the destination's sends, births, phases, or
/// transition implementation. That separation permits closed static actor
/// topologies in which a root sends to an actor whose reply path returns to the
/// same root.
pub trait Protocol {
    type Addr: Address;
    type Msg;
}

/// The only successful effect shape admitted by a [`Behavior`] implementation.
pub type BehaviorActed<B> = Acted<
    BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
    <B as Behavior>::Error,
>;

/// Address namespace projected from a behavior's stable public protocol.
pub type BehaviorAddr<B> = <<B as Behavior>::Protocol as Protocol>::Addr;

/// Public message algebra projected from a behavior's stable protocol.
pub type BehaviorMessage<B> = <<B as Behavior>::Protocol as Protocol>::Msg;

/// Capability for defining one initialization fold.
///
/// The constructor is private: only the lifecycle boundary can issue this
/// capability, exactly once for an owned behavior value.
pub struct InitializationTurn {
    #[allow(dead_code, reason = "private field prevents external construction")]
    private: (),
}

impl InitializationTurn {
    pub(crate) const fn new() -> Self {
        Self { private: () }
    }
}

/// Capability for defining one active mailbox fold.
///
/// Values are issued only by the lifecycle and wrapper-composition boundaries.
pub struct ActiveTurn {
    #[allow(dead_code, reason = "private field prevents external construction")]
    private: (),
}

impl ActiveTurn {
    pub(crate) const fn new() -> Self {
        Self { private: () }
    }
}

/// A composed pure behavior. `Event` is the complete accepted input algebra;
/// the separately declared [`Protocol`] is stable public destination identity,
/// and every successful transition returns the declared [`Actions`] value.
///
/// The public protocol must exactly match the address and user-message lane:
///
/// ```compile_fail
/// use behavior::{Actions, ActiveTurn, Behavior, BehaviorActed, MailAddr, MessageProtocol,
///     Never, NoBirths, Protocol, User};
/// struct Wrong;
/// impl Protocol for Wrong {
///     type Addr = MailAddr;
///     type Msg = String;
/// }
/// struct Counter;
/// impl Protocol for Counter {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// impl Behavior for Counter {
///     type Protocol = Wrong;
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
/// ```
pub trait Behavior {
    /// Stable public communication identity owned by this actor template.
    ///
    /// Behavior wrappers must preserve this type unless their documented
    /// purpose is to adapt the public message protocol. The equality bound
    /// prevents a behavior from consuming a different address or user-message
    /// signature than the protocol established for its actor identity.
    type Protocol: Protocol;

    type Event: UserEvent<Addr = BehaviorAddr<Self>, Message = BehaviorMessage<Self>>;
    type Sends: SendEffects + crate::SendsFor<Self::Event>;
    type Ph;
    type Error;
    type Birth: BirthMode;

    /// Produce initialization actions before the first event is accepted.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled initialization failure.
    fn init(&mut self, _turn: InitializationTurn) -> BehaviorActed<Self>
    where
        Self: Sized,
    {
        Ok(Actions::cont())
    }

    /// Fold exactly one event into explicit actions and the next behavior.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    fn transition(&mut self, _turn: ActiveTurn, event: Self::Event) -> BehaviorActed<Self>;

    /// Apply one statically dispatched construction layer.
    ///
    /// The returned concrete behavior is inferred from `layer`; no trait
    /// object or erased behavior is introduced. Construction performs no
    /// actor effect and does not run either initialization or a transition.
    #[must_use]
    fn layer<L>(self, layer: L) -> L::Output
    where
        Self: Sized,
        L: BehaviorLayer<Self>,
    {
        BehaviorLayer::layer(layer, self)
    }
}

/// Static construction from one concrete behavior to another.
///
/// This is Bombay's generic consumer contract for Tower-like behavior
/// composition. `Output` remains a fully concrete [`Behavior`], so its public
/// protocol, complete event sum, named send product, birth algebra,
/// initialization fold, phase, error, and next-behavior decision all remain
/// available through ordinary associated types. A layer value performs no
/// send, creation, initialization, transition, or runtime lookup; it only owns
/// the information needed to construct its output.
///
/// The output remains in the input behavior's address namespace. It may
/// preserve or deliberately adapt the public protocol, event, sends, births,
/// phase, and error only as documented by the concrete output behavior. This
/// trait does not assert that an arbitrary closure is topology-transparent;
/// that semantic law belongs to the concrete transformation and its tests.
///
/// Closures implement this contract directly, allowing existing concrete
/// wrapper constructors to compose without a parallel catalogue of marker or
/// configuration-only `*Layer` types:
///
/// ```
/// use behavior::{Actions, Behavior, BehaviorActed, BehaviorLayer, MailAddr,
///     Never, NoBirths, Protocol, User};
///
/// struct Inner;
/// impl Protocol for Inner { type Addr = MailAddr; type Msg = (); }
/// impl Behavior for Inner {
///     type Protocol = Self;
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event)
///         -> BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
///
/// fn apply<B, L>(behavior: B, layer: L) -> L::Output
/// where
///     B: Behavior,
///     L: BehaviorLayer<B>,
/// {
///     layer.layer(behavior)
/// }
///
/// let _: Inner = apply(Inner, core::convert::identity::<Inner>);
/// ```
pub trait BehaviorLayer<B: Behavior>
where
    <Self::Output as Behavior>::Protocol: Protocol<Addr = BehaviorAddr<B>>,
{
    /// Fully concrete behavior constructed by this layer.
    type Output: Behavior;

    /// Consume the layer and inner behavior into the concrete composition.
    #[must_use]
    fn layer(self, inner: B) -> Self::Output;
}

impl<B, F, Output> BehaviorLayer<B> for F
where
    B: Behavior,
    F: FnOnce(B) -> Output,
    Output: Behavior,
    Output::Protocol: Protocol<Addr = BehaviorAddr<B>>,
{
    type Output = Output;

    fn layer(self, inner: B) -> Self::Output {
        self(inner)
    }
}

/// Static projection from a composed behavior to its authored base behavior.
///
/// Wrappers preserve this associated type, so inspection never depends on
/// wrapper nesting depth or a positional path.
pub trait BehaviorBase {
    type Base;

    fn base(&self) -> &Self::Base;
}

/// Initialize an inner behavior owned by a semantic composition.
///
/// This is Bombay's derived, canonical boundary for wrapper composition; it is
/// not an additional actor-model operation. It invokes the inner initialization
/// fold exactly once and returns its complete typed action value without
/// inspecting or transforming it. It does not execute a runtime turn,
/// interpret effects, or provide an alternate actor executor; top-level runtime
/// transitions remain the responsibility of the runtime's machine adapter.
///
/// # Errors
///
/// Returns the inner behavior's controlled transition failure unchanged.
#[doc(hidden)]
pub fn initialize<B: Behavior>(behavior: &mut B) -> BehaviorActed<B> {
    B::init(behavior, InitializationTurn::new())
}

/// Fold one event through an inner behavior owned by a semantic wrapper.
///
/// This invokes the inner deterministic fold exactly once and returns its
/// complete typed action value without interpreting it.
///
/// # Errors
///
/// Returns the inner behavior's controlled transition failure unchanged.
#[doc(hidden)]
pub fn delegate_transition<B: Behavior>(behavior: &mut B, event: B::Event) -> BehaviorActed<B> {
    B::transition(behavior, ActiveTurn::new(), event)
}
