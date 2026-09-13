//! FIFO-only queued customer ownership.

use super::super::pool::assignment::CustomerJob;

pub(super) struct QueuedJob<Role, Job, Customer> {
    pub(super) customer: CustomerJob<Job, Customer>,
    pub(super) assigned_role: Option<Role>,
}
