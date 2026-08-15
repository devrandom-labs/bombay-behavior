//! Pure restart-admission window.

use std::time::Duration;

use std::time::Instant;

use crate::RestartDenial;

/// A sliding-window restart budget.
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

    /// Admit an atomic replacement set or leave the budget unchanged.
    pub fn admit(&mut self, at: Instant, requested: usize) -> Result<(), RestartDenial> {
        self.prune(at);
        if self.admitted.len() + requested > self.maximum as usize {
            return Err(RestartDenial::BudgetExceeded {
                restarts_in_window: self.admitted.len(),
                replacements_requested: requested,
                maximum_restarts: self.maximum,
            });
        }
        self.admitted.resize(self.admitted.len() + requested, at);
        Ok(())
    }

    fn prune(&mut self, now: Instant) {
        if self.window == Duration::MAX {
            return;
        }
        self.admitted.retain(|stamp| {
            now.checked_duration_since(*stamp)
                .is_none_or(|age| age <= self.window)
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
}
