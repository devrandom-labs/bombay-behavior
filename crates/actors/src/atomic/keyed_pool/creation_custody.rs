use super::super::{KeyedEvent, KeyedPool, KeyedPoolState, keyed};
use super::{KeyedDiagnosticCause, KeyedOutcome, KeyedQueuedReturnReason};
use crate::atomic::pool::worker::{MemberState, RetirementStatus};
use crate::atomic::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, Assignment, BacklogCapacity,
    BindingCapacity, BindingEvidence, CustomerDelivery, Interruption, JobId, OrderedRoles,
    PoolFailureReaction, PoolRecovery, SubmissionId, WorkerCreationRejection, WorkerSubmission,
};
use crate::{
    Activate, ChildStopped, Crash, DiagnosticAction, DiagnosticDisposition, ShutdownRequested,
    StopOnShutdown,
};
use behavior::{
    Actions, Address, Behavior, ChildCreationOutcome, ChildHead, CreateChild, CreationKind,
    CreationSequence, CreationSettlement, Creations, CreationsSettled, EndpointAddress,
    ItemSettlement, MessageProtocol, Never, Protocol, Recipient, RoutedCreation, SettledItem, Step,
};
use core::convert::Infallible;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SearchAddress(u64);
impl Address for SearchAddress {
    type Nonce = u64;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SearchEndpoint(u64);
impl EndpointAddress for SearchAddress {
    type Established<P>
        = SearchEndpoint
    where
        P: Protocol<Addr = Self>;
    type Installed<B>
        = SearchEndpoint
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;
    fn recipient<B>(installed: &Self::Installed<B>) -> Self::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        *installed
    }
}

#[derive(Debug, Eq, PartialEq)]
enum SearchRole {
    Primary,
    Replica,
}
#[derive(Clone, Debug)]
struct SearchJob(Box<[u8]>);
struct SearchWorker {
    definition: Box<[u8]>,
}
#[crate::atomic::pool_worker(addr = SearchAddress, result = usize)]
impl SearchWorker {
    fn transition(&mut self, assignment: Assignment<SearchJob>) -> WorkerActed<Self> {
        let result = assignment.payload().0.len();
        Ok(Actions::cont().with_send(assignment.complete(result)))
    }
}
struct SearchEndpointTransfer;
impl behavior::InterpretEstablished<<SearchWorker as Behavior>::Protocol>
    for SearchEndpointTransfer
{
    type Output = SearchEndpoint;
    fn interpret_established(&mut self, endpoint: SearchEndpoint) -> Self::Output {
        endpoint
    }
}
struct SearchActivation {
    authorization: Box<[u16]>,
}
impl ActivationPlan for SearchActivation {
    type Ready = ();
    type Rejection = Never;
    fn activate(self) -> impl core::future::Future<Output = Result<(), Never>> + Send {
        async move {
            drop(self);
            Ok(())
        }
    }
}
fn prepare_search(
    role: &SearchRole,
) -> Result<WorkerSubmission<SearchWorker, SearchActivation>, Never> {
    let (definition, authorization): (&[u8], &[u16]) = match role {
        SearchRole::Primary => (&[31, 37], &[47, 53]),
        SearchRole::Replica => (&[41, 43], &[59, 61]),
    };
    Ok(WorkerSubmission::activated(
        SearchWorker {
            definition: definition.into(),
        },
        SearchActivation {
            authorization: authorization.into(),
        },
    ))
}
fn role_for_key(key: &u64) -> SearchRole {
    match key % 2 {
        0 => SearchRole::Primary,
        _ => SearchRole::Replica,
    }
}
type SearchPool = KeyedPool<
    SearchRole,
    SearchWorker,
    SearchActivation,
    Never,
    fn(&u64) -> SearchRole,
    Infallible,
    u64,
    SearchJob,
    usize,
>;
type SearchActions =
    Actions<SearchAddress, Never, <SearchPool as Behavior>::Sends, <SearchPool as Behavior>::Birth>;
fn search_pool(policy: PoolFailureReaction) -> SearchPool {
    search_pool_with_roles(
        policy,
        OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica]).expect("two real roles"),
    )
}
fn search_pool_with_roles(
    policy: PoolFailureReaction,
    roles: OrderedRoles<SearchRole>,
) -> SearchPool {
    keyed(
        prepare_search,
        roles,
        role_for_key as fn(&u64) -> SearchRole,
        ActivationPolicy::new(2).expect("two genuine activation slots"),
        PoolRecovery::temporary(policy),
        BacklogCapacity::new(2),
        BindingCapacity::new(2).expect("both accepted keys have binding capacity"),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("two prepared search roles"))
}
fn panic_creation(
    creation: CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>,
    route: u64,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    CreationsSettled::new(CreationSettlement::Settled(Creations::one(
        SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::<
            _,
            ChildHead,
        >::InitializationPanicked {
            creation: RoutedCreation::new(creation, route),
        })),
    )))
}
fn established_creation(
    creation: CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    let (id, child, kind) = creation.into_parts();
    drop(child);
    CreationsSettled::new(CreationSettlement::Settled(Creations::one(
        SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::<
            StopOnShutdown<SearchWorker>,
            ChildHead,
        >::Established(
            behavior::CommittedChild::new(
                id,
                kind,
                behavior::EstablishedActor::issued(SearchEndpoint(83)),
            ),
        ))),
    )))
}
fn admitted(
    actions: SearchActions,
    address: SearchAddress,
    submission: SubmissionId,
) -> (JobId, BindingEvidence<SearchRole>) {
    assert!(actions.creates.is_empty());
    assert!(actions.sends.worker_assignments.is_empty());
    assert!(actions.sends.diagnostics.is_empty());
    let mut deliveries = actions.sends.customer_outcomes.into_requests();
    assert_eq!(deliveries.len(), 1);
    let CustomerDelivery::Logical { delivery } = deliveries.pop().expect("one accepted job") else {
        panic!("the actual logical customer route survives");
    };
    assert_eq!(delivery.to.address(), address);
    let KeyedOutcome::Accepted {
        submission: accepted,
        job,
        binding,
    } = delivery.message
    else {
        panic!("both jobs are genuinely admitted while their workers are Creating");
    };
    assert_eq!(accepted, submission);
    (job, binding)
}
fn assert_panic_input(
    input: CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
    id: behavior::CreationId,
    kind: CreationKind,
    route: u64,
    allocation: *const u8,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    let CreationSettlement::Settled(creations) = input.into_settlement() else {
        panic!("the whole original settled creation batch survives");
    };
    assert_eq!(creations.len(), 1);
    let item = creations.into_iter().next().expect("one original creation");
    let SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::InitializationPanicked { creation },
    )) = item
    else {
        panic!("the exact original initialization panic receipt survives");
    };
    assert_eq!(creation.id(), id);
    assert_eq!(creation.kind(), kind);
    assert_eq!(creation.route(), route);
    let (request, route) = creation.into_parts();
    let (id, child, kind) = request.into_parts();
    let worker = child.into_inner();
    assert_eq!(worker.definition.as_ptr(), allocation);
    assert_eq!(&*worker.definition, &[31, 37]);
    panic_creation(
        match kind {
            CreationKind::Birth => CreateChild::birth(id, StopOnShutdown::new(worker)),
            CreationKind::Replacement { previous } => {
                CreateChild::replacement(id, previous, StopOnShutdown::new(worker))
            }
        },
        route,
    )
}

