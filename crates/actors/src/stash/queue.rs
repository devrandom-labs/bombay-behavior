//! The authoritative original-input FIFO and replay policy.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, BirthMode, SendEffects, User, UserEvent};
use behavior::{Never, Step};

mod sealed {
    pub trait StaticallyInfallible {}
}

/// Statically proven uninhabited transition error accepted by both stash policies.
///
/// The trait is sealed: replay safety depends on the error being genuinely
/// impossible, so downstream code cannot assert the capability for an
/// inhabited type.
#[diagnostic::on_unimplemented(
    message = "stash replay requires an infallible inner behavior",
    label = "this error type is not supported for infallible replay",
    note = "use Never or MachineError<A, M, Never> for the inner transition error; replay cannot undo actions from earlier messages"
)]
pub trait StaticallyInfallible: sealed::StaticallyInfallible {}

impl sealed::StaticallyInfallible for Never {}
impl StaticallyInfallible for Never {}

impl<A, M> sealed::StaticallyInfallible for crate::MachineError<A, M, Never> {}
impl<A, M> StaticallyInfallible for crate::MachineError<A, M, Never> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashRoute {
    Stash,
    Deliver,
    Release,
}

/// Semantic observation of the outermost composed stash, independent of its
/// structural nesting depth.
pub trait StashStatus {
    fn stashed_messages(&self) -> usize;
}

/// Deliberately unbounded FIFO holding for an infallible inner fold.
///
/// Prefer [`super::Stash`] when original-input retention must be finite. This
/// explicit policy needs no refusal recipient, including for Never protocols
/// and compositions whose selector never holds an input.
///
/// Releasing one command may replay several retained mailbox inputs in one
/// transition. Because [`behavior::Actions`] has no rollback effect, a
/// fallible inner fold could reject after earlier replayed inputs had already
/// produced unreturnable actions. Such an inner behavior is therefore not a
/// valid `UnlimitedStash` composition:
///
/// ```compile_fail,E0277
/// struct Fallible;
/// impl behavior::Protocol for Fallible { type Addr = behavior::MailAddr; type Msg = (); }
/// impl behavior::Behavior for Fallible {
///     type Protocol = Self;
///     type Event = behavior::User<behavior::MailAddr, ()>;
///     type Sends = Vec<behavior::Never>;
///     type Ph = behavior::Never;
///     type Error = u8;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Err(1)
///     }
/// }
/// fn route(_: &()) -> behavior_actors::StashRoute { behavior_actors::StashRoute::Release }
/// let _ = behavior_actors::UnlimitedStash::new(Fallible, |_, message| route(message));
/// ```
pub struct UnlimitedStash<B: Behavior> {
    inner: B,
    route: fn(&B, &behavior::BehaviorMessage<B>) -> StashRoute,
    held: VecDeque<User<behavior::BehaviorAddr<B>, behavior::BehaviorMessage<B>>>,
}

// Still-undelegated originals belong to the actor even if selection or inner
// execution unwinds. Reunite the unread suffix after the already reheld prefix.
struct HeldInputs<'a, A: Address, M> {
    held: &'a mut VecDeque<User<A, M>>,
    unread: VecDeque<User<A, M>>,
}

impl<A: Address, M> Drop for HeldInputs<'_, A, M> {
    fn drop(&mut self) {
        self.held.append(&mut self.unread);
    }
}

impl<B: Behavior> UnlimitedStash<B> {
    /// Take all held messages in arrival order, leaving this stash empty.
    ///
    /// Original senders and messages move to the caller without cloning.
    /// The inner behavior and routing policy stay unchanged. This operation
    /// does not replay messages or produce actor effects.
    ///
    /// Messages still in a runtime mailbox belong to that mailbox, not this
    /// stash. A second call returns an empty queue.
    ///
    /// With unwinding panics, a replay selector retains its borrowed original;
    /// inner execution retains only originals not yet delegated to it. Neither
    /// case rolls back inner mutations or recovers consumed inputs or Actions
    /// that the panicking transition never returned. Aborts are not recoverable.
    #[must_use]
    pub fn take(
        &mut self,
    ) -> VecDeque<User<behavior::BehaviorAddr<B>, behavior::BehaviorMessage<B>>> {
        core::mem::take(&mut self.held)
    }
}

