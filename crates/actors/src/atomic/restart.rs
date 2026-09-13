//! Checked restart admission shared by fixed supervision and direct pools.

use core::cmp::Ordering;
use core::num::{NonZeroU32, NonZeroUsize};
use core::time::Duration;
use std::time::Instant;

use thiserror::Error;

/// Sliding-window admission limit for automatic worker replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartLimit {
    maximum: u32,
    window: Duration,
}

impl RestartLimit {
    /// Define the admitted replacement count and inclusive time window.
    #[must_use]
    pub const fn new(maximum: u32, window: Duration) -> Self {
        Self { maximum, window }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RestartReleaseSchedule {
    Immediate,
    Constant {
        delay: Duration,
    },
    Linear {
        initial: Duration,
        maximum: Duration,
    },
    Exponential {
        initial: Duration,
        maximum: Duration,
    },
}

/// Checked delay schedule for successive automatic recoveries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartRelease {
    schedule: RestartReleaseSchedule,
}

/// Invalid delay configuration rejected before an aggregate exists.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RestartReleaseError {
    /// A delayed schedule used a zero duration.
    #[error("restart release delay must be positive")]
    ZeroDelay,
    /// A growing schedule capped its delay below the initial duration.
    #[error("restart release maximum must not be below its initial delay")]
    MaximumBelowInitial,
}

/// Exact automatic-release calculation failure.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RestartReleaseFailure {
    /// Checked delay arithmetic exceeded the representable duration.
    #[error("restart release delay arithmetic overflowed")]
    DurationOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::atomic) enum RecoveryRelease {
    Immediate,
    Delayed(Duration),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::atomic) struct RecoveryCount {
    admitted: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::atomic) struct RecoveryOrdinal(NonZeroU32);

pub(in crate::atomic) struct ProposedRecovery<'a> {
    count: &'a mut RecoveryCount,
    ordinal: RecoveryOrdinal,
}

pub(in crate::atomic) enum RecoveryProposal<'a> {
    Available(ProposedRecovery<'a>),
    Exhausted { admitted: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RestartCharge {
    observed_at: Instant,
    replacements: NonZeroU32,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::atomic) struct RestartBudget {
    charges: Vec<RestartCharge>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::atomic) enum RestartDenial {
    RestartLimitReached {
        active: u32,
        requested: NonZeroUsize,
        maximum: u32,
    },
    ClockRegressed {
        previous: Instant,
        observed: Instant,
    },
    RecoveryCountExhausted {
        admitted: u32,
    },
    ReleaseCalculationFailed(RestartReleaseFailure),
}

pub(in crate::atomic) enum RestartAdmission<'a> {
    Proposed(RestartProposal<'a>),
    Denied {
        budget: RestartBudget,
        reason: RestartDenial,
    },
}

pub(in crate::atomic) struct RestartProposal<'a> {
    recovery: ProposedRecovery<'a>,
    budget: RestartBudget,
    proposed_budget: RestartBudget,
    release: RecoveryRelease,
}

enum BudgetCheck {
    Admitted(RestartBudget),
    Denied(RestartDenial),
}

impl RecoveryCount {
    pub(in crate::atomic) const fn new() -> Self {
        Self { admitted: 0 }
    }

    pub(in crate::atomic) fn propose(&mut self) -> RecoveryProposal<'_> {
        match self.admitted.checked_add(1).and_then(NonZeroU32::new) {
            Some(next) => RecoveryProposal::Available(ProposedRecovery {
                count: self,
                ordinal: RecoveryOrdinal(next),
            }),
            None => RecoveryProposal::Exhausted {
                admitted: self.admitted,
            },
        }
    }

    #[cfg(test)]
    const fn from_admitted(admitted: u32) -> Self {
        Self { admitted }
    }

    #[cfg(test)]
    const fn admitted(self) -> u32 {
        self.admitted
    }
}

impl RecoveryOrdinal {
    pub(in crate::atomic) const fn get(self) -> u32 {
        self.0.get()
    }
}

impl ProposedRecovery<'_> {
    pub(in crate::atomic) const fn ordinal(&self) -> RecoveryOrdinal {
        self.ordinal
    }

    pub(in crate::atomic) fn accept(self) {
        self.count.admitted = self.ordinal.get();
    }

    pub(in crate::atomic) fn decline(self) {}
}

