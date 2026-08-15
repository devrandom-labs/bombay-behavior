//! Direct activation and statically typed wrapper composition.

use std::time::Duration;

use std::time::Instant;

use crate::Actions;
use crate::BehaviorBase;
use crate::protocol::TimerId;
use crate::shutdown::{FinalizeOnShutdown, ShutdownReaction, StopOnShutdown};
use crate::stash::{Stash, StashRoute};
use crate::supervision::{
    ChildTopology, RestartConfiguration, RestartPolicy, Strategy, Supervisor,
};
use crate::time::{Deadline, DeadlineReaction};
use crate::time::{OneShot, OneShotReaction};
use crate::time::{Periodic, PeriodicReaction};
use crate::time::{ReceiveTimeout, ReceiveTimeoutReaction};
use crate::watch::{LinkReaction, Watch};
use behavior::Never;
use behavior::{Address, Behavior, Births};
use behavior::{BehaviorActed, EventInput, UserEvent, delegate_transition};

const DEFAULT_STRATEGY: Strategy = Strategy::OneForOne;
const DEFAULT_POLICY: RestartPolicy = RestartPolicy::Transient;
const DEFAULT_BUDGET: (u32, Duration) = (1, Duration::from_secs(5));

/// An initialized behavior and the effects that must be interpreted before
/// its first mailbox turn.
pub struct Initialized<B: Behavior> {
    /// The activated behavior; only this value can fold mailbox events.
    pub behavior: Active<B>,
    /// Ordered initialization effects that the Driver interprets before the
    /// first mailbox event.
    pub actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
}

/// A behavior whose initialization fold has completed exactly once.
///
/// `Active<B>` does not implement [`Behavior`], so initialization cannot be
/// repeated through the public API:
///
/// ```compile_fail
/// use behavior_actors::{Activate, Behavior, Machine, MailAddr, Move, Never};
///
/// let definition = Machine::<MailAddr, _, _, _, Never>::new(
///     (),
///     (),
///     |_, _, _| Ok(Move::Stay),
/// );
/// let active = definition.initialize().unwrap().behavior;
/// active.initialize();
/// ```
pub struct Active<B: Behavior> {
    pub(crate) behavior: B,
}

impl<B: Behavior> Active<B> {
    /// Inspect the authored base behavior through any wrapper depth.
    #[must_use]
    pub fn base(&self) -> &B::Base
    where
        B: BehaviorBase,
    {
        self.behavior.base()
    }

    /// Return the number of messages held by a composed stash layer.
    #[must_use]
    pub fn stashed(&self) -> usize
    where
        B: crate::StashStatus,
    {
        self.behavior.stashed_messages()
    }

    /// Fold exactly one event after initialization.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    pub fn transition(&mut self, event: B::Event) -> BehaviorActed<B> {
        delegate_transition(&mut self.behavior, event)
    }

    /// Fold one user communication after initialization.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    pub fn receive(&mut self, from: B::Addr, message: B::Msg) -> BehaviorActed<B> {
        self.transition(B::Event::user(from, message))
    }

    /// Fold one statically supported semantic input after initialization.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
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

/// Consuming initialization for any concrete behavior definition.
///
/// This trait makes standalone catalogue templates directly usable without a
/// composition container. It is deliberately separate from [`Compose`]:
/// initialization is a lifecycle transition, not a wrapper transformation.
///
/// A raw definition cannot use the active mailbox API:
///
/// ```compile_fail
/// use behavior_actors::{Machine, MailAddr, Move, Never};
///
/// let mut definition = Machine::<MailAddr, _, _, _, Never>::new(
///     (),
///     (),
///     |_, _, _| Ok(Move::Stay),
/// );
/// definition.receive(MailAddr(0), 1_u8);
/// ```
pub trait Activate: Behavior + Sized {
    /// Consume this definition, perform its one initialization fold, and
    /// return the active behavior together with the ordered initialization
    /// effects.
    ///
    /// # Errors
    ///
    /// Returns the behavior's controlled initialization failure. A failed
    /// definition is consumed and cannot be activated or retried.
    fn initialize(self) -> Result<Initialized<Self>, Self::Error> {
        let mut behavior = self;
        let actions = behavior::initialize(&mut behavior)?;
        Ok(Initialized {
            behavior: Active { behavior },
            actions,
        })
    }
}

