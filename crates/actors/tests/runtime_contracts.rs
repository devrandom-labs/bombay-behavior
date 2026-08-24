//! Compile-time manifest for every interpreter-originated template lane.
//!
//! These assertions model the universal adapter boundary in
//! `docs/adapter-contract.md`: a request-producing template must accept the
//! exact fact returned by that request, and a protocol-indexed shutdown target
//! must accept `ShutdownRequested` through its concrete event sum.

use behavior_actors::{
    Actions, BackoffSupervise, BackoffSupervisorEvent, BackoffSupervisorSends,
    BackoffSupervisorWithParent, Behavior, BehaviorActed, BehaviorBase, Births, BreakerOutcome,
    ChildDelivery, ChildHead, ChildRoute, ChildShutdownRejected, ChildStopped, CircuitBreaker,
    Create, CreationResolved, Deadline, DynamicSupervisor, DynamicSupervisorOutcome,
    DynamicSupervisorWithParent, Guardian, Here, Ingress, InjectEvent, Inside, InstallShutdownPlan,
    InterpretChildDelivery, KeyedWorkerPool, KeyedWorkerPoolEvent, KeyedWorkerPoolProtocol,
    KeyedWorkerPoolWithParent, Lease, LeaseOutcome, MailAddr, Never, NoBirths, ObserveChild,
    ObserveCreation, ObservePeer, OneShot, PeerStopped, Periodic, PoolAssignment,
    PoolBehaviorSends, PoolResponse, PoolSends, Presence, PresenceReply, Proxy, ProxyCommand,
    ProxyEvent, ProxyParentIngress, ProxyWithParent, ReceiveTimeout, Recipient,
    ReportSupervisionFailure, ScheduleAfter, ScheduleAt, SendLayer, ShutdownChild,
    ShutdownCoordinator, ShutdownCoordinatorEvent, ShutdownPlan, ShutdownRequested, StopOnShutdown,
    Supervise, SupervisionEvent, SupervisorWithParent, TerminationMonitor, TimerElapsed, User,
    Watch, WatchEvent, WorkerCreationResolved, WorkerPool, WorkerPoolEvent, WorkerPoolProtocol,
    WorkerPoolWithParent, WorkerStopped,
};
use core::future::Future;

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
        Ok(Actions::create(vec![Create::birth(
            99,
            StopOnShutdown::new(Inert),
        )]))
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

fn event_accepts_at<E, Input, Path>()
where
    E: InjectEvent<Input, Path>,
{
}

fn behavior_accepts_at<B, Input, Path>()
where
    B: Behavior,
    B::Event: InjectEvent<Input, Path>,
{
}

#[test]
fn every_topology_owner_exposes_itself_as_its_behavior_base() {
    fn owns_topology<B>()
    where
        B: Behavior + BehaviorBase<Base = B>,
    {
    }

    type Child = StopOnShutdown<Inert>;
    owns_topology::<ProxyWithParent<Child, Here>>();
    owns_topology::<SupervisorWithParent<MailAddr, Child, Here>>();
    owns_topology::<DynamicSupervisorWithParent<MailAddr, Inert, Recipient<DynamicReply>, Here>>();
    owns_topology::<WorkerPoolWithParent<MailAddr, PoolReply, u8, u16, PoolWorker, PoolRoute, Here>>(
    );
    owns_topology::<
        KeyedWorkerPoolWithParent<
            MailAddr,
            PoolReply,
            u8,
            u8,
            u16,
            KeyedPoolWorker,
            PoolRoute,
            fn(&u8) -> u64,
            Here,
        >,
    >();
}

#[test]
fn every_timer_request_has_an_exact_timer_fact_input() {
    accepts::<CircuitBreaker<MailAddr, BreakerReply, Recipient<BreakerReply>>, TimerElapsed>();
    accepts::<Lease<MailAddr, u8, LeaseReply, Recipient<LeaseReply>>, TimerElapsed>();
    accepts::<
        Presence<MailAddr, u8, PresenceReplyBehavior, Recipient<PresenceReplyBehavior>>,
        TimerElapsed,
    >();
    accepts::<Deadline<Inert>, TimerElapsed>();
    accepts::<OneShot<Inert>, TimerElapsed>();
    accepts::<Periodic<Inert>, TimerElapsed>();
    accepts::<ReceiveTimeout<Inert>, TimerElapsed>();
    accepts::<BackoffSupervise<Parent, StopOnShutdown<Inert>>, TimerElapsed>();
}

