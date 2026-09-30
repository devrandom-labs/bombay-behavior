//! Named StableProxy action lanes and their total ordered settlement.

/// Named worker lifecycle, delivery, owner-outcome, and diagnostic lanes.
#[doc(hidden)]
#[derive(behavior_macros::SendProduct)]
pub struct ProxyEffects<
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> {
    pub worker_observations: WorkerObservations,
    pub worker_initializations: WorkerInitializations,
    pub worker_activations: WorkerActivations,
    pub worker_shutdowns: WorkerShutdowns,
    pub worker_deliveries: WorkerDeliveries,
    pub owner_outcomes: OwnerOutcomes,
    pub diagnostics: Diagnostics,
}
