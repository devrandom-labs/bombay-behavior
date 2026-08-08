//! Pure actor algebra. A [`Behavior`] folds its associated event protocol into
//! exactly [`Actions`]: sends, creates, and become. Timing, observation,
//! supervision, stashing, and finite-state behavior are derived compositions.

// The `workers!` macro emits `::behavior::…` paths; this alias lets
// those expansions resolve inside this crate too (macro hygiene).
extern crate self as behavior;

#[path = "behavior.rs"]
mod algebra;
mod deadlined;
mod stashing;
mod supervising;
mod verdict;
mod watching;

// `Fsm` is a thin state-machine helper built from the core stashing primitive.
mod fsm;
mod protocol;
mod receive_timeout;
mod shutdown;
mod spec;

pub use algebra::{
    Acted, Actions, Address, Base, Become, Behavior, BehaviorActed, BirthMode, Births, Create,
    CreationKind, Delivery, FnState, MailAddr, NoBirths, Recipient, Route, SendAlgebra,
    SendProduct, ServiceSends, State, Transcript, User, UserEvent, run,
};
pub use deadlined::{At, AtActions, AtEvent, AtReaction, AtSends};
pub use fsm::{Fsm, Move};
pub use protocol::{
    ChildEvent, ChildStopped, ObserveChild, ObservePeer, PeerEvent, PeerStopped,
    ReportWorkerStopped, ScheduleAfter, ScheduleAt, ShutdownEvent, ShutdownRequested, TimeEvent,
    TimerElapsed, TimerGeneration, TimerId, WorkerEvent, WorkerStopped,
};
pub use receive_timeout::{
    ReceiveTimeout, ReceiveTimeoutActions, ReceiveTimeoutError, ReceiveTimeoutEvent,
    ReceiveTimeoutReaction, ReceiveTimeoutSends,
};
pub use shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use spec::Spec;
pub use stashing::{StashRoute, Stashing};
pub use supervising::{
    Proxy, ProxyCommand, RestartPolicy, Strategy, Supervising, SupervisionEvent,
    SupervisionFailure, SupervisionFailureReaction, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use verdict::{Never, Step};
pub use watching::{LinkReaction, WatchEvent, Watching, stop_on_abnormal_death};

mod exit;

pub use exit::{Crash, Exit, RestartDenial, SupervisionFailureReason};

pub use behavior_macros::workers;
