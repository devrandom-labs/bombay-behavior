//! Bounded least-recently-used value policy.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, MessageProtocol, Never, NoBirths, User,
};
use thiserror::Error;

use crate::DeliveryRoute;

/// One cache entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry<K, V> {
    /// Application-defined key.
    pub key: K,
    /// Owned cached value.
    pub value: V,
}

/// Complete valid cache state ordered least- to most-recently used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheState<K, V> {
    /// Positive maximum number of entries.
    pub capacity: usize,
    entries: Vec<CacheEntry<K, V>>,
}

impl<K, V> CacheState<K, V> {
    /// Entries ordered from next eviction candidate to most recently used.
    #[must_use]
    pub fn entries(&self) -> &[CacheEntry<K, V>] {
        &self.entries
    }

    /// Current retained entry count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache currently retains no entry.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Factual result of one [`Cache`] command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheResult<K, V> {
    /// The value is stored as most recently used.
    Stored {
        /// Stored key.
        key: K,
        /// Previous value replaced at the same key, if any.
        replaced: Option<V>,
        /// Least-recently-used entry evicted for capacity, if any.
        evicted: Option<CacheEntry<K, V>>,
    },
    /// Lookup found and refreshed one value.
    Hit { key: K, value: V },
    /// Lookup found no value.
    Miss { key: K },
    /// Explicit removal returned ownership.
    Removed { key: K, value: V },
    /// Explicit removal found no value.
    Absent { key: K },
}

/// Commands accepted by [`Cache`].
pub enum CacheMessage<K, V, Route> {
    /// Insert or replace one value.
    Put {
        /// Key to store.
        key: K,
        /// Owned value to store.
        value: V,
        /// Typed result recipient.
        reply_to: Route,
    },
    /// Lookup and refresh one key.
    Get {
        /// Key to lookup.
        key: K,
        /// Typed result recipient.
        reply_to: Route,
    },
    /// Remove one key without refreshing another entry.
    Remove {
        /// Key to remove.
        key: K,
        /// Typed result recipient.
        reply_to: Route,
    },
}

/// Invalid cache definition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CacheConfigError {
    /// Zero capacity could never retain accepted ownership.
    #[error("cache capacity must be positive")]
    ZeroCapacity,
}

/// Validated, protocol-independent cache capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheConfiguration {
    capacity: usize,
}

impl CacheConfiguration {
    /// Validate capacity before binding it to key, value, or address types.
    ///
    /// # Errors
    ///
    /// Returns [`CacheConfigError::ZeroCapacity`] for zero capacity.
    pub fn new(capacity: usize) -> Result<Self, CacheConfigError> {
        if capacity == 0 {
            return Err(CacheConfigError::ZeroCapacity);
        }
        Ok(Self { capacity })
    }
}

/// Bounded deterministic recency cache behavior.
///
/// State is [`CacheState`] in least- to most-recent order. Put and successful
/// get move the key to the most-recent position. Replacement returns the old
/// value; capacity eviction returns the complete oldest entry. Miss and absent
/// are factual results, not errors. Every command emits exactly one typed
/// result after committing its state transition. Initialization is empty, no
/// actors are created, and the cache never terminates itself. Recency and
/// eviction are Bombay policy, not durability: Mnesis remains the durable
/// authority. The standard-library vector is intentional for this initial
/// deterministic policy; an `lru` dependency requires a demonstrated scale
/// need and must remain private. No method has a semantic panic condition.
pub struct Cache<A, K, V, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol = MessageProtocol<A, CacheResult<K, V>>>,
{
    state: CacheState<K, V>,
    address: core::marker::PhantomData<fn() -> (A, Route)>,
}

impl<A, K, V, Route> Cache<A, K, V, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol = MessageProtocol<A, CacheResult<K, V>>>,
{
    /// Bind validated capacity to an empty cache actor.
    #[must_use]
    pub fn new(configuration: CacheConfiguration) -> Self {
        Self {
            state: CacheState {
                capacity: configuration.capacity,
                entries: Vec::with_capacity(configuration.capacity),
            },
            address: core::marker::PhantomData,
        }
    }

    /// Borrow the complete current recency state.
    #[must_use]
    pub const fn state(&self) -> &CacheState<K, V> {
        &self.state
    }
}

impl<A, K, V, Route> BehaviorBase for Cache<A, K, V, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol = MessageProtocol<A, CacheResult<K, V>>>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, V, Route> behavior::Protocol for Cache<A, K, V, Route>
where
    A: Address,
    K: Clone + Eq,
    V: Clone,
    Route: DeliveryRoute<Protocol = MessageProtocol<A, CacheResult<K, V>>>,
{
    type Addr = A;
    type Msg = CacheMessage<K, V, Route>;
}

