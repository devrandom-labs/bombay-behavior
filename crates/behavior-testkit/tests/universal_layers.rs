//! Generic construction tests for real catalogue behavior layers.

use std::time::{Duration, Instant};

use behavior::{
    Activate as _, Cache, CacheConfiguration, CacheMessage, CacheResult, ChildTopology,
    CreationResolved, Deadline, FinalizeOnShutdown, OneShot, PeerTermination, Periodic,
    PriorityQueue, PriorityQueueMessage, PriorityQueueOutcome, PropagateTermination, Proxy,
    ProxyCommand, ReceiveTimeout, RestartConfiguration, RestartPolicy, RoundRobin, Router,
    RouterMessage, ShutdownRequested, Stash, StashRoute, StopOnShutdown, Strategy, Supervisor,
    TerminationMonitor, TimerId, Watch, propagate_all, stop_on_abnormal_death,
};
use foundation::{
    Actions, Behavior, BehaviorLayer, CreationKind, Delivery, MailAddr, MessageProtocol, Never,
    NoBirths, Recipient, Step,
};

fn apply<B, L>(behavior: B, layer: L) -> L::Output
where
    B: Behavior,
    L: BehaviorLayer<B>,
{
    layer.layer(behavior)
}

fn successful<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => panic!("expected a successful hierarchy transition"),
    }
}

struct QueueTarget;

impl foundation::Protocol for QueueTarget {
    type Addr = MailAddr;
    type Msg = u8;
}

type QueueReply = MessageProtocol<MailAddr, PriorityQueueOutcome<u8, u8>>;
type Queue = PriorityQueue<MailAddr, u8, u8, Recipient<QueueTarget>, Recipient<QueueReply>>;

fn queue() -> Queue {
    Queue::new(4).unwrap()
}

type CacheReply = MessageProtocol<MailAddr, CacheResult<u8, u16>>;
type TestCache = Cache<MailAddr, u8, u16, Recipient<CacheReply>>;

fn deliver_cache(_: &CacheMessage<u8, u16, Recipient<CacheReply>>) -> StashRoute {
    StashRoute::Deliver
}

fn cache() -> TestCache {
    TestCache::new(CacheConfiguration::new(2).unwrap())
}

fn finalize_cache(
    _: &mut TestCache,
    _: ShutdownRequested,
) -> Actions<MailAddr, Never, Vec<Delivery<CacheReply>>, NoBirths> {
    Actions::send(vec![Delivery::new(
        Recipient::global(MailAddr(31)),
        CacheResult::Absent { key: 7 },
    )])
}

#[test]
fn generic_consumer_layers_routing_inside_supervision_without_naming_the_output() {
    let proxy = apply(queue(), Proxy::new);
    let root = apply(proxy, StopOnShutdown::new);

    let initialized = root.initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 1);
    assert_eq!(initialized.actions.creates[0].nonce, 0);
    assert_eq!(initialized.actions.sends.inner.child_observations.len(), 1);
    assert_eq!(
        initialized.actions.sends.inner.creation_observations.len(),
        1
    );
    assert!(matches!(initialized.actions.become_, Step::Continue));
}

