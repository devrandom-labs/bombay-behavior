//! Compile-contract matrix for every customer-passing actor template.

use behavior_actors::*;
use core::marker::PhantomData;

#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P> {
    id: u64,
    protocol: PhantomData<fn() -> P>,
}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            protocol: PhantomData,
        }
    }
}

impl<P> PartialEq for Endpoint<P> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<P> Eq for Endpoint<P> {}

impl<P> core::fmt::Debug for Endpoint<P> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Endpoint").field(&self.id).finish()
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

fn exact<P>(id: u64) -> EstablishedRecipient<P>
where
    P: Protocol<Addr = RuntimeAddr>,
{
    EstablishedRecipient::issued(Endpoint {
        id,
        protocol: PhantomData,
    })
}

struct Target;

impl Protocol for Target {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

fn assert_behavior<B: Behavior>() {}

fn assert_behavior_value<B: Behavior>(_: &B) {}

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
        Acknowledgements<RuntimeAddr, u8, u16, EstablishedRecipient<Ack>>,
        Acknowledgements<RuntimeAddr, u8, u16, ReplyRoute<Ack>>
    );

    type BufferReply = MessageProtocol<RuntimeAddr, BufferOutcome<u8>>;
    type BufferTarget = MessageProtocol<RuntimeAddr, u8>;
    assert_both_routes!(
        BufferReply,
        Buffer<RuntimeAddr, u8, Recipient<BufferTarget>, EstablishedRecipient<BufferReply>>,
        Buffer<RuntimeAddr, u8, Recipient<BufferTarget>, ReplyRoute<BufferReply>>
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
        CircuitBreaker<RuntimeAddr, EstablishedRecipient<BreakerReply>>,
        CircuitBreaker<RuntimeAddr, ReplyRoute<BreakerReply>>
    );

    type ConfigurationReply = MessageProtocol<RuntimeAddr, ConfigurationState<u8>>;
    assert_both_routes!(
        ConfigurationReply,
        Configuration<RuntimeAddr, u8, EstablishedRecipient<ConfigurationReply>>,
        Configuration<RuntimeAddr, u8, ReplyRoute<ConfigurationReply>>
    );

    type FeaturesReply = MessageProtocol<RuntimeAddr, ConfigurationState<FeatureSet<u8>>>;
    assert_both_routes!(
        FeaturesReply,
        Configuration<RuntimeAddr, FeatureSet<u8>, EstablishedRecipient<FeaturesReply>>,
        Configuration<RuntimeAddr, FeatureSet<u8>, ReplyRoute<FeaturesReply>>
    );

    type CorrelatorReply = MessageProtocol<RuntimeAddr, CorrelationResult<u8, u16>>;
    assert_both_routes!(
        CorrelatorReply,
        Correlator<RuntimeAddr, u8, u16, EstablishedRecipient<CorrelatorReply>>,
        Correlator<RuntimeAddr, u8, u16, ReplyRoute<CorrelatorReply>>
    );

    type DeduplicatorReply = MessageProtocol<RuntimeAddr, DeduplicatorOutcome<u8, u8>>;
    assert_both_routes!(
        DeduplicatorReply,
        Deduplicator<RuntimeAddr, u8, u8, Recipient<Target>, EstablishedRecipient<DeduplicatorReply>>,
        Deduplicator<RuntimeAddr, u8, u8, Recipient<Target>, ReplyRoute<DeduplicatorReply>>
    );

    type HealthReply = MessageProtocol<RuntimeAddr, HealthReport<u8>>;
    assert_both_routes!(
        HealthReply,
        Health<RuntimeAddr, u8, EstablishedRecipient<HealthReply>>,
        Health<RuntimeAddr, u8, ReplyRoute<HealthReply>>
    );

    type LeaseReply = MessageProtocol<RuntimeAddr, LeaseOutcome<u8>>;
    assert_both_routes!(
        LeaseReply,
        Lease<RuntimeAddr, u8, EstablishedRecipient<LeaseReply>>,
        Lease<RuntimeAddr, u8, ReplyRoute<LeaseReply>>
    );

    type GateReply = MessageProtocol<RuntimeAddr, OrderGateOutcome<u8, u8>>;
    assert_both_routes!(
        GateReply,
        OrderGate<RuntimeAddr, u8, u8, Recipient<Target>, EstablishedRecipient<GateReply>>,
        OrderGate<RuntimeAddr, u8, u8, Recipient<Target>, ReplyRoute<GateReply>>
    );

    type PresenceReplyProtocol = MessageProtocol<RuntimeAddr, PresenceReply<u8>>;
    assert_both_routes!(
        PresenceReplyProtocol,
        Presence<RuntimeAddr, u8, EstablishedRecipient<PresenceReplyProtocol>>,
        Presence<RuntimeAddr, u8, ReplyRoute<PresenceReplyProtocol>>
    );

    type PriorityReply = MessageProtocol<RuntimeAddr, PriorityQueueOutcome<u8, u8>>;
    assert_both_routes!(
        PriorityReply,
        PriorityQueue<RuntimeAddr, u8, u8, Recipient<Target>, EstablishedRecipient<PriorityReply>>,
        PriorityQueue<RuntimeAddr, u8, u8, Recipient<Target>, ReplyRoute<PriorityReply>>
    );

    type RateReply = MessageProtocol<RuntimeAddr, RateLimiterOutcome<u8>>;
    assert_both_routes!(
        RateReply,
        RateLimiter<RuntimeAddr, u8, Recipient<Target>, EstablishedRecipient<RateReply>>,
        RateLimiter<RuntimeAddr, u8, Recipient<Target>, ReplyRoute<RateReply>>
    );

    type ReadinessReply = MessageProtocol<RuntimeAddr, ReadinessReport<u8>>;
    assert_both_routes!(
        ReadinessReply,
        Readiness<RuntimeAddr, u8, EstablishedRecipient<ReadinessReply>>,
        Readiness<RuntimeAddr, u8, ReplyRoute<ReadinessReply>>
    );

    type RegistryReply = MessageProtocol<RuntimeAddr, RegistryResult<u8, Target>>;
    assert_both_routes!(
        RegistryReply,
        Registry<RuntimeAddr, u8, Target, EstablishedRecipient<RegistryReply>>,
        Registry<RuntimeAddr, u8, Target, ReplyRoute<RegistryReply>>
    );

    type ResolverReply = MessageProtocol<RuntimeAddr, Resolution<u8, Target>>;
    assert_both_routes!(
        ResolverReply,
        Resolver<RuntimeAddr, u8, Target, EstablishedRecipient<ResolverReply>>,
        Resolver<RuntimeAddr, u8, Target, ReplyRoute<ResolverReply>>
    );

    type SequencerReply = MessageProtocol<RuntimeAddr, SequencerOutcome<u8>>;
    assert_both_routes!(
        SequencerReply,
        Sequencer<RuntimeAddr, u8, Recipient<Target>, EstablishedRecipient<SequencerReply>>,
        Sequencer<RuntimeAddr, u8, Recipient<Target>, ReplyRoute<SequencerReply>>
    );

    type TaskReply = MessageProtocol<RuntimeAddr, TaskResult<u8>>;
    assert_both_routes!(
        TaskReply,
        Task<RuntimeAddr, u8, EstablishedRecipient<TaskReply>>,
        Task<RuntimeAddr, u8, ReplyRoute<TaskReply>>
    );

    type QueueReply = MessageProtocol<RuntimeAddr, WorkQueueOutcome<u8>>;
    assert_both_routes!(
        QueueReply,
        WorkQueue<RuntimeAddr, u8, Recipient<Target>, EstablishedRecipient<QueueReply>>,
        WorkQueue<RuntimeAddr, u8, Recipient<Target>, ReplyRoute<QueueReply>>
    );

    type WorkflowReply = MessageProtocol<RuntimeAddr, WorkflowOutcome<u8>>;
    assert_both_routes!(
        WorkflowReply,
        Workflow<RuntimeAddr, u8, EstablishedRecipient<WorkflowReply>>,
        Workflow<RuntimeAddr, u8, ReplyRoute<WorkflowReply>>
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

#[test]
fn every_payload_and_membership_seam_accepts_exact_capabilities() {
    type Payload = MessageProtocol<RuntimeAddr, u8>;
    type BufferReply = MessageProtocol<RuntimeAddr, BufferOutcome<u8>>;
    assert_behavior::<Buffer<RuntimeAddr, u8, EstablishedRecipient<Payload>, Recipient<BufferReply>>>(
    );

    type DedupReply = MessageProtocol<RuntimeAddr, DeduplicatorOutcome<u8, u8>>;
    assert_behavior::<
        Deduplicator<RuntimeAddr, u8, u8, EstablishedRecipient<Target>, Recipient<DedupReply>>,
    >();

    type GateReply = MessageProtocol<RuntimeAddr, OrderGateOutcome<u8, u8>>;
    assert_behavior::<
        OrderGate<RuntimeAddr, u8, u8, EstablishedRecipient<Target>, Recipient<GateReply>>,
    >();

    type PriorityReply = MessageProtocol<RuntimeAddr, PriorityQueueOutcome<u8, u8>>;
    assert_behavior::<
        PriorityQueue<RuntimeAddr, u8, u8, EstablishedRecipient<Target>, Recipient<PriorityReply>>,
    >();

    type RateReply = MessageProtocol<RuntimeAddr, RateLimiterOutcome<u8>>;
    assert_behavior::<
        RateLimiter<RuntimeAddr, u8, EstablishedRecipient<Target>, Recipient<RateReply>>,
    >();

    type SequenceReply = MessageProtocol<RuntimeAddr, SequencerOutcome<u8>>;
    assert_behavior::<
        Sequencer<RuntimeAddr, u8, EstablishedRecipient<Target>, Recipient<SequenceReply>>,
    >();

    assert_behavior::<Router<RuntimeAddr, EstablishedRecipient<Target>, RoundRobin>>();

    type WorkReply = MessageProtocol<RuntimeAddr, WorkQueueOutcome<u8>>;
    assert_behavior::<WorkQueue<RuntimeAddr, u8, EstablishedRecipient<Target>, Recipient<WorkReply>>>(
    );
    assert_behavior::<Topic<RuntimeAddr, u8, EstablishedRecipient<Target>>>();
    assert_behavior::<PubSub<RuntimeAddr, u8, u8, EstablishedRecipient<Target>>>();
}

#[test]
fn exact_capabilities_survive_every_payload_and_membership_fold() {
    use core::num::NonZeroU64;

    let sender = RuntimeAddr(90);
    let target = exact::<Target>(1);

    type DedupReply = MessageProtocol<RuntimeAddr, DeduplicatorOutcome<u8, u8>>;
    let mut dedup = Deduplicator::<
        RuntimeAddr,
        u8,
        u8,
        EstablishedRecipient<Target>,
        Recipient<DedupReply>,
    >::new(2)
    .unwrap()
    .initialize()
    .unwrap()
    .behavior;
    let deduplicated = dedup
        .receive(
            sender,
            DeduplicatorMessage::Deliver {
                key: 4,
                value: 40,
                to: target.clone(),
                reply_to: Recipient::global(RuntimeAddr(10)),
            },
        )
        .unwrap();
    assert_eq!(deduplicated.sends.deliveries.len(), 1);
    assert_eq!(deduplicated.sends.deliveries[0].to, target);
    assert_eq!(deduplicated.sends.deliveries[0].message, 40);
    assert!(matches!(
        deduplicated.sends.outcomes[0].message,
        DeduplicatorOutcome::Delivered {
            key: 4,
            evicted: None
        }
    ));
    assert!(deduplicated.creates.is_empty());
    assert!(matches!(deduplicated.become_, Step::Continue));

    type RateReply = MessageProtocol<RuntimeAddr, RateLimiterOutcome<u8>>;
    let mut limiter =
        RateLimiter::<RuntimeAddr, u8, EstablishedRecipient<Target>, Recipient<RateReply>>::new(
            TokenCount::new(NonZeroU64::new(2).unwrap()),
            2,
        )
        .unwrap()
        .initialize()
        .unwrap()
        .behavior;
    let admitted = limiter
        .receive(
            sender,
            RateLimiterMessage::Acquire {
                cost: TokenCount::new(NonZeroU64::new(1).unwrap()),
                value: 41,
                to: target.clone(),
                reply_to: Recipient::global(RuntimeAddr(11)),
            },
        )
        .unwrap();
    assert_eq!(admitted.sends.deliveries.len(), 1);
    assert_eq!(admitted.sends.deliveries[0].to, target);
    assert_eq!(admitted.sends.deliveries[0].message, 41);
    assert!(matches!(
        admitted.sends.outcomes[0].message,
        RateLimiterOutcome::Admitted { remaining: 1 }
    ));

    type SequenceReply = MessageProtocol<RuntimeAddr, SequencerOutcome<u8>>;
    let mut sequencer =
        Sequencer::<RuntimeAddr, u8, EstablishedRecipient<Target>, Recipient<SequenceReply>>::new(
            Sequence(3),
        )
        .initialize()
        .unwrap()
        .behavior;
    let sequenced = sequencer
        .receive(
            sender,
            SequencerMessage::Offer {
                sequence: Sequence(3),
                value: 42,
                to: target.clone(),
                reply_to: Recipient::global(RuntimeAddr(12)),
            },
        )
        .unwrap();
    assert_eq!(sequenced.sends.deliveries.len(), 1);
    assert_eq!(sequenced.sends.deliveries[0].to, target);
    assert_eq!(sequenced.sends.deliveries[0].message, 42);
    assert!(matches!(
        sequenced.sends.outcomes[0].message,
        SequencerOutcome::Accepted {
            released: 1,
            buffered: 0
        }
    ));

    type GateReply = MessageProtocol<RuntimeAddr, OrderGateOutcome<u8, u8>>;
    let mut gate =
        OrderGate::<RuntimeAddr, u8, u8, EstablishedRecipient<Target>, Recipient<GateReply>>::new()
            .initialize()
            .unwrap()
            .behavior;
    let held = gate
        .receive(
            sender,
            OrderGateMessage::Hold {
                key: 5,
                value: 43,
                to: target.clone(),
                reply_to: Recipient::global(RuntimeAddr(13)),
            },
        )
        .unwrap();
    assert!(held.sends.deliveries.is_empty());
    let opened = gate
        .receive(
            sender,
            OrderGateMessage::OpenThrough {
                through: 5,
                reply_to: Recipient::global(RuntimeAddr(13)),
            },
        )
        .unwrap();
    assert_eq!(opened.sends.deliveries.len(), 1);
    assert_eq!(opened.sends.deliveries[0].to, target);
    assert_eq!(opened.sends.deliveries[0].message, 43);
    assert!(matches!(
        opened.sends.outcomes[0].message,
        OrderGateOutcome::Opened {
            through: 5,
            released: 1,
            held: 0
        }
    ));

    type PriorityReply = MessageProtocol<RuntimeAddr, PriorityQueueOutcome<u8, u8>>;
    let mut priority = PriorityQueue::<
        RuntimeAddr,
        u8,
        u8,
        EstablishedRecipient<Target>,
        Recipient<PriorityReply>,
    >::new(1)
    .unwrap()
    .initialize()
    .unwrap()
    .behavior;
    let offered = priority
        .receive(
            sender,
            PriorityQueueMessage::Offer {
                value: 44,
                priority: 9,
                reply_to: Recipient::global(RuntimeAddr(14)),
            },
        )
        .unwrap();
    assert!(offered.sends.deliveries.is_empty());
    assert!(matches!(
        offered.sends.outcomes.as_slice(),
        [delivery]
            if matches!(delivery.message, PriorityQueueOutcome::Accepted { depth: 1 })
    ));
    assert!(offered.creates.is_empty());
    assert_eq!(offered.become_, behavior_actors::Step::Continue);
    let released = priority
        .receive(
            sender,
            PriorityQueueMessage::Release {
                to: target.clone(),
                reply_to: Recipient::global(RuntimeAddr(14)),
            },
        )
        .unwrap();
    assert_eq!(released.sends.deliveries.len(), 1);
    assert_eq!(released.sends.deliveries[0].to, target);
    assert_eq!(released.sends.deliveries[0].message, 44);
    assert!(matches!(
        released.sends.outcomes[0].message,
        PriorityQueueOutcome::Released { remaining: 0 }
    ));

    type Payload = MessageProtocol<RuntimeAddr, u8>;
    type BufferReply = MessageProtocol<RuntimeAddr, BufferOutcome<u8>>;
    let payload_target = exact::<Payload>(2);
    let mut buffer =
        Buffer::<RuntimeAddr, u8, EstablishedRecipient<Payload>, Recipient<BufferReply>>::new(
            BufferConfiguration::new(1, OverflowPolicy::Reject).unwrap(),
        )
        .initialize()
        .unwrap()
        .behavior;
    let offered = buffer
        .receive(
            sender,
            BufferMessage::Offer {
                value: 45,
                reply_to: Recipient::global(RuntimeAddr(15)),
            },
        )
        .unwrap();
    assert!(offered.sends.deliveries.is_empty());
    assert!(matches!(
        offered.sends.outcomes.as_slice(),
        [delivery]
            if matches!(delivery.message, BufferOutcome::Accepted { depth: 1 })
    ));
    assert!(offered.creates.is_empty());
    assert_eq!(offered.become_, behavior_actors::Step::Continue);
    let buffered = buffer
        .receive(
            sender,
            BufferMessage::Release {
                to: payload_target.clone(),
                reply_to: Recipient::global(RuntimeAddr(15)),
            },
        )
        .unwrap();
    assert_eq!(buffered.sends.deliveries.len(), 1);
    assert_eq!(buffered.sends.deliveries[0].to, payload_target);
    assert_eq!(buffered.sends.deliveries[0].message, 45);
    assert!(matches!(
        buffered.sends.outcomes[0].message,
        BufferOutcome::Released { remaining: 0 }
    ));

    let one = exact::<Target>(21);
    let two = exact::<Target>(22);
    let mut router = Router::new(vec![one.clone(), two.clone()], RoundRobin::default())
        .initialize()
        .unwrap()
        .behavior;
    let routed = router.receive(sender, RouterMessage::Route(46)).unwrap();
    assert_eq!(routed.sends.len(), 1);
    assert_eq!(routed.sends[0].to, one);
    assert_eq!(routed.sends[0].message, 46);

    type WorkReply = MessageProtocol<RuntimeAddr, WorkQueueOutcome<u8>>;
    let mut work =
        WorkQueue::<RuntimeAddr, u8, EstablishedRecipient<Target>, Recipient<WorkReply>>::new(0)
            .initialize()
            .unwrap()
            .behavior;
    let available = work
        .receive(
            sender,
            WorkQueueMessage::Available {
                worker: two.clone(),
            },
        )
        .unwrap();
    assert!(available.sends.assignments.is_empty());
    assert!(available.sends.outcomes.is_empty());
    assert!(available.creates.is_empty());
    assert_eq!(available.become_, behavior_actors::Step::Continue);
    let assigned = work
        .receive(
            sender,
            WorkQueueMessage::Submit {
                value: 47,
                reply_to: Recipient::global(RuntimeAddr(16)),
            },
        )
        .unwrap();
    assert_eq!(assigned.sends.assignments.len(), 1);
    assert_eq!(assigned.sends.assignments[0].to, two);
    assert_eq!(assigned.sends.assignments[0].message, 47);
    assert!(matches!(
        assigned.sends.outcomes[0].message,
        WorkQueueOutcome::Dispatched { queued: 0 }
    ));

    let subscriber = exact::<Target>(30);
    let mut topic = Topic::<RuntimeAddr, u8, EstablishedRecipient<Target>>::new()
        .initialize()
        .unwrap()
        .behavior;
    let subscribed = topic
        .receive(sender, TopicMessage::Subscribe(subscriber.clone()))
        .unwrap();
    assert!(subscribed.sends.is_empty());
    assert!(subscribed.creates.is_empty());
    assert_eq!(subscribed.become_, behavior_actors::Step::Continue);
    let published = topic.receive(sender, TopicMessage::Publish(48)).unwrap();
    assert_eq!(published.sends.len(), 1);
    assert_eq!(published.sends[0].to, subscriber);
    assert_eq!(published.sends[0].message, 48);

    let subscriber = exact::<Target>(31);
    let mut pub_sub = PubSub::<RuntimeAddr, u8, u8, EstablishedRecipient<Target>>::new()
        .initialize()
        .unwrap()
        .behavior;
    let subscribed = pub_sub
        .receive(
            sender,
            PubSubMessage::Subscribe {
                topic: 7,
                subscriber: subscriber.clone(),
            },
        )
        .unwrap();
    assert!(subscribed.sends.is_empty());
    assert!(subscribed.creates.is_empty());
    assert_eq!(subscribed.become_, behavior_actors::Step::Continue);
    let published = pub_sub
        .receive(
            sender,
            PubSubMessage::Publish {
                topic: 7,
                value: 49,
            },
        )
        .unwrap();
    assert_eq!(published.sends.len(), 1);
    assert_eq!(published.sends[0].to, subscriber);
    assert_eq!(published.sends[0].message, 49);
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
    let exact =
        DynamicSupervisor::<RuntimeAddr, Managed, EstablishedRecipient<Reply>, _>::new(Proxy::new);
    let mixed = DynamicSupervisor::<RuntimeAddr, Managed, ReplyRoute<Reply>, _>::new(Proxy::new);
    assert_behavior_value(&exact);
    assert_behavior_value(&mixed);
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
                type Msg = PoolAssignment<u8>;
            }
            impl Behavior for Worker {
                type Protocol = Self;
                type Event = User<RuntimeAddr, PoolAssignment<u8>>;
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
                type Msg = PoolAssignment<u8>;
            }
            impl Behavior for KeyedWorker {
                type Protocol = Self;
                type Event = User<RuntimeAddr, PoolAssignment<u8>>;
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
                let configuration = PoolConfiguration::new(
                    4,
                    InterruptionPolicy::Retry,
                    RestartPolicy::Permanent,
                    2,
                    core::time::Duration::from_secs(30),
                    behavior_actors::RestartTiming::Immediate,
                );
                let fifo = WorkerPool::<RuntimeAddr, u8, u16, Worker, Route, _>::new(
                    ChildTopology::new([1], |_| Some(Worker)),
                    configuration,
                    Proxy::new,
                )
                .unwrap();
                let keyed =
                    KeyedWorkerPool::<RuntimeAddr, u8, u8, u16, KeyedWorker, Route, _, _>::new(
                        ChildTopology::new([2], |_| Some(KeyedWorker)),
                        configuration,
                        |_: &u8| 2,
                        Proxy::new,
                    )
                    .unwrap();
                assert_behavior_value(&fifo);
                assert_behavior_value(&keyed);
            }
        }
    };
}

pool_route_case!(exact_pool, EstablishedRecipient);
pool_route_case!(mixed_pool, ReplyRoute);
