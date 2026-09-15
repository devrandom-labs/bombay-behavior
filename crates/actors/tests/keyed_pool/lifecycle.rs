use core::ops::ControlFlow;
use std::cell::RefCell;
use std::convert::Infallible;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use behavior::{
    ActionItemResult, ChildCreationOutcome, ChildHead, CreateChild, CreationKind, CreationSequence,
    CreationSettlement, CreationsSettled, EstablishedCreation, EstablishedRecipient,
    InterpreterFault, ItemSettlement, MessageProtocol, Never, Recipient, SettledItem, Step,
};
use behavior_actors::atomic::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, AssignWorker, BacklogCapacity,
    BindingCapacity, CustomerDelivery, DiagnosticAction, DiagnosticDisposition,
    ImmediateActivation, Interruption, KeyedAssignedReturnReason, KeyedCommand, KeyedEvent,
    KeyedOutcome, KeyedQueuedReturnReason, OrderedRoles, PoolFailureReaction, PoolRecovery,
    RestartLimit, RestartRelease, SubmissionId, WorkerInitializationOutcome, WorkerSource,
    WorkerSubmission, keyed,
};
use behavior_actors::{
    Activate, ChildStopped, Crash, EstablishedShutdownResolved, Exit, ScheduleAfterRejection,
    ShutdownId, ShutdownRejection, StopOnShutdown, TimerElapsed, TimerGeneration, TimerId,
    TimerScheduled,
};

use super::domain::{
    Account, Endpoint, RuntimeAddr, SearchJob, SearchResult, SearchRole, SearchWorker,
    prepare_worker,
};

struct HeldActivation(Arc<AtomicUsize>);

impl Drop for HeldActivation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl ActivationPlan for HeldActivation {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        async move {
            drop(self);
            Ok(())
        }
    }
}

