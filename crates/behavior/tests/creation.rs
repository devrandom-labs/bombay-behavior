use behavior::ActionItem;
use behavior::Actions;
use behavior::Behavior;
use behavior::BehaviorActed;
use behavior::Births;
use behavior::ChildCreationOutcome;
use behavior::ChildHead;
use behavior::ChildNamespaceExhausted;
use behavior::Children;
use behavior::CreateChild;
use behavior::CreationKind;
use behavior::CreationRejection;
use behavior::CreationSequence;
use behavior::Creations;
use behavior::EndpointAddress;
use behavior::EstablishChild;
use behavior::EstablishedCreation;
use behavior::EstablishedRecipient;
use behavior::Here;
use behavior::InterpretItem;
use behavior::Interpretation;
use behavior::InterpreterFault;
use behavior::InterpreterRequests;
use behavior::ItemSettlement;
use behavior::Never;
use behavior::NoBirths;
use behavior::NoSends;
use behavior::Protocol;
use behavior::RoutedCreation;
use behavior::SettledItem;
use behavior::Step;
use behavior::User;
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpaqueAddress;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpaqueNonce(u8);

impl behavior::Address for OpaqueAddress {
    type Nonce = OpaqueNonce;
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpaqueEndpoint(u8);

impl EndpointAddress for OpaqueAddress {
    type Established<P>
        = OpaqueEndpoint
    where
        P: Protocol<Addr = Self>;
}

struct WorkerProtocol;

impl Protocol for WorkerProtocol {
    type Addr = OpaqueAddress;
    type Msg = Never;
}

#[derive(Debug, Eq, PartialEq)]
struct Worker(u8);

impl Behavior for Worker {
    type Protocol = WorkerProtocol;
    type Event = User<OpaqueAddress, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Backup(u8);

impl Protocol for Backup {
    type Addr = OpaqueAddress;
    type Msg = Never;
}

impl Behavior for Backup {
    type Protocol = Self;
    type Event = User<OpaqueAddress, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct Parent {
    creations: CreationSequence,
}

#[test]
fn child_creation_names_the_action_and_established_outcome() {
    let mut creations = CreationSequence::new();
    let id = creations.issue().expect("the first creation ID exists");
    let request = CreateChild::<OpaqueAddress, Worker>::birth(id, Worker(7));
    assert_eq!(request.id(), id);

    let outcome = ChildCreationOutcome::<Worker, ChildHead>::Established {
        established: EstablishedCreation::installed(
            id,
            CreationKind::Birth,
            EstablishedRecipient::issued(OpaqueEndpoint(41)),
        ),
    };
    let Ok(actor) = outcome.into_actor() else {
        panic!("an established child carries the exact actor capability");
    };
    assert_eq!(
        actor.recipient(),
        EstablishedRecipient::issued(OpaqueEndpoint(41))
    );
}

#[behavior::behavior(
    addr = OpaqueAddress,
    message = Worker,
    births = { worker: Worker },
    creation_settlements = retain_for_retirement,
)]
impl Parent {
    fn init(&mut self) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn receive(&mut self, _: OpaqueAddress, worker: Worker) -> BehaviorActed<Self> {
        let Some(id) = self.creations.issue() else {
            return Ok(Actions::stop());
        };
        Ok(Actions::create(Creations::one(CreateChild::birth(
            id, worker,
        ))))
    }
}

#[test]
fn creation_uses_a_checked_id_without_an_address_nonce() {
    let mut parent = Parent {
        creations: CreationSequence::new(),
    };
    let acted = parent.receive(OpaqueAddress, Worker(7));
    let actions = acted.expect("the first creation ID is available");

    assert_eq!(actions.creates.len(), 1);
}

#[test]
fn heterogeneous_children_preserve_ids_and_declared_order() {
    let mut sequence = CreationSequence::new();
    let first = sequence.issue().expect("the first creation ID exists");
    let second = sequence.issue().expect("the second creation ID exists");
    let creations = Children::<OpaqueAddress>::new()
        .child(first, Worker(7))
        .child(second, Backup(8))
        .into_creates();
    let mut creations = creations.into_iter();

    let first_creation = creations.next().expect("the first child is retained");
    assert_eq!(first_creation.id(), first);
    let second_creation = creations.next().expect("the second child is retained");
    assert_eq!(second_creation.id(), second);
    assert!(creations.next().is_none());
}

#[test]
fn distinct_child_occurrences_may_use_their_first_local_id() {
    let mut worker_ids = CreationSequence::new();
    let mut backup_ids = CreationSequence::new();
    let worker = worker_ids.issue().expect("the worker ID exists");
    let backup = backup_ids.issue().expect("the backup ID exists");
    assert_eq!(worker, backup);

    let creations = Children::<OpaqueAddress>::new()
        .child(worker, Worker(7))
        .child(backup, Backup(8))
        .into_creates();
    let mut creations = creations.into_iter();
    assert!(matches!(
        creations.next().expect("the worker is retained").child(),
        behavior::ChildChoice::Tail(behavior::ChildChoice::Head(Worker(7)))
    ));
    assert!(matches!(
        creations.next().expect("the backup is retained").child(),
        behavior::ChildChoice::Head(Backup(8))
    ));
    assert!(creations.next().is_none());
}

#[derive(Debug, Eq, PartialEq)]
struct Ping(u8);

impl ActionItem for Ping {
    type Accepted = u8;
    type Rejection = Never;
    type Prerequisite = Never;
}

#[derive(Clone, Copy)]
enum RoutePlan {
    Available,
    Exhausted,
    Corrupt,
}

#[derive(Clone, Copy)]
enum WorkerPlan {
    Accept,
    Reject,
    Corrupt,
}

struct Runtime {
    route_plan: RoutePlan,
    worker_plan: WorkerPlan,
    routes: VecDeque<OpaqueNonce>,
    pings: Vec<u8>,
}

impl Runtime {
    fn new(
        route_plan: RoutePlan,
        worker_plan: WorkerPlan,
        routes: impl IntoIterator<Item = OpaqueNonce>,
    ) -> Self {
        Self {
            route_plan,
            worker_plan,
            routes: routes.into_iter().collect(),
            pings: Vec::new(),
        }
    }
}

impl InterpretItem<Creations<CreateChild<OpaqueAddress, Worker>>, (), Here> for Runtime {
    async fn interpret_item(
        &mut self,
        creations: Creations<CreateChild<OpaqueAddress, Worker>>,
    ) -> ItemSettlement<
        Creations<CreateChild<OpaqueAddress, Worker>>,
        Creations<RoutedCreation<OpaqueAddress, Worker>>,
        ChildNamespaceExhausted,
        Never,
    > {
        match self.route_plan {
            RoutePlan::Exhausted => ItemSettlement::Rejected {
                item: creations,
                reason: ChildNamespaceExhausted,
            },
            RoutePlan::Corrupt => ItemSettlement::Corrupt {
                item: creations,
                fault: InterpreterFault::CorruptTraversal,
            },
            RoutePlan::Available => {
                let Some(_) = self.routes.len().checked_sub(creations.len()) else {
                    return ItemSettlement::Rejected {
                        item: creations,
                        reason: ChildNamespaceExhausted,
                    };
                };
                ItemSettlement::Accepted(creations.map(|creation| {
                    let route = self
                        .routes
                        .pop_front()
                        .expect("preflight proved one route per creation");
                    RoutedCreation::new(creation, route)
                }))
            }
        }
    }
}

impl InterpretItem<Ping, (), Here> for Runtime {
    async fn interpret_item(&mut self, ping: Ping) -> ItemSettlement<Ping, u8, Never, Never> {
        self.pings.push(ping.0);
        ItemSettlement::Accepted(ping.0)
    }
}

impl EstablishChild<ChildHead, Worker> for Runtime {
    async fn establish_child(
        &mut self,
        creation: RoutedCreation<OpaqueAddress, Worker>,
    ) -> ItemSettlement<
        RoutedCreation<OpaqueAddress, Worker>,
        ChildCreationOutcome<Worker, ChildHead>,
        CreationRejection,
        Never,
    > {
        match self.worker_plan {
            WorkerPlan::Reject => ItemSettlement::Rejected {
                item: creation,
                reason: CreationRejection::EnvironmentFailed,
            },
            WorkerPlan::Corrupt => ItemSettlement::Corrupt {
                item: creation,
                fault: InterpreterFault::CorruptTraversal,
            },
            WorkerPlan::Accept => {
                let id = creation.id();
                let kind = creation.kind();
                let route = creation.route();
                let (creation, _) = creation.into_parts();
                let (_, _worker, _) = creation.into_parts();
                ItemSettlement::Accepted(ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        id,
                        kind,
                        EstablishedRecipient::issued(OpaqueEndpoint(route.0)),
                    ),
                })
            }
        }
    }
}

