//! The pure actor algebra: receive one event, then send, create, and become.

use core::future::Future;
use core::marker::PhantomData;

use communication::{Consumer, Received};

use crate::Exit;
use crate::deadlined::{TimeEvent, TimeReached};
use crate::supervising::{ChildEvent, ChildStopped};
use crate::verdict::{Never, Step};
use crate::watching::{PeerEvent, PeerStopped};

/// A pure actor-address namespace.
pub trait Address: Copy + Eq {
    type Nonce: Copy + Eq;

    #[must_use]
    fn birth(self, nonce: Self::Nonce) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailAddr(pub u64);

impl Address for MailAddr {
    type Nonce = u64;

    fn birth(self, nonce: u64) -> Self {
        Self(self.0 ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
}

/// An address expression for ordinary actor delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route<A: Address> {
    Global(A),
    Child(A::Nonce),
}

/// A recipient statically coupled to the message it accepts.
pub struct Recipient<A: Address, M> {
    route: Route<A>,
    message: PhantomData<fn(M)>,
}

impl<A: Address, M> Copy for Recipient<A, M> {}

impl<A: Address, M> Clone for Recipient<A, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Address, M> Recipient<A, M> {
    #[must_use]
    pub fn global(address: A) -> Self {
        Self::from_route(Route::Global(address))
    }

    #[must_use]
    pub fn child(nonce: A::Nonce) -> Self {
        Self::from_route(Route::Child(nonce))
    }

    #[must_use]
    pub fn route(self) -> Route<A> {
        self.route
    }

    const fn from_route(route: Route<A>) -> Self {
        Self {
            route,
            message: PhantomData,
        }
    }
}

/// One statically typed send operation.
#[derive(Clone, PartialEq, Eq)]
pub struct Delivery<A: Address, M> {
    pub to: Recipient<A, M>,
    pub message: M,
}

impl<A: Address, M> Delivery<A, M> {
    #[must_use]
    pub fn new(to: Recipient<A, M>, message: M) -> Self {
        Self { to, message }
    }
}

impl<A: Address, M> PartialEq for Recipient<A, M> {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

impl<A: Address, M> Eq for Recipient<A, M> {}

impl<A: Address + core::fmt::Debug, M> core::fmt::Debug for Recipient<A, M>
where
    A::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.route.fmt(f)
    }
}

/// A product of independently typed send protocols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendProduct<L, R> {
    pub inner: L,
    pub own: R,
}

/// The monoid required to accumulate sends across transitions.
pub trait SendAlgebra: Sized {
    fn empty() -> Self;
    fn append(&mut self, other: Self);
}

impl<T> SendAlgebra for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }

    fn append(&mut self, mut other: Self) {
        Vec::append(self, &mut other);
    }
}

impl<L: SendAlgebra, R: SendAlgebra> SendAlgebra for SendProduct<L, R> {
    fn empty() -> Self {
        Self {
            inner: L::empty(),
            own: R::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.inner.append(other.inner);
        self.own.append(other.own);
    }
}

/// Requests interpreted by the runtime local to the emitting actor.
///
/// Unlike [`Delivery`], a service request has no actor address. Its recipient
/// is definitionally the interpreter of the actor whose transition emitted
/// it. This distinct algebra lets interpreters route ordinary deliveries and
/// local services with disjoint static implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSends<M> {
    requests: Vec<M>,
}

impl<M> ServiceSends<M> {
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

impl<M> core::ops::Index<usize> for ServiceSends<M> {
    type Output = M;

    fn index(&self, index: usize) -> &Self::Output {
        &self.requests[index]
    }
}

impl<M> IntoIterator for ServiceSends<M> {
    type Item = M;
    type IntoIter = std::vec::IntoIter<M>;

    fn into_iter(self) -> Self::IntoIter {
        self.requests.into_iter()
    }
}

impl<M> SendAlgebra for ServiceSends<M> {
    fn empty() -> Self {
        Self::new(Vec::new())
    }

    fn append(&mut self, mut other: Self) {
        self.requests.append(&mut other.requests);
    }
}

/// Fresh actor creation. Replacement at an existing address is deliberately
/// absent; stable restart is derived with a proxy actor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Create<A: Address, New> {
    pub nonce: A::Nonce,
    pub child: New,
}

/// A type-level description of the creation leg of the actor algebra.
pub trait BirthMode {
    type Child;
}

/// This behavior cannot emit child births.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoBirths;

impl BirthMode for NoBirths {
    type Child = Never;
}

/// This behavior may emit births of `C`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Births<C>(PhantomData<fn() -> C>);

impl<C> BirthMode for Births<C> {
    type Child = C;
}

pub type Become<A, Ph = Never> = Step<Ph, Exit<A>>;

/// Exactly Agha's effect triple, with a Bombay interpretation-order policy.
///
/// An interpreter installs every fresh actor in `creates` before interpreting
/// any ordinary delivery or [`ServiceSends`] request in `sends` from this
/// value. This makes actors created by a transition available to that
/// transition's deliveries and local observation requests. Creation order is
/// vector order, and each concrete send lane retains its own order; this
/// contract does not impose an order between independent lanes of a
/// [`SendProduct`].
///
/// The ordering rule belongs to the interpreter boundary. Constructing an
/// `Actions` value remains pure and performs none of its effects.
pub struct Actions<A: Address, Ph, Sends, Birth: BirthMode> {
    pub sends: Sends,
    pub creates: Vec<Create<A, Birth::Child>>,
    pub become_: Become<A, Ph>,
}

