//! Bounded policy buffering over ordinary typed delivery.

use std::collections::VecDeque;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    SendAlgebra, User,
};
use thiserror::Error;

/// Exhaustive behavior-owned policy when a [`Buffer`] is full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverflowPolicy {
    /// Refuse the offered value and retain the complete queue.
    Reject,
    /// Return the oldest accepted value, then accept the new value.
    DropOldest,
    /// Return the newly offered value without changing the queue.
    DropNewest,
}

/// Why an offered value was not accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferRejection {
    /// The configured policy rejects offers at capacity.
    Full,
    /// The configured policy explicitly selects the newest value for return.
    DroppedNewest,
}

/// Factual result delivered for one buffer command.
#[derive(Debug, PartialEq, Eq)]
pub enum BufferOutcome<T> {
    /// The offered value is now owned by the queue.
    Accepted {
        /// Queue depth after acceptance.
        depth: usize,
    },
    /// The offer was not accepted; ownership is returned here.
    Rejected {
        /// Unaccepted owned value.
        value: T,
        /// Policy reason for rejection.
        reason: BufferRejection,
    },
    /// A previously accepted oldest value was evicted and returned.
    Evicted {
        /// Evicted owned value.
        value: T,
    },
    /// One value was released to its target.
    Released {
        /// Queue depth after release.
        remaining: usize,
    },
    /// A release found no accepted value.
    Empty,
}

/// One accepted value and the recipient that must receive an eviction fact.
pub struct Buffered<T, Reply: Behavior> {
    /// Value owned by the buffer.
    pub value: T,
    /// Typed outcome recipient supplied with the original offer.
    pub reply_to: Recipient<Reply>,
}

/// Complete valid state product of a [`Buffer`].
pub struct BufferState<T, Reply: Behavior> {
    /// Positive maximum accepted queue length.
    pub capacity: usize,
    /// Exhaustive policy used at capacity.
    pub overflow: OverflowPolicy,
    queued: VecDeque<Buffered<T, Reply>>,
}

impl<T, Reply: Behavior> BufferState<T, Reply> {
    /// Number of accepted values currently owned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queued.len()
    }

    /// Whether no accepted value is currently owned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queued.is_empty()
    }

    /// Iterate accepted values in release order.
    #[must_use]
    pub fn queued(&self) -> impl ExactSizeIterator<Item = &Buffered<T, Reply>> {
        self.queued.iter()
    }
}

/// Commands accepted by [`Buffer`].
pub enum BufferMessage<T, Target: Behavior, Reply: Behavior> {
    /// Offer ownership of one value under the configured overflow policy.
    Offer {
        /// Offered value.
        value: T,
        /// Recipient for acceptance, rejection, or later eviction.
        reply_to: Recipient<Reply>,
    },
    /// Release the oldest accepted value to a concrete typed destination.
    Release {
        /// Destination for the released value.
        to: Recipient<Target>,
        /// Recipient for the release or empty result.
        reply_to: Recipient<Reply>,
    },
}

/// Named delivery products emitted by [`Buffer`].
pub struct BufferSends<Target: Behavior, Reply: Behavior> {
    /// Released values in FIFO order.
    pub deliveries: Vec<Delivery<Target>>,
    /// Acceptance, rejection, eviction, release, and empty facts.
    pub outcomes: Vec<Delivery<Reply>>,
}

impl<Target: Behavior, Reply: Behavior> SendAlgebra for BufferSends<Target, Reply> {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            outcomes: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.outcomes.append(&mut other.outcomes);
    }
}

/// Invalid buffer definition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BufferConfigError {
    /// A zero-capacity buffer could never accept ownership.
    #[error("buffer capacity must be positive")]
    ZeroCapacity,
}

/// Bounded FIFO policy behavior over typed actor deliveries.
///
/// State is the named [`BufferState`] product. Offers below capacity commit
/// ownership and report `Accepted`. At capacity, [`OverflowPolicy`] either
/// returns the new value, or returns the oldest value before accepting the new
/// one. Release moves exactly the oldest value to the destination lane and
/// reports the remaining depth; empty release reports `Empty`. No payload is
/// silently discarded. Initialization is empty, transitions do not create
/// actors, and the buffer never terminates itself. FIFO and overflow semantics
/// are Bombay policy. Physical mailbox buffering, admission, fairness, and
/// backpressure remain `bombay-communication` responsibilities. Construction
/// returns [`BufferConfigError`] rather than creating an ownership sink. The
/// internal full-queue eviction uses the proven invariant that positive
/// capacity plus the full-offer branch implies a non-empty queue; callers
/// cannot violate that invariant through the public API.
pub struct Buffer<
    A: Address,
    T,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = BufferOutcome<T>>,