#[test]
fn construction_rejection_returns_the_worker_roster() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum WorkerRole {
        Search,
        Index,
        Spellcheck,
    }

    let calls = Rc::new(RefCell::new(Vec::new()));
    let factory_calls = Rc::clone(&calls);
    let roles = OrderedRoles::new(
        WorkerRole::Search,
        [WorkerRole::Index, WorkerRole::Spellcheck],
    )
    .unwrap_or_else(|_| panic!("worker roles are unique"));

    let mut rejection = match keyed(
        move |role: &WorkerRole| {
            factory_calls.borrow_mut().push(*role);
            match role {
                WorkerRole::Index => Err(SearchWorkerRejection::Unavailable),
                WorkerRole::Search | WorkerRole::Spellcheck => {
                    Ok(WorkerSubmission::immediate(SearchWorker))
                }
            }
        },
        roles,
        |_: &Account| WorkerRole::Search,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(4),
        BindingCapacity::new(4).unwrap_or_else(|_| panic!("four bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    ) {
        Ok(_) => panic!("index worker preparation must reject"),
        Err(rejection) => rejection,
    };

    assert_eq!(*calls.borrow(), [WorkerRole::Search, WorkerRole::Index]);
    assert_eq!(rejection.workers.prepared.len(), 1);
    assert_eq!(rejection.workers.prepared[0].role, WorkerRole::Search);
    assert_eq!(rejection.workers.role, WorkerRole::Index);
    assert_eq!(rejection.workers.reason, SearchWorkerRejection::Unavailable);
    assert_eq!(rejection.workers.remaining, [WorkerRole::Spellcheck]);
    let returned = (rejection.workers.factory)(&WorkerRole::Spellcheck);
    assert!(matches!(returned, Ok(_)));
}

#[derive(Debug, Eq, PartialEq)]
struct SearchSource;

impl WorkerSource<SearchRole, SearchWorker, ImmediateActivation> for SearchSource {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

impl WorkerSource<SearchRole, SearchWorker, HeldActivation> for SearchSource {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

#[derive(Debug, Eq, PartialEq)]
enum SearchWorkerRejection {
    Unavailable,
}

#[derive(Debug, Eq, PartialEq)]
enum SearchSourceRejection {
    Closed,
}

struct FallibleSearchSource;

impl WorkerSource<SearchRole, SearchWorker, ImmediateActivation> for FallibleSearchSource {
    type WorkerRejection = SearchWorkerRejection;
    type SourceRejection = SearchSourceRejection;
}

#[derive(Clone, Copy)]
enum ReturnedPreparation {
    Prepared,
    WorkerRejected,
    SourceRejected,
    InterpreterCorrupt,
    InterpretationSkipped,
}

#[derive(Clone, Copy)]
enum RestartScheduleDisposition {
    Rejected,
    InterpreterCorrupt,
    InterpretationSkipped,
}

#[derive(Clone, Copy)]
enum ShutdownDisposition {
    Accepted,
    Rejected,
}

#[derive(Clone, Copy)]
enum ShutdownJoinOrder {
    SettlementThenExit,
    ExitThenSettlement,
}

#[derive(Clone, Copy)]
enum UnavailableRolePolicy {
    RetireRole,
    StopPool,
}

fn commit_worker_creation(
    creation: CreateChild<RuntimeAddr, StopOnShutdown<SearchWorker>>,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<SearchWorker>> {
    let (worker, _, kind) = creation.into_parts();
    worker_creation_settlement(worker, kind)
}

fn worker_creation_settlement(
    worker: behavior::CreationId,
    kind: CreationKind,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<SearchWorker>> {
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::<StopOnShutdown<SearchWorker>, ChildHead>::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(Endpoint(40 + worker.get())),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

#[tokio::test]
async fn concurrent_worker_failures_claim_one_recovery_source() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |account: &Account| match account.0 % 2 {
            0 => SearchRole::Primary,
            _ => SearchRole::Replica,
        },
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(4),
        BindingCapacity::new(8).unwrap_or_else(|_| panic!("eight bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let workers: Vec<_> = creations.iter().map(|creation| creation.id()).collect();

    for creation in creations {
        let created = pool
            .on(commit_worker_creation(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = created
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("created worker awaits initialization"));
        let initialized = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("initialized worker begins activation"));
        let started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
        assert!(started.sends.worker_preparations.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.sends.worker_preparations.is_empty());
    }

    let primary_stopped = pool
        .on(ChildStopped::new(
            workers[0],
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("primary worker failure failed: {error}"));
    assert!(primary_stopped.creates.is_empty());
    let mut preparations = primary_stopped.sends.worker_preparations.into_items();
    let mut preparation = preparations
        .pop()
        .unwrap_or_else(|| panic!("the first failure claims the recovery source"));
    assert!(preparations.is_empty());
    let (source, role) = preparation.source_and_role();
    assert_eq!(source, &mut SearchSource);
    assert_eq!(role, &SearchRole::Primary);

    let replica_stopped = pool
        .on(ChildStopped::new(
            workers[1],
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("replica worker failure failed: {error}"));
    assert!(replica_stopped.creates.is_empty());
    assert!(replica_stopped.sends.worker_preparations.is_empty());

    let waiting_role = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(1),
                Account(3),
                SearchJob(30),
                Recipient::global(RuntimeAddr(90)),
            ),
        )
        .unwrap_or_else(|error| panic!("waiting-role submission failed: {error}"));
    assert!(waiting_role.sends.worker_assignments.is_empty());
    let accepted = waiting_role
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the waiting role accepts queued work"));
    match accepted {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::Accepted { binding, .. } => {
                assert_eq!(binding.role(), &SearchRole::Replica);
            }
            _ => panic!("the waiting role returns acceptance"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the waiting role must accept the queued job")
        }
    }

    let ControlFlow::Break(prepared_primary) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes one worker preparation")
    };
    let primary_recovery = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared_primary)),
        ))
        .unwrap_or_else(|error| panic!("primary worker preparation failed: {error}"));
    let mut replacements: Vec<_> = primary_recovery.creates.into_iter().collect();
    let replacement = replacements
        .pop()
        .unwrap_or_else(|| panic!("exact preparation creates the primary replacement"));
    assert!(replacements.is_empty());
    assert_eq!(replacement.kind(), CreationKind::replacement(workers[0]));
    let mut next_preparations = primary_recovery.sends.worker_preparations.into_items();
    let mut replica_preparation = next_preparations
        .pop()
        .unwrap_or_else(|| panic!("the returned source prepares the waiting replica"));
    assert!(next_preparations.is_empty());
    let (source, role) = replica_preparation.source_and_role();
    assert_eq!(source, &mut SearchSource);
    assert_eq!(role, &SearchRole::Replica);
}

#[tokio::test]
async fn nonzero_role_delayed_recovery_requires_the_exact_schedule_and_timer() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive release delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(2).unwrap_or_else(|_| panic!("two bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let workers: Vec<_> = creations.iter().map(|creation| creation.id()).collect();
    for creation in creations {
        let created = pool
            .on(commit_worker_creation(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = created
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("created worker awaits initialization"));
        let initialized = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("initialized worker begins activation"));
        let activation_started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
        assert!(activation_started.creates.is_empty());
        assert!(activation_started.sends.worker_observations.is_empty());
        assert!(activation_started.sends.worker_initializations.is_empty());
        assert!(activation_started.sends.worker_activations.is_empty());
        assert!(activation_started.sends.customer_outcomes.is_empty());
        assert!(
            activation_started
                .sends
                .binding_replies
                .as_slice()
                .is_empty()
        );
        assert!(activation_started.sends.worker_assignments.is_empty());
        assert!(activation_started.sends.worker_preparations.is_empty());
        assert!(activation_started.sends.restart_schedules.is_empty());
        assert!(activation_started.sends.worker_shutdowns.is_empty());
        assert!(activation_started.sends.diagnostics.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.creates.is_empty());
        assert!(ready.sends.worker_observations.is_empty());
        assert!(ready.sends.worker_initializations.is_empty());
        assert!(ready.sends.worker_activations.is_empty());
        assert!(ready.sends.customer_outcomes.is_empty());
        assert!(ready.sends.binding_replies.as_slice().is_empty());
        assert!(ready.sends.worker_assignments.is_empty());
        assert!(ready.sends.worker_preparations.is_empty());
        assert!(ready.sends.restart_schedules.is_empty());
        assert!(ready.sends.worker_shutdowns.is_empty());
        assert!(ready.sends.diagnostics.is_empty());
    }
    let worker = workers[1];

    let stopped = pool
        .on(ChildStopped::new(
            worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
    let mut preparation = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("worker failure requests one preparation"));
    let (source, role) = preparation.source_and_role();
    assert_eq!(source, &mut SearchSource);
    assert_eq!(role, &SearchRole::Replica);
    let ControlFlow::Break(prepared) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one role completes one worker preparation")
    };
    let scheduling = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
        ))
        .unwrap_or_else(|error| panic!("worker preparation failed: {error}"));
    assert!(scheduling.creates.is_empty());
    assert!(scheduling.sends.diagnostics.is_empty());
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));

    let foreign_schedule = behavior_actors::ScheduleAfter::new(
        TimerId(schedule.id.0 + 1),
        schedule.generation,
        schedule.after,
    );
    let unrelated_schedule = pool
        .transition(KeyedEvent::RestartScheduleSettled(
            SettledItem::Unattempted(foreign_schedule),
        ))
        .unwrap_or_else(|error| panic!("foreign schedule input failed: {error}"));
    assert!(unrelated_schedule.creates.is_empty());
    let mut diagnostics = unrelated_schedule.sends.diagnostics.into_requests();
    let diagnostic = match diagnostics.pop() {
        Some(DiagnosticAction::Terminal { diagnostic }) => diagnostic,
        Some(DiagnosticAction::Deliver { .. }) => {
            panic!("the configured diagnostic disposition is terminal")
        }
        None => panic!("the foreign schedule emits one diagnostic"),
    };
    assert!(diagnostics.is_empty());
    let diagnostic_debug = format!("{diagnostic:?}");
    assert!(diagnostic_debug.contains("KeyedDiagnostic"));
    assert!(!diagnostic_debug.contains("RestartScheduleSettled"));
    assert!(!diagnostic_debug.contains("TimerId"));

    let scheduled = pool
        .transition(KeyedEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("exact schedule receipt failed: {error}"));
    assert!(scheduled.creates.is_empty());
    assert!(scheduled.sends.diagnostics.is_empty());

    let foreign_timer = pool
        .on(TimerElapsed::new(
            schedule.id,
            TimerGeneration(schedule.generation.0 + 1),
        ))
        .unwrap_or_else(|error| panic!("foreign timer input failed: {error}"));
    assert!(foreign_timer.creates.is_empty());
    assert_eq!(foreign_timer.sends.diagnostics.len(), 1);

    let restarted = pool
        .on(TimerElapsed::new(schedule.id, schedule.generation))
        .unwrap_or_else(|error| panic!("exact restart timer failed: {error}"));
    let mut replacements: Vec<_> = restarted.creates.into_iter().collect();
    let replacement = replacements
        .pop()
        .unwrap_or_else(|| panic!("the exact timer creates one replacement"));
    assert!(replacements.is_empty());
    assert_eq!(replacement.kind(), CreationKind::replacement(worker));
    assert!(restarted.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn shutdown_cancels_a_replacement_waiting_for_its_restart_timer() {
    let initial_drops = Arc::new(AtomicUsize::new(0));
    let replacement_drops = Arc::new(AtomicUsize::new(0));
    let mut initial_activation = Some(HeldActivation(Arc::clone(&initial_drops)));
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        move |_: &SearchRole| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                initial_activation
                    .take()
                    .unwrap_or_else(|| panic!("one role prepares one worker")),
            ))
        },
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive release delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the declared role creates one worker"));
    let worker = creation.id();
    let created = pool
        .on(commit_worker_creation(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let initialized = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
    assert!(started.creates.is_empty());
    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(ready.creates.is_empty());
    assert_eq!(initial_drops.load(Ordering::SeqCst), 1);

    let stopped = pool
        .on(ChildStopped::new(
            worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
    let preparation = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("worker failure requests one preparation"));
    let ControlFlow::Break(prepared) = preparation.accept(WorkerSubmission::activated(
        SearchWorker,
        HeldActivation(Arc::clone(&replacement_drops)),
    )) else {
        panic!("one role completes one worker preparation")
    };
    let scheduling = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
        ))
        .unwrap_or_else(|error| panic!("worker preparation failed: {error}"));
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));
    let scheduled = pool
        .transition(KeyedEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("exact schedule receipt failed: {error}"));
    assert!(scheduled.creates.is_empty());
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 0);

    let shutdown = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    assert!(matches!(shutdown.become_, Step::Stop(_)));
    assert!(shutdown.creates.is_empty());
    assert!(shutdown.sends.restart_schedules.is_empty());
    assert!(shutdown.sends.worker_shutdowns.is_empty());
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 0);
    assert_eq!(shutdown.sends.diagnostics.len(), 1);

    drop(shutdown);
    assert_eq!(replacement_drops.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn preparation_failures_and_restart_denial_obey_the_unavailable_role_policy() {
    const PREPARATIONS: [ReturnedPreparation; 5] = [
        ReturnedPreparation::Prepared,
        ReturnedPreparation::WorkerRejected,
        ReturnedPreparation::SourceRejected,
        ReturnedPreparation::InterpreterCorrupt,
        ReturnedPreparation::InterpretationSkipped,
    ];
    const POLICIES: [UnavailableRolePolicy; 2] = [
        UnavailableRolePolicy::RetireRole,
        UnavailableRolePolicy::StopPool,
    ];

    for policy in POLICIES {
        for returned in PREPARATIONS {
            let roles = OrderedRoles::new(SearchRole::Primary, [])
                .unwrap_or_else(|_| panic!("one role is a valid roster"));
            let failure = match policy {
                UnavailableRolePolicy::RetireRole => PoolFailureReaction::RetireRole,
                UnavailableRolePolicy::StopPool => PoolFailureReaction::StopPool,
            };
            let pool = keyed(
                prepare_worker,
                roles,
                |_: &Account| SearchRole::Primary,
                ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
                PoolRecovery::permanent(
                    FallibleSearchSource,
                    RestartLimit::new(1, Duration::from_secs(10)),
                    RestartRelease::immediate(),
                    failure,
                ),
                BacklogCapacity::new(1),
                BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
                Interruption::Retry,
                ActorDrainPolicy::WaitForActorGraph,
                DiagnosticDisposition::terminate(),
            )
            .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
            let initialized = pool
                .initialize()
                .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
            let mut pool = initialized.behavior;
            let creation = initialized
                .actions
                .creates
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("the declared role creates one worker"));
            let worker = creation.id();
            let created = pool
                .on(commit_worker_creation(creation))
                .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
            let initialization = created
                .sends
                .worker_initializations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("created worker awaits initialization"));
            let initialized = pool
                .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
                .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
            let activation = initialized
                .sends
                .worker_activations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("initialized worker begins activation"));
            let activation_started = pool
                .on(activation.started())
                .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
            assert!(activation_started.creates.is_empty());
            assert!(activation_started.sends.worker_preparations.is_empty());
            let ready = pool
                .on(activation.activate().await)
                .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
            assert!(ready.creates.is_empty());
            assert!(ready.sends.worker_preparations.is_empty());

            let stopped = pool
                .on(ChildStopped::new(
                    worker,
                    Err(Crash::Failed),
                    Instant::now(),
                ))
                .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
            assert!(stopped.creates.is_empty());
            let preparation = stopped
                .sends
                .worker_preparations
                .into_items()
                .pop()
                .unwrap_or_else(|| panic!("worker failure requests one preparation"));
            let input = match returned {
                ReturnedPreparation::Prepared => {
                    let ControlFlow::Break(prepared) =
                        preparation.accept(WorkerSubmission::immediate(SearchWorker))
                    else {
                        panic!("one role completes one worker preparation")
                    };
                    SettledItem::Attempted(ItemSettlement::Accepted(prepared))
                }
                ReturnedPreparation::WorkerRejected => {
                    SettledItem::Attempted(ItemSettlement::Accepted(
                        preparation.reject(SearchWorkerRejection::Unavailable),
                    ))
                }
                ReturnedPreparation::SourceRejected => {
                    SettledItem::Attempted(ItemSettlement::Rejected {
                        item: preparation,
                        reason: SearchSourceRejection::Closed,
                    })
                }
                ReturnedPreparation::InterpreterCorrupt => {
                    SettledItem::Attempted(ItemSettlement::Corrupt {
                        item: preparation,
                        fault: InterpreterFault::CorruptTraversal,
                    })
                }
                ReturnedPreparation::InterpretationSkipped => SettledItem::Unattempted(preparation),
            };
            let acted = pool
                .transition(KeyedEvent::WorkerPreparationSettled(input))
                .unwrap_or_else(|error| panic!("worker preparation input failed: {error}"));
            assert!(acted.sends.worker_initializations.is_empty());
            assert!(acted.sends.worker_activations.is_empty());
            assert!(acted.sends.customer_outcomes.is_empty());
            assert!(acted.sends.binding_replies.as_slice().is_empty());
            assert!(acted.sends.worker_assignments.is_empty());
            assert!(acted.sends.worker_preparations.is_empty());
            assert!(acted.sends.restart_schedules.is_empty());
            assert!(acted.sends.worker_shutdowns.is_empty());

            match returned {
                ReturnedPreparation::Prepared => {
                    assert!(matches!(acted.become_, Step::Continue));
                    assert_eq!(acted.creates.len(), 1);
                    assert_eq!(acted.sends.worker_observations.len(), 1);
                    assert!(acted.sends.diagnostics.is_empty());

                    let replacement =
                        acted.creates.into_iter().next().unwrap_or_else(|| {
                            panic!("the first recovery creates one replacement")
                        });
                    let replacement_worker = replacement.id();
                    let created = pool
                        .on(commit_worker_creation(replacement))
                        .unwrap_or_else(|error| panic!("replacement creation failed: {error}"));
                    let initialization = created
                        .sends
                        .worker_initializations
                        .into_requests()
                        .pop()
                        .unwrap_or_else(|| panic!("replacement awaits initialization"));
                    let initialized = pool
                        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
                        .unwrap_or_else(|error| {
                            panic!("replacement initialization failed: {error}")
                        });
                    let activation = initialized
                        .sends
                        .worker_activations
                        .into_requests()
                        .pop()
                        .unwrap_or_else(|| panic!("replacement begins activation"));
                    let started = pool.on(activation.started()).unwrap_or_else(|error| {
                        panic!("replacement activation start failed: {error}")
                    });
                    assert!(started.creates.is_empty());
                    let ready = pool
                        .on(activation.activate().await)
                        .unwrap_or_else(|error| panic!("replacement readiness failed: {error}"));
                    assert!(ready.creates.is_empty());

                    let stopped_again = pool
                        .on(ChildStopped::new(
                            replacement_worker,
                            Err(Crash::Failed),
                            Instant::now(),
                        ))
                        .unwrap_or_else(|error| {
                            panic!("replacement worker failure failed: {error}")
                        });
                    let preparation = stopped_again
                        .sends
                        .worker_preparations
                        .into_items()
                        .pop()
                        .unwrap_or_else(|| panic!("second failure requests preparation"));
                    let ControlFlow::Break(prepared) =
                        preparation.accept(WorkerSubmission::immediate(SearchWorker))
                    else {
                        panic!("one role completes the second worker preparation")
                    };
                    let denied = pool
                        .transition(KeyedEvent::WorkerPreparationSettled(
                            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
                        ))
                        .unwrap_or_else(|error| panic!("restart-limit input failed: {error}"));
                    assert!(denied.creates.is_empty());
                    assert!(denied.sends.worker_observations.is_empty());
                    assert_eq!(denied.sends.diagnostics.len(), 1);
                    match policy {
                        UnavailableRolePolicy::RetireRole => {
                            assert!(matches!(denied.become_, Step::Continue));
                        }
                        UnavailableRolePolicy::StopPool => {
                            assert!(matches!(denied.become_, Step::Stop(_)));
                        }
                    }
                }
                ReturnedPreparation::WorkerRejected
                | ReturnedPreparation::SourceRejected
                | ReturnedPreparation::InterpreterCorrupt
                | ReturnedPreparation::InterpretationSkipped => {
                    assert!(acted.creates.is_empty());
                    assert!(acted.sends.worker_observations.is_empty());
                    assert_eq!(acted.sends.diagnostics.len(), 1);
                    match policy {
                        UnavailableRolePolicy::RetireRole => {
                            assert!(matches!(acted.become_, Step::Continue));
                        }
                        UnavailableRolePolicy::StopPool => {
                            assert!(matches!(acted.become_, Step::Stop(_)));
                        }
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn shutdown_retains_an_inflight_worker_preparation_until_it_returns() {
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the declared role creates one worker"));
    let worker = creation.id();
    let created = pool
        .on(commit_worker_creation(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let initialized = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
    assert!(activation_started.creates.is_empty());
    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(ready.creates.is_empty());

    let stopped = pool
        .on(ChildStopped::new(
            worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
    let preparation = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("worker failure requests one preparation"));
    let shutdown = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    assert!(matches!(shutdown.become_, Step::Continue));
    assert!(shutdown.creates.is_empty());
    assert!(shutdown.sends.worker_preparations.is_empty());

    let foreign_roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let foreign_pool = keyed(
        prepare_worker,
        foreign_roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the foreign worker declaration constructs a keyed pool"));
    let foreign_initialized = foreign_pool
        .initialize()
        .unwrap_or_else(|error| panic!("foreign keyed initialization failed: {error}"));
    let mut foreign_pool = foreign_initialized.behavior;
    let foreign_creation = foreign_initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the foreign role creates one worker"));
    let foreign_worker = foreign_creation.id();
    let foreign_created = foreign_pool
        .on(commit_worker_creation(foreign_creation))
        .unwrap_or_else(|error| panic!("foreign worker creation failed: {error}"));
    let foreign_initialization = foreign_created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("foreign worker awaits initialization"));
    let foreign_initialized = foreign_pool
        .on(foreign_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("foreign worker initialization failed: {error}"));
    let foreign_activation = foreign_initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("foreign worker begins activation"));
    let foreign_started = foreign_pool
        .on(foreign_activation.started())
        .unwrap_or_else(|error| panic!("foreign activation start failed: {error}"));
    assert!(foreign_started.creates.is_empty());
    let foreign_ready = foreign_pool
        .on(foreign_activation.activate().await)
        .unwrap_or_else(|error| panic!("foreign worker readiness failed: {error}"));
    assert!(foreign_ready.creates.is_empty());
    let foreign_stopped = foreign_pool
        .on(ChildStopped::new(
            foreign_worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("foreign worker failure failed: {error}"));
    let foreign_preparation = foreign_stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("foreign failure requests one preparation"));

    let unrelated = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Unattempted(foreign_preparation),
        ))
        .unwrap_or_else(|error| panic!("foreign retired preparation failed: {error}"));
    assert!(matches!(unrelated.become_, Step::Continue));
    assert!(unrelated.creates.is_empty());
    assert_eq!(unrelated.sends.diagnostics.len(), 1);

    let ControlFlow::Break(prepared) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one role completes one worker preparation")
    };
    let returned = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
        ))
        .unwrap_or_else(|error| panic!("retired preparation input failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert!(returned.creates.is_empty());
    assert!(returned.sends.worker_observations.is_empty());
    assert!(returned.sends.worker_initializations.is_empty());
    assert!(returned.sends.worker_activations.is_empty());
    assert!(returned.sends.customer_outcomes.is_empty());
    assert!(returned.sends.binding_replies.as_slice().is_empty());
    assert!(returned.sends.worker_assignments.is_empty());
    assert!(returned.sends.worker_preparations.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());
    assert!(returned.sends.worker_shutdowns.is_empty());
    assert_eq!(returned.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn rejected_restart_schedule_retires_the_role_and_returns_its_queue() {
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive release delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the declared role creates one worker"));
    let worker = creation.id();
    let created = pool
        .on(commit_worker_creation(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let initialized = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
    assert!(activation_started.creates.is_empty());
    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(ready.creates.is_empty());

    let stopped = pool
        .on(ChildStopped::new(
            worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
    let preparation = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("worker failure requests one preparation"));
    let ControlFlow::Break(prepared) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one role completes one worker preparation")
    };
    let scheduling = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
        ))
        .unwrap_or_else(|error| panic!("worker preparation failed: {error}"));
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));

    let queued = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(44),
                Account(8),
                SearchJob(31),
                Recipient::global(RuntimeAddr(90)),
            ),
        )
        .unwrap_or_else(|error| panic!("recovering role rejected queued work: {error}"));
    assert!(queued.sends.worker_assignments.is_empty());
    assert_eq!(queued.sends.customer_outcomes.len(), 1);

    let rejected = pool
        .transition(KeyedEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: schedule,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("restart schedule rejection failed: {error}"));
    assert!(matches!(rejected.become_, Step::Continue));
    assert!(rejected.creates.is_empty());
    assert!(rejected.sends.worker_observations.is_empty());
    assert!(rejected.sends.worker_initializations.is_empty());
    assert!(rejected.sends.worker_activations.is_empty());
    assert!(rejected.sends.binding_replies.as_slice().is_empty());
    assert!(rejected.sends.worker_assignments.is_empty());
    assert!(rejected.sends.worker_preparations.is_empty());
    assert!(rejected.sends.restart_schedules.is_empty());
    assert!(rejected.sends.worker_shutdowns.is_empty());
    assert_eq!(rejected.sends.diagnostics.len(), 2);
    let returned = rejected
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("retired role returns its queued customer"));
    match returned {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::ReturnedQueued {
                binding,
                payload,
                reason: KeyedQueuedReturnReason::RolePermanentlyUnavailable,
                ..
            } => {
                assert_eq!(binding.role(), &SearchRole::Primary);
                assert_eq!(payload, SearchJob(31));
            }
            _ => panic!("role retirement returns queued work"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the queued customer route is logical")
        }
    }
}

