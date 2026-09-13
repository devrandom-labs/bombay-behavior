//! Static logical-delivery projections for handwritten actor send products.

use behavior::{
    BirthProtocol, BirthProtocolProduct, EndpointAddress, LogicalDeliveryProtocols,
    NoBirthProtocols, Protocol,
};

impl<ReplySends, Schedules> LogicalDeliveryProtocols for crate::BreakerSends<ReplySends, Schedules>
where
    ReplySends: LogicalDeliveryProtocols,
    Schedules: LogicalDeliveryProtocols,
{
    type Protocols = <ReplySends::Protocols as BirthProtocolProduct>::Append<Schedules::Protocols>;
}

impl<Deliveries, OutcomeSends> LogicalDeliveryProtocols
    for crate::BufferSends<Deliveries, OutcomeSends>
where
    Deliveries: LogicalDeliveryProtocols,
    OutcomeSends: LogicalDeliveryProtocols,
{
    type Protocols =
        <Deliveries::Protocols as BirthProtocolProduct>::Append<OutcomeSends::Protocols>;
}

impl<Deliveries, OutcomeSends> LogicalDeliveryProtocols
    for crate::DeliveryOutcomes<Deliveries, OutcomeSends>
where
    Deliveries: LogicalDeliveryProtocols,
    OutcomeSends: LogicalDeliveryProtocols,
{
    type Protocols =
        <Deliveries::Protocols as BirthProtocolProduct>::Append<OutcomeSends::Protocols>;
}

impl<Assignments, OutcomeSends> LogicalDeliveryProtocols
    for crate::WorkQueueSends<Assignments, OutcomeSends>
where
    Assignments: LogicalDeliveryProtocols,
    OutcomeSends: LogicalDeliveryProtocols,
{
    type Protocols =
        <Assignments::Protocols as BirthProtocolProduct>::Append<OutcomeSends::Protocols>;
}

impl<OutcomeSends, Schedules> LogicalDeliveryProtocols
    for crate::LeaseSends<OutcomeSends, Schedules>
where
    OutcomeSends: LogicalDeliveryProtocols,
    Schedules: LogicalDeliveryProtocols,
{
    type Protocols =
        <OutcomeSends::Protocols as BirthProtocolProduct>::Append<Schedules::Protocols>;
}

impl<P> LogicalDeliveryProtocols
    for crate::ReplyDeliveries<behavior::Delivery<P>, behavior::EstablishedDelivery<P>>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Protocols = BirthProtocol<P, NoBirthProtocols>;
}

impl<T> LogicalDeliveryProtocols for crate::HeterogeneousShutdownSends<T> {
    type Protocols = NoBirthProtocols;
}

impl<ReplySends, Schedules> LogicalDeliveryProtocols for crate::PresenceSends<ReplySends, Schedules>
where
    ReplySends: LogicalDeliveryProtocols,
    Schedules: LogicalDeliveryProtocols,
{
    type Protocols = <ReplySends::Protocols as BirthProtocolProduct>::Append<Schedules::Protocols>;
}

impl<Observations, Reports> LogicalDeliveryProtocols
    for crate::TerminalPropagationSends<Observations, Reports>