#[test]
fn every_observation_and_parent_report_has_an_exact_fact_input() {
    accepts::<Watch<Inert>, PeerStopped<MailAddr>>();
    accepts::<TerminationMonitor<Inert>, PeerStopped<MailAddr>>();
    accepts::<Proxy<Inert>, ChildStopped<MailAddr>>();
    accepts::<Proxy<Inert>, CreationResolved<MailAddr>>();
    accepts::<Proxy<Inert>, ShutdownRequested>();
    accepts::<Proxy<Inert>, ChildShutdownRejected<u64>>();

    type Dynamic = DynamicSupervisor<MailAddr, Inert, Recipient<DynamicReply>>;
    accepts::<Dynamic, ChildStopped<MailAddr>>();
    accepts::<Dynamic, CreationResolved<MailAddr>>();
    accepts::<Dynamic, WorkerStopped<MailAddr>>();
    accepts::<Dynamic, WorkerCreationResolved<u64>>();
    accepts::<Dynamic, ShutdownRequested>();
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
    event_accepts::<SupervisorProtocol, ShutdownRequested>();
    event_accepts::<SupervisorProtocol, ChildShutdownRejected<u64>>();

    type WatchProtocol = WatchEvent<User<MailAddr, ()>>;
    event_accepts::<WatchProtocol, PeerStopped<MailAddr>>();
}

#[test]
fn every_shutdown_request_names_a_shutdown_capable_child_protocol() {
    accepts::<Proxy<Inert>, ShutdownRequested>();
    accepts::<StopOnShutdown<Inert>, ShutdownRequested>();

    // An explicit outer direct-stop policy still takes precedence over the
    // dynamic supervisor's own coordinated subtree shutdown.
    type Dynamic = DynamicSupervisor<MailAddr, Inert, Recipient<DynamicReply>>;
    accepts::<StopOnShutdown<Dynamic>, ShutdownRequested>();

    type CoordinatorProtocol = ShutdownCoordinatorEvent<User<MailAddr, ()>, ShutdownPlan<u64>>;
    event_accepts::<CoordinatorProtocol, ShutdownRequested>();
    event_accepts::<CoordinatorProtocol, InstallShutdownPlan<ShutdownPlan<u64>>>();
    event_accepts::<CoordinatorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<CoordinatorProtocol, ChildShutdownRejected<u64>>();

    fn coordinator_is_closed<B: Behavior>() {}
    coordinator_is_closed::<ShutdownCoordinator<Parent, StopOnShutdown<Inert>, ChildHead>>();

    type Fixed = Supervise<Parent, StopOnShutdown<Inert>>;
    behavior_accepts_at::<Fixed, ShutdownRequested, Here>();
    behavior_accepts_at::<Fixed, ChildShutdownRejected<u64>, Here>();

    type Delayed = BackoffSupervise<Parent, StopOnShutdown<Inert>>;
    behavior_accepts_at::<Delayed, TimerElapsed, Here>();
    behavior_accepts_at::<Delayed, ShutdownRequested, Here>();
    behavior_accepts_at::<Delayed, ChildStopped<MailAddr>, Here>();
    behavior_accepts_at::<Delayed, CreationResolved<MailAddr>, Here>();
    behavior_accepts_at::<Delayed, WorkerStopped<MailAddr>, Here>();
    behavior_accepts_at::<Delayed, WorkerCreationResolved<u64>, Here>();
    behavior_accepts_at::<Delayed, ChildShutdownRejected<u64>, Here>();

    fn coordinated_child_is_closed<B: Behavior>() {}
    coordinated_child_is_closed::<ShutdownCoordinator<Parent, Delayed, ChildHead>>();

    type Pool = WorkerPoolEvent<MailAddr, PoolReply, u8, u16, PoolRoute>;
    event_accepts_at::<Pool, ShutdownRequested, Here>();
    event_accepts_at::<Pool, ChildShutdownRejected<u64>, Here>();

    type KeyedPool = KeyedWorkerPoolEvent<MailAddr, PoolReply, u8, u8, u16, PoolRoute>;
    event_accepts_at::<KeyedPool, ShutdownRequested, Here>();
    event_accepts_at::<KeyedPool, ChildShutdownRejected<u64>, Here>();

    type ConcretePool = WorkerPool<MailAddr, PoolReply, u8, u16, PoolWorker, PoolRoute>;
    behavior_accepts_at::<ConcretePool, ShutdownRequested, Here>();
    behavior_accepts_at::<ConcretePool, ChildShutdownRejected<u64>, Here>();

    type ConcreteKeyedPool = KeyedWorkerPool<
        MailAddr,
        PoolReply,
        u8,
        u8,
        u16,
        KeyedPoolWorker,
        PoolRoute,
        fn(&u8) -> u64,
    >;
    behavior_accepts_at::<ConcreteKeyedPool, ShutdownRequested, Here>();
    behavior_accepts_at::<ConcreteKeyedPool, ChildShutdownRejected<u64>, Here>();
}

