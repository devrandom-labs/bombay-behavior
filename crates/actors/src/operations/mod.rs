//! Operational policies over typed application and runtime observations.
//!
//! Exporters, probes, tracing subscribers, metrics recorders, and external
//! gateways remain System capabilities. Behaviors here only fold explicit
//! observations and emit typed replies or export requests.

mod configuration;
mod features;
mod health;
mod readiness;

pub use configuration::{
    Configuration, ConfigurationError, ConfigurationMessage, ConfigurationState,
    ConfigurationVersion,
};
pub use features::{Feature, FeatureSet, FeatureStatus, Features, FeaturesState};
pub use health::{
    ComponentHealth, ComponentHealthState, Health, HealthError, HealthEvidence, HealthMessage,
    HealthReport, HealthStatus, ObservationVersion,
};
pub use readiness::{
    DependencyReadiness, Readiness, ReadinessError, ReadinessEvidence, ReadinessMessage,
    ReadinessReport, ReadinessStatus,
};
