//! FixedSupervisor request order.

/// Named request lanes in their declared interpretation order.
#[doc(hidden)]
#[derive(behavior_macros::SendProduct)]
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
