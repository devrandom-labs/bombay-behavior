//! Keyed request/result lifecycle correlation.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// Result delivered to the reply recipient retained by [`Correlator`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrelationResult<K, V> {
    /// A matching pending key accepted one value.
    Resolved { key: K, value: V },
    /// A matching pending key was explicitly cancelled.
    Cancelled { key: K },
}

/// Complete retained lifecycle of one correlation key.
pub enum CorrelationState<K, Reply: behavior::Protocol> {
    /// The key owns one reply recipient and may accept resolution or cancel.
    Pending {
        /// Correlation key.
        key: K,
        /// Typed recipient for the terminal result.
        reply_to: Recipient<Reply>,
    },
    /// The key resolved and later replies are stale.
    Completed { key: K },
    /// The key was cancelled and later replies are stale.
    Cancelled { key: K },
}

impl<K, Reply: behavior::Protocol> CorrelationState<K, Reply> {
    fn key(&self) -> &K {
        match self {
            Self::Pending { key, .. } | Self::Completed { key } | Self::Cancelled { key } => key,
        }
    }
}

/// Commands accepted by [`Correlator`].
pub enum CorrelatorMessage<K, V, Reply: behavior::Protocol> {
    /// Establish a unique pending key and its terminal reply recipient.
    Begin { key: K, reply_to: Recipient<Reply> },
    /// Resolve one pending key with an owned value.
    Resolve { key: K, value: V },
    /// Cancel one pending key.
    Cancel { key: K },
}

/// A rejected correlation transition.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CorrelatorError<K, V> {
    /// The key is already pending.
    #[error("correlation key is already pending")]
    AlreadyPending(K),
    /// A completed key cannot be opened again without an explicit retention policy.
    #[error("correlation key is already completed")]
    AlreadyCompleted(K),
    /// A cancelled key cannot be opened again without an explicit retention policy.
    #[error("correlation key is already cancelled")]
    AlreadyCancelled(K),
    /// No lifecycle fact exists for the supplied key.
    #[error("correlation key is unknown")]
    Unknown(K),
    /// No lifecycle fact exists for a reply; the owned value is returned.
    #[error("correlation reply key is unknown")]
    UnknownReply { key: K, value: V },
    /// A reply arrived after successful completion; the owned value is returned.
    #[error("correlation reply is stale because the key completed")]
    StaleCompleted { key: K, value: V },
    /// A reply arrived after cancellation; the owned value is returned.
    #[error("correlation reply is stale because the key was cancelled")]
    StaleCancelled { key: K, value: V },
}

/// Deterministic keyed request/result lifecycle behavior.
///
/// Each key is exactly pending, completed, or cancelled. `Begin` owns the
/// reply recipient. `Resolve` atomically marks a pending key completed and
/// emits one typed result; `Cancel` marks it cancelled and emits one typed
/// cancellation. Terminal keys are retained, making duplicate and stale input
/// explicit rather than indistinguishable from never-seen input. Reopening and
/// retention expiry require a separate explicit policy and are not inferred
/// from time or key reuse. Initialization is empty and the behavior does not
/// terminate itself. These lifecycle and retention choices are Bombay policy;
/// delivery remains an Address/Communication capability. No method has a
/// semantic panic condition.
pub struct Correlator<
    A: Address,
    K,
    V,
    Reply: behavior::Protocol<Addr = A, Msg = CorrelationResult<K, V>>,
> {
    states: Vec<CorrelationState<K, Reply>>,
    address: core::marker::PhantomData<A>,
}

impl<A: Address, K, V, Reply: behavior::Protocol<Addr = A, Msg = CorrelationResult<K, V>>>
    Correlator<A, K, V, Reply>
{
    /// Construct an empty correlator definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            states: Vec::new(),
            address: core::marker::PhantomData,
        }
    }

    /// Borrow every retained lifecycle in first-begin order.
    #[must_use]
    pub fn states(&self) -> &[CorrelationState<K, Reply>] {
        &self.states
    }
}

impl<A: Address, K, V, Reply: behavior::Protocol<Addr = A, Msg = CorrelationResult<K, V>>> Default
    for Correlator<A, K, V, Reply>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address, K, V, Reply: behavior::Protocol<Addr = A, Msg = CorrelationResult<K, V>>>
    BehaviorBase for Correlator<A, K, V, Reply>
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, V, Reply> behavior::Protocol for Correlator<A, K, V, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = CorrelationResult<K, V>>,
{
    type Addr = A;
    type Msg = CorrelatorMessage<K, V, Reply>;
}

