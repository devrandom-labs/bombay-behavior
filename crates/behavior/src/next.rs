//! The host sum used to represent the behavior algebra's `become` seat.

/// The uninhabited type with two structural jobs. As a phase menu,
/// `Step<Never>` has no constructible `Goto` — a plain actor is a one-phase
/// machine. As an outbound/offspring menu, it proves a layer sends or creates
/// nothing. The law is the type, not a convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Never {}

/// The behavior has designated termination as its next state.
///
/// This marker deliberately carries no lifecycle, supervision, collection, or
/// runtime-failure provenance. Those facts are typed observations owned by
/// actor compositions and the Bombay runtime, not part of the behavior
/// algebra's termination decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stopped;

/// A generic next-state verdict. Bombay Behavior pins `R` to [`Stopped`], so
/// actor-specific lifecycle data cannot enter the `become` seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<Ph = Never, R = Never> {
    /// Keep the current behavior; poll for the next event.
    Continue,
    /// Transition to another phase from the menu (no-op when already there).
    Goto(Ph),
    /// Select a terminal result. In [`crate::Become`] this is the payload-free
    /// [`Stopped`] marker.
    Stop(R),
}
