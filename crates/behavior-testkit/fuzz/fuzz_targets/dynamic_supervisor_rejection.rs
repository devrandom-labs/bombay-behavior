//! Rejected proxy creation shared by management and shutdown scenarios.

use behavior::atomic::{ImmediateActivation, StableProxy};
use behavior::{
    CreateChild, CreationKind, CreationRejection, CreationSettlement, CreationsSettled,
    ItemSettlement, RoutedCreation, SettledItem,
};

use crate::dynamic_supervisor::{RuntimeAddress, Worker};

pub(super) fn rejected_proxy(
    created: CreateChild<RuntimeAddress, StableProxy<Worker, ImmediateActivation>>,
) -> CreationsSettled<RuntimeAddress, StableProxy<Worker, ImmediateActivation>> {
    let (creation, proxy, kind) = created.into_parts();
    let request = match kind {
        CreationKind::Birth => CreateChild::birth(creation, proxy),
        CreationKind::Replacement { previous } => {
            CreateChild::replacement(creation, previous, proxy)
        }
    };
    let routed = RoutedCreation::new(request, 239);
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Rejected {
            item: routed,
            reason: CreationRejection::EnvironmentFailed,
        })]
        .into_iter()
        .collect(),
    ))
}
