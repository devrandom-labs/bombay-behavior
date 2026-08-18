//! Typed recipient bindings and lookup replies.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Protocol,
    Recipient, User,
};
use thiserror::Error;

/// The complete lookup result returned by [`Registry`].
pub enum RegistryResult<K, D: Protocol> {
    /// The key was bound to this typed recipient when the lookup was folded.
    Found { key: K, recipient: Recipient<D> },
    /// No binding existed when the lookup was folded.
    Missing { key: K },
}

impl<K: Clone, D: Protocol> Clone for RegistryResult<K, D> {
    fn clone(&self) -> Self {
        match self {
            Self::Found { key, recipient } => Self::Found {
                key: key.clone(),
                recipient: *recipient,
            },
            Self::Missing { key } => Self::Missing { key: key.clone() },
        }
    }
}

impl<K: PartialEq, D: Protocol> PartialEq for RegistryResult<K, D> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Found {
                    key: left_key,
                    recipient: left_recipient,
                },
                Self::Found {
                    key: right_key,
                    recipient: right_recipient,
                },
            ) => left_key == right_key && left_recipient == right_recipient,
            (Self::Missing { key: left }, Self::Missing { key: right }) => left == right,
            (Self::Found { .. }, Self::Missing { .. })
            | (Self::Missing { .. }, Self::Found { .. }) => false,
        }
    }
}

impl<K: Eq, D: Protocol> Eq for RegistryResult<K, D> {}

/// Commands accepted by a [`Registry`].
///
/// A lookup owns a typed reply recipient; no runtime registry or reply-channel
/// discovery is performed.
pub enum RegistryMessage<K, D: Protocol, Reply: behavior::Protocol> {
    /// Establish a previously absent binding.
    Bind { key: K, recipient: Recipient<D> },
    /// Remove a binding only if it still names the supplied recipient.
    Unbind { key: K, recipient: Recipient<D> },
    /// Return the exact current result to `reply_to`.
    Lookup { key: K, reply_to: Recipient<Reply> },
}

/// A rejected registry mutation.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RegistryError<K> {
    /// A different recipient already owns the key.
    #[error("registry key is already bound")]
    AlreadyBound(K),
    /// Unbinding named an absent key.
    #[error("registry key is not bound")]
    NotBound(K),
    /// Unbinding named a recipient other than the current binding.
    #[error("registry unbind is stale")]
    StaleBinding(K),
}

/// Insertion-ordered typed recipient registry.
///
/// State is a sequence of unique key/recipient bindings. Inputs are
/// [`RegistryMessage`] values and lookup outputs are one typed
/// [`Delivery<Reply>`]. Initialization is empty. Duplicate bind, absent
/// unbind, and stale unbind are explicit errors and leave state unchanged.
/// Lookups never fail: absence is a factual [`RegistryResult::Missing`]. The
/// actor does not terminate by policy. Ordering and conflict behavior are
/// deliberate Bombay policy; endpoint generation and delivery are interpreted
/// by Bombay Address and Communication.
pub struct Registry<A: Address, K, D: Protocol<Addr = A>, Reply: behavior::Protocol<Addr = A>> {
    bindings: Vec<(K, Recipient<D>)>,
    address: core::marker::PhantomData<fn() -> (A, Reply)>,
}

impl<A: Address, K, D: Protocol<Addr = A>, Reply: behavior::Protocol<Addr = A>>
    Registry<A, K, D, Reply>
{
    /// Construct an empty registry definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bindings: Vec::new(),
            address: core::marker::PhantomData,
        }
    }

    /// Current bindings in establishment order.
    #[must_use]
    pub fn bindings(&self) -> &[(K, Recipient<D>)] {
        &self.bindings
    }
}

impl<A: Address, K, D: Protocol<Addr = A>, Reply: behavior::Protocol<Addr = A>> Default
    for Registry<A, K, D, Reply>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address, K, D: Protocol<Addr = A>, Reply: behavior::Protocol<Addr = A>> BehaviorBase
    for Registry<A, K, D, Reply>
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, K, D, Reply> behavior::Protocol for Registry<A, K, D, Reply>
where
    A: Address,
    K: Clone + Eq,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = RegistryResult<K, D>>,
{
    type Addr = A;
    type Msg = RegistryMessage<K, D, Reply>;
}

