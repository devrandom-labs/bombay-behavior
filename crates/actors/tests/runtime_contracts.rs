//! Compile-time manifest for every interpreter-originated template lane.
//!
//! These assertions model the universal adapter boundary in
//! `docs/adapter-contract.md`: a request-producing template must accept the
//! exact fact returned by that request, and a protocol-indexed shutdown target
//! must accept `ShutdownRequested` through its concrete event sum.

use behavior_actors::composition::RelayChildReports;
use behavior_actors::{
    Actions, Address, Backoff, Behavior, BehaviorActed, BehaviorBase, BirthMode, BirthNodeAt,
    Births, BreakerOutcome, ChildDelivery, ChildHead, ChildInput, ChildReport, ChildRoute,
    ChildShutdownRejected, ChildStopped, ChildTail, ChildTopology, CircuitBreaker, Create,
    CreationResolved, Deadline, DynamicSupervisor, DynamicSupervisorOutcome, EndpointAddress,
    EstablishedCreation, EventIngress, Here, Ingress, InjectEvent, Inside, InstallShutdownPlan,
    InterpretChildDelivery, InterpretChildInput, InterruptionPolicy, KeyedWorkerPool, Lease,
    LeaseOutcome, MailAddr, Never, NoBirths, ObserveChild, ObserveCreation, ObservePeer, OneShot,
    PeerStopped, Periodic, PoolAssignment, PoolCompletion, PoolConfiguration, PoolResponse,
    PoolSends, Presence, PresenceReply, Proxy, ProxyEvent, ReceiveTimeout, Recipient,
    ReplacementRequested, ReportSupervisionFailure, ReportWorkerCreationResolved,
    ReportWorkerStopped, RestartConfiguration, RestartPolicy, RestartTiming, ScheduleAfter,
    ScheduleAt, SendLayer, ShutdownChild, ShutdownCoordinator, ShutdownCoordinatorEvent,
    ShutdownPlan, ShutdownRequested, StopOnShutdown, Strategy, Supervise, SupervisionEvent,
    SupervisionLifecycle, TerminationMonitor, TimerElapsed, User, UserEvent, Watch, WatchEvent,
    WorkerCreationResolved, WorkerPool, WorkerStopped,
};
use core::future::Future;
use core::marker::PhantomData;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DynamicAddr(u64);

impl Address for DynamicAddr {
    type Nonce = u64;
}

struct DynamicEndpoint<P>(PhantomData<fn() -> P>);

impl<P> Clone for DynamicEndpoint<P> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl EndpointAddress for DynamicAddr {
    type Established<P>
        = DynamicEndpoint<P>
    where
        P: behavior::Protocol<Addr = Self>;
}

struct DynamicInert;

impl behavior::Protocol for DynamicInert {
    type Addr = DynamicAddr;
    type Msg = ();
}

impl Behavior for DynamicInert {
    type Protocol = Self;
    type Event = User<DynamicAddr, ()>;
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
    type Addr = DynamicAddr;
    type Msg = DynamicSupervisorOutcome<DynamicAddr, DynamicInert>;
}

impl Behavior for DynamicReply {
    type Protocol = Self;
    type Event = User<DynamicAddr, behavior::BehaviorMessage<Self>>;
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

struct ForwardingParent;

enum OwnerEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    User(User<MailAddr, ()>),
}

impl UserEvent for OwnerEvent {
    type Addr = MailAddr;
    type Message = ();

    fn user(from: MailAddr, message: ()) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<MailAddr, ()>, Self> {
        match self {
            Self::User(user) => Ok(user),
            lifecycle => Err(lifecycle),
        }
    }
}

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for OwnerEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl behavior::Protocol for ForwardingParent {
    type Addr = MailAddr;
    type Msg = ();
}

impl BehaviorBase for ForwardingParent {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for ForwardingParent {
    type Protocol = Self;
    type Event = OwnerEvent;
    type Sends = Vec<ChildDelivery<Inert, ChildHead>>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Inert>;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event {
            OwnerEvent::Lifecycle(_lifecycle) => {}
            OwnerEvent::User(_user) => {}
        }
        Ok(Actions::send(vec![ChildDelivery::at(
            ChildRoute::<Inert, ChildHead>::new(7),
            (),
        )]))
    }
}