impl<B: Behavior<Ph = Never>> UnlimitedStash<B> {
    /// Wrap `inner` with `route`, a pure decision over the actual inner state and message.
    ///
    /// Stashed messages retain FIFO order and ownership until a later
    /// [`StashRoute::Release`]. Each replay borrows the state returned by the
    /// preceding inner transition; the route must perform no effect.
    /// Construction performs no transition or runtime
    /// operation.
    #[must_use]
    pub fn new(inner: B, route: fn(&B, &behavior::BehaviorMessage<B>) -> StashRoute) -> Self
    where
        B::Error: StaticallyInfallible,
    {
        Self {
            inner,
            route,
            held: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn held(&self) -> usize {
        self.held.len()
    }

    pub(super) fn route(&self, message: &behavior::BehaviorMessage<B>) -> StashRoute {
        (self.route)(&self.inner, message)
    }
}

impl<B: Behavior<Ph = Never>> StashStatus for UnlimitedStash<B> {
    fn stashed_messages(&self) -> usize {
        self.held()
    }
}

impl<B> behavior::BehaviorBase for UnlimitedStash<B>
where
    B: Behavior<Ph = Never> + behavior::BehaviorBase,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, A, Sends, Br> UnlimitedStash<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Never, Sends = Sends, Birth = Br>,
    B::Error: StaticallyInfallible,
    B::Protocol: behavior::Protocol<Addr = A>,
{
    pub(super) fn receive(
        &mut self,
        user: User<behavior::BehaviorAddr<B>, behavior::BehaviorMessage<B>>,
        route: StashRoute,
    ) -> Result<Actions<behavior::BehaviorAddr<B>, Never, B::Sends, B::Birth>, B::Error> {
        match route {
            StashRoute::Stash => {
                self.held.push_back(user);
                Ok(Actions::cont())
            }
            StashRoute::Deliver => behavior::delegate_transition(
                &mut self.inner,
                B::Event::user(user.from, user.message),
            ),
            StashRoute::Release => {
                let mut actions = behavior::delegate_transition(
                    &mut self.inner,
                    B::Event::user(user.from, user.message),
                )?;
                if !matches!(actions.become_, Step::Stop(_)) {
                    self.drain_into(&mut actions)?;
                }
                Ok(actions)
            }
        }
    }

    fn drain_into(
        &mut self,
        acc: &mut Actions<behavior::BehaviorAddr<B>, Never, B::Sends, B::Birth>,
    ) -> Result<(), B::Error> {
        let mut inputs = HeldInputs {
            unread: core::mem::take(&mut self.held),
            held: &mut self.held,
        };
        while let Some(user) = inputs.unread.front() {
            let route = (self.route)(&self.inner, &user.message);
            let Some(user) = inputs.unread.pop_front() else {
                break;
            };
            match route {
                StashRoute::Stash => inputs.held.push_back(user),
                StashRoute::Deliver | StashRoute::Release => {
                    let actions = behavior::delegate_transition(
                        &mut self.inner,
                        B::Event::user(user.from, user.message),
                    )?;
                    acc.sends.append(actions.sends);
                    acc.creates.extend(actions.creates);
                    if let Step::Stop(exit) = actions.become_ {
                        acc.become_ = Step::Stop(exit);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl<B, A, Sends, Br> Behavior for UnlimitedStash<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Never, Sends = Sends, Birth = Br>,
    B::Error: StaticallyInfallible,
    B::Protocol: behavior::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = B::Event;
    type Sends = Sends;
    type Ph = Never;
    type Error = B::Error;
    type Birth = Br;

    fn init(
        &mut self,
        _: behavior::InitializationTurn,
    ) -> Result<Actions<A, Never, Sends, Br>, Self::Error> {
        behavior::initialize(&mut self.inner)
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        event: B::Event,
    ) -> Result<Actions<A, Never, Sends, Br>, Self::Error> {
        let user = match event.into_user() {
            Ok(user) => user,
            Err(other) => return behavior::delegate_transition(&mut self.inner, other),
        };
        let route = self.route(&user.message);
        self.receive(user, route)
    }
}
