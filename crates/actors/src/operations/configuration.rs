//! Versioned configuration acceptance and query policy.

#[cfg(test)]
use behavior::Recipient;
use behavior::{Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, User};
use thiserror::Error;

use crate::DeliveryRoute;

/// Monotonic version in one configuration stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigurationVersion(pub u64);

/// Complete configuration lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigurationState<C> {
    /// No configuration has been accepted.
    Unconfigured,
    /// One versioned value is current.
    Configured {
        /// Latest accepted version.
        version: ConfigurationVersion,
        /// Latest accepted owned value.
        value: C,
    },
}

/// Commands accepted by [`Configuration`].
pub enum ConfigurationMessage<C, Route> {
    /// Attempt to atomically replace the current configuration.
    Apply {
        /// Candidate version.
        version: ConfigurationVersion,
        /// Candidate owned configuration.
        value: C,
    },
    /// Return a snapshot of the complete current state.
    Query {
        /// Typed state recipient.
        reply_to: Route,
    },
}

/// Typed configuration rejection preserving candidate ownership.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ConfigurationError<C> {
    /// Candidate version predates the committed version.
    #[error("configuration candidate is stale")]
    Stale {
        /// Rejected version.
        proposed: ConfigurationVersion,
        /// Current version.
        current: ConfigurationVersion,
        /// Rejected owned value.
        value: C,
    },
    /// Candidate reuses the committed version with a different value.
    #[error("configuration candidate conflicts at the committed version")]
    ConflictingVersion {
        /// Reused version.
        version: ConfigurationVersion,
        /// Rejected owned value.
        value: C,
    },
}

/// Atomic versioned-configuration behavior.
///
/// The first candidate is accepted. A greater version atomically replaces the
/// prior value, an identical version and value is idempotent, and stale or
/// same-version contradictory candidates return ownership in a concrete
/// [`ConfigurationError`] without mutation. Query clones the explicit
/// [`ConfigurationState`] to one typed recipient. Initialization is empty, no
/// actor is created, and the template does not terminate its host. Version
/// ordering and idempotence are Bombay policy; acquiring, decoding, validating,
/// or exporting external configuration remains an application/System adapter
/// responsibility. No method has a semantic panic condition.
pub struct Configuration<
    A: Address,
    C,
    Reply: behavior::Protocol<Addr = A, Msg = ConfigurationState<C>>,
    Route: DeliveryRoute<Reply>,
> {
    state: ConfigurationState<C>,
    marker: core::marker::PhantomData<fn() -> (A, Reply, Route)>,
}

impl<A, C, Reply, Route> Configuration<A, C, Reply, Route>
where
    A: Address,
    C: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ConfigurationState<C>>,
    Route: DeliveryRoute<Reply>,
{
    /// Construct an explicitly unconfigured policy.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ConfigurationState::Unconfigured,
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow the complete current state.
    #[must_use]
    pub const fn state(&self) -> &ConfigurationState<C> {
        &self.state
    }

    fn apply(
        &mut self,
        version: ConfigurationVersion,
        value: C,
    ) -> Result<(), ConfigurationError<C>> {
        let ConfigurationState::Configured {
            version: current,
            value: committed,
        } = &self.state
        else {
            self.state = ConfigurationState::Configured { version, value };
            return Ok(());
        };
        if version < *current {
            return Err(ConfigurationError::Stale {
                proposed: version,
                current: *current,
                value,
            });
        }
        if version == *current {
            if value == *committed {
                return Ok(());
            }
            return Err(ConfigurationError::ConflictingVersion { version, value });
        }
        self.state = ConfigurationState::Configured { version, value };
        Ok(())
    }
}

impl<A, C, Reply, Route> Default for Configuration<A, C, Reply, Route>
where
    A: Address,
    C: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ConfigurationState<C>>,
    Route: DeliveryRoute<Reply>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A, C, Reply, Route> BehaviorBase for Configuration<A, C, Reply, Route>
where
    A: Address,
    C: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ConfigurationState<C>>,
    Route: DeliveryRoute<Reply>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, C, Reply, Route> behavior::Protocol for Configuration<A, C, Reply, Route>
where
    A: Address,
    C: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ConfigurationState<C>>,
    Route: DeliveryRoute<Reply>,
{
    type Addr = A;
    type Msg = ConfigurationMessage<C, Route>;
}

impl<A, C, Reply, Route> Behavior for Configuration<A, C, Reply, Route>
where
    A: Address,
    C: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ConfigurationState<C>>,
    Route: DeliveryRoute<Reply>,
    Route::Sends: behavior::SendsFor<User<A, ConfigurationMessage<C, Route>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Route::Sends;
    type Ph = Never;
    type Error = ConfigurationError<C>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            ConfigurationMessage::Apply { version, value } => {
                self.apply(version, value)?;
                Ok(Actions::cont())
            }
            ConfigurationMessage::Query { reply_to } => {
                Ok(Actions::send(reply_to.deliver(self.state.clone())))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Activate as _;
    use behavior::MailAddr;

    use super::*;

    struct Reply;
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = ConfigurationState<u8>;
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

    type Subject = Configuration<MailAddr, u8, Reply, Recipient<Reply>>;

    #[test]
    fn stale_and_conflicting_candidates_return_ownership_atomically() {
        let mut subject = (Subject::new()).initialize().unwrap().behavior;
        let applied = subject
            .receive(
                MailAddr(9),
                ConfigurationMessage::Apply {
                    version: ConfigurationVersion(2),
                    value: 20,
                },
            )
            .unwrap();
        assert!(applied.sends.is_empty());
        assert!(applied.creates.is_empty());
        assert_eq!(applied.become_, behavior::Step::Continue);
        assert!(matches!(
            subject.receive(
                MailAddr(9),
                ConfigurationMessage::Apply {
                    version: ConfigurationVersion(1),
                    value: 10
                }
            ),
            Err(ConfigurationError::Stale { value: 10, .. })
        ));
        assert!(matches!(
            subject.receive(
                MailAddr(9),
                ConfigurationMessage::Apply {
                    version: ConfigurationVersion(2),
                    value: 21
                }
            ),
            Err(ConfigurationError::ConflictingVersion { value: 21, .. })
        ));
        assert_eq!(
            subject.state(),
            &ConfigurationState::Configured {
                version: ConfigurationVersion(2),
                value: 20
            }
        );
    }

    #[test]
    fn query_reports_unconfigured_and_configured_as_distinct_states() {
        let mut subject = (Subject::new()).initialize().unwrap().behavior;
        let initial = subject
            .receive(
                MailAddr(9),
                ConfigurationMessage::Query {
                    reply_to: Recipient::global(MailAddr(1)),
                },
            )
            .unwrap();
        assert!(matches!(
            initial.sends[0].message,
            ConfigurationState::Unconfigured
        ));
    }
}
