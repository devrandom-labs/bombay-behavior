//! Bounded stable priority release policy.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Protocol,
    Recipient, SendEffects, User,
};
use thiserror::Error;

/// Complete admission phase of a [`PriorityQueue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityQueueState {
    /// Further offers can receive a fresh tie-order token.
    Active {
        /// Next insertion token.
        next: u64,
        /// Current queue depth.
        queued: usize,
    },
    /// The insertion-token domain is exhausted; retained values remain releasable.
    Exhausted {
        /// Current queue depth.
        queued: usize,
    },
}

/// Why an offer was not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriorityQueueRejection {
    /// Positive capacity is full.
    Full,
    /// No fresh stable tie-order token is representable.
    SequenceExhausted,
}

/// Factual result of one priority-queue operation.
#[derive(Debug, PartialEq, Eq)]
pub enum PriorityQueueOutcome<T> {
    /// Value was accepted.
    Accepted {
        /// Queue depth after admission.
        depth: usize,
    },
    /// Value was not accepted; ownership is returned.
    Rejected {
        /// Unaccepted value.
        value: T,
        /// Exhaustive reason.
        reason: PriorityQueueRejection,
    },
    /// Highest-priority oldest-tie value was released.
    Released {
        /// Queue depth after release.
        remaining: usize,
    },
    /// No retained value was available.
    Empty,
}

/// Operations accepted by [`PriorityQueue`].
pub enum PriorityQueueMessage<T, P, Target: Protocol, Reply: behavior::Protocol> {
    /// Offer one owned value at an immutable priority.
    Offer {
        /// Owned value.
        value: T,
        /// Comparable priority; greater values release first.
        priority: P,
        /// Typed admission-result recipient.
        reply_to: Recipient<Reply>,
    },
    /// Release one value to a typed destination.
    Release {
        /// Destination.
        to: Recipient<Target>,
        /// Typed release-result recipient.
        reply_to: Recipient<Reply>,
    },
}

/// Named effect lanes emitted by [`PriorityQueue`].
pub struct PriorityQueueSends<Target: Protocol, Reply: behavior::Protocol> {
    /// Released values.
    pub deliveries: Vec<Delivery<Target>>,
    /// Operation results.
    pub outcomes: Vec<Delivery<Reply>>,
}
impl<Target: Protocol, Reply: behavior::Protocol> SendEffects
    for PriorityQueueSends<Target, Reply>
{
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

impl<Event, Target: Protocol, Reply: behavior::Protocol> behavior::SendsFor<Event>
    for PriorityQueueSends<Target, Reply>
{
}

impl<I, RootEvent, Path, Target, Reply> behavior::InterpretSends<I, RootEvent, Path>
    for PriorityQueueSends<Target, Reply>
where
    I: behavior::SendInterpreter,
    Target: Protocol,
    Reply: behavior::Protocol,
    Vec<Delivery<Target>>: behavior::InterpretSends<I, RootEvent, Path>,
    Vec<Delivery<Reply>>: behavior::InterpretSends<I, RootEvent, Path>,
{
    fn interpret(self, interpreter: &mut I) -> Result<(), I::Error> {
        behavior::InterpretSends::interpret(self.deliveries, interpreter)?;
        behavior::InterpretSends::interpret(self.outcomes, interpreter)
    }
}

/// Invalid priority-queue definition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PriorityQueueConfigError {
    /// A zero-capacity definition can never accept ownership.
    #[error("priority queue capacity must be positive")]
    ZeroCapacity,
}

struct Entry<T, P> {
    value: T,
    priority: P,
    order: u64,
}
impl<T, P: Ord> PartialEq for Entry<T, P> {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.order == other.order
    }
}
impl<T, P: Ord> Eq for Entry<T, P> {}
impl<T, P: Ord> PartialOrd for Entry<T, P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<T, P: Ord> Ord for Entry<T, P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.order.cmp(&self.order))
    }
}

