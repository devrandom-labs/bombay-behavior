//! Final-composition proof for child-derived shutdown planning.

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
struct Application;

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
            .child_at(
                ChildRoute::<ManagedStore, StoreRole>::new(11),
                StopOnShutdown::new(Store),
            )
            .child_at(
                ChildRoute::<ManagedGateway, GatewayRole>::new(12),
                StopOnShutdown::new(Gateway),
            )
            .into_creates()
            .expect("fixture child routes are distinct");
        Ok(Actions::create(creates))
    }

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Recording<Event> {
    events: Vec<Event>,
    shutdowns: Vec<u64>,
}

impl<Event> Default for Recording<Event> {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            shutdowns: Vec::new(),
        }
    }
}

impl<Event: Send> SendInterpreter for Recording<Event> {
    type Error = Never;
}

impl<Event, Plan, TargetPath, Path>
    InterpretRequest<ReportShutdownPlan<Plan, TargetPath>, Event, Path> for Recording<Event>
where
    Event: InjectEvent<InstallShutdownPlan<Plan>, TargetPath> + Send,
    Plan: Send,
{
    fn interpret_request(
        &mut self,
        request: ReportShutdownPlan<Plan, TargetPath>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.events.push(request.into_event());
        async { Ok(()) }
    }
}

impl<Event, Occurrence, Path> InterpretRequest<ObserveCreation<MailAddr, Occurrence>, Event, Path>
    for Recording<Event>
where
    Event: InjectEvent<CreationResolved<MailAddr>, Path> + Send,
    Occurrence: Send,
{
    fn interpret_request(
        &mut self,
        request: ObserveCreation<MailAddr, Occurrence>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.events
            .push(Ingress::<_, Path>::new().event(CreationResolved::birth(
                request.nonce,
                MailAddr(100 + request.nonce),
            )));
        async { Ok(()) }
    }
}

impl<Event, Child, Occurrence, Path> InterpretRequest<ShutdownChild<Child, Occurrence>, Event, Path>
    for Recording<Event>
where
    Event: InjectEvent<ChildStopped<MailAddr>, Path> + Send,
    Child: Behavior<Protocol: Protocol<Addr = MailAddr>>,
    Occurrence: Send,
{
    fn interpret_request(
        &mut self,
        request: ShutdownChild<Child, Occurrence>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.shutdowns.push(request.nonce);
        self.events
            .push(Ingress::<_, Path>::new().event(ChildStopped::new(
                request.nonce,
                Ok(Exit::Normal),
                Instant::now(),
            )));
        async { Ok(()) }
    }
}

async fn interpret<Sends, Event>(sends: Sends, interpreter: &mut Recording<Event>)
where
    Event: Send,
    Sends: InterpretSends<Recording<Event>, Event, Here>,
{
    sends.interpret(interpreter).await.unwrap();
}

async fn shutdown_trace<B>(initialized: Initialized<B>, shutdown_first: bool) -> Vec<u64>
where
    B: Behavior<Protocol: Protocol<Addr = MailAddr>>,
    B::Event: InjectEvent<ShutdownRequested, Here> + Send,
    B::Sends: InterpretSends<Recording<B::Event>, B::Event, Here>,
    B::Error: Debug,
{
    let mut active = initialized.behavior;
    let mut interpreter = Recording::default();
    interpret(initialized.actions.sends, &mut interpreter).await;
    assert_eq!(interpreter.events.len(), 2);

    let first_creation = interpreter.events.remove(0);
    active.transition(first_creation).unwrap();

    if shutdown_first {
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

    if !shutdown_first {
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

#[tokio::test]
async fn outer_guardian_preserves_phase_order_for_both_arrival_orders() {
    let plan_first = Guardian::coordinated(
        shutdown_after_children(Application)
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish(),
    )
    .initialize()
    .unwrap();
    assert_eq!(shutdown_trace(plan_first, false).await, [11, 12]);

    let shutdown_first = Guardian::coordinated(
        shutdown_after_children(Application)
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish(),
    )
    .initialize()
    .unwrap();
    assert_eq!(shutdown_trace(shutdown_first, true).await, [11, 12]);
}

#[tokio::test]
async fn reversing_phases_reverses_interpreted_child_shutdown_order() {
    let reversed = Guardian::coordinated(
        shutdown_after_children(Application)
            .shutdown_phase(GatewayRole)
            .shutdown_phase(StoreRole)
            .finish(),
    )
    .initialize()
    .unwrap();

    assert_eq!(shutdown_trace(reversed, false).await, [12, 11]);
}
