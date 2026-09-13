use crate::atomic::OrderedRoles;

use super::{PreparedWorker, WorkerSubmission};

/// Complete custody when an initial worker is unavailable.
pub struct InitialWorkerRejection<Factory, Role, Worker, Plan, Rejection> {
    /// Original construction-only worker function.
    pub factory: Factory,
    /// Roles and worker submissions prepared before rejection.
    pub prepared: Vec<PreparedWorker<Role, Worker, Plan>>,
    /// Exact role whose worker was unavailable.
    pub role: Role,
    /// Typed reason returned by the worker function.
    pub reason: Rejection,
    /// Untouched declaration-order suffix.
    pub remaining: Vec<Role>,
}

pub(in crate::atomic) fn prepare_initial_workers<Factory, Role, Worker, Plan, Rejection>(
    mut factory: Factory,
    roles: OrderedRoles<Role>,
) -> Result<
    Vec<PreparedWorker<Role, Worker, Plan>>,
    InitialWorkerRejection<Factory, Role, Worker, Plan, Rejection>,
>
where
    Factory: FnMut(&Role) -> Result<WorkerSubmission<Worker, Plan>, Rejection>,
{
    let mut remaining_roles = roles.into_roles().into_iter();
    let mut prepared = Vec::new();
    for role in remaining_roles.by_ref() {
        match factory(&role) {
            Ok(submission) => prepared.push(PreparedWorker { role, submission }),
            Err(rejection) => {
                return Err(InitialWorkerRejection {
                    factory,
                    prepared,
                    role,
                    reason: rejection,
                    remaining: remaining_roles.collect(),
                });
            }
        }
    }
    Ok(prepared)
}
