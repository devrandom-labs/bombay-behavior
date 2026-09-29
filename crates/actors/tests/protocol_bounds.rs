use behavior::{MailAddr, MessageProtocol, Protocol, Recipient};
use behavior_actors::atomic::FifoError;
use behavior_actors::{
    Cache, CacheResult, Configuration, ConfigurationMessage, ConfigurationState, Health,
    HealthMessage, HealthReport, Machine, PubSub, PubSubMessage, Readiness, ReadinessMessage,
    ReadinessReport, Registry, RegistryMessage, RegistryResult, Resolution, Resolver, Topic,
    TopicMessage,
};

struct Key;
struct Value;
struct Destination;
struct Publication;
struct Subscription;
struct Phase;

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
type MachineProtocol = Machine<MailAddr, (), Publication, Phase, ()>;
type ConfigurationReply = Recipient<MessageProtocol<MailAddr, ConfigurationState<Value>>>;
type ConfigurationProtocol = Configuration<MailAddr, Value, ConfigurationReply>;
type HealthReply = Recipient<MessageProtocol<MailAddr, HealthReport<Key>>>;
type HealthProtocol = Health<MailAddr, Key, HealthReply>;
type ReadinessReply = Recipient<MessageProtocol<MailAddr, ReadinessReport<Key>>>;
type ReadinessProtocol = Readiness<MailAddr, Key, ReadinessReply>;
type RegistryReply = Recipient<MessageProtocol<MailAddr, RegistryResult<Key, Destination>>>;
type RegistryProtocol = Registry<MailAddr, Key, Destination, RegistryReply>;

fn accepts_protocol<P: Protocol<Addr = MailAddr>>() {}

fn accepts_message<P: Protocol<Addr = MailAddr, Msg = M>, M>() {}

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