#[test]
fn shutdown_wrapper_owns_dynamic_supervisor_shutdown_end_to_end() {
    use behavior_actors::Activate as _;

    type Dynamic = DynamicSupervisor<MailAddr, Inert, Recipient<DynamicReply>>;
    let mut active = StopOnShutdown::new(Dynamic::new())
        .initialize()
        .unwrap()
        .behavior;

    let actions = active.on(ShutdownRequested).unwrap();
    assert!(matches!(actions.become_, behavior_actors::Step::Stop(_)));
}

#[test]
fn wrapped_dynamic_supervisor_gives_proxies_the_inner_parent_ingress() {
    use behavior_actors::Activate as _;
    use std::time::Instant;

    type ParentPath = Inside<Here>;
    type Inner = DynamicSupervisorWithParent<MailAddr, Inert, Recipient<DynamicReply>, ParentPath>;
    type Wrapped = StopOnShutdown<Inner>;

    let direct = ProxyParentIngress::<MailAddr, Here>::new();
    let wrapped: Wrapped = StopOnShutdown::new(Inner::with_parent(direct.inside()));

    fn exact_birth<B>()
    where
        B: Behavior<Birth = Births<ProxyWithParent<Inert, Inside<Here>>>>,
    {
    }
    exact_birth::<Wrapped>();
    let _ = wrapped;

    fn stopped(_: Ingress<WorkerStopped<MailAddr>, Inside<Here>>) {}
    fn creation(_: Ingress<WorkerCreationResolved<u64>, Inside<Here>>) {}
    stopped(direct.inside().stopped);
    creation(direct.inside().creation);

    let initialized = ProxyWithParent::with_parent(Inert, direct.inside())
        .initialize()
        .unwrap();
    let mut proxy = initialized.behavior;
    let created = proxy
        .on_path(CreationResolved::birth(0, MailAddr(1)))
        .unwrap();
    creation(created.sends.creation_reports[0].ingress);
    let stopped_actions = proxy
        .on_path(ChildStopped::new(
            0,
            Ok(behavior_actors::Exit::Normal),
            Instant::now(),
        ))
        .unwrap();
    stopped(stopped_actions.sends.stopped_reports[0].ingress);
}