type CreatingActions = Actions<OpaqueAddress, Never, InterpreterRequests<Ping>, Births<Worker>>;

#[tokio::test]
async fn route_rejection_returns_the_batch_and_continues_independent_sends() {
    let mut sequence = CreationSequence::new();
    let id = sequence.issue().expect("the first creation ID exists");
    let actions = CreatingActions::new(
        InterpreterRequests::one(Ping(9)),
        Creations::one(CreateChild::birth(id, Worker(7))),
        Step::Continue,
    );
    let mut runtime = Runtime::new(
        RoutePlan::Exhausted,
        WorkerPlan::Accept,
        [OpaqueNonce(41), OpaqueNonce(43)],
    );

    let Interpretation::Complete(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
        panic!("namespace exhaustion is an expected rejection");
    };

    let behavior::CreationSettlement::Rejected { creations, reason } = settlement.creations else {
        panic!("the complete creation batch must be returned");
    };
    assert_eq!(creations, Creations::one(CreateChild::birth(id, Worker(7))));
    assert_eq!(reason, ChildNamespaceExhausted);
    assert_eq!(runtime.pings, [9]);
}

#[tokio::test]
async fn route_corruption_returns_the_unrouted_batch_and_skips_sends() {
    let mut sequence = CreationSequence::new();
    let id = sequence.issue().expect("the first creation ID exists");
    let actions = CreatingActions::new(
        InterpreterRequests::one(Ping(9)),
        Creations::one(CreateChild::birth(id, Worker(7))),
        Step::Continue,
    );
    let mut runtime = Runtime::new(
        RoutePlan::Corrupt,
        WorkerPlan::Accept,
        [OpaqueNonce(41), OpaqueNonce(43)],
    );

    let Interpretation::Corrupt(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
        panic!("route corruption must stop interpretation");
    };
    let behavior::CreationSettlement::Corrupt { creations, fault } = settlement.creations else {
        panic!("the unrouted batch must remain exact");
    };
    assert_eq!(creations, Creations::one(CreateChild::birth(id, Worker(7))));
    assert_eq!(fault, InterpreterFault::CorruptTraversal);
    assert!(runtime.pings.is_empty());
}

