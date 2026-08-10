use behavior::{
    Address, AtEvent, ChildEvent, ChildStopped, CreationEvent, CreationKind, CreationResolved,
    Exit, MailAddr, PeerEvent, PeerStopped, ReceiveTimeoutEvent, Recipient, ServiceSends,
    ShutdownEvent, ShutdownProtocol, ShutdownRequested, SupervisionEvent, TimeEvent, TimerElapsed,
    TimerGeneration, TimerId, WatchEvent, WorkerCreationEvent, WorkerCreationResolved, WorkerEvent,
    WorkerStopped,
};
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lane {
    Time(TimerElapsed),
    Peer(PeerStopped<MailAddr>),
    Child(ChildStopped<MailAddr>),
    Worker(WorkerStopped<MailAddr>),
    Creation(CreationResolved<MailAddr>),
    WorkerCreation(WorkerCreationResolved<MailAddr>),
    Shutdown,
}

impl TimeEvent for Lane {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        Some(Self::Time(event))
    }
}
impl PeerEvent<MailAddr> for Lane {
    fn peer_stopped(event: PeerStopped<MailAddr>) -> Option<Self> {
        Some(Self::Peer(event))
    }
}
impl ChildEvent<MailAddr> for Lane {
    fn child_stopped(event: ChildStopped<MailAddr>) -> Option<Self> {
        Some(Self::Child(event))
    }
}
impl WorkerEvent<MailAddr> for Lane {
    fn worker_stopped(event: WorkerStopped<MailAddr>) -> Option<Self> {
        Some(Self::Worker(event))
    }
}
impl CreationEvent<MailAddr> for Lane {
    fn creation_resolved(event: CreationResolved<MailAddr>) -> Option<Self> {
        Some(Self::Creation(event))
    }
}
impl WorkerCreationEvent<MailAddr> for Lane {
    fn worker_creation_resolved(event: WorkerCreationResolved<MailAddr>) -> Option<Self> {
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
fn creation() -> CreationResolved<MailAddr> {
    CreationResolved {
        nonce: 17,
        kind: CreationKind::ReplacementIncarnation { replaces: 16 },
        result: Ok(()),
    }
}
fn worker_creation() -> WorkerCreationResolved<MailAddr> {
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
        AtEvent::<Lane>::peer_stopped(peer()),
        Some(AtEvent::Inner(Lane::Peer(_)))
    ));
    assert!(matches!(
        AtEvent::<Lane>::child_stopped(child()),
        Some(AtEvent::Inner(Lane::Child(_)))
    ));
    assert!(matches!(
        AtEvent::<Lane>::worker_stopped(worker()),
        Some(AtEvent::Inner(Lane::Worker(_)))
    ));
    assert!(matches!(
        AtEvent::<Lane>::creation_resolved(creation()),
        Some(AtEvent::Inner(Lane::Creation(_)))
    ));
    assert!(matches!(
        AtEvent::<Lane>::worker_creation_resolved(worker_creation()),
        Some(AtEvent::Inner(Lane::WorkerCreation(_)))
    ));

    assert!(matches!(
        WatchEvent::<Lane, MailAddr>::time_reached(elapsed()),
        Some(WatchEvent::Inner(Lane::Time(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane, MailAddr>::child_stopped(child()),
        Some(WatchEvent::Inner(Lane::Child(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane, MailAddr>::worker_stopped(worker()),
        Some(WatchEvent::Inner(Lane::Worker(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane, MailAddr>::creation_resolved(creation()),
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
        SupervisionEvent::<Lane, MailAddr>::child_stopped(child()),
        Some(SupervisionEvent::ChildStopped(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane, MailAddr>::worker_stopped(worker()),
        Some(SupervisionEvent::WorkerStopped(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane, MailAddr>::creation_resolved(creation()),
        Some(SupervisionEvent::CreationResolved(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane, MailAddr>::worker_creation_resolved(worker_creation()),
        Some(SupervisionEvent::WorkerCreationResolved(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane, MailAddr>::time_reached(elapsed()),
        Some(SupervisionEvent::Inner(Lane::Time(_)))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane, MailAddr>::peer_stopped(peer()),
        Some(SupervisionEvent::Inner(Lane::Peer(_)))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane, MailAddr>::shutdown_requested(ShutdownRequested),
        Some(SupervisionEvent::Inner(Lane::Shutdown))
    ));
}

#[test]
fn addressing_operations_preserve_their_exact_routes() {
    let parent = MailAddr(0xF0);
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
