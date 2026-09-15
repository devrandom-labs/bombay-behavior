//! Compile-time manifest for reusable interpreter-originated inputs.
//!
//! Atomic aggregates own their contracts in their dedicated law suites. This
//! target retains the unrelated timer, observation, shutdown, and local-return
//! contracts shared across the catalogue.

use behavior::{
    Actions, Behavior, BehaviorActed, Births, ChildHead, CreationId, CreationSequence, Here,
    Ingress, InjectEvent, InterpreterRequest, MailAddr, Never, NoBirths, Recipient, User,
};
use behavior_actors::{
    BreakerOutcome, ChildShutdownRejected, ChildStopped, CircuitBreaker, CreationResolved,
    Deadline, InstallShutdownPlan, Lease, LeaseOutcome, ObserveChild, ObserveCreation, ObservePeer,
    OneShot, PeerStopped, Periodic, Presence, PresenceReply, ReceiveTimeout, ScheduleAfter,
    ScheduleAt, ShutdownChild, ShutdownCoordinator, ShutdownCoordinatorEvent, ShutdownPlan,
    ShutdownRequested, StopOnShutdown, TerminationMonitor, TimerElapsed, Watch, WatchEvent,
};

struct Inert;

impl behavior::Protocol for Inert {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for Inert {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct BreakerReply;

impl behavior::Protocol for BreakerReply {
    type Addr = MailAddr;
    type Msg = BreakerOutcome;
}

struct LeaseReply;

impl behavior::Protocol for LeaseReply {
    type Addr = MailAddr;
    type Msg = LeaseOutcome<u8>;
}

struct PresenceReplyBehavior;

impl behavior::Protocol for PresenceReplyBehavior {
    type Addr = MailAddr;
    type Msg = PresenceReply<u8>;
}

struct Parent;

impl behavior::Protocol for Parent {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for Parent {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<StopOnShutdown<Inert>>;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn accepts<B, Input>()
where
    B: Behavior,
    B::Event: InjectEvent<Input, Here>,
{
}

fn event_accepts<Event, Input>()
where
    Event: InjectEvent<Input, Here>,
{
}

fn first_creation() -> CreationId {
    let mut creations = CreationSequence::new();
    let Some(child) = creations.issue() else {
        panic!("the first creation ID is always available");
    };
    child
}

#[test]
fn timer_templates_accept_timer_elapsed() {
    accepts::<CircuitBreaker<MailAddr, Recipient<BreakerReply>>, TimerElapsed>();
    accepts::<Lease<MailAddr, u8, Recipient<LeaseReply>>, TimerElapsed>();
    accepts::<Presence<MailAddr, u8, Recipient<PresenceReplyBehavior>>, TimerElapsed>();
    accepts::<Deadline<Inert>, TimerElapsed>();
    accepts::<OneShot<Inert>, TimerElapsed>();
    accepts::<Periodic<Inert>, TimerElapsed>();
    accepts::<ReceiveTimeout<Inert>, TimerElapsed>();
}

#[test]
fn observation_templates_accept_peer_stopped() {
    accepts::<Watch<Inert>, PeerStopped<MailAddr>>();
    accepts::<TerminationMonitor<Inert>, PeerStopped<MailAddr>>();

    type WatchProtocol = WatchEvent<User<MailAddr, ()>>;
    event_accepts::<WatchProtocol, PeerStopped<MailAddr>>();
}

#[test]
fn shutdown_templates_accept_their_complete_inputs() {
    accepts::<StopOnShutdown<Inert>, ShutdownRequested>();

    type CoordinatorProtocol =
        ShutdownCoordinatorEvent<User<MailAddr, ()>, ShutdownPlan<CreationId>>;
    event_accepts::<CoordinatorProtocol, ShutdownRequested>();
    event_accepts::<CoordinatorProtocol, InstallShutdownPlan<ShutdownPlan<CreationId>>>();
    event_accepts::<CoordinatorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<CoordinatorProtocol, ChildShutdownRejected>();

    fn coordinator_is_closed<B: Behavior>() {}
    coordinator_is_closed::<ShutdownCoordinator<Parent, StopOnShutdown<Inert>, ChildHead>>();
}

#[test]
fn local_requests_return_to_the_emitting_actor() {
    fn returns_here<Request, Input>()
    where
        Request: InterpreterRequest<ReturnToEmitter = behavior::ReturnsToEmitter<Input, Here>>,
    {
    }

    returns_here::<ScheduleAt, TimerElapsed>();
    returns_here::<ScheduleAfter, TimerElapsed>();
    returns_here::<ObservePeer<MailAddr>, PeerStopped<MailAddr>>();
    returns_here::<ObserveChild<Inert, ChildHead>, ChildStopped<MailAddr>>();
    returns_here::<ObserveCreation<Inert, ChildHead>, CreationResolved<MailAddr>>();

    let shutdown = ShutdownChild::<StopOnShutdown<Inert>, ChildHead>::new(first_creation());
    fn here<Input>(_: Ingress<Input, Here>) {}
    here(shutdown.ingress);
    returns_here::<ShutdownChild<StopOnShutdown<Inert>, ChildHead>, ChildShutdownRejected>();
}
