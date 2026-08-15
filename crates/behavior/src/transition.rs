//! Pure behavior folds from one typed event to explicit transition actions.

use crate::actor::{Address, BirthMode};
use crate::effects::{Acted, Actions, SendAlgebra};
use crate::user_event::UserEvent;

/// The only successful effect shape admitted by a [`Behavior`] implementation.
pub type BehaviorActed<B> = Acted<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
    <B as Behavior>::Error,
>;

/// Capability for defining one initialization fold.
///
/// The constructor is private: only the lifecycle boundary can issue this
/// capability, exactly once for an owned behavior value.
pub struct InitializationTurn {
    #[allow(dead_code, reason = "private field prevents external construction")]
    private: (),
}

impl InitializationTurn {
    pub(crate) const fn new() -> Self {
        Self { private: () }
    }
}

/// Capability for defining one active mailbox fold.
///
/// Values are issued only by the lifecycle and wrapper-composition boundaries.
pub struct ActiveTurn {
    #[allow(dead_code, reason = "private field prevents external construction")]
    private: (),
}

impl ActiveTurn {
    pub(crate) const fn new() -> Self {
        Self { private: () }
    }
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
    fn init(&mut self, _turn: InitializationTurn) -> BehaviorActed<Self>
    where
        Self: Sized,
    {
        Ok(Actions::cont())
    }

    /// Fold exactly one event into explicit actions and the next behavior.
    ///
    /// # Errors
    ///
    /// Returns the behavior's declared controlled transition failure.
    fn transition(&mut self, _turn: ActiveTurn, event: Self::Event) -> BehaviorActed<Self>;
}

/// Static projection from a composed behavior to its authored base behavior.
///
/// Wrappers preserve this associated type, so inspection never depends on
/// wrapper nesting depth or a positional path.
pub trait BehaviorBase {
    type Base;

    fn base(&self) -> &Self::Base;
}

/// Initialize an inner behavior owned by a semantic composition.
///
/// This is Bombay's derived, canonical boundary for wrapper composition; it is
/// not an additional actor-model operation. It invokes the inner initialization
/// fold exactly once and returns its complete typed action value without
/// inspecting or transforming it. It does not execute a runtime turn,
/// interpret effects, or provide an alternate actor executor; top-level runtime
/// transitions remain the responsibility of the runtime's machine adapter.
///
/// # Errors
///
/// Returns the inner behavior's controlled transition failure unchanged.
#[doc(hidden)]
pub fn initialize<B: Behavior>(behavior: &mut B) -> BehaviorActed<B> {
    B::init(behavior, InitializationTurn::new())
}

/// Fold one event through an inner behavior owned by a semantic wrapper.
///
/// This invokes the inner deterministic fold exactly once and returns its
/// complete typed action value without interpreting it.
///
/// # Errors
///
/// Returns the inner behavior's controlled transition failure unchanged.
#[doc(hidden)]
pub fn delegate_transition<B: Behavior>(behavior: &mut B, event: B::Event) -> BehaviorActed<B> {
    B::transition(behavior, ActiveTurn::new(), event)
}
