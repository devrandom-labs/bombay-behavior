//! Explicitly bounded first-seen delivery policy.

use std::collections::VecDeque;

use super::DeliveryOutcomes;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Protocol,
    Recipient, User,
};
use thiserror::Error;

/// Complete observable retention state of a [`Deduplicator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeduplicatorState<K> {
    /// Positive maximum number of retained keys.
    pub capacity: usize,
    retained: Vec<K>,
}

impl<K> DeduplicatorState<K> {
    /// Retained keys from oldest to newest.
    #[must_use]
    pub fn retained(&self) -> &[K] {
        &self.retained
    }
}

/// Factual outcome of one keyed delivery attempt.
#[derive(Debug, PartialEq, Eq)]
pub enum DeduplicatorOutcome<K, T> {
    /// The key was new, the value was delivered, and retention was committed.
    Delivered {
        /// Accepted key.
        key: K,
        /// Oldest key removed to remain within capacity, if any.
        evicted: Option<K>,
    },
    /// The key is retained, so ownership of the undelivered value is returned.
    Duplicate {
        /// Duplicate key.
        key: K,
        /// Undelivered value.
        value: T,
    },
}

/// Commands accepted by [`Deduplicator`].
pub enum DeduplicatorMessage<K, T, Target: Protocol, Reply: behavior::Protocol> {
    /// Deliver `value` only if `key` is absent from bounded retention.
    Deliver {
        /// Application-defined idempotency key.
        key: K,
        /// Owned value.
        value: T,
        /// Typed destination for a first-seen value.
        to: Recipient<Target>,
        /// Typed recipient for the complete result.
        reply_to: Recipient<Reply>,
    },
}

/// Invalid deduplication-retention definition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum DeduplicatorConfigError {
    /// Zero retention would make every accepted key immediately forgotten.
    #[error("deduplicator capacity must be positive")]
    ZeroCapacity,
}

type DeduplicatorMarker<A, T, Target, Reply> =
    core::marker::PhantomData<fn() -> (A, T, Target, Reply)>;

/// Bounded, deterministic first-seen delivery behavior.
///
/// A retained key rejects delivery and returns the owned payload. A new key is
/// committed before its delivery is emitted; when retention is full, the
/// oldest key is returned in the result before the new key becomes newest.
/// Duplicate attempts do not refresh retention. Initialization is empty, the
/// fold creates no actors, and it never terminates by policy. The finite FIFO
/// retention window is deliberate Bombay policy and makes the limitation of
/// deduplication explicit: an evicted key can be accepted again. Physical
/// mailbox delivery remains a runtime responsibility. Construction rejects
/// zero capacity and transitions have no panic path.
pub struct Deduplicator<
    A: Address,
    K,
    T,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = DeduplicatorOutcome<K, T>>,
> {
    capacity: usize,
    retained: VecDeque<K>,
    marker: DeduplicatorMarker<A, T, Target, Reply>,
}

impl<A, K, T, Target, Reply> Deduplicator<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Eq,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = DeduplicatorOutcome<K, T>>,
{
    /// Construct empty positive bounded retention.
    ///
    /// # Errors
    ///
    /// Returns [`DeduplicatorConfigError::ZeroCapacity`] for zero capacity.
    pub fn new(capacity: usize) -> Result<Self, DeduplicatorConfigError> {
        if capacity == 0 {
            return Err(DeduplicatorConfigError::ZeroCapacity);
        }
        Ok(Self {
            capacity,
            retained: VecDeque::with_capacity(capacity),
            marker: core::marker::PhantomData,
        })
    }

    /// Return a snapshot of the complete retention policy state.
    #[must_use]
    pub fn state(&self) -> DeduplicatorState<K> {
        DeduplicatorState {
            capacity: self.capacity,
            retained: self.retained.iter().cloned().collect(),
        }
    }
}

