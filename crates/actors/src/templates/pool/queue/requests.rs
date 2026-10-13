//! FIFO request order.

/// Named FIFO request lanes in their declared interpretation order.
#[doc(hidden)]
#[derive(behavior_macros::SendProduct)]
pub struct FifoRequests<
    WorkerObservations,
    WorkerInitializations,
    WorkerStartups,
    CustomerOutcomes,
    WorkerAssignments,
    WorkerPreparations,
    RestartSchedules,
    WorkerShutdowns,
    Diagnostics,
> {
    pub worker_observations: WorkerObservations,
    pub worker_initializations: WorkerInitializations,
    pub worker_startups: WorkerStartups,
    pub customer_outcomes: CustomerOutcomes,
    pub worker_assignments: WorkerAssignments,
    pub worker_preparations: WorkerPreparations,
    pub restart_schedules: RestartSchedules,
    pub worker_shutdowns: WorkerShutdowns,
    pub diagnostics: Diagnostics,
}
