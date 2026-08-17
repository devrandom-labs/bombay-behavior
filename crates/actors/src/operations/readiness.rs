//! Versioned readiness policy over a fixed dependency set.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

use super::ObservationVersion;

/// Classification carried by committed readiness evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessStatus {
    /// The dependency currently permits admission.
    Ready,
    /// The dependency currently prevents admission.
    NotReady,
}

/// Complete evidence phase for one readiness dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessEvidence {
    /// No evidence has yet been committed.
    Unknown,
    /// Versioned evidence has been committed.
    Observed {
        /// Latest committed version.
        version: ObservationVersion,
        /// Latest committed classification.
        status: ReadinessStatus,
    },
}

/// One configured dependency and its latest evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyReadiness<K> {
    /// Application-defined dependency identity.
    pub dependency: K,
    /// Complete current evidence phase.
    pub evidence: ReadinessEvidence,
}

/// Point-in-time readiness result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessReport<K> {
    /// Dependencies in configuration order with complete current evidence.
    pub dependencies: Vec<DependencyReadiness<K>>,
}

impl<K> ReadinessReport<K> {
    /// True exactly when every configured dependency is ready.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.dependencies.iter().all(|state| {
            matches!(
                state.evidence,
                ReadinessEvidence::Observed {
                    status: ReadinessStatus::Ready,
                    ..
                }
            )
        })
    }
}

/// Inputs accepted by [`Readiness`].
pub enum ReadinessMessage<K, Reply: behavior::Protocol> {
    /// Commit versioned readiness evidence for a configured dependency.
    Observe {
        /// Dependency identity.
        dependency: K,
        /// Version within this dependency's evidence stream.
        version: ObservationVersion,
        /// New readiness classification.
        status: ReadinessStatus,
    },
    /// Return a complete point-in-time report.
    Query {
        /// Typed report recipient.
        reply_to: Recipient<Reply>,
    },
}

/// Rejected readiness evidence.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReadinessError<K> {
    /// Evidence named a dependency outside the fixed policy set.
    #[error("readiness dependency is not configured")]
    UnknownDependency {
        /// Rejected dependency.
        dependency: K,
    },
    /// Evidence predates the latest committed version.
    #[error("readiness evidence is stale")]
    Stale {
        /// Rejected dependency.
        dependency: K,
        /// Rejected version.
        observed: ObservationVersion,
        /// Latest committed version.
        current: ObservationVersion,
    },
    /// Evidence reuses one version with a different classification.
    #[error("readiness evidence conflicts at the committed version")]
    ConflictingVersion {
        /// Rejected dependency.
        dependency: K,
        /// Reused version.
        version: ObservationVersion,
    },
}

/// Fixed-dependency, versioned admission-readiness policy.
///
/// Construction normalizes dependencies by first occurrence and initializes
/// each to [`ReadinessEvidence::Unknown`]. Greater evidence versions replace the
/// prior classification, identical evidence is idempotent, and stale or
/// same-version contradictory evidence is rejected without mutation. A report
/// is ready exactly when every configured dependency is `Ready`; the empty set
/// is ready. Initialization emits no effects, the template creates no actors,
/// never terminates by policy, and requires only typed result delivery. Fixed
/// membership, version ordering, and empty-set readiness are deliberate Bombay
/// policy. Export through HTTP or orchestration remains a System adapter
/// responsibility. No method has a semantic panic condition.
pub struct Readiness<A: Address, K, Reply: behavior::Protocol<Addr = A, Msg = ReadinessReport<K>>> {
    dependencies: Vec<DependencyReadiness<K>>,
    marker: core::marker::PhantomData<fn() -> (A, Reply)>,
}

impl<A, K, Reply> Readiness<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ReadinessReport<K>>,
{
    /// Construct readiness policy for a fixed dependency set.
    #[must_use]
    pub fn new(dependencies: impl IntoIterator<Item = K>) -> Self {
        let mut states = Vec::new();
        for dependency in dependencies {
            if !states
                .iter()
                .any(|state: &DependencyReadiness<K>| state.dependency == dependency)
            {
                states.push(DependencyReadiness {
                    dependency,
                    evidence: ReadinessEvidence::Unknown,
                });
            }
        }
        Self {
            dependencies: states,
            marker: core::marker::PhantomData,
        }
    }

    /// Borrow configured dependencies and their complete current evidence.
    #[must_use]
    pub fn dependencies(&self) -> &[DependencyReadiness<K>] {
        &self.dependencies
    }