impl RestartBudget {
    pub(in crate::atomic) const fn empty() -> Self {
        Self {
            charges: Vec::new(),
        }
    }

    fn preview(
        &self,
        limit: RestartLimit,
        observed_at: Instant,
        replacements: NonZeroUsize,
    ) -> BudgetCheck {
        if let Some(previous) = self.charges.last().map(|charge| charge.observed_at) {
            match observed_at.cmp(&previous) {
                Ordering::Less => {
                    return BudgetCheck::Denied(RestartDenial::ClockRegressed {
                        previous,
                        observed: observed_at,
                    });
                }
                Ordering::Equal | Ordering::Greater => {}
            }
        }

        let cutoff = observed_at.checked_sub(limit.window);
        let first_active = cutoff.map_or(0, |cutoff| {
            self.charges
                .partition_point(|charge| charge.observed_at < cutoff)
        });
        let active_charges = &self.charges[first_active..];
        let active = active_charges
            .iter()
            .try_fold(0_u32, |active, charge| {
                active.checked_add(charge.replacements.get())
            })
            .expect("committed restart charges are bounded by their accepted u32 limit");

        let charge = match u32::try_from(replacements.get())
            .ok()
            .and_then(NonZeroU32::new)
        {
            Some(charge) => charge,
            None => {
                return BudgetCheck::Denied(RestartDenial::RestartLimitReached {
                    active,
                    requested: replacements,
                    maximum: limit.maximum,
                });
            }
        };

        let total = match active.checked_add(charge.get()) {
            Some(total) => total,
            None => {
                return BudgetCheck::Denied(RestartDenial::RestartLimitReached {
                    active,
                    requested: replacements,
                    maximum: limit.maximum,
                });
            }
        };
        match total.cmp(&limit.maximum) {
            Ordering::Greater => {
                return BudgetCheck::Denied(RestartDenial::RestartLimitReached {
                    active,
                    requested: replacements,
                    maximum: limit.maximum,
                });
            }
            Ordering::Less | Ordering::Equal => {}
        }

        let mut charges = Vec::with_capacity(active_charges.len() + 1);
        charges.extend_from_slice(active_charges);
        charges.push(RestartCharge {
            observed_at,
            replacements: charge,
        });
        BudgetCheck::Admitted(Self { charges })
    }

    #[cfg(test)]
    fn latest(&self) -> Option<(Instant, NonZeroU32)> {
        self.charges
            .last()
            .map(|charge| (charge.observed_at, charge.replacements))
    }
}

impl RestartProposal<'_> {
    #[cfg(test)]
    pub(in crate::atomic) const fn ordinal(&self) -> RecoveryOrdinal {
        self.recovery.ordinal()
    }

    pub(in crate::atomic) const fn release(&self) -> RecoveryRelease {
        self.release
    }

    pub(in crate::atomic) fn accept(self) -> RestartBudget {
        self.recovery.accept();
        self.proposed_budget
    }

    pub(in crate::atomic) fn decline(self) -> RestartBudget {
        self.recovery.decline();
        self.budget
    }
}

impl RestartRelease {
    /// Release every eligible recovery without a timer prerequisite.
    #[must_use]
    pub const fn immediate() -> Self {
        Self {
            schedule: RestartReleaseSchedule::Immediate,
        }
    }

    /// Use one positive delay for every eligible recovery.
    pub fn constant(delay: Duration) -> Result<Self, RestartReleaseError> {
        let delay = Self::positive(delay)?;
        Ok(Self {
            schedule: RestartReleaseSchedule::Constant { delay },
        })
    }

    /// Increase delay linearly from a positive initial value up to a maximum.
    pub fn linear(initial: Duration, maximum: Duration) -> Result<Self, RestartReleaseError> {
        let (initial, maximum) = Self::growth(initial, maximum)?;
        Ok(Self {
            schedule: RestartReleaseSchedule::Linear { initial, maximum },
        })
    }

    /// Increase delay exponentially from a positive initial value up to a maximum.
    pub fn exponential(initial: Duration, maximum: Duration) -> Result<Self, RestartReleaseError> {
        let (initial, maximum) = Self::growth(initial, maximum)?;
        Ok(Self {
            schedule: RestartReleaseSchedule::Exponential { initial, maximum },
        })
    }

