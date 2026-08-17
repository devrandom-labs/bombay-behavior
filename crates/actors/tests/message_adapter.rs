use behavior_actors::{
    Actions, Behavior, BehaviorActed, Delivery, DynamicSupervisor, DynamicSupervisorMessage,
    DynamicSupervisorOutcome, Guardian, JobId, MailAddr, MessageAdapter, Never, NoBirths,
    PoolAssignment, PoolMessage, PoolResponse, Recipient, User, WorkerPool,
};

struct Device;

impl behavior::Protocol for Device {
    type Addr = MailAddr;
    type Msg = Never;
}

impl Behavior for Device {
    type Event = User<MailAddr, Never>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Worker;

impl behavior::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
}

impl Behavior for Worker {
    type Event = User<MailAddr, Self::Msg>;
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

enum SystemMessage {
    DeviceSupervisor(DynamicSupervisorOutcome<u64, Device>),
    Pool(PoolResponse<u8, u16, MailAddr>),
}

struct Root;

impl behavior::Protocol for Root {
    type Addr = MailAddr;
    type Msg = SystemMessage;
}

impl Behavior for Root {
    type Event = User<MailAddr, Self::Msg>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match event.message {
            SystemMessage::DeviceSupervisor(outcome) => drop(outcome),
            SystemMessage::Pool(response) => drop(response),
        }
        Ok(Actions::cont())
    }
}

fn adapt_supervisor(outcome: DynamicSupervisorOutcome<u64, Device>) -> SystemMessage {
    SystemMessage::DeviceSupervisor(outcome)
}

fn adapt_pool(response: PoolResponse<u8, u16, MailAddr>) -> SystemMessage {
    SystemMessage::Pool(response)
}

type SupervisorReply = MessageAdapter<DynamicSupervisorOutcome<u64, Device>, Root>;
type PoolReply = MessageAdapter<PoolResponse<u8, u16, MailAddr>, Root>;

fn assert_behavior<B: Behavior>() {}

#[test]
fn adapter_is_a_concrete_reply_protocol_for_supervisors_and_pools() {
    assert_behavior::<DynamicSupervisor<MailAddr, Device, SupervisorReply>>();
    assert_behavior::<WorkerPool<MailAddr, PoolReply, u8, u16, Worker>>();

    let root = Recipient::<Root>::global(MailAddr(1));
    let _: SupervisorReply = MessageAdapter::new(root, adapt_supervisor);
    let _: PoolReply = MessageAdapter::new(root, adapt_pool);
}

struct ActualRoot;

type ActualReply = MessageAdapter<DynamicSupervisorOutcome<u64, Device>, Guardian<ActualRoot>>;
type ActualSupervisor = DynamicSupervisor<MailAddr, Device, ActualReply>;

impl behavior::Protocol for ActualRoot {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for ActualRoot {
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Delivery<ActualSupervisor>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        let supervisor = Recipient::<ActualSupervisor>::global(MailAddr(2));
        let reply_to = Recipient::<ActualReply>::global(MailAddr(3));
        Ok(Actions::send(vec![Delivery::new(
            supervisor,
            DynamicSupervisorMessage::Query { nonce: 7, reply_to },
        )]))
    }
}

fn adapt_actual_root(
    _: DynamicSupervisorOutcome<u64, Device>,
) -> <Guardian<ActualRoot> as behavior_actors::Protocol>::Msg {
}

#[test]
fn adapter_can_target_the_root_that_sends_to_its_dynamic_supervisor() {
    assert_behavior::<Guardian<ActualRoot>>();
    let root = Recipient::<Guardian<ActualRoot>>::global(MailAddr(1));
    let _: ActualReply = MessageAdapter::new(root, adapt_actual_root);
}

struct PoolRoot;

type ActualPoolReply = MessageAdapter<PoolResponse<u8, u16, MailAddr>, Guardian<PoolRoot>>;
type ActualPool = WorkerPool<MailAddr, ActualPoolReply, u8, u16, Worker>;

impl behavior::Protocol for PoolRoot {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for PoolRoot {
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Delivery<ActualPool>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        let pool = Recipient::<ActualPool>::global(MailAddr(2));
        let reply_to = Recipient::<ActualPoolReply>::global(MailAddr(3));
        Ok(Actions::send(vec![Delivery::new(
            pool,
            PoolMessage::Submit {
                job: JobId(7),
                payload: 11,
                reply_to,
            },
        )]))
    }
}

fn adapt_actual_pool(_: PoolResponse<u8, u16, MailAddr>) {}

#[test]
fn adapter_can_target_the_root_that_sends_to_its_worker_pool() {
    assert_behavior::<Guardian<PoolRoot>>();
    let root = Recipient::<Guardian<PoolRoot>>::global(MailAddr(1));
    let _: ActualPoolReply = MessageAdapter::new(root, adapt_actual_pool);
}
