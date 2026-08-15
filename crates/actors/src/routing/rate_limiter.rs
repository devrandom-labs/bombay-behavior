//! Explicit token admission over typed refill observations.

use core::num::NonZeroU64;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    SendAlgebra, User,
};
use thiserror::Error;

/// Semantic token quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenCount(pub NonZeroU64);
impl TokenCount {
    /// Construct a non-zero quantity.
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
    /// Return the numeric quantity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Complete observable [`RateLimiter`] state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimiterState {
    /// Positive bucket capacity.
    pub capacity: TokenCount,
    available: u64,
}

impl RateLimiterState {
    /// Currently available tokens, always at most `capacity`.
    #[must_use]
    pub fn available(&self) -> u64 {
        self.available
    }
}

/// Exhaustive admission rejection reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RateLimitRejection {
    /// Requested cost exceeds total capacity.
    ExceedsCapacity,
    /// Cost is valid but current tokens are insufficient.
    InsufficientTokens,
}

/// Factual result of one admission request.
#[derive(Debug, PartialEq, Eq)]
pub enum RateLimiterOutcome<T> {
    /// Value was admitted and delivered.
    Admitted {
        /// Tokens remaining after atomic admission.
        remaining: u64,
    },
    /// Value was not admitted; ownership is returned.
    Rejected {
        /// Unaccepted value.
        value: T,
        /// Rejection reason.
        reason: RateLimitRejection,
    },
}

/// Inputs accepted by [`RateLimiter`].
pub enum RateLimiterMessage<T, Target: Behavior, Reply: Behavior> {
    /// Attempt to consume tokens and deliver one value.
    Acquire {
        /// Required positive token cost.
        cost: TokenCount,
        /// Owned value.
        value: T,
        /// Typed destination.
        to: Recipient<Target>,
        /// Typed outcome recipient.
        reply_to: Recipient<Reply>,
    },
    /// Typed clock-derived refill observation.
    Refill {
        /// Positive quantity to add, saturating at capacity.
        tokens: TokenCount,
    },
}

/// Named effect lanes emitted by [`RateLimiter`].
pub struct RateLimiterSends<Target: Behavior, Reply: Behavior> {
    /// Admitted values.
    pub deliveries: Vec<Delivery<Target>>,
    /// Admission facts.
    pub outcomes: Vec<Delivery<Reply>>,
}
impl<Target: Behavior, Reply: Behavior> SendAlgebra for RateLimiterSends<Target, Reply> {
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

/// Invalid rate-limiter definition.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RateLimiterConfigError {
    /// Initial token count exceeds capacity.
    #[error("initial rate-limit tokens exceed capacity")]
    InitialExceedsCapacity {
        /// Declared capacity.
        capacity: TokenCount,
        /// Invalid initial count.
        initial: u64,
    },
}

/// Deterministic explicit-token admission behavior.
///
/// Admission atomically subtracts a positive cost and emits the value, or
/// returns ownership with an exhaustive rejection. Typed refill observations
/// saturate at capacity and cannot overflow. The initial count may be zero but
/// cannot exceed positive capacity. Initialization is empty, no actors are
/// created, and the host never terminates by policy. Token arithmetic,
/// saturation, and rejection ordering are Bombay policy. Clock cadence is not
/// observed here: a `Periodic` composition or Environment adapter supplies
/// typed `Refill` events. No transition has a semantic panic condition.
type RateLimiterMarker<A, Target, Reply> = core::marker::PhantomData<fn() -> (A, Target, Reply)>;

pub struct RateLimiter<
    A: Address,
    T,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = RateLimiterOutcome<T>>,
