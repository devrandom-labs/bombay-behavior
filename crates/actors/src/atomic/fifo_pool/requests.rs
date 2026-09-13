//! FIFO request order.

use super::super::requests::request_product;

request_product! {
    /// Named FIFO request lanes in their declared interpretation order.
    #[doc(hidden)]
    pub struct FifoRequests<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        CustomerOutcomes,
        WorkerAssignments,
        WorkerPreparations,
        RestartSchedules,
        WorkerShutdowns,
        Diagnostics,
    > {
        pub worker_observations: WorkerObservations,
        pub worker_initializations: WorkerInitializations,
        pub worker_activations: WorkerActivations,
        pub customer_outcomes: CustomerOutcomes,
        pub worker_assignments: WorkerAssignments,
        pub worker_preparations: WorkerPreparations,
        pub restart_schedules: RestartSchedules,
        pub worker_shutdowns: WorkerShutdowns,
        pub diagnostics: Diagnostics,
    }
}
