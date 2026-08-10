mod behavior;
mod reducer;
mod user_event;

pub use behavior::{Behavior, BehaviorActed, FoldFn, Handler, Pure};
pub use reducer::{ActionReducer, Effects, Folded, fold_events};
pub use user_event::{EventInput, User, UserEvent};