#[test]
fn matched_creation_failure_returns_only_primary_queue_and_preserves_replica() {
    let initialized = search_pool(PoolFailureReaction::RetireRole)
        .initialize()
        .expect("ordinary keyed initialization");
    let mut pool = initialized.behavior;
    let mut creations = initialized.actions.creates.into_iter();
    let primary = creations.next().expect("primary request");
    let replica = creations.next().expect("replica request");
    assert!(creations.next().is_none());
    let primary_id = primary.id();
    let replica_id = replica.id();
    let (id, child, kind) = primary.into_parts();
    let definition = child.into_inner();
    let definition_allocation = definition.definition.as_ptr();
    let primary = CreateChild::birth(id, StopOnShutdown::new(definition));
    let (activation_allocation, replica_activation_allocation) = match &pool.state {
        KeyedPoolState::Operating(operating) => {
            let MemberState::Creating(primary) = &operating.roles[0].member.state else {
                panic!("primary Creating");
            };
            let MemberState::Creating(replica) = &operating.roles[1].member.state else {
                panic!("replica Creating");
            };
            (
                primary.activation.authorization.as_ptr(),
                replica.activation.authorization.as_ptr(),
            )
        }
        _ => panic!("ordinary public initialization enters Operating"),
    };
    let primary_payload: Box<[u8]> = [67, 71].into();
    let primary_allocation = primary_payload.as_ptr();
    let replica_payload: Box<[u8]> = [73, 79].into();
    let replica_allocation = replica_payload.as_ptr();
    let (primary_job, primary_binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(11),
                2,
                SearchJob(primary_payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(90)),
            ),
        )
        .expect("primary admission"),
        SearchAddress(90),
        SubmissionId::new(11),
    );
    let (replica_job, replica_binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(13),
                3,
                SearchJob(replica_payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(91)),
            ),
        )
        .expect("replica admission"),
        SearchAddress(91),
        SubmissionId::new(13),
    );
    let acted = pool
        .on(panic_creation(primary, 97))
        .expect("matched existing panic receipt");
    assert_eq!(
        acted.sends.customer_outcomes.len(),
        1,
        "RetireRole must return the primary's already accepted queue"
    );
    let mut outcomes = acted.sends.customer_outcomes.into_requests();
    let CustomerDelivery::Logical { delivery } = outcomes.pop().expect("primary terminal job")
    else {
        panic!("complete original primary customer");
    };
    assert_eq!(delivery.to.address(), SearchAddress(90));
    let KeyedOutcome::ReturnedQueued {
        job,
        binding,
        payload,
        reason,
    } = delivery.message
    else {
        panic!("primary accepted job is returned exactly");
    };
    assert_eq!(job, primary_job);
    assert_eq!(binding.generation(), primary_binding.generation());
    assert_eq!(binding.role(), primary_binding.role());
    assert_eq!(payload.0.as_ptr(), primary_allocation);
    assert_eq!(&*payload.0, &[67, 71]);
    assert_eq!(reason, KeyedQueuedReturnReason::RolePermanentlyUnavailable);
    assert!(acted.creates.is_empty());
    assert!(matches!(acted.become_, Step::Continue));
    assert!(acted.sends.worker_observations.is_empty());
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_assignments.is_empty());
    assert!(acted.sends.worker_preparations.is_empty());
    assert!(acted.sends.restart_schedules.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    assert!(acted.sends.binding_replies.as_slice().is_empty());
    let mut diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 2);
    let DiagnosticAction::Terminal { diagnostic } = diagnostics.remove(0);
    let KeyedDiagnosticCause::WorkerCreationRejected {
        role,
        rejection,
        activation,
        stopped,
    } = diagnostic.cause
    else {
        panic!("the expected role owns its complete failure and activation");
    };
    assert_eq!(role.role(), &SearchRole::Primary);
    let WorkerCreationRejection::WorkerPanicked { worker } = rejection else {
        panic!("original failed definition");
    };
    assert_eq!(worker.definition.as_ptr(), definition_allocation);
    assert_eq!(&*worker.definition, &[31, 37]);
    assert_eq!(activation.authorization.as_ptr(), activation_allocation);
    assert_eq!(&*activation.authorization, &[47, 53]);
    assert!(stopped.is_none());
    let DiagnosticAction::Terminal { diagnostic } = diagnostics.remove(0);
    let KeyedDiagnosticCause::BindingRemoved { key, binding } = diagnostic.cause else {
        panic!("primary binding is discharged once");
    };
    assert_eq!(key, 2);
    assert_eq!(binding.generation(), primary_binding.generation());
    assert_eq!(binding.role(), primary_binding.role());
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("replica remains operational");
    };
    assert!(matches!(
        operating.roles[0].member.state,
        MemberState::Retired
    ));
    assert!(operating.roles[0].queue.is_empty());
    assert!(operating.bindings.binding(&2).is_none());
    assert_eq!(
        operating
            .bindings
            .binding(&3)
            .expect("replica binding")
            .generation(),
        replica_binding.generation()
    );
    assert_eq!(
        operating
            .bindings
            .binding(&3)
            .expect("replica binding")
            .role(),
        replica_binding.role()
    );
    let replica_queued = operating.roles[1]
        .queue
        .values()
        .next()
        .expect("untouched replica queue");
    assert_eq!(replica_queued.id, replica_job);
    assert_eq!(replica_queued.payload.0.as_ptr(), replica_allocation);
    let MemberState::Creating(replica_pending) = &operating.roles[1].member.state else {
        panic!("original replica startup remains");
    };
    assert_eq!(replica_pending.attempt.creation(), replica_id);
    assert_eq!(
        replica_pending.activation.authorization.as_ptr(),
        replica_activation_allocation
    );
    let replica_created = pool
        .on(established_creation(replica))
        .expect("replica independently establishes");
    assert_eq!(replica_created.sends.worker_initializations.len(), 1);
    assert!(replica_created.sends.customer_outcomes.is_empty());
    assert!(replica_created.sends.diagnostics.is_empty());
    assert_ne!(primary_id, replica_id);
    assert_eq!(kind, CreationKind::Birth);
}

#[test]
fn matched_creation_failure_stops_pool_and_returns_both_queues() {
    let initialized = search_pool(PoolFailureReaction::StopPool)
        .initialize()
        .expect("ordinary keyed initialization");
    let mut pool = initialized.behavior;
    let mut creations = initialized.actions.creates.into_iter();
    let primary = creations.next().expect("primary creation");
    let replica = creations.next().expect("replica creation");
    assert!(creations.next().is_none());
    let replica_id = replica.id();
    let (id, child, _) = primary.into_parts();
    let worker = child.into_inner();
    let definition_allocation = worker.definition.as_ptr();
    let primary = CreateChild::birth(id, StopOnShutdown::new(worker));
    let (activation_allocation, replica_activation_allocation) = match &pool.state {
        KeyedPoolState::Operating(operating) => {
            let MemberState::Creating(primary) = &operating.roles[0].member.state else {
                panic!("primary Creating");
            };
            let MemberState::Creating(replica) = &operating.roles[1].member.state else {
                panic!("replica Creating");
            };
            (
                primary.activation.authorization.as_ptr(),
                replica.activation.authorization.as_ptr(),
            )
        }
        _ => panic!("ordinary public initialization enters Operating"),
    };
    let primary_payload: Box<[u8]> = [67, 71].into();
    let primary_allocation = primary_payload.as_ptr();
    let replica_payload: Box<[u8]> = [73, 79].into();
    let replica_allocation = replica_payload.as_ptr();
    let (primary_job, primary_binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(11),
                2,
                SearchJob(primary_payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(90)),
            ),
        )
        .expect("primary admission"),
        SearchAddress(90),
        SubmissionId::new(11),
    );
    let (replica_job, replica_binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(13),
                3,
                SearchJob(replica_payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(91)),
            ),
        )
        .expect("replica admission"),
        SearchAddress(91),
        SubmissionId::new(13),
    );
    let acted = pool
        .on(panic_creation(primary, 97))
        .expect("matched existing panic receipt");
    assert_eq!(
        acted.sends.customer_outcomes.len(),
        2,
        "StopPool must return both already accepted role queues"
    );
    let outcomes = acted.sends.customer_outcomes.into_requests();
    for (outcome, (address, job_id, evidence, allocation, bytes)) in outcomes.into_iter().zip([
        (
            SearchAddress(90),
            primary_job,
            &primary_binding,
            primary_allocation,
            &[67, 71][..],
        ),
        (
            SearchAddress(91),
            replica_job,
            &replica_binding,
            replica_allocation,
            &[73, 79][..],
        ),
    ]) {
        let CustomerDelivery::Logical { delivery } = outcome else {
            panic!("complete original customer route");
        };
        assert_eq!(delivery.to.address(), address);
        let KeyedOutcome::ReturnedQueued {
            job,
            binding,
            payload,
            reason,
        } = delivery.message
        else {
            panic!("original accepted job returned");
        };
        assert_eq!(job, job_id);
        assert_eq!(binding.generation(), evidence.generation());
        assert_eq!(binding.role(), evidence.role());
        assert_eq!(payload.0.as_ptr(), allocation);
        assert_eq!(&*payload.0, bytes);
        assert_eq!(reason, KeyedQueuedReturnReason::PoolShutdown);
    }
    assert!(acted.creates.is_empty());
    assert!(matches!(acted.become_, Step::Continue));
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_assignments.is_empty());
    assert!(acted.sends.worker_preparations.is_empty());
    assert!(acted.sends.restart_schedules.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    let mut diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 3);
    let DiagnosticAction::Terminal { diagnostic } = diagnostics.remove(0);
    let KeyedDiagnosticCause::WorkerCreationRejected {
        role,
        rejection,
        activation,
        stopped,
    } = diagnostic.cause
    else {
        panic!("complete primary failure");
    };
    assert_eq!(role.role(), &SearchRole::Primary);
    let WorkerCreationRejection::WorkerPanicked { worker } = rejection else {
        panic!("original worker definition");
    };
    assert_eq!(worker.definition.as_ptr(), definition_allocation);
    assert_eq!(activation.authorization.as_ptr(), activation_allocation);
    assert!(stopped.is_none());
    for (diagnostic, (key, expected)) in diagnostics
        .into_iter()
        .zip([(2, &primary_binding), (3, &replica_binding)])
    {
        let DiagnosticAction::Terminal { diagnostic } = diagnostic;
        let KeyedDiagnosticCause::BindingRemoved {
            key: original,
            binding,
        } = diagnostic.cause
        else {
            panic!("complete removed binding");
        };
        assert_eq!(original, key);
        assert_eq!(binding.generation(), expected.generation());
        assert_eq!(binding.role(), expected.role());
    }
    let KeyedPoolState::Retiring { workers, .. } = &pool.state else {
        panic!("pending sibling remains retirement custody");
    };
    let primary_retired = workers
        .iter()
        .find(|worker| worker.role.role() == &SearchRole::Primary)
        .expect("original drained primary role");
    assert!(matches!(primary_retired.state, RetirementStatus::Drained));
    let replica_retiring = workers
        .iter()
        .find(|worker| worker.role.role() == &SearchRole::Replica)
        .expect("genuine pending sibling role");
    let RetirementStatus::AwaitingCreation(pending) = &replica_retiring.state else {
        panic!("no synthetic established sibling");
    };
    assert_eq!(pending.attempt.creation(), replica_id);
    assert_eq!(
        pending.activation.authorization.as_ptr(),
        replica_activation_allocation
    );
    assert!(pending.stopped.is_none());
    drop(replica);
}