> {
    state: BufferState<T, Reply>,
    address: core::marker::PhantomData<fn() -> (A, Target)>,
}

impl<A, T, Target, Reply> Buffer<A, T, Target, Reply>
where
    A: Address,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = BufferOutcome<T>>,
{
    /// Construct an empty bounded buffer.
    ///
    /// # Errors
    ///
    /// Returns [`BufferConfigError::ZeroCapacity`] when `capacity` is zero.
    pub fn new(capacity: usize, overflow: OverflowPolicy) -> Result<Self, BufferConfigError> {
        if capacity == 0 {
            return Err(BufferConfigError::ZeroCapacity);
        }
        Ok(Self {
            state: BufferState {
                capacity,
                overflow,
                queued: VecDeque::with_capacity(capacity),
            },
            address: core::marker::PhantomData,
        })
    }

    /// Borrow the complete current buffer state.
    #[must_use]
    pub const fn state(&self) -> &BufferState<T, Reply> {
        &self.state
    }

    fn actions(
        deliveries: Vec<Delivery<Target>>,
        outcomes: Vec<Delivery<Reply>>,
    ) -> Actions<A, Never, BufferSends<Target, Reply>, NoBirths> {
        Actions::send(BufferSends {
            deliveries,
            outcomes,
        })
    }
}

impl<A, T, Target, Reply> BehaviorBase for Buffer<A, T, Target, Reply>
where
    A: Address,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = BufferOutcome<T>>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, T, Target, Reply> Behavior for Buffer<A, T, Target, Reply>
