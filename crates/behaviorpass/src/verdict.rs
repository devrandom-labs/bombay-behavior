//! The verdict vocabulary — owned in-crate, not borrowed from bombay
//! (2026-08-04: the type-exact trace rationale retired with the reference
//! crate; a pass crate depends on nothing upward — this IS the bombay
//! declutter, and this file is what travels back as bombay's vocabulary).

/// The uninhabited type with two structural jobs. As a phase menu,
/// `Step<Never>` has no constructible `Goto` — a plain actor is a one-phase
/// machine. As an outbound/offspring menu, it proves a layer sends or creates
/// nothing. The law is the type, not a convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Never {}

/// The **become** verdict (Agha 1986): what replaces the current behavior as
/// it processes one event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step<Ph = Never, R = Never> {
    /// Keep the current behavior; poll for the next event.
    Continue,
    /// Transition to another phase from the menu (no-op when already there).
    Goto(Ph),
    /// Stop after this reaction, with the carried reason.
    Stop(R),
}