#[test]
fn router_to_supervised_proxy_to_priority_queue_preserves_every_hierarchy_edge() {
    type StableQueue = Proxy<Queue>;

    let supervisor = Supervisor::<MailAddr, Queue>::new(
        ChildTopology::new([7], |_| Some(queue())),
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        ),
    )
    .unwrap()
    .initialize()
    .unwrap();
    assert_eq!(supervisor.actions.creates.len(), 1);
    assert_eq!(supervisor.actions.creates[0].nonce, 7);
    assert!(matches!(
        supervisor.actions.creates[0].kind,
        CreationKind::Birth
    ));
    assert_eq!(supervisor.actions.sends.child_observations.len(), 1);
    assert_eq!(supervisor.actions.sends.creation_observations.len(), 1);
    assert!(supervisor.actions.sends.replacement_commands.is_empty());
    assert!(supervisor.actions.sends.shutdowns.is_empty());
    assert!(supervisor.actions.sends.failure_reports.is_empty());
    assert!(matches!(supervisor.actions.become_, Step::Continue));

    let proxy_creation = supervisor.actions.creates.into_iter().next().unwrap();
    let initialized_proxy = proxy_creation.child.initialize().unwrap();
    assert_eq!(initialized_proxy.actions.creates.len(), 1);
    assert_eq!(initialized_proxy.actions.creates[0].nonce, 0);
    assert!(matches!(
        initialized_proxy.actions.creates[0].kind,
        CreationKind::Birth
    ));
    assert_eq!(initialized_proxy.actions.sends.child_observations.len(), 1);
    assert_eq!(
        initialized_proxy.actions.sends.creation_observations.len(),
        1
    );
    assert!(initialized_proxy.actions.sends.deliveries.is_empty());
    assert!(initialized_proxy.actions.sends.unavailable.is_empty());
    assert!(initialized_proxy.actions.sends.shutdowns.is_empty());
    assert!(matches!(initialized_proxy.actions.become_, Step::Continue));

    let queue_creation = initialized_proxy
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap();
    let mut queue = queue_creation.child.initialize().unwrap().behavior;
    let mut proxy = initialized_proxy.behavior;
    let installed = proxy
        .on_path(CreationResolved::birth(0, MailAddr(90)))
        .unwrap();
    assert!(installed.creates.is_empty());
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable.is_empty());
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(matches!(installed.become_, Step::Continue));

    let stable = Recipient::<StableQueue>::global(MailAddr(80));
    let mut router =
        successful(Router::new(vec![stable], RoundRobin::default()).initialize()).behavior;
    let routed_offer = successful(router.receive(
        MailAddr(1),
        RouterMessage::Route(ProxyCommand::Forward {
            command: PriorityQueueMessage::Offer {
                value: 41,
                priority: 9,
                reply_to: Recipient::global(MailAddr(81)),
            },
            unavailable_to: Recipient::global(MailAddr(82)),
        }),
    ));
    assert_eq!(routed_offer.sends.len(), 1);
    assert_eq!(routed_offer.sends[0].to.address(), MailAddr(80));
    assert!(routed_offer.creates.is_empty());
    assert!(matches!(routed_offer.become_, Step::Continue));

    let offered_to_proxy = routed_offer.sends.into_iter().next().unwrap().message;
    let forwarded_offer = proxy.receive(MailAddr(1), offered_to_proxy).unwrap();
    assert_eq!(forwarded_offer.sends.deliveries.len(), 1);
    assert_eq!(forwarded_offer.sends.deliveries[0].nonce, 0);
    assert!(forwarded_offer.sends.unavailable.is_empty());
    assert!(forwarded_offer.sends.child_observations.is_empty());
    assert!(forwarded_offer.sends.creation_observations.is_empty());
    assert!(forwarded_offer.sends.shutdowns.is_empty());
    assert!(forwarded_offer.creates.is_empty());
    assert!(matches!(forwarded_offer.become_, Step::Continue));

    let offer = forwarded_offer
        .sends
        .deliveries
        .into_iter()
        .next()
        .unwrap()
        .message;
    let accepted = queue.receive(MailAddr(1), offer).unwrap();
    assert!(accepted.sends.deliveries.is_empty());
    assert_eq!(accepted.sends.outcomes.len(), 1);
    assert!(matches!(
        accepted.sends.outcomes[0].message,
        PriorityQueueOutcome::Accepted { depth: 1 }
    ));
    assert!(accepted.creates.is_empty());
    assert!(matches!(accepted.become_, Step::Continue));

    let routed_release = successful(router.receive(
        MailAddr(1),
        RouterMessage::Route(ProxyCommand::Forward {
            command: PriorityQueueMessage::Release {
                to: Recipient::global(MailAddr(83)),
                reply_to: Recipient::global(MailAddr(84)),
            },
            unavailable_to: Recipient::global(MailAddr(85)),
        }),
    ));
    assert_eq!(routed_release.sends.len(), 1);
    assert!(routed_release.creates.is_empty());
    assert!(matches!(routed_release.become_, Step::Continue));

    let released_to_proxy = routed_release.sends.into_iter().next().unwrap().message;
    let forwarded_release = proxy.receive(MailAddr(1), released_to_proxy).unwrap();
    assert_eq!(forwarded_release.sends.deliveries.len(), 1);
    assert_eq!(forwarded_release.sends.deliveries[0].nonce, 0);
    assert!(forwarded_release.sends.unavailable.is_empty());
    assert!(forwarded_release.sends.child_observations.is_empty());
    assert!(forwarded_release.sends.creation_observations.is_empty());
    assert!(forwarded_release.sends.shutdowns.is_empty());
    assert!(forwarded_release.creates.is_empty());
    assert!(matches!(forwarded_release.become_, Step::Continue));

    let release = forwarded_release
        .sends
        .deliveries
        .into_iter()
        .next()
        .unwrap()
        .message;
    let released = queue.receive(MailAddr(1), release).unwrap();
    assert_eq!(released.sends.deliveries.len(), 1);
    assert_eq!(released.sends.deliveries[0].to.address(), MailAddr(83));
    assert_eq!(released.sends.deliveries[0].message, 41);
    assert_eq!(released.sends.outcomes.len(), 1);
    assert!(matches!(
        released.sends.outcomes[0].message,
        PriorityQueueOutcome::Released { remaining: 0 }
    ));
    assert!(released.creates.is_empty());
    assert!(matches!(released.become_, Step::Continue));
}

