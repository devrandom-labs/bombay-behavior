//! Pure message holding and replay composition.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, BirthMode, SendEffects, User, UserEvent};
use behavior::{Never, Step};

mod sealed {
    pub trait StaticallyInfallible {}
}

/// Statically proven uninhabited transition error accepted by [`Stash`].
///
/// The trait is sealed: replay safety depends on the error being genuinely
/// impossible, so downstream code cannot assert the capability for an
/// inhabited type.
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

/// FIFO message holding for an infallible inner fold.
///
/// Releasing one command may replay several retained mailbox inputs in one
/// transition. Because [`behavior::Actions`] has no rollback effect, a
/// fallible inner fold could reject after earlier replayed inputs had already
/// produced unreturnable actions. Such an inner behavior is therefore not a
/// valid `Stash` composition:
///
/// ```compile_fail,E0271
/// use behavior_actors::{Actions, ActiveTurn, Behavior, BehaviorActed, MailAddr,
///     Never, NoBirths, Stash, StashRoute, User};
/// struct Fallible;
/// impl behavior_actors::Protocol for Fallible { type Addr = MailAddr; type Msg = (); }
/// impl Behavior for Fallible {
///     type Protocol = Self;
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = u8;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
///         Err(1)
///     }
/// }
/// fn route(_: &()) -> StashRoute { StashRoute::Release }
/// fn requires_behavior<B: Behavior>(_: &B) {}
/// requires_behavior(&Stash::new(Fallible, route));
/// ```
pub struct Stash<B: Behavior> {
    inner: B,
    route: fn(&crate::BehaviorMessage<B>) -> StashRoute,
    held: VecDeque<User<crate::BehaviorAddr<B>, crate::BehaviorMessage<B>>>,
}

impl<B: Behavior<Ph = Never>> Stash<B> {
    /// Wrap `inner` with the pure message-routing decision `route`.
    ///
    /// Stashed messages retain FIFO order and ownership until a later
    /// [`StashRoute::Release`]. Construction performs no transition or runtime
    /// operation.
    #[must_use]
    pub fn new(inner: B, route: fn(&crate::BehaviorMessage<B>) -> StashRoute) -> Self {
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
}

impl<B: Behavior<Ph = Never>> StashStatus for Stash<B> {
    fn stashed_messages(&self) -> usize {
        self.held()
    }
}

impl<B> crate::BehaviorBase for Stash<B>
where
    B: Behavior<Ph = Never> + crate::BehaviorBase,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, A, Sends, Br> Stash<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Never, Sends = Sends, Birth = Br>,
    B::Error: StaticallyInfallible,
    B::Protocol: crate::Protocol<Addr = A>,
{
    fn drain_into(
        &mut self,
        acc: &mut Actions<crate::BehaviorAddr<B>, Never, B::Sends, B::Birth>,
    ) -> Result<(), B::Error> {
        let mut batch = core::mem::take(&mut self.held);
        while let Some(user) = batch.pop_front() {
            match (self.route)(&user.message) {
                StashRoute::Stash => self.held.push_back(user),
                StashRoute::Deliver | StashRoute::Release => {
                    let actions = behavior::delegate_transition(
                        &mut self.inner,
                        B::Event::user(user.from, user.message),
                    )?;
                    acc.sends.append(actions.sends);
                    acc.creates.extend(actions.creates);
                    if let Step::Stop(exit) = actions.become_ {
                        self.held.extend(batch);
                        acc.become_ = Step::Stop(exit);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl<B, A, Sends, Br> Behavior for Stash<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Never, Sends = Sends, Birth = Br>,
    B::Error: StaticallyInfallible,
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = B::Event;
    type Sends = Sends;
    type Ph = Never;
    type Error = B::Error;
    type Birth = Br;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<Actions<A, Never, Sends, Br>, Self::Error> {
        behavior::initialize(&mut self.inner)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: B::Event,
    ) -> Result<Actions<A, Never, Sends, Br>, Self::Error> {
        let user = match event.into_user() {
            Ok(user) => user,
            Err(other) => return behavior::delegate_transition(&mut self.inner, other),
        };
        match (self.route)(&user.message) {
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
}
