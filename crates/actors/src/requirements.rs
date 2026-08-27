//! Static logical-delivery projections for handwritten actor send products.

use behavior::{
    Address, Behavior, BirthProtocol, BirthProtocolProduct, EndpointAddress,
    LogicalDeliveryProtocols, Never, NoBirthProtocols, Protocol,
};

impl<ReplySends> LogicalDeliveryProtocols for crate::BreakerSends<ReplySends>
where
    ReplySends: LogicalDeliveryProtocols,
{
    type Protocols = ReplySends::Protocols;
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

impl<A, C, Stable, ResponseSends> LogicalDeliveryProtocols
    for crate::PoolSends<A, C, Stable, ResponseSends>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never, Protocol: Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
    ResponseSends: LogicalDeliveryProtocols,
{
    type Protocols = ResponseSends::Protocols;
}

impl<OutcomeSends> LogicalDeliveryProtocols for crate::LeaseSends<OutcomeSends>
where
    OutcomeSends: LogicalDeliveryProtocols,
{
    type Protocols = OutcomeSends::Protocols;
}

impl<A, C, Route, Stable> LogicalDeliveryProtocols
    for crate::DynamicSupervisorSends<A, C, Route, Stable>
where
    A: EndpointAddress,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never, Protocol: Protocol<Addr = A>>,
    Route: crate::DeliveryRoute,
    Route::Protocol: Protocol<Addr = A, Msg = crate::DynamicSupervisorOutcome<A, C>>,
    Route::Sends: LogicalDeliveryProtocols,
    Stable: Behavior<Ph = Never, Protocol = C::Protocol>,
{
    type Protocols = <Route::Sends as LogicalDeliveryProtocols>::Protocols;
}

impl<P> LogicalDeliveryProtocols for crate::ReplyDeliveries<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Protocols = BirthProtocol<P, NoBirthProtocols>;
}

impl<T> LogicalDeliveryProtocols for crate::HeterogeneousShutdownSends<T> {
    type Protocols = NoBirthProtocols;
}

impl<ReplySends> LogicalDeliveryProtocols for crate::PresenceSends<ReplySends>
where
    ReplySends: LogicalDeliveryProtocols,
{
    type Protocols = ReplySends::Protocols;
}

impl<A, Request> LogicalDeliveryProtocols for crate::TerminalPropagationSends<A, Request>
where
    A: Address,
{
    type Protocols = NoBirthProtocols;
}

impl<C> LogicalDeliveryProtocols for crate::ProxySends<C>
where
    C: Behavior<Ph = Never>,
{
    type Protocols = NoBirthProtocols;
}

impl<A, C, Stable> LogicalDeliveryProtocols for crate::SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: Protocol<Addr = A>>,
{
    type Protocols = NoBirthProtocols;
}

#[cfg(test)]
mod tests {
    use core::marker::PhantomData;

    use super::*;
    use behavior::{
        Actions, BehaviorActed, Births, ChildChoice, MailAddr, Never, NoBirths, Protocol, User,
    };

    struct RootProtocol;
    struct SharedProtocol;
    struct LeafProtocol;
    struct ExternalProtocol;
    struct PoolReplies;
    struct PoolWorker;
    struct DynamicWorker;
    struct DynamicReplies;
    #[derive(Clone)]
    struct Job;
    struct ResultValue;

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct DynamicAddr(u64);

    impl Address for DynamicAddr {
        type Nonce = u64;
    }

    struct DynamicEndpoint<P>(PhantomData<fn() -> P>);

    impl<P> Clone for DynamicEndpoint<P> {
        fn clone(&self) -> Self {
            Self(PhantomData)
        }
    }

    impl EndpointAddress for DynamicAddr {
        type Established<P>
            = DynamicEndpoint<P>
        where
            P: Protocol<Addr = Self>;
    }

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

    impl Protocol for PoolReplies {
        type Addr = MailAddr;
        type Msg = crate::PoolResponse<Job, ResultValue, MailAddr>;
    }

    type PoolProtocol =
        crate::WorkerPoolProtocol<MailAddr, Job, ResultValue, behavior::Recipient<PoolReplies>>;

    impl Protocol for PoolWorker {
        type Addr = MailAddr;
        type Msg = crate::PoolAssignment<Job>;
    }

    impl Protocol for DynamicReplies {
        type Addr = DynamicAddr;
        type Msg = crate::DynamicSupervisorOutcome<DynamicAddr, DynamicWorker>;
    }

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
    impl Behavior for PoolWorker {
        type Protocol = Self;
        type Event = User<MailAddr, crate::PoolAssignment<Job>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    impl Protocol for DynamicWorker {
        type Addr = DynamicAddr;
        type Msg = crate::PoolAssignment<Job>;
    }

