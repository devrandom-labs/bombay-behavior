//! Fixed-supervisor naming of shared restart denials and proposals.

use core::num::NonZeroUsize;
use std::time::Instant;

use super::super::super::restart::{
    RestartAdmission as SharedRestartAdmission, RestartDenial, RestartProposal, RestartRelease,
    RestartReleaseFailure,
};
use super::super::RestartLimit;
use super::super::role::MemberRole;
use super::RestartBudget;

/// Exact reason an otherwise prepared fixed-roster recovery was denied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryDenialReason {
    /// The supervisor cannot issue another non-reused recovery ticket.
    RecoveryTicketsExhausted,
    /// The supervisor cannot issue another non-reused restart-timer identity.
    RestartTimersExhausted,
    /// The active inclusive-window charge plus this request exceeds the limit.
    RestartLimitReached {
        /// Replacement attempts still active inside the configured window.
        active: u32,
        /// Positive replacement count requested by this complete recovery.
        requested: NonZeroUsize,
        /// Configured maximum active replacement count.
        maximum: u32,
    },
    /// The interpreter-authored stop time regressed.
    ClockRegressed {
        /// Latest time accepted by the restart history.
        previous: Instant,
        /// Regressing time carried by the triggering worker stop.
        observed: Instant,
    },
    /// The triggering role's checked lifetime recovery count cannot advance.
    RecoveryCountExhausted {
        /// Greatest admitted recovery count representable by this policy.
        admitted: u32,
    },
    /// Checked release-delay calculation failed.
    ReleaseCalculationFailed(RestartReleaseFailure),
}

pub(in super::super) enum RestartAdmission<'a> {
    Proposed(RestartProposal<'a>),
    Denied {
        budget: RestartBudget,
        reason: RecoveryDenialReason,
    },
}

impl RecoveryDenialReason {
    fn restart(reason: RestartDenial) -> Self {
        match reason {
            RestartDenial::RestartLimitReached {
                active,
                requested,
                maximum,
            } => Self::RestartLimitReached {
                active,
                requested,
                maximum,
            },
            RestartDenial::ClockRegressed { previous, observed } => {
                Self::ClockRegressed { previous, observed }
            }
            RestartDenial::RecoveryCountExhausted { admitted } => {
                Self::RecoveryCountExhausted { admitted }
            }
            RestartDenial::ReleaseCalculationFailed(reason) => {
                Self::ReleaseCalculationFailed(reason)
            }
        }
    }
}

pub(in super::super) fn admit_restart<'a, Role>(
    role: &'a mut MemberRole<Role>,
    budget: RestartBudget,
    limit: RestartLimit,
    release: RestartRelease,
    observed_at: Instant,
    replacements: NonZeroUsize,
) -> RestartAdmission<'a> {
    match super::super::super::restart::admit_restart(
        role.recovery_count(),
        budget,
        limit,
        release,
        observed_at,
        replacements,
    ) {
        SharedRestartAdmission::Proposed(proposal) => RestartAdmission::Proposed(proposal),
        SharedRestartAdmission::Denied { budget, reason } => RestartAdmission::Denied {
            budget,
            reason: RecoveryDenialReason::restart(reason),
        },
    }
}
