//! Final-composition regression for stable-worker parent reports.

use behavior_actors::*;
use core::future::Future;
use std::fmt::Debug;
use std::time::Duration;

#[derive(Debug, Clone)]
struct Payment {
    id: u64,
}

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

fn payment_slot(payment: &Payment) -> u64 {
    payment.id
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
async fn outer_guardian_interprets_the_proxy_report_without_a_path_at_the_recipe_call() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
    );
    let backoff = Backoff::constant(Duration::from_secs(1)).unwrap();

    run(Guardian::new(
        supervised_backoff(
            ChildTopology::new([7], worker),
            restart,
            backoff,
            payment_slot as fn(&Payment) -> u64,
        )
        .unwrap(),
    ))
    .await;
}

#[tokio::test]
async fn outer_guardian_interprets_fixed_supervised_worker_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
    );
    run(Guardian::new(
        supervised(
            ChildTopology::new([7], worker),
            restart,
            payment_slot as fn(&Payment) -> u64,
        )
        .unwrap(),
    ))
    .await;
}

#[tokio::test]
async fn outer_guardian_interprets_application_owned_supervision_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
    );
    run(Guardian::new(
        supervise(PaymentApp, ChildTopology::new([7], worker), restart).unwrap(),
    ))
    .await;
}

#[tokio::test]
async fn outer_guardian_interprets_application_owned_backoff_reports() {
    let restart = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        2,
        Duration::from_secs(30),
    );
    run(Guardian::new(
        supervise_backoff(
            PaymentApp,
            ChildTopology::new([7], worker),
            restart,
            Backoff::constant(Duration::from_secs(1)).unwrap(),
            worker_timer,
        )
        .unwrap(),
    ))
    .await;
}

struct DynamicReply;

impl Protocol for DynamicReply {
    type Addr = MailAddr;
    type Msg = DynamicSupervisorOutcome<MailAddr, PaymentWorker>;
}

#[tokio::test]
async fn outer_guardian_interprets_dynamic_supervisor_proxy_reports() {
    let definition = Guardian::new(dynamic_supervisor::<
        MailAddr,
        PaymentWorker,
        Recipient<DynamicReply>,
        _,
    >());
    let initialized = definition.initialize().unwrap();
    let mut parent = initialized.behavior;
    let started = parent
        .receive(
            MailAddr(9),
            DynamicSupervisorMessage::Start {
                nonce: 7,
                child: PaymentWorker,
                reply_to: Recipient::global(MailAddr(8)),
            },
        )
        .unwrap();
    let creation = started.creates.into_iter().next().unwrap();
    interpret_proxy_report(&mut parent, creation).await;
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
async fn outer_guardian_interprets_worker_pool_proxy_reports() {
    let definition = Guardian::new(
        worker_pool(
            ChildTopology::new([7], pool_worker),
            PoolConfiguration::new(
                4,
                InterruptionPolicy::Retry,
                RestartPolicy::Permanent,
                2,
                Duration::from_secs(30),
            ),
            Recipient::global(MailAddr(6)),
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
async fn outer_guardian_interprets_keyed_worker_pool_proxy_reports() {
    let definition = Guardian::new(
        keyed_worker_pool(
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
        )
        .unwrap(),
    );
    let initialized = definition.initialize().unwrap();
    let mut parent = initialized.behavior;
    let creation = initialized.actions.creates.into_iter().next().unwrap();
    interpret_proxy_report(&mut parent, creation).await;
}
