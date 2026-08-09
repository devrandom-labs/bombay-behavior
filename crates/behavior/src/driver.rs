//! A minimal mailbox driver for complete user-lane behaviors.

use communication::{Consumer, Received};

use crate::Exit;
use crate::addressing::Address;
use crate::creation::{BirthMode, Create};
use crate::fold::Behavior;
use crate::sending::SendAlgebra;
use crate::user_event::UserEvent;
use crate::verdict::{Never, Step};

pub struct Transcript<A: Address, Sends, New> {
    pub sends: Sends,
    pub creates: Vec<Create<A, New>>,
    pub exit: Exit<A>,
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
    let mut sends = Sends::empty();
    let mut creates = Vec::new();
    let initial = behavior.init().await?;
    sends.append(initial.sends);
    creates.extend(initial.creates);
    match initial.become_ {
        Step::Continue => {}
        Step::Goto(never) => match never {},
        Step::Stop(exit) => {
            return Ok(Transcript {
                sends,
                creates,
                exit,
            });
        }
    }
    while let Some(received) = mailbox.recv().await {
        let Received::User(message) = received else {
            continue;
        };
        let actions = behavior.step(B::Event::user(from, message)).await?;
        sends.append(actions.sends);
        creates.extend(actions.creates);
        match actions.become_ {
            Step::Continue => {}
            Step::Goto(never) => match never {},
            Step::Stop(exit) => {
                return Ok(Transcript {
                    sends,
                    creates,
                    exit,
                });
            }
        }
    }
    Ok(Transcript {
        sends,
        creates,
        exit: Exit::Collected,
    })
}
