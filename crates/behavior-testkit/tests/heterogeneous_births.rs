//! Independent static-dispatch checks for heterogeneous child creation.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::marker::PhantomData;

use behavior_core::{
    Actions, Address, AllocationRejection, Behavior, BehaviorActed, Births, ChildChoice,
    ChildCreationOutcome, ChildCreationProduct, ChildHead, CreateChild, CreationId, CreationKind,
    CreationRejection, CreationSequence, Creations, DispatchBirth, EndpointAddress, EstablishChild,
    EstablishedCreation, EstablishedRecipient, InterpreterFault, ItemSettlement, Never, NoBirths,
    Protocol, RoutedCreation, User,
};
use proptest::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ModelAddr(u64);

impl Address for ModelAddr {
    type Nonce = u64;
}

#[derive(Debug, Eq, PartialEq)]
struct ModelEndpoint<P>(u64, PhantomData<fn() -> P>);

impl<P> Clone for ModelEndpoint<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P> Copy for ModelEndpoint<P> {}

impl EndpointAddress for ModelAddr {
    type Established<P>
        = ModelEndpoint<P>
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, PartialEq, Eq)]
struct DeviceGroups;

impl Protocol for DeviceGroups {
    type Addr = ModelAddr;
    type Msg = u8;
}

impl Behavior for DeviceGroups {
    type Protocol = Self;
    type Event = User<ModelAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Queries;

impl Protocol for Queries {
    type Addr = ModelAddr;
    type Msg = &'static str;
}

impl Behavior for Queries {
    type Protocol = Self;
    type Event = User<ModelAddr, &'static str>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

type IoTChildren = ChildChoice<DeviceGroups, ChildChoice<Queries, Never>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildKind {
    DeviceGroups,
    Queries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservedCreation {
    Established,
    Rejected(CreationRejection),
    Corrupt(InterpreterFault),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostPlan {
    Commit,
    RejectAfterInitialization,
}

enum HostDecision {
    CommitAt(u64),
    RejectAfterInitialization,
}

struct ModelHost {
    claimed_routes: BTreeMap<u64, ()>,
    plans: BTreeMap<u64, HostPlan>,
    trace: Vec<(CreationId, u64, ChildKind, CreationKind)>,
    next_address: u64,
}

impl ModelHost {
    fn admit(
        &mut self,
        id: CreationId,
        route: u64,
        child: ChildKind,
        kind: CreationKind,
    ) -> Result<HostDecision, CreationRejection> {
        match self.claimed_routes.entry(route) {
            Entry::Occupied(_) => Err(CreationRejection::Allocation(
                AllocationRejection::AddressAlreadyClaimed,
            )),
            Entry::Vacant(entry) => match self.plans.remove(&route).unwrap_or(HostPlan::Commit) {
                HostPlan::Commit => {
                    entry.insert(());
                    self.trace.push((id, route, child, kind));
                    let address = self.next_address;
                    self.next_address += 1;
                    Ok(HostDecision::CommitAt(address))
                }
                HostPlan::RejectAfterInitialization => Ok(HostDecision::RejectAfterInitialization),
            },
        }
    }
}

impl EstablishChild<behavior_core::ChildHead, DeviceGroups> for ModelHost {
    async fn establish_child(
        &mut self,
        creation: RoutedCreation<ModelAddr, DeviceGroups>,
    ) -> ItemSettlement<
        RoutedCreation<ModelAddr, DeviceGroups>,
        ChildCreationOutcome<DeviceGroups, behavior_core::ChildHead>,
        CreationRejection,
        Never,
    > {
        let decision = self.admit(
            creation.id(),
            creation.route(),
            ChildKind::DeviceGroups,
            creation.kind(),
        );
        settle_child(creation, decision)
    }
}

fn settle_child<C, Occurrence>(
    creation: RoutedCreation<ModelAddr, C>,
    decision: Result<HostDecision, CreationRejection>,
) -> ItemSettlement<
    RoutedCreation<ModelAddr, C>,
    ChildCreationOutcome<C, Occurrence>,
    CreationRejection,
    Never,
>
where
    C: Behavior,
    C::Protocol: Protocol<Addr = ModelAddr>,
{
    match decision {
        Err(reason) => ItemSettlement::Rejected {
            item: creation,
            reason,
        },
        Ok(HostDecision::CommitAt(address)) => {
            let id = creation.id();
            let kind = creation.kind();
            let (request, _) = creation.into_parts();
            let (_, _child, _) = request.into_parts();
            ItemSettlement::Accepted(ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    id,
                    kind,
                    EstablishedRecipient::issued(ModelEndpoint(address, PhantomData)),
                ),
            })
        }
        Ok(HostDecision::RejectAfterInitialization) => {
            ItemSettlement::Accepted(ChildCreationOutcome::HostRejected {
                creation,
                initialization: Actions::cont(),
                reason: CreationRejection::EnvironmentFailed,
            })
        }
    }
}

impl EstablishChild<behavior_core::ChildTail<behavior_core::ChildHead>, Queries> for ModelHost {
    async fn establish_child(
        &mut self,
        creation: RoutedCreation<ModelAddr, Queries>,
    ) -> ItemSettlement<
        RoutedCreation<ModelAddr, Queries>,
        ChildCreationOutcome<Queries, behavior_core::ChildTail<behavior_core::ChildHead>>,
        CreationRejection,
        Never,
    > {
        let decision = self.admit(
            creation.id(),
            creation.route(),
            ChildKind::Queries,
            creation.kind(),
        );
        settle_child(creation, decision)
    }
}

fn host() -> ModelHost {
    ModelHost {
        claimed_routes: BTreeMap::new(),
        plans: BTreeMap::new(),
        trace: Vec::new(),
        next_address: 1_000,
    }
}

async fn dispatch(
    child: IoTChildren,
    id: CreationId,
    route: u64,
    kind: CreationKind,
    host: &mut ModelHost,
) -> ItemSettlement<
    RoutedCreation<ModelAddr, IoTChildren>,
    <IoTChildren as ChildCreationProduct<ModelAddr, ChildHead>>::Result,
    CreationRejection,
    Never,
> {
    <IoTChildren as DispatchBirth<ModelAddr, ModelHost>>::dispatch_birth(
        child, id, route, kind, host,
    )
    .await
}

fn normalize(
    settlement: ItemSettlement<
        RoutedCreation<ModelAddr, IoTChildren>,
        <IoTChildren as ChildCreationProduct<ModelAddr, ChildHead>>::Result,
        CreationRejection,
        Never,
    >,
) -> ObservedCreation {
    match settlement {
        ItemSettlement::Accepted(ChildChoice::Head(ChildCreationOutcome::Established {
            ..
        }))
        | ItemSettlement::Accepted(ChildChoice::Tail(ChildChoice::Head(
            ChildCreationOutcome::Established { .. },
        ))) => ObservedCreation::Established,
        ItemSettlement::Accepted(ChildChoice::Head(ChildCreationOutcome::HostRejected {
            reason,
            ..
        }))
        | ItemSettlement::Accepted(ChildChoice::Tail(ChildChoice::Head(
            ChildCreationOutcome::HostRejected { reason, .. },
        ))) => ObservedCreation::Rejected(reason),
        ItemSettlement::Accepted(ChildChoice::Head(
            ChildCreationOutcome::InitializationRejected { error, .. },
        ))
        | ItemSettlement::Accepted(ChildChoice::Tail(ChildChoice::Head(
            ChildCreationOutcome::InitializationRejected { error, .. },
        ))) => match error {},
        ItemSettlement::Accepted(ChildChoice::Tail(ChildChoice::Tail(never))) => match never {},
        ItemSettlement::Rejected { reason, .. } => ObservedCreation::Rejected(reason),
        ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
        ItemSettlement::Corrupt { fault, .. } => ObservedCreation::Corrupt(fault),
    }
}

fn assert_send<T: Send>(_: &T) {}

#[tokio::test]
async fn ordered_creations_dispatch_to_their_concrete_child_hosts() {
    let mut sequence = CreationSequence::new();
    let devices = sequence.issue().expect("device child ID exists");
    let previous = sequence.issue().expect("previous query child ID exists");
    let replacement = sequence.issue().expect("replacement query child ID exists");
    let query = sequence.issue().expect("query child ID exists");
    let actions: Actions<ModelAddr, Never, Vec<Never>, Births<IoTChildren>> = Actions::create(
        Creations::one(CreateChild::birth(devices, ChildChoice::Head(DeviceGroups)))
            .and(CreateChild::replacement(
                replacement,
                previous,
                ChildChoice::Tail(ChildChoice::Head(Queries)),
            ))
            .and(CreateChild::birth(
                query,
                ChildChoice::Tail(ChildChoice::Head(Queries)),
            )),
    );
    let mut model = host();
    for (creation, route) in actions.creates.into_iter().zip([9, 4, 7]) {
        let (id, child, kind) = creation.into_parts();
        assert!(matches!(
            dispatch(child, id, route, kind, &mut model).await,
            ItemSettlement::Accepted(_)
        ));
    }

    assert_eq!(
        model.trace,
        [
            (devices, 9, ChildKind::DeviceGroups, CreationKind::Birth),
            (
                replacement,
                4,
                ChildKind::Queries,
                CreationKind::replacement(previous),
            ),
            (query, 7, ChildKind::Queries, CreationKind::Birth),
        ]
    );
}

#[tokio::test]
async fn host_rejection_returns_the_routed_child_and_initialization_actions() {
    let mut sequence = CreationSequence::new();
    let id = sequence.issue().expect("child ID exists");
    let mut model = host();
    assert_eq!(
        model.plans.insert(9, HostPlan::RejectAfterInitialization),
        None
    );

    let settlement = dispatch(
        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Head(DeviceGroups),
        id,
        9,
        CreationKind::Birth,
        &mut model,
    )
    .await;

    let ItemSettlement::Accepted(ChildChoice::Head(ChildCreationOutcome::HostRejected {
        creation,
        initialization,
        reason,
    })) = settlement
    else {
        panic!("expected exact child-host rejection ownership");
    };
    assert_eq!(
        creation,
        RoutedCreation::new(CreateChild::birth(id, DeviceGroups), 9)
    );
    assert!(initialization.sends.is_empty());
    assert!(initialization.creates.is_empty());
    assert_eq!(initialization.become_, behavior_core::Step::Continue);
    assert_eq!(reason, CreationRejection::EnvironmentFailed);
    assert!(model.claimed_routes.is_empty());
}

#[tokio::test]
async fn claimed_address_is_global_across_child_alternatives() {
    let mut sequence = CreationSequence::new();
    let first_id = sequence.issue().expect("first child ID exists");
    let second_id = sequence.issue().expect("second child ID exists");
    let mut model = host();
    let first = dispatch(
        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Head(DeviceGroups),
        first_id,
        5,
        CreationKind::Birth,
        &mut model,
    );
    assert_send(&first);
    assert!(matches!(first.await, ItemSettlement::Accepted(_)));
    let collision = dispatch(
        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Tail(ChildChoice::Head(Queries)),
        second_id,
        5,
        CreationKind::Birth,
        &mut model,
    );
    assert_send(&collision);
    let ItemSettlement::Rejected { item, reason } = collision.await else {
        panic!("expected exact claimed-address rejection");
    };
    assert_eq!(
        item,
        RoutedCreation::new(
            CreateChild::birth(second_id, ChildChoice::Tail(ChildChoice::Head(Queries))),
            5
        )
    );
    assert_eq!(
        reason,
        CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed)
    );
    assert_eq!(
        model.trace,
        [(first_id, 5, ChildKind::DeviceGroups, CreationKind::Birth)]
    );
}

proptest! {
    #[test]
    fn arbitrary_child_routes_match_one_global_address_model(
        inputs in proptest::collection::vec((
            prop_oneof![Just(ChildKind::DeviceGroups), Just(ChildKind::Queries)],
            0_u8..12,
        ), 0..80)
    ) {
        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let mut ids = CreationSequence::new();
        let mut host = host();
        let mut claimed = BTreeMap::new();
        let mut expected = Vec::new();
        for (child, raw_route) in inputs {
            let id = ids.issue().expect("generated sequence does not exhaust child IDs");
            let route = u64::from(raw_route);
            let expected_result = match claimed.entry(route) {
                Entry::Vacant(entry) => {
                    entry.insert(());
                    expected.push((id, route, child, CreationKind::Birth));
                    ObservedCreation::Established
                }
                Entry::Occupied(_) => ObservedCreation::Rejected(
                    CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed),
                ),
            };
            let actual = runtime.block_on(async {
                match child {
                    ChildKind::DeviceGroups => dispatch(
                        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Head(DeviceGroups),
                        id,
                        route,
                        CreationKind::Birth,
                        &mut host,
                    ).await,
                    ChildKind::Queries => dispatch(
                        ChildChoice::<DeviceGroups, ChildChoice<Queries, Never>>::Tail(
                            ChildChoice::Head(Queries),
                        ),
                        id,
                        route,
                        CreationKind::Birth,
                        &mut host,
                    ).await,
                }
            });
            prop_assert_eq!(normalize(actual), expected_result);
            prop_assert_eq!(&host.trace, &expected);
        }
    }
}
