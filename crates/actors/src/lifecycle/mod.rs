//! Lifecycle observation, application-root, terminal-result, and graceful-shutdown behaviors.

mod guardian;
mod shutdown_coordinator;
mod task;
mod termination_monitor;
mod termination_propagation;

pub use crate::shutdown::{FinalizeOnShutdown, ShutdownEvent, ShutdownReaction, StopOnShutdown};
pub use crate::watch::{LinkReaction, Watch, WatchEvent, stop_on_abnormal_death};
pub use guardian::{CoordinatedGuardian, Guardian};
pub use shutdown_coordinator::{
    CoordinatedTerminalApplication, HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan,
    HeterogeneousShutdownSends, NoShutdownTargets, ShutdownChoice, ShutdownCoordinator,
    ShutdownCoordinatorError, ShutdownCoordinatorEvent, ShutdownPlan, ShutdownPlanError,
    ShutdownState, ShutdownTargetAt, ShutdownTree, ShutdownTreeError, TreeShutdown,
    coordinated_terminal_application, shutdown_target,
};
pub use task::{Task, TaskError, TaskMessage, TaskResult, TaskState};
pub use termination_monitor::{
    CleanupReaction, EstablishedTerminationMonitor, EstablishedTerminationReaction,
    EstablishedTerminationTarget, LifecyclePublication, LifecyclePublisher,
    LogicalTerminationTarget, Reaper, TerminationMonitor, TerminationMonitorWith,
    TerminationObservation, TerminationObservationTarget, TerminationReaction,
};
pub use termination_propagation::{
    ChildTermination, PeerTermination, PropagateTermination, TerminalDisposition,
    TerminalPropagationPolicy, TerminalPropagationSends, TerminalPropagationState,
    TerminationTarget, propagate_abnormal, propagate_all,
};
