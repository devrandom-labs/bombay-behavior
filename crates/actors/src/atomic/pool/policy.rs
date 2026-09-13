//! Policies shared by direct-worker pools.

use std::mem;

use behavior::Never;

use super::super::worker::StopKind;
use super::super::{RestartLimit, RestartRelease};

/// Maximum number of accepted jobs that may wait in one admission queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BacklogCapacity {
    maximum: usize,
}

impl BacklogCapacity {
    /// Select the waiting-job limit. Zero permits only immediate assignment.
    #[must_use]
    pub const fn new(maximum: usize) -> Self {
        Self { maximum }
    }

    pub(in crate::atomic) const fn maximum(self) -> usize {
        self.maximum
    }
}

/// Customer disposition when an assigned worker stops.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interruption {
    /// Return the accepted job to its customer.
    Fail,
    /// Reinsert the accepted job at its immutable admission position.
    Retry,
}

/// Pool disposition when one worker role cannot recover.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolFailureReaction {
    /// Retire only the unavailable role and keep serving through other workers.
    RetireRole,
    /// Close the complete pool and begin actor-graph retirement.
    StopPool,
}

/// Direct-worker eligibility, source, restart policy, and failure reaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolRecovery<Source> {
    /// Recover after normal and abnormal worker stops.
    Permanent {
        /// Typed capability used by Bombay to prepare a replacement worker.
        source: Source,
        /// Sliding-window restart admission limit.
        limit: RestartLimit,
        /// Immediate or delayed release policy.
        release: RestartRelease,
        /// Disposition when preparation or restart admission fails.
        failure: PoolFailureReaction,
    },
    /// Recover only after abnormal worker stops.
    Transient {
        /// Typed capability used by Bombay to prepare a replacement worker.
        source: Source,
        /// Sliding-window restart admission limit.
        limit: RestartLimit,
        /// Immediate or delayed release policy.
        release: RestartRelease,
        /// Disposition when preparation or restart admission fails.
        failure: PoolFailureReaction,
    },
    /// Never create a replacement worker.
    Temporary {
        /// Disposition after the role retires.
        failure: PoolFailureReaction,
    },
}

impl<Source> PoolRecovery<Source> {
    /// Recover after every exact worker stop.
    #[must_use]
    pub const fn permanent(
        source: Source,
        limit: RestartLimit,
        release: RestartRelease,
        failure: PoolFailureReaction,
    ) -> Self {
        Self::Permanent {
            source,
            limit,
            release,
            failure,
        }
    }

    /// Recover only after abnormal worker stops.
    #[must_use]
    pub const fn transient(
        source: Source,
        limit: RestartLimit,
        release: RestartRelease,
        failure: PoolFailureReaction,
    ) -> Self {
        Self::Transient {
            source,
            limit,
            release,
            failure,
        }
    }
}

impl PoolRecovery<Never> {
    /// Retire stopped workers without a source placeholder.
    #[must_use]
    pub const fn temporary(failure: PoolFailureReaction) -> Self {
        Self::Temporary { failure }
    }
}

pub(in crate::atomic) enum WorkerRecoverySource<Source> {
    Available(Source),
    AwaitingReturn,
}

pub(in crate::atomic) enum WorkerRecoveryDecision<Source> {
    PrepareWorker(Source),
    WaitForSource,
    RetireRole,
    StopPool,
}

pub(in crate::atomic) enum PoolRecoveryState<Source> {
    Permanent {
        source: WorkerRecoverySource<Source>,
        limit: RestartLimit,
        release: RestartRelease,
        failure: PoolFailureReaction,
    },
    Transient {
        source: WorkerRecoverySource<Source>,
        limit: RestartLimit,
        release: RestartRelease,
        failure: PoolFailureReaction,
    },
    Temporary {
        failure: PoolFailureReaction,
    },
}

impl<Source> From<PoolRecovery<Source>> for PoolRecoveryState<Source> {
    fn from(recovery: PoolRecovery<Source>) -> Self {
        match recovery {
            PoolRecovery::Permanent {
                source,
                limit,
                release,
                failure,
            } => Self::Permanent {
                source: WorkerRecoverySource::Available(source),
                limit,
                release,
                failure,
            },
            PoolRecovery::Transient {
                source,
                limit,
                release,
                failure,
            } => Self::Transient {
                source: WorkerRecoverySource::Available(source),
                limit,
                release,
                failure,
            },
            PoolRecovery::Temporary { failure } => Self::Temporary { failure },
        }
    }
}

impl<Source> PoolRecoveryState<Source> {
    pub(in crate::atomic) fn decide(&mut self, stop: StopKind) -> WorkerRecoveryDecision<Source> {
        match (self, stop) {
            (Self::Permanent { source, .. }, _)
            | (Self::Transient { source, .. }, StopKind::Abnormal) => source.claim(),
            (Self::Transient { failure, .. }, StopKind::Normal)
            | (Self::Temporary { failure }, _) => match failure {
                PoolFailureReaction::RetireRole => WorkerRecoveryDecision::RetireRole,
                PoolFailureReaction::StopPool => WorkerRecoveryDecision::StopPool,
            },
        }
    }

    pub(in crate::atomic) const fn failure(&self) -> PoolFailureReaction {
        match self {
            Self::Permanent { failure, .. }
            | Self::Transient { failure, .. }
            | Self::Temporary { failure } => *failure,
        }
    }

    pub(in crate::atomic) fn restore_source(&mut self, returned: Source) -> Result<(), Source> {
        let source = match self {
            Self::Permanent { source, .. } | Self::Transient { source, .. } => source,
            Self::Temporary { .. } => return Err(returned),
        };
        match source {
            WorkerRecoverySource::AwaitingReturn => {
                *source = WorkerRecoverySource::Available(returned);
                Ok(())
            }
            WorkerRecoverySource::Available(_) => Err(returned),
        }
    }

    pub(in crate::atomic) fn claim_waiting_source(&mut self) -> Option<Source> {
        match self {
            Self::Permanent { source, .. } | Self::Transient { source, .. } => {
                source.claim_source()
            }
            Self::Temporary { .. } => None,
        }
    }
}

impl<Source> WorkerRecoverySource<Source> {
    fn claim(&mut self) -> WorkerRecoveryDecision<Source> {
        match self.claim_source() {
            Some(source) => WorkerRecoveryDecision::PrepareWorker(source),
            None => WorkerRecoveryDecision::WaitForSource,
        }
    }

    fn claim_source(&mut self) -> Option<Source> {
        match mem::replace(self, Self::AwaitingReturn) {
            Self::Available(source) => Some(source),
            Self::AwaitingReturn => None,
        }
    }
}
