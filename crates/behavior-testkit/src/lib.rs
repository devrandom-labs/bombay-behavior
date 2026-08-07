//! Test-only models shared by adversarial integration tests.

use std::collections::VecDeque;

use behavior::{Actions, Address, Behavior, BirthMode, Create, Exit, SendAlgebra, Step};

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
pub async fn drive<B, A, Sends, Br>(
    behavior: &mut B,
    mailbox: &mut Mailbox<B::Event>,
) -> Result<Trace<A, Sends, Br::Child>, B::Error>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<
            Addr = A,
            Ph = behavior::Never,
            Sends = Sends,
            Birth = Br,
            Effect = Actions<A, behavior::Never, Sends, Br>,
            Done = Exit<A>,
        >,
{
    let mut sends = Sends::empty();
    let mut creates = Vec::new();
    let mut transitions = 1;
    let initial = behavior.init().await?;
    sends.append(initial.sends);
    creates.extend(initial.creates);
    let mut exit = stopped(initial.become_);

    while exit.is_none() {
        let Some(event) = mailbox.receive() else {
            break;
        };
        let actions = behavior.step(event).await?;
        transitions += 1;
        sends.append(actions.sends);
        creates.extend(actions.creates);
        exit = stopped(actions.become_);
    }

    Ok(Trace {
        sends,
        creates,
        exit,
        transitions,
        pending: mailbox.pending(),
    })
}

fn stopped<A: Address>(step: Step<behavior::Never, Exit<A>>) -> Option<Exit<A>> {
    match step {
        Step::Continue => None,
        Step::Goto(never) => match never {},
        Step::Stop(exit) => Some(exit),
    }
}
