//! Lifecycle observation, application-root, terminal-result, and graceful-shutdown behaviors.

mod guardian;
mod shutdown_coordinator;
mod task;
mod termination_monitor;

pub use crate::shutdown::{FinalizeOnShutdown, ShutdownEvent, ShutdownReaction, StopOnShutdown};
pub use crate::watch::{LinkReaction, Watch, WatchEvent, stop_on_abnormal_death};
pub use guardian::{CoordinatedGuardian, Guardian};
pub use shutdown_coordinator::{
    HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan, HeterogeneousShutdownSends,
    NoShutdownTargets, ShutdownChoice, ShutdownCoordinator, ShutdownCoordinatorError,
    ShutdownCoordinatorEvent, ShutdownPlan, ShutdownPlanError, ShutdownState, ShutdownTree,
    ShutdownTreeError, TreeShutdown,
};
pub use task::{Task, TaskError, TaskMessage, TaskResult, TaskState};
pub use termination_monitor::{
    CleanupReaction, LifecyclePublication, LifecyclePublisher, Reaper, TerminationMonitor,
    TerminationObservation, TerminationReaction,
};
