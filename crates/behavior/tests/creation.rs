use behavior::InterpretCreations;
use core::future::{Future, poll_fn};
use core::pin::Pin;
use core::pin::pin;
use core::task::{Context, Poll, Waker};
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::sync::Arc;

use behavior::ActionItem;
use behavior::Actions;
use behavior::Behavior;
use behavior::BehaviorActed;
use behavior::Births;
use behavior::ChildCreationOutcome;
use behavior::ChildHead;
use behavior::ChildNamespaceExhausted;
use behavior::Children;
use behavior::CommittedChild;
use behavior::CreateChild;
use behavior::CreationInterpretationCustody;
use behavior::CreationKind;
use behavior::CreationRejection;
use behavior::CreationSequence;
use behavior::CreationSettlements;
use behavior::Creations;
use behavior::EndpointAddress;
use behavior::EstablishChild;
use behavior::EstablishedActor;
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
use behavior::RetirementBirths;
use behavior::RoutedCreation;
use behavior::SettledItem;
use behavior::Step;
use behavior::User;
use behavior::{InterpretationProgress, finish_item, prepare_item};
use std::collections::VecDeque;

mod installed_control;
use installed_control::InstalledControl;

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

    type Installed<B>
        = InstalledControl<B, OpaqueEndpoint>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> OpaqueEndpoint
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
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

    let outcome = ChildCreationOutcome::<Worker, ChildHead>::Established(CommittedChild::new(
        id,
        CreationKind::Birth,
        EstablishedActor::issued(InstalledControl::new(OpaqueEndpoint(41))),
    ));
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
    let remaining_creations = creations.next();
    assert!(remaining_creations.is_none());
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
    let retained_worker = creations.next().expect("the worker is retained");
    assert!(matches!(
        retained_worker.child(),
        behavior::ChildChoice::Tail(behavior::ChildChoice::Head(Worker(7)))
    ));
    let retained_backup = creations.next().expect("the backup is retained");
    assert!(matches!(
        retained_backup.child(),
        behavior::ChildChoice::Head(Backup(8))
    ));
    let remaining_creations = creations.next();
    assert!(remaining_creations.is_none());
}

#[derive(Debug, Eq, PartialEq)]
struct Ping(u8);

impl ActionItem for Ping {
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;
    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (input, received) = custody;
        match (&*input, &*received) {
            (Some(_), None) => Some((input, received)),
            _ => None,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

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
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Creations<CreateChild<OpaqueAddress, Worker>>>,
        received: &'a mut Option<
            <Creations<CreateChild<OpaqueAddress, Worker>> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Creations<CreateChild<OpaqueAddress, Worker>>: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(creations) = input.take() else {
                return;
            };
            let producer = async move {
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
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

impl InterpretItem<Ping, (), Here> for Runtime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Ping>,
        received: &'a mut Option<<Ping as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Ping: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(ping) = input.take() else {
                return;
            };
            let producer = async move {
                self.pings.push(ping.0);
                ItemSettlement::Accepted(ping.0)
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
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
                ItemSettlement::Accepted(ChildCreationOutcome::Established(CommittedChild::new(
                    id,
                    kind,
                    EstablishedActor::issued(InstalledControl::new(OpaqueEndpoint(route.0))),
                )))
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

    let Interpretation::Complete(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual creation/actions product must retain its complete settlement");
        };
        settlement
    }) else {
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

    let Interpretation::Corrupt(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual creation/actions product must retain its complete settlement");
        };
        settlement
    }) else {
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

    let Interpretation::Complete(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual creation/actions product must retain its complete settlement");
        };
        settlement
    }) else {
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

    let Interpretation::Complete(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual creation/actions product must retain its complete settlement");
        };
        settlement
    }) else {
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
    let remaining_creations = creations.next();
    assert!(remaining_creations.is_none());
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

    let Interpretation::Corrupt(settlement) = ({
        let mut progress = Some(InterpretationProgress::Original(actions));
        CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual creation/actions product must retain its complete settlement");
        };
        settlement
    }) else {
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
    let remaining_creations = creations.next();
    assert!(remaining_creations.is_none());
    assert!(runtime.pings.is_empty());
}

