//! Final-composition regression for stable-worker parent reports.

use behavior_actors::*;
use core::future::Future;
use core::marker::PhantomData;
use std::fmt::Debug;
use std::time::Duration;

type Payment = u8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct RuntimeEndpoint<P> {
    id: u64,
    protocol: PhantomData<fn() -> P>,
}

impl<P> RuntimeEndpoint<P> {
    const fn new(id: u64) -> Self {
        Self {
            id,
            protocol: PhantomData,
        }
    }
}

impl<P> Clone for RuntimeEndpoint<P> {
    fn clone(&self) -> Self {
        Self::new(self.id)
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = RuntimeEndpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct EndpointId;

impl<P> InterpretEstablished<P> for EndpointId
where
    P: Protocol<Addr = RuntimeAddr>,
{
    type Output = u64;

    fn interpret_established(&mut self, endpoint: RuntimeEndpoint<P>) -> Self::Output {
        endpoint.id
    }
}

struct PaymentWorker;

impl Protocol for PaymentWorker {
    type Addr = RuntimeAddr;
    type Msg = Payment;
}

impl Behavior for PaymentWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Payment>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let _ = event;
        Ok(Actions::cont())
    }
}

fn worker(_: usize) -> Option<PaymentWorker> {
    Some(PaymentWorker)
}

struct PaymentApp;

impl Protocol for PaymentApp {
    type Addr = RuntimeAddr;
    type Msg = Payment;
}

#[derive(Debug, PartialEq, Eq)]
enum RecoveringEvent {
    Command(User<RuntimeAddr, Payment>),
    Lifecycle(SupervisionLifecycle<RuntimeAddr>),
    Unavailable(ProxyUnavailable<RuntimeAddr, Payment>),
}

impl UserEvent for RecoveringEvent {
    type Addr = RuntimeAddr;
    type Message = Payment;

    fn user(from: RuntimeAddr, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }

    fn into_user(self) -> Result<User<RuntimeAddr, Self::Message>, Self> {
        match self {
            Self::Command(event) => Ok(event),
            unavailable => Err(unavailable),
        }
    }
}

impl
    EventIngress<
        ChildRoute<Proxy<PaymentWorker>, ChildHead>,
        ProxyUnavailable<RuntimeAddr, Payment>,
    > for RecoveringEvent
{
    fn ingress(unavailable: ProxyUnavailable<RuntimeAddr, Payment>) -> Self {
        Self::Unavailable(unavailable)
    }
}

impl EventIngress<Here, SupervisionLifecycle<RuntimeAddr>> for RecoveringEvent {
    fn ingress(lifecycle: SupervisionLifecycle<RuntimeAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

struct RecoveringApp {
    returned: Vec<ProxyUnavailable<RuntimeAddr, Payment>>,
}

impl Protocol for RecoveringApp {
    type Addr = RuntimeAddr;
    type Msg = Payment;
}

impl BehaviorBase for RecoveringApp {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for RecoveringApp {
    type Protocol = Self;
    type Event = RecoveringEvent;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            RecoveringEvent::Command(_) => {}
            RecoveringEvent::Lifecycle(_lifecycle) => {}
            RecoveringEvent::Unavailable(unavailable) => self.returned.push(unavailable),
        }
        Ok(Actions::cont())
    }
}

impl BehaviorBase for PaymentApp {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for PaymentApp {
    type Protocol = Self;
    type Event = RecoveringEvent;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<PaymentWorker>;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            RecoveringEvent::Command(_command) => {}
            RecoveringEvent::Lifecycle(_lifecycle) => {}
            RecoveringEvent::Unavailable(_unavailable) => {}
        }
        Ok(Actions::cont())
    }
}

struct Recording<Event, Source> {
    proxy: u64,
    events: Vec<Event>,
    effects: Vec<ObservedEffect>,
    source: core::marker::PhantomData<fn() -> Source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservedEffect {
    ChildDelivery,
    ChildObservation,
    CreationObservation,
    Shutdown,
    Schedule,
    ReplacementInput,
    FailureReport,
    ParentReport,
}

impl<Event: Send, Source> SendInterpreter for Recording<Event, Source> {
    type Error = Never;
}

impl<Event, Source, C> InterpretChildDelivery<C, ChildHead> for Recording<Event, Source>
where
    Event: Send,
    C: Behavior + Protocol,
{
    fn interpret_child_delivery(
        &mut self,
        _: ChildDelivery<C, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::ChildDelivery);
        async { Ok(()) }
    }
}