impl<A: Address, Ph, Sends: SendAlgebra, Birth: BirthMode> Actions<A, Ph, Sends, Birth> {
    #[must_use]
    pub fn just(become_: Become<A, Ph>) -> Self {
        Self {
            sends: Sends::empty(),
            creates: Vec::new(),
            become_,
        }
    }

    #[must_use]
    pub fn cont() -> Self {
        Self::just(Step::Continue)
    }

    #[must_use]
    pub fn stop(exit: Exit<A>) -> Self {
        Self::just(Step::Stop(exit))
    }

    #[must_use]
    pub fn goto(phase: Ph) -> Self {
        Self::just(Step::Goto(phase))
    }
}

pub type Acted<A, Ph, Sends, Birth, E> = Result<Actions<A, Ph, Sends, Birth>, E>;

/// The user-message event at the Agha floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User<A, M> {
    pub from: A,
    pub message: M,
}

/// Construction/extraction of the user lane through a composed event type.
pub trait UserEvent: Sized {
    type Addr: Address;
    type Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self;
    /// Extract the user lane.
    ///
    /// # Errors
    /// Returns the unchanged event when it belongs to another composed lane.
    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self>;
}

pub type StateActed<A, Out, Birth, Err> = Acted<A, Never, Vec<Delivery<A, Out>>, Birth, Err>;

pub trait State<Out = Never, Birth = NoBirths, Err = Never>
where
    Birth: BirthMode,
{
    type Addr: Address;
    type Msg;

    /// Fold a user message into the Agha triple.
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
/// successful transitions always return the same Agha effect algebra.
pub trait Behavior {
    type Addr: Address;
    type Msg;
    type Event: UserEvent<Addr = Self::Addr, Message = Self::Msg>;
    type Sends: SendAlgebra;
    type Ph;
    type Error;
    type Birth: BirthMode;
    type Effect;
    type Done;

    fn init(&mut self) -> impl Future<Output = Result<Self::Effect, Self::Error>> + Send;

    fn step(
        &mut self,
        event: Self::Event,
    ) -> impl Future<Output = Result<Self::Effect, Self::Error>> + Send;
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

impl<A: Address, M> UserEvent for User<A, M> {
    type Addr = A;
    type Message = M;

    fn user(from: A, message: M) -> Self {
        Self { from, message }
    }

    fn into_user(self) -> Result<Self, Self> {
        Ok(self)
    }
}

impl<A: Address, M> TimeEvent for User<A, M> {
    fn time_reached(_: TimeReached) -> Option<Self> {
        None
    }
}

impl<A: Address, M> PeerEvent<A> for User<A, M> {
    fn peer_stopped(_: PeerStopped<A>) -> Option<Self> {
        None
    }
}

impl<A: Address, M> ChildEvent<A> for User<A, M> {
    fn child_stopped(_: ChildStopped<A>) -> Option<Self> {
        None
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
    type Effect = Actions<S::Addr, Never, Self::Sends, Br>;
    type Done = Exit<S::Addr>;

    async fn init(&mut self) -> Result<Self::Effect, E> {
        Ok(Actions::cont())
    }

    async fn step(&mut self, event: Self::Event) -> Result<Self::Effect, E> {
        self.state.handle(event.from, event.message)
    }
}

pub struct Transcript<A: Address, Sends, New> {
    pub sends: Sends,
    pub creates: Vec<Create<A, New>>,
    pub exit: Exit<A>,
}

/// Drive user-lane messages through a complete behavior protocol.
///
/// # Errors
/// Returns the first controlled behavior failure.
pub async fn run<B, C, A, Sends, Br>(
    mut behavior: B,
    mut mailbox: Consumer<C, B::Msg>,
    from: A,
) -> Result<Transcript<A, Sends, Br::Child>, B::Error>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<
            Addr = A,
            Ph = Never,
            Sends = Sends,
            Birth = Br,
            Effect = Actions<A, Never, Sends, Br>,
            Done = Exit<A>,
        >,
{
    let mut sends = Sends::empty();
    let mut creates = Vec::new();
    let initial = behavior.init().await?;
    sends.append(initial.sends);
    creates.extend(initial.creates);
    match initial.become_ {
        Step::Continue => {}
        Step::Goto(never) => match never {},
        Step::Stop(exit) => {
            return Ok(Transcript {
                sends,
                creates,
                exit,
            });
        }
    }
    while let Some(received) = mailbox.recv().await {
        let Received::User(message) = received else {
            continue;
        };
        let actions = behavior.step(B::Event::user(from, message)).await?;
        sends.append(actions.sends);
        creates.extend(actions.creates);
        match actions.become_ {
            Step::Continue => {}
            Step::Goto(never) => match never {},
            Step::Stop(exit) => {
                return Ok(Transcript {
                    sends,
                    creates,
                    exit,
                });
            }
        }
    }
    Ok(Transcript {
        sends,
        creates,
        exit: Exit::Collected,
    })
}
