//! Explicit monotonic release gate for ordered keys.

use std::collections::BTreeMap;

use super::DeliveryOutcomes;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Protocol,
    Recipient, User,
};

use crate::DeliveryRoute;

/// Complete observable state of an [`OrderGate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderGateState<K> {
    /// Greatest explicitly opened key, or absence before the first opening.
    pub watermark: Option<K>,
    held: usize,
}

impl<K> OrderGateState<K> {
    /// Number of values retained above the watermark.
    #[must_use]
    pub fn held(&self) -> usize {
        self.held
    }
}

/// Factual result of one gate operation.
#[derive(Debug, PartialEq, Eq)]
pub enum OrderGateOutcome<K, T> {
    /// A value is retained until the watermark reaches its key.
    Held {
        /// Accepted key.
        key: K,
        /// Current retained count.
        held: usize,
    },
    /// The key is already open, so the value was delivered immediately.
    Delivered {
        /// Accepted key.
        key: K,
    },
    /// Another retained value already owns this key.
    Duplicate {
        /// Rejected key.
        key: K,
        /// Rejected owned value.
        value: T,
    },
    /// One monotonic opening was committed.
    Opened {
        /// New watermark.
        through: K,
        /// Number of retained values released in key order.
        released: usize,
        /// Number still held.
        held: usize,
    },
    /// An opening attempted to move the watermark backwards or repeat it.
    StaleOpening {
        /// Rejected watermark.
        requested: K,
        /// Current watermark.
        current: K,
    },
}

/// Commands accepted by [`OrderGate`].
pub enum OrderGateMessage<K, T, Target: Protocol, Route> {
    /// Submit one keyed value.
    Hold {
        /// Ordered release key.
        key: K,
        /// Owned value.
        value: T,
        /// Typed delivery destination.
        to: Recipient<Target>,
        /// Typed outcome recipient.
        reply_to: Route,
    },
    /// Monotonically open every key through the supplied bound.
    OpenThrough {
        /// Inclusive new watermark.
        through: K,
        /// Typed outcome recipient.
        reply_to: Route,
    },
}

struct Held<T, Target: Protocol> {
    value: T,
    to: Recipient<Target>,
}

/// Deterministic explicit ordered-release policy.
///
/// Before a key is open, one value may be retained for it. Opening is strictly
/// monotonic and atomically releases all retained keys at or below the new
/// watermark in key order. Values submitted at an open key are delivered
/// immediately. Duplicate holds return ownership and never overwrite. A stale
/// opening changes nothing. Initialization is empty, no actors are created,
/// and the host never terminates by policy. Inclusive watermark semantics and
/// deterministic release order are Bombay policy; physical delivery remains a
/// runtime responsibility. Transitions have no semantic panic condition.
pub struct OrderGate<
    A: Address,
    K,
    T,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = OrderGateOutcome<K, T>>,
    Route: DeliveryRoute<Reply>,
> {
    watermark: Option<K>,
    held: BTreeMap<K, Held<T, Target>>,
    marker: core::marker::PhantomData<fn() -> (A, Reply, Route)>,
}

impl<A, K, T, Target, Reply, Route> OrderGate<A, K, T, Target, Reply, Route>
where
    A: Address,
    K: Clone + Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = OrderGateOutcome<K, T>>,
    Route: DeliveryRoute<Reply>,
{
    /// Construct a closed gate with no retained values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            watermark: None,
            held: BTreeMap::new(),
            marker: core::marker::PhantomData,
        }
    }

    /// Return the complete observable gate state.
    #[must_use]
    pub fn state(&self) -> OrderGateState<K> {
        OrderGateState {
            watermark: self.watermark.clone(),
            held: self.held.len(),
        }
    }

    fn sends(
        deliveries: Vec<Delivery<Target>>,
        reply_to: Route,
        outcome: OrderGateOutcome<K, T>,
    ) -> Actions<A, Never, DeliveryOutcomes<Target, Route::Sends>, NoBirths> {
        Actions::send(DeliveryOutcomes {
            deliveries,
            outcomes: reply_to.deliver(outcome),
        })
    }

    fn hold(
        &mut self,
        key: K,
        value: T,
        to: Recipient<Target>,
        reply_to: Route,
    ) -> Actions<A, Never, DeliveryOutcomes<Target, Route::Sends>, NoBirths> {
        if self
            .watermark
            .as_ref()
            .is_some_and(|watermark| key <= *watermark)
        {
            return Self::sends(
                vec![Delivery::new(to, value)],
                reply_to,
                OrderGateOutcome::Delivered { key },
            );
        }
        if self.held.contains_key(&key) {
            return Self::sends(
                Vec::new(),
                reply_to,
                OrderGateOutcome::Duplicate { key, value },
            );
        }
        self.held.insert(key.clone(), Held { value, to });
        Self::sends(
            Vec::new(),
            reply_to,
            OrderGateOutcome::Held {
                key,
                held: self.held.len(),
            },
        )
    }

    fn open(
        &mut self,
        through: K,
        reply_to: Route,
    ) -> Actions<A, Never, DeliveryOutcomes<Target, Route::Sends>, NoBirths> {
        if let Some(current) = &self.watermark
            && through <= *current
        {
            return Self::sends(
                Vec::new(),
                reply_to,
                OrderGateOutcome::StaleOpening {
                    requested: through,
                    current: current.clone(),
                },
            );
        }
        let keys = self
            .held
            .range(..=through.clone())
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        let mut deliveries = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(held) = self.held.remove(&key) {
                deliveries.push(Delivery::new(held.to, held.value));
            }
        }
        let released = deliveries.len();
        self.watermark = Some(through.clone());
        Self::sends(
            deliveries,
            reply_to,
            OrderGateOutcome::Opened {
                through,
                released,
                held: self.held.len(),
            },
        )
    }
}

