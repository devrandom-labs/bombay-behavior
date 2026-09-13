use behavior_actors::atomic::{ActivationPlan, ImmediateActivation, StableProxy};
use behavior_actors::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr, EndpointAddress, Never,
    NoBirths, Protocol, User,
};

fn accepts_proxy_child<Worker, Plan>()
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn accepts_behavior<Actor>()
    where
        Actor: Behavior,
    {
    }

    accepts_behavior::<StableProxy<Worker, Plan>>();
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct RuntimeAddress;

impl Address for RuntimeAddress {
    type Nonce = u16;
}

#[derive(Clone, Copy)]
struct AccountEndpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = AccountEndpoint
    where
        P: Protocol<Addr = Self>;
}

struct AccountWorker;

impl Protocol for AccountWorker {
    type Addr = RuntimeAddress;
    type Msg = u8;
}

impl Behavior for AccountWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[test]
fn a_proxy_owner_needs_only_the_proxy_behavior_contract() {
    accepts_proxy_child::<AccountWorker, ImmediateActivation>();
}
