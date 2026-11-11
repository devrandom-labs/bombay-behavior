//! A finite-state behavior derived solely from receive and become.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, Creations, NoBirths, Protocol, SendsFor, User};
use behavior::{Never, Step, Stopped};

use crate::DeliveryRoute;
use crate::inputs::HeldInputs;
use crate::retention::{InputLimit, InputRetention};

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
/// implement Clone. [`Self::new`] retains without a bound; [`Self::bounded`]
/// selects finite holding and complete refusal sends. An outer stash's limit
/// does not bound this separate queue. The default unit policy performs actual
/// unbounded holding; callers do not provide a marker or unused recipient.
pub struct Machine<A: Address, S, M, P, E, R = ()> {
    state: S,
    phase: P,
    on: fn(P, &mut S, &M) -> Result<Move<P>, E>,
    held: VecDeque<User<A, M>>,
    retention: R,
}

impl<A, S, M, P, E, R> Clone for Machine<A, S, M, P, E, R>
where
    A: Address,
    S: Clone,
    M: Clone,
    P: Clone,
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            phase: self.phase.clone(),
            on: self.on,
            held: self.held.clone(),
            retention: self.retention.clone(),
        }
    }
}

impl<A, S, M, P, E, R> behavior::BehaviorBase for Machine<A, S, M, P, E, R>
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
            retention: (),
        }
    }

    /// Start finite holding with a real recipient for complete refused inputs.
    ///
    /// Only a newly Defer-selected input needs a slot. When full, its original
    /// User is emitted and the actor continues; callback mutations remain.
    /// Stay, Goto, Stop and callback errors need no slot. Replaying an admitted
    /// input consumes no additional slot. Initialization emits no refusals.
    ///
    /// A recipient for a bare message cannot receive the original sender too:
    ///
    /// ```compile_fail,E0271
    /// fn receive(_: (), _: &mut (), _: &u8) -> Result<behavior_actors::Move<()>, behavior::Never> {
    ///     Ok(behavior_actors::Move::Defer)
    /// }
    /// let refused = behavior::Recipient::<behavior::MessageProtocol<behavior::MailAddr, u8>>::global(behavior::MailAddr(1));
    /// let _ = behavior_actors::Machine::<behavior::MailAddr, _, _, _, _>::bounded((), (), receive, behavior_actors::InputLimit::new(1, refused));
    /// ```
    #[must_use]
    pub fn bounded<R>(
        state: S,
        phase: P,
        on: fn(P, &mut S, &M) -> Result<Move<P>, E>,
        retention: InputLimit<R>,
    ) -> Machine<A, S, M, P, E, InputLimit<R>>
    where
        R: DeliveryRoute + Clone,
        R::Protocol: Protocol<Addr = A, Msg = User<A, M>>,
        R::Sends: SendsFor<User<A, M>>,
    {
        Machine {
            state,
            phase,
            on,
            held: VecDeque::new(),
            retention,
        }
    }
}

impl<A: Address, S, M, P: Copy + PartialEq, E, R> Machine<A, S, M, P, E, R> {
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

impl<A: Address, S, M, P, E, R> behavior::Protocol for Machine<A, S, M, P, E, R> {
    type Addr = A;
    type Msg = M;
}

impl<A, S, M, P, E, R> Behavior for Machine<A, S, M, P, E, R>
where
    A: Address,
    P: Copy + PartialEq,
    R: InputRetention<A, M>,
    R::Sends: SendsFor<User<A, M>>,
{
    type Protocol = Self;
    type Event = User<A, M>;
    type Sends = R::Sends;
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
            Ok(Move::Defer) => Ok(Actions::new(
                self.retention.hold(&mut self.held, event),
                Creations::empty(),
                Step::Continue,
            )),
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