where
    Observations: LogicalDeliveryProtocols,
    Reports: LogicalDeliveryProtocols,
{
    type Protocols = <Observations::Protocols as BirthProtocolProduct>::Append<Reports::Protocols>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use behavior::{
        Actions, Behavior, BehaviorActed, Births, ChildChoice, MailAddr, Never, NoBirths, Protocol,
        User,
    };

    struct RootProtocol;
    struct SharedProtocol;
    struct LeafProtocol;
    struct ExternalProtocol;
    macro_rules! protocol {
        ($protocol:ty) => {
            impl Protocol for $protocol {
                type Addr = MailAddr;
                type Msg = ();
            }
        };
    }

    protocol!(RootProtocol);
    protocol!(SharedProtocol);
    protocol!(LeafProtocol);
    protocol!(ExternalProtocol);

    struct Leaf;
    struct Primary;
    struct Fallback;
    struct Root;

    macro_rules! behavior {
        ($behavior:ty, $protocol:ty, $birth:ty, $sends:ty) => {
            impl Behavior for $behavior {
                type Protocol = $protocol;
                type Event = User<MailAddr, ()>;
                type Sends = $sends;
                type Ph = Never;
                type Error = Never;
                type Birth = $birth;

                fn transition(
                    &mut self,
                    _: behavior::ActiveTurn,
                    _: Self::Event,
                ) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }
        };
    }

    behavior!(Leaf, LeafProtocol, NoBirths, Vec<Never>);
    behavior!(Primary, SharedProtocol, Births<Leaf>, Vec<Never>);
    behavior!(Fallback, SharedProtocol, NoBirths, Vec<Never>);
    behavior!(
        Root,
        RootProtocol,
        Births<ChildChoice<Primary, ChildChoice<Fallback, Never>>>,
        Vec<behavior::Delivery<ExternalProtocol>>
    );
    #[test]
    fn transitive_births_preserve_every_protocol_occurrence_and_exclude_sends() {
        type Requirements = <Root as behavior::BirthProtocols>::Protocols;

        fn contains<P: Protocol, Position, Product: behavior::BirthProtocolAt<P, Position>>() {}

        contains::<RootProtocol, behavior::BirthProtocolHead, Requirements>();
        contains::<
            SharedProtocol,
            behavior::BirthProtocolTail<behavior::BirthProtocolHead>,
            Requirements,
        >();
        contains::<
            LeafProtocol,
            behavior::BirthProtocolTail<behavior::BirthProtocolTail<behavior::BirthProtocolHead>>,
            Requirements,
        >();
        contains::<
            SharedProtocol,
            behavior::BirthProtocolTail<
                behavior::BirthProtocolTail<
                    behavior::BirthProtocolTail<behavior::BirthProtocolHead>,
                >,
            >,
            Requirements,
        >();
    }

    #[test]
    fn transparent_wrapper_preserves_inner_transitive_requirements_once() {
        type Wrapped = crate::StopOnShutdown<Primary>;
        type Requirements = <Wrapped as behavior::BirthProtocols>::Protocols;
        type Expected =
            BirthProtocol<SharedProtocol, BirthProtocol<LeafProtocol, NoBirthProtocols>>;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}
        exact::<Requirements>();
    }

    #[test]
    fn independently_projected_products_compose_without_losing_occurrences() {
        type LeafRequirements = <Leaf as behavior::BirthProtocols>::Protocols;
        type FallbackRequirements = <Fallback as behavior::BirthProtocols>::Protocols;
        type Combined =
            <LeafRequirements as behavior::BirthProtocolProduct>::Append<FallbackRequirements>;
        type Expected =
            BirthProtocol<LeafProtocol, BirthProtocol<SharedProtocol, NoBirthProtocols>>;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}
        exact::<Combined>();
    }

    #[test]
    fn logical_projection_follows_named_lanes_and_excludes_nonlogical_lanes() {
        type Named = crate::BufferSends<
            Vec<behavior::Delivery<ExternalProtocol>>,
            Vec<behavior::Delivery<SharedProtocol>>,
        >;
        type NamedActual = <Named as LogicalDeliveryProtocols>::Protocols;
        type NamedExpected =
            BirthProtocol<ExternalProtocol, BirthProtocol<SharedProtocol, NoBirthProtocols>>;
        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>, Expected>() {}

        exact::<NamedActual, NamedExpected>();
    }

    #[test]
    fn wrapper_projection_preserves_root_and_transitive_logical_occurrences() {
        type Wrapped = crate::StopOnShutdown<Root>;
        type Actual = <Wrapped as behavior::LogicalHostRequirements>::LogicalHosts;
        type Expected = BirthProtocol<ExternalProtocol, NoBirthProtocols>;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}

        exact::<Actual>();
    }
}
