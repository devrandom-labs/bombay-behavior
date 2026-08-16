//! Direct, consuming behavior activation.

use crate::Actions;
use crate::BehaviorBase;
use behavior::Behavior;
use behavior::{BehaviorActed, EventInput, UserEvent, delegate_transition};

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
/// wrapper construction. Initialization is a lifecycle transition, not a
/// wrapper transformation.
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
