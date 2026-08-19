//! Concise action results for ordinary infallible, no-birth behaviors.

use crate::{Actions, Address, MailAddr, Never, NoBirths, Step, Stopped};

/// Effects emitted by one ordinary infallible, no-birth behavior turn.
///
/// `Effect` is a truthful shorthand for
/// `Actions<A, Never, Vec<S>, NoBirths>`. It introduces no runtime operation,
/// hidden error path, creation capability, or phase transition. Behaviors that
/// need any of those capabilities use the complete [`Actions`] algebra.
pub struct Effect<S, A: Address = MailAddr> {
    actions: Actions<A, Never, Vec<S>, NoBirths>,
}

impl<S, A: Address> Effect<S, A> {
    /// Continue after emitting no sends.
    #[must_use]
    pub fn none() -> Self {
        Self {
            actions: Actions::cont(),
        }
    }

    /// Continue after emitting one send value.
    #[must_use]
    pub fn send(send: S) -> Self {
        Self {
            actions: Actions::send(vec![send]),
        }
    }

    /// Continue after emitting the supplied send values in order.
    #[must_use]
    pub fn send_all(sends: impl IntoIterator<Item = S>) -> Self {
        Self {
            actions: Actions::send(sends.into_iter().collect()),
        }
    }

    /// Stop normally after preserving this turn's sends.
    #[must_use]
    pub fn stop(mut self) -> Self {
        self.actions.become_ = Step::Stop(Stopped);
        self
    }
}

impl<S, A: Address> From<Effect<S, A>> for Actions<A, Never, Vec<S>, NoBirths> {
    fn from(effect: Effect<S, A>) -> Self {
        effect.actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_preserves_send_order_and_terminal_decision() {
        let actions: Actions<MailAddr, Never, Vec<u8>, NoBirths> =
            Effect::send_all([2, 3, 5]).stop().into();

        assert_eq!(actions.sends, [2, 3, 5]);
        assert!(matches!(actions.become_, Step::Stop(Stopped)));
        assert!(actions.creates.is_empty());
    }
}
