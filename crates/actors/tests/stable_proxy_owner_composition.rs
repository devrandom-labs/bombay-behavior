use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr, BirthProtocol, Delivery,
    EndpointAddress, LogicalHostRequirements, Never, NoBirthProtocols, NoBirths, Protocol, User,
};
use behavior_actors::atomic::{ActivationPlan, ImmediateActivation, StableProxy};

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

struct HostedNotice;

impl Protocol for HostedNotice {
    type Addr = RuntimeAddress;
    type Msg = ();
}

impl Protocol for AccountWorker {
    type Addr = RuntimeAddress;
    type Msg = u8;
}

impl Behavior for AccountWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, u8>;
    type Sends = Vec<Delivery<HostedNotice>>;
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

#[test]
fn proxy_hosts_the_worker_transitive_logical_destination_once() {
    trait Same<T> {}
    impl<T> Same<T> for T {}
    fn exact<T: Same<BirthProtocol<HostedNotice, NoBirthProtocols>>>() {}

    exact::<
        <StableProxy<AccountWorker, ImmediateActivation> as LogicalHostRequirements>::LogicalHosts,
    >();
}