#[test]
fn every_proxy_owner_reindexes_parent_reports_through_an_outer_guardian() {
    type ParentPath = Inside<Here>;
    type Child = StopOnShutdown<Inert>;
    type Fixed = Guardian<SupervisorWithParent<MailAddr, Child, ParentPath>>;
    type Delayed = Guardian<BackoffSupervisorWithParent<MailAddr, Child, ParentPath>>;
    type Dynamic =
        Guardian<DynamicSupervisorWithParent<MailAddr, Inert, Recipient<DynamicReply>, ParentPath>>;
    type Fifo = Guardian<
        WorkerPoolWithParent<MailAddr, PoolReply, u8, u16, PoolWorker, PoolRoute, ParentPath>,
    >;
    type Keyed = Guardian<
        KeyedWorkerPoolWithParent<
            MailAddr,
            PoolReply,
            u8,
            u8,
            u16,
            KeyedPoolWorker,
            PoolRoute,
            fn(&u8) -> u64,
            ParentPath,
        >,
    >;

    fn fixed_birth_is_reindexed<B>()
    where
        B: Behavior<Birth = Births<ProxyWithParent<Child, ParentPath>>>,
    {
    }
    fixed_birth_is_reindexed::<Fixed>();
    fixed_birth_is_reindexed::<Delayed>();

    fn dynamic_birth_is_reindexed<B>()
    where
        B: Behavior<Birth = Births<ProxyWithParent<Inert, ParentPath>>>,
    {
    }
    dynamic_birth_is_reindexed::<Dynamic>();

    fn fifo_birth_is_reindexed<B>()
    where
        B: Behavior<Birth = Births<ProxyWithParent<PoolWorker, ParentPath>>>,
    {
    }
    fifo_birth_is_reindexed::<Fifo>();

    fn keyed_birth_is_reindexed<B>()
    where
        B: Behavior<Birth = Births<ProxyWithParent<KeyedPoolWorker, ParentPath>>>,
    {
    }
    keyed_birth_is_reindexed::<Keyed>();

    behavior_accepts_at::<Fixed, WorkerStopped<MailAddr>, ParentPath>();
    behavior_accepts_at::<Fixed, WorkerCreationResolved<u64>, ParentPath>();
    behavior_accepts_at::<Delayed, WorkerStopped<MailAddr>, ParentPath>();
    behavior_accepts_at::<Delayed, WorkerCreationResolved<u64>, ParentPath>();
    behavior_accepts_at::<Dynamic, WorkerStopped<MailAddr>, ParentPath>();
    behavior_accepts_at::<Dynamic, WorkerCreationResolved<u64>, ParentPath>();
    behavior_accepts_at::<Fifo, WorkerStopped<MailAddr>, ParentPath>();
    behavior_accepts_at::<Fifo, WorkerCreationResolved<u64>, ParentPath>();
    behavior_accepts_at::<Keyed, WorkerStopped<MailAddr>, ParentPath>();
    behavior_accepts_at::<Keyed, WorkerCreationResolved<u64>, ParentPath>();
}

#[test]
fn every_deferred_local_fact_request_declares_its_relative_destination() {
    fn returns_here<Request, Fact>()
    where
        Request: behavior_actors::InterpreterRequest<
                ReturnToEmitter = behavior_actors::ReturnsToEmitter<Fact, Here>,
            >,
    {
    }

    returns_here::<ScheduleAt, TimerElapsed>();
    returns_here::<ScheduleAfter, TimerElapsed>();
    returns_here::<ObservePeer<MailAddr>, PeerStopped<MailAddr>>();
    returns_here::<ObserveChild<MailAddr, ChildHead>, ChildStopped<MailAddr>>();
    returns_here::<ObserveCreation<MailAddr, ChildHead>, CreationResolved<MailAddr>>();
    let shutdown = ShutdownChild::<StopOnShutdown<Inert>, ChildHead>::new(8);
    fn here<Input>(_: Ingress<Input, Here>) {}
    here(shutdown.ingress);
    returns_here::<ShutdownChild<StopOnShutdown<Inert>, ChildHead>, ChildShutdownRejected<u64>>();
}

