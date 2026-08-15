//! Pure, typed actor-behavior primitives. A [`Behavior`] folds its associated
//! event protocol into exactly [`Actions`]: sends, fresh creations, and its
//! next behavior or termination. Higher capabilities are composed from these
//! explicit transition parts.

// The `#[behavior]` expansion emits `::behavior::…` paths; this alias lets the
// expansion resolve inside this crate too.
extern crate self as behavior;

mod actor;
mod effects;
mod next;
mod reducer;
mod transition;
mod user_event;

pub use actor::{
    Address, BirthMode, Births, Create, CreationKind, Delivery, MailAddr, NoBirths, Recipient,
};
pub use effects::{Acted, Actions, Become, Own, SendAlgebra, SendInput, ServiceSends};
pub use next::{Never, Step, Stopped};
pub use reducer::{ActionReducer, Effects, FoldFailure, Folded, fold_events};
pub use transition::{
    ActiveTurn, Behavior, BehaviorActed, BehaviorBase, InitializationTurn, delegate_transition,
    initialize,
};
pub use user_event::{EventInput, RouteInput, User, UserEvent};

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