fn unchanged_panic(
    actions: SearchActions,
    id: behavior::CreationId,
    kind: CreationKind,
    route: u64,
    allocation: *const u8,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
    assert!(actions.sends.worker_observations.is_empty());
    assert!(actions.sends.worker_initializations.is_empty());
    assert!(actions.sends.worker_activations.is_empty());
    assert!(actions.sends.customer_outcomes.is_empty());
    assert!(actions.sends.binding_replies.as_slice().is_empty());
    assert!(actions.sends.worker_assignments.is_empty());
    assert!(actions.sends.worker_preparations.is_empty());
    assert!(actions.sends.restart_schedules.is_empty());
    assert!(actions.sends.worker_shutdowns.is_empty());
    let mut diagnostics = actions.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 1);
    let DiagnosticAction::Terminal { diagnostic } = diagnostics
        .pop()
        .expect("complete original unexpected input");
    let KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerCreationsSettled(input)) =
        diagnostic.cause
    else {
        panic!("the unrelated receipt remains an owned whole input");
    };
    assert_panic_input(input, id, kind, route, allocation)
}

#[test]
fn unrelated_creation_failure_returns_foreign_id_and_wrong_kind_twice() {
    let initialized = search_pool(PoolFailureReaction::StopPool)
        .initialize()
        .expect("public keyed initialization");
    let mut pool = initialized.behavior;
    let mut requests = initialized.actions.creates.into_iter();
    let primary = requests.next().expect("primary request");
    let replica = requests.next().expect("replica request");
    let primary_id = primary.id();
    let replica_id = replica.id();
    let (activation_allocation, replica_activation_allocation) = match &pool.state {
        KeyedPoolState::Operating(operating) => {
            let MemberState::Creating(primary) = &operating.roles[0].member.state else {
                panic!("primary Creating");
            };
            let MemberState::Creating(replica) = &operating.roles[1].member.state else {
                panic!("replica Creating");
            };
            (
                primary.activation.authorization.as_ptr(),
                replica.activation.authorization.as_ptr(),
            )
        }
        _ => panic!("public initialization"),
    };
    let payload: Box<[u8]> = [67, 71].into();
    let payload_allocation = payload.as_ptr();
    let (job, binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(11),
                2,
                SearchJob(payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(90)),
            ),
        )
        .expect("genuine accepted primary job"),
        SearchAddress(90),
        SubmissionId::new(11),
    );
    let mut sequence = CreationSequence::new();
    let foreign = loop {
        let issued = sequence.issue().expect("foreign ID available");
        if issued != primary_id && issued != replica_id {
            break issued;
        }
    };
    let (_, child, _) = primary.into_parts();
    let worker = child.into_inner();
    let allocation = worker.definition.as_ptr();
    let mut input = panic_creation(CreateChild::birth(foreign, StopOnShutdown::new(worker)), 97);
    for _ in 0..2 {
        input = unchanged_panic(
            pool.on(input).expect("foreign input preserved"),
            foreign,
            CreationKind::Birth,
            97,
            allocation,
        );
    }
    let CreationSettlement::Settled(items) = input.into_settlement() else {
        panic!("whole foreign batch");
    };
    let SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::InitializationPanicked { creation },
    )) = items.into_iter().next().expect("one foreign receipt")
    else {
        panic!("original receipt");
    };
    let (request, route) = creation.into_parts();
    let (_, child, _) = request.into_parts();
    let kind = CreationKind::Replacement {
        previous: replica_id,
    };
    let mut input = panic_creation(
        CreateChild::replacement(primary_id, replica_id, child),
        route,
    );
    for _ in 0..2 {
        input = unchanged_panic(
            pool.on(input).expect("wrong kind preserved"),
            primary_id,
            kind,
            route,
            allocation,
        );
    }
    drop(input);
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("neither unrelated receipt selects recovery");
    };
    assert_eq!(
        operating
            .bindings
            .binding(&2)
            .expect("original binding")
            .generation(),
        binding.generation()
    );
    assert_eq!(
        operating
            .bindings
            .binding(&2)
            .expect("original binding")
            .role(),
        binding.role()
    );
    let queued = operating.roles[0]
        .queue
        .values()
        .next()
        .expect("original queued job");
    assert_eq!(queued.id, job);
    assert_eq!(queued.payload.0.as_ptr(), payload_allocation);
    assert_eq!(&*queued.payload.0, &[67, 71]);
    let MemberState::Creating(primary) = &operating.roles[0].member.state else {
        panic!("original primary startup");
    };
    let MemberState::Creating(replica_pending) = &operating.roles[1].member.state else {
        panic!("original replica startup");
    };
    assert_eq!(primary.attempt.creation(), primary_id);
    assert_eq!(
        primary.activation.authorization.as_ptr(),
        activation_allocation
    );
    assert_eq!(replica_pending.attempt.creation(), replica_id);
    assert_eq!(
        replica_pending.activation.authorization.as_ptr(),
        replica_activation_allocation
    );
    drop(replica);
}

#[test]
fn settled_creation_failure_replay_returns_complete_original_input_twice() {
    let initialized = search_pool(PoolFailureReaction::RetireRole)
        .initialize()
        .expect("public keyed initialization");
    let mut pool = initialized.behavior;
    let mut requests = initialized.actions.creates.into_iter();
    let primary = requests.next().expect("primary request");
    let replica = requests.next().expect("replica request");
    let id = primary.id();
    let kind = primary.kind();
    let committed = pool
        .on(established_creation(primary))
        .expect("genuine prior primary commitment");
    assert_eq!(committed.sends.worker_initializations.len(), 1);
    let worker = SearchWorker {
        definition: [31, 37].into(),
    };
    let allocation = worker.definition.as_ptr();
    let mut input = panic_creation(CreateChild::birth(id, StopOnShutdown::new(worker)), 97);
    for _ in 0..2 {
        input = unchanged_panic(
            pool.on(input).expect("stale completion preserved"),
            id,
            kind,
            97,
            allocation,
        );
    }
    drop(input);
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("replay cannot retire pool");
    };
    assert!(matches!(
        operating.roles[0].member.state,
        MemberState::Worker(_)
    ));
    assert!(matches!(
        operating.roles[1].member.state,
        MemberState::Creating(_)
    ));
    drop(replica);
}