where
    A: Address,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = BufferOutcome<T>>,
{
    type Addr = A;
    type Msg = BufferMessage<T, Target, Reply>;
    type Event = User<A, Self::Msg>;
    type Sends = BufferSends<Target, Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            BufferMessage::Offer { value, reply_to }
                if self.state.queued.len() < self.state.capacity =>
            {
                self.state.queued.push_back(Buffered { value, reply_to });
                Ok(Self::actions(
                    Vec::new(),
                    vec![Delivery::new(
                        reply_to,
                        BufferOutcome::Accepted {
                            depth: self.state.queued.len(),
                        },
                    )],
                ))
            }
            BufferMessage::Offer { value, reply_to } => match self.state.overflow {
                OverflowPolicy::Reject => Ok(Self::actions(
                    Vec::new(),
                    vec![Delivery::new(
                        reply_to,
                        BufferOutcome::Rejected {
                            value,
                            reason: BufferRejection::Full,
                        },
                    )],
                )),
                OverflowPolicy::DropNewest => Ok(Self::actions(
                    Vec::new(),
                    vec![Delivery::new(
                        reply_to,
                        BufferOutcome::Rejected {
                            value,
                            reason: BufferRejection::DroppedNewest,
                        },
                    )],
                )),
                OverflowPolicy::DropOldest => {
                    // `new` rejects zero capacity and this arm is reached only
                    // after `len >= capacity`, so the queue is non-empty.
                    let evicted = self
                        .state
                        .queued
                        .pop_front()
                        .expect("positive full buffer contains an oldest value");
                    let eviction = Delivery::new(
                        evicted.reply_to,
                        BufferOutcome::Evicted {
                            value: evicted.value,
                        },
                    );
                    self.state.queued.push_back(Buffered { value, reply_to });
                    Ok(Self::actions(
                        Vec::new(),
                        vec![
                            eviction,
                            Delivery::new(
                                reply_to,
                                BufferOutcome::Accepted {
                                    depth: self.state.queued.len(),
                                },
                            ),
                        ],
                    ))
                }
            },
            BufferMessage::Release { to, reply_to } => {
                let Some(buffered) = self.state.queued.pop_front() else {
                    return Ok(Self::actions(
                        Vec::new(),
                        vec![Delivery::new(reply_to, BufferOutcome::Empty)],
                    ));
                };
                Ok(Self::actions(
                    vec![Delivery::new(to, buffered.value)],
                    vec![Delivery::new(
                        reply_to,
                        BufferOutcome::Released {
                            remaining: self.state.queued.len(),
                        },
                    )],
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use behavior::MailAddr;

    struct Target;
    struct Reply;

    impl Behavior for Target {
        type Addr = MailAddr;
        type Msg = u8;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    impl Behavior for Reply {
        type Addr = MailAddr;
        type Msg = BufferOutcome<u8>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    type TestBuffer = Buffer<MailAddr, u8, Target, Reply>;

    fn active(policy: OverflowPolicy) -> crate::Active<TestBuffer> {
        crate::Compose::new(TestBuffer::new(2, policy).unwrap())
            .initialize()
            .unwrap()
            .behavior
    }

    #[test]
    fn zero_capacity_is_rejected_before_ownership_is_possible() {
        assert!(matches!(
            TestBuffer::new(0, OverflowPolicy::Reject),
            Err(BufferConfigError::ZeroCapacity)
        ));
    }

    #[test]
    fn fifo_release_and_empty_result_use_disjoint_named_lanes() {
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let target = Recipient::<Target>::global(MailAddr(7));
        let mut buffer = active(OverflowPolicy::Reject);
        for value in [1, 2] {
            let accepted = buffer
                .receive(
                    MailAddr(9),
                    BufferMessage::Offer {
                        value,
                        reply_to: reply,
                    },
                )
                .unwrap();
            assert!(accepted.sends.deliveries.is_empty());
            assert!(matches!(
                accepted.sends.outcomes.as_slice(),
                [delivery]
                    if delivery.message == (BufferOutcome::Accepted {
                        depth: usize::from(value),
                    })
            ));
        }
        for (value, remaining) in [(1, 1), (2, 0)] {
            let released = buffer
                .receive(
                    MailAddr(9),
                    BufferMessage::Release {
                        to: target,
                        reply_to: reply,
                    },
                )
                .unwrap();
            assert!(released.sends.deliveries == vec![Delivery::new(target, value)]);
            assert!(matches!(
                released.sends.outcomes.as_slice(),
                [delivery]
                    if delivery.message == (BufferOutcome::Released { remaining })
            ));
        }
        let empty = buffer
            .receive(
                MailAddr(9),
                BufferMessage::Release {
                    to: target,
                    reply_to: reply,
                },
            )
            .unwrap();
        assert!(empty.sends.deliveries.is_empty());
        assert!(matches!(
            empty.sends.outcomes.as_slice(),
            [delivery] if delivery.message == BufferOutcome::Empty
        ));
    }

    #[test]
    fn every_overflow_policy_preserves_or_returns_all_owned_values() {
        let first_reply = Recipient::<Reply>::global(MailAddr(1));
        let newest_reply = Recipient::<Reply>::global(MailAddr(2));
        for policy in [
            OverflowPolicy::Reject,
            OverflowPolicy::DropNewest,
            OverflowPolicy::DropOldest,
        ] {
            let mut buffer = active(policy);
            for value in [10, 11] {
                buffer
                    .receive(
                        MailAddr(9),
                        BufferMessage::Offer {
                            value,
                            reply_to: first_reply,
                        },
                    )
                    .unwrap();
            }
            let overflow = buffer
                .receive(
                    MailAddr(9),
                    BufferMessage::Offer {
                        value: 12,
                        reply_to: newest_reply,
                    },
                )
                .unwrap();
            match policy {
                OverflowPolicy::Reject => assert!(matches!(
                    overflow.sends.outcomes.as_slice(),
                    [delivery]
                        if delivery.message == (BufferOutcome::Rejected {
                            value: 12,
                            reason: BufferRejection::Full,
                        })
                )),
                OverflowPolicy::DropNewest => assert!(matches!(
                    overflow.sends.outcomes.as_slice(),
                    [delivery]
                        if delivery.message == (BufferOutcome::Rejected {
                            value: 12,
                            reason: BufferRejection::DroppedNewest,
                        })
                )),
                OverflowPolicy::DropOldest => {
                    assert!(matches!(
                        overflow.sends.outcomes.as_slice(),
                        [evicted, accepted]
                            if evicted.message == (BufferOutcome::Evicted { value: 10 })
                                && accepted.message == (BufferOutcome::Accepted { depth: 2 })
                    ));
                    assert!(buffer.state().queued().map(|item| item.value).eq([11, 12]));
                }
            }
            assert_eq!(buffer.state().len(), 2);
        }
    }
}
