//! Pure, typed actor-behavior primitives. A [`Behavior`] folds its associated
//! event protocol into exactly [`Actions`]: sends, fresh creations, and its
//! next behavior or termination. Higher capabilities are composed from these
//! explicit transition parts.

// The `workers!` macro emits `::behavior::…` paths; this alias lets
// those expansions resolve inside this crate too (macro hygiene).
extern crate self as behavior;

mod actor;
mod calculus;
mod effects;
mod next;
mod pool;
mod stash;
mod supervision;
mod timing;
mod watch;

// `Machine` is a thin state-machine helper built from the core stashing primitive.
mod compose;
mod machine;
mod protocol;
mod shutdown;

pub use actor::{
    Address, BirthMode, Births, Create, CreationKind, Delivery, MailAddr, NoBirths, Recipient,
};
pub use calculus::{
    ActionReducer, ActiveTurn, Behavior, BehaviorActed, BehaviorBase, Effects, EventInput,
    FoldFailure, Folded, InitializationTurn, RouteInput, User, UserEvent, fold_events,
};
pub use compose::{Active, Compose, Initialized};
pub use effects::{Acted, Actions, Become, Own, SendAlgebra, SendInput, ServiceSends};
pub use machine::{Machine, Move};
pub use next::{Never, Step};
pub use pool::{
    AffinitySelector, AssignmentId, InterruptionPolicy, JobId, KeyedPoolEvent, KeyedPoolMessage,
    KeyedWorkerPool, PoolActions, PoolAssignment, PoolBehaviorSends, PoolConfigError, PoolError,
    PoolEvent, PoolInterruption, PoolMessage, PoolRejection, PoolResponse, PoolSends, WorkerPhase,
    WorkerPool, WorkerRetirement,
};
pub use protocol::{
    ChildStopped, CreationRejection, CreationResolved, ObserveChild, ObserveCreation, ObservePeer,
    PeerStopped, ReplacementResolution, ReportWorkerCreationResolved, ReportWorkerStopped,
    ScheduleAfter, ScheduleAt, ShutdownRequested, TimerElapsed, TimerGeneration, TimerId,
    UnwatchPeer, WorkerCreationResolved, WorkerStopped,
};
pub use shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use stash::{Stash, StashRoute, StashStatus};
pub use supervision::{
    FleetError, IncarnationPhase, Proxy, ProxyCommand, ProxyError, ProxyEvent, ProxySends,
    RestartPolicy, Strategy, SupervisionEvent, SupervisionFailure, SupervisionFailureReaction,
    Supervisor, SupervisorError, SupervisorSends, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use timing::{
    Deadline, DeadlineEvent, DeadlineReaction, DeadlineSends, ReceiveTimeout, ReceiveTimeoutEvent,
    ReceiveTimeoutReaction, ReceiveTimeoutSends, TimedEvent,
};
pub use watch::{LinkReaction, Watch, WatchEvent, WatchSends, stop_on_abnormal_death};

mod exit;

pub use exit::{Crash, Exit, RestartDenial, SupervisionFailureReason};

/// Generate `Behavior` wiring for an inherent impl with an exact `receive`
/// method and an optional exact `init` method. When omitted, initialization is
/// the explicit empty transition: no sends, no creations, and `Continue`.
/// Invalid receivers are rejected at compile time.
///
/// ```compile_fail
/// use behavior::{Actions, Delivery, MailAddr, Never, NoBirths};
///
/// struct Invalid;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = u8,
///     sends = Vec<Never>,
///     births = NoBirths,
///     error = Never,
/// )]
/// impl Invalid {
///     fn init(&self) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: u8) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
/// }
/// ```
///
/// Missing receive methods are rejected by the macro itself:
///
/// ```compile_fail
/// use behavior::{Actions, Delivery, MailAddr, Never, NoBirths};
/// struct Missing;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = u8,
///     sends = Vec<Never>,
///     births = NoBirths,
///     error = Never,
/// )]
/// impl Missing {
/// }
/// ```
///
/// Async behavior methods cannot introduce an erased or alternate execution
/// path:
///
/// ```compile_fail
/// use behavior::{Actions, Delivery, MailAddr, Never, NoBirths};
/// struct Async;
/// #[behavior::behavior(
///     addr = MailAddr,
///     message = u8,
///     sends = Vec<Never>,
///     births = NoBirths,
///     error = Never,
/// )]
/// impl Async {
///     async fn init(&mut self, _: crate::InitializationTurn) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
///     fn receive(&mut self, _: MailAddr, _: u8) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
///         Ok(Actions::cont())
///     }
/// }
/// ```
pub use behavior_macros::behavior;
pub use behavior_macros::workers;