#[test]
fn retiring_creation_failure_preserves_original_activation_and_prior_stop() {
    let initialized = search_pool(PoolFailureReaction::RetireRole)
        .initialize()
        .expect("public keyed initialization");
    let mut pool = initialized.behavior;
    let mut requests = initialized.actions.creates.into_iter();
    let primary = requests.next().expect("primary request");
    let replica = requests.next().expect("replica request");
    let replica_id = replica.id();
    let (id, child, _) = primary.into_parts();
    let worker = child.into_inner();
    let allocation = worker.definition.as_ptr();
    let primary = CreateChild::birth(id, StopOnShutdown::new(worker));
    let retired = pool
        .on(ShutdownRequested)
        .expect("public shutdown reaches AwaitingCreation");
    assert!(retired.sends.worker_shutdowns.is_empty());
    assert!(matches!(retired.become_, Step::Continue));
    let (activation_allocation, replica_activation_allocation) = match &pool.state {
        KeyedPoolState::Retiring { workers, .. } => {
            let RetirementStatus::AwaitingCreation(primary) = &workers[0].state else {
                panic!("unestablished primary");
            };
            let RetirementStatus::AwaitingCreation(replica) = &workers[1].state else {
                panic!("unestablished replica");
            };
            (
                primary.activation.authorization.as_ptr(),
                replica.activation.authorization.as_ptr(),
            )
        }
        _ => panic!("genuine retirement journey"),
    };
    let stopped = ChildStopped::new(id, Err(Crash::Panicked), Instant::now());
    let waiting = pool
        .on(stopped)
        .expect("known unestablished primary stop retained");
    assert!(waiting.sends.diagnostics.is_empty());
    let failed = pool
        .on(panic_creation(primary, 97))
        .expect("complete prior stop joins failed creation");
    assert!(failed.creates.is_empty());
    assert!(failed.sends.worker_initializations.is_empty());
    assert!(failed.sends.worker_activations.is_empty());
    assert!(failed.sends.worker_shutdowns.is_empty());
    assert!(failed.sends.customer_outcomes.is_empty());
    let mut diagnostics = failed.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 1);
    let DiagnosticAction::Terminal { diagnostic } =
        diagnostics.pop().expect("complete returned primary");
    let KeyedDiagnosticCause::WorkerCreationRejected {
        role,
        rejection,
        activation,
        stopped: original,
    } = diagnostic.cause
    else {
        panic!("existing complete failure diagnostic");
    };
    assert_eq!(role.role(), &SearchRole::Primary);
    let WorkerCreationRejection::WorkerPanicked { worker } = rejection else {
        panic!("same original definition");
    };
    assert_eq!(worker.definition.as_ptr(), allocation);
    assert_eq!(&*worker.definition, &[31, 37]);
    assert_eq!(activation.authorization.as_ptr(), activation_allocation);
    assert_eq!(&*activation.authorization, &[47, 53]);
    assert_eq!(original, Some(stopped));
    let KeyedPoolState::Retiring { workers, .. } = &pool.state else {
        panic!("pending replica remains owned");
    };
    assert_eq!(workers.len(), 1);
    let RetirementStatus::AwaitingCreation(pending) = &workers[0].state else {
        panic!("original replica custody");
    };
    assert_eq!(pending.attempt.creation(), replica_id);
    assert_eq!(
        pending.activation.authorization.as_ptr(),
        replica_activation_allocation
    );
    assert!(pending.stopped.is_none());
    drop(replica);
}

fn mixed_creation_batch(policy: PoolFailureReaction, first: SearchRole) {
    let roles = match &first {
        SearchRole::Primary => OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica]),
        SearchRole::Replica => OrderedRoles::new(SearchRole::Replica, [SearchRole::Primary]),
    }
    .expect("both meaningful roster orders");
    let initialized = search_pool_with_roles(policy, roles)
        .initialize()
        .expect("public initialization");
    let mut pool = initialized.behavior;
    let mut requests = initialized.actions.creates.into_iter();
    let one = requests.next().expect("first role request");
    let two = requests.next().expect("second role request");
    assert!(requests.next().is_none());
    let (primary, replica) = match &first {
        SearchRole::Primary => (one, two),
        SearchRole::Replica => (two, one),
    };
    let replica_id = replica.id();
    let (_, child, kind) = primary.into_parts();
    let worker = child.into_inner();
    let allocation = worker.definition.as_ptr();
    let primary_id = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .find_map(|cell| {
                if cell.member.role.role() == &SearchRole::Primary {
                    let MemberState::Creating(starting) = &cell.member.state else {
                        panic!("public primary startup");
                    };
                    Some(starting.attempt.creation())
                } else {
                    None
                }
            })
            .expect("real primary role"),
        _ => panic!("public operating state"),
    };
    let activation_allocation = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .find_map(|cell| {
                if cell.member.role.role() == &SearchRole::Primary {
                    let MemberState::Creating(starting) = &cell.member.state else {
                        panic!("public primary startup");
                    };
                    Some(starting.activation.authorization.as_ptr())
                } else {
                    None
                }
            })
            .expect("real primary activation"),
        _ => panic!("public operating state"),
    };
    let failed = panic_creation(
        CreateChild::birth(primary_id, StopOnShutdown::new(worker)),
        97,
    );
    let committed = established_creation(replica);
    let CreationSettlement::Settled(failed) = failed.into_settlement() else {
        panic!("existing failed receipt");
    };
    let CreationSettlement::Settled(committed) = committed.into_settlement() else {
        panic!("real sibling commitment");
    };
    let mut batch = Creations::empty();
    match &first {
        SearchRole::Primary => {
            batch.extend(failed);
            batch.extend(committed);
        }
        SearchRole::Replica => {
            batch.extend(committed);
            batch.extend(failed);
        }
    }
    let acted = pool
        .on(CreationsSettled::new(CreationSettlement::Settled(batch)))
        .expect("one correlated mixed batch");
    let mut diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 1);
    let DiagnosticAction::Terminal { diagnostic } = diagnostics
        .pop()
        .expect("complete expected creation failure");
    let KeyedDiagnosticCause::WorkerCreationRejected {
        role,
        rejection,
        activation,
        stopped,
    } = diagnostic.cause
    else {
        panic!("matching failure in mixed batch must be admitted");
    };
    assert_eq!(role.role(), &SearchRole::Primary);
    let WorkerCreationRejection::WorkerPanicked { worker } = rejection else {
        panic!("complete original failed definition");
    };
    assert_eq!(worker.definition.as_ptr(), allocation);
    assert_eq!(&*worker.definition, &[31, 37]);
    assert_eq!(activation.authorization.as_ptr(), activation_allocation);
    assert_eq!(&*activation.authorization, &[47, 53]);
    assert!(stopped.is_none());
    assert!(acted.creates.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.customer_outcomes.is_empty());
    assert_eq!(kind, CreationKind::Birth);
    match policy {
        PoolFailureReaction::RetireRole => {
            assert!(acted.sends.worker_shutdowns.is_empty());
            assert_eq!(acted.sends.worker_initializations.len(), 1);
            assert_eq!(
                acted
                    .sends
                    .worker_initializations
                    .into_requests()
                    .pop()
                    .expect("independent committed sibling initialization")
                    .worker()
                    .creation(),
                replica_id
            );
            let KeyedPoolState::Operating(operating) = &pool.state else {
                panic!("sibling remains operating");
            };
            let replica = operating
                .roles
                .iter()
                .find(|cell| cell.member.role.role() == &SearchRole::Replica)
                .expect("genuine sibling");
            assert!(matches!(replica.member.state, MemberState::Worker(_)));
        }
        PoolFailureReaction::StopPool => {
            assert_eq!(
                acted.sends.worker_shutdowns.len(),
                1,
                "committed sibling must be retired even when its receipt follows failure"
            );
            let expected_initializations = match first {
                SearchRole::Primary => 0,
                SearchRole::Replica => 1,
            };
            assert_eq!(
                acted.sends.worker_initializations.len(),
                expected_initializations,
                "an already emitted independent initialization stays in the prefix"
            );
            let KeyedPoolState::Retiring { workers, .. } = &pool.state else {
                panic!("committed sibling retained until actual retirement");
            };
            let primary = workers
                .iter()
                .find(|worker| worker.role.role() == &SearchRole::Primary)
                .expect("original drained primary role");
            assert!(matches!(primary.state, RetirementStatus::Drained));
            let replica = workers
                .iter()
                .find(|worker| worker.role.role() == &SearchRole::Replica)
                .expect("genuine committed sibling role");
            let RetirementStatus::Established { current, .. } = &replica.state else {
                panic!("original committed sibling retained");
            };
            assert_eq!(current.attempt.creation(), replica_id);
            let shutdown = acted
                .sends
                .worker_shutdowns
                .into_requests()
                .pop()
                .expect("one genuine sibling shutdown");
            assert_eq!(
                shutdown
                    .actor()
                    .recipient()
                    .interpret(&mut SearchEndpointTransfer),
                SearchEndpoint(83)
            );
        }
    }
}

