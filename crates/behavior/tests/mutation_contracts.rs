use behavior::{
    Address, ChildEvent, ChildStopped, CreationEvent, CreationKind, CreationResolved,
    DeadlineEvent, Exit, MailAddr, Never, ObserveChild, PeerEvent, PeerStopped, ProxyEvent,
    ProxySends, ReceiveTimeoutEvent, ReceiveTimeoutSends, Recipient, ScheduleAfter, SendAlgebra,
    ServiceSends, ShutdownEvent, ShutdownProtocol, ShutdownRequested, SupervisionEvent, TimeEvent,
    TimerElapsed, TimerGeneration, TimerId, User, UserEvent, WatchEvent, WorkerCreationEvent,
    WorkerCreationResolved, WorkerEvent, WorkerStopped,
};
use std::time::Duration;
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lane {
    Time(TimerElapsed),
    Peer(PeerStopped<MailAddr>),
    Child(ChildStopped<MailAddr>),
    Worker(WorkerStopped<MailAddr>),
    Creation(CreationResolved<u64>),
    WorkerCreation(WorkerCreationResolved<u64>),
    Shutdown,
}

impl TimeEvent for Lane {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        Some(Self::Time(event))
    }
}
impl UserEvent for Lane {
    type Addr = MailAddr;
    type Message = Never;

    fn user(_: MailAddr, message: Never) -> Self {
        match message {}
    }

    fn into_user(self) -> Result<User<MailAddr, Never>, Self> {
        Err(self)
    }
}
impl PeerEvent for Lane {
    fn peer_stopped(event: PeerStopped<MailAddr>) -> Option<Self> {
        Some(Self::Peer(event))
    }
}
impl ChildEvent for Lane {
    fn child_stopped(event: ChildStopped<MailAddr>) -> Option<Self> {
        Some(Self::Child(event))
    }
}
impl WorkerEvent for Lane {
    fn worker_stopped(event: WorkerStopped<MailAddr>) -> Option<Self> {
        Some(Self::Worker(event))
    }
}
impl CreationEvent for Lane {
    fn creation_resolved(event: CreationResolved<u64>) -> Option<Self> {
        Some(Self::Creation(event))
    }
}
impl WorkerCreationEvent for Lane {
    fn worker_creation_resolved(event: WorkerCreationResolved<u64>) -> Option<Self> {
        Some(Self::WorkerCreation(event))
    }
}
impl ShutdownEvent for Lane {
    fn shutdown_requested(_: ShutdownRequested) -> Option<Self> {
        Some(Self::Shutdown)
    }
}

fn elapsed() -> TimerElapsed {
    TimerElapsed {
        id: TimerId(7),
        generation: TimerGeneration(3),
    }
}
fn peer() -> PeerStopped<MailAddr> {
    PeerStopped {
        peer: MailAddr(9),
        outcome: Ok(Exit::Normal),
    }
}
fn child() -> ChildStopped<MailAddr> {
    ChildStopped {
        nonce: 11,
        outcome: Ok(Exit::Normal),
        at: Instant::now(),
    }
}
fn worker() -> WorkerStopped<MailAddr> {
    WorkerStopped {
        proxy: 13,
        worker: 13,
        outcome: Ok(Exit::Normal),
        at: Instant::now(),
    }
}
fn creation() -> CreationResolved<u64> {
    CreationResolved {
        nonce: 17,
        kind: CreationKind::ReplacementIncarnation { replaces: 16 },
        result: Ok(()),
    }
}
fn worker_creation() -> WorkerCreationResolved<u64> {
    WorkerCreationResolved {
        proxy: 13,
        worker: 17,
        kind: CreationKind::ReplacementIncarnation { replaces: 16 },
        result: Ok(()),
    }
}

