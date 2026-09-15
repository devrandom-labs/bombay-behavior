//! Structural event layer shared by timer-based behavior compositions.

use behavior::{Actions, Behavior};

/// A timer template owns one elapsed-timer lane in front of the complete event
/// algebra of the behavior it wraps.
pub type TimedEvent<E> = behavior::EventLayer<crate::TimerElapsed, E>;

/// Infallible reaction that may use every action capability of the wrapped behavior.
pub type TimedReaction<B> = fn(
    &mut B,
) -> Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;