> {
    capacity: TokenCount,
    available: u64,
    marker: RateLimiterMarker<A, Target, Reply>,
}
type RateActions<A, Target, Reply> = Actions<A, Never, RateLimiterSends<Target, Reply>, NoBirths>;
impl<A, T, Target, Reply> RateLimiter<A, T, Target, Reply>
where
    A: Address,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = RateLimiterOutcome<T>>,
{
    /// Construct one explicit positive-capacity bucket.
    /// # Errors
    /// Returns [`RateLimiterConfigError::InitialExceedsCapacity`] when invalid.
    pub fn new(capacity: TokenCount, initial: u64) -> Result<Self, RateLimiterConfigError> {
        if initial > capacity.get() {
            return Err(RateLimiterConfigError::InitialExceedsCapacity { capacity, initial });
        }
        Ok(Self {
            capacity,
            available: initial,
            marker: core::marker::PhantomData,
        })
    }
    /// Return complete current capacity and token state.
    #[must_use]
    pub const fn state(&self) -> RateLimiterState {
        RateLimiterState {
            capacity: self.capacity,
            available: self.available,
        }
    }
    fn result(
        deliveries: Vec<Delivery<Target>>,
        reply_to: Recipient<Reply>,
        outcome: RateLimiterOutcome<T>,
    ) -> RateActions<A, Target, Reply> {
        Actions::send(RateLimiterSends {
            deliveries,
            outcomes: vec![Delivery::new(reply_to, outcome)],
        })
    }
    fn acquire(
        &mut self,
        cost: TokenCount,
        value: T,
        to: Recipient<Target>,
        reply_to: Recipient<Reply>,
    ) -> RateActions<A, Target, Reply> {
        let required = cost.get();
        if required > self.capacity.get() {
            return Self::result(
                Vec::new(),
                reply_to,
                RateLimiterOutcome::Rejected {
                    value,
                    reason: RateLimitRejection::ExceedsCapacity,
                },
            );
        }
        if required > self.available {
            return Self::result(
                Vec::new(),
                reply_to,
                RateLimiterOutcome::Rejected {
                    value,
                    reason: RateLimitRejection::InsufficientTokens,
                },
            );
        }
        self.available -= required;
        Self::result(
            vec![Delivery::new(to, value)],
            reply_to,
            RateLimiterOutcome::Admitted {
                remaining: self.available,
            },
        )
    }
}
impl<A, T, Target, Reply> BehaviorBase for RateLimiter<A, T, Target, Reply>
where
    A: Address,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = RateLimiterOutcome<T>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, T, Target, Reply> Behavior for RateLimiter<A, T, Target, Reply>
where
    A: Address,
    Target: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = RateLimiterOutcome<T>>,
{
    type Addr = A;
    type Msg = RateLimiterMessage<T, Target, Reply>;
    type Event = User<A, Self::Msg>;
    type Sends = RateLimiterSends<Target, Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event.message {
            RateLimiterMessage::Acquire {
                cost,
                value,
                to,
                reply_to,
            } => self.acquire(cost, value, to, reply_to),
            RateLimiterMessage::Refill { tokens } => {
                self.available = self
                    .available
                    .saturating_add(tokens.get())
                    .min(self.capacity.get());
                Actions::cont()
            }
        })
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
        type Msg = RateLimiterOutcome<u8>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = RateLimiter<MailAddr, u8, Target, Reply>;
    fn tokens(n: u64) -> TokenCount {
        TokenCount::new(NonZeroU64::new(n).unwrap())
    }
    fn acquire(
        s: &mut crate::Active<Subject>,
        cost: u64,
        value: u8,
    ) -> RateActions<MailAddr, Target, Reply> {
        s.receive(
            MailAddr(0),
            RateLimiterMessage::Acquire {
                cost: tokens(cost),
                value,
                to: Recipient::global(MailAddr(1)),
                reply_to: Recipient::global(MailAddr(2)),
            },
        )
        .unwrap()
    }
    #[test]
    fn admission_and_rejections_preserve_tokens_and_ownership() {
        let mut s = crate::Compose::new(Subject::new(tokens(5), 3).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        assert_eq!(acquire(&mut s, 2, 7).sends.deliveries[0].message, 7);
        assert!(matches!(
            acquire(&mut s, 2, 8).sends.outcomes[0].message,
            RateLimiterOutcome::Rejected {
                value: 8,
                reason: RateLimitRejection::InsufficientTokens
            }
        ));
        assert!(matches!(
            acquire(&mut s, 6, 9).sends.outcomes[0].message,
            RateLimiterOutcome::Rejected {
                value: 9,
                reason: RateLimitRejection::ExceedsCapacity
            }
        ));
        assert_eq!(s.state().available(), 1);
    }
    #[test]
    fn refill_saturates_without_overflow() {
        let mut s = crate::Compose::new(Subject::new(tokens(5), 0).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        s.receive(
            MailAddr(0),
            RateLimiterMessage::Refill {
                tokens: tokens(u64::MAX),
            },
        )
        .unwrap();
        assert_eq!(s.state().available(), 5);
    }
}