impl Behavior for Parent {
    type Protocol = Self;
    type Event = OwnerEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<StopOnShutdown<Inert>>;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event {
            OwnerEvent::Lifecycle(_lifecycle) => {}
            OwnerEvent::User(_user) => {}
        }
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

fn value_accepts_at<B, Input, Path>(_: &B)
where
    B: Behavior,
    B::Event: InjectEvent<Input, Path>,
{
}

fn restart_configuration() -> RestartConfiguration {
    RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        core::time::Duration::from_secs(30),
        RestartTiming::Immediate,
    )
}

fn delayed_restart_configuration() -> RestartConfiguration {
    RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        8,
        core::time::Duration::MAX,
        behavior_actors::RestartTiming::Delayed(
            Backoff::constant(core::time::Duration::from_secs(1)).unwrap(),
        ),
    )
}

fn pool_configuration() -> PoolConfiguration {
    PoolConfiguration::new(
        4,
        InterruptionPolicy::Retry,
        RestartPolicy::Permanent,
        2,
        core::time::Duration::from_secs(30),
        behavior_actors::RestartTiming::Immediate,
    )
}

#[test]
fn behavior_layer_preserves_the_inner_domain_protocol_through_a_proxy_birth() {
    fn preserves_protocol<B, P>()
    where
        B: Behavior<Protocol = P>,
        P: behavior::Protocol,
    {
    }

    fn preserves_authored_child_delivery<B>()
    where
        B: Behavior + behavior::ResolveChildOccurrence<ChildHead>,
        behavior::ResolvedChild<B, ChildHead>: Behavior<Protocol = Inert>,
    {
    }

    fn value_preserves_authored_child_delivery<B>(_: &B)
    where
        B: Behavior + behavior_actors::ResolveChildOccurrence<ChildHead>,
        behavior_actors::ResolvedChild<B, ChildHead>: Behavior<Protocol = Inert>,
    {
    }

    preserves_protocol::<Proxy<Inert>, Inert>();
    preserves_authored_child_delivery::<ForwardingParent>();
    let supervised = Supervise::new(
        ForwardingParent,
        ChildTopology::new([7], |_| Some(Inert)),
        restart_configuration(),
        Proxy::new,
    )
    .unwrap();
    value_preserves_authored_child_delivery(&supervised);
}

#[test]
fn every_topology_owner_exposes_itself_as_its_behavior_base() {
    fn owns_topology<B>(_: &B)
    where
        B: Behavior + BehaviorBase<Base = B>,
    {
    }

    owns_topology(&Proxy::new(StopOnShutdown::new(Inert)));
    let dynamic =
        DynamicSupervisor::<DynamicAddr, DynamicInert, Recipient<DynamicReply>, _>::new(Proxy::new);
    owns_topology(&dynamic);
    let fifo = WorkerPool::<MailAddr, u8, u16, PoolWorker, PoolRoute, _>::new(
        ChildTopology::new([1], |_| Some(PoolWorker)),
        pool_configuration(),
        Proxy::new,
    )
    .unwrap();
    owns_topology(&fifo);
    let keyed = KeyedWorkerPool::<MailAddr, u8, u8, u16, KeyedPoolWorker, PoolRoute, _, _>::new(
        ChildTopology::new([1], |_| Some(KeyedPoolWorker)),
        pool_configuration(),
        |_: &u8| 1,
        Proxy::new,
    )
    .unwrap();
    owns_topology(&keyed);
}

#[test]
fn every_timer_request_has_an_exact_timer_fact_input() {
    accepts::<CircuitBreaker<MailAddr, Recipient<BreakerReply>>, TimerElapsed>();
    accepts::<Lease<MailAddr, u8, Recipient<LeaseReply>>, TimerElapsed>();
    accepts::<Presence<MailAddr, u8, Recipient<PresenceReplyBehavior>>, TimerElapsed>();
    accepts::<Deadline<Inert>, TimerElapsed>();
    accepts::<OneShot<Inert>, TimerElapsed>();
    accepts::<Periodic<Inert>, TimerElapsed>();
    accepts::<ReceiveTimeout<Inert>, TimerElapsed>();
    let delayed = Supervise::new(
        Parent,
        ChildTopology::new([1], |_| Some(StopOnShutdown::new(Inert))),
        delayed_restart_configuration(),
        Proxy::new,
    )
    .unwrap();
    value_accepts_at::<_, TimerElapsed, Here>(&delayed);
}

