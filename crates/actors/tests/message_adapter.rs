use behavior_actors::{
    Actions, Activate, Address, Behavior, BehaviorActed, Delivery, DynamicSupervisor,
    DynamicSupervisorMessage, DynamicSupervisorOutcome, DynamicSupervisorProtocol, EndpointAddress,
    JobId, MessageAdapter, Never, NoBirths, PoolMessage, PoolResponse, Proxy, Recipient,
    StopOnShutdown, User, WorkerPoolProtocol,
};
use core::marker::PhantomData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct RuntimeEndpoint<P>(PhantomData<fn() -> P>);

impl<P> Clone for RuntimeEndpoint<P> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = RuntimeEndpoint<P>
    where
        P: behavior::Protocol<Addr = Self>;
}

struct Device;

impl behavior::Protocol for Device {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Device {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
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

struct Root;

impl behavior::Protocol for Root {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl Behavior for Root {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
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

fn adapt_supervisor(_: DynamicSupervisorOutcome<RuntimeAddr, Device>) {}

fn adapt_pool(_: PoolResponse<u8, u16, RuntimeAddr>) {}

type SupervisorReply = MessageAdapter<DynamicSupervisorOutcome<RuntimeAddr, Device>, Root>;
type PoolReply = MessageAdapter<PoolResponse<u8, u16, RuntimeAddr>, Root>;

fn assert_behavior<B: Behavior>(_: &B) {}

#[test]
fn adapter_is_a_concrete_reply_protocol_for_supervisors_and_pools() {
    let dynamic =
        DynamicSupervisor::<RuntimeAddr, Device, Recipient<SupervisorReply>, _>::new(Proxy::new);
    assert_behavior(&dynamic);

    let root = Recipient::<Root>::global(RuntimeAddr(1));
    let supervisor_reply: SupervisorReply = MessageAdapter::new(root, adapt_supervisor);
    let pool_reply: PoolReply = MessageAdapter::new(root, adapt_pool);
    assert_eq!(supervisor_reply.destination().address(), RuntimeAddr(1));
    assert_eq!(pool_reply.destination().address(), RuntimeAddr(1));
}

struct ActualRoot;

type ActualReply = MessageAdapter<DynamicSupervisorOutcome<RuntimeAddr, Device>, ActualRoot>;
type ActualSupervisorProtocol =
    DynamicSupervisorProtocol<RuntimeAddr, Device, Recipient<ActualReply>>;

impl behavior::Protocol for ActualRoot {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl Behavior for ActualRoot {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Delivery<ActualSupervisorProtocol>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        let supervisor = Recipient::<ActualSupervisorProtocol>::global(RuntimeAddr(2));
        let reply_to = Recipient::<ActualReply>::global(RuntimeAddr(3));
        Ok(Actions::send(vec![Delivery::new(
            supervisor,
            DynamicSupervisorMessage::Query { nonce: 7, reply_to },
        )]))
    }
}

fn adapt_actual_root(
    _: DynamicSupervisorOutcome<RuntimeAddr, Device>,
) -> <ActualRoot as behavior_actors::Protocol>::Msg {
}

#[test]
fn adapter_can_target_the_root_that_sends_to_its_dynamic_supervisor() {
    assert_behavior(&StopOnShutdown::new(ActualRoot));
    let root = Recipient::<ActualRoot>::global(RuntimeAddr(1));
    let mut adapter = MessageAdapter::new(root, adapt_actual_root)
        .initialize()
        .unwrap()
        .behavior;
    let actions = adapter
        .receive(
            RuntimeAddr(99),
            DynamicSupervisorOutcome::State {
                nonce: 7,
                phase: None,
            },
        )
        .unwrap();
    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].to.address(), RuntimeAddr(1));
    assert_eq!(actions.sends[0].message, ());
}

struct PoolRoot;

type ActualPoolReply = MessageAdapter<PoolResponse<u8, u16, RuntimeAddr>, PoolRoot>;
impl behavior::Protocol for PoolRoot {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl Behavior for PoolRoot {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
    type Sends =
        Vec<Delivery<WorkerPoolProtocol<RuntimeAddr, u8, u16, Recipient<ActualPoolReply>>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        let pool = Recipient::global(RuntimeAddr(2));
        let reply_to = Recipient::<ActualPoolReply>::global(RuntimeAddr(3));
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

fn adapt_actual_pool(_: PoolResponse<u8, u16, RuntimeAddr>) {}

#[test]
fn adapter_can_target_the_root_that_sends_to_its_worker_pool() {
    assert_behavior(&StopOnShutdown::new(PoolRoot));
    let root = Recipient::<PoolRoot>::global(RuntimeAddr(1));
    let reply: ActualPoolReply = MessageAdapter::new(root, adapt_actual_pool);
    assert_eq!(reply.destination().address(), RuntimeAddr(1));
}
