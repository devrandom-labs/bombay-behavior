//! Bounded policy buffering over ordinary typed delivery.

use std::collections::VecDeque;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, MessageProtocol, Never,
    NoBirths, Recipient, SendEffects, User,
};
use thiserror::Error;

use crate::DeliveryRoute;

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
pub struct Buffered<T, Route> {
    /// Value owned by the buffer.
    pub value: T,
    /// Typed outcome recipient supplied with the original offer.
    pub reply_to: Route,
}

/// Complete valid state product of a [`Buffer`].
pub struct BufferState<T, Route> {
    /// Positive maximum accepted queue length.
    pub capacity: usize,
    /// Exhaustive policy used at capacity.
    pub overflow: OverflowPolicy,
    queued: VecDeque<Buffered<T, Route>>,
}

impl<T, Route> BufferState<T, Route> {
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
    pub fn queued(&self) -> impl ExactSizeIterator<Item = &Buffered<T, Route>> {
        self.queued.iter()
    }
}

/// Commands accepted by [`Buffer`].
pub enum BufferMessage<A: Address, T, Route> {
    /// Offer ownership of one value under the configured overflow policy.
    Offer {
        /// Offered value.
        value: T,
        /// Recipient for acceptance, rejection, or later eviction.
        reply_to: Route,
    },
    /// Release the oldest accepted value to a concrete typed destination.
    Release {
        /// Destination for the released value.
        to: Recipient<MessageProtocol<A, T>>,
        /// Recipient for the release or empty result.
        reply_to: Route,
    },
}

impl<A: Address, T, Route> behavior::Protocol for BufferMessage<A, T, Route> {
    type Addr = A;
    type Msg = BufferMessage<A, T, Route>;
}

/// Named delivery products emitted by [`Buffer`].
pub struct BufferSends<A: Address, T, OutcomeSends: SendEffects> {
    /// Released values in FIFO order.
    pub deliveries: Vec<Delivery<MessageProtocol<A, T>>>,
    /// Acceptance, rejection, eviction, release, and empty facts.
    pub outcomes: OutcomeSends,
}

impl<A: Address, T, OutcomeSends: SendEffects> SendEffects for BufferSends<A, T, OutcomeSends> {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            outcomes: OutcomeSends::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.outcomes.append(other.outcomes);
    }
}

impl<Event, A, T, OutcomeSends> behavior::SendsFor<Event> for BufferSends<A, T, OutcomeSends>
where
    A: Address,
    OutcomeSends: SendEffects + behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, A, T, OutcomeSends> behavior::InterpretSends<I, RootEvent, Path>
    for BufferSends<A, T, OutcomeSends>
where
    I: behavior::SendInterpreter,
    A: Address,
    Vec<Delivery<MessageProtocol<A, T>>>: behavior::InterpretSends<I, RootEvent, Path>,
    OutcomeSends: SendEffects + behavior::InterpretSends<I, RootEvent, Path>,
    BufferSends<A, T, OutcomeSends>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.deliveries, interpreter).await?;
            behavior::InterpretSends::interpret(self.outcomes, interpreter).await
        }
    }
}

/// Invalid buffer definition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum BufferConfigError {
    /// A zero-capacity buffer could never accept ownership.
    #[error("buffer capacity must be positive")]
    ZeroCapacity,
}

/// Validated, protocol-independent buffer policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferConfiguration {
    capacity: usize,
    overflow: OverflowPolicy,
}

