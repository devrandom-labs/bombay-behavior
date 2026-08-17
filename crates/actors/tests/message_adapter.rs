use behavior_actors::{
    Actions, Behavior, BehaviorActed, DynamicSupervisor, DynamicSupervisorOutcome, MailAddr,
    MessageAdapter, Never, NoBirths, PoolAssignment, PoolResponse, Recipient, User, WorkerPool,
};

struct Device;

impl Behavior for Device {
    type Addr = MailAddr;
    type Msg = Never;
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

impl Behavior for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
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

impl Behavior for Root {
    type Addr = MailAddr;
    type Msg = SystemMessage;
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