#[test]
fn every_observation_and_parent_report_has_an_exact_fact_input() {
    accepts::<Watch<Inert>, PeerStopped<MailAddr>>();
    accepts::<TerminationMonitor<Inert>, PeerStopped<MailAddr>>();
    accepts::<Proxy<Inert>, ChildStopped<MailAddr>>();
    accepts::<Proxy<Inert>, CreationResolved<MailAddr>>();
    accepts::<Proxy<Inert>, ShutdownRequested>();
    accepts::<Proxy<Inert>, ChildShutdownRejected<u64>>();

    let dynamic =
        DynamicSupervisor::<DynamicAddr, DynamicInert, Recipient<DynamicReply>, _>::new(Proxy::new);
    value_accepts_at::<_, ChildStopped<DynamicAddr>, Here>(&dynamic);
    value_accepts_at::<_, EstablishedCreation<DynamicInert, ChildHead>, Here>(&dynamic);
    value_accepts_at::<_, WorkerStopped<DynamicAddr>, Here>(&dynamic);
    value_accepts_at::<_, WorkerCreationResolved<u64>, Here>(&dynamic);
    value_accepts_at::<_, ShutdownRequested, Here>(&dynamic);
    value_accepts_at::<_, ChildShutdownRejected<u64>, Here>(&dynamic);

    type ProxyProtocol = ProxyEvent<Inert>;
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
    let dynamic = StopOnShutdown::new(DynamicSupervisor::<
        DynamicAddr,
        DynamicInert,
        Recipient<DynamicReply>,
        _,
    >::new(Proxy::new));
    value_accepts_at::<_, ShutdownRequested, Here>(&dynamic);

    type CoordinatorProtocol = ShutdownCoordinatorEvent<User<MailAddr, ()>, ShutdownPlan<u64>>;
    event_accepts::<CoordinatorProtocol, ShutdownRequested>();
    event_accepts::<CoordinatorProtocol, InstallShutdownPlan<ShutdownPlan<u64>>>();
    event_accepts::<CoordinatorProtocol, ChildStopped<MailAddr>>();
    event_accepts::<CoordinatorProtocol, ChildShutdownRejected<u64>>();

    fn coordinator_is_closed<B: Behavior>() {}
    coordinator_is_closed::<ShutdownCoordinator<Parent, StopOnShutdown<Inert>, ChildHead>>();

    let delayed = Supervise::new(
        Parent,
        ChildTopology::new([1], |_| Some(StopOnShutdown::new(Inert))),
        delayed_restart_configuration(),
        Proxy::new,
    )
    .unwrap();
    value_accepts_at::<_, TimerElapsed, Here>(&delayed);
    value_accepts_at::<_, ShutdownRequested, Here>(&delayed);
    value_accepts_at::<_, ChildStopped<MailAddr>, Here>(&delayed);
    value_accepts_at::<_, CreationResolved<MailAddr>, Here>(&delayed);
    value_accepts_at::<_, WorkerStopped<MailAddr>, Here>(&delayed);
    value_accepts_at::<_, WorkerCreationResolved<u64>, Here>(&delayed);
    value_accepts_at::<_, ChildShutdownRejected<u64>, Here>(&delayed);

    let fifo = WorkerPool::<MailAddr, u8, u16, PoolWorker, PoolRoute, _>::new(
        ChildTopology::new([1], |_| Some(PoolWorker)),
        pool_configuration(),
        Proxy::new,
    )
    .unwrap();
    value_accepts_at::<_, ShutdownRequested, Here>(&fifo);
    value_accepts_at::<_, ChildShutdownRejected<u64>, Here>(&fifo);

    let keyed = KeyedWorkerPool::<MailAddr, u8, u8, u16, KeyedPoolWorker, PoolRoute, _, _>::new(
        ChildTopology::new([1], |_| Some(KeyedPoolWorker)),
        pool_configuration(),
        |_: &u8| 1,
        Proxy::new,
    )
    .unwrap();
    value_accepts_at::<_, ShutdownRequested, Here>(&keyed);
    value_accepts_at::<_, ChildShutdownRejected<u64>, Here>(&keyed);
}

