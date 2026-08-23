//! End-to-end proof for creation-dependent shutdown-plan installation.

use behavior_actors::*;
use core::future::Future;
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P> {
    address: RuntimeAddr,
    protocol: PhantomData<fn() -> P>,
}

impl<P> Endpoint<P> {
    const fn new(address: RuntimeAddr) -> Self {
        Self {
            address,
            protocol: PhantomData,
        }
    }
}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self::new(self.address)
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct First;

impl Protocol for First {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for First {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Second;

impl Protocol for Second {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Second {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

type FirstChild = StopOnShutdown<First>;
type SecondChild = StopOnShutdown<Second>;

struct FirstRole;
struct SecondRole;

type Targets =
    ShutdownChoice<SecondChild, ShutdownChoice<FirstChild, NoShutdownTargets<RuntimeAddr>>>;
type Plan = HeterogeneousShutdownPlan<Targets>;
type FirstFact = EstablishedCreation<First, FirstRole>;
type SecondFact = EstablishedCreation<Second, SecondRole>;
type PlannerEvent = EventLayer<SecondFact, EventLayer<FirstFact, User<RuntimeAddr, Never>>>;
type PlannerSends<ParentPath> = SendLayer<
    InterpreterRequests<ObserveEstablishedCreation<Second, SecondRole>>,
    SendLayer<
        InterpreterRequests<ObserveEstablishedCreation<First, FirstRole>>,
        InterpreterRequests<ReportShutdownPlan<Plan, ParentPath>>,
    >,
>;

enum PlannerState {
    AwaitingFirst,
    AwaitingSecond(EstablishedChild<FirstChild, FirstRole>),
    Reported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlannerError {
    UnexpectedCreationFact,
    CreationRejected(CreationRejection),
    InvalidPlan(ShutdownPlanError<u64>),
}

struct Planner<ParentPath> {
    state: PlannerState,
    coordinator: ShutdownPlanIngress<Plan, ParentPath>,
}

impl<ParentPath> Planner<ParentPath> {
    const FIRST: ChildRoute<FirstChild, FirstRole> = ChildRoute::new(11);
    const SECOND: ChildRoute<SecondChild, SecondRole> = ChildRoute::new(12);

