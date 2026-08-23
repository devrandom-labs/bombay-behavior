//! Read-only typed name resolution capability.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol, Recipient,
    User,
};
use thiserror::Error;

use crate::DeliveryRoute;

/// Complete factual result returned by [`Resolver`].
pub enum Resolution<K, D: Protocol> {
    /// The key resolved to this typed recipient.
    Found {
        /// Queried key.
        key: K,
        /// Point-in-time recipient.
        recipient: Recipient<D>,
    },
    /// No configured binding names the key.
    Missing {
        /// Queried key.
        key: K,
    },
}

/// The only operation exposed by a [`Resolver`] recipient.
pub enum ResolverMessage<K, Route> {
    /// Resolve one typed key without granting mutation authority.
    Resolve {
        /// Queried key.
        key: K,
        /// Typed result recipient.
        reply_to: Route,
    },
}

/// Rejected immutable resolver definition.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResolverConfigError<K> {
    /// Two source entries name the same key.
    #[error("resolver definition contains a duplicate key")]
    DuplicateKey {
        /// Duplicate key cloned from the borrowed definition; the caller
        /// retains ownership of the complete source slice.
        key: K,
    },
}

/// Immutable read-only typed resolver behavior.
///
/// Construction copies a borrowed unique binding definition, so rejection
/// cannot consume caller ownership. The public protocol contains only
/// [`ResolverMessage::Resolve`], making registry mutation authority
/// unrepresentable for holders of a resolver recipient. Every lookup emits one
/// factual [`Resolution`], including absence. Binding order is retained for
/// deterministic inspection but does not affect lookup meaning. Initialization
/// is empty, the behavior creates no actors, never terminates by policy, and
/// requires only typed delivery. Capability separation and missing-as-fact are
/// Bombay policy; endpoint validity is interpreted by Bombay Address. No
/// method has a semantic panic condition.
pub struct Resolver<
    A: Address,
    K,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = Resolution<K, D>>,
    Route: DeliveryRoute<Reply>,
> {
    bindings: Vec<(K, Recipient<D>)>,
    marker: core::marker::PhantomData<fn() -> (A, Reply, Route)>,
}

impl<A, K, D, Reply, Route> Resolver<A, K, D, Reply, Route>
where
    A: Address,
    K: Clone + Eq,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = Resolution<K, D>>,
    Route: DeliveryRoute<Reply>,
{
    /// Copy one borrowed immutable binding definition.
    ///
    /// # Errors
    ///
    /// Returns [`ResolverConfigError::DuplicateKey`] for the first duplicate;
    /// the borrowed source remains owned by the caller.
    pub fn from_bindings(bindings: &[(K, Recipient<D>)]) -> Result<Self, ResolverConfigError<K>> {
        let mut accepted = Vec::with_capacity(bindings.len());
        for (key, recipient) in bindings {
            if accepted
                .iter()
                .any(|(accepted_key, _): &(K, Recipient<D>)| accepted_key == key)
            {
                return Err(ResolverConfigError::DuplicateKey { key: key.clone() });
            }
            accepted.push((key.clone(), *recipient));
        }
        Ok(Self {
            bindings: accepted,
            marker: core::marker::PhantomData,
        })
    }

    /// Borrow the complete immutable binding definition.
    #[must_use]
    pub fn bindings(&self) -> &[(K, Recipient<D>)] {
        &self.bindings
    }
}

impl<A, K, D, Reply, Route> BehaviorBase for Resolver<A, K, D, Reply, Route>
where
    A: Address,
    K: Clone + Eq,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = Resolution<K, D>>,
    Route: DeliveryRoute<Reply>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, D, Reply, Route> behavior::Protocol for Resolver<A, K, D, Reply, Route>
where
    A: Address,
    K: Clone + Eq,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = Resolution<K, D>>,
    Route: DeliveryRoute<Reply>,
{
    type Addr = A;
    type Msg = ResolverMessage<K, Route>;
}

impl<A, K, D, Reply, Route> Behavior for Resolver<A, K, D, Reply, Route>
where
    A: Address,
    K: Clone + Eq,
    D: Protocol<Addr = A>,
    Reply: behavior::Protocol<Addr = A, Msg = Resolution<K, D>>,
    Route: DeliveryRoute<Reply>,
    Route::Sends: behavior::SendsFor<User<A, ResolverMessage<K, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Route::Sends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let ResolverMessage::Resolve { key, reply_to } = event.message;
        let result = self
            .bindings
            .iter()
            .find(|(bound, _)| bound == &key)
            .map_or(
                Resolution::Missing { key: key.clone() },
                |(_, recipient)| Resolution::Found {
                    key: key.clone(),
                    recipient: *recipient,
                },
            );
        Ok(Actions::send(reply_to.deliver(result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;
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
        type Msg = Resolution<u8, Destination>;
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
    type Subject = Resolver<MailAddr, u8, Destination, Reply, Recipient<Reply>>;
    #[test]
    fn duplicate_definition_is_rejected_without_consuming_source() {
        let destination = Recipient::global(MailAddr(1));
        let source = vec![(1, destination), (1, destination)];
        assert!(matches!(
            Subject::from_bindings(&source),
            Err(ResolverConfigError::DuplicateKey { key: 1 })
        ));
        assert_eq!(source.len(), 2);
    }
    #[test]
    fn protocol_reports_found_and_missing_without_mutation_authority() {
        let destination = Recipient::global(MailAddr(1));
        let mut s = (Subject::from_bindings(&[(1, destination)]).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        for key in [1, 2] {
            let a = s
                .receive(
                    MailAddr(9),
                    ResolverMessage::Resolve {
                        key,
                        reply_to: Recipient::global(MailAddr(2)),
                    },
                )
                .unwrap();
            assert!(matches!(
                (&a.sends[0].message, key),
                (Resolution::Found { .. }, 1) | (Resolution::Missing { .. }, 2)
            ));
        }
        assert_eq!(s.bindings().len(), 1);
    }
}
