//! Pure behavior folds from one typed event to explicit transition actions.

use core::marker::PhantomData;

use super::user_event::{EventInput, User, UserEvent};
use crate::actor::{Address, BirthMode, NoBirths};
use crate::effects::{Acted, Actions, SendAlgebra};
use crate::next::Never;

pub type StateActed<A, Sends, Birth, Err> = Acted<A, Never, Sends, Birth, Err>;

/// The only successful effect shape admitted by a [`Behavior`] implementation.
pub type BehaviorActed<B> = Acted<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
    <B as Behavior>::Error,
>;

pub trait Handler<Sends = Vec<Never>, Birth = NoBirths, Err = Never>
where
    Sends: SendAlgebra,
    Birth: BirthMode,
{
    type Addr: Address;
    type Msg;

    /// Fold a user message into Bombay's typed actor transition effects.
    ///
    /// # Errors
    /// Returns the state's declared controlled failure.
    #[allow(
        clippy::type_complexity,
        reason = "the alias exposes all state protocol seats"
    )]
    fn receive(
        &mut self,
        from: Self::Addr,
        message: Self::Msg,
    ) -> StateActed<Self::Addr, Sends, Birth, Err>;
}

/// A composed pure behavior. `Event` is the complete accepted protocol;
/// every successful transition returns the declared [`Actions`] value.
pub trait Behavior {
    type Addr: Address;
    type Msg;
    type Event: UserEvent<Addr = Self::Addr, Message = Self::Msg>;
    type Sends: SendAlgebra;
    type Ph;
    type Error;
    type Birth: BirthMode;

    /// Produce initialization actions before the first event is accepted.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled initialization failure.
    fn init(&mut self) -> BehaviorActed<Self>;

    /// Fold exactly one event into explicit actions and the next behavior.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    fn transition(&mut self, event: Self::Event) -> BehaviorActed<Self>;

    /// Fold one user communication through the composed protocol.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    fn receive(&mut self, from: Self::Addr, message: Self::Msg) -> BehaviorActed<Self>
    where
        Self: Sized,
    {
        self.transition(Self::Event::user(from, message))
    }

    /// Inject one supported semantic input and fold it through this behavior.
    ///
    /// This method exists only when the concrete composed protocol proves that
    /// it contains `Input`; unsupported lanes therefore fail to compile.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    fn on<Input>(&mut self, input: Input) -> BehaviorActed<Self>
    where
        Self: Sized,
        Self::Event: EventInput<Input>,
    {
        self.transition(Self::Event::inject(input))
    }
}

/// Fold one event through an inner behavior owned by a semantic wrapper.
///
/// This is Bombay's derived, canonical boundary for wrapper composition; it is
/// not an additional actor-model operation. It invokes the inner deterministic
/// fold exactly once and returns its complete typed action value without
/// inspecting or transforming it. It does not execute a runtime turn,
/// interpret effects, or provide an alternate actor executor; top-level runtime
/// transitions remain the responsibility of the runtime's machine adapter.
///
/// # Errors
///
/// Returns the inner behavior's controlled transition failure unchanged.
pub fn delegate_transition<B: Behavior>(behavior: &mut B, event: B::Event) -> BehaviorActed<B> {
    behavior.transition(event)
}

pub struct Pure<S, Sends = Vec<Never>, Br: BirthMode = NoBirths, E = Never>
where
    S: Handler<Sends, Br, E>,
    Sends: SendAlgebra,
{
    state: S,
    marker: PhantomData<fn(Sends, Br, E)>,
}

impl<S, Sends, Br, E> Pure<S, Sends, Br, E>
where
    S: Handler<Sends, Br, E>,
    Sends: SendAlgebra,
    Br: BirthMode,
{
    #[must_use]
    pub fn new(state: S) -> Self {
        Self {
            state,
            marker: PhantomData,
        }
    }
    #[must_use]
    pub fn state(&self) -> &S {
        &self.state
    }
}

pub struct FoldFn<
    S,
    A: Address,
    M,
    Sends = Vec<Never>,
    Br: BirthMode = NoBirths,
    E = Never,
    F = fn(&mut S, A, M) -> Acted<A, Never, Sends, Br, E>,