#[tokio::test]
async fn creation_lane_finalization_retains_the_actual_routing_reply() {
    let mut runtime = Runtime::new(RoutePlan::Available, WorkerPlan::Accept, []);
    let mut progress = Some(InterpretationProgress::Original(Creations::empty()));
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(&mut progress);
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(&mut progress);
    let Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Routing {
        input,
        received,
    })) = &mut progress
    else {
        panic!("the real route batch must be outside-owned before interpretation");
    };
    <Runtime as InterpretItem<Creations<CreateChild<OpaqueAddress, Worker>>, (), Here>>::interpret_item(&mut runtime, input, received).await;
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::finish_interpretation(&mut progress);
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(&mut progress);
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::finish_interpretation(&mut progress);
    match progress {
        Some(InterpretationProgress::Completed(Interpretation::Complete(
            behavior::CreationSettlement::Settled(settlements),
        ))) => assert!(settlements.is_empty()),
        _ => panic!("the actual complete routing reply must survive finalization and replay"),
    }
}

#[tokio::test]
async fn retirement_creation_lane_finalization_retains_the_actual_routing_reply() {
    let mut runtime = Runtime::new(RoutePlan::Available, WorkerPlan::Accept, []);
    let mut progress = Some(InterpretationProgress::Original(Creations::empty()));
    <RetirementBirths<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(
        &mut progress,
    );
    <RetirementBirths<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(
        &mut progress,
    );
    let Some(InterpretationProgress::Interpreting(base)) = &mut progress else {
        panic!("retirement policy must retain the complete underlying creation owner");
    };
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(base);
    let Some(InterpretationProgress::Interpreting(CreationInterpretationCustody::Routing {
        input,
        received,
    })) = base
    else {
        panic!("the underlying route batch must be outside-owned before interpretation");
    };
    <Runtime as InterpretItem<Creations<CreateChild<OpaqueAddress, Worker>>, (), Here>>::interpret_item(&mut runtime, input, received).await;
    <Births<Worker> as CreationSettlements<OpaqueAddress>>::finish_interpretation(base);
    <RetirementBirths<Worker> as CreationSettlements<OpaqueAddress>>::finish_interpretation(
        &mut progress,
    );
    <RetirementBirths<Worker> as CreationSettlements<OpaqueAddress>>::prepare_interpretation(
        &mut progress,
    );
    <RetirementBirths<Worker> as CreationSettlements<OpaqueAddress>>::finish_interpretation(
        &mut progress,
    );
    match progress {
        Some(InterpretationProgress::Completed(Interpretation::Complete(settlement))) => {
            match settlement.into_settlement() {
                behavior::CreationSettlement::Settled(settlements) => {
                    assert!(settlements.is_empty())
                }
                _ => panic!("retirement policy must preserve the actual routing reply"),
            }
        }
        _ => panic!("retirement creation policy must complete and survive replay"),
    }
}