#[tokio::test]
async fn route_preflight_consumes_nothing_when_the_whole_batch_cannot_route() {
    let mut sequence = CreationSequence::new();
    let first = sequence.issue().expect("the first creation ID exists");
    let second = sequence.issue().expect("the second creation ID exists");
    let actions = CreatingActions::new(
        InterpreterRequests::new(Vec::new()),
        Creations::one(CreateChild::birth(first, Worker(7)))
            .and(CreateChild::birth(second, Worker(8))),
        Step::Continue,
    );
    let mut runtime = Runtime::new(RoutePlan::Available, WorkerPlan::Accept, [OpaqueNonce(41)]);

    let Interpretation::Complete(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
        panic!("namespace exhaustion is an expected rejection");
    };
    let behavior::CreationSettlement::Rejected { creations, .. } = settlement.creations else {
        panic!("the complete batch must be returned");
    };
    assert_eq!(creations.len(), 2);
    assert_eq!(runtime.routes, [OpaqueNonce(41)]);
}

#[tokio::test]
async fn routed_rejection_returns_the_id_child_kind_and_route() {
    let mut sequence = CreationSequence::new();
    let id = sequence.issue().expect("the first creation ID exists");
    let actions = CreatingActions::new(
        InterpreterRequests::new(Vec::new()),
        Creations::one(CreateChild::birth(id, Worker(7))),
        Step::Continue,
    );
    let mut runtime = Runtime::new(
        RoutePlan::Available,
        WorkerPlan::Reject,
        [OpaqueNonce(41), OpaqueNonce(43)],
    );

    let Interpretation::Complete(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
        panic!("child rejection is an expected settlement");
    };
    let behavior::CreationSettlement::Settled(creations) = settlement.creations else {
        panic!("route preparation must have committed");
    };
    let mut creations = creations.into_iter();
    let Some(SettledItem::Attempted(ItemSettlement::Rejected { item, reason })) = creations.next()
    else {
        panic!("the routed child must be returned");
    };
    assert_eq!(item.id(), id);
    assert_eq!(item.kind(), CreationKind::Birth);
    assert_eq!(item.route(), OpaqueNonce(41));
    let (creation, _) = item.into_parts();
    let (_, worker, _) = creation.into_parts();
    assert_eq!(worker, Worker(7));
    assert_eq!(reason, CreationRejection::EnvironmentFailed);
    assert!(creations.next().is_none());
}

#[tokio::test]
async fn corrupt_child_retains_the_exact_routed_suffix_and_skips_sends() {
    let mut sequence = CreationSequence::new();
    let first = sequence.issue().expect("the first creation ID exists");
    let second = sequence.issue().expect("the second creation ID exists");
    let actions = CreatingActions::new(
        InterpreterRequests::one(Ping(9)),
        Creations::one(CreateChild::birth(first, Worker(7))).and(CreateChild::replacement(
            second,
            first,
            Worker(8),
        )),
        Step::Continue,
    );
    let mut runtime = Runtime::new(
        RoutePlan::Available,
        WorkerPlan::Corrupt,
        [OpaqueNonce(41), OpaqueNonce(43)],
    );

    let Interpretation::Corrupt(settlement) = actions.interpret::<_, (), Here>(&mut runtime).await
    else {
        panic!("child-host corruption must stop interpretation");
    };
    let behavior::CreationSettlement::Settled(creations) = settlement.creations else {
        panic!("the routed product must remain exact");
    };
    let mut creations = creations.into_iter();
    let Some(SettledItem::Attempted(ItemSettlement::Corrupt { item, fault })) = creations.next()
    else {
        panic!("the first routed creation must retain corruption");
    };
    assert_eq!(item.id(), first);
    assert_eq!(item.route(), OpaqueNonce(41));
    assert_eq!(fault, InterpreterFault::CorruptTraversal);
    let Some(SettledItem::Unattempted(item)) = creations.next() else {
        panic!("the second routed creation must remain untouched");
    };
    assert_eq!(item.id(), second);
    assert_eq!(item.route(), OpaqueNonce(43));
    assert!(creations.next().is_none());
    assert!(runtime.pings.is_empty());
}
