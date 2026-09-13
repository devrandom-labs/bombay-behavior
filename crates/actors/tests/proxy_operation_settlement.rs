use behavior_actors::atomic::{
    ImmediateActivation, ProxyInputReceipt, ProxyInputResult, ProxyOperation,
};
use behavior_actors::{
    ActionItem, ActionItemResult, ActiveTurn, Address, Behavior, BehaviorActed, ChildInputReason,
    EndpointAddress, Never, NoBirths, NoSends, Protocol, SourceAction, User,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Owner;

fn exact_proxy_operation_contract<Item>()
where
    Item: ActionItem<
            Accepted = ProxyInputReceipt<Worker, ImmediateActivation>,
            Rejection = ChildInputReason,
            Prerequisite = Never,
        > + SourceAction<Source = Owner>,
{
}

fn exact_proxy_result(
    result: ProxyInputResult<Owner, Worker, ImmediateActivation>,
) -> ActionItemResult<ProxyOperation<Owner, Worker, ImmediateActivation>> {
    result
}

#[test]
fn proxy_operation_uses_the_generic_source_settlement_contract() {
    exact_proxy_operation_contract::<ProxyOperation<Owner, Worker, ImmediateActivation>>();
    let result = None::<ProxyInputResult<Owner, Worker, ImmediateActivation>>;
    assert!(result.map(exact_proxy_result).is_none());
}
