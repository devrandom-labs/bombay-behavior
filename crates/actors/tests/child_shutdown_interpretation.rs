//! Final-composition proof for child-derived shutdown planning.

use behavior::*;
use behavior_actors::*;
use core::future::Future;
use std::fmt::Debug;
use std::time::Instant;

struct Store;
struct Gateway;

macro_rules! inert_child {
    ($child:ty) => {
        impl Protocol for $child {
            type Addr = MailAddr;
            type Msg = Never;
        }

        impl Behavior for $child {
            type Protocol = Self;
            type Event = User<MailAddr, Never>;
            type Sends = NoSends;
            type Ph = Never;
            type Error = Never;
            type Birth = NoBirths;

            fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
                match event.message {}
            }
        }
    };
}

inert_child!(Store);
inert_child!(Gateway);

type ManagedStore = StopOnShutdown<Store>;
type ManagedGateway = StopOnShutdown<Gateway>;

struct StoreRole;
struct GatewayRole;
struct Application {
    store: CreationId,
    gateway: CreationId,
}

struct EmptyApplication;

impl Protocol for EmptyApplication {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for EmptyApplication {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

impl Application {
    fn issue() -> Self {
        let mut creations = CreationSequence::new();
        let store = creations.issue().expect("the store creation ID exists");
        let gateway = creations.issue().expect("the gateway creation ID exists");
        Self { store, gateway }
    }
}

impl Protocol for Application {
    type Addr = MailAddr;
    type Msg = Never;
}

impl BehaviorBase for Application {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl ChildRole<Application> for StoreRole {
    type Child = ManagedStore;
    type Position = ChildTail<ChildHead>;
}

impl ChildOccurrence<Application> for StoreRole {
    type Resolution = DeclaredChildOccurrence;
}

impl ChildRole<Application> for GatewayRole {
    type Child = ManagedGateway;
    type Position = ChildHead;
}

impl ChildOccurrence<Application> for GatewayRole {
    type Resolution = DeclaredChildOccurrence;
}

impl Behavior for Application {
    type Protocol = Self;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<ChildChoice<ManagedGateway, ChildChoice<ManagedStore, Never>>>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        let creates = Children::<MailAddr>::new()
            .child(self.store, StopOnShutdown::new(Store))
            .child(self.gateway, StopOnShutdown::new(Gateway))
            .into_creates();
        Ok(Actions::create(creates))
    }

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Recording<Event> {
    events: Vec<Event>,
    shutdowns: Vec<CreationId>,
    next_address: u64,
}

impl<Event> Default for Recording<Event> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            shutdowns: Vec::new(),
            next_address: 100,
        }
    }
}

impl<Event, Plan, Path> InterpretItem<ReportShutdownPlan<Plan>, Event, Path> for Recording<Event>
where
    Event: EventIngress<Here, InstallShutdownPlan<Plan>> + Send,
    Plan: Send,
{
    fn interpret_item(
        &mut self,
        request: ReportShutdownPlan<Plan>,
    ) -> impl Future<Output = ItemSettlement<ReportShutdownPlan<Plan>, (), Never, Never>> + Send
    {
        self.events.push(request.into_event());
        async { ItemSettlement::Accepted(()) }
    }
}

impl<Event, P, Occurrence, Path> InterpretItem<ObserveCreation<P, Occurrence>, Event, Path>
    for Recording<Event>
where
    Event: InjectEvent<CreationResolved<MailAddr>, Path> + Send,
    P: Protocol<Addr = MailAddr>,
{
    fn interpret_item(
        &mut self,
        request: ObserveCreation<P, Occurrence>,
    ) -> impl Future<
        Output = ItemSettlement<
            ObserveCreation<P, Occurrence>,
            (),
            Never,
            CreationCorrelation<P, Occurrence>,
        >,
    > + Send {
        let address = MailAddr(self.next_address);
        self.next_address = self
            .next_address
            .checked_add(1)
            .expect("the fixture address sequence has capacity");
        self.events.push(
            Ingress::<_, Path>::new().event(CreationResolved::birth(request.creation, address)),
        );
        async { ItemSettlement::Accepted(()) }
    }
}

impl<Event, Child, Occurrence, Path> InterpretItem<ShutdownChild<Child, Occurrence>, Event, Path>
    for Recording<Event>
where
    Event: InjectEvent<ChildStopped<MailAddr>, Path> + Send,
    Child: Behavior<Protocol: Protocol<Addr = MailAddr>>,
{
    fn interpret_item(
        &mut self,
        request: ShutdownChild<Child, Occurrence>,
    ) -> impl Future<
        Output = ItemSettlement<
            ShutdownChild<Child, Occurrence>,
            (),
            ChildShutdownRejection,
            CreationCorrelation<Child::Protocol, Occurrence>,
        >,
    > + Send {
        self.shutdowns.push(request.child);
        self.events
            .push(Ingress::<_, Path>::new().event(ChildStopped::new(
                request.child,
                Ok(Exit::Normal),
                Instant::now(),
            )));
        async { ItemSettlement::Accepted(()) }
    }
}

async fn interpret<Sends, Event>(sends: Sends, interpreter: &mut Recording<Event>)
where
    Event: Send,
    Sends: InterpretSends<Recording<Event>, Event, Here>,
    Sends::Settlements: ClassifySettlement,
{
    let Interpretation::Complete(settlements) = sends.interpret(interpreter).await else {
        panic!("the recording interpreter cannot corrupt an item");
    };
    assert_eq!(settlements.settlement_status(), SettlementStatus::Accepted);
}

enum ShutdownArrival {
    BeforePlan,
    AfterPlan,
}