impl BufferConfiguration {
    /// Validate policy before it is bound to any actor protocol.
    ///
    /// # Errors
    ///
    /// Returns [`BufferConfigError::ZeroCapacity`] when `capacity` is zero.
    pub fn new(capacity: usize, overflow: OverflowPolicy) -> Result<Self, BufferConfigError> {
        if capacity == 0 {
            return Err(BufferConfigError::ZeroCapacity);
        }
        Ok(Self { capacity, overflow })
    }
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
pub struct Buffer<A, T, Route>
where
    A: Address,
    Route: DeliveryRoute<MessageProtocol<A, BufferOutcome<T>>>,
{
    state: BufferState<T, Route>,
    marker: core::marker::PhantomData<fn() -> A>,
}

impl<A, T, Route> Buffer<A, T, Route>
where
    A: Address,
    Route: DeliveryRoute<MessageProtocol<A, BufferOutcome<T>>>,
{
    /// Bind validated policy to an empty buffer actor.
    #[must_use]
    pub fn new(configuration: BufferConfiguration) -> Self {
        Self {
            state: BufferState {
                capacity: configuration.capacity,
                overflow: configuration.overflow,
                queued: VecDeque::with_capacity(configuration.capacity),
            },
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow the complete current buffer state.
    #[must_use]
    pub const fn state(&self) -> &BufferState<T, Route> {
        &self.state
    }

    fn actions(
        deliveries: Vec<Delivery<MessageProtocol<A, T>>>,
        outcomes: Route::Sends,
    ) -> Actions<A, Never, BufferSends<A, T, Route::Sends>, NoBirths> {
        Actions::send(BufferSends {
            deliveries,
            outcomes,
        })
    }
}

impl<A, T, Route> BehaviorBase for Buffer<A, T, Route>
where
    A: Address,
    Route: DeliveryRoute<MessageProtocol<A, BufferOutcome<T>>>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, T, Route> behavior::Protocol for Buffer<A, T, Route>
where
    A: Address,
    Route: DeliveryRoute<MessageProtocol<A, BufferOutcome<T>>>,
{
    type Addr = A;
    type Msg = BufferMessage<A, T, Route>;
}

impl<A, T, Route> Behavior for Buffer<A, T, Route>
where
    A: Address,
    Route: DeliveryRoute<MessageProtocol<A, BufferOutcome<T>>> + Clone,
    Route::Sends: behavior::SendsFor<User<A, BufferMessage<A, T, Route>>>,
{
    type Protocol = BufferMessage<A, T, Route>;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = BufferSends<A, T, Route::Sends>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            BufferMessage::Offer { value, reply_to }
                if self.state.queued.len() < self.state.capacity =>
            {
                self.state.queued.push_back(Buffered {
                    value,
                    reply_to: reply_to.clone(),
                });
                Ok(Self::actions(
                    Vec::new(),
                    reply_to.deliver(BufferOutcome::Accepted {
                        depth: self.state.queued.len(),
                    }),
                ))
            }
            BufferMessage::Offer { value, reply_to } => match self.state.overflow {
                OverflowPolicy::Reject => Ok(Self::actions(
                    Vec::new(),
                    reply_to.deliver(BufferOutcome::Rejected {
                        value,
                        reason: BufferRejection::Full,
                    }),
                )),
                OverflowPolicy::DropNewest => Ok(Self::actions(
                    Vec::new(),
                    reply_to.deliver(BufferOutcome::Rejected {
                        value,
                        reason: BufferRejection::DroppedNewest,
                    }),
                )),
                OverflowPolicy::DropOldest => {
                    // `new` rejects zero capacity and this arm is reached only
                    // after `len >= capacity`, so the queue is non-empty.
                    let evicted = self
                        .state
                        .queued
                        .pop_front()
                        .expect("positive full buffer contains an oldest value");
                    let mut outcomes = evicted.reply_to.deliver(BufferOutcome::Evicted {
                        value: evicted.value,
                    });
                    self.state.queued.push_back(Buffered {
                        value,
                        reply_to: reply_to.clone(),
                    });
                    outcomes.append(reply_to.deliver(BufferOutcome::Accepted {
                        depth: self.state.queued.len(),
                    }));
                    Ok(Self::actions(Vec::new(), outcomes))
                }
            },
            BufferMessage::Release { to, reply_to } => {
                let Some(buffered) = self.state.queued.pop_front() else {
                    return Ok(Self::actions(
                        Vec::new(),
                        reply_to.deliver(BufferOutcome::Empty),
                    ));
                };
                Ok(Self::actions(
                    vec![Delivery::new(to, buffered.value)],
                    reply_to.deliver(BufferOutcome::Released {
                        remaining: self.state.queued.len(),
                    }),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    fn active(
        policy: OverflowPolicy,
    ) -> crate::Active<Buffer<MailAddr, u8, Recipient<MessageProtocol<MailAddr, BufferOutcome<u8>>>>>
    {
        Buffer::new(BufferConfiguration::new(2, policy).unwrap())
            .initialize()
            .unwrap()
            .behavior
    }

    #[test]
    fn zero_capacity_is_rejected_before_ownership_is_possible() {
        assert!(matches!(
            BufferConfiguration::new(0, OverflowPolicy::Reject),
            Err(BufferConfigError::ZeroCapacity)
        ));
    }

    #[test]
    fn fifo_release_and_empty_result_use_disjoint_named_lanes() {
        let reply = Recipient::from(MailAddr(8));
        let target = Recipient::from(MailAddr(7));
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
        let first_reply = Recipient::from(MailAddr(1));
        let newest_reply = Recipient::from(MailAddr(2));
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
