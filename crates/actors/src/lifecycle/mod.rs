//! Lifecycle observation, application-root, terminal-result, and graceful-shutdown behaviors.

mod child_shutdown;
mod shutdown_coordinator;
mod task;
pub(crate) mod termination_monitor;
mod termination_propagation;

pub use crate::shutdown::{FinalizeOnShutdown, ShutdownEvent, ShutdownReaction, StopOnShutdown};
pub use crate::watch::{LinkReaction, Watch, WatchEvent, stop_on_abnormal_death};
pub use child_shutdown::{
    BeginShutdownPhases, ChildCreationExpectation, ChildShutdownPhases, ChildShutdownPlanError,
    DeclareShutdownPhase, FinishShutdownPhases, shutdown_after_children,
};
pub use shutdown_coordinator::{
    HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan, HeterogeneousShutdownSends,
    InstallShutdownPlan, NoShutdownTargets, ReportShutdownPlan, ShutdownChoice,
    ShutdownCoordinator, ShutdownCoordinatorError, ShutdownCoordinatorEvent, ShutdownPlan,
    ShutdownPlanError, ShutdownState, ShutdownTargetAt, ShutdownTree, ShutdownTreeError,
    shutdown_target,
};
pub use task::{Task, TaskError, TaskMessage, TaskResult, TaskState};
pub use termination_monitor::{
    EstablishedTerminationMonitor, EstablishedTerminationReaction, EstablishedTerminationTarget,
    LogicalTerminationTarget, TerminationMonitor, TerminationMonitorError, TerminationMonitorWith,
    TerminationObservation, TerminationObservationTarget, TerminationReaction,
};
pub use termination_propagation::{
    ChildTermination, PeerTermination, PropagateTermination, TerminalDisposition,
    TerminalPropagationPolicy, TerminalPropagationSends, TerminalPropagationState,
    TerminationPropagationError, TerminationTarget, propagate_abnormal, propagate_all,
};