/// Bounded stable immutable-priority queue behavior.
///
/// Greater priorities release first and equal priorities preserve FIFO by an
/// explicit insertion token. Full and token-exhausted offers return ownership.
/// Token exhaustion is a distinct admission phase and never wraps; retained
/// values remain releasable. Release emits one value and one factual outcome;
/// empty release emits only `Empty`. Initialization is empty, no actors are
/// created, and the host never terminates by policy. Capacity, priority order,
/// and FIFO ties are Bombay policy; mailbox priority and backpressure remain
/// Communication concerns. The implementation uses `BinaryHeap` because
/// accepted priorities are immutable; it does not need `priority-queue`.
/// No transition has a semantic panic condition.
type PriorityQueueMarker<A, Target, Reply> = core::marker::PhantomData<fn() -> (A, Target, Reply)>;

pub struct PriorityQueue<
    A: Address,
    T,
    P: Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = PriorityQueueOutcome<T>>,
> {
    capacity: usize,
    next: Option<u64>,
    queued: BinaryHeap<Entry<T, P>>,
    marker: PriorityQueueMarker<A, Target, Reply>,
}
type PriorityActions<A, Target, Reply> =
    Actions<A, Never, PriorityQueueSends<Target, Reply>, NoBirths>;
impl<A, T, P, Target, Reply> PriorityQueue<A, T, P, Target, Reply>
where
    A: Address,
    P: Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = PriorityQueueOutcome<T>>,
{
    /// Construct an empty positive-capacity queue.
    /// # Errors
    /// Returns [`PriorityQueueConfigError::ZeroCapacity`] for zero capacity.
    pub fn new(capacity: usize) -> Result<Self, PriorityQueueConfigError> {
        if capacity == 0 {
            return Err(PriorityQueueConfigError::ZeroCapacity);
        }
        Ok(Self {
            capacity,
            next: Some(0),
            queued: BinaryHeap::with_capacity(capacity),
            marker: core::marker::PhantomData,
        })
    }
    /// Return the complete admission phase and depth.
    #[must_use]
    pub fn state(&self) -> PriorityQueueState {
        self.next.map_or(
            PriorityQueueState::Exhausted {
                queued: self.queued.len(),
            },
            |next| PriorityQueueState::Active {
                next,
                queued: self.queued.len(),
            },
        )
    }
    fn sends(
        deliveries: Vec<Delivery<Target>>,
        outcomes: Vec<Delivery<Reply>>,
    ) -> PriorityActions<A, Target, Reply> {
        Actions::send(PriorityQueueSends {
            deliveries,
            outcomes,
        })
    }
    fn offer(
        &mut self,
        value: T,
        priority: P,
        reply_to: Recipient<Reply>,
    ) -> PriorityActions<A, Target, Reply> {
        if self.queued.len() == self.capacity {
            return Self::sends(
                Vec::new(),
                vec![Delivery::new(
                    reply_to,
                    PriorityQueueOutcome::Rejected {
                        value,
                        reason: PriorityQueueRejection::Full,
                    },
                )],
            );
        }
        let Some(order) = self.next else {
            return Self::sends(
                Vec::new(),
                vec![Delivery::new(
                    reply_to,
                    PriorityQueueOutcome::Rejected {
                        value,
                        reason: PriorityQueueRejection::SequenceExhausted,
                    },
                )],
            );
        };
        self.next = order.checked_add(1);
        self.queued.push(Entry {
            value,
            priority,
            order,
        });
        Self::sends(
            Vec::new(),
            vec![Delivery::new(
                reply_to,
                PriorityQueueOutcome::Accepted {
                    depth: self.queued.len(),
                },
            )],
        )
    }
}
impl<A, T, P, Target, Reply> BehaviorBase for PriorityQueue<A, T, P, Target, Reply>
where
    A: Address,
    P: Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = PriorityQueueOutcome<T>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, T, P, Target, Reply> behavior::Protocol for PriorityQueue<A, T, P, Target, Reply>
