//! Pure lifecycle domain for one stable proxy's worker incarnation.

use crate::{Address, ChildShutdownRejection, CreationKind, CreationRejection, CreationResolved};

/// The complete lifecycle state of the worker behind one stable proxy.
enum IncarnationState<N, C> {
    Dormant { initial: C },
    Installing { attempt: N, kind: CreationKind<N> },
    InstallingDuringShutdown { attempt: N, kind: CreationKind<N> },
    Running { incarnation: N },
    AwaitingStop { incarnation: N, replacement: C },
    ShuttingDown { incarnation: N },
    Vacant { last_installed: Option<N> },
}
/// A copyable observation of the lifecycle without owned child specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncarnationPhase<N> {
    Dormant,
    Installing { attempt: N, kind: CreationKind<N> },
    InstallingDuringShutdown { attempt: N, kind: CreationKind<N> },
    Running { incarnation: N },
    AwaitingStop { incarnation: N },
    ShuttingDown { incarnation: N },
    Vacant { last_installed: Option<N> },
}

/// A failure of one stable proxy's worker-incarnation lifecycle.
///
/// This is the single authority for incarnation-lifecycle failures;
/// the public façade exposes it as `ProxyError`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IncarnationError {
    #[error("the incarnation lifecycle was already initialized")]
    AlreadyInitialized,
    #[error("the incarnation creation-attempt sequence is exhausted")]
    AttemptSequenceExhausted,
    #[error("the current incarnation phase cannot accept a replacement")]
    ReplacementUnavailable,
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
pub(crate) enum IncarnationEffects<N, C, M, A>
where
    A: Address<Nonce = N>,
{
    None,
    Create(IncarnationCreation<N, C>),
    Deliver {
        incarnation: N,
        message: M,
    },
    Report(CreationResolved<A>),
    ReportAndShutdown {
        resolved: CreationResolved<A>,
        incarnation: N,
    },
    ReportAndStop(CreationResolved<A>),
    Shutdown(N),
    Stop,
}

/// Complete effect selected by one exact child-stop observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IncarnationStopEffects<N, C> {
    Stopped {
        incarnation: N,
    },
    StoppedAndCreate {
        incarnation: N,
        creation: IncarnationCreation<N, C>,
    },
}

/// Failure to consume one exact child-stop observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IncarnationStopError<N> {
    /// The observation does not name a child owned in the current phase.
    Unexpected(N),
    /// The matching transition could not begin its queued replacement.
    Lifecycle(IncarnationError),
}