impl<Event, Source, Path> InterpretRequest<ObserveChild<RuntimeAddr, ChildHead>, Event, Path>
    for Recording<Event, Source>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ObserveChild<RuntimeAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::ChildObservation);
        async { Ok(()) }
    }
}

impl<Event, Source, Path> InterpretRequest<ObserveCreation<RuntimeAddr, ChildHead>, Event, Path>
    for Recording<Event, Source>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ObserveCreation<RuntimeAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::CreationObservation);
        async { Ok(()) }
    }
}

impl<Event, Source, Path, C> InterpretRequest<ShutdownChild<C, ChildHead>, Event, Path>
    for Recording<Event, Source>
where
    Event: Send,
    C: Behavior,
{
    fn interpret_request(
        &mut self,
        _: ShutdownChild<C, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::Shutdown);
        async { Ok(()) }
    }
}

impl<Event, Source, Path, Report> InterpretRequest<ReportToParent<Report>, Event, Path>
    for Recording<Event, Source>
where
    Event: EventIngress<Source, ChildReport<RuntimeAddr, Report>> + Send,
    Report: Send,
{
    fn interpret_request(
        &mut self,
        request: ReportToParent<Report>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let fact = ChildReport::from((self.proxy, request));
        self.events.push(Event::ingress(fact));
        self.effects.push(ObservedEffect::ParentReport);
        async { Ok(()) }
    }
}

impl<Event, Source, Path> InterpretRequest<ScheduleAfter, Event, Path> for Recording<Event, Source>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ScheduleAfter,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::Schedule);
        async { Ok(()) }
    }
}

impl<Event, Source, Path> InterpretRequest<ReportSupervisionFailure<RuntimeAddr>, Event, Path>
    for Recording<Event, Source>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ReportSupervisionFailure<RuntimeAddr>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::FailureReport);
        async { Ok(()) }
    }
}

impl<Event, Source>
    InterpretChildInput<
        Proxy<PaymentWorker>,
        PaymentWorker,
        ReplacementRequested<PaymentWorker>,
        ChildHead,
    > for Recording<Event, Source>
where
    Event: Send,
{
    fn interpret_child_input(
        &mut self,
        _: ChildInput<
            Proxy<PaymentWorker>,
            PaymentWorker,
            ReplacementRequested<PaymentWorker>,
            ChildHead,
        >,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.effects.push(ObservedEffect::ReplacementInput);
        async { Ok(()) }
    }
}

