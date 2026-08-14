//! Pure behavior folds from one typed event to explicit transition actions.

use super::user_event::UserEvent;
use crate::actor::{Address, BirthMode};
use crate::effects::{Acted, Actions, SendAlgebra};

/// The only successful effect shape admitted by a [`Behavior`] implementation.
pub type BehaviorActed<B> = Acted<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
    <B as Behavior>::Error,
>;

/// Crate-minted capability for defining one initialization fold.
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

/// Crate-minted capability for defining one active mailbox fold.
///
/// Values can only be issued by an initialized [`crate::Active`] behavior or
/// by a concrete core wrapper delegating the same active turn inward.
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

/// Fold one event through an inner behavior owned by a semantic wrapper.
///
/// This is Bombay's derived, canonical boundary for wrapper composition; it is
/// not an additional actor-model operation. It invokes the inner deterministic
/// fold exactly once and returns its complete typed action value without
/// inspecting or transforming it. It does not execute a runtime turn,
/// interpret effects, or provide an alternate actor executor; top-level runtime
/// transitions remain the responsibility of the runtime's machine adapter.
///
/// # Errors
///
/// Returns the inner behavior's controlled transition failure unchanged.
pub(crate) fn initialize<B: Behavior>(behavior: &mut B) -> BehaviorActed<B> {
    B::init(behavior, InitializationTurn::new())
}

pub(crate) fn delegate_transition<B: Behavior>(
    behavior: &mut B,
    event: B::Event,
) -> BehaviorActed<B> {
    B::transition(behavior, ActiveTurn::new(), event)
}
