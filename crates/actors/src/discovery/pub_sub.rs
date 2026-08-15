//! Keyed typed publication and subscription membership.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// One topic and its subscribers in delivery order.
pub struct TopicMembership<K, D: Behavior> {
    /// Application-defined topic identity.
    pub topic: K,
    /// Unique subscribers in subscription order.
    pub subscribers: Vec<Recipient<D>>,
}

/// Operations accepted by [`PubSub`].
pub enum PubSubMessage<K, P, D: Behavior> {
    /// Idempotently subscribe one typed recipient.
    Subscribe {
        /// Topic identity.
        topic: K,
        /// Subscriber.
        subscriber: Recipient<D>,
    },
    /// Remove one existing subscription.
    Unsubscribe {
        /// Topic identity.
        topic: K,
        /// Subscriber.
        subscriber: Recipient<D>,
    },
    /// Publish one value to a point-in-time membership snapshot.
    Publish {
        /// Topic identity.
        topic: K,
        /// Owned publication.
        value: P,
    },
}

/// Typed keyed-publication rejection.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PubSubError<K, P> {
    /// Unsubscription named an unknown topic.
    #[error("pub-sub topic is unknown")]
    UnknownTopic {
        /// Rejected topic.
        topic: K,
    },
    /// Recipient is not subscribed to the named topic.
    #[error("recipient is not subscribed to the pub-sub topic")]
    NotSubscribed {
        /// Rejected topic.
        topic: K,
    },
    /// Publication had no recipients; ownership is returned.
    #[error("pub-sub topic has no subscribers")]
    NoSubscribers {
        /// Topic with no recipients.
        topic: K,
        /// Undelivered owned publication.
        value: P,
    },
}

/// Deterministic keyed typed publish/subscribe behavior.
///
/// Topics are introduced by the first subscription and retained after their
/// final unsubscription so an unknown topic remains distinguishable from a
/// known empty one. Subscription is idempotent. Unsubscription from an unknown
/// topic or absent membership is rejected atomically. Publication clones the
/// value once per point-in-time subscriber in subscription order; an unknown
/// or empty topic returns the original value in [`PubSubError::NoSubscribers`].
/// Initialization is empty, the behavior creates no actors, and it never
/// terminates by policy. Membership retention and ordered snapshot delivery
/// are Bombay policy; physical delivery remains a Communication capability.
/// Cloning a publication can panic only if the application-provided `Clone`
/// implementation panics, before any template state is changed.
pub struct PubSub<A: Address, K, P, D: Behavior<Addr = A, Msg = P>> {
    topics: Vec<TopicMembership<K, D>>,
    marker: core::marker::PhantomData<fn() -> (A, P)>,
}

impl<A, K, P, D> PubSub<A, K, P, D>
where
    A: Address,
    K: Eq,
    D: Behavior<Addr = A, Msg = P>,
{
    /// Construct an empty keyed fabric.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            topics: Vec::new(),
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow complete topic membership in introduction order.
    #[must_use]
    pub fn topics(&self) -> &[TopicMembership<K, D>] {
        &self.topics
    }

    fn subscribe(&mut self, topic: K, subscriber: Recipient<D>) {
        if let Some(membership) = self.topics.iter_mut().find(|entry| entry.topic == topic) {
            if !membership.subscribers.contains(&subscriber) {
                membership.subscribers.push(subscriber);
            }
        } else {
            self.topics.push(TopicMembership {
                topic,
                subscribers: vec![subscriber],
            });
        }
    }

    fn unsubscribe(&mut self, topic: K, subscriber: Recipient<D>) -> Result<(), PubSubError<K, P>> {
        let Some(membership) = self.topics.iter_mut().find(|entry| entry.topic == topic) else {
            return Err(PubSubError::UnknownTopic { topic });
        };
        let Some(index) = membership
            .subscribers
            .iter()
            .position(|member| *member == subscriber)
        else {
            return Err(PubSubError::NotSubscribed { topic });
        };
        membership.subscribers.remove(index);
        Ok(())
    }
}

impl<A, K, P, D> Default for PubSub<A, K, P, D>
where
    A: Address,
    K: Eq,
    D: Behavior<Addr = A, Msg = P>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, K, P, D> BehaviorBase for PubSub<A, K, P, D>
where
    A: Address,
    K: Clone + Eq,
    P: Clone,
    D: Behavior<Addr = A, Msg = P>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, P, D> Behavior for PubSub<A, K, P, D>
where
    A: Address,
    K: Clone + Eq,
    P: Clone,
    D: Behavior<Addr = A, Msg = P>,
{
    type Addr = A;
    type Msg = PubSubMessage<K, P, D>;
    type Event = User<A, Self::Msg>;
    type Sends = Vec<Delivery<D>>;
    type Ph = Never;
    type Error = PubSubError<K, P>;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            PubSubMessage::Subscribe { topic, subscriber } => {
                self.subscribe(topic, subscriber);
                Ok(Actions::cont())
            }
            PubSubMessage::Unsubscribe { topic, subscriber } => {
                self.unsubscribe(topic, subscriber)?;
                Ok(Actions::cont())
            }
            PubSubMessage::Publish { topic, value } => {
                let Some(membership) = self.topics.iter().find(|entry| entry.topic == topic) else {
                    return Err(PubSubError::NoSubscribers { topic, value });
                };
                if membership.subscribers.is_empty() {
                    return Err(PubSubError::NoSubscribers { topic, value });
                }
                Ok(Actions::send(
                    membership
                        .subscribers
                        .iter()
                        .map(|subscriber| Delivery::new(*subscriber, value.clone()))
                        .collect(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use behavior::MailAddr;
    struct Destination;
    impl Behavior for Destination {
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
    type Subject = PubSub<MailAddr, u8, u8, Destination>;
    #[test]
    fn topics_and_subscribers_preserve_first_order() {
        let one = Recipient::global(MailAddr(1));
        let two = Recipient::global(MailAddr(2));
        let mut s = crate::Compose::new(Subject::new())
            .initialize()
            .unwrap()
            .behavior;
        for subscriber in [one, two, one] {
            s.receive(
                MailAddr(9),
                PubSubMessage::Subscribe {
                    topic: 7,
                    subscriber,
                },
            )
            .unwrap();
        }
        let a = s
            .receive(MailAddr(9), PubSubMessage::Publish { topic: 7, value: 4 })
            .unwrap();
        assert!(a.sends == vec![Delivery::new(one, 4), Delivery::new(two, 4)]);
    }
    #[test]
    fn empty_known_and_unknown_topics_return_publication() {
        let one = Recipient::global(MailAddr(1));
        let mut s = crate::Compose::new(Subject::new())
            .initialize()
            .unwrap()
            .behavior;
        s.receive(
            MailAddr(9),
            PubSubMessage::Subscribe {
                topic: 1,
                subscriber: one,
            },
        )
        .unwrap();
        s.receive(
            MailAddr(9),
            PubSubMessage::Unsubscribe {
                topic: 1,
                subscriber: one,
            },
        )
        .unwrap();
        for topic in [1, 2] {
            assert!(
                matches!(s.receive(MailAddr(9),PubSubMessage::Publish{topic,value:8}),Err(PubSubError::NoSubscribers{topic:returned,value:8}) if returned==topic)
            );
        }
    }
}
