//! Concrete supervision strategies, restart policy, and failure reactions.

use crate::{
    Address, Become, Behavior, Crash, CreationKind, CreationRejection, Exit, RestartDenial, Step,
    Stopped, SupervisionFailureReason,
};

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
///
/// Termination and creation failures are distinct variants because a rejected
/// fresh installation has no truthful child-terminal outcome. Each variant
/// owns exactly the identity, provenance, and failure data supplied by its
/// authoritative fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionFailure<A: Address> {
    RestartDenied {
        child: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
        denial: RestartDenial,
    },
    StableChildStopped {
        child: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
    },
    StableChildCreationRejected {
        child: A::Nonce,
        kind: CreationKind<A::Nonce>,
        rejection: CreationRejection,
    },
    WorkerFactoryRejected {
        child: A::Nonce,
        index: usize,
    },
    WorkerCreationRejected {
        proxy: A::Nonce,
        worker: A::Nonce,
        kind: CreationKind<A::Nonce>,
        rejection: CreationRejection,
    },
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

impl<A: Address> behavior::InterpreterRequest for ReportSupervisionFailure<A> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
}

impl<A: Address> SupervisionFailure<A> {
    #[must_use]
    pub const fn restart_denied(
        child: A::Nonce,
        outcome: Result<Exit<A>, Crash>,
        denial: RestartDenial,
    ) -> Self {
        Self::RestartDenied {
            child,
            outcome,
            denial,
        }
    }

    #[must_use]
    pub const fn stable_child_stopped(child: A::Nonce, outcome: Result<Exit<A>, Crash>) -> Self {
        Self::StableChildStopped { child, outcome }
    }

    #[must_use]
    pub const fn stable_child_creation_rejected(
        child: A::Nonce,
        kind: CreationKind<A::Nonce>,
        rejection: CreationRejection,
    ) -> Self {
        Self::StableChildCreationRejected {
            child,
            kind,
            rejection,
        }
    }

    #[must_use]
    pub const fn worker_factory_rejected(child: A::Nonce, index: usize) -> Self {
        Self::WorkerFactoryRejected { child, index }
    }

    #[must_use]
    pub const fn worker_creation_rejected(
        proxy: A::Nonce,
        worker: A::Nonce,
        kind: CreationKind<A::Nonce>,
        rejection: CreationRejection,
    ) -> Self {
        Self::WorkerCreationRejected {
            proxy,
            worker,
            kind,
            rejection,
        }
    }

    /// Terminal classification published for the supervisor incarnation.
    /// The complete diagnostic remains in this value and is not reconstructed
    /// from the terminal classification.
    #[must_use]
    pub const fn reason(self) -> SupervisionFailureReason {
        match self {
            Self::RestartDenied { denial, .. } => SupervisionFailureReason::RestartDenied(denial),
            Self::StableChildStopped { .. } => SupervisionFailureReason::StableChildStopped,
            Self::StableChildCreationRejected { rejection, .. } => {
                SupervisionFailureReason::StableChildCreationRejected(rejection)
            }
            Self::WorkerFactoryRejected { .. } => SupervisionFailureReason::WorkerFactoryRejected,
            Self::WorkerCreationRejected { rejection, .. } => {
                SupervisionFailureReason::WorkerCreationRejected(rejection)
            }
        }
    }
}

/// Pure policy applied when a supervisor cannot preserve its child topology.
pub type SupervisionFailureReaction<B> =
    fn(&B, &SupervisionFailure<crate::BehaviorAddr<B>>) -> Become;

/// Retire the failed slot and keep the supervisor alive.
///
/// # Errors
/// This supplied policy never returns a controlled behavior error.
pub fn retire_on_supervision_failure<B: Behavior>(
    _behavior: &B,
    _failure: &SupervisionFailure<crate::BehaviorAddr<B>>,
) -> Become {
    Step::Continue
}

/// Stop the supervisor with a typed failure outcome.
///
/// # Errors
/// This supplied policy never returns a controlled behavior error.
pub fn stop_on_supervision_failure<B: Behavior>(
    _behavior: &B,
    _failure: &SupervisionFailure<crate::BehaviorAddr<B>>,
) -> Become {
    Step::Stop(Stopped)
}
