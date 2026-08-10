//! Pure behavior folds from one typed event to explicit transition actions.

use core::marker::PhantomData;

use super::user_event::{EventInput, User, UserEvent};
use crate::actor::{Address, BirthMode, Delivery, NoBirths};
use crate::effects::{Acted, Actions, SendAlgebra};
use crate::next::Never;

pub type StateActed<A, Out, Birth, Err> = Acted<A, Never, Vec<Delivery<A, Out>>, Birth, Err>;

/// The only successful effect shape admitted by a [`Behavior`] implementation.
pub type BehaviorActed<B> = Acted<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
    <B as Behavior>::Error,
>;

pub trait Handler<Out = Never, Birth = NoBirths, Err = Never>
where
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
    ) -> StateActed<Self::Addr, Out, Birth, Err>;
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
    fn init(&mut self) -> BehaviorActed<Self>;

    /// Fold exactly one event into explicit actions and the next behavior.
    fn transition(&mut self, event: Self::Event) -> BehaviorActed<Self>;

    /// Fold one user communication through the composed protocol.
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
    fn on<Input>(&mut self, input: Input) -> BehaviorActed<Self>
    where
        Self: Sized,
        Self::Event: EventInput<Input>,
    {
        self.transition(Self::Event::inject(input))
    }
}

pub struct Pure<S: Handler<O, Br, E>, O = Never, Br: BirthMode = NoBirths, E = Never> {
    state: S,
    marker: PhantomData<fn(O, Br, E)>,
}

impl<S: Handler<O, Br, E>, O, Br: BirthMode, E> Pure<S, O, Br, E> {
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
    O = Never,
    Br: BirthMode = NoBirths,
    E = Never,
    F = fn(&mut S, A, M) -> Acted<A, Never, Vec<Delivery<A, O>>, Br, E>,
> {
    pub state: S,
    pub transition: F,
    marker: PhantomData<fn(A, M, O, Br, E)>,
}

impl<S, F, A: Address, M, O, Br: BirthMode, E> Handler<O, Br, E> for FoldFn<S, A, M, O, Br, E, F>
where
    F: FnMut(&mut S, A, M) -> Acted<A, Never, Vec<Delivery<A, O>>, Br, E>,
{
    type Addr = A;
    type Msg = M;

    fn receive(&mut self, from: A, message: M) -> Acted<A, Never, Vec<Delivery<A, O>>, Br, E> {
        (self.transition)(&mut self.state, from, message)
    }
}

impl<S, F, A: Address, M, O, Br: BirthMode, E> Pure<FoldFn<S, A, M, O, Br, E, F>, O, Br, E>
where
    F: FnMut(&mut S, A, M) -> Acted<A, Never, Vec<Delivery<A, O>>, Br, E>,
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

impl<S, O, Br, E> Behavior for Pure<S, O, Br, E>
where
    S: Handler<O, Br, E>,
    Br: BirthMode,
{
    type Addr = S::Addr;
    type Msg = S::Msg;
    type Event = User<S::Addr, S::Msg>;
    type Sends = Vec<Delivery<S::Addr, O>>;
    type Ph = Never;
    type Error = E;
    type Birth = Br;

    fn init(&mut self) -> StateActed<S::Addr, O, Br, E> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, event: Self::Event) -> StateActed<S::Addr, O, Br, E> {
        self.state.receive(event.from, event.message)
    }
}
