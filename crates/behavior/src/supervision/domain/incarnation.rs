//! Pure lifecycle domain for one stable proxy's worker incarnation.

use crate::{CreationKind, CreationRejection};

/// The complete lifecycle state of the worker behind one stable proxy.
pub enum IncarnationState<N, C> {
    Dormant {
        initial: C,
    },
    Installing {
        attempt: N,
        kind: CreationKind<N>,
    },
    Running {
        incarnation: N,
        queued_replacement: Option<C>,
    },
    Vacant {
        last_installed: Option<N>,
    },
}

/// A copyable observation of the lifecycle without owned child specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncarnationPhase<N> {
    Dormant,
    Installing { attempt: N, kind: CreationKind<N> },
    Running { incarnation: N },
    AwaitingStop { incarnation: N },
    Vacant { last_installed: Option<N> },
}

/// Inputs understood by the incarnation lifecycle domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncarnationInput<N, C, M> {
    CreationResolved {
        attempt: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    },
    ChildStopped {
        incarnation: N,
    },
    Forward(M),
    Replace(C),
}

impl<N, C, M> IncarnationInput<N, C, M> {
    #[must_use]
    pub const fn creation_resolved(
        attempt: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self::CreationResolved {
            attempt,
            kind,
            result,
        }
    }

    #[must_use]
    pub const fn child_stopped(incarnation: N) -> Self {
        Self::ChildStopped { incarnation }
    }

    #[must_use]
    pub const fn forward(message: M) -> Self {
        Self::Forward(message)
    }

    #[must_use]
    pub const fn replace(child: C) -> Self {
        Self::Replace(child)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncarnationError {
    AlreadyInitialized,
}

/// A fresh child creation selected by the lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncarnationCreation<N, C> {
    pub attempt: N,
    pub kind: CreationKind<N>,
    pub child: C,
}

impl<N, C> IncarnationCreation<N, C> {
    #[must_use]
    pub const fn new(attempt: N, kind: CreationKind<N>, child: C) -> Self {
        Self {
            attempt,
            kind,
            child,
        }
    }
}

/// A lifecycle fact to report to the stable proxy's parent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncarnationReport<N> {
    CreationResolved {
        incarnation: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    },
    Stopped {
        incarnation: N,
    },
}

impl<N> IncarnationReport<N> {
    #[must_use]
    pub const fn creation_resolved(
        incarnation: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    ) -> Self {
        Self::CreationResolved {
            incarnation,
            kind,
            result,
        }
    }

    #[must_use]
    pub const fn stopped(incarnation: N) -> Self {
        Self::Stopped { incarnation }
    }
}

/// Independent effects selected by one lifecycle transition.
///
/// `creation` and `reports` are independent because accepting an exact stop
/// can both report that stop and begin an already-queued replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncarnationEffects<N, C, M> {
    pub creation: Option<IncarnationCreation<N, C>>,
    pub delivery: Option<(N, M)>,
    pub reports: Vec<IncarnationReport<N>>,
}

impl<N, C, M> IncarnationEffects<N, C, M> {
    #[must_use]
    pub fn new(
        creation: Option<IncarnationCreation<N, C>>,
        delivery: Option<(N, M)>,
        reports: Vec<IncarnationReport<N>>,
    ) -> Self {
        Self {
            creation,
            delivery,
            reports,
        }
    }

    #[must_use]
    pub fn none() -> Self {
        Self {
            creation: None,
            delivery: None,
            reports: Vec::new(),
        }
    }
}

/// The typed state machine for one proxy's sequence of fresh incarnations.
pub struct Incarnation<N, C> {
    state: IncarnationState<N, C>,
    next_attempt: u64,
}

impl<N, C> Incarnation<N, C> {
    #[must_use]
    pub const fn new(initial: C) -> Self {
        Self {
            state: IncarnationState::Dormant { initial },
            next_attempt: 0,
        }
    }
}

impl<N: Copy, C> Incarnation<N, C> {
    #[must_use]
    pub const fn phase(&self) -> IncarnationPhase<N> {
        match &self.state {
            IncarnationState::Dormant { .. } => IncarnationPhase::Dormant,
            IncarnationState::Installing { attempt, kind } => IncarnationPhase::Installing {
                attempt: *attempt,
                kind: *kind,
            },
            IncarnationState::Running {
                incarnation,
                queued_replacement: Some(_),
            } => IncarnationPhase::AwaitingStop {
                incarnation: *incarnation,
            },
            IncarnationState::Running {
                incarnation,
                queued_replacement: None,
            } => IncarnationPhase::Running {
                incarnation: *incarnation,
            },
            IncarnationState::Vacant { last_installed } => IncarnationPhase::Vacant {
                last_installed: *last_installed,
            },
        }
    }
}