#[tokio::test]
async fn backoff_event_and_sends_are_exact_sum_product_duals() {
    use behavior_actors::{
        InterpretRequest, InterpretSends, InterpreterRequests, SendInterpreter, SupervisionFailure,
        SupervisorSends,
    };

    type Child = StopOnShutdown<Inert>;
    type Event = BackoffSupervisorEvent<User<MailAddr, ()>>;
    type RootEvent = WatchEvent<Event>;
    type Path = Inside<Here>;

    fn structural_proofs()
    where
        RootEvent: InjectEvent<TimerElapsed, Path>
            + InjectEvent<ShutdownRequested, Path>
            + InjectEvent<ChildStopped<MailAddr>, Path>
            + InjectEvent<CreationResolved<MailAddr>, Path>
            + InjectEvent<WorkerStopped<MailAddr>, Path>
            + InjectEvent<WorkerCreationResolved<u64>, Path>
            + InjectEvent<ChildShutdownRejected<u64>, Path>,
    {
    }
    structural_proofs();

    #[derive(Debug, PartialEq, Eq)]
    enum Seen {
        Schedule,
        ChildObservation,
        CreationObservation,
        Replacement,
        Failure,
        Shutdown,
    }
    struct Recording(Vec<Seen>);
    impl SendInterpreter for Recording {
        type Error = Never;
    }
    impl InterpretRequest<ScheduleAfter, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ScheduleAfter,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Schedule);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ObserveChild<MailAddr, ChildHead>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ObserveChild<MailAddr, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::ChildObservation);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ObserveCreation<MailAddr, ChildHead>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ObserveCreation<MailAddr, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::CreationObservation);
                Ok(())
            }
        }
    }
    impl InterpretChildDelivery<Proxy<Child>, ChildHead> for Recording {
        fn interpret_child_delivery(
            &mut self,
            _: ChildDelivery<Proxy<Child>, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Replacement);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ReportSupervisionFailure<MailAddr>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ReportSupervisionFailure<MailAddr>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Failure);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ShutdownChild<Proxy<Child>, ChildHead>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            request: ShutdownChild<Proxy<Child>, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            let _: RootEvent =
                <RootEvent as InjectEvent<_, Path>>::inject_at(ChildShutdownRejected::new(
                    request.nonce,
                    behavior_actors::ChildShutdownRejection::NotEstablished,
                ));
            async move {
                self.0.push(Seen::Shutdown);
                Ok(())
            }
        }
    }

    let sends = SendLayer::new(
        BackoffSupervisorSends {
            schedules: InterpreterRequests::one(ScheduleAfter::new(
                behavior_actors::TimerId(1),
                behavior_actors::TimerGeneration(0),
                std::time::Duration::from_secs(1),
            )),
            supervision: SupervisorSends {
                child_observations: InterpreterRequests::one(ObserveChild::new(1)),
                creation_observations: InterpreterRequests::one(ObserveCreation::new(1)),
                replacement_commands: vec![ChildDelivery::at(
                    ChildRoute::<Proxy<Child>, ChildHead>::new(1),
                    ProxyCommand::Forward(()),
                )],
                worker_commands: Vec::new(),
                failure_reports: InterpreterRequests::one(ReportSupervisionFailure::new(
                    SupervisionFailure::stable_child_stopped(1, Ok(behavior_actors::Exit::Normal)),
                )),
                shutdowns: InterpreterRequests::one(ShutdownChild::new(1)),
            },
        },
        Vec::<Never>::new(),
    );
    let mut recording = Recording(Vec::new());
    <_ as InterpretSends<_, RootEvent, Path>>::interpret(sends, &mut recording)
        .await
        .unwrap();
    assert_eq!(
        recording.0,
        [
            Seen::Schedule,
            Seen::ChildObservation,
            Seen::CreationObservation,
            Seen::Replacement,
            Seen::Failure,
            Seen::Shutdown,
        ]
    );
}

struct PoolReply;
impl behavior::Protocol for PoolReply {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, u16, MailAddr>;
}

type PoolRoute = Recipient<PoolReply>;
type PoolProtocol = WorkerPoolProtocol<MailAddr, PoolReply, u8, u16, PoolRoute>;
type KeyedPoolProtocol = KeyedWorkerPoolProtocol<MailAddr, PoolReply, u8, u8, u16, PoolRoute>;

struct PoolWorker;
impl behavior::Protocol for PoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<PoolProtocol>;
}