#[tokio::test]
async fn two_created_children_complete_in_order_and_replay_without_recreation() {
    let mut sequence = CreationSequence::new();
    let first = sequence.issue().expect("the first creation exists");
    let second = sequence.issue().expect("the second creation exists");
    let actions = CreatingActions::new(
        InterpreterRequests::one(Ping(13)),
        Creations::one(CreateChild::birth(first, Worker(7))).and(CreateChild::replacement(
            second,
            first,
            Worker(11),
        )),
        Step::Continue,
    );
    let mut runtime = Runtime::new(
        RoutePlan::Available,
        WorkerPlan::Accept,
        [OpaqueNonce(41), OpaqueNonce(43)],
    );
    let mut progress = Some(InterpretationProgress::Original(actions));
    CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
    assert!(
        matches!(
            &progress,
            Some(InterpretationProgress::Completed(Interpretation::Complete(
                _
            )))
        ),
        "the first interpretation must complete every original child"
    );
    CreatingActions::interpret::<_, (), Here>(&mut progress, &mut runtime).await;
    let Some(InterpretationProgress::Completed(Interpretation::Complete(settlement))) = progress
    else {
        panic!("every accepted child must complete before independent sends");
    };
    let behavior::CreationSettlement::Settled(creations) = settlement.creations else {
        panic!("the complete batch must retain its exact established outcomes");
    };
    let observed = creations
        .into_iter()
        .map(|creation| {
            let SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established(child),
            )) = creation
            else {
                panic!("each real creation must remain established");
            };
            {
                let (id, kind, actor) = child.into_parts();
                (id, kind, actor.into_recipient())
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(
        observed,
        [
            (
                first,
                CreationKind::Birth,
                EstablishedRecipient::issued(OpaqueEndpoint(41))
            ),
            (
                second,
                CreationKind::replacement(first),
                EstablishedRecipient::issued(OpaqueEndpoint(43))
            )
        ]
    );
    assert_eq!(
        runtime.pings,
        [13],
        "replay must not repeat independent sends"
    );
    assert!(
        runtime.routes.is_empty(),
        "each original route is consumed once"
    );
}

enum CreationDisposal {
    Ordinary,
    Panicked(Arc<[u8]>),
}

struct CreationAttempt<Producer> {
    producer: Pin<Box<Producer>>,
    disposal: Option<CreationDisposal>,
}

impl<Producer: Future> Future for CreationAttempt<Producer> {
    type Output = Producer::Output;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().producer.as_mut().poll(context)
    }
}

impl<Producer> Drop for CreationAttempt<Producer> {
    fn drop(&mut self) {
        match self
            .disposal
            .take()
            .expect("the actual creation producer is disposed once")
        {
            CreationDisposal::Ordinary => {}
            CreationDisposal::Panicked(cause) => panic_any(cause),
        }
    }
}

struct CreationDisposalHost {
    runtime: Runtime,
    disposals: VecDeque<CreationDisposal>,
    attempts: Vec<behavior::CreationId>,
}

impl InterpretItem<Creations<CreateChild<OpaqueAddress, Worker>>, (), Here>
    for CreationDisposalHost
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Creations<CreateChild<OpaqueAddress, Worker>>>,
        received: &'a mut Option<
            <Creations<CreateChild<OpaqueAddress, Worker>> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Creations<CreateChild<OpaqueAddress, Worker>>: 'a,
    {
        <Runtime as InterpretItem<Creations<CreateChild<OpaqueAddress, Worker>>, (), Here>>::interpret_item(&mut self.runtime, input, received)
    }
}

impl EstablishChild<ChildHead, Worker> for CreationDisposalHost {
    fn establish_child(
        &mut self,
        creation: RoutedCreation<OpaqueAddress, Worker>,
    ) -> impl Future<
        Output = ItemSettlement<
            RoutedCreation<OpaqueAddress, Worker>,
            ChildCreationOutcome<Worker, ChildHead>,
            CreationRejection,
            Never,
        >,
    > + Send {
        self.attempts.push(creation.id());
        let disposal = self
            .disposals
            .pop_front()
            .expect("one producer disposition per original child");
        CreationAttempt {
            producer: Box::pin(self.runtime.establish_child(creation)),
            disposal: Some(disposal),
        }
    }
}

