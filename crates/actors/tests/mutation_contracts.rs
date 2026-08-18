#![allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    reason = "fixture methods intentionally match the fallible behavior macro contract"
)]

use behavior::{
    Acted, Actions, Address, ChildStopped, CreationKind, CreationResolved, DeadlineEvent,
    DeadlineSends, Delivery, EventLayer, Exit, Here, InjectEvent, Inside, MailAddr, Never,
    ObserveChild, ObserveCreation, ObservePeer, PeerStopped, ProxyCommand, ProxyEvent, ProxySends,
    ReceiveTimeoutEvent, ReceiveTimeoutSends, Recipient, ReportWorkerCreationResolved,
    ReportWorkerStopped, ScheduleAfter, ScheduleAt, SendAlgebra, ServiceSends, ShutdownEvent,
    ShutdownRequested, SupervisionEvent, SupervisorSends, TimerElapsed, TimerGeneration, TimerId,
    UnwatchPeer, User, UserEvent, WatchEvent, WatchSends, WorkerCreationResolved, WorkerStopped,
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
    Creation(CreationResolved<MailAddr>),
    WorkerCreation(WorkerCreationResolved<u64>),
    Shutdown,
}

impl InjectEvent<TimerElapsed, Here> for Lane {
    fn inject_at(event: TimerElapsed) -> Self {
        Self::Time(event)
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
impl InjectEvent<PeerStopped<MailAddr>, Here> for Lane {
    fn inject_at(event: PeerStopped<MailAddr>) -> Self {
        Self::Peer(event)
    }
}
impl InjectEvent<ChildStopped<MailAddr>, Here> for Lane {
    fn inject_at(event: ChildStopped<MailAddr>) -> Self {
        Self::Child(event)
    }
}
impl InjectEvent<WorkerStopped<MailAddr>, Here> for Lane {
    fn inject_at(event: WorkerStopped<MailAddr>) -> Self {
        Self::Worker(event)
    }
}
impl InjectEvent<CreationResolved<MailAddr>, Here> for Lane {
    fn inject_at(event: CreationResolved<MailAddr>) -> Self {
        Self::Creation(event)
    }
}
impl InjectEvent<WorkerCreationResolved<u64>, Here> for Lane {
    fn inject_at(event: WorkerCreationResolved<u64>) -> Self {
        Self::WorkerCreation(event)
    }
}
impl InjectEvent<ShutdownRequested, Here> for Lane {
    fn inject_at(_: ShutdownRequested) -> Self {
        Self::Shutdown
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
fn creation() -> CreationResolved<MailAddr> {
    CreationResolved {
        nonce: 17,
        kind: CreationKind::ReplacementIncarnation { replaces: 16 },
        result: Ok(MailAddr(17)),
    }
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one mutation contract exhaustively checks every environment lane"
)]
fn structural_paths_select_owners_without_forwarding_lists() {
    let proxy = <ProxyEvent<Lane> as InjectEvent<_, Here>>::inject_at(creation());
    assert!(matches!(proxy, ProxyEvent::CreationResolved(_)));

    let deadline = <DeadlineEvent<Lane> as InjectEvent<_, Inside<Here>>>::inject_at(peer());
    assert!(matches!(deadline, EventLayer::Inner(Lane::Peer(_))));

    let timeout = <ReceiveTimeoutEvent<Lane> as InjectEvent<_, Here>>::inject_at(elapsed());
    assert!(matches!(timeout, EventLayer::Owned(_)));
    let nested_timeout =
        <ReceiveTimeoutEvent<Lane> as InjectEvent<_, Inside<Here>>>::inject_at(peer());
    assert!(matches!(nested_timeout, EventLayer::Inner(Lane::Peer(_))));

    let shutdown = <ShutdownEvent<Lane> as InjectEvent<_, Here>>::inject_at(ShutdownRequested);
    assert!(matches!(shutdown, EventLayer::Owned(_)));
    let nested_timer = <ShutdownEvent<Lane> as InjectEvent<_, Inside<Here>>>::inject_at(elapsed());
    assert!(matches!(nested_timer, EventLayer::Inner(Lane::Time(_))));

    let supervised = <SupervisionEvent<Lane> as InjectEvent<_, Here>>::inject_at(worker());
    assert!(matches!(supervised, SupervisionEvent::WorkerStopped(_)));
    let nested_peer = <SupervisionEvent<Lane> as InjectEvent<_, Inside<Here>>>::inject_at(peer());
    assert!(matches!(
        nested_peer,
        SupervisionEvent::Behavior(Lane::Peer(_))
    ));

    let watched = <WatchEvent<Lane> as InjectEvent<_, Inside<Here>>>::inject_at(child());
    assert!(matches!(watched, EventLayer::Inner(Lane::Child(_))));
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
    let child = behavior::DeliveryTarget::<Child>::LocalChild(behavior::ChildRecipient::new(1));
    assert_eq!(one, same);
    assert_ne!(one, other);
    assert_ne!(child, one);
    assert_eq!(format!("{one:?}"), "MailAddr(1)");
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
    proxy.send(Delivery::local_child(behavior::ChildRecipient::new(1), 7));
    proxy.send(ObserveCreation::new(2));
    proxy.send(ReportWorkerStopped::from(child()));
    proxy.send(ReportWorkerCreationResolved::from(creation()));
    assert_eq!(proxy.deliveries[0].message, 7);
    assert_eq!(proxy.creation_observations[0].nonce, 2);
    assert_eq!(proxy.stopped_reports[0].worker, 11);
    assert_eq!(proxy.creation_reports[0].worker, 17);

    let mut supervisor = SupervisorSends::<MailAddr, Vec<u8>, Child>::empty();
    supervisor.send(ObserveChild::new(8));
    supervisor.send(ObserveCreation::new(8));
    supervisor.send(Delivery::local_child(
        behavior::ChildRecipient::new(8),
        ProxyCommand::Replace(Quiet),
    ));
    supervisor.behavior.send(9_u8);
    assert_eq!(supervisor.child_observations[0].nonce, 8);
    assert_eq!(supervisor.creation_observations[0].nonce, 8);
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
