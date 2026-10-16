//! Named construction choices for a dynamic service directory.

use core::num::NonZeroU64;
use std::collections::BTreeMap;

use behavior::{Behavior, BehaviorAddr, CreationSequence, EndpointAddress};

use super::{DynamicSupervisor, EntryCapacity, SupervisorAvailability, UnexpectedExit};
use crate::diagnostic::DiagnosticDisposition;
use crate::lifecycle::ActorDrainPolicy;
use crate::proxy::StableProxy;
use crate::worker::{StartupLimit, StartupPlan};

/// All simultaneous choices for one bounded dynamic supervisor.
///
/// The directory owns retained service entries; startup bounds concurrent worker
/// work. Lifecycle and diagnostic routes remain distinct typed capabilities.
/// Construction consumes this product into the existing flat actor storage.
pub struct Config<LifecycleRoute, DiagnosticRoute> {
    /// Bound the number of retained service entries.
    pub entries: EntryCapacity,
    /// Bound concurrent worker startup.
    pub startup: StartupLimit,
    /// Select what happens after an unrequested worker stop.
    pub exit: UnexpectedExit,
    /// Select how the service graph drains during shutdown.
    pub shutdown: ActorDrainPolicy,
    /// Publish lifecycle events through this typed route.
    pub lifecycle: LifecycleRoute,
    /// Deliver complete diagnostics through a typed route or terminate.
    pub diagnostics: DiagnosticDisposition<DiagnosticRoute>,
}

/// Construct one empty dynamic supervisor from all named policy choices.
///
/// Capacity and startup limits are already validated by their owning values.
/// Construction performs no worker preparation or runtime effect.
#[must_use]
pub fn new<Key, Worker, Plan, LifecycleRoute, DiagnosticRoute>(
    config: Config<LifecycleRoute, DiagnosticRoute>,
) -> DynamicSupervisor<Key, Worker, Plan, LifecycleRoute, DiagnosticRoute>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let Config {
        entries,
        startup,
        exit: unexpected_exit,
        shutdown: actor_drain,
        lifecycle,
        diagnostics,
    } = config;
    DynamicSupervisor {
        services: BTreeMap::new(),
        entries,
        startup,
        generations: Some(NonZeroU64::MIN),
        operations: Some(NonZeroU64::MIN),
        creations: CreationSequence::new(),
        unexpected_exit,
        availability: SupervisorAvailability::Accepting(actor_drain),
        lifecycle,
        diagnostics,
    }
}
