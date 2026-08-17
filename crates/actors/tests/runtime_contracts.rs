//! Compile-time manifest for every interpreter-originated template lane.
//!
//! These assertions model the universal adapter boundary in
//! `docs/adapter-contract.md`: a request-producing template must accept the
//! exact fact returned by that request, and a protocol-indexed shutdown target
//! must accept `ShutdownRequested` through its concrete event sum.

use behavior_actors::{
    Actions, BackoffSupervisor, Behavior, BehaviorActed, Births, BreakerOutcome,
    ChildShutdownRejected, ChildStopped, CircuitBreaker, CreationResolved, Deadline, DynamicProxy,
    DynamicSupervisor, DynamicSupervisorOutcome, EventInput, Lease, LeaseOutcome, MailAddr, Never,
    NoBirths, OneShot, PeerStopped, Periodic, Presence, PresenceReply, Proxy, ProxyCommand,
    ProxyEvent, ReceiveTimeout, ShutdownCoordinator, ShutdownCoordinatorEvent, ShutdownRequested,
    StopOnShutdown, SupervisionEvent, TerminationMonitor, TimerElapsed, User, Watch, WatchEvent,
    WorkerCreationResolved, WorkerStopped,
};

struct Inert;

impl Behavior for Inert {
    type Addr = MailAddr;
    type Msg = ();
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct BreakerReply;
impl Behavior for BreakerReply {
    type Addr = MailAddr;
    type Msg = BreakerOutcome;
    type Event = User<MailAddr, BreakerOutcome>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct LeaseReply;
impl Behavior for LeaseReply {
    type Addr = MailAddr;
    type Msg = LeaseOutcome<u8>;
    type Event = User<MailAddr, Self::Msg>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct PresenceReplyBehavior;
impl Behavior for PresenceReplyBehavior {
    type Addr = MailAddr;
    type Msg = PresenceReply<u8>;
    type Event = User<MailAddr, Self::Msg>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct DynamicReply;
impl Behavior for DynamicReply {
    type Addr = MailAddr;
    type Msg = DynamicSupervisorOutcome<u64, Inert>;
    type Event = User<MailAddr, Self::Msg>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Parent;
impl Behavior for Parent {
    type Addr = MailAddr;
    type Msg = ();
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<StopOnShutdown<Inert>>;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn accepts<B, Input>()
where
    B: Behavior,
    B::Event: EventInput<Input>,
{
}

fn event_accepts<E, Input>()
where
    E: EventInput<Input>,
{
}

#[test]
fn every_timer_request_has_an_exact_timer_fact_input() {
    accepts::<CircuitBreaker<MailAddr, BreakerReply>, TimerElapsed>();
    accepts::<Lease<MailAddr, u8, LeaseReply>, TimerElapsed>();
    accepts::<Presence<MailAddr, u8, PresenceReplyBehavior>, TimerElapsed>();
    accepts::<Deadline<Inert>, TimerElapsed>();
    accepts::<OneShot<Inert>, TimerElapsed>();
    accepts::<Periodic<Inert>, TimerElapsed>();
    accepts::<ReceiveTimeout<Inert>, TimerElapsed>();
    accepts::<BackoffSupervisor<Parent, StopOnShutdown<Inert>>, TimerElapsed>();
}

#[test]
fn every_observation_and_parent_report_has_an_exact_fact_input() {
    accepts::<Watch<Inert>, PeerStopped<MailAddr>>();
    accepts::<TerminationMonitor<Inert>, PeerStopped<MailAddr>>();
    accepts::<Proxy<Inert>, ChildStopped<MailAddr>>();
    accepts::<Proxy<Inert>, CreationResolved<u64>>();

    type Dynamic = DynamicSupervisor<MailAddr, Inert, DynamicReply>;
    accepts::<Dynamic, ChildStopped<MailAddr>>();
    accepts::<Dynamic, CreationResolved<u64>>();
    accepts::<Dynamic, WorkerStopped<MailAddr>>();
    accepts::<Dynamic, WorkerCreationResolved<u64>>();
    accepts::<Dynamic, ChildShutdownRejected<u64>>();

    type ProxyProtocol = ProxyEvent<User<MailAddr, ProxyCommand<Inert>>>;
    event_accepts::<ProxyProtocol, ChildStopped<MailAddr>>();
    event_accepts::<ProxyProtocol, CreationResolved<u64>>();

    type SupervisorProtocol = SupervisionEvent<User<MailAddr, ()>>;
    event_accepts::<SupervisorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<SupervisorProtocol, CreationResolved<u64>>();
    event_accepts::<SupervisorProtocol, WorkerStopped<MailAddr>>();
    event_accepts::<SupervisorProtocol, WorkerCreationResolved<u64>>();

    type WatchProtocol = WatchEvent<User<MailAddr, ()>>;
    event_accepts::<WatchProtocol, PeerStopped<MailAddr>>();
}

#[test]
fn every_shutdown_request_names_a_shutdown_capable_child_protocol() {
    accepts::<DynamicProxy<Inert>, ShutdownRequested>();
    accepts::<StopOnShutdown<Inert>, ShutdownRequested>();

    type CoordinatorProtocol = ShutdownCoordinatorEvent<User<MailAddr, ()>>;
    event_accepts::<CoordinatorProtocol, ShutdownRequested>();
    event_accepts::<CoordinatorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<CoordinatorProtocol, ChildShutdownRejected<u64>>();

    fn coordinator_is_closed<B: Behavior>() {}
    coordinator_is_closed::<ShutdownCoordinator<Parent, StopOnShutdown<Inert>>>();
}
