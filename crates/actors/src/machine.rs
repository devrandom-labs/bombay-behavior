//! A finite-state behavior derived solely from receive and become.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, NoBirths, User};
use behavior::{Never, Step, Stopped};

pub enum Move<P> {
    Stay,
    Goto(P),
    Defer,
    Stop,
}

/// Controlled machine rejection with the complete unaccepted mailbox input.
#[derive(Clone, PartialEq, Eq)]
pub struct MachineError<A, M, E> {
    /// Exact user event whose transition or induced drain was rejected.
    pub event: User<A, M>,
    /// Domain error returned by the phase function.
    pub cause: E,
}

impl<A, M, E: core::fmt::Debug> core::fmt::Debug for MachineError<A, M, E> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MachineError")
            .field("event", &"<retained>")
            .field("cause", &self.cause)
            .finish()
    }
}

impl<A, M, E> core::fmt::Display for MachineError<A, M, E>
where
    E: core::fmt::Display,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "machine rejected its mailbox input: {}",
            self.cause
        )
    }
}

impl<A, M, E> std::error::Error for MachineError<A, M, E> where E: std::error::Error + 'static {}

enum Advance {
    Continue,
    PhaseChanged,
    Stop,
}

pub struct Machine<A: Address, S, M, P, E> {
    state: S,
    phase: P,
    on: fn(P, &mut S, &M) -> Result<Move<P>, E>,
    held: VecDeque<M>,
    address: core::marker::PhantomData<A>,
}

impl<A, S, M, P, E> Clone for Machine<A, S, M, P, E>
where
    A: Address,
    S: Clone,
    M: Clone,
    P: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            phase: self.phase.clone(),
            on: self.on,
            held: self.held.clone(),
            address: core::marker::PhantomData,
        }
    }
}

impl<A, S, M, P, E> crate::BehaviorBase for Machine<A, S, M, P, E>
where
    A: Address,
    P: Copy + PartialEq,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
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

    fn advance(&mut self, message: M) -> Result<Advance, E> {
        Ok(match (self.on)(self.phase, &mut self.state, &message)? {
            Move::Stay => Advance::Continue,
            Move::Defer => {
                self.held.push_back(message);
                Advance::Continue
            }
            Move::Stop => Advance::Stop,
            Move::Goto(next) => {
                let changed = next != self.phase;
                self.phase = next;
                if changed {
                    Advance::PhaseChanged
                } else {
                    Advance::Continue
                }
            }
        })
    }

    fn drain(&mut self) -> Result<Step<Never, Stopped>, E> {
        let mut batch = core::mem::take(&mut self.held);
        while let Some(message) = batch.pop_front() {
            let outcome = match self.advance(message) {
                Ok(transition) => transition,
                Err(error) => {
                    self.held.extend(batch);
                    return Err(error);
                }
            };
            match outcome {
                Advance::Continue => {}
                Advance::PhaseChanged => batch.extend(self.held.drain(..)),
                Advance::Stop => {
                    self.held.extend(batch);
                    return Ok(Step::Stop(Stopped));
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<A, S, M, P, E> behavior::Protocol for Machine<A, S, M, P, E>
where
    A: Address,
    P: Copy + PartialEq,
{
    type Addr = A;
    type Msg = M;
}

impl<A, S, M, P, E> Behavior for Machine<A, S, M, P, E>
where
    A: Address,
    S: Clone,
    M: Clone,
    P: Copy + PartialEq,
{
    type Protocol = Self;
    type Event = User<A, M>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = MachineError<A, M, E>;
    type Birth = NoBirths;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<Actions<A, Never, Self::Sends, NoBirths>, Self::Error> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<Actions<A, Never, Self::Sends, NoBirths>, Self::Error> {
        let mut staged = self.clone();
        let actions = match staged.advance(event.message.clone()) {
            Err(cause) => return Err(MachineError { event, cause }),
            Ok(Advance::Stop) => Actions::stop(),
            Ok(Advance::PhaseChanged) => match staged.drain() {
                Ok(step) => Actions::just(step),
                Err(cause) => return Err(MachineError { event, cause }),
            },
            Ok(Advance::Continue) => Actions::cont(),
        };
        *self = staged;
        Ok(actions)
    }
}