#[test]
fn mixed_creation_failure_retires_role_before_sibling_commitment() {
    mixed_creation_batch(PoolFailureReaction::RetireRole, SearchRole::Primary);
}
#[test]
fn mixed_creation_failure_retires_role_after_sibling_commitment() {
    mixed_creation_batch(PoolFailureReaction::RetireRole, SearchRole::Replica);
}
#[test]
fn mixed_creation_failure_stops_pool_before_sibling_commitment() {
    mixed_creation_batch(PoolFailureReaction::StopPool, SearchRole::Primary);
}
#[test]
fn mixed_creation_failure_stops_pool_after_sibling_commitment() {
    mixed_creation_batch(PoolFailureReaction::StopPool, SearchRole::Replica);
}

#[test]
fn matching_namespace_batch_failure_preserves_both_creating_workers() {
    let initialized = search_pool(PoolFailureReaction::RetireRole)
        .initialize()
        .expect("public two-role initialization");
    let mut pool = initialized.behavior;
    let activation_allocations: Vec<_> = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .map(|cell| {
                let MemberState::Creating(starting) = &cell.member.state else {
                    panic!("actual Creating member");
                };
                starting.activation.authorization.as_ptr()
            })
            .collect(),
        _ => panic!("genuine initial state"),
    };
    let mut allocations = Vec::new();
    let requests: Creations<_> = initialized
        .actions
        .creates
        .into_iter()
        .map(|request| {
            let (id, child, kind) = request.into_parts();
            assert_eq!(kind, CreationKind::Birth);
            let worker = child.into_inner();
            allocations.push(worker.definition.as_ptr());
            CreateChild::birth(id, StopOnShutdown::new(worker))
        })
        .collect();
    let acted = pool
        .on(CreationsSettled::new(CreationSettlement::Rejected {
            creations: requests,
            reason: behavior::ChildNamespaceExhausted,
        }))
        .expect("the whole original failed batch is an ordinary typed input");
    let diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(
        diagnostics.len(),
        2,
        "matching batch failure must return both actual workers and original plans"
    );
    for (diagnostic, (role_name, allocation, authorization, bytes, plan)) in
        diagnostics.into_iter().zip([
            (
                SearchRole::Primary,
                allocations[0],
                activation_allocations[0],
                &[31, 37][..],
                &[47, 53][..],
            ),
            (
                SearchRole::Replica,
                allocations[1],
                activation_allocations[1],
                &[41, 43][..],
                &[59, 61][..],
            ),
        ])
    {
        let DiagnosticAction::Terminal { diagnostic } = diagnostic;
        let KeyedDiagnosticCause::WorkerCreationRejected {
            role,
            rejection,
            activation,
            stopped,
        } = diagnostic.cause
        else {
            panic!("one existing complete creation rejection per actual role");
        };
        assert_eq!(role.role(), &role_name);
        let WorkerCreationRejection::NamespaceExhausted { worker } = rejection else {
            panic!("original namespace rejection");
        };
        assert_eq!(worker.definition.as_ptr(), allocation);
        assert_eq!(&*worker.definition, bytes);
        assert_eq!(activation.authorization.as_ptr(), authorization);
        assert_eq!(&*activation.authorization, plan);
        assert!(stopped.is_none());
    }
    assert!(acted.creates.is_empty());
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    assert!(acted.sends.customer_outcomes.is_empty());
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("explicit RetireRole policy retains pool");
    };
    assert!(
        operating
            .roles
            .iter()
            .all(|cell| matches!(cell.member.state, MemberState::Retired))
    );
}

#[test]
fn matching_corrupt_batch_failure_preserves_both_creating_workers() {
    let initialized = search_pool(PoolFailureReaction::RetireRole)
        .initialize()
        .expect("public two-role initialization");
    let mut pool = initialized.behavior;
    let activation_allocations: Vec<_> = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .map(|cell| {
                let MemberState::Creating(starting) = &cell.member.state else {
                    panic!("actual Creating member");
                };
                starting.activation.authorization.as_ptr()
            })
            .collect(),
        _ => panic!("genuine initial state"),
    };
    let mut allocations = Vec::new();
    let requests: Creations<_> = initialized
        .actions
        .creates
        .into_iter()
        .map(|request| {
            let (id, child, kind) = request.into_parts();
            assert_eq!(kind, CreationKind::Birth);
            let worker = child.into_inner();
            allocations.push(worker.definition.as_ptr());
            CreateChild::birth(id, StopOnShutdown::new(worker))
        })
        .collect();
    let acted = pool
        .on(CreationsSettled::new(CreationSettlement::Corrupt {
            creations: requests,
            fault: behavior::InterpreterFault::CorruptTraversal,
        }))
        .expect("the whole original failed batch is an ordinary typed input");
    let diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(
        diagnostics.len(),
        2,
        "matching batch failure must return both actual workers and original plans"
    );
    for (diagnostic, (role_name, allocation, authorization, bytes, plan)) in
        diagnostics.into_iter().zip([
            (
                SearchRole::Primary,
                allocations[0],
                activation_allocations[0],
                &[31, 37][..],
                &[47, 53][..],
            ),
            (
                SearchRole::Replica,
                allocations[1],
                activation_allocations[1],
                &[41, 43][..],
                &[59, 61][..],
            ),
        ])
    {
        let DiagnosticAction::Terminal { diagnostic } = diagnostic;
        let KeyedDiagnosticCause::WorkerCreationRejected {
            role,
            rejection,
            activation,
            stopped,
        } = diagnostic.cause
        else {
            panic!("one existing complete creation rejection per actual role");
        };
        assert_eq!(role.role(), &role_name);
        let WorkerCreationRejection::InterpreterCorrupt { worker, fault } = rejection else {
            panic!("original interpreter corruption");
        };
        assert_eq!(fault, behavior::InterpreterFault::CorruptTraversal);
        assert_eq!(worker.definition.as_ptr(), allocation);
        assert_eq!(&*worker.definition, bytes);
        assert_eq!(activation.authorization.as_ptr(), authorization);
        assert_eq!(&*activation.authorization, plan);
        assert!(stopped.is_none());
    }
    assert!(acted.creates.is_empty());
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    assert!(acted.sends.customer_outcomes.is_empty());
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("explicit RetireRole policy retains pool");
    };
    assert!(
        operating
            .roles
            .iter()
            .all(|cell| matches!(cell.member.state, MemberState::Retired))
    );
}

