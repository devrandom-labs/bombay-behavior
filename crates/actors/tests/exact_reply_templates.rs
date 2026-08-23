//! Compile-contract matrix for every customer-passing actor template.

use behavior_actors::*;
use core::marker::PhantomData;

#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P>(PhantomData<fn() -> P>);

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct Target;

impl Protocol for Target {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

fn assert_behavior<B: Behavior>() {}

macro_rules! assert_both_routes {
    ($reply:ty, $exact:ty, $mixed:ty) => {{
        fn assert_reply<P: Protocol<Addr = RuntimeAddr>>() {}
        assert_reply::<$reply>();
        assert_behavior::<$exact>();
        assert_behavior::<$mixed>();
    }};
}

#[test]
fn every_non_owning_customer_seam_accepts_exact_and_mixed_capabilities() {
    type Ack = MessageProtocol<RuntimeAddr, AcknowledgementOutcome<u8, u16>>;
    assert_both_routes!(
        Ack,
        Acknowledgements<RuntimeAddr, u8, u16, Ack, EstablishedRecipient<Ack>>,
        Acknowledgements<RuntimeAddr, u8, u16, Ack, ReplyRoute<Ack>>
    );

    type BufferReply = MessageProtocol<RuntimeAddr, BufferOutcome<u8>>;
    assert_both_routes!(
        BufferReply,
        Buffer<RuntimeAddr, u8, EstablishedRecipient<BufferReply>>,
        Buffer<RuntimeAddr, u8, ReplyRoute<BufferReply>>
    );

    type CacheReply = MessageProtocol<RuntimeAddr, CacheResult<u8, u16>>;
    assert_both_routes!(
        CacheReply,
        Cache<RuntimeAddr, u8, u16, EstablishedRecipient<CacheReply>>,
        Cache<RuntimeAddr, u8, u16, ReplyRoute<CacheReply>>
    );

    type BreakerReply = MessageProtocol<RuntimeAddr, BreakerOutcome>;
    assert_both_routes!(
        BreakerReply,
        CircuitBreaker<RuntimeAddr, BreakerReply, EstablishedRecipient<BreakerReply>>,
        CircuitBreaker<RuntimeAddr, BreakerReply, ReplyRoute<BreakerReply>>
    );

    type ConfigurationReply = MessageProtocol<RuntimeAddr, ConfigurationState<u8>>;
    assert_both_routes!(
        ConfigurationReply,
        Configuration<RuntimeAddr, u8, ConfigurationReply, EstablishedRecipient<ConfigurationReply>>,
        Configuration<RuntimeAddr, u8, ConfigurationReply, ReplyRoute<ConfigurationReply>>
    );

    type FeaturesReply = MessageProtocol<RuntimeAddr, ConfigurationState<FeatureSet<u8>>>;
    assert_both_routes!(
        FeaturesReply,
        Features<RuntimeAddr, u8, FeaturesReply, EstablishedRecipient<FeaturesReply>>,
        Features<RuntimeAddr, u8, FeaturesReply, ReplyRoute<FeaturesReply>>
    );

    type CorrelatorReply = MessageProtocol<RuntimeAddr, CorrelationResult<u8, u16>>;
    assert_both_routes!(
        CorrelatorReply,
        Correlator<RuntimeAddr, u8, u16, CorrelatorReply, EstablishedRecipient<CorrelatorReply>>,
        Correlator<RuntimeAddr, u8, u16, CorrelatorReply, ReplyRoute<CorrelatorReply>>
    );

    type DeduplicatorReply = MessageProtocol<RuntimeAddr, DeduplicatorOutcome<u8, u8>>;
    assert_both_routes!(
        DeduplicatorReply,
        Deduplicator<RuntimeAddr, u8, u8, Target, DeduplicatorReply, EstablishedRecipient<DeduplicatorReply>>,
        Deduplicator<RuntimeAddr, u8, u8, Target, DeduplicatorReply, ReplyRoute<DeduplicatorReply>>
    );

    type HealthReply = MessageProtocol<RuntimeAddr, HealthReport<u8>>;
    assert_both_routes!(
        HealthReply,
        Health<RuntimeAddr, u8, HealthReply, EstablishedRecipient<HealthReply>>,
        Health<RuntimeAddr, u8, HealthReply, ReplyRoute<HealthReply>>
    );

    type LeaseReply = MessageProtocol<RuntimeAddr, LeaseOutcome<u8>>;
    assert_both_routes!(
        LeaseReply,
        Lease<RuntimeAddr, u8, LeaseReply, EstablishedRecipient<LeaseReply>>,
        Lease<RuntimeAddr, u8, LeaseReply, ReplyRoute<LeaseReply>>
    );

    type GateReply = MessageProtocol<RuntimeAddr, OrderGateOutcome<u8, u8>>;
    assert_both_routes!(
        GateReply,
        OrderGate<RuntimeAddr, u8, u8, Target, GateReply, EstablishedRecipient<GateReply>>,
        OrderGate<RuntimeAddr, u8, u8, Target, GateReply, ReplyRoute<GateReply>>
    );

    type PresenceReplyProtocol = MessageProtocol<RuntimeAddr, PresenceReply<u8>>;
    assert_both_routes!(
        PresenceReplyProtocol,
        Presence<RuntimeAddr, u8, PresenceReplyProtocol, EstablishedRecipient<PresenceReplyProtocol>>,
        Presence<RuntimeAddr, u8, PresenceReplyProtocol, ReplyRoute<PresenceReplyProtocol>>
    );

    type PriorityReply = MessageProtocol<RuntimeAddr, PriorityQueueOutcome<u8>>;
    assert_both_routes!(
        PriorityReply,
        PriorityQueue<RuntimeAddr, u8, u8, Target, PriorityReply, EstablishedRecipient<PriorityReply>>,
        PriorityQueue<RuntimeAddr, u8, u8, Target, PriorityReply, ReplyRoute<PriorityReply>>
    );

    type RateReply = MessageProtocol<RuntimeAddr, RateLimiterOutcome<u8>>;
    assert_both_routes!(
        RateReply,
        RateLimiter<RuntimeAddr, u8, Target, RateReply, EstablishedRecipient<RateReply>>,
        RateLimiter<RuntimeAddr, u8, Target, RateReply, ReplyRoute<RateReply>>
    );

    type ReadinessReply = MessageProtocol<RuntimeAddr, ReadinessReport<u8>>;
    assert_both_routes!(
        ReadinessReply,
        Readiness<RuntimeAddr, u8, ReadinessReply, EstablishedRecipient<ReadinessReply>>,
        Readiness<RuntimeAddr, u8, ReadinessReply, ReplyRoute<ReadinessReply>>
    );

    type RegistryReply = MessageProtocol<RuntimeAddr, RegistryResult<u8, Target>>;
    assert_both_routes!(
        RegistryReply,
        Registry<RuntimeAddr, u8, Target, RegistryReply, EstablishedRecipient<RegistryReply>>,
        Registry<RuntimeAddr, u8, Target, RegistryReply, ReplyRoute<RegistryReply>>
    );

    type ResolverReply = MessageProtocol<RuntimeAddr, Resolution<u8, Target>>;
    assert_both_routes!(
        ResolverReply,
        Resolver<RuntimeAddr, u8, Target, ResolverReply, EstablishedRecipient<ResolverReply>>,
        Resolver<RuntimeAddr, u8, Target, ResolverReply, ReplyRoute<ResolverReply>>
    );

    type SequencerReply = MessageProtocol<RuntimeAddr, SequencerOutcome<u8>>;
    assert_both_routes!(
        SequencerReply,
        Sequencer<RuntimeAddr, u8, Target, SequencerReply, EstablishedRecipient<SequencerReply>>,
        Sequencer<RuntimeAddr, u8, Target, SequencerReply, ReplyRoute<SequencerReply>>
    );

    type TaskReply = MessageProtocol<RuntimeAddr, TaskResult<u8>>;
    assert_both_routes!(
        TaskReply,
        Task<RuntimeAddr, u8, TaskReply, EstablishedRecipient<TaskReply>>,
        Task<RuntimeAddr, u8, TaskReply, ReplyRoute<TaskReply>>
    );

    type QueueReply = MessageProtocol<RuntimeAddr, WorkQueueOutcome<u8>>;
    assert_both_routes!(
        QueueReply,
        WorkQueue<RuntimeAddr, u8, Target, QueueReply, EstablishedRecipient<QueueReply>>,
        WorkQueue<RuntimeAddr, u8, Target, QueueReply, ReplyRoute<QueueReply>>
    );

    type WorkflowReply = MessageProtocol<RuntimeAddr, WorkflowOutcome<u8>>;
    assert_both_routes!(
        WorkflowReply,
        Workflow<RuntimeAddr, u8, WorkflowReply, EstablishedRecipient<WorkflowReply>>,
        Workflow<RuntimeAddr, u8, WorkflowReply, ReplyRoute<WorkflowReply>>
    );

    type BarrierReply = MessageProtocol<RuntimeAddr, BarrierReleased>;
    assert_both_routes!(
        BarrierReply,
        Barrier<RuntimeAddr, u8, EstablishedRecipient<BarrierReply>>,
        Barrier<RuntimeAddr, u8, ReplyRoute<BarrierReply>>
    );

    type LatchReply = MessageProtocol<RuntimeAddr, LatchReleased>;
    assert_both_routes!(
        LatchReply,
        Latch<RuntimeAddr, EstablishedRecipient<LatchReply>>,
        Latch<RuntimeAddr, ReplyRoute<LatchReply>>
    );
}

struct Managed;

impl Protocol for Managed {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl Behavior for Managed {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[test]
fn dynamic_supervision_projects_its_reply_protocol_from_each_route() {
    type Reply = MessageProtocol<RuntimeAddr, DynamicSupervisorOutcome<RuntimeAddr, Managed>>;
    assert_behavior::<DynamicSupervisor<RuntimeAddr, Managed, EstablishedRecipient<Reply>>>();
    assert_behavior::<DynamicSupervisor<RuntimeAddr, Managed, ReplyRoute<Reply>>>();
}

macro_rules! pool_route_case {
    ($module:ident, $route:ident) => {
        mod $module {
            use super::*;

            type Reply = MessageProtocol<RuntimeAddr, PoolResponse<u8, u16, RuntimeAddr>>;
            type Route = $route<Reply>;

            struct Worker;
            impl Protocol for Worker {
                type Addr = RuntimeAddr;
                type Msg = PoolAssignment<WorkerPoolProtocol<RuntimeAddr, Reply, u8, u16, Route>>;
            }
            impl Behavior for Worker {
                type Protocol = Self;
                type Event = User<
                    RuntimeAddr,
                    PoolAssignment<WorkerPoolProtocol<RuntimeAddr, Reply, u8, u16, Route>>,
                >;
                type Sends = Vec<Never>;
                type Ph = Never;
                type Error = Never;
                type Birth = NoBirths;
                fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }

            struct KeyedWorker;
            impl Protocol for KeyedWorker {
                type Addr = RuntimeAddr;
                type Msg =
                    PoolAssignment<KeyedWorkerPoolProtocol<RuntimeAddr, Reply, u8, u8, u16, Route>>;
            }
            impl Behavior for KeyedWorker {
                type Protocol = Self;
                type Event = User<
                    RuntimeAddr,
                    PoolAssignment<KeyedWorkerPoolProtocol<RuntimeAddr, Reply, u8, u8, u16, Route>>,
                >;
                type Sends = Vec<Never>;
                type Ph = Never;
                type Error = Never;
                type Birth = NoBirths;
                fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }

            #[test]
            fn pool_protocols_retain_the_selected_customer_capability() {
                assert_behavior::<WorkerPool<RuntimeAddr, Reply, u8, u16, Worker, Route>>();
                assert_behavior::<
                    KeyedWorkerPool<
                        RuntimeAddr,
                        Reply,
                        u8,
                        u8,
                        u16,
                        KeyedWorker,
                        Route,
                        fn(&u8) -> u64,
                    >,
                >();
            }
        }
    };
}

pool_route_case!(exact_pool, EstablishedRecipient);
pool_route_case!(mixed_pool, ReplyRoute);
