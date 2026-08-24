//! Generic construction tests for real catalogue behavior layers.

use std::time::{Duration, Instant};

use behavior::{
    Activate as _, Cache, CacheConfiguration, CacheMessage, CacheResult, Deadline,
    FinalizeOnShutdown, OneShot, PeerTermination, Periodic, PriorityQueue, PriorityQueueOutcome,
    PropagateTermination, Proxy, ReceiveTimeout, ShutdownRequested, Stash, StashRoute,
    StopOnShutdown, TerminationMonitor, TimerId, Watch, propagate_all, stop_on_abnormal_death,
};
use foundation::{
    Actions, Behavior, BehaviorLayer, Delivery, MailAddr, MessageProtocol, Never, NoBirths,
    Recipient, Step,
};

fn apply<B, L>(behavior: B, layer: L) -> L::Output
where
    B: Behavior,
    L: BehaviorLayer<B>,
{
    layer.layer(behavior)
}

struct QueueTarget;

impl foundation::Protocol for QueueTarget {
    type Addr = MailAddr;
    type Msg = u8;
}

type QueueReply = MessageProtocol<MailAddr, PriorityQueueOutcome<u8, u8>>;
type Queue = PriorityQueue<MailAddr, u8, u8, Recipient<QueueTarget>, Recipient<QueueReply>>;

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
    let queue = Queue::new(4).unwrap();
    let proxy = apply(queue, Proxy::new);
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