#[test]
fn shutdown_wrapper_owns_dynamic_supervisor_shutdown_end_to_end() {
    use behavior_actors::Activate as _;

    let mut active = StopOnShutdown::new(DynamicSupervisor::<
        DynamicAddr,
        DynamicInert,
        Recipient<DynamicReply>,
        _,
    >::new(Proxy::new))
    .initialize()
    .unwrap()
    .behavior;

    let actions = active.on(ShutdownRequested).unwrap();
    assert!(matches!(actions.become_, behavior_actors::Step::Stop(_)));
}

#[test]
fn outer_behavior_layers_preserve_source_indexed_proxy_report_ingress() {
    fn accepts_report<B>(_: &B)
    where
        B: Behavior,
        B::Event: EventIngress<
                ChildRoute<Proxy<DynamicInert>, ChildHead>,
                ChildReport<DynamicAddr, ReportWorkerStopped<DynamicAddr>>,
            > + EventIngress<
                ChildRoute<Proxy<DynamicInert>, ChildHead>,
                ChildReport<DynamicAddr, ReportWorkerCreationResolved<u64>>,
            >,
    {
    }

    let wrapped = StopOnShutdown::new(DynamicSupervisor::<
        DynamicAddr,
        DynamicInert,
        Recipient<DynamicReply>,
        _,
    >::new(Proxy::new));
    accepts_report(&wrapped);
}

