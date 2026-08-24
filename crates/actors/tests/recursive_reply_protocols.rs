//! Compile-time regression matrix for reply adapters that target their sender's root.

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

macro_rules! recursive_reply_case {
    ($module:ident, $input:ty, $subject:ty) => {
        mod $module {
            use super::*;

            struct Root;
            type Reply = MessageAdapter<$input, Root>;
            type Subject = $subject;

            impl Protocol for Root {
                type Addr = MailAddr;
                type Msg = ();
            }

            impl Behavior for Root {
                type Protocol = Self;
                type Event = User<MailAddr, ()>;
                type Sends = Vec<Delivery<Subject>>;
                type Ph = Never;
                type Error = Never;
                type Birth = NoBirths;

                fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }

            fn adapt(_: $input) {}

            #[test]
            fn root_and_reply_template_form_a_finite_trait_proof() {
                fn assert_behavior<B: Behavior>() {}
                assert_behavior::<StopOnShutdown<Root>>();
                assert_behavior::<Subject>();
                let root = Recipient::<Root>::global(MailAddr(1));
                let _: Reply = MessageAdapter::new(root, adapt);
            }
        }
    };
}

recursive_reply_case!(
    acknowledgements,
    AcknowledgementOutcome<u8, u16>,
    Acknowledgements<MailAddr, u8, u16, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    buffer,
    BufferOutcome<u8>,
    Buffer<MailAddr, u8, Recipient<MessageProtocol<MailAddr, BufferOutcome<u8>>>>
);
recursive_reply_case!(
    cache,
    CacheResult<u8, u16>,
    Cache<
        MailAddr,
        u8,
        u16,
        Recipient<MessageProtocol<MailAddr, CacheResult<u8, u16>>>
    >
);
recursive_reply_case!(
    circuit_breaker,
    BreakerOutcome,
    CircuitBreaker<MailAddr, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    configuration,
    ConfigurationState<u8>,
    Configuration<MailAddr, u8, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    correlator,
    CorrelationResult<u8, u16>,
    Correlator<MailAddr, u8, u16, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    deduplicator,
    DeduplicatorOutcome<u8, u8>,
    Deduplicator<MailAddr, u8, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(health, HealthReport<u8>, Health<MailAddr, u8, Reply, Recipient<Reply>>);
recursive_reply_case!(lease, LeaseOutcome<u8>, Lease<MailAddr, u8, Reply, Recipient<Reply>>);
recursive_reply_case!(
    order_gate,
    OrderGateOutcome<u8, u8>,
    OrderGate<MailAddr, u8, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    presence,
    PresenceReply<u8>,
    Presence<MailAddr, u8, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    priority_queue,
    PriorityQueueOutcome<u8, u8>,
    PriorityQueue<MailAddr, u8, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    rate_limiter,
    RateLimiterOutcome<u8>,
    RateLimiter<MailAddr, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    readiness,
    ReadinessReport<u8>,
    Readiness<MailAddr, u8, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    registry,
    RegistryResult<u8, Target>,
    Registry<MailAddr, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    resolver,
    Resolution<u8, Target>,
    Resolver<MailAddr, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    sequencer,
    SequencerOutcome<u8>,
    Sequencer<MailAddr, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(task, TaskResult<u8>, Task<MailAddr, u8, Reply, Recipient<Reply>>);
recursive_reply_case!(
    work_queue,
    WorkQueueOutcome<u8>,
    WorkQueue<MailAddr, u8, Target, Reply, Recipient<Reply>>
);
recursive_reply_case!(
    workflow,
    WorkflowOutcome<u8>,
    Workflow<MailAddr, u8, Reply, Recipient<Reply>>
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

    assert_behavior::<Acknowledgements<MailAddr, u8, u16, AckReply, Recipient<AckReply>>>();
    assert_behavior::<Buffer<MailAddr, u8, Recipient<BufferReply>>>();
    assert_behavior::<Cache<MailAddr, u8, u16, Recipient<CacheReply>>>();
    assert_behavior::<CircuitBreaker<MailAddr, BreakerReply, Recipient<BreakerReply>>>();
    assert_behavior::<Configuration<MailAddr, u8, ConfigurationReply, Recipient<ConfigurationReply>>>(
    );
    assert_behavior::<Correlator<MailAddr, u8, u16, CorrelatorReply, Recipient<CorrelatorReply>>>();
    assert_behavior::<
        Deduplicator<MailAddr, u8, u8, Target, DeduplicatorReply, Recipient<DeduplicatorReply>>,
    >();
    assert_behavior::<Health<MailAddr, u8, HealthReply, Recipient<HealthReply>>>();
    assert_behavior::<Lease<MailAddr, u8, LeaseReply, Recipient<LeaseReply>>>();
    assert_behavior::<OrderGate<MailAddr, u8, u8, Target, GateReply, Recipient<GateReply>>>();
    assert_behavior::<
        Presence<MailAddr, u8, PresenceReplyProtocol, Recipient<PresenceReplyProtocol>>,
    >();
    assert_behavior::<
        PriorityQueue<MailAddr, u8, u8, Target, PriorityReply, Recipient<PriorityReply>>,
    >();
    assert_behavior::<RateLimiter<MailAddr, u8, Target, RateReply, Recipient<RateReply>>>();
    assert_behavior::<Readiness<MailAddr, u8, ReadinessReply, Recipient<ReadinessReply>>>();
    assert_behavior::<Registry<MailAddr, u8, Target, RegistryReply, Recipient<RegistryReply>>>();
    assert_behavior::<Resolver<MailAddr, u8, Target, ResolverReply, Recipient<ResolverReply>>>();
    assert_behavior::<Sequencer<MailAddr, u8, Target, SequencerReply, Recipient<SequencerReply>>>();
    assert_behavior::<Task<MailAddr, u8, TaskReply, Recipient<TaskReply>>>();
    assert_behavior::<WorkQueue<MailAddr, u8, Target, QueueReply, Recipient<QueueReply>>>();
    assert_behavior::<Workflow<MailAddr, u8, WorkflowReply, Recipient<WorkflowReply>>>();
}

#[test]
fn every_send_only_destination_accepts_a_protocol_without_a_behavior() {
    fn assert_behavior<B: Behavior>() {}

    type Bytes = MessageProtocol<MailAddr, u8>;
    type GateReply = MessageProtocol<MailAddr, OrderGateOutcome<u8, u8>>;
    type PriorityReply = MessageProtocol<MailAddr, PriorityQueueOutcome<u8, u8>>;
    type QueueReply = MessageProtocol<MailAddr, WorkQueueOutcome<u8>>;
    type RateReply = MessageProtocol<MailAddr, RateLimiterOutcome<u8>>;
    type SequenceReply = MessageProtocol<MailAddr, SequencerOutcome<u8>>;
    type DedupReply = MessageProtocol<MailAddr, DeduplicatorOutcome<u8, u8>>;
    type RegistryReply = MessageProtocol<MailAddr, RegistryResult<u8, Bytes>>;
    type ResolverReply = MessageProtocol<MailAddr, Resolution<u8, Bytes>>;
    type BufferReply = MessageProtocol<MailAddr, BufferOutcome<u8>>;
    type BarrierReply = MessageProtocol<MailAddr, BarrierReleased>;
    type LatchReply = MessageProtocol<MailAddr, LatchReleased>;

    assert_behavior::<Buffer<MailAddr, u8, Recipient<BufferReply>>>();
    assert_behavior::<OrderGate<MailAddr, u8, u8, Bytes, GateReply, Recipient<GateReply>>>();
    assert_behavior::<
        PriorityQueue<MailAddr, u8, u8, Bytes, PriorityReply, Recipient<PriorityReply>>,
    >();
    assert_behavior::<WorkQueue<MailAddr, u8, Bytes, QueueReply, Recipient<QueueReply>>>();
    assert_behavior::<RateLimiter<MailAddr, u8, Bytes, RateReply, Recipient<RateReply>>>();
    assert_behavior::<Sequencer<MailAddr, u8, Bytes, SequenceReply, Recipient<SequenceReply>>>();
    assert_behavior::<Deduplicator<MailAddr, u8, u8, Bytes, DedupReply, Recipient<DedupReply>>>();
    assert_behavior::<Router<MailAddr, Bytes, RoundRobin>>();
    assert_behavior::<Topic<MailAddr, u8>>();
    assert_behavior::<PubSub<MailAddr, u8, u8, Bytes>>();
    assert_behavior::<Registry<MailAddr, u8, Bytes, RegistryReply, Recipient<RegistryReply>>>();
    assert_behavior::<Resolver<MailAddr, u8, Bytes, ResolverReply, Recipient<ResolverReply>>>();
    assert_behavior::<Barrier<MailAddr, u8, Recipient<BarrierReply>>>();
    assert_behavior::<Latch<MailAddr, Recipient<LatchReply>>>();
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
