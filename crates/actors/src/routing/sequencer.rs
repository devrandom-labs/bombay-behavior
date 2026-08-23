//! Gap-buffered delivery in an explicit sequence domain.

use std::collections::BTreeMap;

use super::DeliveryOutcomes;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Protocol,
    Recipient, User,
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
        /// Rejected value.
        value: T,
    },
}

/// Commands accepted by [`Sequencer`].
pub enum SequencerMessage<T, Target: Protocol, Route> {
    /// Offer one value for delivery after all preceding positions.
    Offer {
        /// Explicit sequence position.
        sequence: Sequence,
        /// Owned value.
        value: T,
        /// Destination used when this position becomes contiguous.
        to: Recipient<Target>,
        /// Recipient for the complete offer outcome.
        reply_to: Route,
    },
}

struct Pending<T, Target: Protocol> {
    value: T,
    to: Recipient<Target>,
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
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = SequencerOutcome<T>>,
    Route: DeliveryRoute<Reply>,
> {
    expected: Option<Sequence>,
    pending: BTreeMap<Sequence, Pending<T, Target>>,
    marker: core::marker::PhantomData<fn() -> (A, Reply, Route)>,
}

impl<A, T, Target, Reply, Route> Sequencer<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = SequencerOutcome<T>>,
    Route: DeliveryRoute<Reply>,
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
        deliveries: Vec<Delivery<Target>>,
        outcomes: Route::Sends,
    ) -> Actions<A, Never, DeliveryOutcomes<Target, Route::Sends>, NoBirths> {
        Actions::send(DeliveryOutcomes {
            deliveries,
            outcomes,
        })
    }
}

impl<A, T, Target, Reply, Route> BehaviorBase for Sequencer<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = SequencerOutcome<T>>,
    Route: DeliveryRoute<Reply>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, T, Target, Reply, Route> behavior::Protocol for Sequencer<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = SequencerOutcome<T>>,
    Route: DeliveryRoute<Reply>,
{
    type Addr = A;
    type Msg = SequencerMessage<T, Target, Route>;
}

impl<A, T, Target, Reply, Route> Behavior for Sequencer<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = SequencerOutcome<T>>,
    Route: DeliveryRoute<Reply>,
    Route::Sends: behavior::SendsFor<User<A, SequencerMessage<T, Target, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = DeliveryOutcomes<Target, Route::Sends>;
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
                Vec::new(),
                reply_to.deliver(SequencerOutcome::Exhausted { value }),
            ));
        };
        if sequence < expected {
            return Ok(Self::actions(
                Vec::new(),
                reply_to.deliver(SequencerOutcome::Stale { value, expected }),
            ));
        }
        if self.pending.contains_key(&sequence) {
            return Ok(Self::actions(
                Vec::new(),
                reply_to.deliver(SequencerOutcome::Duplicate { value, sequence }),
            ));
        }

        self.pending.insert(sequence, Pending { value, to });
        let mut cursor = expected;
        let mut deliveries = Vec::new();
        while let Some(pending) = self.pending.remove(&cursor) {
            deliveries.push(Delivery::new(pending.to, pending.value));
            if cursor.0 == u64::MAX {
                self.expected = None;
                break;
            }
            cursor = Sequence(cursor.0 + 1);
            self.expected = Some(cursor);
        }
        let outcome = SequencerOutcome::Accepted {
            released: deliveries.len(),
            buffered: self.pending.len(),
        };
        Ok(Self::actions(deliveries, reply_to.deliver(outcome)))
    }
}

#[cfg(test)]
mod tests {
    use crate::Activate as _;
    use behavior::{MailAddr, Step};

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

    type Subject = Sequencer<MailAddr, u8, Target, Reply, Recipient<Reply>>;

    fn offer(
        subject: &mut crate::Active<Subject>,
        sequence: u64,
        value: u8,
    ) -> behavior::Actions<
        MailAddr,
        behavior::Never,
        DeliveryOutcomes<Target, Vec<Delivery<Reply>>>,
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
        let _ = offer(&mut subject, 2, 20);
        let duplicate = offer(&mut subject, 2, 21);
        assert!(matches!(
            duplicate.sends.outcomes[0].message,
            SequencerOutcome::Duplicate {
                value: 21,
                sequence: Sequence(2)
            }
        ));
        let _ = offer(&mut subject, 1, 10);
        let stale = offer(&mut subject, 1, 11);
        assert!(matches!(
            stale.sends.outcomes[0].message,
            SequencerOutcome::Stale {
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
            SequencerOutcome::Exhausted { value: 2 }
        ));
    }
}