#[test]
fn every_stable_owner_has_one_path_free_stable_birth() {
    fn owns_proxy<B, C>(_: &B)
    where
        B: Behavior<Birth = Births<Proxy<C>>>,
        C: Behavior<Ph = Never>,
    {
    }

    fn owns_relayed_proxy<B, C, R>(_: &B)
    where
        B: Behavior<Birth = Births<RelayChildReports<Proxy<C>, C, PoolCompletion<R>>>>,
        C: Behavior<Ph = Never>,
        <behavior_actors::BehaviorAddr<C> as behavior_actors::Address>::Nonce: From<u64>,
    {
    }

    fn owns_fixed_proxy_occurrence<B>(_: &B)
    where
        B: Behavior,
        <B::Birth as BirthMode>::Child: BirthNodeAt<ChildHead, Child = StopOnShutdown<Inert>>
            + BirthNodeAt<ChildTail<ChildHead>, Child = Proxy<StopOnShutdown<Inert>>>,
    {
    }

    let fixed = Supervise::new(
        Parent,
        ChildTopology::new([1], |_| Some(StopOnShutdown::new(Inert))),
        restart_configuration(),
        Proxy::new,
    )
    .unwrap();
    owns_fixed_proxy_occurrence(&fixed);
    let dynamic =
        DynamicSupervisor::<DynamicAddr, DynamicInert, Recipient<DynamicReply>, _>::new(Proxy::new);
    owns_proxy::<_, DynamicInert>(&dynamic);
    let fifo = WorkerPool::<MailAddr, u8, u16, PoolWorker, PoolRoute, _>::new(
        ChildTopology::new([1], |_| Some(PoolWorker)),
        pool_configuration(),
        Proxy::new,
    )
    .unwrap();
    owns_relayed_proxy::<_, PoolWorker, u16>(&fifo);
    let keyed = KeyedWorkerPool::<MailAddr, u8, u8, u16, KeyedPoolWorker, PoolRoute, _, _>::new(
        ChildTopology::new([1], |_| Some(KeyedPoolWorker)),
        pool_configuration(),
        |_: &u8| 1,
        Proxy::new,
    )
    .unwrap();
    owns_relayed_proxy::<_, KeyedPoolWorker, u16>(&keyed);
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
async fn supervision_event_and_sends_are_exact_sum_product_duals() {
    use behavior_actors::{
        InterpretRequest, InterpretSends, InterpreterRequests, SendInterpreter, SupervisionFailure,
        SupervisorSends,
    };

    type Child = StopOnShutdown<Inert>;
    type Event = SupervisionEvent<User<MailAddr, ()>>;
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
    impl InterpretChildInput<Proxy<Child>, Child, ReplacementRequested<Child>, ChildHead>
        for Recording
    {
        fn interpret_child_input(
            &mut self,
            _: ChildInput<Proxy<Child>, Child, ReplacementRequested<Child>, ChildHead>,
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
        SupervisorSends {
            child_observations: InterpreterRequests::one(ObserveChild::new(1)),
            creation_observations: InterpreterRequests::one(ObserveCreation::new(1)),
            schedules: InterpreterRequests::one(ScheduleAfter::new(
                behavior_actors::TimerId(1),
                behavior_actors::TimerGeneration(0),
                std::time::Duration::from_secs(1),
            )),
            replacement_inputs: vec![ChildInput::at(
                ChildRoute::<Proxy<Child>, ChildHead>::new(1),
                ReplacementRequested::new(StopOnShutdown::new(Inert)),
            )],
            failure_reports: InterpreterRequests::one(ReportSupervisionFailure::new(
                SupervisionFailure::stable_child_stopped(1, Ok(behavior_actors::Exit::Normal)),
            )),
            shutdowns: InterpreterRequests::one(ShutdownChild::new(1)),
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
            Seen::ChildObservation,
            Seen::CreationObservation,
            Seen::Schedule,
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
struct PoolWorker;
impl behavior::Protocol for PoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
}

struct KeyedPoolWorker;
impl behavior::Protocol for KeyedPoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
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

    type PoolProtocolEvent =
        SupervisionEvent<behavior_actors::WorkerPoolEvent<MailAddr, u8, u16, PoolRoute>>;
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
        Replacement,
        ChildObservation,
        CreationObservation,
        Schedule,
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
    impl InterpretChildDelivery<PoolWorker, ChildHead> for Recording {
        fn interpret_child_delivery(
            &mut self,
            _: ChildDelivery<PoolWorker, ChildHead>,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Assignment);
                Ok(())
            }
        }
    }
    impl
        InterpretChildInput<
            Proxy<PoolWorker>,
            PoolWorker,
            ReplacementRequested<PoolWorker>,
            ChildHead,
        > for Recording
    {
        fn interpret_child_input(
            &mut self,
            _: ChildInput<
                Proxy<PoolWorker>,
                PoolWorker,
                ReplacementRequested<PoolWorker>,
                ChildHead,
            >,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Replacement);
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
    impl InterpretRequest<ShutdownChild<Proxy<PoolWorker>, ChildHead>, RootEvent, Path> for Recording {
        fn interpret_request(
            &mut self,
            request: ShutdownChild<Proxy<PoolWorker>, ChildHead>,
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

    let sends = PoolSends::<MailAddr, PoolWorker, Proxy<PoolWorker>, _> {
        responses: vec![Delivery::new(
            Recipient::global(MailAddr(8)),
            PoolResponse::Accepted {
                job: behavior_actors::JobId(2),
            },
        )],
        assignments: vec![ChildDelivery::at(
            ChildRoute::<Proxy<PoolWorker>, ChildHead>::new(1),
            PoolAssignment {
                assignment: behavior_actors::AssignmentId(2),
                job: behavior_actors::JobId(2),
                payload: 8,
            },
        )],
        supervision: SupervisorSends {
            child_observations: InterpreterRequests::one(ObserveChild::<MailAddr, ChildHead>::new(
                1,
            )),
            creation_observations: InterpreterRequests::one(
                ObserveCreation::<MailAddr, ChildHead>::new(1),
            ),
            schedules: InterpreterRequests::one(ScheduleAfter::new(
                behavior_actors::TimerId(1),
                behavior_actors::TimerGeneration(0),
                std::time::Duration::from_secs(1),
            )),
            replacement_inputs: vec![ChildInput::at(
                ChildRoute::<Proxy<PoolWorker>, ChildHead>::new(1),
                ReplacementRequested::new(PoolWorker),
            )],
            failure_reports: InterpreterRequests::one(ReportSupervisionFailure::new(
                behavior_actors::SupervisionFailure::stable_child_stopped(
                    1,
                    Ok(behavior_actors::Exit::Normal),
                ),
            )),
            shutdowns: InterpreterRequests::one(ShutdownChild::new(1)),
        },
    };
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
            Seen::Schedule,
            Seen::Replacement,
            Seen::Failure,
            Seen::Shutdown,
        ]
    );
}
