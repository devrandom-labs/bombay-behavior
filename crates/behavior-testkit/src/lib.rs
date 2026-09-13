//! Test-only models shared by adversarial integration tests.

use std::collections::VecDeque;

use behavior::{Active, Behavior, BirthMode, CreateChild, SendEffects, Step, Stopped};
use core::marker::PhantomData;

/// A nominal, inert destination used by behavior tests that inspect emitted
/// communications without interpreting a recipient mailbox.
pub struct TestRecipient<M>(PhantomData<fn(M)>);

impl<M> behavior::Protocol for TestRecipient<M> {
    type Addr = behavior::MailAddr;
    type Msg = M;
}

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

/// Why the finite test driver returned control to its caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriveDisposition {
    /// Every queued event was processed without a termination decision.
    MailboxDrained,
    /// The behavior designated termination and the mailbox may retain a suffix.
    BehaviorStopped(Stopped),
}

/// Complete observation of one finite test-driver run.
pub struct Trace<B: Behavior> {
    pub behavior: Active<B>,
    pub sends: B::Sends,
    pub creates: Vec<CreateChild<behavior::BehaviorAddr<B>, <B::Birth as BirthMode>::Child>>,
    pub disposition: DriveDisposition,
    pub transitions: usize,
    pub pending: usize,
}

/// Drive `init` then every queued event through `behavior` until it stops or
/// the mailbox drains. Returns the accumulated effect triple plus driver
/// bookkeeping (transition count and unconsumed tail).
///
/// # Errors
/// Returns the behavior's first controlled failure (`B::Error`).
pub fn drive<B>(definition: B, mailbox: &mut Mailbox<B::Event>) -> Result<Trace<B>, B::Error>
where
    B: Behavior<Ph = behavior::Never>,
{
    let initialized = behavior::Activate::initialize(definition)?;
    let mut behavior = initialized.behavior;
    let mut sends = B::Sends::empty();
    let mut creates = Vec::new();
    let mut transitions = 0;
    let mut actions = initialized.actions;

    let disposition = loop {
        transitions += 1;
        sends.append(actions.sends);
        creates.extend(actions.creates);
        match actions.become_ {
            Step::Continue => {}
            Step::Goto(never) => match never {},
            Step::Stop(stopped) => break DriveDisposition::BehaviorStopped(stopped),
        }

        let Some(event) = mailbox.receive() else {
            break DriveDisposition::MailboxDrained;
        };
        actions = behavior.transition(event)?;
    };

    Ok(Trace {
        behavior,
        sends,
        creates,
        disposition,
        transitions,
        pending: mailbox.pending(),
    })
}