#[tokio::test]
async fn shutdown_retains_an_inflight_restart_schedule_until_it_returns() {
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive release delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the declared role creates one worker"));
    let worker = creation.id();
    let created = pool
        .on(commit_worker_creation(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let initialized = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
    assert!(activation_started.creates.is_empty());
    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(ready.creates.is_empty());

    let stopped = pool
        .on(ChildStopped::new(
            worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
    let preparation = stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("worker failure requests one preparation"));
    let ControlFlow::Break(prepared) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one role completes one worker preparation")
    };
    let scheduling = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
        ))
        .unwrap_or_else(|error| panic!("worker preparation failed: {error}"));
    let schedule = scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("delayed recovery requests one timer"));

    let shutdown = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    assert!(matches!(shutdown.become_, Step::Continue));
    assert!(shutdown.creates.is_empty());
    assert!(shutdown.sends.restart_schedules.is_empty());

    let returned = pool
        .transition(KeyedEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: schedule.id,
                generation: schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("retired restart schedule input failed: {error}"));
    assert!(matches!(returned.become_, Step::Stop(_)));
    assert!(returned.creates.is_empty());
    assert!(returned.sends.worker_observations.is_empty());
    assert!(returned.sends.worker_initializations.is_empty());
    assert!(returned.sends.worker_activations.is_empty());
    assert!(returned.sends.customer_outcomes.is_empty());
    assert!(returned.sends.binding_replies.as_slice().is_empty());
    assert!(returned.sends.worker_assignments.is_empty());
    assert!(returned.sends.worker_preparations.is_empty());
    assert!(returned.sends.restart_schedules.is_empty());
    assert!(returned.sends.worker_shutdowns.is_empty());
    assert_eq!(returned.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn ready_worker_drains_only_its_role_queue() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |account: &Account| match account.0 % 2 {
            0 => SearchRole::Primary,
            _ => SearchRole::Replica,
        },
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(4),
        BindingCapacity::new(8).unwrap_or_else(|_| panic!("eight bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let primary_creation = creations.remove(0);
    let replica_creation = creations.remove(0);
    let primary_worker = primary_creation.id();
    let replica_worker = replica_creation.id();
    let customer = Recipient::<
        MessageProtocol<RuntimeAddr, KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>>,
    >::global(RuntimeAddr(90));

    let queued = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(SubmissionId::new(1), Account(2), SearchJob(9), customer),
        )
        .unwrap_or_else(|error| panic!("primary submission failed: {error}"));
    assert!(queued.sends.worker_assignments.is_empty());

    let mut foreign_creations = CreationSequence::new();
    for _ in 0..2 {
        foreign_creations
            .issue()
            .unwrap_or_else(|| panic!("foreign creation sequence remains available"));
    }
    let foreign_creation = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("third foreign creation remains available"));
    let foreign_creation = CreateChild::birth(foreign_creation, StopOnShutdown::new(SearchWorker));
    let rejected_creation = pool
        .on(commit_worker_creation(foreign_creation))
        .unwrap_or_else(|error| panic!("foreign creation input failed: {error}"));
    assert_eq!(rejected_creation.sends.diagnostics.len(), 1);
    assert!(rejected_creation.sends.worker_initializations.is_empty());

    let wrong_kind_creation = worker_creation_settlement(
        replica_creation.id(),
        CreationKind::replacement(primary_creation.id()),
    );
    let rejected_kind = pool
        .on(wrong_kind_creation)
        .unwrap_or_else(|error| panic!("wrong-kind creation input failed: {error}"));
    assert_eq!(rejected_kind.sends.diagnostics.len(), 1);
    assert!(rejected_kind.sends.worker_initializations.is_empty());

    let replica_created = pool
        .on(commit_worker_creation(replica_creation))
        .unwrap_or_else(|error| panic!("replica creation failed: {error}"));
    let replica_initialization = replica_created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created replica awaits initialization"));

    let foreign_roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct foreign roles are a valid roster"));
    let foreign_pool = keyed(
        prepare_worker,
        foreign_roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(4),
        BindingCapacity::new(8).unwrap_or_else(|_| panic!("eight bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the foreign worker declaration constructs a keyed pool"));
    let foreign_initialized = foreign_pool
        .initialize()
        .unwrap_or_else(|error| panic!("foreign keyed initialization failed: {error}"));
    let mut foreign_pool = foreign_initialized.behavior;
    let mut foreign_workers: Vec<_> = foreign_initialized.actions.creates.into_iter().collect();
    let foreign_primary = foreign_pool
        .on(commit_worker_creation(foreign_workers.remove(0)))
        .unwrap_or_else(|error| panic!("foreign primary creation failed: {error}"));
    let foreign_initialization = foreign_primary
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("foreign primary awaits initialization"));
    let rejected_initialization = pool
        .on(foreign_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("foreign initialization input failed: {error}"));
    assert_eq!(rejected_initialization.sends.diagnostics.len(), 1);
    assert!(rejected_initialization.sends.worker_activations.is_empty());

    let foreign_replica = foreign_pool
        .on(commit_worker_creation(foreign_workers.remove(0)))
        .unwrap_or_else(|error| panic!("foreign replica creation failed: {error}"));
    let foreign_initialization = foreign_replica
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("foreign replica awaits initialization"));
    let foreign_replica = foreign_pool
        .on(foreign_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("foreign replica initialization failed: {error}"));
    let foreign_activation = foreign_replica
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("foreign replica begins activation"));
    let rejected_activation = pool
        .on(foreign_activation.started())
        .unwrap_or_else(|error| panic!("foreign activation input failed: {error}"));
    assert_eq!(rejected_activation.sends.diagnostics.len(), 1);
    assert!(rejected_activation.sends.worker_assignments.is_empty());

    let replica_initialized = pool
        .on(replica_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("replica initialization failed: {error}"));
    let replica_activation = replica_initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized replica begins activation"));
    let duplicate_start = replica_activation.started();
    let replica_started = pool
        .on(replica_activation.started())
        .unwrap_or_else(|error| panic!("replica activation start failed: {error}"));
    assert!(replica_started.sends.worker_assignments.is_empty());
    let duplicate_start = pool
        .on(duplicate_start)
        .unwrap_or_else(|error| panic!("duplicate activation input failed: {error}"));
    assert_eq!(duplicate_start.sends.diagnostics.len(), 1);
    assert!(duplicate_start.sends.worker_assignments.is_empty());
    let replica_ready = pool
        .on(replica_activation.activate().await)
        .unwrap_or_else(|error| panic!("replica readiness failed: {error}"));
    assert!(replica_ready.sends.worker_assignments.is_empty());

    let primary_created = pool
        .on(commit_worker_creation(primary_creation))
        .unwrap_or_else(|error| panic!("primary creation failed: {error}"));
    let primary_initialization = primary_created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created primary awaits initialization"));
    let primary_initialized = pool
        .on(primary_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("primary initialization failed: {error}"));
    let primary_activation = primary_initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized primary begins activation"));
    let primary_started = pool
        .on(primary_activation.started())
        .unwrap_or_else(|error| panic!("primary activation start failed: {error}"));
    assert!(primary_started.sends.worker_assignments.is_empty());
    let primary_ready = pool
        .on(primary_activation.activate().await)
        .unwrap_or_else(|error| panic!("primary readiness failed: {error}"));
    let queued_assignment = primary_ready
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("ready primary receives its queued work"));
    let (_, queued_assignment, queued_receipt) = queued_assignment.into_parts();
    assert_eq!(queued_assignment.payload(), &SearchJob(9));

    let completion_before_receipt = pool
        .on(behavior::ChildReport::new(
            primary_worker,
            queued_assignment.complete(SearchResult(109)).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("completion-before-receipt failed: {error}"));
    assert!(completion_before_receipt.sends.customer_outcomes.is_empty());
    assert!(
        completion_before_receipt
            .sends
            .worker_assignments
            .is_empty()
    );

    let accepted: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Accepted(queued_receipt));
    let completion_released = pool
        .transition(KeyedEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("assignment receipt failed: {error}"));
    let queued_outcome = completion_released
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("receipt releases the retained completion"));
    match queued_outcome {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::Completed {
                job: _,
                binding,
                worker_result,
            } => {
                assert_eq!(binding.role(), &SearchRole::Primary);
                assert_eq!(worker_result, SearchResult(109));
            }
            _ => panic!("receipt releases one completed keyed job"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the completed job retains its logical customer")
        }
    }

    let followup_assigned = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(2),
                Account(2),
                SearchJob(10),
                Recipient::global(RuntimeAddr(90)),
            ),
        )
        .unwrap_or_else(|error| panic!("follow-up primary submission failed: {error}"));
    let followup_assignment = followup_assigned
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle primary receives the follow-up job"));
    let (_, followup_assignment, followup_receipt) = followup_assignment.into_parts();
    let accepted: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Accepted(followup_receipt));
    let receipt_before_completion = pool
        .transition(KeyedEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("follow-up assignment receipt failed: {error}"));
    assert!(receipt_before_completion.sends.customer_outcomes.is_empty());

    let followup_completed = pool
        .on(behavior::ChildReport::new(
            primary_worker,
            followup_assignment.complete(SearchResult(110)).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("completion-after-receipt failed: {error}"));
    let followup_outcome = followup_completed
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("completion resolves the follow-up customer"));
    match followup_outcome {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::Completed {
                job: _,
                binding,
                worker_result,
            } => {
                assert_eq!(binding.role(), &SearchRole::Primary);
                assert_eq!(worker_result, SearchResult(110));
            }
            _ => panic!("completion resolves one keyed job"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the follow-up job retains its logical customer")
        }
    }

    let replica_assigned = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(3),
                Account(3),
                SearchJob(20),
                Recipient::global(RuntimeAddr(90)),
            ),
        )
        .unwrap_or_else(|error| panic!("replica submission failed: {error}"));
    let replica_assignment = replica_assigned
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle replica receives its job"));
    let (_, replica_assignment, replica_receipt) = replica_assignment.into_parts();
    let accepted: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Accepted(replica_receipt));
    let accepted = pool
        .transition(KeyedEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("replica assignment receipt failed: {error}"));
    assert!(accepted.sends.customer_outcomes.is_empty());

    let interrupted_assigned = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(4),
                Account(2),
                SearchJob(11),
                Recipient::global(RuntimeAddr(90)),
            ),
        )
        .unwrap_or_else(|error| panic!("interrupted primary submission failed: {error}"));
    let interrupted_assignment = interrupted_assigned
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle primary receives the interrupted job"));
    let (_, _, interrupted_receipt) = interrupted_assignment.into_parts();
    let accepted: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Accepted(interrupted_receipt));
    let accepted = pool
        .transition(KeyedEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("interrupted assignment receipt failed: {error}"));
    assert!(accepted.sends.customer_outcomes.is_empty());

    let foreign_completion = pool
        .on(behavior::ChildReport::new(
            primary_worker,
            replica_assignment.complete(SearchResult(120)).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("foreign completion failed: {error}"));
    assert!(foreign_completion.sends.customer_outcomes.is_empty());
    assert_eq!(foreign_completion.sends.diagnostics.len(), 1);

    let primary_stopped = pool
        .on(ChildStopped::new(
            primary_worker,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("primary worker failure failed: {error}"));
    let returned = primary_stopped
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("retiring the role returns its assigned job"));
    match returned {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::ReturnedAssigned {
                binding,
                payload,
                reason,
                ..
            } => {
                assert_eq!(binding.role(), &SearchRole::Primary);
                assert_eq!(payload, SearchJob(11));
                assert_eq!(
                    reason,
                    KeyedAssignedReturnReason::RolePermanentlyUnavailable
                );
            }
            _ => panic!("retiring the role returns the interrupted assignment"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the interrupted job retains its logical customer")
        }
    }
    assert_eq!(primary_stopped.sends.diagnostics.len(), 1);

    let draining = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    let returned = draining
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown returns the replica assignment"));
    match returned {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::ReturnedAssigned {
                binding,
                payload,
                reason,
                ..
            } => {
                assert_eq!(binding.role(), &SearchRole::Replica);
                assert_eq!(payload, SearchJob(20));
                assert_eq!(reason, KeyedAssignedReturnReason::PoolShutdown);
            }
            _ => panic!("shutdown returns the replica assignment"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the replica job retains its logical customer")
        }
    }
    let shutdowns = draining.sends.worker_shutdowns.into_requests();
    assert_eq!(shutdowns.len(), 1);
    assert_eq!(shutdowns[0].id, ShutdownId(0));
    assert!(matches!(draining.become_, Step::Continue));

    for shutdown in shutdowns {
        let settled = pool
            .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
                shutdown.id,
            ))
            .unwrap_or_else(|error| panic!("worker shutdown settlement failed: {error}"));
        assert!(matches!(settled.become_, Step::Continue));
    }
    let final_exit = pool
        .on(ChildStopped::new(
            replica_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("replica worker exit failed: {error}"));
    assert!(matches!(final_exit.become_, Step::Stop(_)));
}

