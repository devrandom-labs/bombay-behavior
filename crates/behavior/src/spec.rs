//! Intent-facing typestate composition. Every method immediately builds a
//! concrete pure behavior; there is no separate intent representation.

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::{Address, Behavior, BirthMode, Births};
use crate::deadlined::{At, AtReaction};
use crate::protocol::TimerId;
use crate::receive_timeout::{ReceiveTimeout, ReceiveTimeoutReaction};
use crate::shutdown::{FinalizeOnShutdown, ShutdownReaction, StopOnShutdown};
use crate::stashing::{StashRoute, Stashing};
use crate::supervising::{RestartPolicy, Strategy, Supervising, SupervisionFailureReaction};
use crate::verdict::Never;
use crate::watching::{LinkReaction, Watching};
use crate::{Actions, Base, Fsm, Move, SendAlgebra, State};

const DEFAULT_STRATEGY: Strategy = Strategy::OneForOne;
const DEFAULT_POLICY: RestartPolicy = RestartPolicy::Transient;
const DEFAULT_BUDGET: (u32, Duration) = (1, Duration::from_secs(5));

fn identity_nonce<N: From<u64>>(index: usize) -> N {
    N::from(u64::try_from(index).expect("fleet index fits u64"))
}

pub struct Spec<B> {
    behavior: B,
    next_timer: u64,
}

impl<S: State<O, Br, E>, O, Br: BirthMode, E> Spec<Base<S, O, Br, E>> {
    #[must_use]
    pub fn new(state: S) -> Self {
        Self {
            behavior: Base::new(state),
            next_timer: 0,
        }
    }
}

impl<A, S, M, P, E> Spec<Fsm<A, S, M, P, E>>
where
    A: Address,
    P: Copy + PartialEq,
{
    #[must_use]
    pub fn machine(state: S, phase: P, on: fn(P, &mut S, &M) -> Result<Move<P>, E>) -> Self {
        Self {
            behavior: Fsm::new(state, phase, on),
            next_timer: 0,
        }
    }
}

impl<B: Behavior> Spec<B> {
    #[must_use]
    pub fn from_behavior(behavior: B) -> Self {
        Self {
            behavior,
            next_timer: 0,
        }
    }

    #[must_use]
    pub fn build(self) -> B {
        self.behavior
    }

    #[must_use]
    pub fn behavior(&self) -> &B {
        &self.behavior
    }

    /// Stop normally when a typed shutdown request is folded.
    #[must_use]
    pub fn stop_on_shutdown(self) -> Spec<StopOnShutdown<B>> {
        Spec {
            behavior: StopOnShutdown::new(self.behavior),
            next_timer: self.next_timer,
        }
    }

    /// Apply one final pure fold, retain its sends and creations, and stop
    /// normally regardless of the fold's become verdict.
    #[must_use]
    pub fn finalize_on_shutdown(
        self,
        finalize: ShutdownReaction<B>,
    ) -> Spec<FinalizeOnShutdown<B>> {
        Spec {
            behavior: FinalizeOnShutdown::new(self.behavior, finalize),
            next_timer: self.next_timer,
        }
    }

    /// Observe a peer and apply a pure reaction when it stops.
    #[must_use]
    pub fn watch(self, peer: B::Addr, on_stopped: LinkReaction<B>) -> Spec<Watching<B>> {
        Spec {
            behavior: Watching::new(self.behavior, peer, on_stopped),
            next_timer: self.next_timer,
        }
    }

    /// Apply a pure reaction when the given absolute time is reached.
    ///
    /// # Panics
    ///
    /// Panics if one specification composes more than `u64::MAX` timer
    /// capabilities.
    #[must_use]
    pub fn at(self, when: Option<Instant>, on_reached: AtReaction<B>) -> Spec<At<B>> {
        Spec {
            behavior: At::new(self.behavior, TimerId(self.next_timer), when, on_reached),
            next_timer: self
                .next_timer
                .checked_add(1)
                .expect("timer identity exhausted"),
        }
    }

    /// Notify the behavior once after an idle period containing no successful
    /// user communication.
    ///
    /// Initialization and each successful continuing user fold emit a relative
    /// schedule. Service events never reset inactivity. A matching delivery is
    /// consumed before `on_elapsed` runs, and a continuing reaction remains
    /// unarmed until another successful continuing user communication.
    ///
    /// # Panics
    ///
    /// Panics if one specification composes more than `u64::MAX` timer
    /// capabilities.
    #[must_use]
    pub fn receive_timeout(
        self,
        after: Duration,
        on_elapsed: ReceiveTimeoutReaction<B>,
    ) -> Spec<ReceiveTimeout<B>> {
        Spec {
            behavior: ReceiveTimeout::new(
                self.behavior,
                TimerId(self.next_timer),
                after,
                on_elapsed,
            ),
            next_timer: self
                .next_timer
                .checked_add(1)
                .expect("timer identity exhausted"),
        }
    }

    /// Hold messages selected by `route` and replay them on `Release`.
    #[must_use]
    pub fn stash(self, route: fn(&B::Msg) -> StashRoute) -> Spec<Stashing<B>>
    where
        B: Behavior<Ph = Never>,
    {
        Spec {
            behavior: Stashing::new(self.behavior, route),
            next_timer: self.next_timer,
        }
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
        Spec {
            behavior: Supervising::new(
                self.behavior,
                nonces,
                count,
                build,
                DEFAULT_STRATEGY,
                DEFAULT_POLICY,
                DEFAULT_BUDGET.0,
                DEFAULT_BUDGET.1,
            ),
            next_timer: self.next_timer,
        }
    }
}

impl<B, C> Spec<Supervising<B, C>>
where
    B: Behavior<Birth = Births<C>>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    #[must_use]
    pub fn restart(self, strategy: Strategy) -> Self {
        Self {
            behavior: self.behavior.with_strategy(strategy),
            next_timer: self.next_timer,
        }
    }

    #[must_use]
    pub fn when(self, policy: RestartPolicy) -> Self {
        Self {
            behavior: self.behavior.with_policy(policy),
            next_timer: self.next_timer,
        }
    }

    #[must_use]
    pub fn within(self, maximum: u32, window: Duration) -> Self {
        Self {
            behavior: self.behavior.with_budget(maximum, window),
            next_timer: self.next_timer,
        }
    }

    /// Apply a pure reaction when supervision can no longer preserve its
    /// child topology.
    #[must_use]
    pub fn on_supervision_failure(self, reaction: SupervisionFailureReaction<B>) -> Self {
        Self {
            behavior: self.behavior.with_failure_reaction(reaction),
            next_timer: self.next_timer,
        }
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Spec<B>
where
    A: Address + Send,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br> + Send,
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

    async fn init(&mut self) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
        self.behavior.init().await
    }

    async fn step(&mut self, event: B::Event) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
        self.behavior.step(event).await
    }
}
