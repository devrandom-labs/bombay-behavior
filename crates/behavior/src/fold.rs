//! Pure behavior folds from one typed event to explicit transition actions.

use core::future::Future;
use core::marker::PhantomData;

use crate::addressing::{Address, Delivery};
use crate::creation::{BirthMode, NoBirths};
use crate::sending::SendAlgebra;
use crate::transition::{Acted, Actions};
use crate::user_event::{User, UserEvent};
use crate::verdict::Never;

pub type StateActed<A, Out, Birth, Err> = Acted<A, Never, Vec<Delivery<A, Out>>, Birth, Err>;

/// The only successful effect shape admitted by a [`Behavior`] implementation.
pub type BehaviorActed<B> = Acted<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
    <B as Behavior>::Error,
>;

pub trait State<Out = Never, Birth = NoBirths, Err = Never>
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
    fn handle(
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

    fn init(&mut self) -> impl Future<Output = BehaviorActed<Self>> + Send;
    fn step(&mut self, event: Self::Event) -> impl Future<Output = BehaviorActed<Self>> + Send;
}

pub struct Base<S: State<O, Br, E>, O = Never, Br: BirthMode = NoBirths, E = Never> {
    state: S,
    marker: PhantomData<fn(O, Br, E)>,
}

impl<S: State<O, Br, E>, O, Br: BirthMode, E> Base<S, O, Br, E> {
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

pub type Transition<S, A, M, O, Br, E> =
    fn(&mut S, A, M) -> Acted<A, Never, Vec<Delivery<A, O>>, Br, E>;

pub struct FnState<S, A: Address, M, O = Never, Br: BirthMode = NoBirths, E = Never> {
    pub state: S,
    pub handle: Transition<S, A, M, O, Br, E>,
}

impl<S, A: Address, M, O, Br: BirthMode, E> State<O, Br, E> for FnState<S, A, M, O, Br, E> {
    type Addr = A;
    type Msg = M;

    fn handle(&mut self, from: A, message: M) -> Acted<A, Never, Vec<Delivery<A, O>>, Br, E> {
        (self.handle)(&mut self.state, from, message)
    }
}

impl<S, A: Address, M, O, Br: BirthMode, E> Base<FnState<S, A, M, O, Br, E>, O, Br, E> {
    #[must_use]
    pub fn from_fn(state: S, handle: Transition<S, A, M, O, Br, E>) -> Self {
        Self::new(FnState { state, handle })
    }
}

impl<S, O, Br, E> Behavior for Base<S, O, Br, E>
where
    S: State<O, Br, E> + Send,
    S::Addr: Send,
    S::Msg: Send,
    Br: BirthMode,
    Br::Child: Send,
    E: Send,
{
    type Addr = S::Addr;
    type Msg = S::Msg;
    type Event = User<S::Addr, S::Msg>;
    type Sends = Vec<Delivery<S::Addr, O>>;
    type Ph = Never;
    type Error = E;
    type Birth = Br;

    async fn init(&mut self) -> StateActed<S::Addr, O, Br, E> {
        Ok(Actions::cont())
    }

    async fn step(&mut self, event: Self::Event) -> StateActed<S::Addr, O, Br, E> {
        self.state.handle(event.from, event.message)
    }
}
