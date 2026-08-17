//! Concrete supervision strategies, restart policy, and failure reactions.

use crate::{Address, Become, Behavior, Crash, Exit, Step, Stopped, SupervisionFailureReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    OneForOne,
    OneForAll,
    RestForOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Permanent,
    Transient,
    Temporary,
}

#[must_use]
pub const fn restart_one() -> Strategy {
    Strategy::OneForOne
}

#[must_use]
pub const fn restart_all() -> Strategy {
    Strategy::OneForAll
}

#[must_use]
pub const fn restart_rest() -> Strategy {
    Strategy::RestForOne
}

/// A typed failure of the supervisor's child-topology contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisionFailure<A: Address> {
    pub child: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub reason: SupervisionFailureReason,
}

/// A supervisor's request for the Bombay runtime to publish one exact typed
/// topology failure before interpreting the same action's terminal `become`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ReportSupervisionFailure<A: Address> {
    pub failure: SupervisionFailure<A>,
}

impl<A: Address> ReportSupervisionFailure<A> {
    #[must_use]
    pub const fn new(failure: SupervisionFailure<A>) -> Self {
        Self { failure }
    }
}

impl<A: Address> SupervisionFailure<A> {
    #[must_use]
    pub const fn new(
        child: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
        reason: SupervisionFailureReason,
    ) -> Self {
        Self {
            child,
            outcome,
            reason,
        }
    }
}

/// Pure policy applied when a supervisor cannot preserve its child topology.
pub type SupervisionFailureReaction<B> = fn(
    &mut B,
    &SupervisionFailure<<B as crate::Protocol>::Addr>,
) -> Result<Become, <B as Behavior>::Error>;

/// Retire the failed slot and keep the supervisor alive.
///
/// # Errors
/// This supplied policy never returns a controlled behavior error.
pub fn retire_on_supervision_failure<B: Behavior>(
    _behavior: &mut B,
    _failure: &SupervisionFailure<B::Addr>,
) -> Result<Become, B::Error> {
    Ok(Step::Continue)
}

/// Stop the supervisor with a typed failure outcome.
///
/// # Errors
/// This supplied policy never returns a controlled behavior error.
pub fn stop_on_supervision_failure<B: Behavior>(
    _behavior: &mut B,
    _failure: &SupervisionFailure<B::Addr>,
) -> Result<Become, B::Error> {
    Ok(Step::Stop(Stopped))
}
