//! A finite-state behavior derived solely from receive and become.

use std::collections::VecDeque;

use crate::Exit;
use crate::behavior::{Actions, Address, Behavior, NoBirths, User};
use crate::next::{Never, Step};

pub enum Move<P> {
    Stay,
    Goto(P),
    Defer,
    Stop,
}

pub struct Machine<A: Address, S, M, P, E> {
    state: S,
    phase: P,
    on: fn(P, &mut S, &M) -> Result<Move<P>, E>,
    held: VecDeque<M>,
    address: core::marker::PhantomData<A>,
}

impl<A: Address, S, M, P: Copy + PartialEq, E> Machine<A, S, M, P, E> {
    #[must_use]
    pub fn new(state: S, phase: P, on: fn(P, &mut S, &M) -> Result<Move<P>, E>) -> Self {
        Self {
            state,
            phase,
            on,
            held: VecDeque::new(),
            address: core::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn state(&self) -> &S {
        &self.state
    }

    #[must_use]
    pub fn phase(&self) -> P {
        self.phase
    }

    #[must_use]
    pub fn held(&self) -> usize {
        self.held.len()
    }

    fn advance(&mut self, message: M) -> Result<(Step<Never, Exit<A>>, bool), E> {
        Ok(match (self.on)(self.phase, &mut self.state, &message)? {
            Move::Stay => (Step::Continue, false),
            Move::Defer => {
                self.held.push_back(message);
                (Step::Continue, false)
            }
            Move::Stop => (Step::Stop(Exit::Normal), false),
            Move::Goto(next) => {
                let changed = next != self.phase;
                self.phase = next;
                (Step::Continue, changed)
            }
        })
    }

    fn drain(&mut self) -> Result<Step<Never, Exit<A>>, E> {
        let mut batch: VecDeque<M> = self.held.drain(..).collect();
        while let Some(message) = batch.pop_front() {
            let (verdict, changed) = self.advance(message)?;
            if let Step::Stop(exit) = verdict {
                self.held.extend(batch);
                return Ok(Step::Stop(exit));
            }
            if changed {
                batch.extend(self.held.drain(..));
            }
        }
        Ok(Step::Continue)
    }
}

impl<A, S, M, P, E> Behavior for Machine<A, S, M, P, E>
where
    A: Address,
    P: Copy + PartialEq,
{
    type Addr = A;
    type Msg = M;
    type Event = User<A, M>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = E;
    type Birth = NoBirths;

    fn init(&mut self) -> Result<Actions<A, Never, Self::Sends, NoBirths>, E> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        event: Self::Event,
    ) -> Result<Actions<A, Never, Self::Sends, NoBirths>, E> {
        let (verdict, changed) = self.advance(event.message)?;
        match verdict {
            Step::Stop(exit) => Ok(Actions::stop(exit)),
            _ if changed => Ok(Actions::just(self.drain()?)),
            _ => Ok(Actions::cont()),
        }
    }
}
