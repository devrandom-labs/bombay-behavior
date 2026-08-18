//! Compile-time manifest for every interpreter-originated template lane.
//!
//! These assertions model the universal adapter boundary in
//! `docs/adapter-contract.md`: a request-producing template must accept the
//! exact fact returned by that request, and a protocol-indexed shutdown target
//! must accept `ShutdownRequested` through its concrete event sum.

use behavior_actors::{
    Actions, BackoffSupervisor, Behavior, BehaviorActed, Births, BreakerOutcome,
    ChildShutdownRejected, ChildStopped, CircuitBreaker, CreationKind, CreationResolved, Deadline,
    DynamicProxy, DynamicSupervisor, DynamicSupervisorOutcome, Here, Ingress, InjectEvent, Lease,
    LeaseOutcome, MailAddr, Never, NoBirths, ObserveChild, ObserveCreation, ObservePeer, OneShot,
    PeerStopped, Periodic, Presence, PresenceReply, Proxy, ProxyCommand, ProxyEvent,
    ReceiveTimeout, ReportWorkerCreationResolved, ReportWorkerStopped, ScheduleAfter, ScheduleAt,
    ShutdownChild, ShutdownCoordinator, ShutdownCoordinatorEvent, ShutdownRequested,
    StopOnShutdown, SupervisionEvent, TerminationMonitor, TimerElapsed, TimerGeneration, TimerId,
    User, Watch, WatchEvent, WorkerCreationResolved, WorkerStopped,
};
use std::time::{Duration, Instant};

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

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct BreakerReply;
impl behavior::Protocol for BreakerReply {
    type Addr = MailAddr;
    type Msg = BreakerOutcome;
}

impl Behavior for BreakerReply {
    type Protocol = Self;
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
impl behavior::Protocol for LeaseReply {
    type Addr = MailAddr;
    type Msg = LeaseOutcome<u8>;
}

impl Behavior for LeaseReply {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
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
impl behavior::Protocol for PresenceReplyBehavior {
    type Addr = MailAddr;
    type Msg = PresenceReply<u8>;
}

impl Behavior for PresenceReplyBehavior {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
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
impl behavior::Protocol for DynamicReply {
    type Addr = MailAddr;
    type Msg = DynamicSupervisorOutcome<MailAddr, Inert>;
}

impl Behavior for DynamicReply {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
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
    B::Event: InjectEvent<Input, Here>,
{
}

fn event_accepts<E, Input>()
where
    E: InjectEvent<Input, Here>,
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
    accepts::<Proxy<Inert>, CreationResolved<MailAddr>>();
    accepts::<Proxy<Inert>, ShutdownRequested>();
    accepts::<Proxy<Inert>, ChildShutdownRejected<u64>>();

    type Dynamic = DynamicSupervisor<MailAddr, Inert, DynamicReply>;
    accepts::<Dynamic, ChildStopped<MailAddr>>();
    accepts::<Dynamic, CreationResolved<MailAddr>>();
    accepts::<Dynamic, WorkerStopped<MailAddr>>();
    accepts::<Dynamic, WorkerCreationResolved<u64>>();
    accepts::<Dynamic, ChildShutdownRejected<u64>>();

    type ProxyProtocol = ProxyEvent<User<MailAddr, ProxyCommand<Inert>>>;
    event_accepts::<ProxyProtocol, ChildStopped<MailAddr>>();
    event_accepts::<ProxyProtocol, CreationResolved<MailAddr>>();
    event_accepts::<ProxyProtocol, ShutdownRequested>();
    event_accepts::<ProxyProtocol, ChildShutdownRejected<u64>>();

    type SupervisorProtocol = SupervisionEvent<User<MailAddr, ()>>;
    event_accepts::<SupervisorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<SupervisorProtocol, CreationResolved<MailAddr>>();
    event_accepts::<SupervisorProtocol, WorkerStopped<MailAddr>>();
    event_accepts::<SupervisorProtocol, WorkerCreationResolved<u64>>();

    type WatchProtocol = WatchEvent<User<MailAddr, ()>>;
    event_accepts::<WatchProtocol, PeerStopped<MailAddr>>();
}

#[test]
fn every_shutdown_request_names_a_shutdown_capable_child_protocol() {
    accepts::<DynamicProxy<Inert>, ShutdownRequested>();
    accepts::<StopOnShutdown<Inert>, ShutdownRequested>();

    // The wrapper owns the added lane, so the inner dynamic supervisor does
    // not need to pretend that it implements coordinated shutdown.
    type Dynamic = DynamicSupervisor<MailAddr, Inert, DynamicReply>;
    accepts::<StopOnShutdown<Dynamic>, ShutdownRequested>();

    type CoordinatorProtocol = ShutdownCoordinatorEvent<User<MailAddr, ()>>;
    event_accepts::<CoordinatorProtocol, ShutdownRequested>();
    event_accepts::<CoordinatorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<CoordinatorProtocol, ChildShutdownRejected<u64>>();

    fn coordinator_is_closed<B: Behavior>() {}
    coordinator_is_closed::<ShutdownCoordinator<Parent, StopOnShutdown<Inert>>>();
}

#[test]
fn shutdown_wrapper_owns_dynamic_supervisor_shutdown_end_to_end() {
    use behavior_actors::Activate as _;

    type Dynamic = DynamicSupervisor<MailAddr, Inert, DynamicReply>;
    let mut active = StopOnShutdown::new(Dynamic::new())
        .initialize()
        .unwrap()
        .behavior;

    let actions = active.on(ShutdownRequested).unwrap();
    assert!(matches!(actions.become_, behavior_actors::Step::Stop(_)));
}

#[test]
fn every_deferred_local_fact_request_carries_its_here_destination() {
    fn here<Input>(_: Ingress<Input, Here>) {}

    here(ScheduleAt::new(TimerId(1), TimerGeneration(2), Instant::now()).ingress);
    here(ScheduleAfter::new(TimerId(1), TimerGeneration(2), Duration::from_secs(1)).ingress);
    here(ObservePeer::new(MailAddr(3)).ingress);
    here(ObserveChild::<MailAddr>::new(4).ingress);
    here(ObserveCreation::<MailAddr>::new(5).ingress);
    here(
        ReportWorkerStopped::<MailAddr>::new(6, Ok(behavior_actors::Exit::Normal), Instant::now())
            .ingress,
    );
    here(ReportWorkerCreationResolved::new(7_u64, CreationKind::Birth, Ok(())).ingress);

    let shutdown = ShutdownChild::<StopOnShutdown<Inert>>::new(8);
    here(shutdown.ingress);
    here(shutdown.rejection);
}
