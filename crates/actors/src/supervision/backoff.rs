//! Checked restart-delay policy.

use std::time::Duration;

/// Pure delay progression for supervised replacement attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backoff {
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

impl Backoff {
    /// # Errors
    /// Returns `ZeroDelay` for a zero duration.
    pub fn constant(delay: Duration) -> Result<Self, BackoffConfigError> {
        if delay.is_zero() {
            return Err(BackoffConfigError::ZeroDelay);
        }
        Ok(Self::Constant { delay })
    }

    /// # Errors
    /// Returns a configuration error for zero or inverted bounds.
    pub fn linear(initial: Duration, maximum: Duration) -> Result<Self, BackoffConfigError> {
        Self::bounded(initial, maximum).map(|()| Self::Linear { initial, maximum })
    }

    /// # Errors
    /// Returns a configuration error for zero or inverted bounds.
    pub fn exponential(initial: Duration, maximum: Duration) -> Result<Self, BackoffConfigError> {
        Self::bounded(initial, maximum).map(|()| Self::Exponential { initial, maximum })
    }

    fn bounded(initial: Duration, maximum: Duration) -> Result<(), BackoffConfigError> {
        if initial.is_zero() {
            return Err(BackoffConfigError::ZeroDelay);
        }
        if maximum < initial {
            return Err(BackoffConfigError::MaximumBelowInitial);
        }
        Ok(())
    }

    /// Return the delay for a one-based attempt number.
    ///
    /// # Errors
    /// Returns `ZeroAttempt` or `DurationOverflow` when arithmetic is invalid.
    pub fn delay(self, attempt: u32) -> Result<Duration, BackoffError> {
        if attempt == 0 {
            return Err(BackoffError::ZeroAttempt);
        }
        match self {
            Self::Constant { delay } => Ok(delay),
            Self::Linear { initial, maximum } => initial
                .checked_mul(attempt)
                .map(|d| d.min(maximum))
                .ok_or(BackoffError::DurationOverflow),
            Self::Exponential { initial, maximum } => {
                let shift = attempt - 1;
                let factor = 1_u32
                    .checked_shl(shift)
                    .ok_or(BackoffError::DurationOverflow)?;
                initial
                    .checked_mul(factor)
                    .map(|d| d.min(maximum))
                    .ok_or(BackoffError::DurationOverflow)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BackoffConfigError {
    #[error("a restart backoff delay must be non-zero")]
    ZeroDelay,
    #[error("the maximum restart delay is below the initial delay")]
    MaximumBelowInitial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BackoffError {
    #[error("restart attempt numbers are one-based")]
    ZeroAttempt,
    #[error("restart delay arithmetic overflowed")]
    DurationOverflow,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_zero_and_inverted_bounds() {
        assert_eq!(
            Backoff::constant(Duration::ZERO),
            Err(BackoffConfigError::ZeroDelay)
        );
        assert_eq!(
            Backoff::linear(Duration::from_secs(2), Duration::from_secs(1)),
            Err(BackoffConfigError::MaximumBelowInitial)
        );
    }

    #[test]
    fn policies_are_one_based_checked_and_bounded() {
        let constant = Backoff::constant(Duration::from_secs(3)).unwrap();
        assert_eq!(constant.delay(0), Err(BackoffError::ZeroAttempt));
        assert_eq!(constant.delay(9), Ok(Duration::from_secs(3)));

        let linear = Backoff::linear(Duration::from_secs(2), Duration::from_secs(5)).unwrap();
        assert_eq!(linear.delay(1), Ok(Duration::from_secs(2)));
        assert_eq!(linear.delay(2), Ok(Duration::from_secs(4)));
        assert_eq!(linear.delay(3), Ok(Duration::from_secs(5)));

        let exponential =
            Backoff::exponential(Duration::from_secs(2), Duration::from_secs(9)).unwrap();
        assert_eq!(exponential.delay(1), Ok(Duration::from_secs(2)));
        assert_eq!(exponential.delay(3), Ok(Duration::from_secs(8)));
        assert_eq!(exponential.delay(4), Ok(Duration::from_secs(9)));
        assert_eq!(exponential.delay(33), Err(BackoffError::DurationOverflow));
    }
}
