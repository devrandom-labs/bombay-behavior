//! Deterministic recipient selection and delivery-policy behaviors.
//!
//! Routing policies in this module select typed [`behavior::Recipient`]
//! values. Endpoint resolution, mailbox admission, delivery, and physical
//! backpressure remain runtime capabilities.

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
    AcknowledgementError, AcknowledgementMessage, AcknowledgementOutcome, AcknowledgementRecord,
    AcknowledgementState, Acknowledgements,
};
pub use buffer::{
    Buffer, BufferConfigError, BufferConfiguration, BufferMessage, BufferOutcome, BufferRejection,
    BufferSends, BufferState, Buffered, OverflowPolicy,
};
pub use circuit_breaker::{
    BreakerAttempt, BreakerConfigError, BreakerMessage, BreakerOutcome, BreakerPhase,
    BreakerRejection, BreakerSends, CircuitBreaker, ClosedPhase, ProbePhase,
};
pub use correlator::{
    CorrelationResult, CorrelationState, Correlator, CorrelatorError, CorrelatorMessage,
};
pub use deduplicator::{
    Deduplicator, DeduplicatorConfigError, DeduplicatorMessage, DeduplicatorOutcome,
    DeduplicatorSends, DeduplicatorState,
};
pub use order_gate::{
    OrderGate, OrderGateMessage, OrderGateOutcome, OrderGateSends, OrderGateState,
};
pub use priority_queue::{
    PriorityQueue, PriorityQueueConfigError, PriorityQueueMessage, PriorityQueueOutcome,
    PriorityQueueRejection, PriorityQueueSends, PriorityQueueState,
};
pub use rate_limiter::{
    RateLimitRejection, RateLimiter, RateLimiterConfigError, RateLimiterMessage,
    RateLimiterOutcome, RateLimiterSends, RateLimiterState, TokenCount,
};
pub use router::{
    Broadcast, ConsistentHash, HashPolicyError, LeastLoaded, LeastLoadedError, Load, LoadEvidence,
    LoadObservation, LoadVersion, MemberToken, MemberTokenEvidence, MemberTokenObservation,
    MemberTokenVersion, RendezvousHash, RoundRobin, RouteKey, Router, RouterError, RouterMessage,
    RoutingStrategy,
};
pub use sequencer::{
    Sequence, Sequencer, SequencerMessage, SequencerOutcome, SequencerSends, SequencerState,
};
pub use work_queue::{
    WorkQueue, WorkQueueMessage, WorkQueueOutcome, WorkQueueRejection, WorkQueueSends,
    WorkQueueState,
};