    const fn new(coordinator: ShutdownPlanIngress<Plan, ParentPath>) -> Self {
        Self {
            state: PlannerState::AwaitingFirst,
            coordinator,
        }
    }
}

impl<ParentPath> Protocol for Planner<ParentPath> {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl<ParentPath> BehaviorBase for Planner<ParentPath> {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<ParentPath> ChildRole<Planner<ParentPath>> for FirstRole {
    type Child = FirstChild;
    type Position = ChildTail<ChildHead>;
}

impl<ParentPath> ChildRole<Planner<ParentPath>> for SecondRole {
    type Child = SecondChild;
    type Position = ChildHead;
}

impl<ParentPath> Behavior for Planner<ParentPath> {
    type Protocol = Self;
    type Event = PlannerEvent;
    type Sends = PlannerSends<ParentPath>;
    type Ph = Never;
    type Error = PlannerError;
    type Birth = Births<ChildChoice<SecondChild, ChildChoice<FirstChild, Never>>>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        let creates = Children::<RuntimeAddr>::new()
            .child_at(Self::FIRST, StopOnShutdown::new(First))
            .child_at(Self::SECOND, StopOnShutdown::new(Second))
            .into_creates()
            .expect("fixture routes are distinct");
        let sends = SendLayer::new(
            InterpreterRequests::one(ObserveEstablishedCreation::at(Self::SECOND)),
            SendLayer::new(
                InterpreterRequests::one(ObserveEstablishedCreation::at(Self::FIRST)),
                InterpreterRequests::empty(),
            ),
        );
        Ok(Actions::new(sends, creates, Step::Continue))
    }

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Inner(EventLayer::Owned(fact)) => {
                if !matches!(self.state, PlannerState::AwaitingFirst) {
                    return Err(PlannerError::UnexpectedCreationFact);
                }
                let child = established_child::<Self, FirstRole>(fact)
                    .map_err(PlannerError::CreationRejected)?;
                self.state = PlannerState::AwaitingSecond(child);
                Ok(Actions::cont())
            }
            EventLayer::Owned(fact) => {
                let PlannerState::AwaitingSecond(first) = &self.state else {
                    return Err(PlannerError::UnexpectedCreationFact);
                };
                let second = established_child::<Self, SecondRole>(fact)
                    .map_err(PlannerError::CreationRejected)?;
                let plan = HeterogeneousShutdownPlan::new([
                    vec![second.shutdown_target::<Self, Targets>()],
                    vec![first.shutdown_target::<Self, Targets>()],
                ])
                .map_err(PlannerError::InvalidPlan)?;
                let report = self.coordinator.report(plan);
                self.state = PlannerState::Reported;
                Ok(Actions::send(SendLayer::new(
                    InterpreterRequests::empty(),
                    SendLayer::new(
                        InterpreterRequests::empty(),
                        InterpreterRequests::one(report),
                    ),
                )))
            }
            EventLayer::Inner(EventLayer::Inner(user)) => match user.message {},
        }
    }
}

type Coordinator = HeterogeneousShutdownCoordinator<Planner<Inside<Here>>, Targets>;
type Root = CoordinatedGuardian<Coordinator>;
type RootEvent = <Root as Behavior>::Event;

#[derive(Default)]
struct Recording {
    installation: Option<RootEvent>,
    shutdowns: Vec<u64>,
}

impl SendInterpreter for Recording {
    type Error = Never;
}

impl
    InterpretRequest<
        ObserveEstablishedCreation<Second, SecondRole>,
        RootEvent,
        Inside<Inside<Here>>,
    > for Recording
{
    fn interpret_request(
        &mut self,
        _: ObserveEstablishedCreation<Second, SecondRole>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl
    InterpretRequest<
        ObserveEstablishedCreation<First, FirstRole>,
        RootEvent,
        Inside<Inside<Inside<Here>>>,
    > for Recording
{
    fn interpret_request(
        &mut self,
        _: ObserveEstablishedCreation<First, FirstRole>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl
    InterpretRequest<
        ReportShutdownPlan<Plan, Inside<Here>>,
        RootEvent,
        Inside<Inside<Inside<Inside<Here>>>>,
    > for Recording
{
    fn interpret_request(
        &mut self,
        request: ReportShutdownPlan<Plan, Inside<Here>>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.installation = Some(request.into_event());
        async { Ok(()) }
    }
}

impl InterpretRequest<ShutdownChild<SecondChild, ChildHead>, RootEvent, Inside<Here>>
    for Recording
{
    fn interpret_request(
        &mut self,
        request: ShutdownChild<SecondChild, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.shutdowns.push(request.nonce);
        async { Ok(()) }
    }
}

impl InterpretRequest<ShutdownChild<FirstChild, ChildTail<ChildHead>>, RootEvent, Inside<Here>>
    for Recording
{
    fn interpret_request(
        &mut self,
        request: ShutdownChild<FirstChild, ChildTail<ChildHead>>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.shutdowns.push(request.nonce);
        async { Ok(()) }
    }
}

#[tokio::test]
async fn committed_children_report_their_plan_through_actions_to_an_outer_coordinator() {
    let planner = Planner::new(ShutdownPlanIngress::<Plan, Here>::new().inside());
    let initialized = Guardian::coordinated(
        HeterogeneousShutdownCoordinator::<_, Targets>::awaiting_plan(planner),
    )
    .initialize()
    .unwrap();
    assert_eq!(initialized.actions.creates.len(), 2);
    assert_eq!(
        initialized.actions.sends.inner.inner.inner.owned.as_slice()[0].nonce,
        11
    );
    assert_eq!(
        initialized.actions.sends.inner.inner.owned.as_slice()[0].nonce,
        12
    );

    let mut active = initialized.behavior;
    active
        .on_path::<_, Inside<Inside<Inside<Here>>>>(EstablishedCreation::installed(
            11,
            CreationKind::Birth,
            EstablishedRecipient::issued(Endpoint::<First>::new(RuntimeAddr(101))),
        ))
        .unwrap();
    let waiting = active.on(ShutdownRequested).unwrap();
    let mut interpreter = Recording::default();
    <_ as InterpretSends<_, RootEvent, Here>>::interpret(waiting.sends, &mut interpreter)
        .await
        .unwrap();
    assert!(interpreter.shutdowns.is_empty());

    let reported = active
        .on_path::<_, Inside<Inside<Here>>>(EstablishedCreation::installed(
            12,
            CreationKind::Birth,
            EstablishedRecipient::issued(Endpoint::<Second>::new(RuntimeAddr(102))),
        ))
        .unwrap();
    assert!(matches!(active.base().state, PlannerState::Reported));

    <_ as InterpretSends<_, RootEvent, Here>>::interpret(reported.sends, &mut interpreter)
        .await
        .unwrap();
    let installation = interpreter
        .installation
        .take()
        .expect("the explicit report request produced one root event");
    let installation_actions = active.transition(installation).unwrap();
    <_ as InterpretSends<_, RootEvent, Here>>::interpret(
        installation_actions.sends,
        &mut interpreter,
    )
    .await
    .unwrap();
    assert_eq!(interpreter.shutdowns, [12]);
}
