//! Pure message holding and replay composition.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, BirthMode, SendAlgebra, User, UserEvent};
use behavior::{Never, Step};

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

pub struct Stash<B: Behavior> {
    inner: B,
    route: fn(&B::Msg) -> StashRoute,
    held: VecDeque<User<B::Addr, B::Msg>>,
}

impl<B: Behavior<Ph = Never>> Stash<B> {
    /// Wrap `inner` with the pure message-routing decision `route`.
    ///
    /// Stashed messages retain FIFO order and ownership until a later
    /// [`StashRoute::Release`]. Construction performs no transition or runtime
    /// operation.
    #[must_use]
    pub fn new(inner: B, route: fn(&B::Msg) -> StashRoute) -> Self {
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
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Never, Sends = Sends, Birth = Br>,
{
    fn drain_into(
        &mut self,
        acc: &mut Actions<B::Addr, Never, B::Sends, B::Birth>,
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
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Never, Sends = Sends, Birth = Br>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = B::Event;
    type Sends = Sends;
    type Ph = Never;
    type Error = B::Error;
    type Birth = Br;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<Actions<A, Never, Sends, Br>, B::Error> {
        behavior::initialize(&mut self.inner)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: B::Event,
    ) -> Result<Actions<A, Never, Sends, Br>, B::Error> {
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
