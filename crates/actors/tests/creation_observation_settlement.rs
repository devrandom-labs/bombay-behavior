use behavior_actors::{
    ActionItem, CreationCorrelation, CreationId, MailAddr, Never, ObserveCreation, Protocol,
    ReportShutdownPlan, ShutdownPlan,
};

struct Store;
struct StoreRole;

impl Protocol for Store {
    type Addr = MailAddr;
    type Msg = Never;
}

fn requires_creation_observation<Item, P, Occurrence>()
where
    P: Protocol,
    Item: ActionItem<
            Accepted = (),
            Rejection = Never,
            Prerequisite = CreationCorrelation<P, Occurrence>,
        >,
{
}

fn requires_independent_request<Item>()
where
    Item: ActionItem<Accepted = (), Rejection = Never, Prerequisite = Never>,
{
}

#[test]
fn planning_requests_use_total_item_settlement() {
    requires_creation_observation::<ObserveCreation<Store, StoreRole>, Store, StoreRole>();
    requires_independent_request::<ReportShutdownPlan<ShutdownPlan<CreationId>>>();
}
