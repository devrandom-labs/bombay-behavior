use crate::worker::OrderedRoles;

use super::{PreparedWorker, WorkerSubmission};
use behavior::Never;

mod sealed {
    pub trait InitialWorkerPreparation {}
}

/// The owned result of preparing one initial worker.
///
/// Factories may return a [`WorkerSubmission`] directly when preparation cannot
/// fail, or a [`Result`] when it has an actual refusal. The factory's return
/// value determines its worker, startup plan and exact rejection type.
/// This closed port performs no startup, allocation or actor transition.
///
/// Other factory return values cannot substitute for worker preparation:
///
/// ```compile_fail,E0277
/// fn require_preparation<P: behavior_actors::worker::InitialWorkerPreparation>(_: P) {}
/// require_preparation(String::from("search"));
/// ```
pub trait InitialWorkerPreparation: sealed::InitialWorkerPreparation {
    /// Complete worker definition prepared by the factory.
    type Worker;
    /// Owned startup work prepared with that worker.
    type Plan;
    /// Exact preparation refusal; [`Never`] for direct submissions.
    type Rejection;

    /// Transfer the complete submission or its unchanged preparation refusal.
    fn into_submission(self)
    -> Result<WorkerSubmission<Self::Worker, Self::Plan>, Self::Rejection>;
}

impl<Worker, Plan> sealed::InitialWorkerPreparation for WorkerSubmission<Worker, Plan> {}

impl<Worker, Plan> InitialWorkerPreparation for WorkerSubmission<Worker, Plan> {
    type Worker = Worker;
    type Plan = Plan;
    type Rejection = Never;

    fn into_submission(self) -> Result<Self, Never> {
        Ok(self)
    }
}

impl<Worker, Plan, Rejection> sealed::InitialWorkerPreparation
    for Result<WorkerSubmission<Worker, Plan>, Rejection>
{
}

impl<Worker, Plan, Rejection> InitialWorkerPreparation
    for Result<WorkerSubmission<Worker, Plan>, Rejection>
{
    type Worker = Worker;
    type Plan = Plan;
    type Rejection = Rejection;

    fn into_submission(self) -> Self {
        self
    }
}

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

pub(in crate::templates) fn prepare_initial_workers<Factory, Role, Preparation>(
    mut factory: Factory,
    roles: OrderedRoles<Role>,
) -> Result<
    Vec<PreparedWorker<Role, Preparation::Worker, Preparation::Plan>>,
    InitialWorkerRejection<
        Factory,
        Role,
        Preparation::Worker,
        Preparation::Plan,
        Preparation::Rejection,
    >,
>
where
    Factory: FnMut(&Role) -> Preparation,
    Preparation: InitialWorkerPreparation,
{
    let mut remaining_roles = roles.into_roles().into_iter();
    let mut prepared = Vec::new();
    for role in remaining_roles.by_ref() {
        match factory(&role).into_submission() {
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

#[cfg(test)]
mod tests {
    use super::prepare_initial_workers;
    use crate::worker::{OrderedRoles, WorkerSubmission};
    use std::collections::VecDeque;

    #[derive(Debug, Eq, PartialEq)]
    enum WorkerRole {
        Primary,
        Replica,
        Archive,
    }

    #[test]
    fn direct_factory_prepares_all_roles_without_an_error_annotation() {
        let primary = Box::<[u8]>::from([3, 5]);
        let replica = Box::<[u8]>::from([7, 11]);
        let allocations = [primary.as_ptr(), replica.as_ptr()];
        let mut definitions = VecDeque::from([primary, replica]);
        let roles = OrderedRoles::new(WorkerRole::Primary, [WorkerRole::Replica])
            .expect("the declared roles are distinct");
        let prepared = prepare_initial_workers(
            move |_: &WorkerRole| {
                WorkerSubmission::immediate(
                    definitions.pop_front().expect("one definition per role"),
                )
            },
            roles,
        )
        .unwrap_or_else(|rejected| {
            drop(rejected);
            panic!("direct preparation requires no invented error type");
        });
        assert_eq!(prepared.len(), 2);
        assert_eq!(prepared[0].role, WorkerRole::Primary);
        assert_eq!(prepared[1].role, WorkerRole::Replica);
        assert_eq!(prepared[0].submission.worker.as_ref(), [3, 5]);
        assert_eq!(prepared[1].submission.worker.as_ref(), [7, 11]);
        assert_eq!(prepared[0].submission.worker.as_ptr(), allocations[0]);
        assert_eq!(prepared[1].submission.worker.as_ptr(), allocations[1]);
    }

    #[test]
    fn fallible_factory_returns_original_prefix_refusal_suffix_and_factory() {
        let primary = Box::<[u8]>::from([13, 17]);
        let refusal = Box::<[u8]>::from([19, 23]);
        let archive = Box::<[u8]>::from([29, 31]);
        let primary_allocation = primary.as_ptr();
        let refusal_allocation = refusal.as_ptr();
        let archive_allocation = archive.as_ptr();
        let mut definitions = VecDeque::from([
            Ok(WorkerSubmission::immediate(primary)),
            Err(refusal),
            Ok(WorkerSubmission::immediate(archive)),
        ]);
        let roles = OrderedRoles::new(
            WorkerRole::Primary,
            [WorkerRole::Replica, WorkerRole::Archive],
        )
        .expect("the declared roles are distinct");
        let result = prepare_initial_workers(
            move |_: &WorkerRole| definitions.pop_front().expect("one preparation per role"),
            roles,
        );
        let Err(mut rejected) = result else {
            panic!("the exact middle preparation refuses");
        };
        assert_eq!(rejected.prepared.len(), 1);
        assert_eq!(rejected.prepared[0].role, WorkerRole::Primary);
        assert_eq!(rejected.prepared[0].submission.worker.as_ref(), [13, 17]);
        assert_eq!(
            rejected.prepared[0].submission.worker.as_ptr(),
            primary_allocation
        );
        assert_eq!(rejected.role, WorkerRole::Replica);
        assert_eq!(rejected.reason.as_ref(), [19, 23]);
        assert_eq!(rejected.reason.as_ptr(), refusal_allocation);
        assert_eq!(rejected.remaining, [WorkerRole::Archive]);
        let untouched = (rejected.factory)(&WorkerRole::Archive)
            .expect("the original factory still owns its unrequested definition");
        assert_eq!(untouched.worker.as_ref(), [29, 31]);
        assert_eq!(untouched.worker.as_ptr(), archive_allocation);
    }
}
