//! Typed name binding, lookup, and subscription behaviors.
//!
//! These folds own application-visible naming policy. Bombay Address retains
//! endpoint registration, generation authority, and physical resolution.

mod presence;
mod pub_sub;
mod registry;
mod resolver;
mod topic;

pub use presence::{
    Presence, PresenceEntry, PresenceError, PresenceMessage, PresenceOutcome, PresencePhase,
    PresenceReply, PresenceReport, PresenceSends, PresenceVersion,
};
pub use pub_sub::{PubSub, PubSubError, PubSubMessage, TopicMembership};
pub use registry::{Registry, RegistryError, RegistryMessage, RegistryResult};
pub use resolver::{Resolution, Resolver, ResolverConfigError, ResolverMessage};
pub use topic::{Topic, TopicError, TopicMessage};
