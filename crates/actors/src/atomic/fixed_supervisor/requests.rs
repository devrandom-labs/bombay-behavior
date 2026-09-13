//! FixedSupervisor request order.

use crate::atomic::requests::request_product;

request_product! {
    /// Named request lanes in their declared interpretation order.
    #[doc(hidden)]
    pub struct FixedSupervisorRequests<
        ProxyObservations,
        WorkerPreparations,
        ProxyOperations,
        RestartSchedules,
        Lifecycle,
        StatusReplies,
        CapabilityReplies,
        Diagnostics,
    > {
        pub proxy_observations: ProxyObservations,
        pub worker_preparations: WorkerPreparations,
        pub proxy_operations: ProxyOperations,
        pub restart_schedules: RestartSchedules,
        pub lifecycle: Lifecycle,
        pub status_replies: StatusReplies,
        pub capability_replies: CapabilityReplies,
        pub diagnostics: Diagnostics,
    }
}
