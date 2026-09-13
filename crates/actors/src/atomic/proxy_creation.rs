//! Exact creation result for a StableProxy child.

use behavior::{
    Behavior, BehaviorAddr, ChildCreationOutcome, ChildHead, CreationId, CreationKind,
    CreationRejection, EndpointAddress, EstablishedActor, ItemSettlement, Never, RoutedCreation,
    SettledItem,
};

use super::{ActivationPlan, StableProxy};

pub(crate) type StableProxyCreationSettlement<Worker, Plan> = SettledItem<
    RoutedCreation<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    ItemSettlement<
        RoutedCreation<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
        ChildCreationOutcome<StableProxy<Worker, Plan>, ChildHead>,
        CreationRejection,
        Never,
    >,
>;

pub(crate) enum StableProxyCreation<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Committed(EstablishedActor<StableProxy<Worker, Plan>>),
    Rejected(StableProxyCreationSettlement<Worker, Plan>),
}

pub(crate) fn identity<Worker, Plan>(
    settlement: &StableProxyCreationSettlement<Worker, Plan>,
) -> (CreationId, CreationKind)
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    match settlement {
        SettledItem::Attempted(ItemSettlement::Accepted(created)) => match created {
            ChildCreationOutcome::Established { established } => {
                (established.id(), established.kind())
            }
            ChildCreationOutcome::InitializationRejected { creation, .. }
            | ChildCreationOutcome::HostRejected { creation, .. } => {
                (creation.id(), creation.kind())
            }
        },
        SettledItem::Attempted(ItemSettlement::Rejected { item, .. })
        | SettledItem::Attempted(ItemSettlement::Corrupt { item, .. })
        | SettledItem::Unattempted(item) => (item.id(), item.kind()),
        SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
            match *prerequisite {}
        }
    }
}

pub(crate) fn resolve<Worker, Plan>(
    settlement: StableProxyCreationSettlement<Worker, Plan>,
) -> StableProxyCreation<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let (_, kind) = identity(&settlement);
    match kind {
        CreationKind::Birth => match settlement {
            SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established { established },
            )) => match (ChildCreationOutcome::Established { established }).into_actor() {
                Ok(proxy) => StableProxyCreation::Committed(proxy),
                Err(created) => StableProxyCreation::Rejected(SettledItem::Attempted(
                    ItemSettlement::Accepted(created),
                )),
            },
            settlement => StableProxyCreation::Rejected(settlement),
        },
        CreationKind::Replacement { .. } => StableProxyCreation::Rejected(settlement),
    }
}
