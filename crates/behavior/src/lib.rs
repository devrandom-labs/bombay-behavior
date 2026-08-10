//! Pure, typed actor-behavior primitives. A [`Behavior`] folds its associated
//! event protocol into exactly [`Actions`]: sends, fresh creations, and its
//! next behavior or termination. Higher capabilities are composed from these
//! explicit transition parts.

// The `workers!` macro emits `::behavior::…` paths; this alias lets
// those expansions resolve inside this crate too (macro hygiene).
extern crate self as behavior;

mod actor;
mod deadlined;
mod fold;
mod stashing;
mod supervision;
mod timing;
mod transition;
mod verdict;
mod watching;

// `Fsm` is a thin state-machine helper built from the core stashing primitive.
mod fsm;
mod protocol;
mod receive_timeout;
mod shutdown;
mod spec;

pub use actor::{
    Address, BirthMode, Births, Create, CreationKind, Delivery, MailAddr, NoBirths, Recipient,
    Route,
};
pub use deadlined::{At, AtActions, AtEvent, AtReaction, AtSends};
pub use fold::{Base, Behavior, BehaviorActed, FnState, State, Transcript, User, UserEvent, run};
pub use fsm::{Fsm, Move};
pub use protocol::{
    ChildEvent, ChildStopped, CreationEvent, CreationRejection, CreationResolved, ObserveChild,
    ObserveCreation, ObservePeer, PeerEvent, PeerStopped, ReportWorkerCreationResolved,
    ReportWorkerStopped, ScheduleAfter, ScheduleAt, ShutdownEvent, ShutdownRequested, TimeEvent,
    TimerElapsed, TimerGeneration, TimerId, WorkerCreationEvent, WorkerCreationResolved,
    WorkerEvent, WorkerStopped,
};
pub use receive_timeout::{
    ReceiveTimeout, ReceiveTimeoutActions, ReceiveTimeoutError, ReceiveTimeoutEvent,
    ReceiveTimeoutReaction, ReceiveTimeoutSends,
};
pub use shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use spec::Spec;
pub use stashing::{StashRoute, Stashing};
pub use supervision::{
    Incarnation, IncarnationCreation, IncarnationEffects, IncarnationError, IncarnationInput,
    IncarnationPhase, IncarnationReport, IncarnationState, Proxy, ProxyActions, ProxyCommand,
    ProxySends, RestartPolicy, Strategy, Supervising, SupervisionEvent, SupervisionFailure,
    SupervisionFailureReaction, SupervisorActions, SupervisorSends, restart_all, restart_one,
    restart_rest, retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use transition::{Acted, Actions, Become, SendAlgebra, SendProduct, ServiceSends};
pub use verdict::{Never, Step};
pub use watching::{LinkReaction, WatchEvent, Watching, stop_on_abnormal_death};

mod exit;

pub use exit::{Crash, Exit, RestartDenial, SupervisionFailureReason};

pub use behavior_macros::workers;
