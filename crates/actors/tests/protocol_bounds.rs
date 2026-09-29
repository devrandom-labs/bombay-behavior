use behavior::{MailAddr, MessageProtocol, Protocol, Recipient};
use behavior_actors::atomic::FifoError;
use behavior_actors::{Cache, CacheResult, Resolution, Resolver};

struct Key;
struct Value;
struct Destination;

impl Protocol for Destination {
    type Addr = MailAddr;
    type Msg = u8;
}

type CacheReply = Recipient<MessageProtocol<MailAddr, CacheResult<Key, Value>>>;
type CacheProtocol = Cache<MailAddr, Key, Value, CacheReply>;

type ResolverReply = Recipient<MessageProtocol<MailAddr, Resolution<Key, Destination>>>;
type ResolverProtocol = Resolver<MailAddr, Key, Destination, ResolverReply>;

fn accepts_protocol<P: Protocol<Addr = MailAddr>>() {}

#[test]
fn protocol_identity_does_not_require_transition_or_construction_bounds() {
    accepts_protocol::<CacheProtocol>();
    accepts_protocol::<ResolverProtocol>();
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
