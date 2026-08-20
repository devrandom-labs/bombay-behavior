//! Typed subscription membership and publication.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, MessageProtocol, Never,
    NoBirths, Recipient, User,
};
use thiserror::Error;

/// Commands accepted by a [`Topic`].
pub enum TopicMessage<A: Address, P> {
    /// Add a subscriber if absent.
    Subscribe(Recipient<MessageProtocol<A, P>>),
    /// Remove a subscriber if present.
    Unsubscribe(Recipient<MessageProtocol<A, P>>),
    /// Publish one owned value to the current membership snapshot.
    Publish(P),
}

impl<A: Address, P> behavior::Protocol for TopicMessage<A, P> {
    type Addr = A;
    type Msg = TopicMessage<A, P>;
}

impl<A: Address, P> behavior::KeyedProtocol for TopicMessage<A, P> {
    type Key = behavior::NominalProtocolKey<Self>;
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
pub struct Topic<A: Address, P> {
    subscribers: Vec<Recipient<MessageProtocol<A, P>>>,
}

impl<A: Address, P> Topic<A, P> {
    /// Construct an empty topic definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Borrow subscribers in publication order.
    #[must_use]
    pub fn subscribers(&self) -> &[Recipient<MessageProtocol<A, P>>] {
        &self.subscribers
    }
}

impl<A: Address, P> Default for Topic<A, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address, P> BehaviorBase for Topic<A, P> {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, P> behavior::Protocol for Topic<A, P>
where
    A: Address,
    P: Clone,
{
    type Addr = A;
    type Msg = TopicMessage<A, P>;
}

impl<A, P> Behavior for Topic<A, P>
where
    A: Address,
    P: Clone,
{
    type Protocol = TopicMessage<A, P>;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Vec<Delivery<MessageProtocol<A, P>>>;
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
                    .position(|current| *current == subscriber)
                {
                    self.subscribers.remove(index);
                }
                Ok(Actions::cont())
            }
            TopicMessage::Publish(value) => {
                if self.subscribers.is_empty() {
                    return Err(TopicError::NoSubscribers(value));
                }
                Ok(Actions::send(
                    self.subscribers
                        .iter()
                        .copied()
                        .map(|subscriber| Delivery::new(subscriber, value.clone()))
                        .collect(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    #[test]
    fn membership_is_idempotent_and_publication_ordered() {
        let one = Recipient::from(MailAddr(1));
        let two = Recipient::from(MailAddr(2));
        let mut topic = Topic::new().initialize().unwrap().behavior;
        for subscriber in [one, two, one] {
            topic
                .receive(MailAddr(9), TopicMessage::Subscribe(subscriber))
                .unwrap();
        }
        assert!(topic.subscribers() == [one, two]);
        let published = topic
            .receive(MailAddr(9), TopicMessage::Publish(7))
            .unwrap();
        assert!(published.sends == vec![Delivery::new(one, 7), Delivery::new(two, 7)]);
        topic
            .receive(MailAddr(9), TopicMessage::Unsubscribe(one))
            .unwrap();
        assert!(topic.subscribers() == [two]);
    }

    #[test]
    fn empty_publication_returns_owned_value() {
        let mut topic = Topic::new().initialize().unwrap().behavior;
        assert!(matches!(
            topic.receive(MailAddr(9), TopicMessage::Publish(7)),
            Err(TopicError::NoSubscribers(7))
        ));
    }
}