fn completed_child_reply_survives_producer_disposal(plan: WorkerPlan) {
    let mut sequence = CreationSequence::new();
    let first = sequence.issue().expect("the first creation exists");
    let second = sequence.issue().expect("the second creation exists");
    let first_request = RoutedCreation::new(CreateChild::birth(first, Worker(7)), OpaqueNonce(41));
    let second_request = RoutedCreation::new(
        CreateChild::replacement(second, first, Worker(11)),
        OpaqueNonce(43),
    );
    let native_cause: Arc<[u8]> = Arc::from([37, 41]);
    let expected_cause = Arc::clone(&native_cause);
    let mut host = CreationDisposalHost {
        runtime: Runtime::new(
            RoutePlan::Available,
            plan,
            [OpaqueNonce(41), OpaqueNonce(43)],
        ),
        disposals: VecDeque::from([
            CreationDisposal::Panicked(native_cause),
            CreationDisposal::Ordinary,
        ]),
        attempts: Vec::new(),
    };
    let mut progress = Some(InterpretationProgress::Original(
        Creations::one(CreateChild::birth(first, Worker(7))).and(CreateChild::replacement(
            second,
            first,
            Worker(11),
        )),
    ));
    let interruption = catch_unwind(AssertUnwindSafe(|| {
        let mut execution = pin!(<Births<Worker> as InterpretCreations<
            OpaqueAddress,
            CreationDisposalHost,
            (),
            Here,
        >>::interpret_creations(&mut progress, &mut host));
        let polled = execution
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        match polled {
            Poll::Ready(()) => panic!("the actual first producer disposal must unwind"),
            Poll::Pending => panic!("the actual creation producer supplies a completed reply"),
        }
    }));
    let cause =
        interruption.expect_err("the original producer disposal must remain a native failure");
    let cause = cause
        .downcast::<Arc<[u8]>>()
        .expect("the original native cause type must survive");
    assert!(
        Arc::ptr_eq(&cause, &expected_cause),
        "the source cause must not be reconstructed"
    );
    {
        let mut replay = pin!(<Births<Worker> as InterpretCreations<
            OpaqueAddress,
            CreationDisposalHost,
            (),
            Here,
        >>::interpret_creations(&mut progress, &mut host));
        let polled = replay
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()));
        assert!(
            matches!(polled, Poll::Ready(())),
            "the acquired prefix must not prevent complete replay"
        );
    }
    let (settlements, expected, attempts) = match (plan, progress) {
        (
            WorkerPlan::Reject,
            Some(InterpretationProgress::Completed(Interpretation::Complete(
                behavior::CreationSettlement::Settled(settlements),
            ))),
        ) => (
            settlements,
            vec![
                SettledItem::Attempted(ItemSettlement::Rejected {
                    item: first_request,
                    reason: CreationRejection::EnvironmentFailed,
                }),
                SettledItem::Attempted(ItemSettlement::Rejected {
                    item: second_request,
                    reason: CreationRejection::EnvironmentFailed,
                }),
            ],
            vec![first, second],
        ),
        (
            WorkerPlan::Corrupt,
            Some(InterpretationProgress::Completed(Interpretation::Corrupt(
                behavior::CreationSettlement::Settled(settlements),
            ))),
        ) => (
            settlements,
            vec![
                SettledItem::Attempted(ItemSettlement::Corrupt {
                    item: first_request,
                    fault: InterpreterFault::CorruptTraversal,
                }),
                SettledItem::Unattempted(second_request),
            ],
            vec![first],
        ),
        _ => panic!("the actual creation prefix and exact untouched suffix must remain complete"),
    };
    // Compare the complete typed product, including every original child, kind and route.
    let observed: Vec<
        SettledItem<
            RoutedCreation<OpaqueAddress, Worker>,
            ItemSettlement<RoutedCreation<OpaqueAddress, Worker>, Never, CreationRejection, Never>,
        >,
    > = settlements
        .into_iter()
        .map(|settlement| match settlement {
            SettledItem::Unattempted(item) => SettledItem::Unattempted(item),
            SettledItem::Attempted(ItemSettlement::Rejected { item, reason }) => {
                SettledItem::Attempted(ItemSettlement::Rejected { item, reason })
            }
            SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }) => {
                SettledItem::Attempted(ItemSettlement::Corrupt { item, fault })
            }
            SettledItem::Attempted(ItemSettlement::Accepted(_)) => {
                panic!("a refusal fixture must not acquire successful creation authority")
            }
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match prerequisite {}
            }
        })
        .collect();
    assert_eq!(observed, expected);
    assert_eq!(
        host.attempts, attempts,
        "no acquired child reply may be executed twice"
    );
}

#[test]
fn acquired_creation_rejection_survives_disposal_and_replay_without_duplicate_creation() {
    completed_child_reply_survives_producer_disposal(WorkerPlan::Reject);
}

#[test]
fn acquired_creation_corruption_survives_disposal_and_keeps_the_exact_untouched_suffix() {
    completed_child_reply_survives_producer_disposal(WorkerPlan::Corrupt);
}
