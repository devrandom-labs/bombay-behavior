#![allow(
    clippy::unnecessary_wraps,
    clippy::unused_self,
    reason = "fixture methods intentionally match the fallible behavior macro contract"
)]

use behavior::{
    Acted, Actions, ChildStopped, CreationSequence, DeadlineEvent, EventLayer, Exit, Here,
    InjectEvent, Inside, InterpreterRequests, MailAddr, Never, ObservePeer, PeerStopped,
    ReceiveTimeoutEvent, Recipient, ScheduleAfter, ScheduleAt, SendEffects, SendLayer,
    ShutdownEvent, ShutdownRequested, TimerElapsed, TimerGeneration, TimerId, UnwatchPeer, User,
    UserEvent, WatchEvent,
};
use behavior_actors as behavior;
use std::time::Duration;
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Lane {
    Time(TimerElapsed),
    Peer(PeerStopped<MailAddr>),
    Child(ChildStopped<MailAddr>),
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
    let child = CreationSequence::new()
        .issue()
        .expect("the first child creation ID exists");
    ChildStopped::new(child, Ok(Exit::Normal), Instant::now())
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one mutation contract exhaustively checks every environment lane"
)]
fn structural_paths_select_owners_without_forwarding_lists() {
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

    let watched = <WatchEvent<Lane> as InjectEvent<_, Inside<Here>>>::inject_at(child());
    assert!(matches!(watched, EventLayer::Inner(Lane::Child(_))));
}

#[test]
fn addressing_operations_preserve_their_exact_routes() {
    type Child = Quiet;
    let parent = MailAddr(0xF0);
    assert_eq!(u64::from(parent), 0xF0);

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
}

#[test]
fn typed_send_accumulation_routes_every_named_lane_once() {
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