struct KeyedPoolWorker;
impl behavior::Protocol for KeyedPoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<KeyedPoolProtocol>;
}
impl Behavior for KeyedPoolWorker {
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
impl Behavior for PoolWorker {
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

#[tokio::test]
async fn worker_pool_event_and_sends_interpret_every_lane_at_the_same_structural_path() {
    use behavior_actors::{
        Delivery, InterpretDelivery, InterpretRequest, InterpretSends, InterpreterRequests,
        Recipient, SendInterpreter, SupervisorSends,
    };

    type PoolProtocolEvent = WorkerPoolEvent<MailAddr, PoolReply, u8, u16, PoolRoute>;
    type RootEvent = WatchEvent<PoolProtocolEvent>;
    type Path = Inside<Here>;

    fn structural_proofs()
    where
        RootEvent: InjectEvent<ChildStopped<MailAddr>, Path>
            + InjectEvent<CreationResolved<MailAddr>, Path>
            + InjectEvent<WorkerStopped<MailAddr>, Path>
            + InjectEvent<WorkerCreationResolved<u64>, Path>
            + InjectEvent<ShutdownRequested, Path>
            + InjectEvent<ChildShutdownRejected<u64>, Path>,
    {
    }
    structural_proofs();

    #[derive(Debug, PartialEq, Eq)]
    enum Seen {
        Response,
        Assignment,
        ChildObservation,
        CreationObservation,
        Failure,
        Shutdown,
    }
    struct Recording(Vec<Seen>);
    impl SendInterpreter for Recording {
        type Error = Never;
    }
    impl InterpretDelivery<PoolReply> for Recording {
        fn interpret_delivery(
            &mut self,
            _: Delivery<PoolReply>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Response);
                Ok(())
            }
        }
    }
    impl InterpretChildDelivery<Proxy<PoolWorker>, ChildHead> for Recording {
        fn interpret_child_delivery(
            &mut self,
            _: ChildDelivery<Proxy<PoolWorker>, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Assignment);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ObserveChild<MailAddr, ChildHead>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ObserveChild<MailAddr, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::ChildObservation);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ObserveCreation<MailAddr, ChildHead>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ObserveCreation<MailAddr, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::CreationObservation);
                Ok(())
            }
        }
    }
    impl InterpretRequest<ReportSupervisionFailure<MailAddr>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            _: ReportSupervisionFailure<MailAddr>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Failure);
                Ok(())
            }
        }
    }
    impl
        InterpretRequest<
            ShutdownChild<ProxyWithParent<PoolWorker, Here>, ChildHead>,
            RootEvent,
            Path,
        > for Recording
    {
        fn interpret_request(
            &mut self,
            request: ShutdownChild<ProxyWithParent<PoolWorker, Here>, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            let _: RootEvent =
                <RootEvent as InjectEvent<_, Path>>::inject_at(ChildShutdownRejected::new(
                    request.nonce,
                    behavior_actors::ChildShutdownRejection::NotEstablished,
                ));
            async move {
                self.0.push(Seen::Shutdown);
                Ok(())
            }
        }
    }

    let sends: PoolSends<MailAddr, PoolWorker, PoolRoute, behavior::Here> = SendLayer::new(
        SupervisorSends {
            child_observations: InterpreterRequests::one(ObserveChild::new(1)),
            creation_observations: InterpreterRequests::one(ObserveCreation::new(1)),
            replacement_commands: vec![ChildDelivery::at(
                ChildRoute::<Proxy<PoolWorker>, ChildHead>::new(1),
                ProxyCommand::Forward(PoolAssignment {
                    assignment: behavior_actors::AssignmentId(1),
                    job: behavior_actors::JobId(1),
                    payload: 7,
                    worker: 1,
                    complete_to: Recipient::global(MailAddr(9)),
                }),
            )],
            worker_commands: Vec::new(),
            failure_reports: InterpreterRequests::one(ReportSupervisionFailure::new(
                behavior_actors::SupervisionFailure::stable_child_stopped(
                    1,
                    Ok(behavior_actors::Exit::Normal),
                ),
            )),
            shutdowns: InterpreterRequests::one(ShutdownChild::new(1)),
        },
        PoolBehaviorSends {
            responses: vec![Delivery::new(
                Recipient::global(MailAddr(8)),
                PoolResponse::Accepted {
                    job: behavior_actors::JobId(2),
                },
            )],
            assignments: vec![ChildDelivery::at(
                ChildRoute::<Proxy<PoolWorker>, ChildHead>::new(1),
                ProxyCommand::Forward(PoolAssignment {
                    assignment: behavior_actors::AssignmentId(2),
                    job: behavior_actors::JobId(2),
                    payload: 8,
                    worker: 1,
                    complete_to: Recipient::global(MailAddr(9)),
                }),
            )],
        },
    );
    let mut recording = Recording(Vec::new());
    <_ as InterpretSends<_, RootEvent, Path>>::interpret(sends, &mut recording)
        .await
        .unwrap();
    assert_eq!(
        recording.0,
        [
            Seen::Response,
            Seen::Assignment,
            Seen::ChildObservation,
            Seen::CreationObservation,
            Seen::Assignment,
            Seen::Failure,
            Seen::Shutdown,
        ]
    );
}
