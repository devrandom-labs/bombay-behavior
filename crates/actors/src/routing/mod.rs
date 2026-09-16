//! Deterministic recipient selection and delivery-policy behaviors.
//!
//! Routing policies in this module select typed [`behavior::Recipient`]
//! values. Endpoint resolution, mailbox admission, delivery, and physical
//! backpressure remain runtime capabilities.

use behavior::{InterpretSends, Interpretation, SendEffects, SendSettlements};

/// Ordered target deliveries followed by ordered factual outcomes.
///
/// Routing templates use this product when these are their complete and
/// semantically distinct effect lanes. Interpretation always exhausts
/// `deliveries` before beginning `outcomes`.
pub struct DeliveryOutcomes<Deliveries, OutcomeSends> {
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

impl<Deliveries, OutcomeSends> behavior::ClassifySettlement
    for DeliveryOutcomes<Deliveries, OutcomeSends>
where
    Deliveries: behavior::ClassifySettlement,
    OutcomeSends: behavior::ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        self.deliveries
            .settlement_status()
            .combine(self.outcomes.settlement_status())
    }
}

impl<Deliveries, OutcomeSends> SendSettlements for DeliveryOutcomes<Deliveries, OutcomeSends>
where
    Deliveries: SendSettlements,
    OutcomeSends: SendSettlements,
{
    type Settlements = DeliveryOutcomes<Deliveries::Settlements, OutcomeSends::Settlements>;

    fn unattempted(self) -> Self::Settlements {
        DeliveryOutcomes {
            deliveries: self.deliveries.unattempted(),
            outcomes: self.outcomes.unattempted(),
        }
    }
}

impl<Host, RootEvent, Deliveries, OutcomeSends> behavior::SourceSettlementCustody<Host, RootEvent>
    for DeliveryOutcomes<Deliveries, OutcomeSends>
where
    Host: Send,
    Deliveries: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    OutcomeSends: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
{
    fn offer_next_to_source(
        self,
        host: &mut Host,
    ) -> impl core::future::Future<Output = behavior::SourceCustody<Self>> + Send {
        async move {
            (self.deliveries, self.outcomes)
                .offer_next_to_source(host)
                .await
                .map(|(deliveries, outcomes)| DeliveryOutcomes {
                    deliveries,
                    outcomes,
                })
        }
    }
}

impl<I, RootEvent, Path, Deliveries, OutcomeSends> InterpretSends<I, RootEvent, Path>
    for DeliveryOutcomes<Deliveries, OutcomeSends>
where
    I: Send,
    Deliveries: SendEffects + InterpretSends<I, RootEvent, Path>,
    OutcomeSends: SendEffects + InterpretSends<I, RootEvent, Path>,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Interpretation<Self::Settlements>> + Send {
        async move {
            behavior::settle_in_order(self.deliveries, self.outcomes, interpreter)
                .await
                .map(|(deliveries, outcomes)| DeliveryOutcomes {
                    deliveries,
                    outcomes,
                })
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