#[test]
fn matching_namespace_batch_failure_stops_pool_with_complete_original_workers() {
    let initialized = search_pool(PoolFailureReaction::StopPool)
        .initialize()
        .expect("public two-role initialization");
    let mut pool = initialized.behavior;
    let activation_allocations: Vec<_> = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .map(|cell| {
                let MemberState::Creating(starting) = &cell.member.state else {
                    panic!("actual Creating member");
                };
                starting.activation.authorization.as_ptr()
            })
            .collect(),
        _ => panic!("genuine initial state"),
    };
    let mut allocations = Vec::new();
    let requests: Creations<_> = initialized
        .actions
        .creates
        .into_iter()
        .map(|request| {
            let (id, child, kind) = request.into_parts();
            assert_eq!(kind, CreationKind::Birth);
            let worker = child.into_inner();
            allocations.push(worker.definition.as_ptr());
            CreateChild::birth(id, StopOnShutdown::new(worker))
        })
        .collect();
    let acted = pool
        .on(CreationsSettled::new(CreationSettlement::Rejected {
            creations: requests,
            reason: behavior::ChildNamespaceExhausted,
        }))
        .expect("the whole original failed batch is an ordinary typed input");
    let diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(
        diagnostics.len(),
        2,
        "matching batch failure must return both actual workers and original plans"
    );
    for (diagnostic, (role_name, allocation, authorization, bytes, plan)) in
        diagnostics.into_iter().zip([
            (
                SearchRole::Primary,
                allocations[0],
                activation_allocations[0],
                &[31, 37][..],
                &[47, 53][..],
            ),
            (
                SearchRole::Replica,
                allocations[1],
                activation_allocations[1],
                &[41, 43][..],
                &[59, 61][..],
            ),
        ])
    {
        let DiagnosticAction::Terminal { diagnostic } = diagnostic;
        let KeyedDiagnosticCause::WorkerCreationRejected {
            role,
            rejection,
            activation,
            stopped,
        } = diagnostic.cause
        else {
            panic!("one existing complete creation rejection per actual role");
        };
        assert_eq!(role.role(), &role_name);
        let WorkerCreationRejection::NamespaceExhausted { worker } = rejection else {
            panic!("original namespace rejection");
        };
        assert_eq!(worker.definition.as_ptr(), allocation);
        assert_eq!(&*worker.definition, bytes);
        assert_eq!(activation.authorization.as_ptr(), authorization);
        assert_eq!(&*activation.authorization, plan);
        assert!(stopped.is_none());
    }
    assert!(acted.creates.is_empty());
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    assert!(acted.sends.customer_outcomes.is_empty());
    assert!(matches!(pool.state, KeyedPoolState::Stopped));
    assert!(matches!(acted.become_, Step::Stop(_)));
}

#[test]
fn matching_corrupt_batch_failure_stops_pool_with_complete_original_workers() {
    let initialized = search_pool(PoolFailureReaction::StopPool)
        .initialize()
        .expect("public two-role initialization");
    let mut pool = initialized.behavior;
    let activation_allocations: Vec<_> = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .map(|cell| {
                let MemberState::Creating(starting) = &cell.member.state else {
                    panic!("actual Creating member");
                };
                starting.activation.authorization.as_ptr()
            })
            .collect(),
        _ => panic!("genuine initial state"),
    };
    let mut allocations = Vec::new();
    let requests: Creations<_> = initialized
        .actions
        .creates
        .into_iter()
        .map(|request| {
            let (id, child, kind) = request.into_parts();
            assert_eq!(kind, CreationKind::Birth);
            let worker = child.into_inner();
            allocations.push(worker.definition.as_ptr());
            CreateChild::birth(id, StopOnShutdown::new(worker))
        })
        .collect();
    let acted = pool
        .on(CreationsSettled::new(CreationSettlement::Corrupt {
            creations: requests,
            fault: behavior::InterpreterFault::CorruptTraversal,
        }))
        .expect("the whole original failed batch is an ordinary typed input");
    let diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(
        diagnostics.len(),
        2,
        "matching batch failure must return both actual workers and original plans"
    );
    for (diagnostic, (role_name, allocation, authorization, bytes, plan)) in
        diagnostics.into_iter().zip([
            (
                SearchRole::Primary,
                allocations[0],
                activation_allocations[0],
                &[31, 37][..],
                &[47, 53][..],
            ),
            (
                SearchRole::Replica,
                allocations[1],
                activation_allocations[1],
                &[41, 43][..],
                &[59, 61][..],
            ),
        ])
    {
        let DiagnosticAction::Terminal { diagnostic } = diagnostic;
        let KeyedDiagnosticCause::WorkerCreationRejected {
            role,
            rejection,
            activation,
            stopped,
        } = diagnostic.cause
        else {
            panic!("one existing complete creation rejection per actual role");
        };
        assert_eq!(role.role(), &role_name);
        let WorkerCreationRejection::InterpreterCorrupt { worker, fault } = rejection else {
            panic!("original interpreter corruption");
        };
        assert_eq!(fault, behavior::InterpreterFault::CorruptTraversal);
        assert_eq!(worker.definition.as_ptr(), allocation);
        assert_eq!(&*worker.definition, bytes);
        assert_eq!(activation.authorization.as_ptr(), authorization);
        assert_eq!(&*activation.authorization, plan);
        assert!(stopped.is_none());
    }
    assert!(acted.creates.is_empty());
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    assert!(acted.sends.customer_outcomes.is_empty());
    assert!(matches!(pool.state, KeyedPoolState::Stopped));
    assert!(matches!(acted.become_, Step::Stop(_)));
}

fn namespace_batch(
    requests: Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>>,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    CreationsSettled::new(CreationSettlement::Rejected {
        creations: requests,
        reason: behavior::ChildNamespaceExhausted,
    })
}
fn corrupt_batch(
    requests: Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>>,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    CreationsSettled::new(CreationSettlement::Corrupt {
        creations: requests,
        fault: behavior::InterpreterFault::CorruptTraversal,
    })
}
fn namespace_requests(
    returned: CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
) -> Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>> {
    let CreationSettlement::Rejected { creations, reason } = returned.into_settlement() else {
        panic!("the exact original namespace batch variant survives");
    };
    assert_eq!(reason, behavior::ChildNamespaceExhausted);
    creations
}
fn corrupt_requests(
    returned: CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
) -> Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>> {
    let CreationSettlement::Corrupt { creations, fault } = returned.into_settlement() else {
        panic!("the exact original corrupt batch variant survives");
    };
    assert_eq!(fault, behavior::InterpreterFault::CorruptTraversal);
    creations
}
fn unrelated_batch(
    settle: fn(
        Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>>,
    ) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
    recover: fn(
        CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
    ) -> Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>>,
    provenance: CreationKind,
) {
    let initialized = search_pool(PoolFailureReaction::StopPool)
        .initialize()
        .expect("genuine initial request batch");
    let mut pool = initialized.behavior;
    let mut requests = initialized.actions.creates.into_iter();
    let primary = requests.next().expect("valid original first item");
    let replica = requests.next().expect("original second worker");
    let primary_id = primary.id();
    let replica_id = replica.id();
    let payload: Box<[u8]> = [67, 71].into();
    let payload_allocation = payload.as_ptr();
    let (job, binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(11),
                2,
                SearchJob(payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(90)),
            ),
        )
        .expect("existing complete accepted customer obligation"),
        SearchAddress(90),
        SubmissionId::new(11),
    );
    let mut sequence = CreationSequence::new();
    let foreign_id = loop {
        let id = sequence.issue().expect("independent input ID");
        if id != primary_id && id != replica_id {
            break id;
        }
    };
    let (replica_input_id, replica_input_kind) = match provenance {
        CreationKind::Birth => (foreign_id, CreationKind::Birth),
        CreationKind::Replacement { previous } => {
            (replica_id, CreationKind::Replacement { previous })
        }
    };
    let (id, child, kind) = primary.into_parts();
    let primary_worker = child.into_inner();
    let primary_allocation = primary_worker.definition.as_ptr();
    let primary = CreateChild::birth(id, StopOnShutdown::new(primary_worker));
    let (_, child, _) = replica.into_parts();
    let replica_worker = child.into_inner();
    let replica_allocation = replica_worker.definition.as_ptr();
    let replica = match replica_input_kind {
        CreationKind::Birth => {
            CreateChild::birth(replica_input_id, StopOnShutdown::new(replica_worker))
        }
        CreationKind::Replacement { previous } => CreateChild::replacement(
            replica_input_id,
            previous,
            StopOnShutdown::new(replica_worker),
        ),
    };
    let activation_allocations: Vec<_> = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .map(|cell| {
                let MemberState::Creating(starting) = &cell.member.state else {
                    panic!("original activation owner");
                };
                starting.activation.authorization.as_ptr()
            })
            .collect(),
        _ => panic!("neither input is yet interpreted"),
    };
    let mut input = settle(Creations::one(primary).and(replica));
    for _ in 0..2 {
        let acted = pool
            .on(input)
            .expect("complete invalid second item rejects whole batch");
        assert!(acted.creates.is_empty());
        assert!(acted.sends.customer_outcomes.is_empty());
        assert!(acted.sends.worker_initializations.is_empty());
        assert!(acted.sends.worker_shutdowns.is_empty());
        let mut diagnostics = acted.sends.diagnostics.into_requests();
        assert_eq!(diagnostics.len(), 1);
        let DiagnosticAction::Terminal { diagnostic } =
            diagnostics.pop().expect("whole batch diagnostic");
        let KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerCreationsSettled(returned)) =
            diagnostic.cause
        else {
            panic!("valid first item cannot be consumed before whole correlation");
        };
        let requests = recover(returned);
        assert_eq!(requests.len(), 2);
        let requests = requests
            .into_iter()
            .zip([
                (primary_id, kind, primary_allocation, &[31, 37][..]),
                (
                    replica_input_id,
                    replica_input_kind,
                    replica_allocation,
                    &[41, 43][..],
                ),
            ])
            .map(
                |(request, (expected_id, expected_kind, allocation, bytes))| {
                    assert_eq!(request.id(), expected_id);
                    assert_eq!(request.kind(), expected_kind);
                    let (id, child, kind) = request.into_parts();
                    let worker = child.into_inner();
                    assert_eq!(worker.definition.as_ptr(), allocation);
                    assert_eq!(&*worker.definition, bytes);
                    match kind {
                        CreationKind::Birth => CreateChild::birth(id, StopOnShutdown::new(worker)),
                        CreationKind::Replacement { previous } => {
                            CreateChild::replacement(id, previous, StopOnShutdown::new(worker))
                        }
                    }
                },
            )
            .collect();
        input = settle(requests);
    }
    drop(input);
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("invalid batch cannot select StopPool");
    };
    assert_eq!(
        operating
            .bindings
            .binding(&2)
            .expect("original binding")
            .generation(),
        binding.generation()
    );
    let queued = operating.roles[0]
        .queue
        .values()
        .next()
        .expect("valid first role preserves original queue");
    assert_eq!(queued.id, job);
    assert_eq!(queued.payload.0.as_ptr(), payload_allocation);
    assert_eq!(&*queued.payload.0, &[67, 71]);
    for (cell, allocation) in operating.roles.iter().zip(activation_allocations) {
        let MemberState::Creating(starting) = &cell.member.state else {
            panic!("every current member unchanged");
        };
        assert_eq!(starting.activation.authorization.as_ptr(), allocation);
    }
}
#[test]
fn namespace_batch_rejects_foreign_second_item_and_wrong_kind_unchanged_twice() {
    unrelated_batch(namespace_batch, namespace_requests, CreationKind::Birth);
    let previous = CreationSequence::new()
        .issue()
        .expect("named replacement provenance");
    unrelated_batch(
        namespace_batch,
        namespace_requests,
        CreationKind::Replacement { previous },
    );
}
#[test]
fn corrupt_batch_rejects_foreign_second_item_and_wrong_kind_unchanged_twice() {
    unrelated_batch(corrupt_batch, corrupt_requests, CreationKind::Birth);
    let previous = CreationSequence::new()
        .issue()
        .expect("named replacement provenance");
    unrelated_batch(
        corrupt_batch,
        corrupt_requests,
        CreationKind::Replacement { previous },
    );
}

