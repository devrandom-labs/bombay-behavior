//! A minimal mailbox driver for complete user-lane behaviors.

use communication::{Consumer, Received};
use core::ops::ControlFlow;

use crate::Exit;
use crate::actor::{Address, BirthMode, Create};
use crate::calculus::ActionReducer;
use crate::calculus::Behavior;
use crate::calculus::UserEvent;
use crate::effects::SendAlgebra;
use crate::next::Never;

pub struct Transcript<A: Address, Sends, New> {
    pub sends: Sends,
    pub creates: Vec<Create<A, New>>,
    pub exit: Exit<A>,
}

impl<A: Address, Sends, New> Transcript<A, Sends, New> {
    #[must_use]
    pub const fn new(sends: Sends, creates: Vec<Create<A, New>>, exit: Exit<A>) -> Self {
        Self {
            sends,
            creates,
            exit,
        }
    }
}

/// Drive user-lane messages through a complete behavior protocol.
///
/// # Errors
/// Returns the first controlled behavior failure.
pub async fn run<B, C, A, Sends, Br>(
    mut behavior: B,
    mut mailbox: Consumer<C, B::Msg>,
    from: A,
) -> Result<Transcript<A, Sends, Br::Child>, B::Error>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Never, Sends = Sends, Birth = Br>,
{
    let mut fold = ActionReducer::new();
    if let ControlFlow::Break(exit) = fold.push(behavior.init()?) {
        let effects = fold.finish(Some(exit)).effects;
        return Ok(Transcript::new(effects.sends, effects.creates, exit));
    }
    while let Some(received) = mailbox.recv().await {
        let Received::User(message) = received else {
            continue;
        };
        if let ControlFlow::Break(exit) =
            fold.push(behavior.transition(B::Event::user(from, message))?)
        {
            let effects = fold.finish(Some(exit)).effects;
            return Ok(Transcript::new(effects.sends, effects.creates, exit));
        }
    }
    let effects = fold.finish(None).effects;
    Ok(Transcript::new(
        effects.sends,
        effects.creates,
        Exit::Collected,
    ))
}