/// Failure to consume one exact child-shutdown rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IncarnationShutdownError<N> {
    /// The rejection does not belong to the outstanding shutdown request.
    Unexpected {
        nonce: N,
        reason: ChildShutdownRejection,
    },
    /// The matching outstanding request was rejected by the interpreter.
    Rejected(ChildShutdownRejection),
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
            IncarnationState::InstallingDuringShutdown { attempt, kind } => {
                IncarnationPhase::InstallingDuringShutdown {
                    attempt: *attempt,
                    kind: *kind,
                }
            }
            IncarnationState::AwaitingStop { incarnation, .. } => IncarnationPhase::AwaitingStop {
                incarnation: *incarnation,
            },
            IncarnationState::Running { incarnation } => IncarnationPhase::Running {
                incarnation: *incarnation,
            },
            IncarnationState::ShuttingDown { incarnation } => IncarnationPhase::ShuttingDown {
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
    pub(crate) fn initialize<M, A: Address<Nonce = N>>(
        &mut self,
    ) -> Result<IncarnationEffects<N, C, M, A>, IncarnationError> {
        let previous = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: None,
            },
        );
        match previous {
            IncarnationState::Dormant { initial } => {
                match self.begin(initial, CreationKind::Birth) {
                    Ok(creation) => Ok(IncarnationEffects::Create(creation)),
                    Err((error, initial)) => {
                        self.state = IncarnationState::Dormant { initial };
                        Err(error)
                    }
                }
            }
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
    ) -> Result<IncarnationCreation<N, C>, (IncarnationError, C)> {
        let attempt = N::from(self.next_attempt);
        let Some(next_attempt) = self.next_attempt.checked_add(1) else {
            return Err((IncarnationError::AttemptSequenceExhausted, child));
        };
        self.next_attempt = next_attempt;
        self.state = IncarnationState::Installing { attempt, kind };
        Ok(IncarnationCreation::new(attempt, kind, child))
    }

    pub(crate) fn creation_resolved<M, A: Address<Nonce = N>>(
        &mut self,
        attempt: N,
        kind: CreationKind<N>,
        result: Result<A, CreationRejection>,
    ) -> Result<IncarnationEffects<N, C, M, A>, CreationResolved<A>> {
        let observed = CreationResolved::new(attempt, kind, result);
        let (pending, pending_kind, shutting_down) = match self.state {
            IncarnationState::Installing { attempt, kind } => (attempt, kind, false),
            IncarnationState::InstallingDuringShutdown { attempt, kind } => (attempt, kind, true),
            _ => return Err(observed),
        };
        if attempt != pending || kind != pending_kind {
            return Err(observed);
        }
        self.state = match result {
            Ok(_) if shutting_down => IncarnationState::ShuttingDown {
                incarnation: attempt,
            },
            Ok(_) => IncarnationState::Running {
                incarnation: attempt,
            },
            Err(_) => IncarnationState::Vacant {
                last_installed: match kind {
                    CreationKind::Birth => None,
                    CreationKind::ReplacementIncarnation { replaces } => Some(replaces),
                },
            },
        };
        Ok(match (shutting_down, result) {
            (true, Ok(_)) => IncarnationEffects::ReportAndShutdown {
                resolved: observed,
                incarnation: attempt,
            },
            (true, Err(_)) => IncarnationEffects::ReportAndStop(observed),
            (false, _) => IncarnationEffects::Report(observed),
        })
    }

    pub(crate) fn child_stopped(
        &mut self,
        stopped: N,
    ) -> Result<IncarnationStopEffects<N, C>, IncarnationStopError<N>> {
        if let IncarnationState::ShuttingDown { incarnation } = self.state {
            if stopped == incarnation {
                self.state = IncarnationState::Vacant {
                    last_installed: Some(incarnation),
                };
                return Ok(IncarnationStopEffects::Stopped { incarnation });
            }
            return Err(IncarnationStopError::Unexpected(stopped));
        }
        let incarnation = match self.state {
            IncarnationState::Running { incarnation }
            | IncarnationState::AwaitingStop { incarnation, .. } => incarnation,
            _ => return Err(IncarnationStopError::Unexpected(stopped)),
        };
        if stopped != incarnation {
            return Err(IncarnationStopError::Unexpected(stopped));
        }
        let previous = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: Some(incarnation),
            },
        );
        match previous {
            IncarnationState::Running { incarnation } => {
                Ok(IncarnationStopEffects::Stopped { incarnation })
            }
            IncarnationState::AwaitingStop {
                incarnation,
                replacement,
            } => {
                let creation = match self.begin(
                    replacement,
                    CreationKind::ReplacementIncarnation {
                        replaces: incarnation,
                    },
                ) {
                    Ok(creation) => creation,
                    Err((error, replacement)) => {
                        self.state = IncarnationState::AwaitingStop {
                            incarnation,
                            replacement,
                        };
                        return Err(IncarnationStopError::Lifecycle(error));
                    }
                };
                Ok(IncarnationStopEffects::StoppedAndCreate {
                    incarnation,
                    creation,
                })
            }
            state => {
                self.state = state;
                Err(IncarnationStopError::Unexpected(stopped))
            }
        }
    }

    pub(crate) fn forward<M, A: Address<Nonce = N>>(
        &self,
        message: M,
    ) -> Result<IncarnationEffects<N, C, M, A>, (IncarnationPhase<N>, M)> {
        match self.state {
            IncarnationState::Running { incarnation }
            | IncarnationState::AwaitingStop { incarnation, .. } => {
                Ok(IncarnationEffects::Deliver {
                    incarnation,
                    message,
                })
            }
            IncarnationState::Dormant { .. }
            | IncarnationState::Installing { .. }
            | IncarnationState::InstallingDuringShutdown { .. }
            | IncarnationState::ShuttingDown { .. }
            | IncarnationState::Vacant { .. } => Err((self.phase(), message)),
        }
    }

    pub(crate) fn replace<M, A: Address<Nonce = N>>(
        &mut self,
        child: C,
    ) -> Result<IncarnationEffects<N, C, M, A>, (IncarnationError, C)> {
        Ok(match &mut self.state {
            IncarnationState::Running { incarnation } => {
                let incarnation = *incarnation;
                self.state = IncarnationState::AwaitingStop {
                    incarnation,
                    replacement: child,
                };
                IncarnationEffects::Shutdown(incarnation)
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
            | IncarnationState::InstallingDuringShutdown { .. }
            | IncarnationState::ShuttingDown { .. }
            | IncarnationState::AwaitingStop { .. }
            | IncarnationState::Vacant {
                last_installed: None,
            } => return Err((IncarnationError::ReplacementUnavailable, child)),
        })
    }

    /// Begin orderly shutdown without inferring whether an in-flight creation
    /// has been committed. A pending creation is resolved before the proxy
    /// decides whether a child must be shut down.
    pub(crate) fn shutdown<M, A: Address<Nonce = N>>(&mut self) -> IncarnationEffects<N, C, M, A> {
        let previous = core::mem::replace(
            &mut self.state,
            IncarnationState::Vacant {
                last_installed: None,
            },
        );
        match previous {
            IncarnationState::Dormant { .. } => IncarnationEffects::Stop,
            IncarnationState::Installing { attempt, kind } => {
                self.state = IncarnationState::InstallingDuringShutdown { attempt, kind };
                IncarnationEffects::None
            }
            IncarnationState::InstallingDuringShutdown { attempt, kind } => {
                self.state = IncarnationState::InstallingDuringShutdown { attempt, kind };
                IncarnationEffects::None
            }
            IncarnationState::Running { incarnation }
            | IncarnationState::AwaitingStop { incarnation, .. } => {
                self.state = IncarnationState::ShuttingDown { incarnation };
                IncarnationEffects::Shutdown(incarnation)
            }
            IncarnationState::ShuttingDown { incarnation } => {
                self.state = IncarnationState::ShuttingDown { incarnation };
                IncarnationEffects::None
            }
            IncarnationState::Vacant { last_installed } => {
                self.state = IncarnationState::Vacant { last_installed };
                IncarnationEffects::Stop
            }
        }
    }

    pub(crate) fn shutdown_complete_after(&self, stopped: N) -> bool {
        matches!(
            self.state,
            IncarnationState::ShuttingDown { incarnation } if incarnation == stopped
        )
    }

    /// Resolve only a rejection for the exact incarnation whose shutdown this
    /// lifecycle requested. `NotEstablished` proves that no owned child
    /// remains at that nonce; `AlreadyStopping` still requires the terminal
    /// observation.
    pub(crate) fn shutdown_rejected<M, A: Address<Nonce = N>>(
        &mut self,
        nonce: N,
        reason: ChildShutdownRejection,
    ) -> Result<IncarnationEffects<N, C, M, A>, IncarnationShutdownError<N>> {
        let IncarnationState::ShuttingDown { incarnation } = self.state else {
            return Err(IncarnationShutdownError::Unexpected { nonce, reason });
        };
        if nonce != incarnation {
            return Err(IncarnationShutdownError::Unexpected { nonce, reason });
        }
        match reason {
            ChildShutdownRejection::AlreadyStopping => Ok(IncarnationEffects::None),
            ChildShutdownRejection::NotEstablished => {
                Err(IncarnationShutdownError::Rejected(reason))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_attempt_preserves_successful_provenance() {
        let mut machine = Incarnation::<u64, &'static str>::new("first");
        machine.initialize::<(), crate::MailAddr>().unwrap();
        machine
            .creation_resolved::<(), crate::MailAddr>(
                0,
                CreationKind::Birth,
                Ok(crate::MailAddr(10)),
            )
            .unwrap();
        machine.replace::<(), crate::MailAddr>("second").unwrap();
        machine.child_stopped(0).unwrap();
        machine
            .creation_resolved::<(), crate::MailAddr>(
                1,
                CreationKind::ReplacementIncarnation { replaces: 0 },
                Err(CreationRejection::EnvironmentFailed),
            )
            .unwrap();

        assert_eq!(
            machine.phase(),
            IncarnationPhase::Vacant {
                last_installed: Some(0)
            }
        );
        let effects = machine.replace::<(), crate::MailAddr>("third").unwrap();
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
    fn exhausted_attempts_return_or_retain_the_exact_replacement() {
        let mut vacant = Incarnation::<u64, &'static str>::new("first");
        vacant.initialize::<(), crate::MailAddr>().unwrap();
        vacant
            .creation_resolved::<(), crate::MailAddr>(
                0,
                CreationKind::Birth,
                Ok(crate::MailAddr(10)),
            )
            .unwrap();
        vacant.replace::<(), crate::MailAddr>("failed").unwrap();
        vacant.child_stopped(0).unwrap();
        vacant
            .creation_resolved::<(), crate::MailAddr>(
                1,
                CreationKind::ReplacementIncarnation { replaces: 0 },
                Err(CreationRejection::EnvironmentFailed),
            )
            .unwrap();
        vacant.next_attempt = u64::MAX;
        let error = vacant.replace::<(), crate::MailAddr>("replacement");
        assert_eq!(
            error,
            Err((IncarnationError::AttemptSequenceExhausted, "replacement"))
        );
        assert_eq!(
            vacant.phase(),
            IncarnationPhase::Vacant {
                last_installed: Some(0),
            }
        );

        let mut queued = Incarnation::<u64, &'static str>::new("first");
        queued.initialize::<(), crate::MailAddr>().unwrap();
        queued
            .creation_resolved::<(), crate::MailAddr>(
                0,
                CreationKind::Birth,
                Ok(crate::MailAddr(10)),
            )
            .unwrap();
        queued.replace::<(), crate::MailAddr>("queued").unwrap();
        queued.next_attempt = u64::MAX;
        assert_eq!(
            queued.child_stopped(0),
            Err(IncarnationStopError::Lifecycle(
                IncarnationError::AttemptSequenceExhausted
            ))
        );
        assert_eq!(
            queued.phase(),
            IncarnationPhase::AwaitingStop { incarnation: 0 }
        );

        queued.next_attempt = 1;
        let IncarnationStopEffects::StoppedAndCreate { creation, .. } =
            queued.child_stopped(0).unwrap()
        else {
            panic!("the retained replacement was not available for a later attempt");
        };
        assert_eq!(creation.child, "queued");
    }

    #[test]
    fn stale_creation_is_returned_exactly_without_advancing_installation() {
        let mut machine = Incarnation::<u64, ()>::new(());
        machine.initialize::<(), crate::MailAddr>().unwrap();
        let effects = machine.creation_resolved::<(), crate::MailAddr>(
            9,
            CreationKind::Birth,
            Ok(crate::MailAddr(90)),
        );
        assert_eq!(
            effects,
            Err(CreationResolved::birth(9, crate::MailAddr(90)))
        );
        assert_eq!(
            machine.phase(),
            IncarnationPhase::Installing {
                attempt: 0,
                kind: CreationKind::Birth
            }
        );
    }
}