impl<B: Behavior> Activate for B {}

/// Statically typed wrapper composition for any concrete behavior.
///
/// Catalogue templates are constructed directly through their owning types.
/// This trait exists only for transformations that wrap a complete behavior,
/// change its closed event sum or named effect products, and preserve those
/// lanes through initialization and transition folds.
///
/// Standalone catalogue types do not require this trait. Import it only when a
/// wrapper method below is part of the concrete behavior definition.
pub trait Compose: Behavior + Sized {
    /// Stop normally when a typed shutdown request is folded.
    #[must_use]
    fn stop_on_shutdown(self) -> StopOnShutdown<Self> {
        StopOnShutdown::new(self)
    }

    /// Apply one final pure fold, retain its sends and creations, and stop
    /// normally regardless of the fold's become verdict.
    #[must_use]
    fn finalize_on_shutdown(self, finalize: ShutdownReaction<Self>) -> FinalizeOnShutdown<Self> {
        FinalizeOnShutdown::new(self, finalize)
    }

    /// Observe a peer and apply a pure reaction when it stops.
    #[must_use]
    fn watch(self, peer: Self::Addr, on_stopped: LinkReaction<Self>) -> Watch<Self> {
        Watch::new(self, peer, on_stopped)
    }

    /// Apply a pure reaction when the given absolute time is reached.
    ///
    #[must_use]
    fn deadline(
        self,
        timer: TimerId,
        when: Option<Instant>,
        on_reached: DeadlineReaction<Self>,
    ) -> Deadline<Self> {
        Deadline::new(self, timer, when, on_reached)
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
    fn receive_timeout(
        self,
        timer: TimerId,
        after: Duration,
        on_elapsed: ReceiveTimeoutReaction<Self>,
    ) -> ReceiveTimeout<Self> {
        ReceiveTimeout::new(self, timer, after, on_elapsed)
    }

    /// Notify the behavior exactly once after a relative delay.
    #[must_use]
    fn one_shot(
        self,
        timer: TimerId,
        after: Duration,
        on_elapsed: OneShotReaction<Self>,
    ) -> OneShot<Self>
    where
        Self::Event: crate::RouteInput<crate::TimerElapsed>,
    {
        OneShot::new(self, timer, after, on_elapsed)
    }

    /// Notify the behavior after every accepted relative timer generation.
    #[must_use]
    fn periodic(
        self,
        timer: TimerId,
        every: Duration,
        on_elapsed: PeriodicReaction<Self>,
    ) -> Periodic<Self>
    where
        Self::Event: crate::RouteInput<crate::TimerElapsed>,
    {
        Periodic::new(self, timer, every, on_elapsed)
    }

    /// Hold messages selected by `route` and replay them on `Release`.
    #[must_use]
    fn stash(self, route: fn(&Self::Msg) -> StashRoute) -> Stash<Self>
    where
        Self: Behavior<Ph = Never>,
    {
        Stash::new(self, route)
    }

    /// Create a supervised child topology with an explicit creator-local
    /// nonce assignment. Bombay does not infer routing identity from an
    /// integer position.
    ///
    /// # Errors
    ///
    /// Returns a typed fleet error when a configured nonce is duplicated or
    /// another topology invariant cannot be established.
    fn children<C>(
        self,
        nonces: fn(usize) -> <Self::Addr as Address>::Nonce,
        count: usize,
        build: fn(usize) -> Option<C>,
    ) -> ChildrenResult<Self, C>
    where
        Self: Behavior<Birth = Births<C>>,
        C: Behavior<Ph = Never, Addr = Self::Addr>,
        <Self::Addr as Address>::Nonce: From<u64>,
    {
        Supervisor::new(
            self,
            ChildTopology::new((0..count).map(nonces), build),
            RestartConfiguration::new(
                DEFAULT_STRATEGY,
                DEFAULT_POLICY,
                DEFAULT_BUDGET.0,
                DEFAULT_BUDGET.1,
            ),
        )
    }
}

impl<B: Behavior> Compose for B {}

/// Result of constructing a statically typed supervised child topology.
pub type ChildrenResult<B, C> =
    Result<Supervisor<B, C>, crate::FleetError<<<B as Behavior>::Addr as Address>::Nonce>>;
