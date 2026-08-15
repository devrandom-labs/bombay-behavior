//! Lifecycle observation, terminal-result, and graceful-shutdown behaviors.

mod task;

pub use crate::shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use crate::watch::{LinkReaction, Watch, WatchEvent, WatchSends, stop_on_abnormal_death};
pub use task::{Task, TaskError, TaskMessage, TaskResult, TaskState};
