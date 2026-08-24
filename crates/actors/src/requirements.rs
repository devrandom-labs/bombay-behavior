//! Static local-installation requirements derived from Behavior births.

use behavior::{Behavior, BirthProtocols};

/// Complete closed product of protocols for actors a behavior may install
/// locally, including itself and every transitive birth alternative.
///
/// The product is derived solely from the behavior's staged-birth algebra.
/// Merely external delivery destinations are therefore excluded. Repeated
/// protocol occurrences are preserved with distinct structural positions;
/// application-wide canonicalization remains a runtime-composition concern.
///
/// A protocol mentioned only by a delivery lane is not a requirement:
///
/// ```compile_fail
/// use behavior_actors::{
///     Actions, Behavior, BehaviorActed, Delivery, InstallationRequirements,
///     MailAddr, Never, NoBirths, Protocol, RequirementAt, RequirementHead,
///     User,
/// };
/// struct RootProtocol;
/// struct ExternalProtocol;
/// impl Protocol for RootProtocol { type Addr = MailAddr; type Msg = (); }
/// impl Protocol for ExternalProtocol { type Addr = MailAddr; type Msg = (); }
/// struct Root;
/// impl Behavior for Root {
///     type Protocol = RootProtocol;
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Delivery<ExternalProtocol>>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(
///         &mut self,
///         _: behavior_actors::ActiveTurn,
///         _: Self::Event,
///     ) -> BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// type Requirements = <Root as InstallationRequirements>::Requirements;
/// fn external_is_local<T: RequirementAt<ExternalProtocol, RequirementHead>>() {}
/// external_is_local::<Requirements>();
/// ```
pub trait InstallationRequirements: Behavior {
    /// Ordered protocol occurrences reachable through local installation.
    type Requirements: behavior::BirthProtocolProduct;
}

impl<B> InstallationRequirements for B
where
    B: BirthProtocols,
{
    type Requirements = B::Protocols;
}

/// Owner-authored, complete product of protocols requiring logical hosting.
///
/// This metadata covers every intentional [`behavior::Recipient`] destination
/// reachable through the owner's composed behavior, including transitive
/// child and inner-behavior destinations. Exact established recipients are not
/// logical hosts and must not be listed merely because they share a protocol.
/// Repeated occurrences remain repeated when they represent distinct
/// composition requirements.
///
/// Completeness is a Bombay owner contract rather than an actor-model law or
/// an inference from `Sends`: generic Rust cannot recover every protocol from
/// arbitrary nested effect products without unconstrained type parameters.
/// A framework can recursively implement its hosting operation for
/// [`behavior::BirthProtocol`] and [`behavior::NoBirthProtocols`], then consume
/// [`LogicalHosts`](Self::LogicalHosts) without a registry, erased envelope, or
/// runtime protocol lookup.
pub trait LogicalHostRequirements: Behavior {
    /// Ordered, duplicate-preserving logical protocol occurrences.
    type LogicalHosts: behavior::BirthProtocolProduct;
}

/// Empty local-installation requirement product.
pub type NoInstallationRequirements = behavior::NoBirthProtocols;

/// One locally installable protocol followed by the remaining closed product.
pub type RequiredProtocol<P, Tail> = behavior::BirthProtocol<P, Tail>;

/// Structural position selecting the current required protocol.
pub type RequirementHead = behavior::BirthProtocolHead;

/// Structural position selecting inside the remaining requirement product.
pub type RequirementTail<Position> = behavior::BirthProtocolTail<Position>;

/// Static membership evidence for one protocol occurrence.
pub trait RequirementAt<P: behavior::Protocol, Position>:
    behavior::BirthProtocolAt<P, Position>
{
}

impl<Product, P, Position> RequirementAt<P, Position> for Product
where
    P: behavior::Protocol,
    Product: behavior::BirthProtocolAt<P, Position>,
{
}

#[cfg(test)]
mod tests {
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
    struct DynamicReplies;
    #[derive(Clone)]
    struct Job;
    struct ResultValue;

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

    type PoolProtocol = crate::WorkerPoolProtocol<
        MailAddr,
        PoolReplies,
        Job,
        ResultValue,
        behavior::Recipient<PoolReplies>,
    >;

    impl Protocol for PoolWorker {
        type Addr = MailAddr;
        type Msg = crate::PoolAssignment<PoolProtocol>;
    }

    impl Protocol for DynamicReplies {
        type Addr = MailAddr;
        type Msg = crate::DynamicSupervisorOutcome<MailAddr, PoolWorker>;
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
        type Event = User<MailAddr, crate::PoolAssignment<PoolProtocol>>;
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
        type Requirements = <Root as InstallationRequirements>::Requirements;

        fn contains<P: Protocol, Position, Product: RequirementAt<P, Position>>() {}

        contains::<RootProtocol, RequirementHead, Requirements>();
        contains::<SharedProtocol, RequirementTail<RequirementHead>, Requirements>();
        contains::<LeafProtocol, RequirementTail<RequirementTail<RequirementHead>>, Requirements>();
        contains::<
            SharedProtocol,
            RequirementTail<RequirementTail<RequirementTail<RequirementHead>>>,
            Requirements,
        >();
    }

    #[test]
    fn worker_pool_requirements_include_pool_proxy_and_worker_protocols() {
        type Pool = crate::WorkerPool<
            MailAddr,
            PoolReplies,
            Job,
            ResultValue,
            PoolWorker,
            behavior::Recipient<PoolReplies>,
        >;
        type Requirements = <Pool as InstallationRequirements>::Requirements;
        type Expected = RequiredProtocol<
            PoolProtocol,
            RequiredProtocol<
                crate::Proxy<PoolWorker>,
                RequiredProtocol<PoolWorker, NoInstallationRequirements>,
            >,
        >;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}
        exact::<Requirements>();
    }

    #[test]
    fn transparent_wrapper_preserves_inner_transitive_requirements_once() {
        type Wrapped = crate::StopOnShutdown<Primary>;
        type Requirements = <Wrapped as InstallationRequirements>::Requirements;
        type Expected = RequiredProtocol<
            SharedProtocol,
            RequiredProtocol<LeafProtocol, NoInstallationRequirements>,
        >;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}
        exact::<Requirements>();
    }

    #[test]
    fn supervision_templates_expose_proxy_and_transitive_child_requirements() {
        type ProxyRequirements =
            <crate::Proxy<PoolWorker> as InstallationRequirements>::Requirements;
        type ProxyExpected = RequiredProtocol<
            crate::Proxy<PoolWorker>,
            RequiredProtocol<PoolWorker, NoInstallationRequirements>,
        >;
        type SupervisorRequirements =
            <crate::Supervisor<MailAddr, PoolWorker> as InstallationRequirements>::Requirements;
        type SupervisorExpected = RequiredProtocol<
            crate::SupervisorProtocol<MailAddr>,
            RequiredProtocol<
                crate::Proxy<PoolWorker>,
                RequiredProtocol<PoolWorker, NoInstallationRequirements>,
            >,
        >;
        type BackoffRequirements =
            <crate::BackoffSupervisor<MailAddr, PoolWorker> as InstallationRequirements>::Requirements;
        type Dynamic =
            crate::DynamicSupervisor<MailAddr, PoolWorker, behavior::Recipient<DynamicReplies>>;
        type DynamicRequirements = <Dynamic as InstallationRequirements>::Requirements;
        type DynamicExpected = RequiredProtocol<
            Dynamic,
            RequiredProtocol<
                crate::Proxy<PoolWorker>,
                RequiredProtocol<PoolWorker, NoInstallationRequirements>,
            >,
        >;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>, Expected>() {}

        exact::<ProxyRequirements, ProxyExpected>();
        exact::<SupervisorRequirements, SupervisorExpected>();
        exact::<BackoffRequirements, SupervisorExpected>();
        exact::<DynamicRequirements, DynamicExpected>();
    }

    #[test]
    fn independently_projected_products_compose_without_losing_occurrences() {
        type LeafRequirements = <Leaf as InstallationRequirements>::Requirements;
        type FallbackRequirements = <Fallback as InstallationRequirements>::Requirements;
        type Combined =
            <LeafRequirements as behavior::BirthProtocolProduct>::Append<FallbackRequirements>;
        type Expected = RequiredProtocol<
            LeafProtocol,
            RequiredProtocol<SharedProtocol, NoInstallationRequirements>,
        >;

        trait Same<T> {}
        impl<T> Same<T> for T {}
        fn exact<T: Same<Expected>>() {}
        exact::<Combined>();
    }
}
