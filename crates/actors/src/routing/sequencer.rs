//! Gap-buffered delivery in an explicit sequence domain.

use std::collections::BTreeMap;

use super::DeliveryOutcomes;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol,
    SendEffects, User,
};

use crate::DeliveryRoute;

/// A position in a [`Sequencer`] stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(pub u64);

/// Complete lifecycle state of a [`Sequencer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequencerState {
    /// The next position can still be accepted.
    Active {
        /// Position whose delivery releases the next contiguous run.
        expected: Sequence,
        /// Number of accepted values waiting beyond a gap.
        buffered: usize,
    },
    /// `Sequence(u64::MAX)` was delivered; no successor is representable.
    Exhausted,
}

/// Factual result of offering one sequenced value.
#[derive(Debug, PartialEq, Eq)]
pub enum SequencerOutcome<T> {
    /// The value was accepted and this many contiguous values were delivered.
    Accepted {
        /// Number released by this offer, including the offered value when it
        /// filled the current position.
        released: usize,
        /// Number still retained beyond a gap.
        buffered: usize,
    },
    /// The position precedes the current expected position.
    Stale {
        /// Rejected sequence position.
        sequence: Sequence,
        /// Rejected value, returned without cloning or loss.
        value: T,
        /// Current expected position.
        expected: Sequence,
    },
    /// A value already occupies this future position.
    Duplicate {
        /// Rejected value, returned without replacing the accepted value.
        value: T,
        /// Occupied position.
        sequence: Sequence,
    },
    /// The sequence domain has no representable successor.
    Exhausted {
        /// Rejected sequence position.
        sequence: Sequence,
        /// Rejected value.
        value: T,
    },
}

/// Commands accepted by [`Sequencer`].
pub enum SequencerMessage<T, TargetRoute, ReplyRoute> {
    /// Offer one value for delivery after all preceding positions.
    Offer {
        /// Explicit sequence position.
        sequence: Sequence,
        /// Owned value.
        value: T,
        /// Destination used when this position becomes contiguous.
        to: TargetRoute,
        /// Recipient for the complete offer outcome.
        reply_to: ReplyRoute,
    },
}

struct Pending<T, TargetRoute> {
    value: T,
    to: TargetRoute,
}

/// Deterministic gap-buffered ordered-delivery policy.
///
/// `Sequencer` accepts each position at most once, retains future positions in
/// a `BTreeMap`, and releases only the contiguous run beginning at `expected`.
/// Stale, duplicate, and exhausted offers return ownership through
/// [`SequencerOutcome`]; an accepted value is never overwritten. Delivering
/// `Sequence(u64::MAX)` moves to the explicit terminal policy state
/// [`SequencerState::Exhausted`] rather than wrapping. Initialization is empty,
/// the template creates no actors, and it does not terminate the hosting actor.
/// Ordered release and bounded integer exhaustion are deliberate Bombay policy;
/// mailbox FIFO and physical delivery remain `bombay-communication`
/// responsibilities. The fold requires no capability beyond ordinary typed
/// sends and has no panic path.
pub struct Sequencer<
    A: Address,
    T,
    TargetRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>>,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = SequencerOutcome<T>>>,
> {
    expected: Option<Sequence>,
    pending: BTreeMap<Sequence, Pending<T, TargetRoute>>,
    marker: core::marker::PhantomData<fn() -> (A, ReplyRoute)>,
}

impl<A, T, TargetRoute, ReplyRoute> Sequencer<A, T, TargetRoute, ReplyRoute>
where
    A: Address,
    TargetRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>>,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = SequencerOutcome<T>>>,
{
    /// Construct an empty sequencer beginning at `first`.
    #[must_use]
    pub fn new(first: Sequence) -> Self {
        Self {
            expected: Some(first),
            pending: BTreeMap::new(),
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow the complete observable lifecycle state.
    #[must_use]
    pub fn state(&self) -> SequencerState {
        match self.expected {
            Some(expected) => SequencerState::Active {
                expected,
                buffered: self.pending.len(),
            },
            None => SequencerState::Exhausted,
        }
    }

    fn actions(
        deliveries: TargetRoute::Sends,
        outcomes: ReplyRoute::Sends,
    ) -> Actions<A, Never, DeliveryOutcomes<TargetRoute::Sends, ReplyRoute::Sends>, NoBirths> {
        Actions::send(DeliveryOutcomes {
            deliveries,
            outcomes,
        })
    }
}

impl<A, T, TargetRoute, ReplyRoute> BehaviorBase for Sequencer<A, T, TargetRoute, ReplyRoute>
where
    A: Address,
    TargetRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>>,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = SequencerOutcome<T>>>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, T, TargetRoute, ReplyRoute> behavior::Protocol for Sequencer<A, T, TargetRoute, ReplyRoute>
where
    A: Address,
    TargetRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>>,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = SequencerOutcome<T>>>,
{
    type Addr = A;
    type Msg = SequencerMessage<T, TargetRoute, ReplyRoute>;
}

impl<A, T, TargetRoute, ReplyRoute> Behavior for Sequencer<A, T, TargetRoute, ReplyRoute>
where
    A: Address,
    TargetRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>>,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = SequencerOutcome<T>>>,
    TargetRoute::Sends: behavior::SendsFor<User<A, SequencerMessage<T, TargetRoute, ReplyRoute>>>,
    ReplyRoute::Sends: behavior::SendsFor<User<A, SequencerMessage<T, TargetRoute, ReplyRoute>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = DeliveryOutcomes<TargetRoute::Sends, ReplyRoute::Sends>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let SequencerMessage::Offer {
            sequence,
            value,
            to,
            reply_to,
        } = event.message;
        let Some(expected) = self.expected else {
            return Ok(Self::actions(
                TargetRoute::Sends::empty(),
                reply_to.deliver(SequencerOutcome::Exhausted { sequence, value }),
            ));
        };
        if sequence < expected {
            return Ok(Self::actions(
                TargetRoute::Sends::empty(),
                reply_to.deliver(SequencerOutcome::Stale {
                    sequence,
                    value,
                    expected,
                }),
            ));
        }
        if self.pending.contains_key(&sequence) {
            return Ok(Self::actions(
                TargetRoute::Sends::empty(),
                reply_to.deliver(SequencerOutcome::Duplicate { value, sequence }),
            ));
        }

        self.pending.insert(sequence, Pending { value, to });
        let mut cursor = expected;
        let mut deliveries = TargetRoute::Sends::empty();
        let mut released = 0;
        while let Some(pending) = self.pending.remove(&cursor) {
            deliveries.append(pending.to.deliver(pending.value));
            released += 1;
            if cursor.0 == u64::MAX {
                self.expected = None;
                break;
            }
            cursor = Sequence(cursor.0 + 1);
            self.expected = Some(cursor);
        }
        let outcome = SequencerOutcome::Accepted {
            released,
            buffered: self.pending.len(),
        };
        Ok(Self::actions(deliveries, reply_to.deliver(outcome)))
    }
}

