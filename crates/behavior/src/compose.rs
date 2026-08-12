//! Intent-facing typestate composition. Every method immediately builds a
//! concrete pure behavior; there is no separate intent representation.

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::{Address, Behavior, BirthMode, Births};
use crate::next::Never;
use crate::protocol::TimerId;
use crate::shutdown::{FinalizeOnShutdown, ShutdownReaction, StopOnShutdown};
use crate::stash::{Stash, StashRoute};
use crate::supervision::{RestartPolicy, Strategy, SupervisionFailureReaction, Supervisor};
use crate::timing::{Deadline, DeadlineReaction};
use crate::timing::{ReceiveTimeout, ReceiveTimeoutReaction};
use crate::watch::{LinkReaction, Watch};
use crate::{Actions, BehaviorFn, Handler, Machine, Move, Pure, SendAlgebra, delegate_transition};

const DEFAULT_STRATEGY: Strategy = Strategy::OneForOne;
const DEFAULT_POLICY: RestartPolicy = RestartPolicy::Transient;
const DEFAULT_BUDGET: (u32, Duration) = (1, Duration::from_secs(5));

fn identity_nonce<N: From<u64>>(index: usize) -> N {
    N::from(u64::try_from(index).expect("fleet index fits u64"))
}

pub struct Compose<B> {
    behavior: B,
    next_timer: u64,
}

impl<S: Handler<O, Br, E>, O, Br: BirthMode, E> Compose<Pure<S, O, Br, E>> {
    #[must_use]
    pub fn new(state: S) -> Self {
        Self {
            behavior: Pure::new(state),
            next_timer: 0,
        }
    }
}

impl<A, S, M, P, E> Compose<Machine<A, S, M, P, E>>
where
    A: Address,
    P: Copy + PartialEq,
{
    #[must_use]
    pub fn machine(state: S, phase: P, on: fn(P, &mut S, &M) -> Result<Move<P>, E>) -> Self {
        Self {
            behavior: Machine::new(state, phase, on),
            next_timer: 0,
        }
    }
}

impl<S, A, M, Sends, Br, E, I, F> Compose<BehaviorFn<S, A, M, Sends, Br, E, I, F>>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    I: FnMut(&mut S) -> crate::Acted<A, Never, Sends, Br, E>,
    F: FnMut(&mut S, A, M) -> crate::Acted<A, Never, Sends, Br, E>,
{
    /// Define a concrete behavior from explicit initialization and user-event
    /// folds, ready for typed wrapper composition.
    #[must_use]
    pub fn from_fns(state: S, initialize: I, transition: F) -> Self {
        Self {
            behavior: BehaviorFn::new(state, initialize, transition),
            next_timer: 0,
        }
    }
}

impl<B: Behavior> Compose<B> {
    fn map_behavior<Mapped>(self, map: impl FnOnce(B) -> Mapped) -> Compose<Mapped> {
        Compose {
            behavior: map(self.behavior),
            next_timer: self.next_timer,
        }
    }

    fn map_timer<Mapped>(self, map: impl FnOnce(B, TimerId) -> Mapped) -> Compose<Mapped> {
        let timer = TimerId(self.next_timer);
        Compose {
            behavior: map(self.behavior, timer),
            next_timer: self
                .next_timer
                .checked_add(1)
                .expect("timer identity exhausted"),
        }
    }

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
    pub fn stop_on_shutdown(self) -> Compose<StopOnShutdown<B>> {
        self.map_behavior(StopOnShutdown::new)
    }

    /// Apply one final pure fold, retain its sends and creations, and stop
    /// normally regardless of the fold's become verdict.
    #[must_use]
    pub fn finalize_on_shutdown(
        self,
        finalize: ShutdownReaction<B>,
    ) -> Compose<FinalizeOnShutdown<B>> {
        self.map_behavior(|behavior| FinalizeOnShutdown::new(behavior, finalize))
    }

    /// Observe a peer and apply a pure reaction when it stops.
    #[must_use]
    pub fn watch(self, peer: B::Addr, on_stopped: LinkReaction<B>) -> Compose<Watch<B>> {
        self.map_behavior(|behavior| Watch::new(behavior, peer, on_stopped))
    }

    /// Apply a pure reaction when the given absolute time is reached.
    ///
    /// # Panics
    ///
    /// Panics if one specification composes more than `u64::MAX` timer
    /// capabilities.
    #[must_use]
    pub fn deadline(
        self,
        when: Option<Instant>,
        on_reached: DeadlineReaction<B>,
    ) -> Compose<Deadline<B>> {
        self.map_timer(|behavior, timer| Deadline::new(behavior, timer, when, on_reached))
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
    ) -> Compose<ReceiveTimeout<B>> {
        self.map_timer(|behavior, timer| ReceiveTimeout::new(behavior, timer, after, on_elapsed))
    }

    /// Hold messages selected by `route` and replay them on `Release`.
    #[must_use]
    pub fn stash(self, route: fn(&B::Msg) -> StashRoute) -> Compose<Stash<B>>
    where
        B: Behavior<Ph = Never>,
    {
        self.map_behavior(|behavior| Stash::new(behavior, route))
    }

    /// Create a supervised child topology. Concrete proxy and monitor types
    /// remain hidden in the returned typestate.
    #[must_use]
    pub fn children<C>(self, fleet: (usize, fn(usize) -> C)) -> Compose<Supervisor<B, C>>
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
    ) -> Compose<Supervisor<B, C>>
    where
        B: Behavior<Birth = Births<C>>,
        C: Behavior<Ph = Never, Addr = B::Addr>,
    {
        self.map_behavior(|behavior| {
            Supervisor::new(
                behavior,
                nonces,
                count,
                build,
                DEFAULT_STRATEGY,
                DEFAULT_POLICY,
                DEFAULT_BUDGET.0,
                DEFAULT_BUDGET.1,
            )
        })
    }
}

impl<B, C> Compose<Supervisor<B, C>>
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

impl<B, A, Ph, Sends, Br> Behavior for Compose<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = B::Event;
    type Sends = Sends;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
        self.behavior.init()
    }

    fn transition(&mut self, event: B::Event) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
        delegate_transition(&mut self.behavior, event)
    }
}
