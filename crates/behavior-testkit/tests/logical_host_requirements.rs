//! Structural logical-host projection from real sends and birth algebras.

mod installed_control;

use behavior_actors::DeliveryOutcomes;
use behavior_actors::atomic::{CustomerDelivery, DiagnosticAction};
use behavior_core::{
    Actions, Address, Behavior, BehaviorActed, BirthProtocol, BirthProtocolAt, BirthProtocolHead,
    BirthProtocolProduct, BirthProtocolTail, Births, ChildChoice, Delivery, EndpointAddress,
    EstablishedDelivery, EstablishedRecipient, InterpreterRequests, LogicalDeliveryProtocols,
    LogicalHostRequirements, Never, NoBirthProtocols, NoBirths, Protocol, Recipient, SendLayer,
    User,
};
use core::marker::PhantomData;

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

    type Installed<B>
        =
        installed_control::InstalledControl<B, <Self as EndpointAddress>::Established<B::Protocol>>
    where
        B: behavior_core::Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: behavior_core::Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
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

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
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

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
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

#[test]
fn interpreter_requests_project_possible_logical_customer_and_diagnostic_routes() {
    type Expected = BirthProtocol<PublicCommands, NoBirthProtocols>;
    fn exact<T: Same<Expected>>() {}
    fn empty<T: Same<NoBirthProtocols>>() {}

    type CustomerHosts =
        <InterpreterRequests<CustomerDelivery<PublicCommands>> as LogicalDeliveryProtocols>::Protocols;
    type LogicalDiagnosticHosts = <InterpreterRequests<
        DiagnosticAction<Recipient<PublicCommands>, ()>,
    > as LogicalDeliveryProtocols>::Protocols;
    type ExactDiagnosticHosts = <InterpreterRequests<
        DiagnosticAction<EstablishedRecipient<PublicCommands>, ()>,
    > as LogicalDeliveryProtocols>::Protocols;

    exact::<CustomerHosts>();
    exact::<LogicalDiagnosticHosts>();
    empty::<ExactDiagnosticHosts>();
}

#[test]
fn customer_routes_keep_the_two_wrapper_orders_distinct() {
    type InnerFirst = SendLayer<
        InterpreterRequests<CustomerDelivery<PublicCommands>>,
        Vec<Delivery<StableDestination>>,
    >;
    type OuterFirst = SendLayer<
        Vec<Delivery<StableDestination>>,
        InterpreterRequests<CustomerDelivery<PublicCommands>>,
    >;
    type InnerFirstExpected =
        BirthProtocol<StableDestination, BirthProtocol<PublicCommands, NoBirthProtocols>>;
    type OuterFirstExpected =
        BirthProtocol<PublicCommands, BirthProtocol<StableDestination, NoBirthProtocols>>;

    fn inner_first<T: Same<InnerFirstExpected>>() {}
    fn outer_first<T: Same<OuterFirstExpected>>() {}
    inner_first::<<InnerFirst as LogicalDeliveryProtocols>::Protocols>();
    outer_first::<<OuterFirst as LogicalDeliveryProtocols>::Protocols>();
}

#[test]
fn repeated_customer_request_routes_keep_both_occurrences() {
    type Sends = SendLayer<
        InterpreterRequests<CustomerDelivery<PublicCommands>>,
        InterpreterRequests<CustomerDelivery<PublicCommands>>,
    >;
    type Expected = BirthProtocol<PublicCommands, BirthProtocol<PublicCommands, NoBirthProtocols>>;
    fn exact<T: Same<Expected>>() {}

    exact::<<Sends as LogicalDeliveryProtocols>::Protocols>();
}
