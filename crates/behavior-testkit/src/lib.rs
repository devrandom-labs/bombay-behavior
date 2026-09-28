//! Test-only models shared by adversarial integration tests.

use std::collections::VecDeque;

use behavior_actors::Active;

use behavior_core::{Behavior, BirthMode, CreateChild, SendEffects, Step, Stopped};
use core::marker::PhantomData;

/// A nominal, inert destination used by behavior tests that inspect emitted
/// communications without interpreting a recipient mailbox.
pub struct TestRecipient<M>(PhantomData<fn(M)>);

impl<M> behavior_core::Protocol for TestRecipient<M> {
    type Addr = behavior_core::MailAddr;
    type Msg = M;
}

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

/// Accumulated effects from a finite run that completed without a fold error.
/// The send and creation lanes are appended independently, so this value does
/// not retain turn boundaries or prove interpreter settlement order.
pub struct Trace<B: Behavior> {
    pub behavior: Active<B>,
    pub sends: B::Sends,
    pub creates: Vec<CreateChild<behavior_core::BehaviorAddr<B>, <B::Birth as BirthMode>::Child>>,
    pub disposition: DriveDisposition,
    pub transitions: usize,
    pub pending: usize,
}

/// Drive `init` then every queued event through `behavior` until it stops or
/// the mailbox drains. Returns the accumulated effect triple plus driver
/// bookkeeping (transition count and unconsumed tail). This driver never
/// interprets the effects. It is suitable for aggregate fold assertions, not
/// for a runtime ordering witness.
///
/// # Errors
/// Returns the behavior's first controlled failure (`B::Error`). A later
/// failure drops the active behavior and all successful prefix actions already
/// accumulated by this driver. The mailbox retains only events after the
/// rejected input. Use direct per-turn folds or a typed interpreter trace when
/// a test must retain prefix custody or prove effect ordering.
pub fn drive<B>(definition: B, mailbox: &mut Mailbox<B::Event>) -> Result<Trace<B>, B::Error>
where
    B: Behavior<Ph = behavior_core::Never>,
{
    let initialized = behavior_actors::Activate::initialize(definition)?;
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
