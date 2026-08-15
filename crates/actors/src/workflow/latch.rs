//! Countdown latch coordination.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};

/// Release fact delivered to every accepted arrival.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LatchReleased;

/// One arrival at a [`Latch`].
///
/// The reply recipient is retained until release. An arrival after release is
/// answered immediately and does not reopen the latch.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LatchMessage<P: Behavior> {
    /// Typed participant recipient awaiting the release fact.
    pub reply_to: Recipient<P>,
}

impl<P: Behavior> LatchMessage<P> {
    /// Construct one owned arrival.
    #[must_use]
    pub const fn arrive(reply_to: Recipient<P>) -> Self {
        Self { reply_to }
    }
}

/// Complete semantic state of a [`Latch`].
pub enum LatchState<P: Behavior> {
    /// More arrivals are required; every listed participant is waiting.
    Counting {
        /// Number of additional arrivals needed, including the next one.
        remaining: usize,
        /// Accepted participants in arrival order.
        waiting: Vec<Recipient<P>>,
    },
    /// The threshold was reached and cannot be reset by this incarnation.
    Released,
}

/// A one-generation countdown latch.
///
/// The state sum is [`LatchState::Counting`] or [`LatchState::Released`]. Each
/// arrival decrements exactly once. The transition reaching zero atomically
/// changes state to `Released` and emits one [`LatchReleased`] delivery to
/// every accepted participant in arrival order. Later arrivals receive an
/// immediate release. Initialization is empty; zero count starts released.
/// There are no rejection, retry, cancellation, or timer paths. The latch
/// never terminates itself. Countdown and ordering are Bombay workflow policy,
/// while typed delivery is interpreted by Address and Communication.
pub struct Latch<A: Address, P: Behavior<Addr = A, Msg = LatchReleased>> {
    state: LatchState<P>,
    address: core::marker::PhantomData<A>,
}

impl<A: Address, P: Behavior<Addr = A, Msg = LatchReleased>> Latch<A, P> {
    /// Construct a latch requiring `count` arrivals.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            state: if count == 0 {
                LatchState::Released
            } else {
                LatchState::Counting {
                    remaining: count,
                    waiting: Vec::with_capacity(count),
                }
            },
            address: core::marker::PhantomData,
        }
    }

    /// Borrow the complete current semantic state.
    #[must_use]
    pub const fn state(&self) -> &LatchState<P> {
        &self.state
    }
}

impl<A: Address, P: Behavior<Addr = A, Msg = LatchReleased>> BehaviorBase for Latch<A, P> {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, P> Behavior for Latch<A, P>
where
    A: Address,
    P: Behavior<Addr = A, Msg = LatchReleased>,
{
    type Addr = A;
    type Msg = LatchMessage<P>;
    type Event = User<A, Self::Msg>;
    type Sends = Vec<Delivery<P>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let current = core::mem::replace(&mut self.state, LatchState::Released);
        let (next, sends) = match current {
            LatchState::Released => (
                LatchState::Released,
                vec![Delivery::new(event.message.reply_to, LatchReleased)],
            ),
            LatchState::Counting {
                remaining,
                mut waiting,
            } if remaining > 1 => {
                waiting.push(event.message.reply_to);
                (
                    LatchState::Counting {
                        remaining: remaining - 1,
                        waiting,
                    },
                    Vec::new(),
                )
            }
            LatchState::Counting { mut waiting, .. } => {
                waiting.push(event.message.reply_to);
                (
                    LatchState::Released,
                    waiting
                        .into_iter()
                        .map(|participant| Delivery::new(participant, LatchReleased))
                        .collect(),
                )
            }
        };
        self.state = next;
        Ok(Actions::send(sends))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    struct Participant;

    impl Behavior for Participant {
        type Addr = MailAddr;
        type Msg = LatchReleased;
        type Event = User<MailAddr, LatchReleased>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[test]
    fn threshold_releases_waiters_in_arrival_order_once() {
        let one = Recipient::<Participant>::global(MailAddr(1));
        let two = Recipient::<Participant>::global(MailAddr(2));
        let late = Recipient::<Participant>::global(MailAddr(3));
        let mut latch = (Latch::<MailAddr, Participant>::new(2))
            .initialize()
            .unwrap()
            .behavior;

        let first = latch
            .receive(MailAddr(9), LatchMessage::arrive(one))
            .unwrap();
        assert!(first.sends.is_empty());
        assert!(matches!(
            latch.state(),
            LatchState::Counting {
                remaining: 1,
                waiting,
            } if waiting == &[one]
        ));

        let released = latch
            .receive(MailAddr(9), LatchMessage::arrive(two))
            .unwrap();
        assert!(
            released.sends
                == vec![
                    Delivery::new(one, LatchReleased),
                    Delivery::new(two, LatchReleased),
                ]
        );
        assert!(released.creates.is_empty());
        assert!(matches!(latch.state(), LatchState::Released));

        let immediate = latch
            .receive(MailAddr(9), LatchMessage::arrive(late))
            .unwrap();
        assert!(immediate.sends == vec![Delivery::new(late, LatchReleased)]);
    }

    #[test]
    fn zero_count_starts_released() {
        let participant = Recipient::<Participant>::global(MailAddr(1));
        let mut latch = (Latch::<MailAddr, Participant>::new(0))
            .initialize()
            .unwrap()
            .behavior;
        assert!(matches!(latch.state(), LatchState::Released));
        let actions = latch
            .receive(MailAddr(9), LatchMessage::arrive(participant))
            .unwrap();
        assert!(actions.sends == vec![Delivery::new(participant, LatchReleased)]);
    }
}