async fn run<B>(definition: B)
where
    B: Behavior<Birth = Births<ChildChoice<PaymentWorker, Proxy<PaymentWorker>>>>
        + BehaviorBase<Base = PaymentApp>,
    B::Protocol: Protocol<Addr = RuntimeAddr, Msg = Payment>,
    B::Event: InjectEvent<CreationResolved<RuntimeAddr>, Inside<Here>>
        + EventIngress<
            ChildRoute<Proxy<PaymentWorker>, ChildHead>,
            ChildReport<RuntimeAddr, ReportWorkerCreationResolved<u64>>,
        > + Send,
    B::Sends: InterpretSends<
            Recording<B::Event, ChildRoute<Proxy<PaymentWorker>, ChildHead>>,
            B::Event,
            Here,
        >,
    B::Error: Debug,
{
    let initialized = definition.initialize().unwrap();
    let mut parent = initialized.behavior;
    let mut creations = initialized.actions.creates.into_iter();
    let creation = creations.next().expect("one configured stable slot");
    assert!(creations.next().is_none());
    assert_eq!(creation.nonce, 7);
    let mut interpreter = Recording {
        proxy: creation.nonce,
        events: Vec::new(),
        effects: Vec::new(),
        source: core::marker::PhantomData,
    };
    <_ as InterpretSends<_, B::Event, Here>>::interpret(
        initialized.actions.sends,
        &mut interpreter,
    )
    .await
    .unwrap();
    assert!(interpreter.events.is_empty());
    assert_eq!(
        interpreter.effects,
        [
            ObservedEffect::ChildObservation,
            ObservedEffect::CreationObservation,
        ]
    );
    assert!(matches!(initialized.actions.become_, Step::Continue));

    let stable_installed = parent
        .on_path::<_, Inside<Here>>(CreationResolved::birth(7, RuntimeAddr(70)))
        .unwrap();
    <_ as InterpretSends<_, B::Event, Here>>::interpret(stable_installed.sends, &mut interpreter)
        .await
        .unwrap();
    assert!(interpreter.events.is_empty());
    assert_eq!(interpreter.effects.len(), 2);
    assert!(stable_installed.creates.is_empty());
    assert!(matches!(stable_installed.become_, Step::Continue));

    let ChildChoice::Tail(proxy) = creation.child else {
        panic!("application initialization emitted an unexpected domain child")
    };
    let proxy_initialized = proxy.initialize().unwrap();
    assert_eq!(proxy_initialized.actions.creates.len(), 1);
    assert_eq!(proxy_initialized.actions.sends.child_observations.len(), 1);
    assert_eq!(
        proxy_initialized.actions.sends.creation_observations.len(),
        1
    );
    assert!(proxy_initialized.actions.sends.deliveries.is_empty());
    assert!(
        proxy_initialized
            .actions
            .sends
            .unavailable_reports
            .is_empty()
    );
    assert!(proxy_initialized.actions.sends.stopped_reports.is_empty());
    assert!(proxy_initialized.actions.sends.creation_reports.is_empty());
    assert!(proxy_initialized.actions.sends.shutdowns.is_empty());
    assert_eq!(proxy_initialized.actions.become_, Step::Continue);
    let mut proxy = proxy_initialized.behavior;
    let report = proxy
        .on_path(CreationResolved::birth(0, RuntimeAddr(100)))
        .unwrap();
    <_ as InterpretSends<_, B::Event, Here>>::interpret(
        report.sends.creation_reports,
        &mut interpreter,
    )
    .await
    .unwrap();
    assert!(report.sends.deliveries.is_empty());
    assert!(report.sends.unavailable_reports.is_empty());
    assert!(report.sends.child_observations.is_empty());
    assert!(report.sends.creation_observations.is_empty());
    assert!(report.sends.stopped_reports.is_empty());
    assert!(report.sends.shutdowns.is_empty());
    assert!(report.creates.is_empty());
    assert_eq!(report.become_, Step::Continue);
    assert_eq!(interpreter.events.len(), 1);
    assert_eq!(
        interpreter.effects.last(),
        Some(&ObservedEffect::ParentReport)
    );

    let parent_report = interpreter.events.pop().unwrap();
    let joined = parent.transition(parent_report).unwrap();
    <_ as InterpretSends<_, B::Event, Here>>::interpret(joined.sends, &mut interpreter)
        .await
        .unwrap();
    assert!(interpreter.events.is_empty());
    assert_eq!(interpreter.effects.len(), 3);
    assert!(joined.creates.is_empty());
    assert!(matches!(joined.become_, Step::Continue));
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_application_owned_supervision_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
        behavior_actors::RestartTiming::Immediate,
    );
    run(StopOnShutdown::new(
        Supervise::new(
            PaymentApp,
            ChildTopology::new([7], worker),
            restart,
            |worker: PaymentWorker| worker.layer(Proxy::new),
        )
        .unwrap(),
    ))
    .await;
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_standalone_supervisor_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
        behavior_actors::RestartTiming::Immediate,
    );
    let initialized = StopOnShutdown::new(
        Supervisor::new(
            ChildTopology::new([7], worker),
            restart,
            |worker: PaymentWorker| worker.layer(Proxy::new),
        )
        .unwrap(),
    )
    .initialize()
    .unwrap();
    let mut parent = initialized.behavior;
    let creation = initialized.actions.creates.into_iter().next().unwrap();

    let stable_installed = parent
        .on_path::<_, Inside<Here>>(CreationResolved::birth(creation.nonce, RuntimeAddr(70)))
        .unwrap();
    assert!(matches!(stable_installed.sends.owned, NoSends));
    assert!(stable_installed.sends.inner.child_observations.is_empty());
    assert!(
        stable_installed
            .sends
            .inner
            .creation_observations
            .is_empty()
    );
    assert!(stable_installed.sends.inner.replacement_inputs.is_empty());
    assert!(stable_installed.sends.inner.failure_reports.is_empty());
    assert!(stable_installed.sends.inner.shutdowns.is_empty());
    assert!(stable_installed.creates.is_empty());
    assert_eq!(stable_installed.become_, Step::Continue);
    let proxy_initialized = creation.child.initialize().unwrap();
    assert_eq!(proxy_initialized.actions.creates.len(), 1);
    assert_eq!(proxy_initialized.actions.sends.child_observations.len(), 1);
    assert_eq!(
        proxy_initialized.actions.sends.creation_observations.len(),
        1
    );
    assert_eq!(proxy_initialized.actions.become_, Step::Continue);
    let mut proxy = proxy_initialized.behavior;
    let report = proxy
        .on_path(CreationResolved::birth(0, RuntimeAddr(100)))
        .unwrap();
    let mut interpreter = Recording {
        proxy: creation.nonce,
        events: Vec::new(),
        effects: Vec::new(),
        source: core::marker::PhantomData::<fn() -> ChildRoute<Proxy<PaymentWorker>, ChildHead>>,
    };
    <_ as InterpretSends<_, _, Here>>::interpret(report.sends.creation_reports, &mut interpreter)
        .await
        .unwrap();

    assert_eq!(interpreter.events.len(), 1);
    let joined = parent
        .transition(interpreter.events.pop().unwrap())
        .unwrap();
    assert!(matches!(joined.sends.owned, NoSends));
    assert!(joined.sends.inner.child_observations.is_empty());
    assert!(joined.sends.inner.creation_observations.is_empty());
    assert!(joined.sends.inner.replacement_inputs.is_empty());
    assert!(joined.sends.inner.failure_reports.is_empty());
    assert!(joined.sends.inner.shutdowns.is_empty());
    assert!(joined.creates.is_empty());
    assert!(matches!(joined.become_, Step::Continue));
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_application_owned_backoff_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
        behavior_actors::RestartTiming::Delayed(Backoff::constant(Duration::from_secs(1)).unwrap()),
    );
    run(StopOnShutdown::new(
        Supervise::new(
            PaymentApp,
            ChildTopology::new([7], worker),
            restart,
            |worker: PaymentWorker| worker.layer(Proxy::new),
        )
        .unwrap(),
    ))
    .await;
}

