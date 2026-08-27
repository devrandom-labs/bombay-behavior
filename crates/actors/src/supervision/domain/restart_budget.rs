//! Pure restart-admission window.

use std::time::Duration;

use std::time::Instant;

use crate::RestartDenial;

/// A sliding-window restart budget.
#[derive(Clone)]
pub(crate) struct RestartBudget {
    maximum: u32,
    window: Duration,
    admitted: Vec<Instant>,
}

impl RestartBudget {
    #[must_use]
    pub const fn new(maximum: u32, window: Duration) -> Self {
        Self {
            maximum,
            window,
            admitted: Vec::new(),
        }
    }

    #[must_use]
    pub fn admitted(&self) -> usize {
        self.admitted.len()
    }

    /// Observe `at`, prune admissions no longer in that window, and then
    /// admit the complete replacement set or none of it.
    pub fn admit(&mut self, at: Instant, requested: usize) -> Result<(), RestartDenial> {
        Self::prune(&mut self.admitted, self.window, at);
        if requested > (self.maximum as usize).saturating_sub(self.admitted.len()) {
            return Err(RestartDenial::BudgetExceeded {
                restarts_in_window: self.admitted.len(),
                replacements_requested: requested,
                maximum_restarts: self.maximum,
            });
        }
        self.admitted.resize(self.admitted.len() + requested, at);
        Ok(())
    }

    fn prune(admitted: &mut Vec<Instant>, window: Duration, now: Instant) {
        if window == Duration::MAX {
            return;
        }
        admitted.retain(|stamp| {
            now.checked_duration_since(*stamp)
                .is_none_or(|age| age <= window)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_atomic_set_without_partially_charging_it() {
        let now = Instant::now();
        let mut budget = RestartBudget::new(2, Duration::MAX);
        budget.admit(now, 1).unwrap();
        assert!(budget.admit(now, 2).is_err());
        assert_eq!(budget.admitted(), 1);
    }

    #[test]
    fn rejection_prunes_aged_evidence_without_partially_charging_the_batch() {
        let start = Instant::now();
        let mut budget = RestartBudget::new(1, Duration::from_secs(1));
        budget.admit(start, 1).unwrap();

        let later = start + Duration::from_secs(2);
        let rejection = budget.admit(later, 2).unwrap_err();
        assert_eq!(
            rejection,
            RestartDenial::BudgetExceeded {
                restarts_in_window: 0,
                replacements_requested: 2,
                maximum_restarts: 1,
            }
        );
        assert_eq!(budget.admitted(), 0);

        budget.admit(later, 1).unwrap();
        assert_eq!(budget.admitted(), 1);
    }
}
