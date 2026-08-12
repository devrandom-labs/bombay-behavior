//! Pure reduction of transition actions and event streams.

use core::ops::ControlFlow;

use super::Behavior;
use crate::Exit;
use crate::actor::{Address, BirthMode, Create};
use crate::effects::{Actions, SendAlgebra};
use crate::next::{Never, Step};

/// The accumulated observable effects of a transition prefix.
pub struct Effects<A: Address, Sends, New> {
    pub sends: Sends,
    pub creates: Vec<Create<A, New>>,
}

/// The result of folding initialization and an event stream.
pub struct Folded<A: Address, Sends, New> {
    pub effects: Effects<A, Sends, New>,
    pub exit: Option<Exit<A>>,
    pub transitions: usize,
}

/// A left fold over Bombay actions.
pub struct ActionReducer<A: Address, Sends, New> {
    effects: Effects<A, Sends, New>,
    transitions: usize,
}

impl<A: Address, Sends: SendAlgebra, New> Default for ActionReducer<A, Sends, New> {
    fn default() -> Self {
        Self {
            effects: Effects {
                sends: Sends::empty(),
                creates: Vec::new(),
            },
            transitions: 0,
        }
    }
}

impl<A: Address, Sends: SendAlgebra, New> ActionReducer<A, Sends, New> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one action value. Order is preserved and the first stop verdict
    /// short-circuits the surrounding fold.
    pub fn push<Birth: BirthMode<Child = New>>(
        &mut self,
        actions: Actions<A, Never, Sends, Birth>,
    ) -> ControlFlow<Exit<A>> {
        self.transitions += 1;
        self.effects.sends.append(actions.sends);
        self.effects.creates.extend(actions.creates);
        match actions.become_ {
            Step::Continue => ControlFlow::Continue(()),
            Step::Goto(never) => match never {},
            Step::Stop(exit) => ControlFlow::Break(exit),
        }
    }

    #[must_use]
    pub fn finish(self, exit: Option<Exit<A>>) -> Folded<A, Sends, New> {
        Folded {
            effects: self.effects,
            exit,
            transitions: self.transitions,
        }
    }
}

/// Initialize a behavior and left-fold events until exhaustion, controlled
/// failure, or the first stop verdict.
///
/// # Errors
/// Returns the first controlled behavior failure.
#[allow(
    clippy::type_complexity,
    reason = "the result exposes every behavior-owned effect and child seat"
)]
pub fn fold_events<B>(
    behavior: &mut B,
    events: impl IntoIterator<Item = B::Event>,
) -> Result<Folded<B::Addr, B::Sends, <B::Birth as BirthMode>::Child>, B::Error>
where
    B: Behavior<Ph = Never>,
{
    let mut reducer = ActionReducer::new();
    if let ControlFlow::Break(exit) = reducer.push(behavior.init()?) {
        return Ok(reducer.finish(Some(exit)));
    }

    let result = events.into_iter().try_fold((), |(), event| {
        let actions = match behavior.transition(event) {
            Ok(actions) => actions,
            Err(error) => return ControlFlow::Break(Err(error)),
        };
        match reducer.push(actions) {
            ControlFlow::Continue(()) => ControlFlow::Continue(()),
            ControlFlow::Break(exit) => ControlFlow::Break(Ok(exit)),
        }
    });

    match result {
        ControlFlow::Continue(()) => Ok(reducer.finish(None)),
        ControlFlow::Break(Ok(exit)) => Ok(reducer.finish(Some(exit))),
        ControlFlow::Break(Err(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Acted, Actions, Delivery, MailAddr, NoBirths, Pure, Recipient, User};

    #[test]
    fn event_fold_short_circuits_and_accepts_capturing_transitions() {
        let stop_at = 3;
        let mut behavior = Pure::from_fn(
            0_u8,
            move |sum: &mut u8,
                  _from: MailAddr,
                  message: u8|
                  -> Acted<
                MailAddr,
                Never,
                Vec<Delivery<MailAddr, u8>>,
                NoBirths,
                Never,
            > {
                *sum += message;
                let sends = vec![Delivery::new(Recipient::global(MailAddr(9)), *sum)];
                Ok(Actions::new(
                    sends,
                    Vec::new(),
                    if *sum >= stop_at {
                        Step::Stop(Exit::Normal)
                    } else {
                        Step::Continue
                    },
                ))
            },
        );

        let folded = fold_events(
            &mut behavior,
            [
                User::new(MailAddr(1), 1),
                User::new(MailAddr(1), 2),
                User::new(MailAddr(1), 100),
            ],
        )
        .unwrap();

        assert_eq!(folded.transitions, 3); // initialization plus two events
        assert_eq!(folded.effects.sends.len(), 2);
        assert!(matches!(folded.exit, Some(Exit::Normal)));
        assert_eq!(behavior.state().state, 3);
    }
}
