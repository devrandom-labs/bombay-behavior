//! Feature-policy products for use with versioned configuration.

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

#[cfg(test)]
mod tests {
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
}
