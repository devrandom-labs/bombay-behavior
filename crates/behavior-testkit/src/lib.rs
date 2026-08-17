//! Test-only models shared by adversarial integration tests.

use std::collections::VecDeque;

use behavior::{ActionReducer, Active, Address, Behavior, BirthMode, Create, SendAlgebra};
use core::marker::PhantomData;

/// A nominal, inert destination used by behavior tests that inspect emitted
/// communications without interpreting a recipient mailbox.
pub struct TestRecipient<M>(PhantomData<fn(M)>);

impl<M> behavior::Protocol for TestRecipient<M> {
    type Addr = behavior::MailAddr;
    type Msg = M;
}

impl<M> Behavior for TestRecipient<M> {
    type Event = behavior::User<behavior::MailAddr, M>;
    type Sends = Vec<behavior::Never>;
    type Ph = behavior::Never;
    type Error = behavior::Never;
    type Birth = behavior::NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(behavior::Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(behavior::Actions::cont())
    }
}
use core::ops::ControlFlow;

/// Test-fixture shorthand for activating a raw concrete behavior through the
/// same activation boundary used by production definitions.
pub trait InitializeTest: Behavior + Sized {
    /// Activate the fixture and preserve its complete initialization actions.
    ///
    /// # Errors
    ///
    /// Returns the concrete behavior error when its initialization fold
    /// rejects activation.
    fn initialize(self) -> Result<behavior::Initialized<Self>, Self::Error> {
        behavior::Activate::initialize(self)
    }
}

impl<B: Behavior> InitializeTest for B {}

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

pub struct Trace<B: Behavior> {
    pub behavior: Active<B>,
    pub sends: B::Sends,
    pub creates: Vec<Create<B::Addr, <B::Birth as BirthMode>::Child>>,
    pub stopped: bool,
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
    definition: B,
    mailbox: &mut Mailbox<B::Event>,
) -> Result<Trace<B>, B::Error>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = behavior::Never, Sends = Sends, Birth = Br>,
{
    let initialized = behavior::Activate::initialize(definition)?;
    let mut behavior = initialized.behavior;
    let mut fold = ActionReducer::new();
    let mut exit = match fold.push(initialized.actions) {
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

    let folded = fold.finish(exit.is_some());

    Ok(Trace {
        behavior,
        sends: folded.effects.sends,
        creates: folded.effects.creates,
        stopped: folded.stopped,
        transitions: folded.transitions,
        pending: mailbox.pending(),
    })
}