#[cfg(test)]
mod tests {
    use crate::Activate as _;
    use behavior::{Delivery, MailAddr, Recipient, Step};

    use super::*;

    struct Target;
    struct Reply;

    macro_rules! leaf {
        ($name:ident, $msg:ty) => {
            impl behavior::Protocol for $name {
                type Addr = MailAddr;
                type Msg = $msg;
            }

            impl Behavior for $name {
                type Protocol = Self;
                type Event = behavior::User<MailAddr, crate::BehaviorMessage<Self>>;
                type Sends = Vec<behavior::Never>;
                type Ph = behavior::Never;
                type Error = behavior::Never;
                type Birth = behavior::NoBirths;
                fn transition(
                    &mut self,
                    _: crate::ActiveTurn,
                    _: Self::Event,
                ) -> behavior::BehaviorActed<Self> {
                    Ok(behavior::Actions::cont())
                }
            }
        };
    }

    leaf!(Target, u8);
    leaf!(Reply, SequencerOutcome<u8>);

    type Subject = Sequencer<MailAddr, u8, Recipient<Target>, Recipient<Reply>>;

    fn offer(
        subject: &mut crate::Active<Subject>,
        sequence: u64,
        value: u8,
    ) -> behavior::Actions<
        MailAddr,
        behavior::Never,
        DeliveryOutcomes<Vec<Delivery<Target>>, Vec<Delivery<Reply>>>,
        behavior::NoBirths,
    > {
        subject
            .receive(
                MailAddr(9),
                SequencerMessage::Offer {
                    sequence: Sequence(sequence),
                    value,
                    to: Recipient::global(MailAddr(1)),
                    reply_to: Recipient::global(MailAddr(2)),
                },
            )
            .unwrap()
    }

    #[test]
    fn gaps_release_only_after_the_missing_position_arrives() {
        let mut subject = (Subject::new(Sequence(3))).initialize().unwrap().behavior;
        let future = offer(&mut subject, 4, 40);
        assert!(future.sends.deliveries.is_empty());
        assert_eq!(
            subject.state(),
            SequencerState::Active {
                expected: Sequence(3),
                buffered: 1
            }
        );

        let released = offer(&mut subject, 3, 30);
        assert_eq!(
            released
                .sends
                .deliveries
                .iter()
                .map(|delivery| delivery.message)
                .collect::<Vec<_>>(),
            vec![30, 40]
        );
        assert_eq!(
            subject.state(),
            SequencerState::Active {
                expected: Sequence(5),
                buffered: 0
            }
        );
    }

    #[test]
    fn stale_and_duplicate_offers_return_the_rejected_value() {
        let mut subject = (Subject::new(Sequence(1))).initialize().unwrap().behavior;
        assert!(matches!(
            offer(&mut subject, 2, 20).sends.outcomes[0].message,
            SequencerOutcome::Accepted {
                released: 0,
                buffered: 1
            }
        ));
        let duplicate = offer(&mut subject, 2, 21);
        assert!(matches!(
            duplicate.sends.outcomes[0].message,
            SequencerOutcome::Duplicate {
                value: 21,
                sequence: Sequence(2)
            }
        ));
        assert!(matches!(
            offer(&mut subject, 1, 10).sends.outcomes[0].message,
            SequencerOutcome::Accepted {
                released: 2,
                buffered: 0
            }
        ));
        let stale = offer(&mut subject, 1, 11);
        assert!(matches!(
            stale.sends.outcomes[0].message,
            SequencerOutcome::Stale {
                sequence: Sequence(1),
                value: 11,
                expected: Sequence(3)
            }
        ));
    }

    #[test]
    fn maximum_position_exhausts_without_wrapping() {
        let mut subject = (Subject::new(Sequence(u64::MAX)))
            .initialize()
            .unwrap()
            .behavior;
        let delivered = offer(&mut subject, u64::MAX, 1);
        assert_eq!(delivered.sends.deliveries.len(), 1);
        assert!(matches!(delivered.become_, Step::Continue));
        assert_eq!(subject.state(), SequencerState::Exhausted);
        let rejected = offer(&mut subject, u64::MAX, 2);
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            SequencerOutcome::Exhausted {
                sequence: Sequence(u64::MAX),
                value: 2,
            }
        ));
    }
}
