//! Intent-facing typestate composition. Every method immediately builds a
//! concrete pure behavior; there is no separate intent representation.

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::{Address, Behavior, BirthMode, Births};
use crate::deadlined::{At, AtReaction};
use crate::stashing::{StashRoute, Stashing};
use crate::supervising::{RestartPolicy, Strategy, Supervising};
use crate::verdict::Never;
use crate::watching::{LinkReaction, Watching};
use crate::{Actions, Base, Exit, Fsm, Move, SendAlgebra, State};

const DEFAULT_STRATEGY: Strategy = Strategy::OneForOne;
const DEFAULT_POLICY: RestartPolicy = RestartPolicy::Transient;
const DEFAULT_BUDGET: (u32, Duration) = (1, Duration::from_secs(5));

fn identity_nonce<N: From<u64>>(index: usize) -> N {
    N::from(u64::try_from(index).expect("fleet index fits u64"))
}

pub struct Spec<B>(B);

impl<S: State<O, Br, E>, O, Br: BirthMode, E> Spec<Base<S, O, Br, E>> {
    #[must_use]
    pub fn new(state: S) -> Self {
        Self(Base::new(state))
    }
}

impl<A, S, M, P, E> Spec<Fsm<A, S, M, P, E>>
where
    A: Address,
    P: Copy + PartialEq,
{
    #[must_use]
    pub fn machine(state: S, phase: P, on: fn(P, &mut S, &M) -> Result<Move<P>, E>) -> Self {
        Self(Fsm::new(state, phase, on))
    }
}

impl<B: Behavior> Spec<B> {
    #[must_use]
    pub fn from_behavior(behavior: B) -> Self {
        Self(behavior)
    }

    #[must_use]
    pub fn build(self) -> B {
        self.0
    }

    #[must_use]
    pub fn behavior(&self) -> &B {
        &self.0
    }

    /// Observe a peer and apply a pure reaction when it stops.
    #[must_use]
    pub fn watch(self, peer: B::Addr, on_stopped: LinkReaction<B>) -> Spec<Watching<B>> {
        Spec(Watching::new(self.0, peer, on_stopped))
    }

    /// Apply a pure reaction when the given absolute time is reached.
    #[must_use]
    pub fn at(self, when: Option<Instant>, on_reached: AtReaction<B>) -> Spec<At<B>> {
        Spec(At::new(self.0, when, on_reached))
    }

    /// Hold messages selected by `route` and replay them on `Release`.
    #[must_use]
    pub fn stash(self, route: fn(&B::Msg) -> StashRoute) -> Spec<Stashing<B>>
    where
        B: Behavior<Ph = Never>,
    {
        Spec(Stashing::new(self.0, route))
    }

    /// Create a supervised child topology. Concrete proxy and monitor types
    /// remain hidden in the returned typestate.
    #[must_use]
    pub fn children<C>(self, fleet: (usize, fn(usize) -> C)) -> Spec<Supervising<B, C>>
    where
        B: Behavior<Birth = Births<C>>,
        C: Behavior<Ph = Never, Addr = B::Addr>,
        <B::Addr as Address>::Nonce: From<u64>,
    {
        self.children_with_nonces(identity_nonce, fleet.0, fleet.1)
    }

    #[must_use]
    pub fn children_with_nonces<C>(
        self,
        nonces: fn(usize) -> <B::Addr as Address>::Nonce,
        count: usize,
        build: fn(usize) -> C,
    ) -> Spec<Supervising<B, C>>
    where
        B: Behavior<Birth = Births<C>>,
        C: Behavior<Ph = Never, Addr = B::Addr>,
    {
        Spec(Supervising::new(
            self.0,
            nonces,
            count,
            build,
            DEFAULT_STRATEGY,
            DEFAULT_POLICY,
            DEFAULT_BUDGET.0,
            DEFAULT_BUDGET.1,
        ))
    }
}

impl<B, C> Spec<Supervising<B, C>>
where
    B: Behavior<Birth = Births<C>>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    #[must_use]
    pub fn restart(self, strategy: Strategy) -> Self {
        Self(self.0.with_strategy(strategy))
    }

    #[must_use]
    pub fn when(self, policy: RestartPolicy) -> Self {
        Self(self.0.with_policy(policy))
    }

    #[must_use]
    pub fn within(self, maximum: u32, window: Duration) -> Self {
        Self(self.0.with_budget(maximum, window))
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Spec<B>
where
    A: Address + Send,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<
            Addr = A,
            Ph = Ph,
            Sends = Sends,
            Birth = Br,
            Effect = Actions<A, Ph, Sends, Br>,
            Done = Exit<A>,
        > + Send,
    A::Nonce: Send,
    B::Msg: Send,
    B::Event: Send,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = B::Event;
    type Sends = Sends;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;
    type Effect = B::Effect;
    type Done = B::Done;

    async fn init(&mut self) -> Result<Self::Effect, B::Error> {
        self.0.init().await
    }

    async fn step(&mut self, event: B::Event) -> Result<Self::Effect, B::Error> {
        self.0.step(event).await
    }
}
