//! Fixed-supervisor correlation failures around shared restart admission.

mod admission;

pub(super) use crate::templates::restart::{RecoveryRelease, RestartBudget};
pub use admission::RecoveryDenialReason;
pub(super) use admission::{RestartAdmission, admit_restart};
