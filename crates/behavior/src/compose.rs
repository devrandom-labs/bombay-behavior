//! Intent-facing typestate composition. Every method immediately builds a
//! concrete pure behavior; there is no separate intent representation.

use std::time::Duration;

use std::time::Instant;

use crate::Actions;
use crate::BehaviorBase;
use crate::behavior::{Address, Behavior, Births};
use crate::calculus::{BehaviorActed, EventInput, UserEvent, delegate_transition};
use crate::next::Never;
use crate::protocol::TimerId;
use crate::shutdown::{FinalizeOnShutdown, ShutdownReaction, StopOnShutdown};
use crate::stash::{Stash, StashRoute};
use crate::supervision::{RestartPolicy, Strategy, SupervisionFailureReaction, Supervisor};
use crate::timing::{Deadline, DeadlineReaction};
use crate::timing::{ReceiveTimeout, ReceiveTimeoutReaction};
use crate::watch::{LinkReaction, Watch};

const DEFAULT_STRATEGY: Strategy = Strategy::OneForOne;
const DEFAULT_POLICY: RestartPolicy = RestartPolicy::Transient;
const DEFAULT_BUDGET: (u32, Duration) = (1, Duration::from_secs(5));

/// A behavior definition that may still be wrapped and initialized.
///
/// Definitions deliberately expose no mailbox fold:
///
/// ```compile_fail
/// use behavior::{Compose, Machine, MailAddr, Move, Never};
///
/// let mut definition: Compose<Machine<MailAddr, (), u8, (), Never>> =
///     Compose::new(Machine::new((), (), |_, _, _| Ok(Move::Stay)));
/// definition.receive(MailAddr(0), 1);
/// ```
pub struct Compose<B> {
    behavior: B,
}

/// An initialized behavior and the effects that must be interpreted before
/// its first mailbox turn.
pub struct Initialized<B: Behavior> {
    pub behavior: Active<B>,
    pub actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
}

/// A behavior whose initialization fold has completed exactly once.
///
/// `Active<B>` does not implement [`Behavior`], so initialization cannot be
/// repeated through the public API:
///
/// ```compile_fail
/// use behavior::{Behavior, Compose, Machine, MailAddr, Move, Never};
///
/// let definition: Compose<Machine<MailAddr, (), u8, (), Never>> =
///     Compose::new(Machine::new((), (), |_, _, _| Ok(Move::Stay)));
/// let active = definition.initialize().unwrap().behavior;
/// active.initialize();
/// ```
pub struct Active<B: Behavior> {
    pub(crate) behavior: B,
}

impl<B: Behavior> Active<B> {
    #[must_use]
    pub fn base(&self) -> &B::Base
    where
        B: BehaviorBase,
    {
        self.behavior.base()
    }

    #[must_use]
    pub fn stashed(&self) -> usize
    where
        B: crate::StashStatus,
    {
        self.behavior.stashed_messages()
    }

    /// Fold exactly one event after initialization.
    pub fn transition(&mut self, event: B::Event) -> BehaviorActed<B> {
        delegate_transition(&mut self.behavior, event)
    }

    /// Fold one user communication after initialization.
    pub fn receive(&mut self, from: B::Addr, message: B::Msg) -> BehaviorActed<B> {
        self.transition(B::Event::user(from, message))
    }

    /// Fold one statically supported semantic input after initialization.
    pub fn on<Input>(&mut self, input: Input) -> BehaviorActed<B>
    where
        B::Event: EventInput<Input>,
    {
        self.transition(B::Event::inject(input))
    }
}

impl<B: Behavior> core::ops::Deref for Active<B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.behavior
    }
}

impl<B> Compose<B> {
    /// Begin a composition from the one concrete [`Behavior`] value being
    /// defined directly or by `#[behavior]`.
    #[must_use]
    pub const fn new(behavior: B) -> Self {
        Self { behavior }
    }
}

impl<B: Behavior> Compose<B> {
    fn map_behavior<Mapped>(self, map: impl FnOnce(B) -> Mapped) -> Compose<Mapped> {
        Compose {
            behavior: map(self.behavior),
        }
    }

