//! Explicit token admission over typed refill observations.

use core::num::NonZeroU64;

use super::DeliveryOutcomes;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Protocol,
    Recipient, User,
};
use thiserror::Error;

use crate::DeliveryRoute;

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
        /// Exact requested token cost.
        cost: TokenCount,
        /// Unaccepted value.
        value: T,
        /// Rejection reason.
        reason: RateLimitRejection,
    },
}

/// Inputs accepted by [`RateLimiter`].
pub enum RateLimiterMessage<T, Target: Protocol, Route> {
    /// Attempt to consume tokens and deliver one value.
    Acquire {
        /// Required positive token cost.
        cost: TokenCount,
        /// Owned value.
        value: T,
        /// Typed destination.
        to: Recipient<Target>,
        /// Typed outcome recipient.
        reply_to: Route,
    },
    /// Typed clock-derived refill observation.
    Refill {
        /// Positive quantity to add, saturating at capacity.
        tokens: TokenCount,
    },
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
type RateLimiterMarker<A, Target, Reply, Route> =
    core::marker::PhantomData<fn() -> (A, Target, Reply, Route)>;

pub struct RateLimiter<
    A: Address,
    T,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = RateLimiterOutcome<T>>,
    Route: DeliveryRoute<Reply>,
> {
    capacity: TokenCount,
    available: u64,
    marker: RateLimiterMarker<A, Target, Reply, Route>,
}
type RateActions<A, Target, OutcomeSends> =
    Actions<A, Never, DeliveryOutcomes<Target, OutcomeSends>, NoBirths>;
impl<A, T, Target, Reply, Route> RateLimiter<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = RateLimiterOutcome<T>>,
    Route: DeliveryRoute<Reply>,
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
        reply_to: Route,
        outcome: RateLimiterOutcome<T>,
    ) -> RateActions<A, Target, Route::Sends> {
        Actions::send(DeliveryOutcomes {
            deliveries,
            outcomes: reply_to.deliver(outcome),
        })
    }
    fn acquire(
        &mut self,
        cost: TokenCount,
        value: T,
        to: Recipient<Target>,
        reply_to: Route,
    ) -> RateActions<A, Target, Route::Sends> {
        let required = cost.get();
        if required > self.capacity.get() {
            return Self::result(
                Vec::new(),
                reply_to,
                RateLimiterOutcome::Rejected {
                    cost,
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
                    cost,
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
impl<A, T, Target, Reply, Route> BehaviorBase for RateLimiter<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = RateLimiterOutcome<T>>,
    Route: DeliveryRoute<Reply>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, T, Target, Reply, Route> behavior::Protocol for RateLimiter<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = RateLimiterOutcome<T>>,
    Route: DeliveryRoute<Reply>,
{
    type Addr = A;
    type Msg = RateLimiterMessage<T, Target, Route>;
}

impl<A, T, Target, Reply, Route> Behavior for RateLimiter<A, T, Target, Reply, Route>
where
    A: Address,
    Target: Protocol<Addr = A, Msg = T>,
    Reply: behavior::Protocol<Addr = A, Msg = RateLimiterOutcome<T>>,
    Route: DeliveryRoute<Reply>,
    Route::Sends: behavior::SendsFor<User<A, RateLimiterMessage<T, Target, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = DeliveryOutcomes<Target, Route::Sends>;
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
        type Msg = RateLimiterOutcome<u8>;
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
    type Subject = RateLimiter<MailAddr, u8, Target, Reply, Recipient<Reply>>;
    fn tokens(n: u64) -> TokenCount {
        TokenCount::new(NonZeroU64::new(n).unwrap())
    }
    fn acquire(
        s: &mut crate::Active<Subject>,
        cost: u64,
        value: u8,
    ) -> RateActions<MailAddr, Target, Vec<Delivery<Reply>>> {
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
        let mut s = (Subject::new(tokens(5), 3).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        assert_eq!(acquire(&mut s, 2, 7).sends.deliveries[0].message, 7);
        assert!(matches!(
            acquire(&mut s, 2, 8).sends.outcomes[0].message,
            RateLimiterOutcome::Rejected {
                cost,
                value: 8,
                reason: RateLimitRejection::InsufficientTokens
            } if cost == tokens(2)
        ));
        assert!(matches!(
            acquire(&mut s, 6, 9).sends.outcomes[0].message,
            RateLimiterOutcome::Rejected {
                cost,
                value: 9,
                reason: RateLimitRejection::ExceedsCapacity
            } if cost == tokens(6)
        ));
        assert_eq!(s.state().available(), 1);
    }
    #[test]
    fn refill_saturates_without_overflow() {
        let mut s = (Subject::new(tokens(5), 0).unwrap())
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
