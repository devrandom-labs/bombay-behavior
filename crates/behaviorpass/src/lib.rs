//! Pure actor algebra. A [`Behavior`] folds its associated event protocol into
//! exactly [`Actions`]: sends, creates, and become. Timing, observation,
//! supervision, stashing, and finite-state behavior are derived compositions.

// The `workers!` macro emits `::behaviorpass::…` paths; this alias lets
// those expansions resolve inside this crate too (macro hygiene).
extern crate self as behaviorpass;

mod behavior;
mod deadlined;
mod stashing; // Stashing — the buffer primitive (STAYS in core)
mod supervising; // Supervising — decides the FULL strategy space as pure fold decisions
mod verdict; // Never, Step — owned verdict vocabulary (no upward deps)
mod watching; // Watching

// `Fsm` is NOT a core capability — it's a thin helper (a state machine built
// from core, using the Stash buffer). `Phased`/`Admit` left with it: phases are
// the aggregate's (nexus's) concern, not core's.
mod fsm;
mod spec;

pub use behavior::{
    Acted, Actions, Address, Base, Become, Behavior, Create, Delivery, FnState, MailAddr,
    Recipient, Route, SendAlgebra, SendProduct, State, Transcript, User, UserEvent, run,
};
pub use deadlined::{At, AtEvent, AtId, AtReaction, ScheduleAt, TimeEvent, TimeReached};
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

mod exit; // Exit, Crash — the trace-exit vocabulary (moved in-crate; reference crate retired)

pub use exit::{Crash, Exit};

pub use behaviorpass_macros::workers;
