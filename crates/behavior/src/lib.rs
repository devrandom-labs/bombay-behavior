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
mod spec;

pub use algebra::{
    Acted, Actions, Address, Base, Become, Behavior, BirthMode, Births, Create, Delivery, FnState,
    MailAddr, NoBirths, Recipient, Route, SendAlgebra, SendProduct, State, Transcript, User,
    UserEvent, run,
};
pub use deadlined::{
    At, AtEvent, AtGeneration, AtId, AtReaction, ScheduleAt, TimeEvent, TimeReached,
};
pub use fsm::{Fsm, Move};
pub use spec::Spec;
pub use stashing::{StashRoute, Stashing};
pub use supervising::{
    ChildEvent, ChildStopped, ObserveChild, Proxy, ProxyCommand, RestartPolicy, Strategy,
    Supervising, SupervisionEvent, restart_all, restart_one, restart_rest,
};
pub use verdict::{Never, Step};
pub use watching::{
    LinkReaction, ObservePeer, PeerEvent, PeerStopped, WatchEvent, Watching, stop_on_abnormal_death,
};

mod exit;

pub use exit::{Crash, Exit};

pub use behavior_macros::workers;
