//! Deterministic recipient selection and delivery-policy behaviors.
//!
//! Routing policies in this module select typed [`behavior::Recipient`]
//! values. Endpoint resolution, mailbox admission, delivery, and physical
//! backpressure remain runtime capabilities.

/// Ordered target deliveries followed by ordered factual outcomes.
///
/// Routing templates use this product when these are their complete and
/// semantically distinct effect lanes. Interpretation always exhausts
/// `deliveries` before beginning `outcomes`.
#[derive(behavior_macros::SendProduct)]
pub struct DeliveryOutcomes<Deliveries, OutcomeSends> {
    pub deliveries: Deliveries,
    pub outcomes: OutcomeSends,
}

mod acknowledgements;
mod buffer;
mod circuit_breaker;
mod correlator;
mod deduplicator;
mod order_gate;
mod priority_queue;
mod rate_limiter;
mod router;
mod sequencer;
mod work_queue;

pub use acknowledgements::{
    AcknowledgementError, AcknowledgementInput, AcknowledgementMessage, AcknowledgementOutcome,
    AcknowledgementRecord, AcknowledgementState, Acknowledgements,
};
pub use buffer::{
    Buffer, BufferConfigError, BufferConfiguration, BufferMessage, BufferOutcome, BufferRejection,
    BufferState, Buffered, OverflowPolicy,
};
pub use circuit_breaker::{
    BreakerAttempt, BreakerCompletion, BreakerConfigError, BreakerError, BreakerMessage,
    BreakerOutcome, BreakerPhase, BreakerRejection, BreakerSends, CircuitBreaker, ClosedPhase,
    ProbePhase,
};
pub use correlator::{
    CorrelationResult, CorrelationState, Correlator, CorrelatorError, CorrelatorMessage,
};
pub use deduplicator::{
    Deduplicator, DeduplicatorConfigError, DeduplicatorMessage, DeduplicatorOutcome,
    DeduplicatorState,
};
pub use order_gate::{OrderGate, OrderGateMessage, OrderGateOutcome, OrderGateState};
pub use priority_queue::{
    PriorityQueue, PriorityQueueConfigError, PriorityQueueMessage, PriorityQueueOutcome,
    PriorityQueueRejection, PriorityQueueState,
};
pub use rate_limiter::{
    RateLimitRejection, RateLimiter, RateLimiterConfigError, RateLimiterMessage,
    RateLimiterOutcome, RateLimiterState, TokenCount,
};
pub use router::{
    ConsistentHash, LeastLoaded, Load, LoadEvidence, LoadObservation, LoadVersion,
    MemberEvidenceError, MemberToken, MemberTokenEvidence, MemberTokenObservation,
    MemberTokenVersion, RendezvousHash, RoundRobin, RouteKey, Router, RouterError, RouterMessage,
    RoutingObservationRejection, RoutingStrategy,
};
pub use sequencer::{Sequence, Sequencer, SequencerMessage, SequencerOutcome, SequencerState};
pub use work_queue::{
    WorkQueue, WorkQueueMessage, WorkQueueOutcome, WorkQueueRejection, WorkQueueSends,
    WorkQueueState,
};
