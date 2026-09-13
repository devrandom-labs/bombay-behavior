//! Capacity for unresolved worker activation authorizations.

use super::capacity::{PositiveCapacity, ZeroCapacity};

/// Positive maximum for unresolved worker activation authorizations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivationPolicy {
    maximum: PositiveCapacity,
}

impl ActivationPolicy {
    /// Select the maximum number of unresolved activation authorizations.
    ///
    /// # Errors
    ///
    /// Returns [`ZeroCapacity`] when `maximum` is zero.
    pub fn new(maximum: usize) -> Result<Self, ZeroCapacity> {
        PositiveCapacity::new(maximum).map(|maximum| Self { maximum })
    }

    pub(crate) const fn maximum(self) -> usize {
        self.maximum.get()
    }
}

/// Positive maximum number of entries retained by a dynamic supervisor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EntryCapacity {
    maximum: PositiveCapacity,
}

impl EntryCapacity {
    /// Validate a dynamic-supervisor entry capacity.
    ///
    /// # Errors
    ///
    /// Returns [`ZeroCapacity`] when `maximum` is zero.
    pub fn new(maximum: usize) -> Result<Self, ZeroCapacity> {
        PositiveCapacity::new(maximum).map(|maximum| Self { maximum })
    }

    pub(crate) const fn maximum(self) -> usize {
        self.maximum.get()
    }
}
