//! Pure, typed actor-behavior primitives. A [`Behavior`] folds its associated
//! event protocol into exactly [`Actions`]: sends, fresh creations, and its
//! next behavior or termination. Higher capabilities are composed from these
//! explicit transition parts.

// The `workers!` macro emits `::behavior::…` paths; this alias lets
// those expansions resolve inside this crate too (macro hygiene).
extern crate self as behavior;

mod addressing;
mod creation;
mod deadlined;
mod driver;
mod fold;
mod sending;
mod stashing;
mod supervising;
mod supervision_policy;
mod supervision_protocol;
mod transition;
mod user_event;
mod verdict;
mod watching;

// `Fsm` is a thin state-machine helper built from the core stashing primitive.
mod fsm;
mod protocol;
mod receive_timeout;
mod shutdown;
mod spec;

pub use addressing::{Address, Delivery, MailAddr, Recipient, Route};
pub use creation::{BirthMode, Births, Create, CreationKind, NoBirths};
pub use deadlined::{At, AtActions, AtEvent, AtReaction, AtSends};
pub use driver::{Transcript, run};
pub use fold::{Base, Behavior, BehaviorActed, FnState, State};
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
pub use sending::{SendAlgebra, SendProduct, ServiceSends};
pub use shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use spec::Spec;
pub use stashing::{StashRoute, Stashing};
pub use supervising::{Proxy, Supervising};
pub use supervision_policy::{
    RestartPolicy, Strategy, SupervisionFailure, SupervisionFailureReaction, restart_all,
    restart_one, restart_rest, retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use supervision_protocol::{ProxyCommand, SupervisionEvent};
pub use transition::{Acted, Actions, Become};
pub use user_event::{User, UserEvent};
pub use verdict::{Never, Step};
pub use watching::{LinkReaction, WatchEvent, Watching, stop_on_abnormal_death};

mod exit;

pub use exit::{Crash, Exit, RestartDenial, SupervisionFailureReason};

pub use behavior_macros::workers;