fn namespace_worker(rejection: WorkerCreationRejection<SearchWorker>) -> SearchWorker {
    let WorkerCreationRejection::NamespaceExhausted { worker } = rejection else {
        panic!("exact replayed namespace cause");
    };
    worker
}
fn corrupt_worker(rejection: WorkerCreationRejection<SearchWorker>) -> SearchWorker {
    let WorkerCreationRejection::InterpreterCorrupt { worker, fault } = rejection else {
        panic!("exact replayed corruption cause");
    };
    assert_eq!(fault, behavior::InterpreterFault::CorruptTraversal);
    worker
}
fn replay_batch(
    settle: fn(
        Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>>,
    ) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
    recover: fn(
        CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>>,
    ) -> Creations<CreateChild<SearchAddress, StopOnShutdown<SearchWorker>>>,
    returned_worker: fn(WorkerCreationRejection<SearchWorker>) -> SearchWorker,
) {
    let initialized = search_pool(PoolFailureReaction::RetireRole)
        .initialize()
        .expect("actual initial creating batch");
    let mut pool = initialized.behavior;
    let identities: Vec<_> = initialized
        .actions
        .creates
        .iter()
        .map(|request| (request.id(), request.kind()))
        .collect();
    let activation_allocations: Vec<_> = match &pool.state {
        KeyedPoolState::Operating(operating) => operating
            .roles
            .iter()
            .map(|cell| {
                let MemberState::Creating(starting) = &cell.member.state else {
                    panic!("real original creating plan");
                };
                starting.activation.authorization.as_ptr()
            })
            .collect(),
        _ => panic!("public initialized state"),
    };
    let acted = pool
        .on(settle(initialized.actions.creates))
        .expect("genuine prior matched complete batch rejection");
    let diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), 2);
    let mut allocations = Vec::new();
    let requests: Creations<_> = diagnostics
        .into_iter()
        .zip(identities.iter().zip(activation_allocations))
        .zip([SearchRole::Primary, SearchRole::Replica])
        .map(
            |((diagnostic, (&(id, kind), authorization)), expected_role)| {
                let DiagnosticAction::Terminal { diagnostic } = diagnostic;
                let KeyedDiagnosticCause::WorkerCreationRejected {
                    role,
                    rejection,
                    activation,
                    stopped,
                } = diagnostic.cause
                else {
                    panic!("actual complete creation failure precedes replay");
                };
                assert_eq!(role.role(), &expected_role);
                assert_eq!(activation.authorization.as_ptr(), authorization);
                assert!(stopped.is_none());
                let worker = returned_worker(rejection);
                allocations.push(worker.definition.as_ptr());
                assert_eq!(kind, CreationKind::Birth);
                CreateChild::birth(id, StopOnShutdown::new(worker))
            },
        )
        .collect();
    let mut input = settle(requests);
    for _ in 0..2 {
        let acted = pool
            .on(input)
            .expect("replayed settled IDs conserve complete original bodies");
        assert!(acted.creates.is_empty());
        assert!(acted.sends.customer_outcomes.is_empty());
        assert!(acted.sends.worker_initializations.is_empty());
        assert!(acted.sends.worker_activations.is_empty());
        assert!(acted.sends.worker_shutdowns.is_empty());
        assert!(matches!(acted.become_, Step::Continue));
        let mut diagnostics = acted.sends.diagnostics.into_requests();
        assert_eq!(diagnostics.len(), 1);
        let DiagnosticAction::Terminal { diagnostic } =
            diagnostics.pop().expect("whole unchanged replay");
        let KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerCreationsSettled(returned)) =
            diagnostic.cause
        else {
            panic!("replay cannot apply recovery twice");
        };
        let requests = recover(returned);
        assert_eq!(requests.len(), 2);
        let requests = requests
            .into_iter()
            .zip(identities.iter().zip(allocations.iter()))
            .map(|(request, (&(id, kind), &allocation))| {
                assert_eq!(request.id(), id);
                assert_eq!(request.kind(), kind);
                let (id, child, _) = request.into_parts();
                let worker = child.into_inner();
                assert_eq!(worker.definition.as_ptr(), allocation);
                CreateChild::birth(id, StopOnShutdown::new(worker))
            })
            .collect();
        input = settle(requests);
    }
    drop(input);
    let KeyedPoolState::Operating(operating) = &pool.state else {
        panic!("replay cannot stop current pool");
    };
    assert!(
        operating
            .roles
            .iter()
            .all(|cell| matches!(cell.member.state, MemberState::Retired))
    );
}
#[test]
fn matched_namespace_batch_replay_preserves_complete_original_workers_twice() {
    replay_batch(namespace_batch, namespace_requests, namespace_worker);
}
#[test]
fn matched_corrupt_batch_replay_preserves_complete_original_workers_twice() {
    replay_batch(corrupt_batch, corrupt_requests, corrupt_worker);
}