impl<A, K, V, Route> Behavior for Cache<A, K, V, Route>
where
    A: Address,
    K: Clone + Eq,
    V: Clone,
    Route: DeliveryRoute<Protocol = MessageProtocol<A, CacheResult<K, V>>>,
    Route::Sends: behavior::SendsFor<User<A, CacheMessage<K, V, Route>>>,
{
    type Protocol = MessageProtocol<A, CacheMessage<K, V, Route>>;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Route::Sends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let (reply_to, result) = match event.message {
            CacheMessage::Put {
                key,
                value,
                reply_to,
            } => {
                let replaced = self
                    .state
                    .entries
                    .iter()
                    .position(|entry| entry.key == key)
                    .map(|index| self.state.entries.remove(index).value);
                let evicted =
                    if replaced.is_none() && self.state.entries.len() == self.state.capacity {
                        Some(self.state.entries.remove(0))
                    } else {
                        None
                    };
                self.state.entries.push(CacheEntry {
                    key: key.clone(),
                    value,
                });
                (
                    reply_to,
                    CacheResult::Stored {
                        key,
                        replaced,
                        evicted,
                    },
                )
            }
            CacheMessage::Get { key, reply_to } => {
                let result = self
                    .state
                    .entries
                    .iter()
                    .position(|entry| entry.key == key)
                    .map_or_else(
                        || CacheResult::Miss { key: key.clone() },
                        |index| {
                            let entry = self.state.entries.remove(index);
                            let value = entry.value.clone();
                            self.state.entries.push(entry);
                            CacheResult::Hit {
                                key: key.clone(),
                                value,
                            }
                        },
                    );
                (reply_to, result)
            }
            CacheMessage::Remove { key, reply_to } => {
                let result = self
                    .state
                    .entries
                    .iter()
                    .position(|entry| entry.key == key)
                    .map_or_else(
                        || CacheResult::Absent { key: key.clone() },
                        |index| {
                            let entry = self.state.entries.remove(index);
                            CacheResult::Removed {
                                key: key.clone(),
                                value: entry.value,
                            }
                        },
                    );
                (reply_to, result)
            }
        };
        Ok(Actions::send(reply_to.deliver(result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::{MailAddr, Recipient};

    fn put(
        cache: &mut crate::Active<
            Cache<MailAddr, u8, u16, Recipient<MessageProtocol<MailAddr, CacheResult<u8, u16>>>>,
        >,
        reply: Recipient<MessageProtocol<MailAddr, CacheResult<u8, u16>>>,
        key: u8,
        value: u16,
    ) -> CacheResult<u8, u16> {
        cache
            .receive(
                MailAddr(9),
                CacheMessage::Put {
                    key,
                    value,
                    reply_to: reply,
                },
            )
            .unwrap()
            .sends
            .pop()
            .unwrap()
            .message
    }

    #[test]
    fn zero_capacity_is_rejected_before_values_can_be_owned() {
        assert!(matches!(
            CacheConfiguration::new(0),
            Err(CacheConfigError::ZeroCapacity)
        ));
    }

    #[test]
    fn hits_refresh_recency_and_capacity_returns_eviction() {
        let reply = Recipient::from(MailAddr(8));
        let mut cache = Cache::new(CacheConfiguration::new(2).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        assert!(matches!(
            put(&mut cache, reply, 1, 10),
            CacheResult::Stored {
                replaced: None,
                evicted: None,
                ..
            }
        ));
        put(&mut cache, reply, 2, 20);
        let hit = cache
            .receive(
                MailAddr(9),
                CacheMessage::Get {
                    key: 1,
                    reply_to: reply,
                },
            )
            .unwrap();
        assert!(matches!(
            hit.sends.as_slice(),
            [delivery] if delivery.message == (CacheResult::Hit { key: 1, value: 10 })
        ));
        assert!(
            cache
                .state()
                .entries()
                .iter()
                .map(|entry| entry.key)
                .eq([2, 1])
        );

        assert_eq!(
            put(&mut cache, reply, 3, 30),
            CacheResult::Stored {
                key: 3,
                replaced: None,
                evicted: Some(CacheEntry { key: 2, value: 20 }),
            }
        );
        assert!(
            cache
                .state()
                .entries()
                .iter()
                .map(|entry| entry.key)
                .eq([1, 3])
        );
    }

    #[test]
    fn replacement_and_remove_return_every_displaced_value() {
        let reply = Recipient::from(MailAddr(8));
        let mut cache = Cache::new(CacheConfiguration::new(2).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        put(&mut cache, reply, 1, 10);
        assert_eq!(
            put(&mut cache, reply, 1, 11),
            CacheResult::Stored {
                key: 1,
                replaced: Some(10),
                evicted: None,
            }
        );
        let removed = cache
            .receive(
                MailAddr(9),
                CacheMessage::Remove {
                    key: 1,
                    reply_to: reply,
                },
            )
            .unwrap();
        assert!(matches!(
            removed.sends.as_slice(),
            [delivery] if delivery.message == (CacheResult::Removed { key: 1, value: 11 })
        ));
        assert!(cache.state().is_empty());
    }
}