impl<N: Copy + From<u64> + PartialEq, C> Incarnation<N, C> {
    /// Emit the initial fresh creation exactly once.
    ///
    /// # Errors
    /// Returns [`IncarnationError::AlreadyInitialized`] after leaving
    /// `Dormant`.
    pub fn initialize<M>(&mut self) -> Result<IncarnationEffects<N, C, M>, IncarnationError> {
        let IncarnationState::Dormant { .. } = self.state else {
            return Err(IncarnationError::AlreadyInitialized);
        };
        let IncarnationState::Dormant { initial } = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: None,
            },
        ) else {
            unreachable!("state was matched as dormant")
        };
        Ok(self.begin(initial, CreationKind::Birth))
    }

    /// Apply one input and return every selected effect explicitly.
    ///
    /// The fold is deterministic and performs no runtime work. Inputs that do
    /// not belong to the current exact incarnation are deliberately inert.
    pub fn transition<M>(
        &mut self,
        input: IncarnationInput<N, C, M>,
    ) -> IncarnationEffects<N, C, M> {
        match input {
            IncarnationInput::CreationResolved {
                attempt,
                kind,
                result,
            } => self.creation_resolved(attempt, kind, result),
            IncarnationInput::ChildStopped { incarnation } => self.child_stopped(incarnation),
            IncarnationInput::Forward(message) => self.forward(message),
            IncarnationInput::Replace(child) => self.replace(child),
        }
    }

    fn begin<M>(&mut self, child: C, kind: CreationKind<N>) -> IncarnationEffects<N, C, M> {
        let attempt = N::from(self.next_attempt);
        self.next_attempt = self
            .next_attempt
            .checked_add(1)
            .expect("incarnation creation nonce exhausted");
        self.state = IncarnationState::Installing { attempt, kind };
        IncarnationEffects::new(
            Some(IncarnationCreation::new(attempt, kind, child)),
            None,
            Vec::new(),
        )
    }

    fn creation_resolved<M>(
        &mut self,
        attempt: N,
        kind: CreationKind<N>,
        result: Result<(), CreationRejection>,
    ) -> IncarnationEffects<N, C, M> {
        let IncarnationState::Installing {
            attempt: pending,
            kind: pending_kind,
        } = self.state
        else {
            return IncarnationEffects::none();
        };
        if attempt != pending || kind != pending_kind {
            return IncarnationEffects::none();
        }
        self.state = match result {
            Ok(()) => IncarnationState::Running {
                incarnation: attempt,
                queued_replacement: None,
            },
            Err(_) => IncarnationState::Vacant {
                last_installed: match kind {
                    CreationKind::Birth => None,
                    CreationKind::ReplacementIncarnation { replaces } => Some(replaces),
                },
            },
        };
        IncarnationEffects::new(
            None,
            None,
            vec![IncarnationReport::creation_resolved(attempt, kind, result)],
        )
    }

    fn child_stopped<M>(&mut self, stopped: N) -> IncarnationEffects<N, C, M> {
        let IncarnationState::Running { incarnation, .. } = self.state else {
            return IncarnationEffects::none();
        };
        if stopped != incarnation {
            return IncarnationEffects::none();
        }
        let IncarnationState::Running {
            incarnation,
            queued_replacement,
        } = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: Some(incarnation),
            },
        )
        else {
            unreachable!("state was matched as running")
        };
        let mut effects = match queued_replacement {
            Some(child) => self.begin(
                child,
                CreationKind::ReplacementIncarnation {
                    replaces: incarnation,
                },
            ),
            None => IncarnationEffects::none(),
        };
        effects
            .reports
            .push(IncarnationReport::stopped(incarnation));
        effects
    }

    fn forward<M>(&self, message: M) -> IncarnationEffects<N, C, M> {
        let delivery = match self.state {
            IncarnationState::Running { incarnation, .. } => Some((incarnation, message)),
            IncarnationState::Dormant { .. }
            | IncarnationState::Installing { .. }
            | IncarnationState::Vacant { .. } => None,
        };
        IncarnationEffects::new(None, delivery, Vec::new())
    }

    fn replace<M>(&mut self, child: C) -> IncarnationEffects<N, C, M> {
        match &mut self.state {
            IncarnationState::Running {
                queued_replacement: queued_replacement @ None,
                ..
            } => {
                *queued_replacement = Some(child);
                IncarnationEffects::none()
            }
            IncarnationState::Vacant {
                last_installed: Some(last),
            } => {
                let replaces = *last;
                self.begin(child, CreationKind::ReplacementIncarnation { replaces })
            }
            IncarnationState::Dormant { .. }
            | IncarnationState::Installing { .. }
            | IncarnationState::Running { .. }
            | IncarnationState::Vacant {
                last_installed: None,
            } => IncarnationEffects::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_attempt_preserves_successful_provenance() {
        let mut machine = Incarnation::<u64, &'static str>::new("first");
        machine.initialize::<()>().unwrap();
        machine.transition::<()>(IncarnationInput::CreationResolved {
            attempt: 0,
            kind: CreationKind::Birth,
            result: Ok(()),
        });
        machine.transition::<()>(IncarnationInput::Replace("second"));
        machine.transition::<()>(IncarnationInput::ChildStopped { incarnation: 0 });
        machine.transition::<()>(IncarnationInput::CreationResolved {
            attempt: 1,
            kind: CreationKind::ReplacementIncarnation { replaces: 0 },
            result: Err(CreationRejection::EnvironmentFailed),
        });

        assert_eq!(
            machine.phase(),
            IncarnationPhase::Vacant {
                last_installed: Some(0)
            }
        );
        let effects = machine.transition::<()>(IncarnationInput::Replace("third"));
        let creation = effects.creation.expect("replacement begins");
        assert_eq!(creation.attempt, 2);
        assert_eq!(
            creation.kind,
            CreationKind::ReplacementIncarnation { replaces: 0 }
        );
    }

    #[test]
    fn stale_inputs_are_inert() {
        let mut machine = Incarnation::<u64, ()>::new(());
        machine.initialize::<()>().unwrap();
        let effects = machine.transition::<()>(IncarnationInput::CreationResolved {
            attempt: 9,
            kind: CreationKind::Birth,
            result: Ok(()),
        });
        assert!(effects.creation.is_none());
        assert!(effects.delivery.is_none());
        assert!(effects.reports.is_empty());
        assert_eq!(
            machine.phase(),
            IncarnationPhase::Installing {
                attempt: 0,
                kind: CreationKind::Birth
            }
        );
    }
}
