//! Versioned component-health aggregation.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// Monotonic version of one component's health evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObservationVersion(pub u64);

/// Closed health classification ordered from best to worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HealthStatus {
    /// The component is fully available under its declared policy.
    Healthy,
    /// The component remains usable with a declared impairment.
    Degraded,
    /// The component is unavailable or must not receive work.
    Unhealthy,
}

/// One present component in a [`HealthReport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentHealth<K> {
    /// Application-defined component identity.
    pub component: K,
    /// Latest committed evidence version.
    pub version: ObservationVersion,
    /// Latest committed classification.
    pub status: HealthStatus,
}

/// Complete retained state for one known component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentHealthState<K> {
    /// Latest evidence describes a present component.
    Present(ComponentHealth<K>),
    /// The component was explicitly removed at this version.
    Removed {
        /// Application-defined component identity.
        component: K,
        /// Removal version retained to reject stale resurrection.
        version: ObservationVersion,
    },
}

impl<K> ComponentHealthState<K> {
    fn component(&self) -> &K {
        match self {
            Self::Present(health) => &health.component,
            Self::Removed { component, .. } => component,
        }
    }

    const fn version(&self) -> ObservationVersion {
        match self {
            Self::Present(health) => health.version,
            Self::Removed { version, .. } => *version,
        }
    }
}

/// Factual point-in-time result returned by [`Health`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport<K> {
    /// Present components in first-observation order; tombstones are omitted.
    pub components: Vec<ComponentHealth<K>>,
}

impl<K> HealthReport<K> {
    /// Worst status among present components, or `Healthy` when none exist.
    #[must_use]
    pub fn overall(&self) -> HealthStatus {
        self.components
            .iter()
            .map(|component| component.status)
            .max()
            .unwrap_or(HealthStatus::Healthy)
    }
}

/// Commands accepted by [`Health`].
pub enum HealthMessage<K, Reply: behavior::Protocol> {
    /// Commit versioned evidence for a present component.
    Observe {
        /// Component identity.
        component: K,
        /// Evidence version within that component's stream.
        version: ObservationVersion,
        /// New factual classification.
        status: HealthStatus,
    },
    /// Commit a versioned tombstone.
    Remove {
        /// Component identity.
        component: K,
        /// Removal version within that component's stream.
        version: ObservationVersion,
    },
    /// Return a point-in-time report to a typed recipient.
    Query {
        /// Recipient whose protocol accepts [`HealthReport<K>`].
        reply_to: Recipient<Reply>,
    },
}

/// Rejected versioned health evidence.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HealthError<K> {
    /// Evidence predates the latest committed version.
    #[error("health evidence is stale")]
    Stale {
        /// Component whose evidence was rejected.
        component: K,
        /// Rejected version.
        observed: ObservationVersion,
        /// Latest committed version.
        current: ObservationVersion,
    },
    /// Evidence reuses a committed version with different meaning.
    #[error("health evidence contradicts the committed value at the same version")]
    ConflictingVersion {
        /// Component whose evidence was rejected.
        component: K,
        /// Reused version.
        version: ObservationVersion,
    },
}

/// Versioned typed health aggregation behavior.
///
/// State is an insertion-ordered product of [`ComponentHealthState`] values.
/// A greater version replaces the prior state. An identical observation is
/// idempotent. A lower version is [`HealthError::Stale`], and a same-version
/// contradiction is [`HealthError::ConflictingVersion`]; both preserve the
/// complete prior state. Removal retains a tombstone, so stale evidence cannot
/// resurrect a component. Query emits one named typed delivery and never
/// mutates state. Initialization is empty, the behavior never terminates by
/// policy, and it requires only ordinary typed delivery interpretation.
/// Versioning, aggregate ordering, and empty-set health are Bombay policy, not
/// actor-model laws. No method has a semantic panic condition.
pub struct Health<A: Address, K, Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>> {
    components: Vec<ComponentHealthState<K>>,
    address: core::marker::PhantomData<fn() -> (A, Reply)>,
}

impl<A: Address, K, Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>>
    Health<A, K, Reply>
{
    /// Construct an empty health definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            components: Vec::new(),
            address: core::marker::PhantomData,
        }
    }

    /// Borrow every retained present or removed component state.
    #[must_use]
    pub fn components(&self) -> &[ComponentHealthState<K>] {
        &self.components
    }
}

impl<A: Address, K, Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>> Default
    for Health<A, K, Reply>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address, K, Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>> BehaviorBase
    for Health<A, K, Reply>
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

#[derive(Clone, Copy)]
enum Evidence {
    Present(HealthStatus),
    Removed,
}