impl<A, K, T, Target, Reply, Route> Default for OrderGate<A, K, T, Target, Reply, Route>
where
    A: Address,
    K: Clone + Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = OrderGateOutcome<K, T>>,
    Route: DeliveryRoute<Reply>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, K, T, Target, Reply, Route> BehaviorBase for OrderGate<A, K, T, Target, Reply, Route>
where
    A: Address,
    K: Clone + Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = OrderGateOutcome<K, T>>,
    Route: DeliveryRoute<Reply>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, T, Target, Reply, Route> behavior::Protocol for OrderGate<A, K, T, Target, Reply, Route>
where
    A: Address,
    K: Clone + Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = OrderGateOutcome<K, T>>,
    Route: DeliveryRoute<Reply>,
{
    type Addr = A;
    type Msg = OrderGateMessage<K, T, Target, Route>;
}

impl<A, K, T, Target, Reply, Route> Behavior for OrderGate<A, K, T, Target, Reply, Route>
where
    A: Address,
    K: Clone + Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = OrderGateOutcome<K, T>>,
    Route: DeliveryRoute<Reply>,
    Route::Sends: behavior::SendsFor<User<A, OrderGateMessage<K, T, Target, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = DeliveryOutcomes<Target, Route::Sends>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event.message {
            OrderGateMessage::Hold {
                key,
                value,
                to,
                reply_to,
            } => self.hold(key, value, to, reply_to),
            OrderGateMessage::OpenThrough { through, reply_to } => self.open(through, reply_to),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;
    struct Target;
    struct Reply;
    macro_rules! leaf {
        ($n:ident,$m:ty) => {
            impl behavior::Protocol for $n {
                type Addr = MailAddr;
                type Msg = $m;
            }

            impl Behavior for $n {
                type Protocol = Self;
                type Event = User<MailAddr, $m>;
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
    leaf!(Reply,OrderGateOutcome<u8,u8>);
    type Subject = OrderGate<MailAddr, u8, u8, Target, Reply, Recipient<Reply>>;
    fn hold(
        s: &mut crate::Active<Subject>,
        key: u8,
        value: u8,
    ) -> Actions<MailAddr, Never, DeliveryOutcomes<Target, Vec<Delivery<Reply>>>, NoBirths> {
        s.receive(
            MailAddr(9),
            OrderGateMessage::Hold {
                key,
                value,
                to: Recipient::global(MailAddr(1)),
                reply_to: Recipient::global(MailAddr(2)),
            },
        )
        .unwrap()
    }
    fn open(
        s: &mut crate::Active<Subject>,
        through: u8,
    ) -> Actions<MailAddr, Never, DeliveryOutcomes<Target, Vec<Delivery<Reply>>>, NoBirths> {
        s.receive(
            MailAddr(9),
            OrderGateMessage::OpenThrough {
                through,
                reply_to: Recipient::global(MailAddr(2)),
            },
        )
        .unwrap()
    }
    #[test]
    fn opening_releases_in_key_order_and_future_open_keys_deliver_immediately() {
        let mut s = (Subject::new()).initialize().unwrap().behavior;
        assert!(matches!(
            hold(&mut s, 3, 30).sends.outcomes[0].message,
            OrderGateOutcome::Held { key: 3, held: 1 }
        ));
        assert!(matches!(
            hold(&mut s, 1, 10).sends.outcomes[0].message,
            OrderGateOutcome::Held { key: 1, held: 2 }
        ));
        let a = open(&mut s, 2);
        assert_eq!(
            a.sends
                .deliveries
                .iter()
                .map(|d| d.message)
                .collect::<Vec<_>>(),
            vec![10]
        );
        assert_eq!(hold(&mut s, 2, 20).sends.deliveries.len(), 1);
        assert_eq!(s.state().watermark, Some(2));
        assert_eq!(s.state().held(), 1);
    }
    #[test]
    fn duplicate_and_stale_opening_are_atomic() {
        let mut s = (Subject::new()).initialize().unwrap().behavior;
        assert!(matches!(
            hold(&mut s, 2, 20).sends.outcomes[0].message,
            OrderGateOutcome::Held { key: 2, held: 1 }
        ));
        assert!(matches!(
            hold(&mut s, 2, 21).sends.outcomes[0].message,
            OrderGateOutcome::Duplicate { value: 21, .. }
        ));
        assert!(matches!(
            open(&mut s, 1).sends.outcomes[0].message,
            OrderGateOutcome::Opened {
                through: 1,
                released: 0,
                held: 1
            }
        ));
        assert!(matches!(
            open(&mut s, 1).sends.outcomes[0].message,
            OrderGateOutcome::StaleOpening { .. }
        ));
        assert_eq!(s.state().held, 1);
    }
}
