//! Pure lifecycle domain for one stable proxy's worker incarnation.

use crate::{CreationKind, CreationRejection, CreationResolved};

/// The complete lifecycle state of the worker behind one stable proxy.
enum IncarnationState<N, C> {
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

/// A failure of one stable proxy's worker-incarnation lifecycle.
///
/// This is the single authority for the two incarnation-lifecycle failures;
/// the public façade exposes it as `ProxyError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IncarnationError {
    #[error("the incarnation lifecycle was already initialized")]
    AlreadyInitialized,
    #[error("the incarnation creation-attempt sequence is exhausted")]
    AttemptSequenceExhausted,
}

/// A fresh child creation selected by the lifecycle transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IncarnationCreation<N, C> {
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

/// The single effect selected by one lifecycle transition.
///
/// A transition emits at most one effect; [`IncarnationEffects::None`] is the
/// explicit empty transition. The combined stop-and-replace case is carried by
/// [`IncarnationStopEffects`], so stop provenance and a queued replacement are
/// never reconstructed from unrelated optional fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IncarnationEffects<N, C, M> {
    None,
    Create(IncarnationCreation<N, C>),
    Deliver { incarnation: N, message: M },
    Report(CreationResolved<N>),
}

/// Effects of accepting an exact child-stop observation.
///
/// This is distinct from [`IncarnationEffects`] so the adapter never has to
/// reconstruct stop provenance from an optional, unrelated input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IncarnationStopEffects<N, C> {
    pub creation: Option<IncarnationCreation<N, C>>,
    pub stopped: Option<N>,
}

/// The typed state machine for one proxy's sequence of fresh incarnations.
pub(crate) struct Incarnation<N, C> {
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
    pub(crate) fn initialize<M>(
        &mut self,
    ) -> Result<IncarnationEffects<N, C, M>, IncarnationError> {
        let previous = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: None,
            },
        );
        match previous {
            IncarnationState::Dormant { initial } => self
                .begin(initial, CreationKind::Birth)
                .map(IncarnationEffects::Create),
            state => {
                self.state = state;
                Err(IncarnationError::AlreadyInitialized)
            }
        }
    }

    fn begin(
        &mut self,
        child: C,
        kind: CreationKind<N>,
    ) -> Result<IncarnationCreation<N, C>, IncarnationError> {
        let attempt = N::from(self.next_attempt);
        self.next_attempt = self
            .next_attempt
            .checked_add(1)
            .ok_or(IncarnationError::AttemptSequenceExhausted)?;
        self.state = IncarnationState::Installing { attempt, kind };
        Ok(IncarnationCreation::new(attempt, kind, child))
    }

    pub(crate) fn creation_resolved<M>(
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
            return IncarnationEffects::None;
        };
        if attempt != pending || kind != pending_kind {
            return IncarnationEffects::None;
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
        IncarnationEffects::Report(CreationResolved::new(attempt, kind, result))
    }

    pub(crate) fn child_stopped(
        &mut self,
        stopped: N,
    ) -> Result<IncarnationStopEffects<N, C>, IncarnationError> {
        let IncarnationState::Running { incarnation, .. } = self.state else {
            return Ok(IncarnationStopEffects {
                creation: None,
                stopped: None,
            });
        };
        if stopped != incarnation {
            return Ok(IncarnationStopEffects {
                creation: None,
                stopped: None,
            });
        }
        let previous = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: Some(incarnation),
            },
        );
        let IncarnationState::Running {
            incarnation,
            queued_replacement,
        } = previous
        else {
            self.state = previous;
            return Ok(IncarnationStopEffects {
                creation: None,
                stopped: None,
            });
        };
        let creation = match queued_replacement {
            Some(child) => Some(self.begin(
                child,
                CreationKind::ReplacementIncarnation {
                    replaces: incarnation,
                },
            )?),
            None => None,
        };
        Ok(IncarnationStopEffects {
            creation,
            stopped: Some(incarnation),
        })
    }

    pub(crate) fn forward<M>(&self, message: M) -> IncarnationEffects<N, C, M> {
        match self.state {
            IncarnationState::Running { incarnation, .. } => IncarnationEffects::Deliver {
                incarnation,
                message,
            },
            IncarnationState::Dormant { .. }
            | IncarnationState::Installing { .. }
            | IncarnationState::Vacant { .. } => IncarnationEffects::None,
        }
    }

    pub(crate) fn replace<M>(
        &mut self,
        child: C,
    ) -> Result<IncarnationEffects<N, C, M>, IncarnationError> {
        Ok(match &mut self.state {
            IncarnationState::Running {
                queued_replacement: queued_replacement @ None,
                ..
            } => {
                *queued_replacement = Some(child);
                IncarnationEffects::None
            }
            IncarnationState::Vacant {
                last_installed: Some(last),
            } => {
                let replaces = *last;
                return self
                    .begin(child, CreationKind::ReplacementIncarnation { replaces })
                    .map(IncarnationEffects::Create);
            }
            IncarnationState::Dormant { .. }
            | IncarnationState::Installing { .. }
            | IncarnationState::Running { .. }
            | IncarnationState::Vacant {
                last_installed: None,
            } => IncarnationEffects::None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_attempt_preserves_successful_provenance() {
        let mut machine = Incarnation::<u64, &'static str>::new("first");
        machine.initialize::<()>().unwrap();
        machine.creation_resolved::<()>(0, CreationKind::Birth, Ok(()));
        machine.replace::<()>("second").unwrap();
        machine.child_stopped(0).unwrap();
        machine.creation_resolved::<()>(
            1,
            CreationKind::ReplacementIncarnation { replaces: 0 },
            Err(CreationRejection::EnvironmentFailed),
        );

        assert_eq!(
            machine.phase(),
            IncarnationPhase::Vacant {
                last_installed: Some(0)
            }
        );
        let effects = machine.replace::<()>("third").unwrap();
        let IncarnationEffects::Create(creation) = effects else {
            panic!("replace on a vacant slot must begin a creation");
        };
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
        let effects = machine.creation_resolved::<()>(9, CreationKind::Birth, Ok(()));
        assert_eq!(effects, IncarnationEffects::None);
        assert_eq!(
            machine.phase(),
            IncarnationPhase::Installing {
                attempt: 0,
                kind: CreationKind::Birth
            }
        );
    }
}
