//! Explicit monotonic release gate for ordered keys.

use std::collections::BTreeMap;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    SendAlgebra, User,
};

/// Complete observable state of an [`OrderGate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderGateState<K> {
    /// Greatest explicitly opened key, or absence before the first opening.
    pub watermark: Option<K>,
    /// Number of values retained above the watermark.
    pub held: usize,
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
pub enum OrderGateMessage<K, T, Target: Behavior, Reply: Behavior> {
    /// Submit one keyed value.
    Hold {
        /// Ordered release key.
        key: K,
        /// Owned value.
        value: T,
        /// Typed delivery destination.
        to: Recipient<Target>,
        /// Typed outcome recipient.
        reply_to: Recipient<Reply>,
    },
    /// Monotonically open every key through the supplied bound.
    OpenThrough {
        /// Inclusive new watermark.
        through: K,
        /// Typed outcome recipient.
        reply_to: Recipient<Reply>,
    },
}

/// Named effect lanes emitted by [`OrderGate`].
pub struct OrderGateSends<Target: Behavior, Reply: Behavior> {
    /// Values emitted in increasing key order for an opening.
    pub deliveries: Vec<Delivery<Target>>,
    /// Exactly one factual operation result.
    pub outcomes: Vec<Delivery<Reply>>,
}

impl<Target: Behavior, Reply: Behavior> SendAlgebra for OrderGateSends<Target, Reply> {
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

struct Held<T, Target: Behavior> {
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
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = OrderGateOutcome<K, T>>,
> {
    watermark: Option<K>,
    held: BTreeMap<K, Held<T, Target>>,
    marker: core::marker::PhantomData<fn() -> (A, Reply)>,
}

impl<A, K, T, Target, Reply> OrderGate<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Ord,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = OrderGateOutcome<K, T>>,
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
        reply_to: Recipient<Reply>,
        outcome: OrderGateOutcome<K, T>,
    ) -> Actions<A, Never, OrderGateSends<Target, Reply>, NoBirths> {
        Actions::send(OrderGateSends {
            deliveries,
            outcomes: vec![Delivery::new(reply_to, outcome)],
        })
    }

    fn hold(
        &mut self,
        key: K,
        value: T,
        to: Recipient<Target>,
        reply_to: Recipient<Reply>,
    ) -> Actions<A, Never, OrderGateSends<Target, Reply>, NoBirths> {
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
        reply_to: Recipient<Reply>,
    ) -> Actions<A, Never, OrderGateSends<Target, Reply>, NoBirths> {
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

impl<A, K, T, Target, Reply> Default for OrderGate<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Ord,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = OrderGateOutcome<K, T>>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, K, T, Target, Reply> BehaviorBase for OrderGate<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Ord,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = OrderGateOutcome<K, T>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, T, Target, Reply> Behavior for OrderGate<A, K, T, Target, Reply>
where
    A: Address,
    K: Clone + Ord,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = OrderGateOutcome<K, T>>,
{
    type Addr = A;
    type Msg = OrderGateMessage<K, T, Target, Reply>;
    type Event = User<A, Self::Msg>;
    type Sends = OrderGateSends<Target, Reply>;
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
    use behavior::MailAddr;
    struct Target;
    struct Reply;
    macro_rules! leaf {
        ($n:ident,$m:ty) => {
            impl Behavior for $n {
                type Addr = MailAddr;
                type Msg = $m;
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
    type Subject = OrderGate<MailAddr, u8, u8, Target, Reply>;
    fn hold(
        s: &mut crate::Active<Subject>,
        key: u8,
        value: u8,
    ) -> Actions<MailAddr, Never, OrderGateSends<Target, Reply>, NoBirths> {
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
    ) -> Actions<MailAddr, Never, OrderGateSends<Target, Reply>, NoBirths> {
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
        let mut s = crate::Compose::new(Subject::new())
            .initialize()
            .unwrap()
            .behavior;
        let _ = hold(&mut s, 3, 30);
        let _ = hold(&mut s, 1, 10);
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
        assert_eq!(
            s.state(),
            OrderGateState {
                watermark: Some(2),
                held: 1
            }
        );
    }
    #[test]
    fn duplicate_and_stale_opening_are_atomic() {
        let mut s = crate::Compose::new(Subject::new())
            .initialize()
            .unwrap()
            .behavior;
        let _ = hold(&mut s, 2, 20);
        assert!(matches!(
            hold(&mut s, 2, 21).sends.outcomes[0].message,
            OrderGateOutcome::Duplicate { value: 21, .. }
        ));
        let _ = open(&mut s, 1);
        assert!(matches!(
            open(&mut s, 1).sends.outcomes[0].message,
            OrderGateOutcome::StaleOpening { .. }
        ));
        assert_eq!(s.state().held, 1);
    }
}
