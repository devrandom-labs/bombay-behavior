use behavior::{
    Address, BehaviorBase, EndpointAddress, MailAddr, MessageProtocol, Never, Protocol, Recipient,
};
use behavior_actors::atomic::FifoError;
use behavior_actors::{
    AcknowledgementMessage, AcknowledgementOutcome, Acknowledgements, Barrier, BarrierMessage,
    BarrierReleased, Cache, CacheResult, Configuration, ConfigurationMessage, ConfigurationState,
    CorrelationResult, Correlator, CorrelatorMessage, Deduplicator, DeduplicatorMessage,
    DeduplicatorOutcome, Health, HealthMessage, HealthReport, Lease, LeaseMessage, LeaseOutcome,
    Machine, OrderGate, OrderGateMessage, OrderGateOutcome, Presence, PresenceMessage,
    PresenceReply, PriorityQueue, PriorityQueueMessage, PriorityQueueOutcome, PubSub,
    PubSubMessage, Readiness, ReadinessMessage, ReadinessReport, Registry, RegistryMessage,
    RegistryResult, ReplyRoute, Resolution, Resolver, Router, RouterMessage, RoutingStrategy,
    Topic, TopicMessage, WorkQueue, WorkQueueMessage, WorkQueueOutcome, Workflow, WorkflowMessage,
    WorkflowOutcome,
};

struct Key;
struct Value;
struct Destination;
struct Publication;
struct Subscription;
struct Phase;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ExactAddr(u8);

impl Address for ExactAddr {
    type Nonce = u64;
}

impl EndpointAddress for ExactAddr {
    type Established<P>
        = u64
    where
        P: Protocol<Addr = Self>;
}

struct ExactDestination;

impl Protocol for ExactDestination {
    type Addr = ExactAddr;
    type Msg = u8;
}

struct AcceptedPayloads(Vec<u8>);

#[derive(Clone)]
struct ObservedSelection {
    accepted: Vec<u8>,
}

impl RoutingStrategy<Recipient<Destination>> for ObservedSelection {
    type Observation = AcceptedPayloads;
    type Error = Never;

    fn select(&mut self, members: &[Recipient<Destination>], message: &u8) -> Option<usize> {
        if members.is_empty() {
            return None;
        }
        if self.accepted.contains(message) {
            Some(0)
        } else {
            None
        }
    }

    fn observe(
        &mut self,
        _: &[Recipient<Destination>],
        observation: Self::Observation,
    ) -> Result<(), Self::Error> {
        self.accepted = observation.0;
        Ok(())
    }
}

impl Protocol for Destination {
    type Addr = MailAddr;
    type Msg = u8;
}

type CacheReply = Recipient<MessageProtocol<MailAddr, CacheResult<Key, Value>>>;
type CacheProtocol = Cache<MailAddr, Key, Value, CacheReply>;

type ResolverReply = Recipient<MessageProtocol<MailAddr, Resolution<Key, Destination>>>;
type ResolverProtocol = Resolver<MailAddr, Key, Destination, ResolverReply>;
type TopicProtocol = Topic<MailAddr, Publication, Subscription>;
type PubSubProtocol = PubSub<MailAddr, Key, Publication, Subscription>;
type TopicBaseProtocol =
    Topic<MailAddr, Publication, Recipient<MessageProtocol<MailAddr, Publication>>>;
type PubSubBaseProtocol =
    PubSub<MailAddr, Key, Publication, Recipient<MessageProtocol<MailAddr, Publication>>>;
type MachineProtocol = Machine<MailAddr, (), Publication, Phase, ()>;
type ConfigurationReply = Recipient<MessageProtocol<MailAddr, ConfigurationState<Value>>>;
type ConfigurationProtocol = Configuration<MailAddr, Value, ConfigurationReply>;
type HealthReply = Recipient<MessageProtocol<MailAddr, HealthReport<Key>>>;
type HealthProtocol = Health<MailAddr, Key, HealthReply>;
type ReadinessReply = Recipient<MessageProtocol<MailAddr, ReadinessReport<Key>>>;
type ReadinessProtocol = Readiness<MailAddr, Key, ReadinessReply>;
type RegistryReply = Recipient<MessageProtocol<MailAddr, RegistryResult<Key, Destination>>>;
type RegistryProtocol = Registry<MailAddr, Key, Destination, RegistryReply>;
type CorrelatorReply = Recipient<MessageProtocol<MailAddr, CorrelationResult<Key, Value>>>;
type CorrelatorProtocol = Correlator<MailAddr, Key, Value, CorrelatorReply>;
type AcknowledgementReply =
    Recipient<MessageProtocol<MailAddr, AcknowledgementOutcome<Key, Value>>>;
