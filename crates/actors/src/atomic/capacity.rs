//! Positive capacities shared by distinct atomic policies.

use core::num::NonZeroUsize;

use thiserror::Error;

/// Zero cannot bound work that an atomic actor may accept.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("capacity must be positive")]
pub struct ZeroCapacity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::atomic) struct PositiveCapacity(NonZeroUsize);

impl PositiveCapacity {
    pub(in crate::atomic) fn new(maximum: usize) -> Result<Self, ZeroCapacity> {
        NonZeroUsize::new(maximum).map(Self).ok_or(ZeroCapacity)
    }

    pub(in crate::atomic) const fn get(self) -> usize {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::{PositiveCapacity, ZeroCapacity};
    use crate::atomic::{ActivationPolicy, BindingCapacity, EntryCapacity};

    #[test]
    fn every_atomic_capacity_rejects_zero() {
        assert_eq!(ActivationPolicy::new(0), Err(ZeroCapacity));
        assert_eq!(EntryCapacity::new(0), Err(ZeroCapacity));
        assert_eq!(BindingCapacity::new(0), Err(ZeroCapacity));
    }

    #[test]
    fn positive_capacity_preserves_the_validated_maximum() {
        let capacity =
            PositiveCapacity::new(7).unwrap_or_else(|_| panic!("seven is a positive capacity"));
        assert_eq!(capacity.get(), 7);
    }
}
