//! Compile-time regression matrix for reply adapters that target their sender's root.

use behavior::*;
use behavior_actors::*;

struct Target;

impl Protocol for Target {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Target {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct NominalBytes;
struct NominalBufferReply;
struct NominalCacheReply;
struct NominalBarrierReply;
struct NominalLatchReply;

impl Protocol for NominalBytes {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Protocol for NominalBufferReply {
    type Addr = MailAddr;
    type Msg = BufferOutcome<u8>;
}

impl Protocol for NominalCacheReply {
    type Addr = MailAddr;
    type Msg = CacheResult<u8, u16>;
}

impl Protocol for NominalBarrierReply {
    type Addr = MailAddr;
    type Msg = BarrierReleased;
}

impl Protocol for NominalLatchReply {
    type Addr = MailAddr;
    type Msg = LatchReleased;
}

macro_rules! recursive_reply_case {
    ($module:ident, $input:ty, $subject:ty) => {
        mod $module {
            struct Root;
            type Reply = super::MessageAdapter<$input, Root>;
            type Subject = $subject;

            impl super::Protocol for Root {
                type Addr = super::MailAddr;
                type Msg = ();
            }

            impl super::Behavior for Root {
                type Protocol = Self;
                type Event = super::User<super::MailAddr, ()>;
                type Sends = Vec<super::Delivery<Subject>>;
                type Ph = super::Never;
                type Error = super::Never;
                type Birth = super::NoBirths;

                fn transition(
                    &mut self,
                    _: super::ActiveTurn,
                    _: Self::Event,
                ) -> super::BehaviorActed<Self> {
                    Ok(super::Actions::cont())
                }
            }

            fn adapt(_: $input) {}

            #[test]
            fn root_and_reply_template_form_a_finite_trait_proof() {
                fn assert_behavior<B: super::Behavior>() {}
                assert_behavior::<super::StopOnShutdown<Root>>();
                assert_behavior::<Subject>();
                let root = super::Recipient::<Root>::global(super::MailAddr(1));
                let reply: Reply = super::MessageAdapter::new(root, adapt);
                assert_eq!(reply.destination().address(), super::MailAddr(1));
            }
        }
    };
}

recursive_reply_case!(
    acknowledgements,
    super::AcknowledgementOutcome<u8, u16>,
    super::Acknowledgements<super::MailAddr, u8, u16, super::Recipient<Reply>>
);
recursive_reply_case!(
    buffer,
    super::BufferOutcome<u8>,
    super::Buffer<
        super::MailAddr,
        u8,
        super::Recipient<super::MessageProtocol<super::MailAddr, u8>>,
        super::Recipient<
            super::MessageProtocol<super::MailAddr, super::BufferOutcome<u8>>
        >
    >
);
recursive_reply_case!(
    cache,
    super::CacheResult<u8, u16>,
    super::Cache<
        super::MailAddr,
        u8,
        u16,
        super::Recipient<
            super::MessageProtocol<super::MailAddr, super::CacheResult<u8, u16>>
        >
    >
);
recursive_reply_case!(
    circuit_breaker,
    super::BreakerOutcome,
    super::CircuitBreaker<super::MailAddr, super::Recipient<Reply>>
);
recursive_reply_case!(
    configuration,
    super::ConfigurationState<u8>,
    super::Configuration<super::MailAddr, u8, super::Recipient<Reply>>
);
recursive_reply_case!(
    correlator,
    super::CorrelationResult<u8, u16>,
    super::Correlator<super::MailAddr, u8, u16, super::Recipient<Reply>>
);
recursive_reply_case!(
    deduplicator,
    super::DeduplicatorOutcome<u8, u8>,
    super::Deduplicator<
        super::MailAddr,
        u8,
        u8,
        super::Recipient<super::Target>,
        super::Recipient<Reply>
    >
);
recursive_reply_case!(
    health,
    super::HealthReport<u8>,
    super::Health<super::MailAddr, u8, super::Recipient<Reply>>
);
recursive_reply_case!(
    lease,
    super::LeaseOutcome<u8>,
    super::Lease<super::MailAddr, u8, super::Recipient<Reply>>
);
recursive_reply_case!(
    order_gate,
    super::OrderGateOutcome<u8, u8>,
    super::OrderGate<
        super::MailAddr,
        u8,
        u8,
        super::Recipient<super::Target>,
        super::Recipient<Reply>
    >
);
recursive_reply_case!(
    presence,
    super::PresenceReply<u8>,
    super::Presence<super::MailAddr, u8, super::Recipient<Reply>>
);
recursive_reply_case!(
    priority_queue,
    super::PriorityQueueOutcome<u8, u8>,
    super::PriorityQueue<
        super::MailAddr,
        u8,
        u8,
        super::Recipient<super::Target>,
        super::Recipient<Reply>
    >
);
recursive_reply_case!(
    rate_limiter,
    super::RateLimiterOutcome<u8>,
    super::RateLimiter<
        super::MailAddr,
        u8,
        super::Recipient<super::Target>,
        super::Recipient<Reply>
    >
);
recursive_reply_case!(
    readiness,
    super::ReadinessReport<u8>,
    super::Readiness<super::MailAddr, u8, super::Recipient<Reply>>
);
recursive_reply_case!(
    registry,
    super::RegistryResult<u8, super::Target>,
    super::Registry<super::MailAddr, u8, super::Target, super::Recipient<Reply>>
);
recursive_reply_case!(
    resolver,
    super::Resolution<u8, super::Target>,
    super::Resolver<super::MailAddr, u8, super::Target, super::Recipient<Reply>>
);
recursive_reply_case!(
    sequencer,
    super::SequencerOutcome<u8>,
    super::Sequencer<
        super::MailAddr,
        u8,
        super::Recipient<super::Target>,
        super::Recipient<Reply>
    >
);
recursive_reply_case!(
    task,
    super::TaskResult<u8>,
    super::Task<super::MailAddr, u8, super::Recipient<Reply>>
);
recursive_reply_case!(
    work_queue,
    super::WorkQueueOutcome<u8>,
    super::WorkQueue<
        super::MailAddr,
        u8,
        super::Recipient<super::Target>,
        super::Recipient<Reply>
    >
);
recursive_reply_case!(
    workflow,
    super::WorkflowOutcome<u8>,
    super::Workflow<super::MailAddr, u8, super::Recipient<Reply>>
);

#[test]
fn every_reply_template_accepts_a_pure_message_protocol() {
    fn assert_behavior<B: Behavior>() {}

    type AckReply = MessageProtocol<MailAddr, AcknowledgementOutcome<u8, u16>>;
    type BufferReply = MessageProtocol<MailAddr, BufferOutcome<u8>>;
    type CacheReply = MessageProtocol<MailAddr, CacheResult<u8, u16>>;
    type BreakerReply = MessageProtocol<MailAddr, BreakerOutcome>;
    type ConfigurationReply = MessageProtocol<MailAddr, ConfigurationState<u8>>;
    type CorrelatorReply = MessageProtocol<MailAddr, CorrelationResult<u8, u16>>;
    type DeduplicatorReply = MessageProtocol<MailAddr, DeduplicatorOutcome<u8, u8>>;
    type HealthReply = MessageProtocol<MailAddr, HealthReport<u8>>;
    type LeaseReply = MessageProtocol<MailAddr, LeaseOutcome<u8>>;
    type GateReply = MessageProtocol<MailAddr, OrderGateOutcome<u8, u8>>;
    type PresenceReplyProtocol = MessageProtocol<MailAddr, PresenceReply<u8>>;
    type PriorityReply = MessageProtocol<MailAddr, PriorityQueueOutcome<u8, u8>>;
    type RateReply = MessageProtocol<MailAddr, RateLimiterOutcome<u8>>;
    type ReadinessReply = MessageProtocol<MailAddr, ReadinessReport<u8>>;
    type RegistryReply = MessageProtocol<MailAddr, RegistryResult<u8, Target>>;
    type ResolverReply = MessageProtocol<MailAddr, Resolution<u8, Target>>;
    type SequencerReply = MessageProtocol<MailAddr, SequencerOutcome<u8>>;
    type TaskReply = MessageProtocol<MailAddr, TaskResult<u8>>;
    type QueueReply = MessageProtocol<MailAddr, WorkQueueOutcome<u8>>;
    type WorkflowReply = MessageProtocol<MailAddr, WorkflowOutcome<u8>>;

    assert_behavior::<Acknowledgements<MailAddr, u8, u16, Recipient<AckReply>>>();
    assert_behavior::<
        Buffer<MailAddr, u8, Recipient<MessageProtocol<MailAddr, u8>>, Recipient<BufferReply>>,
    >();
    assert_behavior::<Cache<MailAddr, u8, u16, Recipient<CacheReply>>>();
    assert_behavior::<CircuitBreaker<MailAddr, Recipient<BreakerReply>>>();
    assert_behavior::<Configuration<MailAddr, u8, Recipient<ConfigurationReply>>>();
    assert_behavior::<Correlator<MailAddr, u8, u16, Recipient<CorrelatorReply>>>();
    assert_behavior::<
        Deduplicator<MailAddr, u8, u8, Recipient<Target>, Recipient<DeduplicatorReply>>,
    >();
    assert_behavior::<Health<MailAddr, u8, Recipient<HealthReply>>>();
    assert_behavior::<Lease<MailAddr, u8, Recipient<LeaseReply>>>();
    assert_behavior::<OrderGate<MailAddr, u8, u8, Recipient<Target>, Recipient<GateReply>>>();
    assert_behavior::<Presence<MailAddr, u8, Recipient<PresenceReplyProtocol>>>();
    assert_behavior::<PriorityQueue<MailAddr, u8, u8, Recipient<Target>, Recipient<PriorityReply>>>(
    );
    assert_behavior::<RateLimiter<MailAddr, u8, Recipient<Target>, Recipient<RateReply>>>();
    assert_behavior::<Readiness<MailAddr, u8, Recipient<ReadinessReply>>>();
    assert_behavior::<Registry<MailAddr, u8, Target, Recipient<RegistryReply>>>();
    assert_behavior::<Resolver<MailAddr, u8, Target, Recipient<ResolverReply>>>();
    assert_behavior::<Sequencer<MailAddr, u8, Recipient<Target>, Recipient<SequencerReply>>>();
    assert_behavior::<Task<MailAddr, u8, Recipient<TaskReply>>>();
    assert_behavior::<WorkQueue<MailAddr, u8, Recipient<Target>, Recipient<QueueReply>>>();
    assert_behavior::<Workflow<MailAddr, u8, Recipient<WorkflowReply>>>();
}

#[test]
fn every_send_only_destination_accepts_an_ordinary_nominal_protocol() {
    fn assert_behavior<B: Behavior>() {}

    type Bytes = NominalBytes;
    type GateReply = MessageProtocol<MailAddr, OrderGateOutcome<u8, u8>>;
    type PriorityReply = MessageProtocol<MailAddr, PriorityQueueOutcome<u8, u8>>;
    type QueueReply = MessageProtocol<MailAddr, WorkQueueOutcome<u8>>;
    type RateReply = MessageProtocol<MailAddr, RateLimiterOutcome<u8>>;
    type SequenceReply = MessageProtocol<MailAddr, SequencerOutcome<u8>>;
    type DedupReply = MessageProtocol<MailAddr, DeduplicatorOutcome<u8, u8>>;
    type RegistryReply = MessageProtocol<MailAddr, RegistryResult<u8, Bytes>>;
    type ResolverReply = MessageProtocol<MailAddr, Resolution<u8, Bytes>>;
    type BufferReply = NominalBufferReply;
    type CacheReply = NominalCacheReply;
    type BarrierReply = NominalBarrierReply;
    type LatchReply = NominalLatchReply;

    assert_behavior::<Buffer<MailAddr, u8, Recipient<Bytes>, Recipient<BufferReply>>>();
    assert_behavior::<Cache<MailAddr, u8, u16, Recipient<CacheReply>>>();
    assert_behavior::<OrderGate<MailAddr, u8, u8, Recipient<Bytes>, Recipient<GateReply>>>();
    assert_behavior::<PriorityQueue<MailAddr, u8, u8, Recipient<Bytes>, Recipient<PriorityReply>>>(
    );
    assert_behavior::<WorkQueue<MailAddr, u8, Recipient<Bytes>, Recipient<QueueReply>>>();
    assert_behavior::<RateLimiter<MailAddr, u8, Recipient<Bytes>, Recipient<RateReply>>>();
    assert_behavior::<Sequencer<MailAddr, u8, Recipient<Bytes>, Recipient<SequenceReply>>>();
    assert_behavior::<Deduplicator<MailAddr, u8, u8, Recipient<Bytes>, Recipient<DedupReply>>>();
    assert_behavior::<Router<MailAddr, Recipient<Bytes>, RoundRobin>>();
    assert_behavior::<Topic<MailAddr, u8, Recipient<Bytes>>>();
    assert_behavior::<PubSub<MailAddr, u8, u8, Recipient<Bytes>>>();
    assert_behavior::<Registry<MailAddr, u8, Bytes, Recipient<RegistryReply>>>();
    assert_behavior::<Resolver<MailAddr, u8, Bytes, Recipient<ResolverReply>>>();
    assert_behavior::<Barrier<MailAddr, u8, Recipient<BarrierReply>>>();
    assert_behavior::<Latch<MailAddr, Recipient<LatchReply>>>();
}

#[test]
fn standalone_templates_host_the_same_nominal_capability_users_hold() {
    fn nominal<B: Behavior<Protocol = B>>() {}
    fn transparent<B, W>(_: &W)
    where
        B: Behavior,
        W: Behavior<Protocol = B>,
    {
    }

    type CacheActor = Cache<MailAddr, u8, u16, Recipient<NominalCacheReply>>;
    type BarrierActor = Barrier<MailAddr, u8, Recipient<NominalBarrierReply>>;
    type LatchActor = Latch<MailAddr, Recipient<NominalLatchReply>>;

    nominal::<CacheActor>();
    nominal::<BarrierActor>();
    nominal::<LatchActor>();

    let cache_capability = Recipient::<CacheActor>::global(MailAddr(41));
    let barrier_capability = Recipient::<BarrierActor>::global(MailAddr(42));
    let latch_capability = Recipient::<LatchActor>::global(MailAddr(43));
    assert_eq!(cache_capability.address(), MailAddr(41));
    assert_eq!(barrier_capability.address(), MailAddr(42));
    assert_eq!(latch_capability.address(), MailAddr(43));

    let cache = CacheActor::new(CacheConfiguration::new(2).unwrap());
    let barrier = BarrierActor::new(BarrierMembership::new(vec![1]).unwrap());
    let latch = LatchActor::new(1);
    transparent::<CacheActor, _>(&StopOnShutdown::new(cache));
    transparent::<BarrierActor, _>(&StopOnShutdown::new(barrier));
    transparent::<LatchActor, _>(&StopOnShutdown::new(latch));
}

#[test]
fn protocol_preserving_wrappers_keep_one_public_identity() {
    fn preserves<B, W>()
    where
        B: Behavior,
        W: Behavior<Protocol = B::Protocol>,
    {
    }

    preserves::<RootProtocolProbe, StopOnShutdown<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, Stash<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, Watch<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, ReceiveTimeout<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, Deadline<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, Periodic<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, OneShot<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, TerminationMonitor<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, StopOnShutdown<RootProtocolProbe>>();
    preserves::<RootProtocolProbe, FinalizeOnShutdown<RootProtocolProbe>>();
}

struct RootProtocolProbe;

impl Protocol for RootProtocolProbe {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for RootProtocolProbe {
    type Protocol = MessageProtocol<MailAddr, ()>;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}
