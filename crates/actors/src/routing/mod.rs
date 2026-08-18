//! Deterministic recipient selection and delivery-policy behaviors.
//!
//! Routing policies in this module select typed [`behavior::Recipient`]
//! values. Endpoint resolution, mailbox admission, delivery, and physical
//! backpressure remain runtime capabilities.

use behavior::{Delivery, InterpretSends, Protocol, SendEffects, SendInterpreter};

/// Ordered target deliveries followed by ordered factual outcomes.
///
/// Routing templates use this product when these are their complete and
/// semantically distinct effect lanes. Interpretation always exhausts
/// `deliveries` before beginning `outcomes`.
pub struct DeliveryOutcomes<Target: Protocol, Reply: Protocol> {
    pub deliveries: Vec<Delivery<Target>>,
    pub outcomes: Vec<Delivery<Reply>>,
}

impl<Target: Protocol, Reply: Protocol> SendEffects for DeliveryOutcomes<Target, Reply> {
    fn empty() -> Self {
        Self {
            deliveries: Vec::new(),
            outcomes: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.deliveries.append(&mut other.deliveries);
        self.outcomes.append(&mut other.outcomes);
    }
}

impl<Event, Target: Protocol, Reply: Protocol> behavior::SendsFor<Event>
    for DeliveryOutcomes<Target, Reply>
{
}

impl<I, RootEvent, Path, Target, Reply> InterpretSends<I, RootEvent, Path>
    for DeliveryOutcomes<Target, Reply>
where
    I: SendInterpreter,
    Target: Protocol,
    Reply: Protocol,
    Vec<Delivery<Target>>: InterpretSends<I, RootEvent, Path>,
    Vec<Delivery<Reply>>: InterpretSends<I, RootEvent, Path>,
{
    fn interpret(self, interpreter: &mut I) -> Result<(), I::Error> {
        self.deliveries.interpret(interpreter)?;
        self.outcomes.interpret(interpreter)
    }
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
    Broadcast, ConsistentHash, HashPolicyError, LeastLoaded, LeastLoadedError, Load, LoadEvidence,
    LoadObservation, LoadVersion, MemberToken, MemberTokenEvidence, MemberTokenObservation,
    MemberTokenVersion, RendezvousHash, RoundRobin, RouteKey, Router, RouterError, RouterMessage,
    RoutingStrategy,
};
pub use sequencer::{Sequence, Sequencer, SequencerMessage, SequencerOutcome, SequencerState};
pub use work_queue::{
    WorkQueue, WorkQueueMessage, WorkQueueOutcome, WorkQueueRejection, WorkQueueSends,
    WorkQueueState,
};