    impl Behavior for DynamicWorker {
        type Protocol = Self;
        type Event = User<DynamicAddr, crate::PoolAssignment<Job>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

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
    fn worker_pool_requirements_include_pool_proxy_and_worker_protocols() {
        type Expected = BirthProtocol<
            PoolProtocol,
            BirthProtocol<PoolWorker, BirthProtocol<PoolWorker, NoBirthProtocols>>,
        >;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<B>(_: &B)
        where
            B: behavior::BirthProtocols,
            B::Protocols: Same<Expected>,
        {
        }

        let pool = crate::WorkerPool::new(
            crate::ChildTopology::new([1], |_| Some(PoolWorker)),
            crate::PoolConfiguration::new(
                0,
                crate::InterruptionPolicy::Fail,
                crate::RestartPolicy::Permanent,
                1,
                std::time::Duration::MAX,
                crate::RestartTiming::Immediate,
            ),
            |worker: PoolWorker| crate::Proxy::new(worker),
        )
        .unwrap();
        exact(&pool);
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
    fn supervision_templates_expose_proxy_and_transitive_child_requirements() {
        type ProxyRequirements = <crate::Proxy<PoolWorker> as behavior::BirthProtocols>::Protocols;
        type ProxyExpected = BirthProtocol<PoolWorker, BirthProtocol<PoolWorker, NoBirthProtocols>>;
        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>, Expected>() {}
        fn exact_dynamic<B>(_: &B)
        where
            B: behavior::BirthProtocols,
            B::Protocol: Protocol<
                    Addr = DynamicAddr,
                    Msg = crate::DynamicSupervisorMessage<
                        DynamicAddr,
                        DynamicWorker,
                        behavior::Recipient<DynamicReplies>,
                    >,
                >,
            B::Protocols: behavior::BirthProtocolAt<
                    DynamicWorker,
                    behavior::BirthProtocolTail<behavior::BirthProtocolHead>,
                > + behavior::BirthProtocolAt<
                    DynamicWorker,
                    behavior::BirthProtocolTail<
                        behavior::BirthProtocolTail<behavior::BirthProtocolHead>,
                    >,
                >,
        {
        }

        exact::<ProxyRequirements, ProxyExpected>();
        let dynamic =
            crate::DynamicSupervisor::new(|worker: DynamicWorker| crate::Proxy::new(worker));
        exact_dynamic(&dynamic);
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
        type InternalActual = <crate::SupervisorSends<
            MailAddr,
            PoolWorker,
            crate::Proxy<PoolWorker>,
        > as LogicalDeliveryProtocols>::Protocols;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>, Expected>() {}

        exact::<NamedActual, NamedExpected>();
        exact::<InternalActual, NoBirthProtocols>();
    }

    #[test]
    fn logical_projection_reaches_real_pool_and_dynamic_child_trees() {
        type PoolExpected = BirthProtocol<PoolReplies, NoBirthProtocols>;
        type DynamicExpected = BirthProtocol<DynamicReplies, NoBirthProtocols>;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact_pool<B>(_: &B)
        where
            B: behavior::LogicalHostRequirements,
            B::LogicalHosts: Same<PoolExpected>,
        {
        }
        fn exact_dynamic<B>(_: &B)
        where
            B: behavior::LogicalHostRequirements,
            B::LogicalHosts: Same<DynamicExpected>,
        {
        }

        let pool = crate::WorkerPool::<
            MailAddr,
            Job,
            ResultValue,
            PoolWorker,
            behavior::Recipient<PoolReplies>,
            _,
        >::new(
            crate::ChildTopology::new([1], |_| Some(PoolWorker)),
            crate::PoolConfiguration::new(
                0,
                crate::InterruptionPolicy::Fail,
                crate::RestartPolicy::Permanent,
                1,
                std::time::Duration::MAX,
                crate::RestartTiming::Immediate,
            ),
            |worker: PoolWorker| crate::Proxy::new(worker),
        )
        .unwrap();
        let dynamic = crate::DynamicSupervisor::<
            DynamicAddr,
            DynamicWorker,
            behavior::Recipient<DynamicReplies>,
            _,
        >::new(|worker: DynamicWorker| crate::Proxy::new(worker));

        exact_pool(&pool);
        exact_dynamic(&dynamic);
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