struct DynamicReply;

impl Protocol for DynamicReply {
    type Addr = RuntimeAddr;
    type Msg = DynamicSupervisorOutcome<RuntimeAddr, PaymentWorker>;
}

#[tokio::test]
async fn outer_shutdown_wrapper_joins_dynamic_supervisor_reports_in_both_orders() {
    for worker_first in [false, true] {
        let definition = StopOnShutdown::new(DynamicSupervisor::new(|worker: PaymentWorker| {
            worker.layer(Proxy::new)
        }));
        let initialized = definition.initialize().unwrap();
        let mut parent = initialized.behavior;
        let accepted = parent
            .receive(
                RuntimeAddr(9),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: PaymentWorker,
                    reply_to: Recipient::<DynamicReply>::global(RuntimeAddr(8)),
                },
            )
            .unwrap();
        let creation = accepted.creates.into_iter().next().unwrap();

        let mut proxy = creation.child.initialize().unwrap().behavior;
        let report = proxy
            .on_path(CreationResolved::birth(0, RuntimeAddr(100)))
            .unwrap();
        let mut interpreter = Recording {
            proxy: creation.nonce,
            events: Vec::new(),
            effects: Vec::new(),
            source: core::marker::PhantomData::<fn() -> ChildRoute<Proxy<PaymentWorker>, ChildHead>>,
        };
        <_ as InterpretSends<_, _, Here>>::interpret(report.sends, &mut interpreter)
            .await
            .unwrap();
        assert_eq!(interpreter.events.len(), 1);
        let worker = interpreter.events.pop().unwrap();
        let proxy = EstablishedCreation::<PaymentWorker, ChildHead>::installed(
            creation.nonce,
            CreationKind::Birth,
            EstablishedRecipient::issued(RuntimeEndpoint::new(70)),
        );

        let (first, joined) = if worker_first {
            (
                parent.transition(worker).unwrap(),
                parent.on_path::<_, Inside<Here>>(proxy).unwrap(),
            )
        } else {
            (
                parent.on_path::<_, Inside<Here>>(proxy).unwrap(),
                parent.transition(worker).unwrap(),
            )
        };

        assert!(first.sends.inner.outcomes.is_empty());
        assert_eq!(joined.sends.inner.outcomes.len(), 1);
        let outcome = joined
            .sends
            .inner
            .outcomes
            .into_iter()
            .next()
            .expect("the completed join emits one outcome")
            .message;
        let DynamicSupervisorOutcome::Started { nonce, child } = outcome else {
            panic!("the completed join emitted an unexpected outcome")
        };
        assert_eq!(nonce, 7);
        assert_eq!(child.interpret(&mut EndpointId), 70);
    }
}

