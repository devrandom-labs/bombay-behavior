//! Structural event layer shared by timer-based behavior compositions.

/// A timer template owns one elapsed-timer lane in front of the complete event
/// algebra of the behavior it wraps.
pub type TimedEvent<E> = behavior::EventLayer<crate::TimerElapsed, E>;