where
    A: Address,
    P: Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = PriorityQueueOutcome<T>>,
{
    type Addr = A;
    type Msg = PriorityQueueMessage<T, P, Target, Reply>;
}

impl<A, T, P, Target, Reply> Behavior for PriorityQueue<A, T, P, Target, Reply>
where
    A: Address,
    P: Ord,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = PriorityQueueOutcome<T>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = PriorityQueueSends<Target, Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event.message {
            PriorityQueueMessage::Offer {
                value,
                priority,
                reply_to,
            } => self.offer(value, priority, reply_to),
            PriorityQueueMessage::Release { to, reply_to } => self.queued.pop().map_or_else(
                || {
                    Self::sends(
                        Vec::new(),
                        vec![Delivery::new(reply_to, PriorityQueueOutcome::Empty)],
                    )
                },
                |entry| {
                    Self::sends(
                        vec![Delivery::new(to, entry.value)],
                        vec![Delivery::new(
                            reply_to,
                            PriorityQueueOutcome::Released {
                                remaining: self.queued.len(),
                            },
                        )],
                    )
                },
            ),
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
    impl behavior::Protocol for Target {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Target {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = PriorityQueueOutcome<u8>;
    }

    impl Behavior for Reply {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = PriorityQueue<MailAddr, u8, u8, Target, Reply>;
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(2))
    }
    fn offer(s: &mut crate::Active<Subject>, value: u8, priority: u8) {
        s.receive(
            MailAddr(0),
            PriorityQueueMessage::Offer {
                value,
                priority,
                reply_to: reply(),
            },
        )
        .unwrap();
    }
    fn release(s: &mut crate::Active<Subject>) -> PriorityActions<MailAddr, Target, Reply> {
        s.receive(
            MailAddr(0),
            PriorityQueueMessage::Release {
                to: Recipient::global(MailAddr(1)),
                reply_to: reply(),
            },
        )
        .unwrap()
    }
    #[test]
    fn greater_priority_and_fifo_ties_are_stable() {
        let mut s = (Subject::new(4).unwrap()).initialize().unwrap().behavior;
        for pair in [(1, 2), (2, 3), (3, 3), (4, 1)] {
            offer(&mut s, pair.0, pair.1);
        }
        assert_eq!(
            [
                release(&mut s).sends.deliveries[0].message,
                release(&mut s).sends.deliveries[0].message,
                release(&mut s).sends.deliveries[0].message,
                release(&mut s).sends.deliveries[0].message
            ],
            [2, 3, 1, 4]
        );
    }
    #[test]
    fn full_and_empty_are_explicit() {
        let mut s = (Subject::new(1).unwrap()).initialize().unwrap().behavior;
        offer(&mut s, 1, 0);
        let rejected = s
            .receive(
                MailAddr(0),
                PriorityQueueMessage::Offer {
                    value: 2,
                    priority: 9,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            PriorityQueueOutcome::Rejected {
                value: 2,
                reason: PriorityQueueRejection::Full
            }
        ));
        let _ = release(&mut s);
        assert!(matches!(
            release(&mut s).sends.outcomes[0].message,
            PriorityQueueOutcome::Empty
        ));
    }

    #[test]
    fn insertion_sequence_exhaustion_never_wraps_or_consumes_the_next_value() {
        let mut definition = Subject::new(2).unwrap();
        definition.next = Some(u64::MAX);
        let mut subject = (definition).initialize().unwrap().behavior;
        offer(&mut subject, 1, 0);
        assert_eq!(subject.state(), PriorityQueueState::Exhausted { queued: 1 });
        let rejected = subject
            .receive(
                MailAddr(0),
                PriorityQueueMessage::Offer {
                    value: 2,
                    priority: 9,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            PriorityQueueOutcome::Rejected {
                value: 2,
                reason: PriorityQueueRejection::SequenceExhausted
            }
        ));
    }
}