type AcknowledgementProtocol = Acknowledgements<MailAddr, Key, Value, AcknowledgementReply>;
type BarrierReply = Recipient<MessageProtocol<MailAddr, BarrierReleased>>;
type BarrierProtocol = Barrier<MailAddr, Key, BarrierReply>;
type PresenceResult = Recipient<MessageProtocol<MailAddr, PresenceReply<Key>>>;
type PresenceProtocol = Presence<MailAddr, Key, PresenceResult>;
type LeaseResult = Recipient<MessageProtocol<MailAddr, LeaseOutcome<Key>>>;
type LeaseProtocol = Lease<MailAddr, Key, LeaseResult>;
type WorkflowResult = Recipient<MessageProtocol<MailAddr, WorkflowOutcome<Key>>>;
type WorkflowProtocol = Workflow<MailAddr, Key, WorkflowResult>;
type RouterProtocol = Router<MailAddr, Recipient<Destination>, ObservedSelection>;
type WorkQueueWorker = ReplyRoute<ExactDestination>;
type WorkQueueReply = Recipient<MessageProtocol<ExactAddr, WorkQueueOutcome<u8>>>;
type WorkQueueProtocol = WorkQueue<ExactAddr, u8, WorkQueueWorker, WorkQueueReply>;
type OrderGateReply = Recipient<MessageProtocol<MailAddr, OrderGateOutcome<Key, u8>>>;
type OrderGateProtocol = OrderGate<MailAddr, Key, u8, Recipient<Destination>, OrderGateReply>;
type DeduplicatorReply = Recipient<MessageProtocol<MailAddr, DeduplicatorOutcome<Key, u8>>>;
type DeduplicatorProtocol =
    Deduplicator<MailAddr, Key, u8, Recipient<Destination>, DeduplicatorReply>;
type PriorityReply = Recipient<MessageProtocol<MailAddr, PriorityQueueOutcome<u8, Value>>>;
type PriorityProtocol = PriorityQueue<MailAddr, u8, Value, Recipient<Destination>, PriorityReply>;

fn accepts_protocol<P: Protocol<Addr = MailAddr>>() {}

fn accepts_base<B: BehaviorBase<Base = B>>() {}

fn accepts_message<P: Protocol<Addr = MailAddr, Msg = M>, M>() {}

fn accepts_message_at<A: Address, P: Protocol<Addr = A, Msg = M>, M>() {}

#[test]
fn protocol_identity_does_not_require_transition_or_construction_bounds() {
    accepts_protocol::<CacheProtocol>();
    accepts_protocol::<ResolverProtocol>();
    accepts_message::<TopicProtocol, TopicMessage<Publication, Subscription>>();
    accepts_message::<PubSubProtocol, PubSubMessage<Key, Publication, Subscription>>();
    accepts_message::<MachineProtocol, Publication>();
    accepts_message::<ConfigurationProtocol, ConfigurationMessage<Value, ConfigurationReply>>();
    accepts_message::<HealthProtocol, HealthMessage<Key, HealthReply>>();
    accepts_message::<ReadinessProtocol, ReadinessMessage<Key, ReadinessReply>>();
    accepts_message::<RegistryProtocol, RegistryMessage<Key, Destination, RegistryReply>>();
    accepts_message::<CorrelatorProtocol, CorrelatorMessage<Key, Value, CorrelatorReply>>();
    accepts_message::<
        AcknowledgementProtocol,
        AcknowledgementMessage<Key, Value, AcknowledgementReply>,
    >();
    accepts_message::<BarrierProtocol, BarrierMessage<Key, BarrierReply>>();
    accepts_message::<PresenceProtocol, PresenceMessage<Key, PresenceResult>>();
    accepts_message::<LeaseProtocol, LeaseMessage<Key, LeaseResult>>();
    accepts_message::<WorkflowProtocol, WorkflowMessage<Key, WorkflowResult>>();
    accepts_message::<RouterProtocol, RouterMessage<Recipient<Destination>, ObservedSelection>>();
    accepts_message::<
        DeduplicatorProtocol,
        DeduplicatorMessage<Key, u8, Recipient<Destination>, DeduplicatorReply>,
    >();
    accepts_message::<
        OrderGateProtocol,
        OrderGateMessage<Key, u8, Recipient<Destination>, OrderGateReply>,
    >();
    accepts_message::<
        PriorityProtocol,
        PriorityQueueMessage<u8, Value, Recipient<Destination>, PriorityReply>,
    >();
    accepts_message_at::<
        ExactAddr,
        WorkQueueProtocol,
        WorkQueueMessage<u8, WorkQueueWorker, WorkQueueReply>,
    >();
}

#[test]
fn base_projection_does_not_require_transition_or_construction_bounds() {
    accepts_base::<AcknowledgementProtocol>();
    accepts_base::<ResolverProtocol>();
    accepts_base::<ConfigurationProtocol>();
    accepts_base::<ReadinessProtocol>();
    accepts_base::<MachineProtocol>();
    accepts_base::<TopicBaseProtocol>();
    accepts_base::<PubSubBaseProtocol>();
    accepts_base::<PresenceProtocol>();
    accepts_base::<LeaseProtocol>();
    accepts_base::<BarrierProtocol>();
    accepts_base::<WorkflowProtocol>();
    accepts_base::<OrderGateProtocol>();
    accepts_base::<DeduplicatorProtocol>();
    accepts_base::<PriorityProtocol>();
}

#[test]
fn fifo_transition_failures_are_nameable_and_exhaustive_to_callers() {
    fn classify(error: FifoError) -> &'static str {
        match error {
            FifoError::InitializationUnavailable => "initialization already advanced",
            FifoError::WorkerCreationsExhausted => "creation identifiers exhausted",
        }
    }

    assert_eq!(
        classify(FifoError::InitializationUnavailable),
        "initialization already advanced"
    );
    assert_eq!(
        classify(FifoError::WorkerCreationsExhausted),
        "creation identifiers exhausted"
    );
}