impl<A, K, Reply> Health<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>,
{
    fn commit(
        &mut self,
        component: K,
        version: ObservationVersion,
        evidence: Evidence,
    ) -> Result<(), HealthError<K>> {
        let replacement = match evidence {
            Evidence::Present(status) => ComponentHealthState::Present(ComponentHealth {
                component: component.clone(),
                version,
                status,
            }),
            Evidence::Removed => ComponentHealthState::Removed {
                component: component.clone(),
                version,
            },
        };
        let Some(index) = self
            .components
            .iter()
            .position(|state| state.component() == &component)
        else {
            self.components.push(replacement);
            return Ok(());
        };
        let current = self.components[index].version();
        if version < current {
            return Err(HealthError::Stale {
                component,
                observed: version,
                current,
            });
        }
        if version == current {
            if self.components[index] == replacement {
                return Ok(());
            }
            return Err(HealthError::ConflictingVersion { component, version });
        }
        self.components[index] = replacement;
        Ok(())
    }
    fn report(&self) -> HealthReport<K> {
        let components = self
            .components
            .iter()
            .filter_map(|state| match state {
                ComponentHealthState::Present(health) => Some(health.clone()),
                ComponentHealthState::Removed { .. } => None,
            })
            .collect::<Vec<_>>();
        HealthReport { components }
    }
}

impl<A, K, Reply> behavior::Protocol for Health<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>,
{
    type Addr = A;
    type Msg = HealthMessage<K, Reply>;
}

impl<A, K, Reply> behavior::KeyedProtocol for Health<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>,
{
    type Key = behavior::NominalProtocolKey<Self>;
}

impl<A, K, Reply> Behavior for Health<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = HealthReport<K>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = HealthError<K>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            HealthMessage::Observe {
                component,
                version,
                status,
            } => {
                self.commit(component, version, Evidence::Present(status))?;
                Ok(Actions::cont())
            }
            HealthMessage::Remove { component, version } => {
                self.commit(component, version, Evidence::Removed)?;
                Ok(Actions::cont())
            }
            HealthMessage::Query { reply_to } => {
                Ok(Actions::send(vec![Delivery::new(reply_to, self.report())]))
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
        type Msg = HealthReport<u8>;
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

    type TestHealth = Health<MailAddr, u8, Reply>;

    #[test]
    fn stale_and_conflicting_evidence_preserve_committed_state() {
        let mut health = (TestHealth::new()).initialize().unwrap().behavior;
        health
            .receive(
                MailAddr(9),
                HealthMessage::Observe {
                    component: 1,
                    version: ObservationVersion(3),
                    status: HealthStatus::Degraded,
                },
            )
            .unwrap();

        assert!(matches!(
            health.receive(
                MailAddr(9),
                HealthMessage::Observe {
                    component: 1,
                    version: ObservationVersion(2),
                    status: HealthStatus::Healthy,
                },
            ),
            Err(HealthError::Stale {
                component: 1,
                observed: ObservationVersion(2),
                current: ObservationVersion(3),
            })
        ));
        assert!(matches!(
            health.receive(
                MailAddr(9),
                HealthMessage::Observe {
                    component: 1,
                    version: ObservationVersion(3),
                    status: HealthStatus::Unhealthy,
                },
            ),
            Err(HealthError::ConflictingVersion {
                component: 1,
                version: ObservationVersion(3),
            })
        ));
        assert_eq!(
            health.components(),
            [ComponentHealthState::Present(ComponentHealth {
                component: 1,
                version: ObservationVersion(3),
                status: HealthStatus::Degraded,
            })]
        );
    }

    #[test]
    fn tombstone_rejects_resurrection_and_report_aggregates_worst_status() {
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let mut health = (TestHealth::new()).initialize().unwrap().behavior;
        for (component, status) in [(1, HealthStatus::Healthy), (2, HealthStatus::Unhealthy)] {
            health
                .receive(
                    MailAddr(9),
                    HealthMessage::Observe {
                        component,
                        version: ObservationVersion(1),
                        status,
                    },
                )
                .unwrap();
        }
        let report = health
            .receive(MailAddr(9), HealthMessage::Query { reply_to: reply })
            .unwrap();
        let sent = report
            .sends
            .into_iter()
            .next()
            .expect("one report delivery");
        assert_eq!(sent.to, reply);
        assert_eq!(sent.message.overall(), HealthStatus::Unhealthy);
        assert_eq!(
            sent.message.components,
            vec![
                ComponentHealth {
                    component: 1,
                    version: ObservationVersion(1),
                    status: HealthStatus::Healthy,
                },
                ComponentHealth {
                    component: 2,
                    version: ObservationVersion(1),
                    status: HealthStatus::Unhealthy,
                },
            ]
        );

        health
            .receive(
                MailAddr(9),
                HealthMessage::Remove {
                    component: 2,
                    version: ObservationVersion(2),
                },
            )
            .unwrap();
        assert!(matches!(
            health.receive(
                MailAddr(9),
                HealthMessage::Observe {
                    component: 2,
                    version: ObservationVersion(1),
                    status: HealthStatus::Healthy,
                },
            ),
            Err(HealthError::Stale { .. })
        ));
        let after = health
            .receive(MailAddr(9), HealthMessage::Query { reply_to: reply })
            .unwrap();
        let sent = after.sends.into_iter().next().expect("one report delivery");
        assert_eq!(sent.to, reply);
        assert_eq!(sent.message.overall(), HealthStatus::Healthy);
        assert_eq!(
            sent.message.components,
            vec![ComponentHealth {
                component: 1,
                version: ObservationVersion(1),
                status: HealthStatus::Healthy,
            }]
        );
    }
}
