//! Capacity for unresolved worker startup authorizations.

use crate::templates::capacity::{PositiveCapacity, ZeroCapacity};

/// Positive maximum for unresolved worker startup authorizations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupLimit {
    maximum: PositiveCapacity,
}

impl StartupLimit {
    /// Select the maximum number of unresolved startup authorizations.
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