    fn try_map_behavior<Mapped, E>(
        self,
        map: impl FnOnce(B) -> Result<Mapped, E>,
    ) -> Result<Compose<Mapped>, E> {
        map(self.behavior).map(|behavior| Compose { behavior })
    }

    #[must_use]
    pub fn base(&self) -> &B::Base
    where
        B: BehaviorBase,
    {
        self.behavior.base()
    }

    #[must_use]
    pub fn definition(&self) -> &B {
        &self.behavior
    }

    /// Consume this definition, perform its one initialization fold, and
    /// return the active behavior together with the ordered initialization
    /// effects.
    ///
    /// # Errors
    ///
    /// Returns the behavior's controlled initialization failure. A failed
    /// definition is consumed and cannot be activated or retried.
    pub fn initialize(self) -> Result<Initialized<B>, B::Error> {
        let mut behavior = self.behavior;
        let actions = crate::calculus::initialize(&mut behavior)?;
        Ok(Initialized {
            behavior: Active { behavior },
            actions,
        })
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
    #[must_use]
    pub fn deadline(
        self,
        timer: TimerId,
        when: Option<Instant>,
        on_reached: DeadlineReaction<B>,
    ) -> Compose<Deadline<B>> {
        self.map_behavior(|behavior| Deadline::new(behavior, timer, when, on_reached))
    }

    /// Notify the behavior once after an idle period containing no successful
    /// user communication.
    ///
    /// Initialization and each successful continuing user fold emit a relative
    /// schedule. Service events never reset inactivity. A matching delivery is
    /// consumed before `on_elapsed` runs, and a continuing reaction remains
    /// unarmed until another successful continuing user communication.
    ///
    #[must_use]
    pub fn receive_timeout(
        self,
        timer: TimerId,
        after: Duration,
        on_elapsed: ReceiveTimeoutReaction<B>,
    ) -> Compose<ReceiveTimeout<B>> {
        self.map_behavior(|behavior| ReceiveTimeout::new(behavior, timer, after, on_elapsed))
    }

    /// Hold messages selected by `route` and replay them on `Release`.
    #[must_use]
    pub fn stash(self, route: fn(&B::Msg) -> StashRoute) -> Compose<Stash<B>>
    where
        B: Behavior<Ph = Never>,
    {
        self.map_behavior(|behavior| Stash::new(behavior, route))
    }

    /// Create a supervised child topology with an explicit creator-local
    /// nonce assignment. Bombay does not infer routing identity from an
    /// integer position.
    #[must_use]
    pub fn children<C>(
        self,
        nonces: fn(usize) -> <B::Addr as Address>::Nonce,
        count: usize,
        build: fn(usize) -> Option<C>,
    ) -> Result<Compose<Supervisor<B, C>>, crate::FleetError<<B::Addr as Address>::Nonce>>
    where
        B: Behavior<Birth = Births<C>>,
        C: Behavior<Ph = Never, Addr = B::Addr>,
        <B::Addr as Address>::Nonce: From<u64>,
    {
        self.try_map_behavior(|behavior| {
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
    <B::Addr as Address>::Nonce: From<u64>,
{
    #[must_use]
    pub fn restart(self, strategy: Strategy) -> Self {
        Self {
            behavior: self.behavior.with_strategy(strategy),
        }
    }

    #[must_use]
    pub fn when(self, policy: RestartPolicy) -> Self {
        Self {
            behavior: self.behavior.with_policy(policy),
        }
    }

    #[must_use]
    pub fn within(self, maximum: u32, window: Duration) -> Self {
        Self {
            behavior: self.behavior.with_budget(maximum, window),
        }
    }

    /// Apply a pure reaction when supervision can no longer preserve its
    /// child topology.
    #[must_use]
    pub fn on_supervision_failure(self, reaction: SupervisionFailureReaction<B>) -> Self {
        Self {
            behavior: self.behavior.with_failure_reaction(reaction),
        }
    }
}