    fn calculate(self, ordinal: RecoveryOrdinal) -> Result<RecoveryRelease, RestartReleaseFailure> {
        let delayed = match self.schedule {
            RestartReleaseSchedule::Immediate => return Ok(RecoveryRelease::Immediate),
            RestartReleaseSchedule::Constant { delay } => delay,
            RestartReleaseSchedule::Linear { initial, maximum } => {
                multiply(initial, ordinal.get().into())?.min(maximum)
            }
            RestartReleaseSchedule::Exponential { initial, maximum } => {
                let shift = ordinal.get() - 1;
                let factor = 1_u128
                    .checked_shl(shift)
                    .ok_or(RestartReleaseFailure::DurationOverflow)?;
                multiply(initial, factor)?.min(maximum)
            }
        };
        Ok(RecoveryRelease::Delayed(delayed))
    }

    fn positive(delay: Duration) -> Result<Duration, RestartReleaseError> {
        match delay.cmp(&Duration::ZERO) {
            Ordering::Equal => Err(RestartReleaseError::ZeroDelay),
            Ordering::Less | Ordering::Greater => Ok(delay),
        }
    }

    fn growth(
        initial: Duration,
        maximum: Duration,
    ) -> Result<(Duration, Duration), RestartReleaseError> {
        let initial = Self::positive(initial)?;
        match maximum.cmp(&initial) {
            Ordering::Less => Err(RestartReleaseError::MaximumBelowInitial),
            Ordering::Equal | Ordering::Greater => Ok((initial, maximum)),
        }
    }
}

pub(in crate::atomic) fn admit_restart(
    count: &mut RecoveryCount,
    budget: RestartBudget,
    limit: RestartLimit,
    release: RestartRelease,
    observed_at: Instant,
    replacements: NonZeroUsize,
) -> RestartAdmission<'_> {
    let recovery = match count.propose() {
        RecoveryProposal::Available(proposal) => proposal,
        RecoveryProposal::Exhausted { admitted } => {
            return RestartAdmission::Denied {
                budget,
                reason: RestartDenial::RecoveryCountExhausted { admitted },
            };
        }
    };
    let proposed_budget = match budget.preview(limit, observed_at, replacements) {
        BudgetCheck::Admitted(budget) => budget,
        BudgetCheck::Denied(reason) => {
            recovery.decline();
            return RestartAdmission::Denied { budget, reason };
        }
    };
    match release.calculate(recovery.ordinal()) {
        Ok(release) => RestartAdmission::Proposed(RestartProposal {
            recovery,
            budget,
            proposed_budget,
            release,
        }),
        Err(reason) => {
            recovery.decline();
            RestartAdmission::Denied {
                budget,
                reason: RestartDenial::ReleaseCalculationFailed(reason),
            }
        }
    }
}

fn multiply(duration: Duration, factor: u128) -> Result<Duration, RestartReleaseFailure> {
    let nanoseconds = duration
        .as_nanos()
        .checked_mul(factor)
        .ok_or(RestartReleaseFailure::DurationOverflow)?;
    let seconds = u64::try_from(nanoseconds / 1_000_000_000)
        .map_err(|_| RestartReleaseFailure::DurationOverflow)?;
    let subsecond = u32::try_from(nanoseconds % 1_000_000_000)
        .map_err(|_| RestartReleaseFailure::DurationOverflow)?;
    Ok(Duration::new(seconds, subsecond))
}

#[cfg(test)]
mod tests {
    use core::num::{NonZeroU32, NonZeroUsize};
    use std::time::{Duration, Instant};

    use super::{
        RecoveryCount, RecoveryRelease, RestartAdmission, RestartBudget, RestartDenial,
        RestartLimit, RestartRelease, RestartReleaseFailure, admit_restart,
    };

    fn workers(count: usize) -> NonZeroUsize {
        NonZeroUsize::new(count).expect("test worker count is positive")
    }

    fn charge(count: u32) -> NonZeroU32 {
        NonZeroU32::new(count).expect("test charge is positive")
    }