async fn shutdown_trace<B>(initialized: Initialized<B>, arrival: ShutdownArrival) -> Vec<CreationId>
where
    B: Behavior<Protocol: Protocol<Addr = MailAddr>>,
    B::Event: InjectEvent<ShutdownRequested, Here> + Send,
    B::Sends: InterpretSends<Recording<B::Event>, B::Event, Here>,
    <B::Sends as SendSettlements>::Settlements: ClassifySettlement,
    B::Error: Debug,
{
    let mut active = initialized.behavior;
    let mut interpreter = Recording::default();
    interpret(initialized.actions.sends, &mut interpreter).await;
    assert_eq!(interpreter.events.len(), 2);

    let first_creation = interpreter.events.remove(0);
    let first = active.transition(first_creation).unwrap();
    interpret(first.sends, &mut interpreter).await;
    assert_eq!(interpreter.events.len(), 1);
    assert!(first.creates.is_empty());
    assert!(matches!(first.become_, behavior::Step::Continue));

    if matches!(arrival, ShutdownArrival::BeforePlan) {
        let waiting = active.on(ShutdownRequested).unwrap();
        interpret(waiting.sends, &mut interpreter).await;
        assert!(interpreter.shutdowns.is_empty());
        assert_eq!(interpreter.events.len(), 1);
    }

    let second_creation = interpreter.events.remove(0);
    let reported = active.transition(second_creation).unwrap();

    interpret(reported.sends, &mut interpreter).await;
    assert_eq!(interpreter.events.len(), 1);
    assert!(interpreter.shutdowns.is_empty());

    let installation = interpreter
        .events
        .pop()
        .expect("the report must enqueue one final root event");
    let started = active.transition(installation).unwrap();
    interpret(started.sends, &mut interpreter).await;

    if matches!(arrival, ShutdownArrival::AfterPlan) {
        assert!(interpreter.shutdowns.is_empty());
        let started = active.on(ShutdownRequested).unwrap();
        interpret(started.sends, &mut interpreter).await;
    }

    while !interpreter.events.is_empty() {
        let stopped = interpreter.events.remove(0);
        let next = active.transition(stopped).unwrap();
        interpret(next.sends, &mut interpreter).await;
    }

    interpreter.shutdowns
}

fn framework_phase<Builder, Role>(builder: Builder, role: Role) -> Builder::Output
where
    Builder: DeclareShutdownPhase<Role>,
{
    builder.shutdown_phase(role)
}

fn framework_begin<B>(application: B) -> B::Output
where
    B: BeginShutdownPhases,
{
    application.begin_shutdown_phases()
}

fn framework_finish<Builder>(builder: Builder) -> Builder::Output
where
    Builder: FinishShutdownPhases,
{
    builder.finish()
}

#[tokio::test]
async fn application_without_children_reports_an_empty_ready_plan() {
    let initialized = shutdown_after_children(EmptyApplication)
        .finish()
        .initialize()
        .unwrap();
    let mut active = initialized.behavior;
    let mut interpreter = Recording::default();

    interpret(initialized.actions.sends, &mut interpreter).await;
    assert_eq!(interpreter.events.len(), 1);
    assert!(interpreter.shutdowns.is_empty());

    let installation = interpreter
        .events
        .pop()
        .expect("the empty plan report enqueues one installation");
    let installed = active.transition(installation).unwrap();
    interpret(installed.sends, &mut interpreter).await;

    assert!(interpreter.events.is_empty());
    assert!(interpreter.shutdowns.is_empty());
    assert!(installed.creates.is_empty());
    assert!(matches!(installed.become_, Step::Continue));
    let ShutdownState::Ready { plan } = active.state() else {
        panic!("the empty plan was not installed as ready");
    };
    assert!(plan.phases().is_empty());
}

#[tokio::test]
async fn generic_framework_carries_hidden_phase_states_without_copying_the_typestate() {
    let application = Application::issue();
    let expected = [application.store, application.gateway];
    let builder = framework_begin(application);
    let builder = framework_phase(builder, StoreRole);
    let builder = framework_phase(builder, GatewayRole);
    let initialized = framework_finish(builder).initialize().unwrap();

    assert_eq!(
        shutdown_trace(initialized, ShutdownArrival::AfterPlan).await,
        expected
    );
}

#[tokio::test]
async fn coordinator_preserves_phase_order_for_both_arrival_orders() {
    let application = Application::issue();
    let expected = [application.store, application.gateway];
    let plan_first = shutdown_after_children(application)
        .shutdown_phase(StoreRole)
        .shutdown_phase(GatewayRole)
        .finish()
        .initialize()
        .unwrap();
    assert_eq!(
        shutdown_trace(plan_first, ShutdownArrival::AfterPlan).await,
        expected
    );

    let application = Application::issue();
    let expected = [application.store, application.gateway];
    let shutdown_first = shutdown_after_children(application)
        .shutdown_phase(StoreRole)
        .shutdown_phase(GatewayRole)
        .finish()
        .initialize()
        .unwrap();
    assert_eq!(
        shutdown_trace(shutdown_first, ShutdownArrival::BeforePlan).await,
        expected
    );
}

#[tokio::test]
async fn reversing_phases_reverses_interpreted_child_shutdown_order() {
    let application = Application::issue();
    let expected = [application.gateway, application.store];
    let reversed = shutdown_after_children(application)
        .shutdown_phase(GatewayRole)
        .shutdown_phase(StoreRole)
        .finish()
        .initialize()
        .unwrap();

    assert_eq!(
        shutdown_trace(reversed, ShutdownArrival::AfterPlan).await,
        expected
    );
}