#[tokio::test]
async fn dynamic_start_owner_receives_proxy_unavailability_through_the_outer_wrapper() {
    let initialized = StopOnShutdown::new(DynamicSupervisor::new(|worker: PaymentWorker| {
        worker.layer(Proxy::new)
    }))
    .initialize()
    .unwrap();
    let mut parent = initialized.behavior;
    let accepted = parent
        .receive(
            RuntimeAddr(9),
            DynamicSupervisorMessage::Start {
                nonce: 7,
                child: PaymentWorker,
                reply_to: Recipient::<DynamicReply>::global(RuntimeAddr(8)),
            },
        )
        .unwrap();
    let creation = accepted.creates.into_iter().next().unwrap();
    let mut proxy = creation.child.initialize().unwrap().behavior;

    let unavailable = proxy.receive(RuntimeAddr(41), 23).unwrap();
    assert!(unavailable.sends.deliveries.is_empty());
    assert_eq!(unavailable.sends.unavailable_reports.len(), 1);
    let mut interpreter = Recording {
        proxy: creation.nonce,
        events: Vec::new(),
        effects: Vec::new(),
        source: core::marker::PhantomData::<fn() -> ChildRoute<Proxy<PaymentWorker>, ChildHead>>,
    };
    <_ as InterpretSends<_, _, Here>>::interpret(unavailable.sends, &mut interpreter)
        .await
        .unwrap();
    assert_eq!(interpreter.events.len(), 1);

    let returned = parent
        .transition(interpreter.events.pop().unwrap())
        .unwrap();
    assert!(returned.creates.is_empty());
    assert!(matches!(returned.sends.owned, NoSends));
    assert!(returned.sends.inner.child_observations.is_empty());
    assert!(returned.sends.inner.creation_observations.is_empty());
    assert!(returned.sends.inner.replacement_inputs.is_empty());
    assert!(returned.sends.inner.shutdowns.is_empty());
    assert_eq!(returned.sends.inner.outcomes.len(), 1);
    assert_eq!(
        returned.sends.inner.outcomes[0].to.address(),
        RuntimeAddr(8)
    );
    assert!(matches!(
        returned.sends.inner.outcomes[0].message,
        DynamicSupervisorOutcome::CommandUnavailable {
            nonce: 7,
            from: RuntimeAddr(41),
            phase: IncarnationPhase::Installing {
                attempt: 0,
                kind: CreationKind::Birth,
            },
            command: 23,
        }
    ));
    assert!(matches!(returned.become_, Step::Continue));
}

#[tokio::test]
async fn application_owned_supervision_delivers_unavailability_to_the_authored_domain_event() {
    let initialized = StopOnShutdown::new(
        Supervise::new(
            RecoveringApp {
                returned: Vec::new(),
            },
            ChildTopology::new([7], worker),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                2,
                Duration::from_secs(30),
                behavior_actors::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap(),
    )
    .initialize()
    .unwrap();
    let mut parent = initialized.behavior;
    let creation = initialized.actions.creates.into_iter().next().unwrap();
    let mut proxy = creation.child.initialize().unwrap().behavior;

    let unavailable = proxy.receive(RuntimeAddr(41), 29).unwrap();
    let mut interpreter = Recording {
        proxy: creation.nonce,
        events: Vec::new(),
        effects: Vec::new(),
        source: core::marker::PhantomData::<fn() -> ChildRoute<Proxy<PaymentWorker>, ChildHead>>,
    };
    <_ as InterpretSends<_, _, Here>>::interpret(unavailable.sends, &mut interpreter)
        .await
        .unwrap();
    assert_eq!(interpreter.events.len(), 1);

    let recovered = parent
        .transition(interpreter.events.pop().unwrap())
        .unwrap();
    assert!(matches!(recovered.sends.owned, NoSends));
    assert!(recovered.sends.inner.owned.child_observations.is_empty());
    assert!(recovered.sends.inner.owned.creation_observations.is_empty());
    assert!(recovered.sends.inner.owned.schedules.is_empty());
    assert!(recovered.sends.inner.owned.replacement_inputs.is_empty());
    assert!(recovered.sends.inner.owned.failure_reports.is_empty());
    assert!(recovered.sends.inner.owned.shutdowns.is_empty());
    assert!(matches!(recovered.sends.inner.inner, NoSends));
    assert!(recovered.creates.is_empty());
    assert!(matches!(recovered.become_, Step::Continue));
    assert!(matches!(
        parent.base().returned.as_slice(),
        [ProxyUnavailable {
            proxy: 7,
            from: RuntimeAddr(41),
            phase: IncarnationPhase::Installing {
                attempt: 0,
                kind: CreationKind::Birth,
            },
            command: 29,
        }]
    ));
}