    #[test]
    fn count_budget_and_release_commit_together() {
        let observed = Instant::now();
        let mut count = RecoveryCount::new();
        let RestartAdmission::Proposed(proposal) = admit_restart(
            &mut count,
            RestartBudget::empty(),
            RestartLimit::new(2, Duration::from_secs(10)),
            RestartRelease::immediate(),
            observed,
            workers(2),
        ) else {
            panic!("valid restart is proposed")
        };
        assert_eq!(proposal.ordinal().get(), 1);
        assert_eq!(proposal.release(), RecoveryRelease::Immediate);
        let budget = proposal.accept();
        assert_eq!(count.admitted(), 1);
        assert_eq!(budget.latest(), Some((observed, charge(2))));
    }

    #[test]
    fn denial_preserves_count_and_budget() {
        let observed = Instant::now();
        let mut count = RecoveryCount::new();
        let budget = RestartBudget::empty();
        let RestartAdmission::Denied { budget, reason } = admit_restart(
            &mut count,
            budget,
            RestartLimit::new(0, Duration::from_secs(10)),
            RestartRelease::immediate(),
            observed,
            workers(1),
        ) else {
            panic!("zero maximum denies the restart")
        };
        assert_eq!(count.admitted(), 0);
        assert_eq!(budget.latest(), None);
        assert_eq!(
            reason,
            RestartDenial::RestartLimitReached {
                active: 0,
                requested: NonZeroUsize::MIN,
                maximum: 0,
            }
        );
    }

    #[test]
    fn release_failure_does_not_commit_count_or_budget() {
        let observed = Instant::now();
        let mut count = RecoveryCount::from_admitted(95);
        let release = RestartRelease::exponential(Duration::from_nanos(2), Duration::MAX)
            .expect("valid release policy");
        let RestartAdmission::Denied { budget, reason } = admit_restart(
            &mut count,
            RestartBudget::empty(),
            RestartLimit::new(1, Duration::from_secs(10)),
            release,
            observed,
            workers(1),
        ) else {
            panic!("unrepresentable release is denied")
        };
        assert_eq!(count.admitted(), 95);
        assert_eq!(budget.latest(), None);
        assert_eq!(
            reason,
            RestartDenial::ReleaseCalculationFailed(RestartReleaseFailure::DurationOverflow)
        );
    }

    #[test]
    fn maximum_committed_budget_has_only_proposal_or_denial() {
        let observed = Instant::now();
        let mut count = RecoveryCount::new();
        let budget = match admit_restart(
            &mut count,
            RestartBudget::empty(),
            RestartLimit::new(u32::MAX, Duration::from_secs(10)),
            RestartRelease::immediate(),
            observed,
            workers(u32::MAX as usize),
        ) {
            RestartAdmission::Proposed(proposal) => proposal.accept(),
            RestartAdmission::Denied { .. } => panic!("the maximum charge is representable"),
        };
        assert_eq!(count.admitted(), 1);
        assert_eq!(budget.latest(), Some((observed, charge(u32::MAX))));

        match admit_restart(
            &mut count,
            budget,
            RestartLimit::new(u32::MAX, Duration::from_secs(10)),
            RestartRelease::immediate(),
            observed,
            workers(1),
        ) {
            RestartAdmission::Proposed(_) => panic!("the successor exceeds the maximum"),
            RestartAdmission::Denied { budget, reason } => {
                assert_eq!(count.admitted(), 1);
                assert_eq!(budget.latest(), Some((observed, charge(u32::MAX))));
                assert_eq!(
                    reason,
                    RestartDenial::RestartLimitReached {
                        active: u32::MAX,
                        requested: workers(1),
                        maximum: u32::MAX,
                    }
                );
            }
        }
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn oversized_worker_request_is_an_unchanged_limit_denial() {
        let observed = Instant::now();
        let mut count = RecoveryCount::new();
        let requested = workers(u32::MAX as usize + 1);
        match admit_restart(
            &mut count,
            RestartBudget::empty(),
            RestartLimit::new(u32::MAX, Duration::from_secs(10)),
            RestartRelease::immediate(),
            observed,
            requested,
        ) {
            RestartAdmission::Proposed(_) => panic!("the request exceeds the charge width"),
            RestartAdmission::Denied { budget, reason } => {
                assert_eq!(count.admitted(), 0);
                assert_eq!(budget.latest(), None);
                assert_eq!(
                    reason,
                    RestartDenial::RestartLimitReached {
                        active: 0,
                        requested,
                        maximum: u32::MAX,
                    }
                );
            }
        }
    }
}
