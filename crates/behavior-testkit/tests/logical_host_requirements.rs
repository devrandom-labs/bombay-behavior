//! Public, owner-authored metadata for transitive logical destinations.

use behavior::{
    Actions, Behavior, BehaviorActed, BirthProtocol, BirthProtocolProduct, Births, ChildChoice,
    Delivery, LogicalHostRequirements, MailAddr, Never, NoBirthProtocols, NoBirths, Protocol,
    RequirementAt, RequirementHead, RequirementTail, User,
};

struct RootProtocol;
struct PublicCommands;
struct StableDestination;
struct ExactOnly;

macro_rules! protocol {
    ($protocol:ty) => {
        impl Protocol for $protocol {
            type Addr = MailAddr;
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
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Delivery<StableDestination>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

impl LogicalHostRequirements for Leaf {
    type LogicalHosts = BirthProtocol<StableDestination, NoBirthProtocols>;
}

struct Application;

impl Behavior for Application {
    type Protocol = RootProtocol;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Delivery<PublicCommands>>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<ChildChoice<Leaf, ChildChoice<Leaf, Never>>>;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

impl LogicalHostRequirements for Application {
    type LogicalHosts = BirthProtocol<
        PublicCommands,
        BirthProtocol<StableDestination, BirthProtocol<StableDestination, NoBirthProtocols>>,
    >;
}

trait MaterializeHosts {
    const COUNT: usize;
}

impl MaterializeHosts for NoBirthProtocols {
    const COUNT: usize = 0;
}

impl<P, Tail> MaterializeHosts for BirthProtocol<P, Tail>
where
    P: Protocol,
    Tail: BirthProtocolProduct + MaterializeHosts,
{
    const COUNT: usize = 1 + Tail::COUNT;
}

fn framework_host_count<B>() -> usize
where
    B: LogicalHostRequirements,
    B::LogicalHosts: MaterializeHosts,
{
    B::LogicalHosts::COUNT
}

#[test]
fn generic_framework_materializes_the_complete_ordered_product() {
    assert_eq!(framework_host_count::<Application>(), 3);
}

#[test]
fn duplicate_logical_protocol_occurrences_keep_distinct_positions() {
    type Hosts = <Application as LogicalHostRequirements>::LogicalHosts;

    fn contains<P: Protocol, Position, Product: RequirementAt<P, Position>>() {}

    contains::<PublicCommands, RequirementHead, Hosts>();
    contains::<StableDestination, RequirementTail<RequirementHead>, Hosts>();
    contains::<StableDestination, RequirementTail<RequirementTail<RequirementHead>>, Hosts>();
}

#[test]
fn exact_only_protocols_are_not_declared_as_logical_hosts() {
    let _exact_only_protocol = ExactOnly;
    type Hosts = <Application as LogicalHostRequirements>::LogicalHosts;
    trait Exactly<T> {}
    impl<T> Exactly<T> for T {}

    type Expected = BirthProtocol<
        PublicCommands,
        BirthProtocol<StableDestination, BirthProtocol<StableDestination, NoBirthProtocols>>,
    >;
    fn exact<Product: Exactly<Expected>>() {}
    exact::<Hosts>();
}