fn unavailable_creation(
    id: behavior::CreationId,
    kind: CreationKind,
) -> CreationsSettled<SearchAddress, StopOnShutdown<SearchWorker>> {
    CreationsSettled::new(CreationSettlement::Settled(Creations::one(
        SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Unavailable { id, kind },
        )),
    )))
}
fn unavailable_creation_policy(policy: PoolFailureReaction) {
    let initialized = search_pool(policy)
        .initialize()
        .expect("public two-role initialization");
    let mut pool = initialized.behavior;
    let mut requests = initialized.actions.creates.into_iter();
    let primary = requests.next().expect("primary request");
    let replica = requests.next().expect("replica request");
    assert!(requests.next().is_none());
    let id = primary.id();
    let kind = primary.kind();
    let replica_id = replica.id();
    let (authorization, replica_authorization) = match &pool.state {
        KeyedPoolState::Operating(operating) => {
            let MemberState::Creating(primary) = &operating.roles[0].member.state else {
                panic!("actual primary Creating");
            };
            let MemberState::Creating(replica) = &operating.roles[1].member.state else {
                panic!("actual replica Creating");
            };
            (
                primary.activation.authorization.as_ptr(),
                replica.activation.authorization.as_ptr(),
            )
        }
        _ => panic!("actual initialized state"),
    };
    let primary_payload: Box<[u8]> = [67, 71].into();
    let primary_allocation = primary_payload.as_ptr();
    let replica_payload: Box<[u8]> = [73, 79].into();
    let replica_allocation = replica_payload.as_ptr();
    let (primary_job, primary_binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(11),
                2,
                SearchJob(primary_payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(90)),
            ),
        )
        .expect("actual primary obligation"),
        SearchAddress(90),
        SubmissionId::new(11),
    );
    let (replica_job, replica_binding) = admitted(
        pool.receive(
            SearchAddress(7),
            super::KeyedCommand::submit(
                SubmissionId::new(13),
                3,
                SearchJob(replica_payload),
                Recipient::<
                    MessageProtocol<SearchAddress, KeyedOutcome<u64, SearchRole, SearchJob, usize>>,
                >::global(SearchAddress(91)),
            ),
        )
        .expect("actual replica obligation"),
        SearchAddress(91),
        SubmissionId::new(13),
    );
    drop(primary);
    let acted = pool
        .on(unavailable_creation(id, kind))
        .expect("authoritative exact no-surviving-definition receipt");
    let expected_returns = match policy {
        PoolFailureReaction::RetireRole => 1,
        PoolFailureReaction::StopPool => 2,
    };
    assert_eq!(
        acted.sends.customer_outcomes.len(),
        expected_returns,
        "matching unavailability applies explicit configured failure policy"
    );
    let outcomes = acted.sends.customer_outcomes.into_requests();
    for (outcome, (customer, job_id, expected_binding, allocation, bytes)) in
        outcomes.into_iter().zip([
            (
                SearchAddress(90),
                primary_job,
                &primary_binding,
                primary_allocation,
                &[67, 71][..],
            ),
            (
                SearchAddress(91),
                replica_job,
                &replica_binding,
                replica_allocation,
                &[73, 79][..],
            ),
        ])
    {
        let CustomerDelivery::Logical { delivery } = outcome else {
            panic!("exact actual customer route");
        };
        assert_eq!(delivery.to.address(), customer);
        let KeyedOutcome::ReturnedQueued {
            job,
            binding,
            payload,
            reason,
        } = delivery.message
        else {
            panic!("original accepted job returned");
        };
        assert_eq!(job, job_id);
        assert_eq!(binding.generation(), expected_binding.generation());
        assert_eq!(binding.role(), expected_binding.role());
        assert_eq!(payload.0.as_ptr(), allocation);
        assert_eq!(&*payload.0, bytes);
        assert_eq!(
            reason,
            match policy {
                PoolFailureReaction::RetireRole =>
                    KeyedQueuedReturnReason::RolePermanentlyUnavailable,
                PoolFailureReaction::StopPool => KeyedQueuedReturnReason::PoolShutdown,
            }
        );
    }
    assert!(acted.creates.is_empty());
    assert!(acted.sends.worker_initializations.is_empty());
    assert!(acted.sends.worker_activations.is_empty());
    assert!(acted.sends.worker_shutdowns.is_empty());
    assert!(matches!(acted.become_, Step::Continue));
    let mut diagnostics = acted.sends.diagnostics.into_requests();
    assert_eq!(diagnostics.len(), expected_returns + 1);
    let DiagnosticAction::Terminal { diagnostic } = diagnostics.remove(0);
    let KeyedDiagnosticCause::WorkerCreationRejected {
        role,
        rejection,
        activation,
        stopped,
    } = diagnostic.cause
    else {
        panic!("one complete actual failure plus original plan");
    };
    assert_eq!(role.role(), &SearchRole::Primary);
    let WorkerCreationRejection::Unavailable {
        id: original,
        kind: provenance,
    } = rejection
    else {
        panic!("missing worker definition must not be fabricated");
    };
    assert_eq!(original, id);
    assert_eq!(provenance, kind);
    assert_eq!(activation.authorization.as_ptr(), authorization);
    assert_eq!(&*activation.authorization, &[47, 53]);
    assert!(stopped.is_none());
    for (diagnostic, (key, evidence)) in diagnostics
        .into_iter()
        .zip([(2, &primary_binding), (3, &replica_binding)])
    {
        let DiagnosticAction::Terminal { diagnostic } = diagnostic;
        let KeyedDiagnosticCause::BindingRemoved {
            key: original,
            binding,
        } = diagnostic.cause
        else {
            panic!("exact affected binding discharge");
        };
        assert_eq!(original, key);
        assert_eq!(binding.generation(), evidence.generation());
        assert_eq!(binding.role(), evidence.role());
    }
    let mut input = unavailable_creation(id, kind);
    for _ in 0..2 {
        let replay = pool.on(input).expect("exact replay survives unchanged");
        assert!(replay.creates.is_empty());
        assert!(replay.sends.customer_outcomes.is_empty());
        assert!(replay.sends.worker_initializations.is_empty());
        assert!(replay.sends.worker_shutdowns.is_empty());
        let mut diagnostics = replay.sends.diagnostics.into_requests();
        assert_eq!(diagnostics.len(), 1);
        let DiagnosticAction::Terminal { diagnostic } =
            diagnostics.pop().expect("complete replay diagnostic");
        let KeyedDiagnosticCause::Unexpected(KeyedEvent::WorkerCreationsSettled(returned)) =
            diagnostic.cause
        else {
            panic!("replay cannot apply configured policy twice");
        };
        let CreationSettlement::Settled(items) = returned.into_settlement() else {
            panic!("exact original settled variant");
        };
        assert_eq!(items.len(), 1);
        let SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Unavailable {
            id: original,
            kind: provenance,
        })) = items.into_iter().next().expect("one original replay")
        else {
            panic!("exact original unavailable receipt");
        };
        assert_eq!(original, id);
        assert_eq!(provenance, kind);
        input = unavailable_creation(original, provenance);
    }
    drop(input);
    match policy {
        PoolFailureReaction::RetireRole => {
            let KeyedPoolState::Operating(operating) = &pool.state else {
                panic!("independent sibling remains operating");
            };
            assert!(matches!(
                operating.roles[0].member.state,
                MemberState::Retired
            ));
            assert!(operating.roles[0].queue.is_empty());
            assert!(operating.bindings.binding(&2).is_none());
            assert_eq!(
                operating
                    .bindings
                    .binding(&3)
                    .expect("original sibling binding")
                    .generation(),
                replica_binding.generation()
            );
            let queued = operating.roles[1]
                .queue
                .values()
                .next()
                .expect("unaffected sibling job");
            assert_eq!(queued.id, replica_job);
            assert_eq!(queued.payload.0.as_ptr(), replica_allocation);
            let MemberState::Creating(sibling) = &operating.roles[1].member.state else {
                panic!("unaffected sibling startup");
            };
            assert_eq!(sibling.attempt.creation(), replica_id);
            assert_eq!(
                sibling.activation.authorization.as_ptr(),
                replica_authorization
            );
            let settled = pool
                .on(established_creation(replica))
                .expect("sibling can independently commit");
            assert_eq!(settled.sends.worker_initializations.len(), 1);
        }
        PoolFailureReaction::StopPool => {
            let KeyedPoolState::Retiring { workers, .. } = &pool.state else {
                panic!("original pending sibling retirement custody");
            };
            let sibling = workers
                .iter()
                .find(|worker| worker.role.role() == &SearchRole::Replica)
                .expect("original sibling");
            let RetirementStatus::AwaitingCreation(starting) = &sibling.state else {
                panic!("no fictitious committed worker");
            };
            assert_eq!(starting.attempt.creation(), replica_id);
            assert_eq!(
                starting.activation.authorization.as_ptr(),
                replica_authorization
            );
            assert!(starting.stopped.is_none());
            drop(replica);
        }
    }
}
#[test]
fn unavailable_creation_retires_only_primary_and_returns_exact_replay() {
    unavailable_creation_policy(PoolFailureReaction::RetireRole);
}
#[test]
fn unavailable_creation_stops_pool_and_returns_exact_replay() {
    unavailable_creation_policy(PoolFailureReaction::StopPool);
}