    fn observe(
        &mut self,
        dependency: K,
        version: ObservationVersion,
        status: ReadinessStatus,
    ) -> Result<(), ReadinessError<K>> {
        let Some(current) = self
            .dependencies
            .iter_mut()
            .find(|state| state.dependency == dependency)
        else {
            return Err(ReadinessError::UnknownDependency { dependency });
        };
        let ReadinessEvidence::Observed {
            version: committed,
            status: committed_status,
        } = current.evidence
        else {
            current.evidence = ReadinessEvidence::Observed { version, status };
            return Ok(());
        };
        if version < committed {
            return Err(ReadinessError::Stale {
                dependency,
                observed: version,
                current: committed,
            });
        }
        if version == committed {
            if committed_status == status {
                return Ok(());
            }
            return Err(ReadinessError::ConflictingVersion {
                dependency,
                version,
            });
        }
        current.evidence = ReadinessEvidence::Observed { version, status };
        Ok(())
    }

    fn report(&self) -> ReadinessReport<K> {
        ReadinessReport {
            dependencies: self.dependencies.clone(),
        }
    }
}

impl<A, K, Reply> BehaviorBase for Readiness<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ReadinessReport<K>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, Reply> behavior::Protocol for Readiness<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ReadinessReport<K>>,
{
    type Addr = A;
    type Msg = ReadinessMessage<K, Reply>;
}

impl<A, K, Reply> Behavior for Readiness<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = ReadinessReport<K>>,
{
    type Event = User<A, Self::Msg>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = ReadinessError<K>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {
            ReadinessMessage::Observe {
                dependency,
                version,
                status,
            } => {
                self.observe(dependency, version, status)?;
                Ok(Actions::cont())
            }
            ReadinessMessage::Query { reply_to } => {
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
        type Msg = ReadinessReport<u8>;
    }

    impl Behavior for Reply {
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    type Subject = Readiness<MailAddr, u8, Reply>;

    #[test]
    fn all_dependencies_must_have_ready_evidence() {
        let mut subject = (Subject::new([1, 1, 2])).initialize().unwrap().behavior;
        let query = |subject: &mut crate::Active<Subject>| {
            subject
                .receive(
                    MailAddr(9),
                    ReadinessMessage::Query {
                        reply_to: Recipient::global(MailAddr(1)),
                    },
                )
                .unwrap()
        };
        assert!(!query(&mut subject).sends[0].message.ready());
        let _ = subject
            .receive(
                MailAddr(9),
                ReadinessMessage::Observe {
                    dependency: 1,
                    version: ObservationVersion(1),
                    status: ReadinessStatus::Ready,
                },
            )
            .unwrap();
        assert!(!query(&mut subject).sends[0].message.ready());
        let _ = subject
            .receive(
                MailAddr(9),
                ReadinessMessage::Observe {
                    dependency: 2,
                    version: ObservationVersion(1),
                    status: ReadinessStatus::Ready,
                },
            )
            .unwrap();
        assert!(query(&mut subject).sends[0].message.ready());
    }

    #[test]
    fn stale_conflicting_and_unknown_evidence_are_atomic() {
        let mut subject = (Subject::new([1])).initialize().unwrap().behavior;
        let _ = subject
            .receive(
                MailAddr(9),
                ReadinessMessage::Observe {
                    dependency: 1,
                    version: ObservationVersion(2),
                    status: ReadinessStatus::Ready,
                },
            )
            .unwrap();
        assert!(matches!(
            subject.receive(
                MailAddr(9),
                ReadinessMessage::Observe {
                    dependency: 1,
                    version: ObservationVersion(1),
                    status: ReadinessStatus::NotReady
                }
            ),
            Err(ReadinessError::Stale { .. })
        ));
        assert!(matches!(
            subject.receive(
                MailAddr(9),
                ReadinessMessage::Observe {
                    dependency: 1,
                    version: ObservationVersion(2),
                    status: ReadinessStatus::NotReady
                }
            ),
            Err(ReadinessError::ConflictingVersion { .. })
        ));
        assert!(matches!(
            subject.receive(
                MailAddr(9),
                ReadinessMessage::Observe {
                    dependency: 9,
                    version: ObservationVersion(1),
                    status: ReadinessStatus::Ready
                }
            ),
            Err(ReadinessError::UnknownDependency { dependency: 9 })
        ));
        assert!(matches!(
            subject.dependencies()[0].evidence,
            ReadinessEvidence::Observed {
                status: ReadinessStatus::Ready,
                ..
            }
        ));
    }

    #[test]
    fn empty_dependency_set_is_ready() {
        let mut subject = (Subject::new([])).initialize().unwrap().behavior;
        let report = subject
            .receive(
                MailAddr(9),
                ReadinessMessage::Query {
                    reply_to: Recipient::global(MailAddr(1)),
                },
            )
            .unwrap();
        assert!(report.sends[0].message.ready());
    }
}