> {
    pub state: S,
    pub transition: F,
    #[allow(
        clippy::type_complexity,
        reason = "the marker retains the complete inferred behavior signature"
    )]
    marker: PhantomData<fn(A, M, Sends, Br, E)>,
}

/// A concrete behavior defined by initialization and user-event folds.
///
/// Each function is invoked exactly once for its corresponding input and its
/// returned [`Actions`] value is preserved unchanged. This adapter adds no
/// event routing or effect interpretation; those remain the responsibility of
/// concrete behavior wrappers and the runtime interpreter.
pub struct BehaviorFn<S, A: Address, M, Sends, Br: BirthMode, E, I, F> {
    state: S,
    initialize: I,
    transition: F,
    #[allow(
        clippy::type_complexity,
        reason = "the marker retains the complete inferred behavior signature"
    )]
    marker: PhantomData<fn(A, M, Sends, Br, E)>,
}

impl<S, A: Address, M, Sends, Br: BirthMode, E, I, F> BehaviorFn<S, A, M, Sends, Br, E, I, F>
where
    Sends: SendAlgebra,
    I: FnMut(&mut S) -> Acted<A, Never, Sends, Br, E>,
    F: FnMut(&mut S, A, M) -> Acted<A, Never, Sends, Br, E>,
{
    #[must_use]
    pub fn new(state: S, initialize: I, transition: F) -> Self {
        Self {
            state,
            initialize,
            transition,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S, A: Address, M, Sends, Br: BirthMode, E, I, F> Behavior
    for BehaviorFn<S, A, M, Sends, Br, E, I, F>
where
    Sends: SendAlgebra,
    I: FnMut(&mut S) -> Acted<A, Never, Sends, Br, E>,
    F: FnMut(&mut S, A, M) -> Acted<A, Never, Sends, Br, E>,
{
    type Addr = A;
    type Msg = M;
    type Event = User<A, M>;
    type Sends = Sends;
    type Ph = Never;
    type Error = E;
    type Birth = Br;

    fn init(&mut self) -> BehaviorActed<Self> {
        (self.initialize)(&mut self.state)
    }

    fn transition(&mut self, event: Self::Event) -> BehaviorActed<Self> {
        (self.transition)(&mut self.state, event.from, event.message)
    }
}

impl<S, F, A: Address, M, Sends, Br: BirthMode, E> Handler<Sends, Br, E>
    for FoldFn<S, A, M, Sends, Br, E, F>
where
    Sends: SendAlgebra,
    F: FnMut(&mut S, A, M) -> Acted<A, Never, Sends, Br, E>,
{
    type Addr = A;
    type Msg = M;

    fn receive(&mut self, from: A, message: M) -> Acted<A, Never, Sends, Br, E> {
        (self.transition)(&mut self.state, from, message)
    }
}

impl<S, F, A: Address, M, Sends, Br: BirthMode, E>
    Pure<FoldFn<S, A, M, Sends, Br, E, F>, Sends, Br, E>
where
    Sends: SendAlgebra,
    F: FnMut(&mut S, A, M) -> Acted<A, Never, Sends, Br, E>,
{
    #[must_use]
    pub fn from_fn(state: S, transition: F) -> Self {
        Self::new(FoldFn {
            state,
            transition,
            marker: PhantomData,
        })
    }
}

impl<S, Sends, Br, E> Behavior for Pure<S, Sends, Br, E>
where
    S: Handler<Sends, Br, E>,
    Sends: SendAlgebra,
    Br: BirthMode,
{
    type Addr = S::Addr;
    type Msg = S::Msg;
    type Event = User<S::Addr, S::Msg>;
    type Sends = Sends;
    type Ph = Never;
    type Error = E;
    type Birth = Br;

    fn init(&mut self) -> StateActed<S::Addr, Sends, Br, E> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, event: Self::Event) -> StateActed<S::Addr, Sends, Br, E> {
        self.state.receive(event.from, event.message)
    }
}
