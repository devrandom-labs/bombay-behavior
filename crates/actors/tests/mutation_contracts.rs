#![allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    reason = "fixture methods intentionally match the fallible behavior macro contract"
)]

use behavior::{
    Acted, Actions, ChildDelivery, ChildHead, ChildInput, ChildRoute, ChildStopped, CreationKind,
    CreationResolved, DeadlineEvent, Delivery, EventLayer, Exit, Here, InjectEvent, Inside,
    InterpreterRequests, MailAddr, Never, ObserveChild, ObserveCreation, ObservePeer, PeerStopped,
    ProxyEvent, ProxySends, ReceiveTimeoutEvent, Recipient, ReplacementRequested, ReportToParent,
    ReportWorkerCreationResolved, ReportWorkerStopped, ScheduleAfter, ScheduleAt, SendEffects,
    SendLayer, ShutdownEvent, ShutdownRequested, SupervisionEvent, SupervisorSends, TimerElapsed,
    TimerGeneration, TimerId, UnwatchPeer, User, UserEvent, WatchEvent, WorkerCreationResolved,
    WorkerStopped,
};
use behavior_actors as behavior;
use core::future::Future;
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
    let proxy = <ProxyEvent<Quiet> as InjectEvent<_, Here>>::inject_at(creation());
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
    let child = ChildRoute::<Child, ChildHead>::new(2);
    assert_eq!(child.nonce(), 2);

    let one = Recipient::<Child>::global(MailAddr(1));
    let same = Recipient::<Child>::global(MailAddr(1));
    let other = Recipient::<Child>::global(MailAddr(2));
    assert_eq!(one, same);
    assert_ne!(one, other);
    assert_eq!(format!("{one:?}"), "MailAddr(1)");
}

#[test]
fn named_wrapper_products_append_their_owned_lanes() {
    let mut timeout = SendLayer::<InterpreterRequests<ScheduleAfter>, Vec<u8>>::empty();
    timeout.append(SendLayer::sending(ScheduleAfter::new(
        TimerId(4),
        TimerGeneration(5),
        Duration::from_secs(6),
    )));
    assert_eq!(timeout.owned.len(), 1);

    let mut proxy = ProxySends::<Quiet>::empty();
    proxy.append(ProxySends::sending(ObserveChild::new(7)));
    assert_eq!(proxy.child_observations[0].nonce, 7);
}

#[test]
fn named_behavior_fields_reach_composed_lanes() {
    let at = Instant::now();
    let mut sends = SendLayer::<
        InterpreterRequests<ObservePeer<MailAddr>>,
        SendLayer<InterpreterRequests<ScheduleAt>, Vec<u8>>,
    >::empty();
    sends
        .inner
        .send(ScheduleAt::new(TimerId(8), TimerGeneration(9), at));

    assert!(sends.owned.is_empty());
    assert!(sends.inner.inner.is_empty());
    assert_eq!(sends.inner.owned[0].at, at);

    let mut watching = SendLayer::<
        InterpreterRequests<ObservePeer<MailAddr>>,
        InterpreterRequests<UnwatchPeer<MailAddr>>,
    >::empty();
    watching.inner.send(UnwatchPeer::new(MailAddr(12)));
    assert!(watching.owned.is_empty());
    assert_eq!(watching.inner[0].peer, MailAddr(12));
}

