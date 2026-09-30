#[path = "keyed_pool/assignment.rs"]
mod assignment;
#[path = "support/assignment_delivery.rs"]
mod assignment_delivery;
#[path = "keyed_pool/compile.rs"]
mod compile;
#[path = "keyed_pool/customer.rs"]
mod customer;
mod direct_pool_customer;
#[path = "keyed_pool/domain.rs"]
mod domain;
#[path = "keyed_pool/lifecycle.rs"]
mod lifecycle;

fn keyed_pool_requires_its_customer_and_management_hosts<B>(_: &B)
where
    B: behavior::LogicalHostRequirements,
    B::LogicalHosts: behavior::BirthProtocolAt<
            behavior::MessageProtocol<
                domain::RuntimeAddr,
                behavior_actors::atomic::KeyedOutcome<
                    domain::Account,
                    domain::SearchRole,
                    domain::SearchJob,
                    domain::SearchResult,
                >,
            >,
            behavior::BirthProtocolHead,
        > + behavior::BirthProtocolAt<
            behavior::MessageProtocol<
                domain::RuntimeAddr,
                behavior_actors::atomic::BindingReply<
                    domain::RuntimeAddr,
                    domain::Account,
                    domain::SearchRole,
                >,
            >,
            behavior::BirthProtocolTail<behavior::BirthProtocolHead>,
        >,
{
}
