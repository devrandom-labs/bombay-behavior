//! Complete interpreted worker-pool hierarchy and alias-free user construction.
//!
//! The runtime harness names the concrete actors whose mailboxes it hosts;
//! the application-facing construction regression above the harness does not
//! name the composed pool, stable-child, or layer output types.

use behavior_actors::composition::RelayChildReports;
use behavior_actors::*;
use core::future::Future;
use std::collections::VecDeque;
use std::time::Duration;

struct Replies;

impl Protocol for Replies {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, u16, MailAddr>;
}

struct Worker;

impl Protocol for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, PoolAssignment<u8>>;
    type Sends = InterpreterRequests<ReportToParent<PoolCompletion<u16>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::send(InterpreterRequests::one(
            ReportToParent::new(PoolCompletion {
                assignment: event.message.assignment,
                result: u16::from(event.message.payload) * 2,
            }),
        )))
    }
}

#[test]
fn fifo_and_keyed_pool_construction_needs_no_composed_behavior_alias() {
    fn accepts_fifo<B>(_: B)
    where
        B: Behavior,
        B::Protocol:
            Protocol<Addr = MailAddr, Msg = PoolMessage<MailAddr, u8, u16, Recipient<Replies>>>,
    {
    }

    fn accepts_keyed<B>(_: B)
    where
        B: Behavior,
        B::Protocol: Protocol<
                Addr = MailAddr,
                Msg = KeyedPoolMessage<MailAddr, u8, u8, u16, Recipient<Replies>>,
            >,
    {
    }

    let configuration = || {
        PoolConfiguration::new(
            4,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            2,
            Duration::from_secs(30),
            behavior_actors::RestartTiming::Immediate,
        )
    };

    accepts_fifo(
        WorkerPool::new(
            ChildTopology::new([7], |_| Some(Worker)),
            configuration(),
            Proxy::new,
        )
        .unwrap(),
    );
    accepts_keyed(
        KeyedWorkerPool::new(
            ChildTopology::new([7], |_| Some(Worker)),
            configuration(),
            |_: &u8| 7,
            Proxy::new,
        )
        .unwrap(),
    );
}

type StableLayer = fn(Worker) -> Proxy<Worker>;
type Stable = RelayChildReports<Proxy<Worker>, Worker, PoolCompletion<u16>>;
type Pool = WorkerPool<MailAddr, u8, u16, Worker, Recipient<Replies>, StableLayer>;
type Root = StopOnShutdown<Pool>;
type RootEvent = <Root as Behavior>::Event;
type StableEvent = <Stable as Behavior>::Event;
type WorkerEvent = <Worker as Behavior>::Event;

fn definition() -> Root {
    let pool = WorkerPool::new(
        ChildTopology::new([7], |_| Some(Worker)),
        PoolConfiguration::new(
            4,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            2,
            Duration::from_secs(30),
            behavior_actors::RestartTiming::Immediate,
        ),
        Proxy::new as StableLayer,
    )
    .unwrap();
    StopOnShutdown::new(pool)
}

struct RootEffects<'a> {
    replies: &'a mut Vec<PoolResponse<u8, u16, MailAddr>>,
    stable_events: &'a mut VecDeque<StableEvent>,
    observations: &'a mut usize,
    failures: &'a mut Vec<SupervisionFailure<MailAddr>>,
}

impl SendInterpreter for RootEffects<'_> {
    type Error = Never;
}

impl InterpretDelivery<Replies> for RootEffects<'_> {
    fn interpret_delivery(
        &mut self,
        delivery: Delivery<Replies>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.replies.push(delivery.message);
        async { Ok(()) }
    }
}

impl InterpretChildDelivery<Worker, ChildHead> for RootEffects<'_> {
    fn interpret_child_delivery(
        &mut self,
        delivery: ChildDelivery<Worker, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        assert_eq!(delivery.nonce, 7);
        self.stable_events
            .push_back(StableEvent::user(MailAddr(1), delivery.message));
        async { Ok(()) }
    }
}

