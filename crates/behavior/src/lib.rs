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
mod stash;
mod supervision;
mod timing;
mod watch;

// `Machine` is a thin state-machine helper built from the core stashing primitive.
mod compose;
mod machine;
mod mailbox;
mod protocol;
mod shutdown;

pub use actor::{
    Address, BirthMode, Births, Create, CreationKind, Delivery, MailAddr, NoBirths, Recipient,
    Route,
};
pub use calculus::{
    ActionReducer, Behavior, BehaviorActed, BehaviorFn, Effects, EventInput, FoldFn, Folded,
    Handler, Pure, User, UserEvent, fold_events,
};
pub use compose::Compose;
pub use effects::{
    Acted, Actions, Become, Inner, Own, SendAlgebra, SendInput, SendProduct, ServiceSends,
};
pub use machine::{Machine, Move};
pub use mailbox::{Transcript, run};
pub use next::{Never, Step};
pub use protocol::{
    ChildEvent, ChildStopped, CreationEvent, CreationRejection, CreationResolved, ObserveChild,
    ObserveCreation, ObservePeer, PeerEvent, PeerStopped, ReportWorkerCreationResolved,
    ReportWorkerStopped, ScheduleAfter, ScheduleAt, ShutdownEvent, ShutdownRequested, TimeEvent,
    TimerElapsed, TimerGeneration, TimerId, UnwatchPeer, WorkerCreationEvent,
    WorkerCreationResolved, WorkerEvent, WorkerStopped,
};
pub use shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use stash::{Stash, StashRoute};
pub use supervision::{
    IncarnationPhase, Proxy, ProxyActions, ProxyCommand, ProxyEvent, ProxySends, RestartPolicy,
    Strategy, SupervisionEvent, SupervisionFailure, SupervisionFailureReaction, Supervisor,
    SupervisorActions, SupervisorSends, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use timing::{
    Deadline, DeadlineActions, DeadlineEvent, DeadlineReaction, DeadlineSends, ReceiveTimeout,
    ReceiveTimeoutActions, ReceiveTimeoutError, ReceiveTimeoutEvent, ReceiveTimeoutReaction,
    ReceiveTimeoutSends, TimedEvent,
};
pub use watch::{LinkReaction, Watch, WatchEvent, WatchSends, stop_on_abnormal_death};

mod exit;

pub use exit::{Crash, Exit, RestartDenial, SupervisionFailureReason};

pub use behavior_macros::workers;
