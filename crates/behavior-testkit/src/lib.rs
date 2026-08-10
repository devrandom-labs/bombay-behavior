//! Test-only models shared by adversarial integration tests.

use std::collections::VecDeque;

use behavior::{ActionReducer, Address, Behavior, BirthMode, Create, Exit, SendAlgebra};
use core::ops::ControlFlow;

pub mod model;

pub struct Mailbox<E> {
    events: VecDeque<E>,
}

impl<E> Mailbox<E> {
    #[must_use]
    pub fn new(events: impl IntoIterator<Item = E>) -> Self {
        Self {
            events: events.into_iter().collect(),
        }
    }

    pub fn receive(&mut self) -> Option<E> {
        self.events.pop_front()
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.events.len()
    }
}

pub struct Trace<A: Address, Sends, New> {
    pub sends: Sends,
    pub creates: Vec<Create<A, New>>,
    pub exit: Option<Exit<A>>,
    pub transitions: usize,
    pub pending: usize,
}

/// Drive `init` then every queued event through `behavior` until it stops or
/// the mailbox drains. Returns the accumulated effect triple plus driver
/// bookkeeping (transition count and unconsumed tail).
///
/// # Errors
/// Returns the behavior's first controlled failure (`B::Error`).
pub fn drive<B, A, Sends, Br>(
    behavior: &mut B,
    mailbox: &mut Mailbox<B::Event>,
) -> Result<Trace<A, Sends, Br::Child>, B::Error>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = behavior::Never, Sends = Sends, Birth = Br>,
{
    let mut fold = ActionReducer::new();
    let mut exit = match fold.push(behavior.init()?) {
        ControlFlow::Continue(()) => None,
        ControlFlow::Break(exit) => Some(exit),
    };

    while exit.is_none() {
        let Some(event) = mailbox.receive() else {
            break;
        };
        exit = match fold.push(behavior.transition(event)?) {
            ControlFlow::Continue(()) => None,
            ControlFlow::Break(exit) => Some(exit),
        };
    }

    let folded = fold.finish(exit);

    Ok(Trace {
        sends: folded.effects.sends,
        creates: folded.effects.creates,
        exit: folded.exit,
        transitions: folded.transitions,
        pending: mailbox.pending(),
    })
}