#[test]
fn composed_protocols_forward_every_supported_environment_lane() {
    assert!(matches!(
        ProxyEvent::<Lane>::creation_resolved(creation()),
        Some(ProxyEvent::CreationResolved(_))
    ));
    assert!(matches!(
        ProxyEvent::<Lane>::child_stopped(child()),
        Some(ProxyEvent::ChildStopped(_))
    ));

    assert!(matches!(
        DeadlineEvent::<Lane>::peer_stopped(peer()),
        Some(DeadlineEvent::Inner(Lane::Peer(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::child_stopped(child()),
        Some(DeadlineEvent::Inner(Lane::Child(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::worker_stopped(worker()),
        Some(DeadlineEvent::Inner(Lane::Worker(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::creation_resolved(creation()),
        Some(DeadlineEvent::Inner(Lane::Creation(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::worker_creation_resolved(worker_creation()),
        Some(DeadlineEvent::Inner(Lane::WorkerCreation(_)))
    ));

    assert!(matches!(
        WatchEvent::<Lane>::time_reached(elapsed()),
        Some(WatchEvent::Inner(Lane::Time(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane>::child_stopped(child()),
        Some(WatchEvent::Inner(Lane::Child(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane>::worker_stopped(worker()),
        Some(WatchEvent::Inner(Lane::Worker(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane>::creation_resolved(creation()),
        Some(WatchEvent::Inner(Lane::Creation(_)))
    ));

    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::time_reached(elapsed()),
        Some(ReceiveTimeoutEvent::Elapsed(_))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::peer_stopped(peer()),
        Some(ReceiveTimeoutEvent::Inner(Lane::Peer(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::child_stopped(child()),
        Some(ReceiveTimeoutEvent::Inner(Lane::Child(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::worker_stopped(worker()),
        Some(ReceiveTimeoutEvent::Inner(Lane::Worker(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::creation_resolved(creation()),
        Some(ReceiveTimeoutEvent::Inner(Lane::Creation(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::shutdown_requested(ShutdownRequested),
        Some(ReceiveTimeoutEvent::Inner(Lane::Shutdown))
    ));

    assert!(matches!(
        ShutdownProtocol::<Lane>::time_reached(elapsed()),
        Some(ShutdownProtocol::Inner(Lane::Time(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::peer_stopped(peer()),
        Some(ShutdownProtocol::Inner(Lane::Peer(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::child_stopped(child()),
        Some(ShutdownProtocol::Inner(Lane::Child(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::worker_stopped(worker()),
        Some(ShutdownProtocol::Inner(Lane::Worker(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::creation_resolved(creation()),
        Some(ShutdownProtocol::Inner(Lane::Creation(_)))
    ));

    assert!(matches!(
        SupervisionEvent::<Lane>::child_stopped(child()),
        Some(SupervisionEvent::ChildStopped(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::worker_stopped(worker()),
        Some(SupervisionEvent::WorkerStopped(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::creation_resolved(creation()),
        Some(SupervisionEvent::CreationResolved(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::worker_creation_resolved(worker_creation()),
        Some(SupervisionEvent::WorkerCreationResolved(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::time_reached(elapsed()),
        Some(SupervisionEvent::Inner(Lane::Time(_)))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::peer_stopped(peer()),
        Some(SupervisionEvent::Inner(Lane::Peer(_)))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::shutdown_requested(ShutdownRequested),
        Some(SupervisionEvent::Inner(Lane::Shutdown))
    ));
}

#[test]
fn addressing_operations_preserve_their_exact_routes() {
    let parent = MailAddr(0xF0);
    assert_eq!(u64::from(parent), 0xF0);
    assert_eq!(
        parent.birth(2),
        MailAddr(0xF0 ^ 2_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    );

    let one = Recipient::<MailAddr, u8>::global(MailAddr(1));
    let same = Recipient::<MailAddr, u8>::global(MailAddr(1));
    let other = Recipient::<MailAddr, u8>::global(MailAddr(2));
    let child = Recipient::<MailAddr, u8>::child(1);
    assert_eq!(one, same);
    assert_ne!(one, other);
    assert_ne!(one, child);
    assert_eq!(format!("{one:?}"), "Global(MailAddr(1))");
}

#[test]
fn named_wrapper_products_append_their_owned_lanes() {
    let mut timeout = ReceiveTimeoutSends::<Vec<u8>>::empty();
    timeout.append(ReceiveTimeoutSends {
        behavior: vec![3],
        schedules: ServiceSends::one(ScheduleAfter::new(
            TimerId(4),
            TimerGeneration(5),
            Duration::from_secs(6),
        )),
    });
    assert_eq!(timeout.behavior, [3]);
    assert_eq!(timeout.schedules.len(), 1);

    let mut proxy = ProxySends::<MailAddr, u8>::empty();
    proxy.append(ProxySends {
        deliveries: Vec::new(),
        child_observations: ServiceSends::one(ObserveChild::new(7)),
        creation_observations: ServiceSends::empty(),
        stopped_reports: ServiceSends::empty(),
        creation_reports: ServiceSends::empty(),
    });
    assert_eq!(proxy.child_observations[0].nonce, 7);
}

#[test]
fn service_send_views_and_iterators_preserve_every_request() {
    let sends = ServiceSends::new(vec![3, 5, 8]);
    assert_eq!(sends.as_slice(), &[3, 5, 8]);
    assert!(!sends.is_empty());
    assert_eq!(sends.clone().into_requests(), vec![3, 5, 8]);
    assert_eq!(sends.clone().into_iter().collect::<Vec<_>>(), vec![3, 5, 8]);
    assert_eq!(
        (&sends).into_iter().copied().collect::<Vec<_>>(),
        vec![3, 5, 8]
    );
}
