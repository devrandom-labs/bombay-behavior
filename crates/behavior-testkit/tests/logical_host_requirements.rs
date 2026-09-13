//! Structural logical-host projection from real sends and birth algebras.

use behavior::DeliveryOutcomes;
use core::marker::PhantomData;
use foundation::{
    Actions, Address, Behavior, BehaviorActed, BirthProtocol, BirthProtocolAt, BirthProtocolHead,
    BirthProtocolProduct, BirthProtocolTail, Births, ChildChoice, Delivery, EndpointAddress,
    EstablishedDelivery, LogicalHostRequirements, Never, NoBirthProtocols, NoBirths, Protocol,
    User,
};

#[derive(Clone, Copy, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P>(PhantomData<fn() -> P>);

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self(PhantomData)
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct RootProtocol;
struct PublicCommands;
struct StableDestination;
struct ExactOnly;

macro_rules! protocol {
    ($protocol:ty) => {
        impl Protocol for $protocol {
            type Addr = RuntimeAddr;
            type Msg = ();
        }
    };
}

protocol!(RootProtocol);
protocol!(PublicCommands);
protocol!(StableDestination);
protocol!(ExactOnly);

struct Leaf;

impl Behavior for Leaf {
    type Protocol = StableDestination;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Delivery<StableDestination>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Application;

impl Behavior for Application {
    type Protocol = RootProtocol;
    type Event = User<RuntimeAddr, ()>;
    type Sends =
        DeliveryOutcomes<Vec<EstablishedDelivery<ExactOnly>>, Vec<Delivery<PublicCommands>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<ChildChoice<Leaf, ChildChoice<Leaf, Never>>>;

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

trait Same<T> {}
impl<T> Same<T> for T {}

type Expected = BirthProtocol<
    PublicCommands,
    BirthProtocol<StableDestination, BirthProtocol<StableDestination, NoBirthProtocols>>,
>;

#[test]
fn complete_product_is_derived_without_owner_authored_metadata() {
    type Actual = <Application as LogicalHostRequirements>::LogicalHosts;
    fn exact<T: Same<Expected>>() {}
    exact::<Actual>();
}

#[test]
fn duplicate_child_destinations_keep_distinct_structural_positions() {
    type Hosts = <Application as LogicalHostRequirements>::LogicalHosts;

    fn contains<P: Protocol, Position, Product: BirthProtocolAt<P, Position>>() {}

    contains::<PublicCommands, BirthProtocolHead, Hosts>();
    contains::<StableDestination, BirthProtocolTail<BirthProtocolHead>, Hosts>();
    contains::<StableDestination, BirthProtocolTail<BirthProtocolTail<BirthProtocolHead>>, Hosts>();
}

trait Hosts<P: Protocol> {}

struct ApplicationSpaces;

impl Hosts<PublicCommands> for ApplicationSpaces {}
impl Hosts<StableDestination> for ApplicationSpaces {}

trait HostsProduct<Product: BirthProtocolProduct> {}

impl HostsProduct<NoBirthProtocols> for ApplicationSpaces {}

impl<P, Tail> HostsProduct<BirthProtocol<P, Tail>> for ApplicationSpaces
where
    P: Protocol,
    Tail: BirthProtocolProduct,
    ApplicationSpaces: Hosts<P> + HostsProduct<Tail>,
{
}

#[test]
fn a_framework_consumes_repeated_requirements_without_normalizing_them() {
    fn requires_every_host<B, Spaces>()
    where
        B: LogicalHostRequirements,
        Spaces: HostsProduct<B::LogicalHosts>,
    {
    }

    requires_every_host::<Application, ApplicationSpaces>();
}