impl<A, K, V, Reply> Behavior for Correlator<A, K, V, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = CorrelationResult<K, V>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = CorrelatorError<K, V>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            CorrelatorMessage::Begin { key, reply_to } => {
                if let Some(existing) = self.states.iter().find(|state| state.key() == &key) {
                    return Err(match existing {
                        CorrelationState::Pending { .. } => CorrelatorError::AlreadyPending(key),
                        CorrelationState::Completed { .. } => {
                            CorrelatorError::AlreadyCompleted(key)
                        }
                        CorrelationState::Cancelled { .. } => {
                            CorrelatorError::AlreadyCancelled(key)
                        }
                    });
                }
                self.states
                    .push(CorrelationState::Pending { key, reply_to });
                Ok(Actions::cont())
            }
            CorrelatorMessage::Resolve { key, value } => {
                let Some(index) = self.states.iter().position(|state| state.key() == &key) else {
                    return Err(CorrelatorError::UnknownReply { key, value });
                };
                let reply_to = match &self.states[index] {
                    CorrelationState::Pending { reply_to, .. } => *reply_to,
                    CorrelationState::Completed { .. } => {
                        return Err(CorrelatorError::StaleCompleted { key, value });
                    }
                    CorrelationState::Cancelled { .. } => {
                        return Err(CorrelatorError::StaleCancelled { key, value });
                    }
                };
                self.states[index] = CorrelationState::Completed { key: key.clone() };
                Ok(Actions::send(vec![Delivery::new(
                    reply_to,
                    CorrelationResult::Resolved { key, value },
                )]))
            }
            CorrelatorMessage::Cancel { key } => {
                let Some(index) = self.states.iter().position(|state| state.key() == &key) else {
                    return Err(CorrelatorError::Unknown(key));
                };
                let reply_to = match &self.states[index] {
                    CorrelationState::Pending { reply_to, .. } => *reply_to,
                    CorrelationState::Completed { .. } => {
                        return Err(CorrelatorError::AlreadyCompleted(key));
                    }
                    CorrelationState::Cancelled { .. } => {
                        return Err(CorrelatorError::AlreadyCancelled(key));
                    }
                };
                self.states[index] = CorrelationState::Cancelled { key: key.clone() };
                Ok(Actions::send(vec![Delivery::new(
                    reply_to,
                    CorrelationResult::Cancelled { key },
                )]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    struct Reply;

    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = CorrelationResult<u8, u16>;
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

    type TestCorrelator = Correlator<MailAddr, u8, u16, Reply>;

    #[test]
    fn resolve_commits_before_one_terminal_delivery_and_marks_duplicates_stale() {
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let mut correlator = (TestCorrelator::new()).initialize().unwrap().behavior;
        correlator
            .receive(
                MailAddr(9),
                CorrelatorMessage::Begin {
                    key: 1,
                    reply_to: reply,
                },
            )
            .unwrap();
        let resolved = correlator
            .receive(
                MailAddr(9),
                CorrelatorMessage::Resolve { key: 1, value: 42 },
            )
            .unwrap();
        assert!(
            resolved.sends
                == vec![Delivery::new(
                    reply,
                    CorrelationResult::Resolved { key: 1, value: 42 },
                )]
        );
        assert!(resolved.creates.is_empty());
        assert!(matches!(
            correlator.states(),
            [CorrelationState::Completed { key: 1 }]
        ));
        assert!(matches!(
            correlator.receive(
                MailAddr(9),
                CorrelatorMessage::Resolve { key: 1, value: 43 },
            ),
            Err(CorrelatorError::StaleCompleted { key: 1, value: 43 })
        ));
    }

    #[test]
    fn cancel_is_terminal_and_unknown_reply_preserves_the_value() {
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let mut correlator = (TestCorrelator::new()).initialize().unwrap().behavior;
        assert!(matches!(
            correlator.receive(
                MailAddr(9),
                CorrelatorMessage::Resolve { key: 7, value: 99 },
            ),
            Err(CorrelatorError::UnknownReply { key: 7, value: 99 })
        ));
        correlator
            .receive(
                MailAddr(9),
                CorrelatorMessage::Begin {
                    key: 2,
                    reply_to: reply,
                },
            )
            .unwrap();
        let cancelled = correlator
            .receive(MailAddr(9), CorrelatorMessage::Cancel { key: 2 })
            .unwrap();
        assert!(
            cancelled.sends
                == vec![Delivery::new(
                    reply,
                    CorrelationResult::Cancelled { key: 2 },
                )]
        );
        assert!(matches!(
            correlator.receive(
                MailAddr(9),
                CorrelatorMessage::Resolve { key: 2, value: 100 },
            ),
            Err(CorrelatorError::StaleCancelled { key: 2, value: 100 })
        ));
    }
}