#[tokio::test]
async fn shutdown_retains_a_worker_waiting_for_activation_capacity() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let prepared_drops = Arc::clone(&activation_drops);
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        move |_: &SearchRole| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                HeldActivation(Arc::clone(&prepared_drops)),
            ))
        },
        roles,
        |account: &Account| match account.0 % 2 {
            0 => SearchRole::Primary,
            _ => SearchRole::Replica,
        },
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        BindingCapacity::new(2).unwrap_or_else(|_| panic!("two bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let primary_creation = creations.remove(0);
    let replica_creation = creations.remove(0);
    let primary_worker = primary_creation.id();
    let replica_worker = replica_creation.id();

    let primary_created = pool
        .on(commit_worker_creation(primary_creation))
        .unwrap_or_else(|error| panic!("primary creation failed: {error}"));
    let primary_initialization = primary_created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the primary worker awaits initialization"));
    let primary_initialized = pool
        .on(primary_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("primary initialization failed: {error}"));
    let primary_activation = primary_initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the primary worker occupies activation capacity"));

    let replica_created = pool
        .on(commit_worker_creation(replica_creation))
        .unwrap_or_else(|error| panic!("replica creation failed: {error}"));
    let replica_initialization = replica_created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the replica worker awaits initialization"));
    let replica_waiting = pool
        .on(replica_initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("replica initialization failed: {error}"));
    assert!(replica_waiting.sends.worker_activations.is_empty());
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);

    let retiring = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    let shutdowns = retiring.sends.worker_shutdowns.into_requests();
    assert_eq!(shutdowns.len(), 2);
    assert!(retiring.sends.worker_activations.is_empty());
    assert!(retiring.sends.worker_preparations.is_empty());
    assert!(retiring.creates.is_empty());
    assert!(matches!(retiring.become_, Step::Continue));
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);

    let started = pool
        .on(primary_activation.started())
        .unwrap_or_else(|error| panic!("primary activation start failed: {error}"));
    assert!(started.sends.worker_activations.is_empty());
    let ready = pool
        .on(primary_activation.activate().await)
        .unwrap_or_else(|error| panic!("primary activation completion failed: {error}"));
    assert!(ready.sends.worker_activations.is_empty());
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);

    for shutdown in shutdowns {
        let settled = pool
            .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
                shutdown.id,
            ))
            .unwrap_or_else(|error| panic!("worker shutdown settlement failed: {error}"));
        assert!(settled.sends.worker_activations.is_empty());
        assert!(matches!(settled.become_, Step::Continue));
    }
    let primary_stopped = pool
        .on(ChildStopped::new(
            primary_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("primary exit failed: {error}"));
    assert!(matches!(primary_stopped.become_, Step::Continue));
    let replica_stopped = pool
        .on(ChildStopped::new(
            replica_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("replica exit failed: {error}"));
    assert!(replica_stopped.sends.worker_activations.is_empty());
    assert_eq!(replica_stopped.sends.diagnostics.len(), 1);
    assert!(matches!(replica_stopped.become_, Step::Stop(_)));
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
    drop(pool);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
    drop(replica_stopped);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn shutdown_cancels_a_worker_waiting_for_the_recovery_source() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |account: &Account| match account.0 % 2 {
            0 => SearchRole::Primary,
            _ => SearchRole::Replica,
        },
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::permanent(
            SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(1),
        BindingCapacity::new(2).unwrap_or_else(|_| panic!("two bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let workers: Vec<_> = creations.iter().map(|creation| creation.id()).collect();

    for creation in creations {
        let created = pool
            .on(commit_worker_creation(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = created
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("the created worker awaits initialization"));
        let initialized = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("the initialized worker begins activation"));
        let started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
        assert!(started.creates.is_empty());
        assert!(started.sends.worker_preparations.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.creates.is_empty());
        assert!(ready.sends.worker_preparations.is_empty());
    }

    let primary_stopped = pool
        .on(ChildStopped::new(
            workers[0],
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("primary worker failure failed: {error}"));
    let preparation = primary_stopped
        .sends
        .worker_preparations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the primary failure claims the recovery source"));
    let replica_stopped = pool
        .on(ChildStopped::new(
            workers[1],
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("replica worker failure failed: {error}"));
    assert!(replica_stopped.sends.worker_preparations.is_empty());

    let retiring = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    assert!(retiring.sends.worker_preparations.is_empty());
    assert!(retiring.sends.worker_shutdowns.is_empty());
    assert!(retiring.creates.is_empty());
    assert!(matches!(retiring.become_, Step::Continue));

    let ControlFlow::Break(prepared) =
        preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes one worker preparation")
    };
    let returned = pool
        .transition(KeyedEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
        ))
        .unwrap_or_else(|error| panic!("retired worker preparation failed: {error}"));
    assert!(returned.sends.worker_preparations.is_empty());
    assert!(returned.sends.worker_activations.is_empty());
    assert!(returned.sends.worker_shutdowns.is_empty());
    assert!(returned.creates.is_empty());
    assert!(matches!(returned.become_, Step::Stop(_)));
}

#[tokio::test]
async fn shutdown_keeps_pending_workers_until_the_runtime_returns_them() {
    let roles = OrderedRoles::new(SearchRole::Primary, [SearchRole::Replica])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |account: &Account| match account.0 % 2 {
            0 => SearchRole::Primary,
            _ => SearchRole::Replica,
        },
        ActivationPolicy::new(2).unwrap_or_else(|_| panic!("two activations are valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(4),
        BindingCapacity::new(8).unwrap_or_else(|_| panic!("eight bindings are valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let mut creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let primary_creation = creations.remove(0);
    let replica_creation = creations.remove(0);
    let primary_worker = primary_creation.id();
    let replica_worker = replica_creation.id();

    let retiring = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    assert!(retiring.sends.worker_shutdowns.is_empty());
    assert!(matches!(retiring.become_, Step::Continue));

    let stopped_before_establishment = pool
        .on(ChildStopped::new(
            primary_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("pending primary exit failed: {error}"));
    assert!(
        stopped_before_establishment
            .sends
            .worker_shutdowns
            .is_empty()
    );
    assert!(matches!(
        stopped_before_establishment.become_,
        Step::Continue
    ));

    let returned_primary = pool
        .on(commit_worker_creation(primary_creation))
        .unwrap_or_else(|error| panic!("primary worker return failed: {error}"));
    assert!(returned_primary.sends.worker_shutdowns.is_empty());
    assert_eq!(returned_primary.sends.diagnostics.len(), 1);
    assert!(matches!(returned_primary.become_, Step::Continue));

    let returned_replica = pool
        .on(commit_worker_creation(replica_creation))
        .unwrap_or_else(|error| panic!("replica worker return failed: {error}"));
    let shutdown = returned_replica
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the running replica receives one shutdown request"));
    assert!(matches!(returned_replica.become_, Step::Continue));

    let accepted = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("replica shutdown settlement failed: {error}"));
    assert!(matches!(accepted.become_, Step::Continue));
    let stopped = pool
        .on(ChildStopped::new(
            replica_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("replica exit failed: {error}"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn initialization_stop_completes_the_worker_shutdown_join() {
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the primary worker creation is emitted"));
    let worker = creation.id();
    let created = pool
        .on(commit_worker_creation(creation))
        .unwrap_or_else(|error| panic!("primary creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the created worker awaits initialization"));

    let retiring = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    let shutdown = retiring
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the initializing worker receives one shutdown"));
    let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());
    let initialization_returned = pool
        .on(initialization.resolve(WorkerInitializationOutcome::Stopped(stopped)))
        .unwrap_or_else(|error| panic!("stopped initialization failed: {error}"));
    assert_eq!(initialization_returned.sends.diagnostics.len(), 1);
    assert!(matches!(initialization_returned.become_, Step::Continue));

    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("worker shutdown settlement failed: {error}"));
    assert!(matches!(settled.become_, Step::Stop(_)));
}

#[tokio::test]
async fn activation_start_remains_a_receipt_during_shutdown() {
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        prepare_worker,
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(1),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the primary worker creation is emitted"));
    let worker = creation.id();
    let created = pool
        .on(commit_worker_creation(creation))
        .unwrap_or_else(|error| panic!("primary creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the created worker awaits initialization"));
    let initialized = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the initialized worker begins activation"));

    let retiring = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    let shutdown = retiring
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the activating worker receives one shutdown"));
    let started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(started.sends.diagnostics.is_empty());
    assert!(matches!(started.become_, Step::Continue));

    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("activation completion failed: {error}"));
    assert_eq!(ready.sends.diagnostics.len(), 1);
    assert!(matches!(ready.become_, Step::Continue));
    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("worker shutdown settlement failed: {error}"));
    assert!(matches!(settled.become_, Step::Continue));
    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker exit failed: {error}"));
    assert!(matches!(stopped.become_, Step::Stop(_)));
}

#[tokio::test]
async fn shutdown_retains_every_failed_worker_preparation_return() {
    const DISPOSITIONS: [ReturnedPreparation; 4] = [
        ReturnedPreparation::WorkerRejected,
        ReturnedPreparation::SourceRejected,
        ReturnedPreparation::InterpreterCorrupt,
        ReturnedPreparation::InterpretationSkipped,
    ];

    for disposition in DISPOSITIONS {
        let roles = OrderedRoles::new(SearchRole::Primary, [])
            .unwrap_or_else(|_| panic!("one role is a valid roster"));
        let pool = keyed(
            prepare_worker,
            roles,
            |_: &Account| SearchRole::Primary,
            ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
            PoolRecovery::permanent(
                FallibleSearchSource,
                RestartLimit::new(3, Duration::from_secs(10)),
                RestartRelease::immediate(),
                PoolFailureReaction::RetireRole,
            ),
            BacklogCapacity::new(1),
            BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
            Interruption::Retry,
            ActorDrainPolicy::WaitForActorGraph,
            DiagnosticDisposition::terminate(),
        )
        .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
        let initialized = pool
            .initialize()
            .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
        let mut pool = initialized.behavior;
        let creation = initialized
            .actions
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the declared role creates one worker"));
        let worker = creation.id();
        let created = pool
            .on(commit_worker_creation(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = created
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("created worker awaits initialization"));
        let initialized = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("initialized worker begins activation"));
        let started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
        assert!(started.creates.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.creates.is_empty());

        let stopped = pool
            .on(ChildStopped::new(
                worker,
                Err(Crash::Failed),
                Instant::now(),
            ))
            .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
        let preparation = stopped
            .sends
            .worker_preparations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("worker failure requests one preparation"));
        let retiring = pool
            .receive(RuntimeAddr(7), KeyedCommand::shutdown())
            .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
        assert!(matches!(retiring.become_, Step::Continue));
        assert!(retiring.creates.is_empty());

        let returned = match disposition {
            ReturnedPreparation::WorkerRejected => SettledItem::Attempted(
                ItemSettlement::Accepted(preparation.reject(SearchWorkerRejection::Unavailable)),
            ),
            ReturnedPreparation::SourceRejected => {
                SettledItem::Attempted(ItemSettlement::Rejected {
                    item: preparation,
                    reason: SearchSourceRejection::Closed,
                })
            }
            ReturnedPreparation::InterpreterCorrupt => {
                SettledItem::Attempted(ItemSettlement::Corrupt {
                    item: preparation,
                    fault: InterpreterFault::CorruptTraversal,
                })
            }
            ReturnedPreparation::InterpretationSkipped => SettledItem::Unattempted(preparation),
            ReturnedPreparation::Prepared => {
                panic!("accepted preparation has its own shutdown witness")
            }
        };
        let terminal = pool
            .transition(KeyedEvent::WorkerPreparationSettled(returned))
            .unwrap_or_else(|error| panic!("retired preparation input failed: {error}"));
        assert!(matches!(terminal.become_, Step::Stop(_)));
        assert!(terminal.creates.is_empty());
        assert!(terminal.sends.worker_observations.is_empty());
        assert!(terminal.sends.worker_initializations.is_empty());
        assert!(terminal.sends.worker_activations.is_empty());
        assert!(terminal.sends.customer_outcomes.is_empty());
        assert!(terminal.sends.binding_replies.as_slice().is_empty());
        assert!(terminal.sends.worker_assignments.is_empty());
        assert!(terminal.sends.worker_preparations.is_empty());
        assert!(terminal.sends.restart_schedules.is_empty());
        assert!(terminal.sends.worker_shutdowns.is_empty());
        assert_eq!(terminal.sends.diagnostics.len(), 1);
    }
}

#[tokio::test]
async fn shutdown_retains_every_failed_restart_schedule_return() {
    const DISPOSITIONS: [RestartScheduleDisposition; 3] = [
        RestartScheduleDisposition::Rejected,
        RestartScheduleDisposition::InterpreterCorrupt,
        RestartScheduleDisposition::InterpretationSkipped,
    ];

    for disposition in DISPOSITIONS {
        let roles = OrderedRoles::new(SearchRole::Primary, [])
            .unwrap_or_else(|_| panic!("one role is a valid roster"));
        let pool = keyed(
            prepare_worker,
            roles,
            |_: &Account| SearchRole::Primary,
            ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
            PoolRecovery::permanent(
                SearchSource,
                RestartLimit::new(3, Duration::from_secs(10)),
                RestartRelease::constant(Duration::from_secs(1))
                    .unwrap_or_else(|error| panic!("positive release delay rejected: {error}")),
                PoolFailureReaction::RetireRole,
            ),
            BacklogCapacity::new(1),
            BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
            Interruption::Retry,
            ActorDrainPolicy::WaitForActorGraph,
            DiagnosticDisposition::terminate(),
        )
        .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
        let initialized = pool
            .initialize()
            .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
        let mut pool = initialized.behavior;
        let creation = initialized
            .actions
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the declared role creates one worker"));
        let worker = creation.id();
        let created = pool
            .on(commit_worker_creation(creation))
            .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
        let initialization = created
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("created worker awaits initialization"));
        let initialized = pool
            .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
        let activation = initialized
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap_or_else(|| panic!("initialized worker begins activation"));
        let started = pool
            .on(activation.started())
            .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
        assert!(started.creates.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.creates.is_empty());

        let stopped = pool
            .on(ChildStopped::new(
                worker,
                Err(Crash::Failed),
                Instant::now(),
            ))
            .unwrap_or_else(|error| panic!("worker failure failed: {error}"));
        let preparation = stopped
            .sends
            .worker_preparations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("worker failure requests one preparation"));
        let ControlFlow::Break(prepared) =
            preparation.accept(WorkerSubmission::immediate(SearchWorker))
        else {
            panic!("one role completes one worker preparation")
        };
        let scheduling = pool
            .transition(KeyedEvent::WorkerPreparationSettled(
                SettledItem::Attempted(ItemSettlement::Accepted(prepared)),
            ))
            .unwrap_or_else(|error| panic!("worker preparation failed: {error}"));
        let schedule = scheduling
            .sends
            .restart_schedules
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("delayed recovery requests one timer"));
        let retiring = pool
            .receive(RuntimeAddr(7), KeyedCommand::shutdown())
            .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
        assert!(matches!(retiring.become_, Step::Continue));
        assert!(retiring.creates.is_empty());

        let returned = match disposition {
            RestartScheduleDisposition::Rejected => {
                SettledItem::Attempted(ItemSettlement::Rejected {
                    item: schedule,
                    reason: ScheduleAfterRejection::DeadlineOverflow,
                })
            }
            RestartScheduleDisposition::InterpreterCorrupt => {
                SettledItem::Attempted(ItemSettlement::Corrupt {
                    item: schedule,
                    fault: InterpreterFault::CorruptTraversal,
                })
            }
            RestartScheduleDisposition::InterpretationSkipped => SettledItem::Unattempted(schedule),
        };
        let terminal = pool
            .transition(KeyedEvent::RestartScheduleSettled(returned))
            .unwrap_or_else(|error| panic!("retired restart schedule input failed: {error}"));
        assert!(matches!(terminal.become_, Step::Stop(_)));
        assert!(terminal.creates.is_empty());
        assert!(terminal.sends.worker_observations.is_empty());
        assert!(terminal.sends.worker_initializations.is_empty());
        assert!(terminal.sends.worker_activations.is_empty());
        assert!(terminal.sends.customer_outcomes.is_empty());
        assert!(terminal.sends.binding_replies.as_slice().is_empty());
        assert!(terminal.sends.worker_assignments.is_empty());
        assert!(terminal.sends.worker_preparations.is_empty());
        assert!(terminal.sends.restart_schedules.is_empty());
        assert!(terminal.sends.worker_shutdowns.is_empty());
        assert_eq!(terminal.sends.diagnostics.len(), 1);
    }
}

#[tokio::test]
async fn shutdown_settlement_and_worker_exit_are_order_independent() {
    const DISPOSITIONS: [ShutdownDisposition; 2] =
        [ShutdownDisposition::Accepted, ShutdownDisposition::Rejected];
    const ORDERS: [ShutdownJoinOrder; 2] = [
        ShutdownJoinOrder::SettlementThenExit,
        ShutdownJoinOrder::ExitThenSettlement,
    ];

    for disposition in DISPOSITIONS {
        for order in ORDERS {
            let roles = OrderedRoles::new(SearchRole::Primary, [])
                .unwrap_or_else(|_| panic!("one role is a valid roster"));
            let pool = keyed(
                prepare_worker,
                roles,
                |_: &Account| SearchRole::Primary,
                ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
                PoolRecovery::temporary(PoolFailureReaction::RetireRole),
                BacklogCapacity::new(1),
                BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
                Interruption::Retry,
                ActorDrainPolicy::WaitForActorGraph,
                DiagnosticDisposition::terminate(),
            )
            .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
            let initialized = pool
                .initialize()
                .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
            let mut pool = initialized.behavior;
            let creation = initialized
                .actions
                .creates
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("the declared role creates one worker"));
            let worker = creation.id();
            let created = pool
                .on(commit_worker_creation(creation))
                .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
            let initialization = created
                .sends
                .worker_initializations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("created worker awaits initialization"));
            let initialized = pool
                .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
                .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
            let activation = initialized
                .sends
                .worker_activations
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("initialized worker begins activation"));
            let started = pool
                .on(activation.started())
                .unwrap_or_else(|error| panic!("worker activation start failed: {error}"));
            assert!(started.creates.is_empty());
            let ready = pool
                .on(activation.activate().await)
                .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
            assert!(ready.creates.is_empty());

            let retiring = pool
                .receive(RuntimeAddr(7), KeyedCommand::shutdown())
                .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
            let shutdown = retiring
                .sends
                .worker_shutdowns
                .into_requests()
                .pop()
                .unwrap_or_else(|| panic!("ready worker receives one shutdown"));
            let settlement = match disposition {
                ShutdownDisposition::Accepted => {
                    EstablishedShutdownResolved::<SearchWorker>::accepted(shutdown.id)
                }
                ShutdownDisposition::Rejected => {
                    EstablishedShutdownResolved::<SearchWorker>::rejected(
                        shutdown.id,
                        ShutdownRejection::AlreadyStopping,
                    )
                }
            };
            let stopped = ChildStopped::new(worker, Ok(Exit::Normal), Instant::now());

            let terminal = match order {
                ShutdownJoinOrder::SettlementThenExit => {
                    let awaiting_exit = pool
                        .on(settlement)
                        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"));
                    assert!(matches!(awaiting_exit.become_, Step::Continue));
                    assert!(awaiting_exit.sends.diagnostics.is_empty());
                    pool.on(stopped)
                        .unwrap_or_else(|error| panic!("worker exit failed: {error}"))
                }
                ShutdownJoinOrder::ExitThenSettlement => {
                    let awaiting_settlement = pool
                        .on(stopped)
                        .unwrap_or_else(|error| panic!("worker exit failed: {error}"));
                    assert!(matches!(awaiting_settlement.become_, Step::Continue));
                    assert!(awaiting_settlement.sends.diagnostics.is_empty());
                    pool.on(settlement)
                        .unwrap_or_else(|error| panic!("shutdown settlement failed: {error}"))
                }
            };
            assert!(matches!(terminal.become_, Step::Stop(_)));
            assert!(terminal.creates.is_empty());
            assert!(terminal.sends.worker_observations.is_empty());
            assert!(terminal.sends.worker_initializations.is_empty());
            assert!(terminal.sends.worker_activations.is_empty());
            assert!(terminal.sends.customer_outcomes.is_empty());
            assert!(terminal.sends.binding_replies.as_slice().is_empty());
            assert!(terminal.sends.worker_assignments.is_empty());
            assert!(terminal.sends.worker_preparations.is_empty());
            assert!(terminal.sends.restart_schedules.is_empty());
            assert!(terminal.sends.worker_shutdowns.is_empty());
            match disposition {
                ShutdownDisposition::Accepted => {
                    assert!(terminal.sends.diagnostics.is_empty());
                }
                ShutdownDisposition::Rejected => {
                    assert_eq!(terminal.sends.diagnostics.len(), 1);
                }
            }
        }
    }
}

#[test]
fn forced_retirement_keeps_a_pending_worker_until_the_pool_is_dropped() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let mut activation = Some(HeldActivation(Arc::clone(&activation_drops)));
    let roles = OrderedRoles::new(SearchRole::Primary, [])
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = keyed(
        move |_: &SearchRole| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                activation
                    .take()
                    .unwrap_or_else(|| panic!("one role prepares one worker")),
            ))
        },
        roles,
        |_: &Account| SearchRole::Primary,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
        Interruption::Fail,
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        },
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a keyed pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("keyed initialization failed: {error}"));
    let mut pool = initialized.behavior;
    drop(initialized.actions.creates);

    let retiring = pool
        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
        .unwrap_or_else(|error| panic!("keyed shutdown failed: {error}"));
    let deadline = retiring
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("bounded retirement schedules one deadline"));
    let forced = pool
        .transition(KeyedEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: deadline,
                reason: ScheduleAfterRejection::DeadlineOverflow,
            },
        )))
        .unwrap_or_else(|error| panic!("deadline rejection failed: {error}"));

    assert!(matches!(forced.become_, Step::Stop(_)));
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(pool);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}
