#![allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    reason = "fixture methods intentionally match the fallible behavior macro contract"
)]

use behavior::{
    Acted, Actions, Address, ChildStopped, CreationKind, CreationResolved, DeadlineEvent,
    DeadlineSends, Delivery, Exit, MailAddr, Never, ObserveChild, ObserveCreation, ObservePeer,
    PeerStopped, ProxyCommand, ProxyEvent, ProxySends, ReceiveTimeoutEvent, ReceiveTimeoutSends,
    Recipient, ReportWorkerCreationResolved, ReportWorkerStopped, RouteInput, ScheduleAfter,
    ScheduleAt, SendAlgebra, ServiceSends, ShutdownProtocol, ShutdownRequested, SupervisionEvent,
    SupervisorSends, TimerElapsed, TimerGeneration, TimerId, UnwatchPeer, User, UserEvent,
    WatchEvent, WatchSends, WorkerCreationResolved, WorkerStopped,
};
use behavior_actors as behavior;
use std::time::Duration;
use std::time::Instant;

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

impl RouteInput<TimerElapsed> for Lane {
    fn route(event: TimerElapsed) -> Result<Self, TimerElapsed> {
        Ok(Self::Time(event))
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
impl RouteInput<PeerStopped<MailAddr>> for Lane {
    fn route(event: PeerStopped<MailAddr>) -> Result<Self, PeerStopped<MailAddr>> {
        Ok(Self::Peer(event))
    }
}
impl RouteInput<ChildStopped<MailAddr>> for Lane {
    fn route(event: ChildStopped<MailAddr>) -> Result<Self, ChildStopped<MailAddr>> {
        Ok(Self::Child(event))
    }
}
impl RouteInput<WorkerStopped<MailAddr>> for Lane {
    fn route(event: WorkerStopped<MailAddr>) -> Result<Self, WorkerStopped<MailAddr>> {
        Ok(Self::Worker(event))
    }
}
impl RouteInput<CreationResolved<u64>> for Lane {
    fn route(event: CreationResolved<u64>) -> Result<Self, CreationResolved<u64>> {
        Ok(Self::Creation(event))
    }
}
impl RouteInput<WorkerCreationResolved<u64>> for Lane {
    fn route(event: WorkerCreationResolved<u64>) -> Result<Self, WorkerCreationResolved<u64>> {
        Ok(Self::WorkerCreation(event))
    }
}
impl RouteInput<ShutdownRequested> for Lane {
    fn route(_: ShutdownRequested) -> Result<Self, ShutdownRequested> {
        Ok(Self::Shutdown)
    }
}

struct Quiet;

#[behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = Vec<Never>,
    births = behavior::NoBirths,
    error = Never,
)]
impl Quiet {
    fn receive(
        &mut self,
        _: MailAddr,
        _: u8,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
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
#[allow(
    clippy::too_many_lines,
    reason = "one mutation contract exhaustively checks every environment lane"
)]
fn composed_protocols_forward_every_supported_environment_lane() {
    assert!(matches!(
        ProxyEvent::<Lane>::route(creation()),
        Ok(ProxyEvent::CreationResolved(_))
    ));
    assert!(matches!(
        ProxyEvent::<Lane>::route(child()),
        Ok(ProxyEvent::ChildStopped(_))
    ));

    assert!(matches!(
        DeadlineEvent::<Lane>::route(peer()),
        Ok(DeadlineEvent::Behavior(Lane::Peer(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::route(child()),
        Ok(DeadlineEvent::Behavior(Lane::Child(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::route(worker()),
        Ok(DeadlineEvent::Behavior(Lane::Worker(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::route(creation()),
        Ok(DeadlineEvent::Behavior(Lane::Creation(_)))
    ));
    assert!(matches!(
        DeadlineEvent::<Lane>::route(worker_creation()),
        Ok(DeadlineEvent::Behavior(Lane::WorkerCreation(_)))
    ));

    assert!(matches!(
        WatchEvent::<Lane>::route(elapsed()),
        Ok(WatchEvent::Behavior(Lane::Time(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane>::route(child()),
        Ok(WatchEvent::Behavior(Lane::Child(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane>::route(worker()),
        Ok(WatchEvent::Behavior(Lane::Worker(_)))
    ));
    assert!(matches!(
        WatchEvent::<Lane>::route(creation()),
        Ok(WatchEvent::Behavior(Lane::Creation(_)))
    ));

    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::route(elapsed()),
        Ok(ReceiveTimeoutEvent::Elapsed(_))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::route(peer()),
        Ok(ReceiveTimeoutEvent::Behavior(Lane::Peer(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::route(child()),
        Ok(ReceiveTimeoutEvent::Behavior(Lane::Child(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::route(worker()),
        Ok(ReceiveTimeoutEvent::Behavior(Lane::Worker(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::route(creation()),
        Ok(ReceiveTimeoutEvent::Behavior(Lane::Creation(_)))
    ));
    assert!(matches!(
        ReceiveTimeoutEvent::<Lane>::route(ShutdownRequested),
        Ok(ReceiveTimeoutEvent::Behavior(Lane::Shutdown))
    ));

    assert!(matches!(
        ShutdownProtocol::<Lane>::route(elapsed()),
        Ok(ShutdownProtocol::Behavior(Lane::Time(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::route(peer()),
        Ok(ShutdownProtocol::Behavior(Lane::Peer(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::route(child()),
        Ok(ShutdownProtocol::Behavior(Lane::Child(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::route(worker()),
        Ok(ShutdownProtocol::Behavior(Lane::Worker(_)))
    ));
    assert!(matches!(
        ShutdownProtocol::<Lane>::route(creation()),
        Ok(ShutdownProtocol::Behavior(Lane::Creation(_)))
    ));

    assert!(matches!(
        SupervisionEvent::<Lane>::route(child()),
        Ok(SupervisionEvent::ChildStopped(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::route(worker()),
        Ok(SupervisionEvent::WorkerStopped(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::route(creation()),
        Ok(SupervisionEvent::CreationResolved(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::route(worker_creation()),
        Ok(SupervisionEvent::WorkerCreationResolved(_))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::route(elapsed()),
        Ok(SupervisionEvent::Behavior(Lane::Time(_)))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::route(peer()),
        Ok(SupervisionEvent::Behavior(Lane::Peer(_)))
    ));
    assert!(matches!(
        SupervisionEvent::<Lane>::route(ShutdownRequested),
        Ok(SupervisionEvent::Behavior(Lane::Shutdown))
    ));
}

#[test]
fn addressing_operations_preserve_their_exact_routes() {
    type Child = Quiet;
    let parent = MailAddr(0xF0);
    assert_eq!(u64::from(parent), 0xF0);
    assert_eq!(
        parent.birth(2),
        MailAddr(0xF0 ^ 2_u64.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    );

    let one = Recipient::<Child>::global(MailAddr(1));
    let same = Recipient::<Child>::global(MailAddr(1));
    let other = Recipient::<Child>::global(MailAddr(2));
    let child = Recipient::<Child>::child(1);
    assert_eq!(one, same);
    assert_ne!(one, other);
    assert_ne!(one, child);
    assert_eq!(format!("{one:?}"), "Global(MailAddr(1))");
}

#[test]
fn named_wrapper_products_append_their_owned_lanes() {
    let mut timeout = ReceiveTimeoutSends::<Vec<u8>>::empty();
    timeout.append(ReceiveTimeoutSends::sending(ScheduleAfter::new(
        TimerId(4),
        TimerGeneration(5),
        Duration::from_secs(6),
    )));
    assert_eq!(timeout.schedules.len(), 1);

    let mut proxy = ProxySends::<Quiet>::empty();
    proxy.append(ProxySends::sending(ObserveChild::new(7)));
    assert_eq!(proxy.child_observations[0].nonce, 7);
}

#[test]
fn named_behavior_fields_reach_composed_lanes() {
    let at = Instant::now();
    let mut sends = WatchSends::<MailAddr, DeadlineSends<Vec<u8>>>::empty();
    sends
        .behavior
        .send(ScheduleAt::new(TimerId(8), TimerGeneration(9), at));

    assert!(sends.observations.is_empty());
    assert!(sends.behavior.behavior.is_empty());
    assert_eq!(sends.behavior.schedules[0].at, at);

    let mut watching = WatchSends::<MailAddr, ServiceSends<UnwatchPeer<MailAddr>>>::empty();
    watching.behavior.send(UnwatchPeer::new(MailAddr(12)));
    assert!(watching.observations.is_empty());
    assert_eq!(watching.behavior[0].peer, MailAddr(12));
}

#[test]
fn typed_send_accumulation_routes_every_named_lane_once() {
    type Child = Quiet;

    let mut values = Vec::<u8>::empty();
    values.send(3);
    assert_eq!(values, [3]);

    let mut watch = WatchSends::<MailAddr, Vec<u8>>::empty();
    watch.send(ObservePeer::new(MailAddr(4)));
    assert_eq!(watch.observations[0].peer, MailAddr(4));

    let mut cancellations = ServiceSends::<UnwatchPeer<MailAddr>>::empty();
    cancellations.send(UnwatchPeer::new(MailAddr(4)));
    assert_eq!(cancellations[0].peer, MailAddr(4));

    let mut deadline = DeadlineSends::<Vec<u8>>::empty();
    deadline.behavior.send(5_u8);
    assert_eq!(deadline.behavior, [5]);

    let mut timeout = ReceiveTimeoutSends::<Vec<u8>>::empty();
    timeout.behavior.send(6_u8);
    assert_eq!(timeout.behavior, [6]);

    let mut proxy = ProxySends::<Child>::empty();
    proxy.send(Delivery::new(Recipient::child(1), 7));
    proxy.send(ObserveCreation::new(2));
    proxy.send(ReportWorkerStopped::from(child()));
    proxy.send(ReportWorkerCreationResolved::from(creation()));
    assert_eq!(proxy.deliveries[0].message, 7);
    assert_eq!(proxy.creation_observations[0].nonce, 2);
    assert_eq!(proxy.stopped_reports[0].worker, 11);
    assert_eq!(proxy.creation_reports[0].worker, 17);

    let mut supervisor = SupervisorSends::<MailAddr, Vec<u8>, Child>::empty();
    supervisor.send(ObserveChild::new(8));
    supervisor.send(Delivery::new(
        Recipient::child(8),
        ProxyCommand::Replace(Quiet),
    ));
    supervisor.behavior.send(9_u8);
    assert_eq!(supervisor.child_observations[0].nonce, 8);
    assert_eq!(
        supervisor.replacement_commands[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 8)
    );
    assert_eq!(supervisor.behavior, [9]);
    assert!(supervisor.failure_reports.is_empty());
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