#[test]
fn typed_send_accumulation_routes_every_named_lane_once() {
    type Child = Quiet;

    let mut values = Vec::<u8>::empty();
    values.send(3);
    assert_eq!(values, [3]);

    let mut watch = SendLayer::<InterpreterRequests<ObservePeer<MailAddr>>, Vec<u8>>::empty();
    watch.send(ObservePeer::new(MailAddr(4)));
    assert_eq!(watch.owned[0].peer, MailAddr(4));

    let mut cancellations = InterpreterRequests::<UnwatchPeer<MailAddr>>::empty();
    cancellations.send(UnwatchPeer::new(MailAddr(4)));
    assert_eq!(cancellations[0].peer, MailAddr(4));

    let mut deadline = SendLayer::<InterpreterRequests<ScheduleAt>, Vec<u8>>::empty();
    deadline.inner.send(5_u8);
    assert_eq!(deadline.inner, [5]);

    let mut timeout = SendLayer::<InterpreterRequests<ScheduleAfter>, Vec<u8>>::empty();
    timeout.inner.send(6_u8);
    assert_eq!(timeout.inner, [6]);

    let mut proxy = ProxySends::<Child>::empty();
    proxy.send(ChildDelivery::at(ChildRoute::<Child, ChildHead>::new(1), 7));
    proxy.send(ObserveCreation::new(2));
    let stopped = child();
    proxy.send(ReportToParent::new(ReportWorkerStopped::new(
        stopped.nonce,
        stopped.outcome,
        stopped.at,
    )));
    let resolved = creation();
    proxy.send(ReportToParent::new(ReportWorkerCreationResolved::new(
        resolved.nonce,
        resolved.kind,
        resolved.result.map(|_| ()),
    )));
    assert_eq!(proxy.deliveries[0].message, 7);
    assert_eq!(proxy.creation_observations[0].nonce, 2);
    assert_eq!(proxy.stopped_reports[0].report.worker, 11);
    assert_eq!(proxy.creation_reports[0].report.worker, 17);

    let mut supervisor = SupervisorSends::<MailAddr, Child, behavior::Proxy<Child>>::empty();
    supervisor.send(ObserveChild::new(8));
    supervisor.send(ObserveCreation::new(8));
    supervisor.send(ChildInput::at(
        ChildRoute::<behavior::Proxy<Child>, ChildHead>::new(8),
        ReplacementRequested::new(Quiet),
    ));
    assert_eq!(supervisor.child_observations[0].nonce, 8);
    assert_eq!(supervisor.creation_observations[0].nonce, 8);
    assert_eq!(supervisor.replacement_inputs[0].nonce, 8);
    assert!(supervisor.failure_reports.is_empty());
}

#[test]
fn service_send_views_and_iterators_preserve_every_request() {
    let sends = InterpreterRequests::new(vec![3, 5, 8]);
    assert_eq!(sends.as_slice(), &[3, 5, 8]);
    assert!(!sends.is_empty());
    assert_eq!(sends.clone().into_requests(), vec![3, 5, 8]);
    assert_eq!(sends.clone().into_iter().collect::<Vec<_>>(), vec![3, 5, 8]);
    assert_eq!(
        (&sends).into_iter().copied().collect::<Vec<_>>(),
        vec![3, 5, 8]
    );
}

#[tokio::test]
async fn nested_actor_send_effects_traverse_every_lane_once_in_structural_order() {
    type Reply = behavior::MessageProtocol<MailAddr, behavior::BreakerOutcome>;
    type RootEvent = EventLayer<
        TimerElapsed,
        EventLayer<TimerElapsed, User<MailAddr, behavior::BreakerMessage<Reply>>>,
    >;

    #[derive(Debug, PartialEq, Eq)]
    enum Seen {
        Reply,
        Reset(TimerId),
        Deadline(TimerId),
    }

    struct RecordingInterpreter(Vec<Seen>);

    impl behavior::SendInterpreter for RecordingInterpreter {
        type Error = Never;
    }

    impl behavior::InterpretDelivery<Reply> for RecordingInterpreter {
        fn interpret_delivery(
            &mut self,
            _: Delivery<Reply>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Reply);
                Ok(())
            }
        }
    }

    impl behavior::InterpretRequest<ScheduleAfter, RootEvent, Inside<Here>> for RecordingInterpreter {
        fn interpret_request(
            &mut self,
            request: ScheduleAfter,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Reset(request.id));
                Ok(())
            }
        }
    }

    impl behavior::InterpretRequest<ScheduleAt, RootEvent, Here> for RecordingInterpreter {
        fn interpret_request(
            &mut self,
            request: ScheduleAt,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Deadline(request.id));
                Ok(())
            }
        }
    }

    let reply = Delivery::new(
        Recipient::global(MailAddr(9)),
        behavior::BreakerOutcome::Rejected(behavior::BreakerRejection::Open {
            generation: TimerGeneration(0),
        }),
    );
    let breaker = behavior::BreakerSends {
        replies: vec![reply],
        schedules: InterpreterRequests::one(ScheduleAfter::new(
            TimerId(2),
            TimerGeneration(0),
            Duration::from_secs(1),
        )),
    };
    let sends = SendLayer::new(
        InterpreterRequests::one(ScheduleAt::new(
            TimerId(3),
            TimerGeneration(0),
            Instant::now(),
        )),
        breaker,
    );

    let mut interpreter = RecordingInterpreter(Vec::new());
    <_ as behavior::InterpretSends<_, RootEvent, Here>>::interpret(sends, &mut interpreter)
        .await
        .unwrap();

    assert_eq!(
        interpreter.0,
        [
            Seen::Reply,
            Seen::Reset(TimerId(2)),
            Seen::Deadline(TimerId(3))
        ]
    );
}
