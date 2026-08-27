//! Typed subscription membership and publication.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol,
    SendEffects, User,
};
use thiserror::Error;

use crate::DeliveryRoute;

/// Commands accepted by a [`Topic`].
pub enum TopicMessage<P, Route> {
    /// Add a subscriber if absent.
    Subscribe(Route),
    /// Remove a subscriber if present.
    Unsubscribe(Route),
    /// Publish one owned value to the current membership snapshot.
    Publish(P),
}

/// Publication rejection preserving the unaccepted value.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TopicError<P> {
    /// No subscriber was present when publication was folded.
    #[error("topic publication rejected because there are no subscribers")]
    NoSubscribers(P),
}

/// Insertion-ordered typed publication behavior.
///
/// Subscription and unsubscription are idempotent and preserve survivor
/// order. Publication snapshots current membership and emits exactly one typed
/// delivery per subscriber in that order. With no subscribers it returns
/// [`TopicError::NoSubscribers`] containing the owned value; it never silently
/// drops a publication. Initialization is empty and the topic never terminates
/// itself. Membership order and empty-publication rejection are Bombay policy;
/// endpoint resolution and delivery are Address/Communication capabilities.
/// No method has a semantic panic condition.
pub struct Topic<A: Address, P, Route> {
    subscribers: Vec<Route>,
    marker: core::marker::PhantomData<fn() -> (A, P)>,
}

impl<A: Address, P, Route> Topic<A, P, Route> {
    /// Construct an empty topic definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow subscribers in publication order.
    #[must_use]
    pub fn subscribers(&self) -> &[Route] {
        &self.subscribers
    }
}

impl<A: Address, P, Route> Default for Topic<A, P, Route> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A, P, Route> BehaviorBase for Topic<A, P, Route>
where
    A: Address,
    P: Clone,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = P>> + Clone + PartialEq,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, P, Route> behavior::Protocol for Topic<A, P, Route>
where
    A: Address,
    P: Clone,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = P>> + Clone + PartialEq,
{
    type Addr = A;
    type Msg = TopicMessage<P, Route>;
}

impl<A, P, Route> Behavior for Topic<A, P, Route>
where
    A: Address,
    P: Clone,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = P>> + Clone + PartialEq,
    Route::Sends: behavior::SendsFor<User<A, TopicMessage<P, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Route::Sends;
    type Ph = Never;
    type Error = TopicError<P>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            TopicMessage::Subscribe(subscriber) => {
                if !self.subscribers.contains(&subscriber) {
                    self.subscribers.push(subscriber);
                }
                Ok(Actions::cont())
            }
            TopicMessage::Unsubscribe(subscriber) => {
                if let Some(index) = self
                    .subscribers
                    .iter()
                    .position(|current| current == &subscriber)
                {
                    self.subscribers.remove(index);
                }
                Ok(Actions::cont())
            }
            TopicMessage::Publish(value) => {
                if self.subscribers.is_empty() {
                    return Err(TopicError::NoSubscribers(value));
                }
                let mut sends = Route::Sends::empty();
                for subscriber in &self.subscribers {
                    sends.append(subscriber.clone().deliver(value.clone()));
                }
                Ok(Actions::send(sends))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Activate as _, Recipient};
    use behavior::{Delivery, MailAddr, MessageProtocol};

    #[test]
    fn membership_is_idempotent_and_publication_ordered() {
        let one = Recipient::from(MailAddr(1));
        let two = Recipient::from(MailAddr(2));
        let mut topic = Topic::<MailAddr, u8, Recipient<MessageProtocol<MailAddr, u8>>>::new()
            .initialize()
            .unwrap()
            .behavior;
        for subscriber in [one, two, one] {
            let subscribed = topic
                .receive(MailAddr(9), TopicMessage::Subscribe(subscriber))
                .unwrap();
            assert!(subscribed.sends.is_empty());
            assert!(subscribed.creates.is_empty());
            assert_eq!(subscribed.become_, crate::Step::Continue);
        }
        assert!(topic.subscribers() == [one, two]);
        let published = topic
            .receive(MailAddr(9), TopicMessage::Publish(7))
            .unwrap();
        assert!(published.sends == vec![Delivery::new(one, 7), Delivery::new(two, 7)]);
        let unsubscribed = topic
            .receive(MailAddr(9), TopicMessage::Unsubscribe(one))
            .unwrap();
        assert!(unsubscribed.sends.is_empty());
        assert!(unsubscribed.creates.is_empty());
        assert_eq!(unsubscribed.become_, crate::Step::Continue);
        assert!(topic.subscribers() == [two]);
    }

    #[test]
    fn empty_publication_returns_owned_value() {
        let mut topic = Topic::<MailAddr, u8, Recipient<MessageProtocol<MailAddr, u8>>>::new()
            .initialize()
            .unwrap()
            .behavior;
        assert!(matches!(
            topic.receive(MailAddr(9), TopicMessage::Publish(7)),
            Err(TopicError::NoSubscribers(7))
        ));
    }
}