impl InterpretChildInput<Stable, Worker, ReplacementRequested<Worker>, ChildHead>
    for RootEffects<'_>
{
    fn interpret_child_input(
        &mut self,
        input: ChildInput<Stable, Worker, ReplacementRequested<Worker>, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        assert_eq!(input.nonce, 7);
        self.stable_events
            .push_back(<StableEvent as ChildInputIngress<
                Worker,
                ReplacementRequested<Worker>,
            >>::child_input(input.input));
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ObserveChild<MailAddr, ChildHead>, RootEvent, Path>
    for RootEffects<'_>
{
    fn interpret_request(
        &mut self,
        _: ObserveChild<MailAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        *self.observations += 1;
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ObserveCreation<MailAddr, ChildHead>, RootEvent, Path>
    for RootEffects<'_>
{
    fn interpret_request(
        &mut self,
        _: ObserveCreation<MailAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        *self.observations += 1;
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ScheduleAfter, RootEvent, Path> for RootEffects<'_> {
    fn interpret_request(
        &mut self,
        _: ScheduleAfter,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ReportSupervisionFailure<MailAddr>, RootEvent, Path>
    for RootEffects<'_>
{
    fn interpret_request(
        &mut self,
        report: ReportSupervisionFailure<MailAddr>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        self.failures.push(report.failure);
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ShutdownChild<Stable, ChildHead>, RootEvent, Path> for RootEffects<'_> {
    fn interpret_request(
        &mut self,
        _: ShutdownChild<Stable, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

struct StableEffects<'a> {
    stable: u64,
    root_events: &'a mut VecDeque<RootEvent>,
    worker_events: &'a mut VecDeque<WorkerEvent>,
    observations: &'a mut usize,
}

impl SendInterpreter for StableEffects<'_> {
    type Error = Never;
}

impl InterpretChildDelivery<Worker, ChildHead> for StableEffects<'_> {
    fn interpret_child_delivery(
        &mut self,
        delivery: ChildDelivery<Worker, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        assert_eq!(delivery.nonce, 0);
        self.worker_events
            .push_back(WorkerEvent::user(MailAddr(70), delivery.message));
        async { Ok(()) }
    }
}

impl<Path, Report> InterpretRequest<ReportToParent<Report>, StableEvent, Path> for StableEffects<'_>
where
    Report: Send,
    RootEvent: EventIngress<ChildRoute<Stable, ChildHead>, ChildReport<MailAddr, Report>>,
{
    fn interpret_request(
        &mut self,
        request: ReportToParent<Report>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let fact = ChildReport::from((self.stable, request));
        self.root_events.push_back(<RootEvent as EventIngress<
            ChildRoute<Stable, ChildHead>,
            ChildReport<MailAddr, Report>,
        >>::ingress(fact));
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ObserveChild<MailAddr, ChildHead>, StableEvent, Path>
    for StableEffects<'_>
{
    fn interpret_request(
        &mut self,
        _: ObserveChild<MailAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        *self.observations += 1;
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ObserveCreation<MailAddr, ChildHead>, StableEvent, Path>
    for StableEffects<'_>
{
    fn interpret_request(
        &mut self,
        _: ObserveCreation<MailAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        *self.observations += 1;
        async { Ok(()) }
    }
}

impl<Path> InterpretRequest<ShutdownChild<Worker, ChildHead>, StableEvent, Path>
    for StableEffects<'_>
{
    fn interpret_request(
        &mut self,
        _: ShutdownChild<Worker, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

struct WorkerEffects<'a> {
    worker: u64,
    stable_events: &'a mut VecDeque<StableEvent>,
}

impl SendInterpreter for WorkerEffects<'_> {
    type Error = Never;
}

impl<Path> InterpretRequest<ReportToParent<PoolCompletion<u16>>, WorkerEvent, Path>
    for WorkerEffects<'_>
{
    fn interpret_request(
        &mut self,
        request: ReportToParent<PoolCompletion<u16>>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let fact = ChildReport::from((self.worker, request));
        self.stable_events.push_back(<StableEvent as EventIngress<
            ChildRoute<Worker, ChildHead>,
            ChildReport<MailAddr, PoolCompletion<u16>>,
        >>::ingress(fact));
        async { Ok(()) }
    }
}

struct Runtime {
    root: Active<Root>,
    stable: Active<Stable>,
    worker: Active<Worker>,
    root_events: VecDeque<RootEvent>,
    stable_events: VecDeque<StableEvent>,
    worker_events: VecDeque<WorkerEvent>,
    replies: Vec<PoolResponse<u8, u16, MailAddr>>,
    observations: usize,
    failures: Vec<SupervisionFailure<MailAddr>>,
}

impl Runtime {
    async fn start(worker_fact_first: bool) -> Self {
        let initialized = definition().initialize().unwrap();
        let root = initialized.behavior;
        let mut stable_creations = initialized.actions.creates.into_iter();
        let stable_creation = stable_creations.next().unwrap();
        assert!(stable_creations.next().is_none());

        let stable_initialized = stable_creation.child.initialize().unwrap();
        let stable = stable_initialized.behavior;
        let mut worker_creations = stable_initialized.actions.creates.into_iter();
        let worker_creation = worker_creations.next().unwrap();
        assert!(worker_creations.next().is_none());

        let worker_initialized = worker_creation.child.initialize().unwrap();
        let worker = worker_initialized.behavior;
        assert!(worker_initialized.actions.creates.is_empty());

        let mut runtime = Self {
            root,
            stable,
            worker,
            root_events: VecDeque::new(),
            stable_events: VecDeque::new(),
            worker_events: VecDeque::new(),
            replies: Vec::new(),
            observations: 0,
            failures: Vec::new(),
        };
        runtime
            .interpret_root_sends(initialized.actions.sends)
            .await;
        runtime
            .interpret_stable_sends(stable_initialized.actions.sends)
            .await;
        runtime
            .interpret_worker_sends(worker_initialized.actions.sends)
            .await;

        runtime.stable_events.push_back(<StableEvent as InjectEvent<
            CreationResolved<MailAddr>,
            Inside<Here>,
        >>::inject_at(CreationResolved::birth(
            worker_creation.nonce,
            MailAddr(100),
        )));
        if worker_fact_first {
            runtime.drain().await;
        }
        runtime.root_events.push_back(<RootEvent as InjectEvent<
            CreationResolved<MailAddr>,
            Inside<Here>,
        >>::inject_at(CreationResolved::birth(
            stable_creation.nonce,
            MailAddr(70),
        )));
        runtime.drain().await;
        assert_eq!(runtime.observations, 4);
        runtime
    }

    async fn submit(&mut self, payload: u8) {
        let actions = self
            .root
            .receive(
                MailAddr(90),
                PoolMessage::Submit {
                    job: JobId(11),
                    payload,
                    reply_to: Recipient::global(MailAddr(91)),
                },
            )
            .unwrap();
        assert!(actions.creates.is_empty());
        self.interpret_root_sends(actions.sends).await;
        self.drain().await;
    }

    async fn drain(&mut self) {
        loop {
            if let Some(event) = self.root_events.pop_front() {
                let actions = self.root.transition(event).unwrap();
                assert!(actions.creates.is_empty());
                self.interpret_root_sends(actions.sends).await;
                continue;
            }
            if let Some(event) = self.stable_events.pop_front() {
                let actions = self.stable.transition(event).unwrap();
                assert!(actions.creates.is_empty());
                self.interpret_stable_sends(actions.sends).await;
                continue;
            }
            if let Some(event) = self.worker_events.pop_front() {
                let actions = self.worker.transition(event).unwrap();
                assert!(actions.creates.is_empty());
                self.interpret_worker_sends(actions.sends).await;
                continue;
            }
            break;
        }
    }

    async fn interpret_root_sends(&mut self, sends: <Root as Behavior>::Sends) {
        let mut effects = RootEffects {
            replies: &mut self.replies,
            stable_events: &mut self.stable_events,
            observations: &mut self.observations,
            failures: &mut self.failures,
        };
        <_ as InterpretSends<_, RootEvent, Here>>::interpret(sends, &mut effects)
            .await
            .unwrap();
    }

    async fn interpret_stable_sends(&mut self, sends: <Stable as Behavior>::Sends) {
        let mut effects = StableEffects {
            stable: 7,
            root_events: &mut self.root_events,
            worker_events: &mut self.worker_events,
            observations: &mut self.observations,
        };
        <_ as InterpretSends<_, StableEvent, Here>>::interpret(sends, &mut effects)
            .await
            .unwrap();
    }

    async fn interpret_worker_sends(&mut self, sends: <Worker as Behavior>::Sends) {
        let mut effects = WorkerEffects {
            worker: 0,
            stable_events: &mut self.stable_events,
        };
        <_ as InterpretSends<_, WorkerEvent, Here>>::interpret(sends, &mut effects)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn completion_crosses_worker_proxy_relay_pool_and_outer_shutdown_exactly_once() {
    for worker_fact_first in [false, true] {
        let mut runtime = Runtime::start(worker_fact_first).await;
        runtime.submit(21).await;

        assert!(runtime.failures.is_empty());
        assert_eq!(runtime.replies.len(), 2);
        assert!(matches!(
            runtime.replies[0],
            PoolResponse::Accepted { job: JobId(11) }
        ));
        assert!(matches!(
            runtime.replies[1],
            PoolResponse::Completed {
                job: JobId(11),
                result: 42,
            }
        ));
        assert!(runtime.root_events.is_empty());
        assert!(runtime.stable_events.is_empty());
        assert!(runtime.worker_events.is_empty());
    }
}

#[tokio::test]
async fn worker_stop_then_proxy_return_joins_through_the_complete_outer_hierarchy() {
    let mut runtime = Runtime::start(false).await;

    let submitted = runtime
        .root
        .receive(
            MailAddr(90),
            PoolMessage::Submit {
                job: JobId(11),
                payload: 21,
                reply_to: Recipient::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert!(submitted.creates.is_empty());
    runtime.interpret_root_sends(submitted.sends).await;
    assert_eq!(runtime.stable_events.len(), 1);

    runtime
        .stable_events
        .push_front(<StableEvent as InjectEvent<
            ChildStopped<MailAddr>,
            Inside<Here>,
        >>::inject_at(ChildStopped::new(
            0,
            Err(Crash::Failed),
            std::time::Instant::now(),
        )));

    let stopped = runtime.stable_events.pop_front().unwrap();
    let stopped_actions = runtime.stable.transition(stopped).unwrap();
    assert!(stopped_actions.creates.is_empty());
    runtime.interpret_stable_sends(stopped_actions.sends).await;
    assert_eq!(runtime.root_events.len(), 1);

    let owner_stop = runtime.root_events.pop_front().unwrap();
    let owner_actions = runtime.root.transition(owner_stop).unwrap();
    assert!(owner_actions.creates.is_empty());
    runtime.interpret_root_sends(owner_actions.sends).await;
    assert_eq!(runtime.root.base().backlog_len(), 1);
    assert_eq!(runtime.stable_events.len(), 2);

    let admitted_assignment = runtime.stable_events.pop_front().unwrap();
    let unavailable = runtime.stable.transition(admitted_assignment).unwrap();
    assert!(unavailable.creates.is_empty());
    runtime.interpret_stable_sends(unavailable.sends).await;
    assert_eq!(runtime.root_events.len(), 1);

    let returned_assignment = runtime.root_events.pop_front().unwrap();
    let joined = runtime
        .root
        .transition(returned_assignment)
        .expect("the outer pool must join worker-stop-first with the returned assignment");
    assert!(joined.creates.is_empty());
    runtime.interpret_root_sends(joined.sends).await;

    assert_eq!(runtime.root.base().backlog_len(), 1);
    assert_eq!(runtime.replies.len(), 1);
    assert!(matches!(
        runtime.replies[0],
        PoolResponse::Accepted { job: JobId(11) }
    ));
    assert!(runtime.failures.is_empty());
    assert!(runtime.root_events.is_empty());
    assert!(runtime.worker_events.is_empty());
    assert_eq!(runtime.stable_events.len(), 1);
}
