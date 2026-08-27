//! Deterministic recipient selection and delivery-policy behaviors.
//!
//! Routing policies in this module select typed [`behavior::Recipient`]
//! values. Endpoint resolution, mailbox admission, delivery, and physical
//! backpressure remain runtime capabilities.

use behavior::{InterpretSends, SendEffects, SendInterpreter};

/// Ordered target deliveries followed by ordered factual outcomes.
///
/// Routing templates use this product when these are their complete and
/// semantically distinct effect lanes. Interpretation always exhausts
/// `deliveries` before beginning `outcomes`.
pub struct DeliveryOutcomes<Deliveries: SendEffects, OutcomeSends: SendEffects> {
    pub deliveries: Deliveries,
    pub outcomes: OutcomeSends,
}

impl<Deliveries: SendEffects, OutcomeSends: SendEffects> SendEffects
    for DeliveryOutcomes<Deliveries, OutcomeSends>
{
    fn empty() -> Self {
        Self {
            deliveries: Deliveries::empty(),
            outcomes: OutcomeSends::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.deliveries.append(other.deliveries);
        self.outcomes.append(other.outcomes);
    }
}

impl<Event, Deliveries, OutcomeSends> behavior::SendsFor<Event>
    for DeliveryOutcomes<Deliveries, OutcomeSends>
where
    Deliveries: SendEffects + behavior::SendsFor<Event>,
    OutcomeSends: SendEffects + behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, Deliveries, OutcomeSends> InterpretSends<I, RootEvent, Path>
    for DeliveryOutcomes<Deliveries, OutcomeSends>
where
    I: SendInterpreter,
    Deliveries: SendEffects + InterpretSends<I, RootEvent, Path>,
    OutcomeSends: SendEffects + InterpretSends<I, RootEvent, Path>,
    DeliveryOutcomes<Deliveries, OutcomeSends>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            self.deliveries.interpret(interpreter).await?;
            self.outcomes.interpret(interpreter).await
        }
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
    AcknowledgementError, AcknowledgementInput, AcknowledgementMessage, AcknowledgementOutcome,
    AcknowledgementRecord, AcknowledgementState, Acknowledgements,
};
pub use buffer::{
    Buffer, BufferConfigError, BufferConfiguration, BufferMessage, BufferOutcome, BufferRejection,
    BufferSends, BufferState, Buffered, OverflowPolicy,
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
    ConsistentHash, HashPolicyError, LeastLoaded, LeastLoadedError, Load, LoadEvidence,
    LoadObservation, LoadVersion, MemberToken, MemberTokenEvidence, MemberTokenObservation,
    MemberTokenVersion, RendezvousHash, RoundRobin, RouteKey, Router, RouterError, RouterMessage,
    RoutingStrategy,
};
pub use sequencer::{Sequence, Sequencer, SequencerMessage, SequencerOutcome, SequencerState};
pub use work_queue::{
    WorkQueue, WorkQueueMessage, WorkQueueOutcome, WorkQueueRejection, WorkQueueSends,
    WorkQueueState,
};
