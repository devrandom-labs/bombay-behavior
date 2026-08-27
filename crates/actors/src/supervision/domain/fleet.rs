//! Errors in a configured fixed-fleet topology.

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FleetError<N> {
    #[error("unknown child nonce")]
    UnknownChild(N),
    #[error("duplicate child nonce")]
    DuplicateChild(N),
}
