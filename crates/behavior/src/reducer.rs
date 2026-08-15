//! Pure reduction of transition actions and event streams.

use core::ops::ControlFlow;

use super::{Behavior, delegate_transition, initialize};
use crate::actor::{Address, BirthMode, Create};
use crate::effects::{Actions, SendAlgebra};
use crate::next::{Never, Step, Stopped};

/// The accumulated observable effects of a transition prefix.
pub struct Effects<A: Address, Sends, New> {
    pub sends: Sends,
    pub creates: Vec<Create<A, New>>,
}

/// The result of folding initialization and an event stream.
pub struct Folded<A: Address, Sends, New> {
    pub effects: Effects<A, Sends, New>,
    pub stopped: bool,
    pub transitions: usize,
}

/// A controlled fold failure together with every previously committed effect.
pub struct FoldFailure<A: Address, Sends, New, E> {
    pub effects: Effects<A, Sends, New>,
    pub error: E,
    pub transitions: usize,
}

impl<A: Address, Sends, New, E: core::fmt::Debug> core::fmt::Debug
    for FoldFailure<A, Sends, New, E>
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("FoldFailure")
            .field("error", &self.error)
            .field("transitions", &self.transitions)
            .finish_non_exhaustive()
    }
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
    ) -> ControlFlow<Stopped> {
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
    pub fn finish(self, stopped: bool) -> Folded<A, Sends, New> {
        Folded {
            effects: self.effects,
            stopped,
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
    mut behavior: B,
    events: impl IntoIterator<Item = B::Event>,
) -> Result<
    Folded<B::Addr, B::Sends, <B::Birth as BirthMode>::Child>,
    FoldFailure<B::Addr, B::Sends, <B::Birth as BirthMode>::Child, B::Error>,
>
where
    B: Behavior<Ph = Never>,
{
    let mut reducer = ActionReducer::new();
    let initialization = match initialize(&mut behavior) {
        Ok(actions) => actions,
        Err(error) => {
            let folded = reducer.finish(false);
            return Err(FoldFailure {
                effects: folded.effects,
                error,
                transitions: folded.transitions,
            });
        }
    };
    if let ControlFlow::Break(_stopped) = reducer.push(initialization) {
        return Ok(reducer.finish(true));
    }

    let result = events.into_iter().try_fold((), |(), event| {
        let actions = match delegate_transition(&mut behavior, event) {
            Ok(actions) => actions,
            Err(error) => return ControlFlow::Break(Err(error)),
        };
        match reducer.push(actions) {
            ControlFlow::Continue(()) => ControlFlow::Continue(()),
            ControlFlow::Break(exit) => ControlFlow::Break(Ok(exit)),
        }
    });

    match result {
        ControlFlow::Continue(()) => Ok(reducer.finish(false)),
        ControlFlow::Break(Ok(_stopped)) => Ok(reducer.finish(true)),
        ControlFlow::Break(Err(error)) => {
            let folded = reducer.finish(false);
            Err(FoldFailure {
                effects: folded.effects,
                error,
                transitions: folded.transitions,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actions, Behavior, Delivery, MailAddr, NoBirths, Recipient, User};

    struct Sink;

    struct Accumulator {
        sum: u8,
        stop_at: u8,
    }

    impl Behavior for Accumulator {
        type Addr = MailAddr;
        type Msg = u8;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Delivery<Sink>>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            event: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            self.sum += event.message;
            let sends = vec![Delivery::new(Recipient::global(MailAddr(9)), self.sum)];
            Ok(Actions::new(
                sends,
                Vec::new(),
                if self.sum >= self.stop_at {
                    Step::Stop(Stopped)
                } else {
                    Step::Continue
                },
            ))
        }
    }

    impl Behavior for Sink {
        type Addr = MailAddr;
        type Msg = u8;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[test]
    fn event_fold_short_circuits_and_accepts_capturing_transitions() {
        let behavior = Accumulator { sum: 0, stop_at: 3 };

        let folded = fold_events(
            behavior,
            [
                User::new(MailAddr(1), 1),
                User::new(MailAddr(1), 2),
                User::new(MailAddr(1), 100),
            ],
        )
        .unwrap();

        assert_eq!(folded.transitions, 3); // initialization plus two events
        assert_eq!(folded.effects.sends.len(), 2);
        assert!(folded.stopped);
    }
}