#[test]
fn captured_and_function_layers_preserve_domain_sends_and_outer_shutdown() {
    let stashed = apply(cache(), |inner| Stash::new(inner, deliver_cache));
    let mut root = apply(stashed, StopOnShutdown::new)
        .initialize()
        .unwrap()
        .behavior;

    let stored = root
        .receive(
            MailAddr(1),
            CacheMessage::Put {
                key: 3,
                value: 30,
                reply_to: Recipient::global(MailAddr(9)),
            },
        )
        .unwrap();
    assert_eq!(stored.sends.inner.len(), 1);
    assert!(stored.creates.is_empty());
    assert!(matches!(stored.become_, Step::Continue));

    let stopped = root.on_path(ShutdownRequested).unwrap();
    assert!(stopped.sends.inner.is_empty());
    assert!(stopped.creates.is_empty());
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[test]
fn finalization_layer_preserves_its_complete_fold_before_stopping() {
    let mut root = apply(cache(), |inner| {
        FinalizeOnShutdown::new(inner, finalize_cache)
    })
    .initialize()
    .unwrap()
    .behavior;

    let finalized = root.on_path(ShutdownRequested).unwrap();
    assert_eq!(finalized.sends.inner.len(), 1);
    assert_eq!(finalized.sends.inner[0].to.address(), MailAddr(31));
    assert!(matches!(
        finalized.sends.inner[0].message,
        CacheResult::Absent { key: 7 }
    ));
    assert!(finalized.creates.is_empty());
    assert!(matches!(finalized.become_, Step::Stop(_)));
}

#[test]
fn generic_consumer_carries_every_stateless_and_timing_transformation_output() {
    let deadline = apply(cache(), |inner| {
        Deadline::new(
            inner,
            TimerId(1),
            Some(Instant::now() + Duration::from_secs(1)),
            |_| Step::Continue,
        )
    })
    .initialize()
    .unwrap();
    assert_eq!(deadline.actions.sends.owned.len(), 1);
    assert!(deadline.actions.sends.inner.is_empty());

    let one_shot = apply(cache(), |inner| {
        OneShot::new(inner, TimerId(2), Duration::from_secs(1), |_| {
            Actions::cont()
        })
    })
    .initialize()
    .unwrap();
    assert_eq!(one_shot.actions.sends.owned.len(), 1);
    assert!(one_shot.actions.sends.inner.is_empty());

    let periodic = apply(cache(), |inner| {
        Periodic::new(inner, TimerId(3), Duration::from_secs(1), |_| {
            Actions::cont()
        })
    })
    .initialize()
    .unwrap();
    assert_eq!(periodic.actions.sends.owned.len(), 1);
    assert!(periodic.actions.sends.inner.is_empty());

    let timeout = apply(cache(), |inner| {
        ReceiveTimeout::new(inner, TimerId(4), Duration::from_secs(1), |_| {
            Actions::cont()
        })
    })
    .initialize()
    .unwrap();
    assert_eq!(timeout.actions.sends.owned.len(), 1);
    assert!(timeout.actions.sends.inner.is_empty());
}

#[test]
fn generic_consumer_carries_every_observation_transformation_output() {
    let watch = apply(cache(), |inner| {
        Watch::new(inner, MailAddr(20), stop_on_abnormal_death)
    })
    .initialize()
    .unwrap();
    assert_eq!(watch.actions.sends.owned.len(), 1);
    assert!(watch.actions.sends.inner.is_empty());

    let monitor = apply(cache(), |inner| {
        TerminationMonitor::new(inner, MailAddr(21), |_, _| Actions::cont())
    })
    .initialize()
    .unwrap();
    assert_eq!(monitor.actions.sends.owned.len(), 1);
    assert!(monitor.actions.sends.inner.is_empty());

    let propagation = apply(cache(), |inner| {
        PropagateTermination::new(inner, PeerTermination::new(MailAddr(22)), propagate_all)
    })
    .initialize()
    .unwrap();
    assert_eq!(propagation.actions.sends.owned.observations.len(), 1);
    assert!(propagation.actions.sends.owned.reports.is_empty());
    assert!(propagation.actions.sends.inner.is_empty());
}

#[test]
fn behavior_method_chains_layers_with_no_composed_type_annotation() {
    let composed = Queue::new(2)
        .unwrap()
        .layer(Proxy::new)
        .layer(StopOnShutdown::new);

    let initialized = composed.initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 1);
    assert_eq!(initialized.actions.creates[0].nonce, 0);
}