impl<A, K, T, Target, Reply> BehaviorBase for Deduplicator<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Eq,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = DeduplicatorOutcome<K, T>>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, T, Target, Reply> behavior::Protocol for Deduplicator<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Eq,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = DeduplicatorOutcome<K, T>>,
{
    type Addr = A;
    type Msg = DeduplicatorMessage<K, T, Target, Reply>;
}

impl<A, K, T, Target, Reply> behavior::KeyedProtocol for Deduplicator<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Eq,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = DeduplicatorOutcome<K, T>>,
{
    type Key = behavior::NominalProtocolKey<Self>;
}

impl<A, K, T, Target, Reply> Behavior for Deduplicator<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Eq,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = DeduplicatorOutcome<K, T>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = DeliveryOutcomes<Target, Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let DeduplicatorMessage::Deliver {
            key,
            value,
            to,
            reply_to,
        } = event.message;
        if self.retained.contains(&key) {
            return Ok(Actions::send(DeliveryOutcomes {
                deliveries: Vec::new(),
                outcomes: vec![Delivery::new(
                    reply_to,
                    DeduplicatorOutcome::Duplicate { key, value },
                )],
            }));
        }

        let evicted = if self.retained.len() == self.capacity {
            self.retained.pop_front()
        } else {
            None
        };
        self.retained.push_back(key.clone());
        Ok(Actions::send(DeliveryOutcomes {
            deliveries: vec![Delivery::new(to, value)],
            outcomes: vec![Delivery::new(
                reply_to,
                DeduplicatorOutcome::Delivered { key, evicted },
            )],
        }))
    }
}

#[cfg(test)]
mod tests {
    use crate::Activate as _;
    use behavior::MailAddr;

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
                type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
                type Sends = Vec<Never>;
                type Ph = Never;
                type Error = Never;
                type Birth = NoBirths;
                fn transition(
                    &mut self,
                    _: crate::ActiveTurn,
                    _: Self::Event,
                ) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }
        };
    }

    leaf!(Target, u8);
    leaf!(Reply, DeduplicatorOutcome<u8, u8>);

    type Subject = Deduplicator<MailAddr, u8, u8, Target, Reply>;

    fn deliver(
        subject: &mut crate::Active<Subject>,
        key: u8,
        value: u8,
    ) -> Actions<MailAddr, Never, DeliveryOutcomes<Target, Reply>, NoBirths> {
        subject
            .receive(
                MailAddr(9),
                DeduplicatorMessage::Deliver {
                    key,
                    value,
                    to: Recipient::global(MailAddr(1)),
                    reply_to: Recipient::global(MailAddr(2)),
                },
            )
            .unwrap()
    }

    #[test]
    fn duplicate_returns_value_without_refreshing_retention() {
        let mut subject = (Subject::new(2).unwrap()).initialize().unwrap().behavior;
        let _ = deliver(&mut subject, 1, 10);
        let duplicate = deliver(&mut subject, 1, 11);
        assert!(duplicate.sends.deliveries.is_empty());
        assert!(matches!(
            duplicate.sends.outcomes[0].message,
            DeduplicatorOutcome::Duplicate { key: 1, value: 11 }
        ));
        assert_eq!(subject.state().retained().to_vec(), vec![1]);
    }

    #[test]
    fn eviction_is_explicit_and_allows_later_readmission() {
        let mut subject = (Subject::new(2).unwrap()).initialize().unwrap().behavior;
        let _ = deliver(&mut subject, 1, 10);
        let _ = deliver(&mut subject, 2, 20);
        let third = deliver(&mut subject, 3, 30);
        assert!(matches!(
            third.sends.outcomes[0].message,
            DeduplicatorOutcome::Delivered {
                key: 3,
                evicted: Some(1)
            }
        ));
        assert_eq!(subject.state().retained().to_vec(), vec![2, 3]);
        assert_eq!(deliver(&mut subject, 1, 12).sends.deliveries.len(), 1);
    }

    #[test]
    fn zero_capacity_is_rejected() {
        assert!(matches!(
            Subject::new(0),
            Err(DeduplicatorConfigError::ZeroCapacity)
        ));
    }
}