impl<A, K, D, Reply> Behavior for Registry<A, K, D, Reply>
where
    A: Address,
    K: Clone + Eq,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = RegistryResult<K, D>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = RegistryError<K>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            RegistryMessage::Bind { key, recipient } => {
                if self.bindings.iter().any(|(bound, _)| *bound == key) {
                    return Err(RegistryError::AlreadyBound(key));
                }
                self.bindings.push((key, recipient));
                Ok(Actions::cont())
            }
            RegistryMessage::Unbind { key, recipient } => {
                let Some(index) = self.bindings.iter().position(|(bound, _)| *bound == key) else {
                    return Err(RegistryError::NotBound(key));
                };
                if self.bindings[index].1 != recipient {
                    return Err(RegistryError::StaleBinding(key));
                }
                self.bindings.remove(index);
                Ok(Actions::cont())
            }
            RegistryMessage::Lookup { key, reply_to } => {
                let result = self
                    .bindings
                    .iter()
                    .find(|(bound, _)| *bound == key)
                    .map_or_else(
                        || RegistryResult::Missing { key: key.clone() },
                        |(_, recipient)| RegistryResult::Found {
                            key: key.clone(),
                            recipient: *recipient,
                        },
                    );
                Ok(Actions::send(vec![Delivery::new(reply_to, result)]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::{MailAddr, Step};

    #[derive(PartialEq, Eq)]
    struct Destination;
    struct Reply;

    impl behavior::Protocol for Destination {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Destination {
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
        type Msg = RegistryResult<u8, Destination>;
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

    type TestRegistry = Registry<MailAddr, u8, Destination, Reply>;

    #[test]
    fn mutations_are_atomic_and_stale_unbind_is_typed() {
        let one = Recipient::<Destination>::global(MailAddr(1));
        let two = Recipient::<Destination>::global(MailAddr(2));
        let mut registry = (TestRegistry::new()).initialize().unwrap().behavior;

        let bound = registry
            .receive(
                MailAddr(9),
                RegistryMessage::Bind {
                    key: 4,
                    recipient: one,
                },
            )
            .unwrap();
        assert!(bound.sends.is_empty());
        assert!(bound.creates.is_empty());
        assert!(matches!(bound.become_, Step::Continue));

        assert!(matches!(
            registry.receive(
                MailAddr(9),
                RegistryMessage::Bind {
                    key: 4,
                    recipient: two,
                },
            ),
            Err(RegistryError::AlreadyBound(4))
        ));
        assert!(matches!(
            registry.receive(
                MailAddr(9),
                RegistryMessage::Unbind {
                    key: 4,
                    recipient: two,
                },
            ),
            Err(RegistryError::StaleBinding(4))
        ));
        assert!(registry.bindings() == [(4, one)]);

        registry
            .receive(
                MailAddr(9),
                RegistryMessage::Unbind {
                    key: 4,
                    recipient: one,
                },
            )
            .unwrap();
        assert!(registry.bindings().is_empty());
    }

    #[test]
    fn lookups_return_found_and_missing_as_facts() {
        let destination = Recipient::<Destination>::global(MailAddr(1));
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let mut registry = (TestRegistry::new()).initialize().unwrap().behavior;
        registry
            .receive(
                MailAddr(9),
                RegistryMessage::Bind {
                    key: 4,
                    recipient: destination,
                },
            )
            .unwrap();

        let found = registry
            .receive(
                MailAddr(9),
                RegistryMessage::Lookup {
                    key: 4,
                    reply_to: reply,
                },
            )
            .unwrap();
        assert!(
            found.sends
                == vec![Delivery::new(
                    reply,
                    RegistryResult::Found {
                        key: 4,
                        recipient: destination,
                    },
                )]
        );
        assert!(found.creates.is_empty());
        assert!(matches!(found.become_, Step::Continue));

        let missing = registry
            .receive(
                MailAddr(9),
                RegistryMessage::Lookup {
                    key: 5,
                    reply_to: reply,
                },
            )
            .unwrap();
        assert!(missing.sends == vec![Delivery::new(reply, RegistryResult::Missing { key: 5 })]);
    }
}
