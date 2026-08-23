//! Feature policy as a named specialization of versioned configuration.

use super::{Configuration, ConfigurationState};

/// Exhaustive state of one feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureStatus {
    /// The feature is enabled.
    Enabled,
    /// The feature is disabled.
    Disabled,
}

/// One member of a statically known application feature identity type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Feature<F> {
    /// Application-defined feature identity.
    pub feature: F,
    /// Explicit state; absence never implies a state.
    pub status: FeatureStatus,
}

/// A duplicate-free, deterministically ordered feature policy product.
///
/// Construction keeps the final declaration for a duplicate identity at its
/// original position. This is deliberate Bombay policy and makes one feature
/// incapable of occupying contradictory states in the same configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureSet<F> {
    features: Vec<Feature<F>>,
}

impl<F: Eq> FeatureSet<F> {
    /// Construct one coherent set from ordered declarations.
    #[must_use]
    pub fn new(features: impl IntoIterator<Item = Feature<F>>) -> Self {
        let mut normalized: Vec<Feature<F>> = Vec::new();
        for candidate in features {
            if let Some(existing) = normalized
                .iter_mut()
                .find(|existing| existing.feature == candidate.feature)
            {
                existing.status = candidate.status;
            } else {
                normalized.push(candidate);
            }
        }
        Self {
            features: normalized,
        }
    }

    /// Borrow declarations in deterministic first-identity order.
    #[must_use]
    pub fn features(&self) -> &[Feature<F>] {
        &self.features
    }
}

/// Versioned closed feature-state protocol.
///
/// `Features` is the demonstrated named specialization
/// `Configuration<FeatureSet<F>>`: it inherits atomic version ordering,
/// idempotence, stale/conflict ownership return, empty initialization, typed
/// query delivery, and non-termination from [`Configuration`]. The product
/// type adds the feature-specific invariant that each identity has exactly one
/// explicit status. External flag sources and evaluation remain application or
/// System responsibilities; no runtime capability beyond ordinary typed sends
/// is required and there is no semantic panic condition.
pub type Features<A, F, Reply, Route> = Configuration<A, FeatureSet<F>, Reply, Route>;

/// Complete state returned by the [`Features`] query protocol.
pub type FeaturesState<F> = ConfigurationState<FeatureSet<F>>;

#[cfg(test)]
mod tests {
    use behavior::{Address, Behavior};

    use super::*;

    #[test]
    fn duplicate_identity_replaces_status_without_reordering() {
        let set = FeatureSet::new([
            Feature {
                feature: 1,
                status: FeatureStatus::Disabled,
            },
            Feature {
                feature: 2,
                status: FeatureStatus::Enabled,
            },
            Feature {
                feature: 1,
                status: FeatureStatus::Enabled,
            },
        ]);
        assert_eq!(
            set.features(),
            [
                Feature {
                    feature: 1,
                    status: FeatureStatus::Enabled
                },
                Feature {
                    feature: 2,
                    status: FeatureStatus::Enabled
                }
            ]
        );
    }

    // This signature proves the specialization remains directly driveable by
    // the same concrete Behavior contract; it introduces no private loop.
    fn assert_behavior<A, F, Reply>()
    where
        A: Address,
        F: Clone + Eq,
        Reply: behavior::Protocol<Addr = A, Msg = FeaturesState<F>>,
        Features<A, F, Reply, behavior::Recipient<Reply>>: Behavior,
    {
    }

    #[test]
    fn specialization_has_the_universal_behavior_contract() {
        struct Reply;
        impl behavior::Protocol for Reply {
            type Addr = behavior::MailAddr;
            type Msg = FeaturesState<u8>;
        }

        impl Behavior for Reply {
            type Protocol = Self;
            type Event = behavior::User<crate::BehaviorAddr<Self>, crate::BehaviorMessage<Self>>;
            type Sends = Vec<behavior::Never>;
            type Ph = behavior::Never;
            type Error = behavior::Never;
            type Birth = behavior::NoBirths;
            fn transition(
                &mut self,
                _: crate::ActiveTurn,
                _: Self::Event,
            ) -> behavior::BehaviorActed<Self> {
                Ok(behavior::Actions::cont())
            }
        }
        assert_behavior::<behavior::MailAddr, u8, Reply>();
    }
}
