//! Pure message holding and replay composition.

use std::collections::VecDeque;

use crate::behavior::{Actions, Address, Behavior, BirthMode, SendAlgebra, User, UserEvent};
use crate::next::{Never, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashRoute {
    Stash,
    Deliver,
    Release,
}

pub struct Stash<B: Behavior> {
    inner: B,
    route: fn(&B::Msg) -> StashRoute,
    held: VecDeque<User<B::Addr, B::Msg>>,
}

impl<B: Behavior<Ph = Never>> Stash<B> {
    #[must_use]
    pub fn new(inner: B, route: fn(&B::Msg) -> StashRoute) -> Self {
        Self {
            inner,
            route,
            held: VecDeque::new(),
        }
    }

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }

    #[must_use]
    pub fn held(&self) -> usize {
        self.held.len()
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
        let mut batch: VecDeque<_> = self.held.drain(..).collect();
        while let Some(user) = batch.pop_front() {
            match (self.route)(&user.message) {
                StashRoute::Stash => self.held.push_back(user),
                StashRoute::Deliver | StashRoute::Release => {
                    let actions = self
                        .inner
                        .transition(B::Event::user(user.from, user.message))?;
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

    fn init(&mut self) -> Result<Actions<A, Never, Sends, Br>, B::Error> {
        self.inner.init()
    }

    fn transition(&mut self, event: B::Event) -> Result<Actions<A, Never, Sends, Br>, B::Error> {
        let user = match event.into_user() {
            Ok(user) => user,
            Err(other) => return self.inner.transition(other),
        };
        match (self.route)(&user.message) {
            StashRoute::Stash => {
                self.held.push_back(user);
                Ok(Actions::cont())
            }
            StashRoute::Deliver => self
                .inner
                .transition(B::Event::user(user.from, user.message)),
            StashRoute::Release => {
                let mut actions = self
                    .inner
                    .transition(B::Event::user(user.from, user.message))?;
                if !matches!(actions.become_, Step::Stop(_)) {
                    self.drain_into(&mut actions)?;
                }
                Ok(actions)
            }
        }
    }
}
