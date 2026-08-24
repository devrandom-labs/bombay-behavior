//! Final-composition regression for stable-worker parent reports.

use behavior_actors::*;
use core::future::Future;
use std::fmt::Debug;
use std::time::Duration;

#[derive(Debug, Clone)]
struct Payment;

struct PaymentWorker;

impl Protocol for PaymentWorker {
    type Addr = MailAddr;
    type Msg = Payment;
}

impl Behavior for PaymentWorker {
    type Protocol = Self;
    type Event = User<MailAddr, Payment>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn worker(_: usize) -> Option<PaymentWorker> {
    Some(PaymentWorker)
}

struct PaymentApp;

impl Protocol for PaymentApp {
    type Addr = MailAddr;
    type Msg = Payment;
}

impl BehaviorBase for PaymentApp {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for PaymentApp {
    type Protocol = Self;
    type Event = User<MailAddr, Payment>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<PaymentWorker>;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn worker_timer(nonce: u64) -> TimerId {
    TimerId(nonce)
}

struct Recording<Event> {
    proxy: u64,
    events: Vec<Event>,
}

impl<Event: Send> SendInterpreter for Recording<Event> {
    type Error = Never;
}

impl<Event, M> InterpretDelivery<MessageProtocol<MailAddr, ProxyUnavailable<MailAddr, M>>>
    for Recording<Event>
where
    Event: Send,
    M: Send,
{
    fn interpret_delivery(
        &mut self,
        _: Delivery<MessageProtocol<MailAddr, ProxyUnavailable<MailAddr, M>>>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Event, C> InterpretChildDelivery<C, ChildHead> for Recording<Event>
where
    Event: Send,
    C: Behavior + Protocol,
{
    fn interpret_child_delivery(
        &mut self,
        _: ChildDelivery<C, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Event, Path> InterpretRequest<ObserveChild<MailAddr, ChildHead>, Event, Path>
    for Recording<Event>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ObserveChild<MailAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Event, Path> InterpretRequest<ObserveCreation<MailAddr, ChildHead>, Event, Path>
    for Recording<Event>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ObserveCreation<MailAddr, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Event, Path, C> InterpretRequest<ShutdownChild<C, ChildHead>, Event, Path> for Recording<Event>
where
    Event: Send,
    C: Behavior,
{
    fn interpret_request(
        &mut self,
        _: ShutdownChild<C, ChildHead>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Event, Path> InterpretRequest<ReportWorkerStopped<MailAddr, Inside<Here>>, Event, Path>
    for Recording<Event>
where
    Event: Send,
{
    fn interpret_request(
        &mut self,
        _: ReportWorkerStopped<MailAddr, Inside<Here>>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async { Ok(()) }
    }
}

impl<Event, Path> InterpretRequest<ReportWorkerCreationResolved<u64, Inside<Here>>, Event, Path>
    for Recording<Event>
where
    Event: InjectEvent<WorkerCreationResolved<u64>, Inside<Here>> + Send,
{
    fn interpret_request(
        &mut self,
        request: ReportWorkerCreationResolved<u64, Inside<Here>>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        let fact = WorkerCreationResolved::from((self.proxy, request));
        self.events
            .push(Ingress::<_, Inside<Here>>::new().event(fact));
        async { Ok(()) }
    }
}

async fn run<B>(definition: B)
where
    B: Behavior<Birth = Births<ProxyWithParent<PaymentWorker, Inside<Here>>>> + BehaviorBase,
    B::Protocol: Protocol<Addr = MailAddr, Msg = Payment>,
    B::Event: InjectEvent<CreationResolved<MailAddr>, Inside<Here>> + Send,
    B::Error: Debug,
    ProxySendsWithParent<PaymentWorker, Inside<Here>>:
        InterpretSends<Recording<B::Event>, B::Event, Here>,
{
    let initialized = definition.initialize().unwrap();
    let mut parent = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .expect("one configured stable slot");
    assert_eq!(creation.nonce, 7);
    parent
        .on_path::<_, Inside<Here>>(CreationResolved::birth(7, MailAddr(70)))
        .unwrap();

    let mut proxy = creation.child.initialize().unwrap().behavior;
    let report = proxy
        .on_path(CreationResolved::birth(0, MailAddr(100)))
        .unwrap();
    let mut interpreter = Recording {
        proxy: creation.nonce,
        events: Vec::new(),
    };
    report.sends.interpret(&mut interpreter).await.unwrap();
    assert_eq!(interpreter.events.len(), 1);

    let parent_report = interpreter.events.pop().unwrap();
    parent.transition(parent_report).unwrap();
}

async fn interpret_proxy_report<B, C>(
    parent: &mut Active<B>,
    creation: Create<MailAddr, ProxyWithParent<C, Inside<Here>>>,
) where
    B: Behavior<Birth = Births<ProxyWithParent<C, Inside<Here>>>>,
    B::Event: InjectEvent<CreationResolved<MailAddr>, Inside<Here>> + Send,
    B::Error: Debug,
    C: Behavior<Ph = Never>,
    C::Protocol: Protocol<Addr = MailAddr>,
    <C::Protocol as Protocol>::Msg: Debug,
    ProxySendsWithParent<C, Inside<Here>>: InterpretSends<Recording<B::Event>, B::Event, Here>,
{
    parent
        .on_path::<_, Inside<Here>>(CreationResolved::birth(creation.nonce, MailAddr(70)))
        .unwrap();
    let mut proxy = creation.child.initialize().unwrap().behavior;
    let report = proxy
        .on_path(CreationResolved::birth(0, MailAddr(100)))
        .unwrap();
    let mut interpreter = Recording {
        proxy: creation.nonce,
        events: Vec::new(),
    };
    report.sends.interpret(&mut interpreter).await.unwrap();
    assert_eq!(interpreter.events.len(), 1);
    parent
        .transition(interpreter.events.pop().unwrap())
        .unwrap();
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_application_owned_supervision_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
    );
    run(StopOnShutdown::new(
        SuperviseWithParent::with_parent(
            PaymentApp,
            ChildTopology::new([7], worker),
            restart,
            ProxyParentIngress::new(),
        )
        .unwrap(),
    ))
    .await;
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_application_owned_backoff_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
    );
    run(StopOnShutdown::new(BackoffSuperviseWithParent::new(
        SuperviseWithParent::with_parent(
            PaymentApp,
            ChildTopology::new([7], worker),
            restart,
            ProxyParentIngress::new(),
        )
        .unwrap(),
        Backoff::constant(Duration::from_secs(1)).unwrap(),
        worker_timer,
    )))
    .await;
}

struct DynamicReply;

impl Protocol for DynamicReply {
    type Addr = MailAddr;
    type Msg = DynamicSupervisorOutcome<MailAddr, PaymentWorker>;
}

#[tokio::test]
async fn outer_shutdown_wrapper_joins_dynamic_supervisor_reports_in_both_orders() {
    for worker_first in [false, true] {
        let definition = StopOnShutdown::new(DynamicSupervisorWithParent::<
            MailAddr,
            PaymentWorker,
            Recipient<DynamicReply>,
            _,
        >::with_parent(ProxyParentIngress::new()));
        let initialized = definition.initialize().unwrap();
        let mut parent = initialized.behavior;
        let accepted = parent
            .receive(
                MailAddr(9),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: PaymentWorker,
                    reply_to: Recipient::global(MailAddr(8)),
                },
            )
            .unwrap();
        let creation = accepted.creates.into_iter().next().unwrap();

        let mut proxy = creation.child.initialize().unwrap().behavior;
        let report = proxy
            .on_path(CreationResolved::birth(0, MailAddr(100)))
            .unwrap();
        let mut interpreter = Recording {
            proxy: creation.nonce,
            events: Vec::new(),
        };
        <_ as InterpretSends<_, _, Here>>::interpret(report.sends, &mut interpreter)
            .await
            .unwrap();
        assert_eq!(interpreter.events.len(), 1);
        let worker = interpreter.events.pop().unwrap();
        let proxy = CreationResolved::birth(creation.nonce, MailAddr(70));

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
        assert!(matches!(
            joined.sends.inner.outcomes[0].message,
            DynamicSupervisorOutcome::Started { nonce: 7, child }
                if child.address() == MailAddr(70)
        ));
    }
}

struct PoolReply;

impl Protocol for PoolReply {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, u16, MailAddr>;
}

type PoolRoute = Recipient<PoolReply>;
type PoolProtocol = WorkerPoolProtocol<MailAddr, PoolReply, u8, u16, PoolRoute>;

struct PoolWorker;

impl Protocol for PoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<PoolProtocol>;
}

impl Behavior for PoolWorker {
    type Protocol = Self;
    type Event = User<MailAddr, PoolAssignment<PoolProtocol>>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn pool_worker(_: usize) -> Option<PoolWorker> {
    Some(PoolWorker)
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_worker_pool_proxy_reports() {
    let definition = StopOnShutdown::new(
        WorkerPoolWithParent::with_parent(
            ChildTopology::new([7], pool_worker),
            PoolConfiguration::new(
                4,
                InterruptionPolicy::Retry,
                RestartPolicy::Permanent,
                2,
                Duration::from_secs(30),
            ),
            Recipient::global(MailAddr(6)),
            ProxyParentIngress::new(),
        )
        .unwrap(),
    );
    let initialized = definition.initialize().unwrap();
    let mut parent = initialized.behavior;
    let creation = initialized.actions.creates.into_iter().next().unwrap();
    interpret_proxy_report(&mut parent, creation).await;
}

type KeyedPoolProtocol = KeyedWorkerPoolProtocol<MailAddr, PoolReply, u8, u8, u16, PoolRoute>;

struct KeyedPoolWorker;

impl Protocol for KeyedPoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<KeyedPoolProtocol>;
}

impl Behavior for KeyedPoolWorker {
    type Protocol = Self;
    type Event = User<MailAddr, PoolAssignment<KeyedPoolProtocol>>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn keyed_pool_worker(_: usize) -> Option<KeyedPoolWorker> {
    Some(KeyedPoolWorker)
}

fn parity(key: &u8) -> u64 {
    u64::from(key % 2)
}

#[tokio::test]
async fn outer_shutdown_wrapper_interprets_keyed_worker_pool_proxy_reports() {
    let definition = StopOnShutdown::new(
        KeyedWorkerPoolWithParent::with_parent(
            ChildTopology::new([7], keyed_pool_worker),
            PoolConfiguration::new(
                4,
                InterruptionPolicy::Retry,
                RestartPolicy::Permanent,
                2,
                Duration::from_secs(30),
            ),
            parity,
            Recipient::global(MailAddr(6)),
            ProxyParentIngress::new(),
        )
        .unwrap(),
    );
    let initialized = definition.initialize().unwrap();
    let mut parent = initialized.behavior;
    let creation = initialized.actions.creates.into_iter().next().unwrap();
    interpret_proxy_report(&mut parent, creation).await;
}
