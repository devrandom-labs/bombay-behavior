//! Validated maximum for the directory’s retained service entries.

use crate::templates::capacity::{PositiveCapacity, ZeroCapacity};

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
