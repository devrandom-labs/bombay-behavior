//! A finite-state behavior derived solely from receive and become.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, NoBirths, User};
use behavior::{Never, Step, Stopped};

use crate::inputs::HeldInputs;

pub enum Move<P> {
    Stay,
    Goto(P),
    Defer,
    Stop,
}

/// Controlled machine rejection with the complete refused input.
///
/// A deferred-input refusal returns that original input and its sender, rather
/// than the earlier mailbox communication that triggered replay. Earlier
/// decisions and actual callback mutations remain in the machine.
#[derive(Clone, PartialEq, Eq, thiserror::Error)]
#[error("machine rejected its mailbox input: {cause}")]
pub struct MachineError<A, M, E> {
    /// Exact direct or deferred user event whose callback refused.
    pub event: User<A, M>,
    /// Domain error returned by the phase function.
    #[source]
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

/// A phase machine with original-input deferral and ordered replay.
///
/// The callback borrows each message and mutates the actual state. Refusal
/// returns the original input through [`MachineError`]; it does not roll back
/// state mutations or earlier successful replay. State and messages need not
/// implement Clone. Deferred retention is unbounded; an outer stash's limit
/// does not bound this separate queue.
pub struct Machine<A: Address, S, M, P, E> {
    state: S,
    phase: P,
    on: fn(P, &mut S, &M) -> Result<Move<P>, E>,
    held: VecDeque<User<A, M>>,
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
        }
    }
}

impl<A, S, M, P, E> behavior::BehaviorBase for Machine<A, S, M, P, E>
where
    A: Address,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A: Address, S, M, P: Copy + PartialEq, E> Machine<A, S, M, P, E> {
    /// Construct a phase machine without processing an input.
    ///
    /// A genuine phase change replays held inputs. Returning the current phase
    /// does not replay them, but still stores the exact chosen phase value.
    /// Stop ends the current drain. The callback must
    /// perform no interpreter effects. Validate before mutation if a domain
    /// refusal must leave a particular state value unchanged.
    #[must_use]
    pub fn new(state: S, phase: P, on: fn(P, &mut S, &M) -> Result<Move<P>, E>) -> Self {
        Self {
            state,
            phase,
            on,
            held: VecDeque::new(),
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

    /// Transfer the remaining original inputs without replay or cloning.
    ///
    /// Reheld inputs precede the unread suffix after Stop, refusal or unwinding
    /// replay. Senders and payloads move together. State, phase and callback
    /// remain unchanged; a second call returns an empty queue. An unwinding
    /// callback retains the held input it only borrowed, but cannot restore
    /// arbitrary state mutations or an incoming value consumed by unwinding.
    #[must_use]
    pub fn take(&mut self) -> VecDeque<User<A, M>> {
        core::mem::take(&mut self.held)
    }

    fn drain(&mut self) -> Result<Step<Never, Stopped>, MachineError<A, M, E>> {
        let mut inputs = HeldInputs {
            unread: core::mem::take(&mut self.held),
            held: &mut self.held,
        };
        while let Some(event) = inputs.unread.front() {
            let decision = (self.on)(self.phase, &mut self.state, &event.message);
            let Some(event) = inputs.unread.pop_front() else {
                break;
            };
            match decision {
                Err(cause) => return Err(MachineError { event, cause }),
                Ok(Move::Stay) => {}
                Ok(Move::Defer) => inputs.held.push_back(event),
                Ok(Move::Stop) => return Ok(Step::Stop(Stopped)),
                Ok(Move::Goto(next)) => {
                    let previous = core::mem::replace(&mut self.phase, next);
                    if next != previous {
                        inputs.unread.append(inputs.held);
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<A: Address, S, M, P, E> behavior::Protocol for Machine<A, S, M, P, E> {
    type Addr = A;
    type Msg = M;
}

impl<A, S, M, P, E> Behavior for Machine<A, S, M, P, E>
where
    A: Address,
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
        _: behavior::InitializationTurn,
    ) -> Result<Actions<A, Never, Self::Sends, NoBirths>, Self::Error> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        event: Self::Event,
    ) -> Result<Actions<A, Never, Self::Sends, NoBirths>, Self::Error> {
        match (self.on)(self.phase, &mut self.state, &event.message) {
            Err(cause) => Err(MachineError { event, cause }),
            Ok(Move::Stay) => Ok(Actions::cont()),
            Ok(Move::Defer) => {
                self.held.push_back(event);
                Ok(Actions::cont())
            }
            Ok(Move::Stop) => Ok(Actions::stop()),
            Ok(Move::Goto(next)) => {
                let previous = core::mem::replace(&mut self.phase, next);
                if next == previous {
                    return Ok(Actions::cont());
                }
                self.drain().map(Actions::just)
            }
        }
    }
}
