use core::ops::ControlFlow;
use std::convert::Infallible;
use std::sync::Arc;

use behavior::Never;
use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, DiagnosticDisposition, ImmediateActivation, WorkerSource,
    fifo,
};
use behavior_actors::{Activate, ObserveChild};

use super::{WorkerInitializationOutcome, created_worker};
use std::time::{Duration, Instant};

use super::{
    BacklogCapacity, ChildStopped, DiagnosticAction, Exit, FallibleSearchSource, FifoCommand,
    FifoEvent, InterpreterFault, Interruption, ItemSettlement, OrderedRoles, PoolFailureReaction,
    PoolRecovery, ReadySearchPool, RestartLimit, RestartRelease, Role, RuntimeAddr,
    SearchSourceRejection, SearchWorker, SearchWorkerRejection, SettledItem, Step,
    WorkerSubmission, ready_search_pool,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeskOwnership {
    Serving,
    ShuttingDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparationEnding {
    Prepared,
    WorkerRejected,
    SourceRejected,
    Corrupt,
    Unattempted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeskTopology {
    RetireRole,
    StopPool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryChoice {
    CreateWorker,
    RetireRole,
    StopPool,
    ReturnDuringShutdown,
}

fn recovery_choice(
    ownership: DeskOwnership,
    topology: DeskTopology,
    ending: PreparationEnding,
) -> RecoveryChoice {
    match (ownership, ending, topology) {
        (DeskOwnership::ShuttingDown, _, _) => RecoveryChoice::ReturnDuringShutdown,
        (DeskOwnership::Serving, PreparationEnding::Prepared, _) => RecoveryChoice::CreateWorker,
        (DeskOwnership::Serving, _, DeskTopology::RetireRole) => RecoveryChoice::RetireRole,
        (DeskOwnership::Serving, _, DeskTopology::StopPool) => RecoveryChoice::StopPool,
    }
}

#[tokio::test]
async fn recovery_desk_matches_every_preparation_disposition() {
    const OWNERSHIP: [DeskOwnership; 2] = [DeskOwnership::Serving, DeskOwnership::ShuttingDown];
    const TOPOLOGY: [DeskTopology; 2] = [DeskTopology::RetireRole, DeskTopology::StopPool];
    const ENDINGS: [PreparationEnding; 5] = [
        PreparationEnding::Prepared,
        PreparationEnding::WorkerRejected,
        PreparationEnding::SourceRejected,
        PreparationEnding::Corrupt,
        PreparationEnding::Unattempted,
    ];

    for ownership in OWNERSHIP {
        for topology in TOPOLOGY {
            for ending in ENDINGS {
                let roles = OrderedRoles::new(Role::Search, [])
                    .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
                let failure = match topology {
                    DeskTopology::RetireRole => PoolFailureReaction::RetireRole,
                    DeskTopology::StopPool => PoolFailureReaction::StopPool,
                };
                let ReadySearchPool {
                    mut pool,
                    mut workers,
                } = ready_search_pool(
                    roles,
                    BacklogCapacity::new(0),
                    Interruption::Fail,
                    PoolRecovery::permanent(
                        FallibleSearchSource,
                        RestartLimit::new(3, Duration::from_secs(10)),
                        RestartRelease::immediate(),
                        failure,
                    ),
                )
                .await;
                let stopped = pool
                    .on(ChildStopped::new(
                        workers
                            .pop()
                            .unwrap_or_else(|| panic!("the ready worker exists")),
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|error| panic!("worker exit input failed: {error}"));
                assert!(matches!(stopped.become_, Step::Continue));
                assert!(stopped.creates.is_empty());
                assert!(stopped.sends.worker_observations.is_empty());
                assert!(stopped.sends.worker_initializations.is_empty());
                assert!(stopped.sends.worker_activations.is_empty());
                assert!(stopped.sends.customer_outcomes.as_slice().is_empty());
                assert!(stopped.sends.worker_assignments.is_empty());
                assert_eq!(stopped.sends.worker_preparations.len(), 1);
                assert!(stopped.sends.restart_schedules.is_empty());
                assert!(stopped.sends.worker_shutdowns.is_empty());
                assert!(stopped.sends.diagnostics.is_empty());
                let request = stopped
                    .sends
                    .worker_preparations
                    .into_items()
                    .pop()
                    .unwrap_or_else(|| panic!("permanent worker exit prepares one replacement"));

                match ownership {
                    DeskOwnership::Serving => {}
                    DeskOwnership::ShuttingDown => {
                        let draining = pool
                            .receive(RuntimeAddr(7), FifoCommand::shutdown())
                            .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
                        assert!(matches!(draining.become_, Step::Continue));
                        assert!(draining.creates.is_empty());
                        assert!(draining.sends.worker_observations.is_empty());
                        assert!(draining.sends.worker_initializations.is_empty());
                        assert!(draining.sends.worker_activations.is_empty());
                        assert!(draining.sends.customer_outcomes.as_slice().is_empty());
                        assert!(draining.sends.worker_assignments.is_empty());
                        assert!(draining.sends.worker_preparations.is_empty());
                        assert!(draining.sends.restart_schedules.is_empty());
                        assert!(draining.sends.worker_shutdowns.is_empty());
                        assert!(draining.sends.diagnostics.is_empty());
                    }
                }

                let event = match ending {
                    PreparationEnding::Prepared => {
                        let starting = start_worker_preparation!(pool, request);
                        let ControlFlow::Break(preparation) =
                            starting.accept(WorkerSubmission::immediate(SearchWorker))
                        else {
                            panic!("one selected role completes one preparation")
                        };
                        FifoEvent::WorkerPreparationReturned(preparation)
                    }
                    PreparationEnding::WorkerRejected => {
                        let starting = start_worker_preparation!(pool, request);
                        FifoEvent::WorkerPreparationReturned(
                            starting.reject(SearchWorkerRejection::Unavailable),
                        )
                    }
                    PreparationEnding::SourceRejected => {
                        let starting = start_worker_preparation!(pool, request);
                        FifoEvent::WorkerPreparationReturned(
                            starting.reject_source(SearchSourceRejection::Closed),
                        )
                    }
                    PreparationEnding::Corrupt => FifoEvent::WorkerPreparationStarted(
                        SettledItem::Attempted(ItemSettlement::Corrupt {
                            item: request,
                            fault: InterpreterFault::CorruptTraversal,
                        }),
                    ),
                    PreparationEnding::Unattempted => {
                        FifoEvent::WorkerPreparationStarted(SettledItem::Unattempted(request))
                    }
                };
                let acted = pool
                    .transition(event)
                    .unwrap_or_else(|error| panic!("worker preparation input failed: {error}"));

                assert!(acted.sends.worker_initializations.is_empty());
                assert!(acted.sends.worker_activations.is_empty());
                assert!(acted.sends.customer_outcomes.as_slice().is_empty());
                assert!(acted.sends.worker_assignments.is_empty());
                assert!(acted.sends.worker_preparations.is_empty());
                assert!(acted.sends.restart_schedules.is_empty());
                assert!(acted.sends.worker_shutdowns.is_empty());
                let diagnostics = acted.sends.diagnostics.into_requests();

                match recovery_choice(ownership, topology, ending) {
                    RecoveryChoice::CreateWorker => {
                        assert!(matches!(acted.become_, Step::Continue));
                        assert_eq!(acted.creates.len(), 1);
                        assert_eq!(acted.sends.worker_observations.len(), 1);
                        assert!(diagnostics.is_empty());
                    }
                    RecoveryChoice::RetireRole => {
                        assert!(matches!(acted.become_, Step::Continue));
                        assert!(acted.creates.is_empty());
                        assert!(acted.sends.worker_observations.is_empty());
                        assert_eq!(diagnostics.len(), 1);
                    }
                    RecoveryChoice::StopPool | RecoveryChoice::ReturnDuringShutdown => {
                        assert!(matches!(acted.become_, Step::Stop(_)));
                        assert!(acted.creates.is_empty());
                        assert!(acted.sends.worker_observations.is_empty());
                        assert_eq!(diagnostics.len(), 1);
                    }
                }
                for diagnostic in diagnostics {
                    let diagnostic = match diagnostic {
                        DiagnosticAction::Terminal { diagnostic } => diagnostic,
                        DiagnosticAction::Deliver { .. } => {
                            panic!("the model selects terminal diagnostics")
                        }
                    };
                    assert_eq!(diagnostic.role(), Some(&Role::Search));
                }
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SearchDesk {
    pub(super) name: Vec<u8>,
}

pub(super) struct SearchRecoverySource;

impl WorkerSource<SearchDesk, SearchWorker, ImmediateActivation> for SearchRecoverySource {
    type WorkerRejection = Arc<Vec<u8>>;
    type SourceRejection = Vec<u8>;
}

#[tokio::test]
async fn source_rejection_consumes_exact_fifo_inputs() {
    let name = vec![31, 47, 59];
    let name_allocation = name.as_ptr();
    let roles = OrderedRoles::new(SearchDesk { name }, []).unwrap_or_else(|_| panic!("one role"));
    let pool = fifo(
        |_: &SearchDesk| Ok::<_, Never>(WorkerSubmission::immediate(SearchWorker)),
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation")),
        PoolRecovery::permanent(
            SearchRecoverySource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::immediate(),
            PoolFailureReaction::RetireRole,
        ),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("infallible worker declaration"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("initialize: {error}"));
    let mut pool = initialized.behavior;
    let initial = initialized.actions;
    assert!(matches!(initial.become_, Step::Continue));
    assert_eq!(initial.creates.len(), 1);
    assert_eq!(initial.sends.worker_observations.len(), 1);
    assert_eq!(initial.sends.worker_initializations.len(), 0);
    assert_eq!(initial.sends.worker_activations.len(), 0);
    assert_eq!(initial.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(initial.sends.worker_assignments.len(), 0);
    assert_eq!(initial.sends.worker_preparations.len(), 0);
    assert_eq!(initial.sends.restart_schedules.len(), 0);
    assert_eq!(initial.sends.worker_shutdowns.len(), 0);
    assert_eq!(initial.sends.diagnostics.len(), 0);
    let mut creations = initial.creates.into_iter();
    let creation = creations.next().unwrap_or_else(|| panic!("one creation"));
    let remaining = creations.next();
    assert!(remaining.is_none());
    let child = creation.id();
    let observations = initial.sends.worker_observations.into_requests();
    assert_eq!(observations[0], ObserveChild::new(child));
    drop(observations);
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("commit: {error}"));
    assert!(matches!(committed.become_, Step::Continue));
    assert_eq!(committed.creates.len(), 0);
    assert_eq!(committed.sends.worker_observations.len(), 0);
    assert_eq!(committed.sends.worker_initializations.len(), 1);
    assert_eq!(committed.sends.worker_activations.len(), 0);
    assert_eq!(committed.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(committed.sends.worker_assignments.len(), 0);
    assert_eq!(committed.sends.worker_preparations.len(), 0);
    assert_eq!(committed.sends.restart_schedules.len(), 0);
    assert_eq!(committed.sends.worker_shutdowns.len(), 0);
    assert_eq!(committed.sends.diagnostics.len(), 0);
    let mut initializations = committed.sends.worker_initializations.into_requests();
    let initialization = initializations
        .pop()
        .unwrap_or_else(|| panic!("one initialization"));
    let previous = initialization.worker();
    assert_eq!(previous.creation(), child);
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization: {error}"));
    assert!(matches!(initialized_worker.become_, Step::Continue));
    assert_eq!(initialized_worker.creates.len(), 0);
    assert_eq!(initialized_worker.sends.worker_observations.len(), 0);
    assert_eq!(initialized_worker.sends.worker_initializations.len(), 0);
    assert_eq!(initialized_worker.sends.worker_activations.len(), 1);
    assert_eq!(
        initialized_worker.sends.customer_outcomes.as_slice().len(),
        0
    );
    assert_eq!(initialized_worker.sends.worker_assignments.len(), 0);
    assert_eq!(initialized_worker.sends.worker_preparations.len(), 0);
    assert_eq!(initialized_worker.sends.restart_schedules.len(), 0);
    assert_eq!(initialized_worker.sends.worker_shutdowns.len(), 0);
    assert_eq!(initialized_worker.sends.diagnostics.len(), 0);
    let mut activations = initialized_worker.sends.worker_activations.into_requests();
    let activation = activations
        .pop()
        .unwrap_or_else(|| panic!("one activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("start: {error}"));
    assert!(matches!(activation_started.become_, Step::Continue));
    assert_eq!(activation_started.creates.len(), 0);
    assert_eq!(activation_started.sends.worker_observations.len(), 0);
    assert_eq!(activation_started.sends.worker_initializations.len(), 0);
    assert_eq!(activation_started.sends.worker_activations.len(), 0);
    assert_eq!(
        activation_started.sends.customer_outcomes.as_slice().len(),
        0
    );
    assert_eq!(activation_started.sends.worker_assignments.len(), 0);
    assert_eq!(activation_started.sends.worker_preparations.len(), 0);
    assert_eq!(activation_started.sends.restart_schedules.len(), 0);
    assert_eq!(activation_started.sends.worker_shutdowns.len(), 0);
    assert_eq!(activation_started.sends.diagnostics.len(), 0);
    let active = activation.activate().await;
    let activated = pool
        .on(active)
        .unwrap_or_else(|error| panic!("activate: {error}"));
    assert!(matches!(activated.become_, Step::Continue));
    assert_eq!(activated.creates.len(), 0);
    assert_eq!(activated.sends.worker_observations.len(), 0);
    assert_eq!(activated.sends.worker_initializations.len(), 0);
    assert_eq!(activated.sends.worker_activations.len(), 0);
    assert_eq!(activated.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(activated.sends.worker_assignments.len(), 0);
    assert_eq!(activated.sends.worker_preparations.len(), 0);
    assert_eq!(activated.sends.restart_schedules.len(), 0);
    assert_eq!(activated.sends.worker_shutdowns.len(), 0);
    assert_eq!(activated.sends.diagnostics.len(), 0);
    let stopped = ChildStopped::new(child, Ok(Exit::Normal), Instant::now());
    let stopping = pool
        .on(stopped)
        .unwrap_or_else(|error| panic!("stopped: {error}"));
    assert!(matches!(stopping.become_, Step::Continue));
    assert_eq!(stopping.creates.len(), 0);
    assert_eq!(stopping.sends.worker_observations.len(), 0);
    assert_eq!(stopping.sends.worker_initializations.len(), 0);
    assert_eq!(stopping.sends.worker_activations.len(), 0);
    assert_eq!(stopping.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(stopping.sends.worker_assignments.len(), 0);
    assert_eq!(stopping.sends.worker_preparations.len(), 1);
    assert_eq!(stopping.sends.restart_schedules.len(), 0);
    assert_eq!(stopping.sends.worker_shutdowns.len(), 0);
    assert_eq!(stopping.sends.diagnostics.len(), 0);
    let mut requests = stopping.sends.worker_preparations.into_items();
    let request = requests.pop().unwrap_or_else(|| panic!("one preparation"));
    let (receipt, mut starting) = request.start();
    let started = pool
        .transition(FifoEvent::WorkerPreparationStarted(SettledItem::Attempted(
            ItemSettlement::Accepted(receipt),
        )))
        .unwrap_or_else(|error| panic!("preparation started: {error}"));
    assert!(matches!(started.become_, Step::Continue));
    assert_eq!(started.creates.len(), 0);
    assert_eq!(started.sends.worker_observations.len(), 0);
    assert_eq!(started.sends.worker_initializations.len(), 0);
    assert_eq!(started.sends.worker_activations.len(), 0);
    assert_eq!(started.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(started.sends.worker_assignments.len(), 0);
    assert_eq!(started.sends.worker_preparations.len(), 0);
    assert_eq!(started.sends.restart_schedules.len(), 0);
    assert_eq!(started.sends.worker_shutdowns.len(), 0);
    assert_eq!(started.sends.diagnostics.len(), 0);
    let (_, selected_role) = starting.source_and_role();
    let role_allocation = selected_role as *const SearchDesk;
    assert_eq!(selected_role.name.as_ptr(), name_allocation);
    let reason = vec![99, 7, 1];
    let reason_allocation = reason.as_ptr();
    let rejected = starting.reject_source(reason);
    let returned = pool
        .transition(FifoEvent::WorkerPreparationReturned(rejected))
        .unwrap_or_else(|error| panic!("source rejected: {error}"));
    assert!(matches!(returned.become_, Step::Continue));
    assert_eq!(returned.creates.len(), 0);
    assert_eq!(returned.sends.worker_observations.len(), 0);
    assert_eq!(returned.sends.worker_initializations.len(), 0);
    assert_eq!(returned.sends.worker_activations.len(), 0);
    assert_eq!(returned.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(returned.sends.worker_assignments.len(), 0);
    assert_eq!(returned.sends.worker_preparations.len(), 0);
    assert_eq!(returned.sends.restart_schedules.len(), 0);
    assert_eq!(returned.sends.worker_shutdowns.len(), 0);
    assert_eq!(returned.sends.diagnostics.len(), 1);
    let mut diagnostics = returned.sends.diagnostics.into_requests();
    let diagnostic = diagnostics
        .pop()
        .unwrap_or_else(|| panic!("one diagnostic"));
    let diagnostic = match diagnostic {
        DiagnosticAction::Terminal { diagnostic } => diagnostic,
        DiagnosticAction::Deliver { route, .. } => match route {},
    };
    let extraction = diagnostic.into_source_rejection();
    let Ok((role, retained_previous, retained_stopped, returned_source, reason)) = extraction
    else {
        panic!("source rejection is consumable")
    };
    assert_eq!(Arc::as_ptr(&role), role_allocation);
    assert_eq!(role.name.as_ptr(), name_allocation);
    assert_eq!(role.name, [31, 47, 59]);
    assert_eq!(retained_previous, previous);
    assert_eq!(retained_stopped, stopped);
    assert!(returned_source.is_none());
    assert_eq!(reason.as_ptr(), reason_allocation);
    assert_eq!(reason, [99, 7, 1]);
    let replayed = pool
        .on(stopped)
        .unwrap_or_else(|error| panic!("replay: {error}"));
    assert!(matches!(replayed.become_, Step::Continue));
    assert_eq!(replayed.creates.len(), 0);
    assert_eq!(replayed.sends.worker_observations.len(), 0);
    assert_eq!(replayed.sends.worker_initializations.len(), 0);
    assert_eq!(replayed.sends.worker_activations.len(), 0);
    assert_eq!(replayed.sends.customer_outcomes.as_slice().len(), 0);
    assert_eq!(replayed.sends.worker_assignments.len(), 0);
    assert_eq!(replayed.sends.worker_preparations.len(), 0);
    assert_eq!(replayed.sends.restart_schedules.len(), 0);
    assert_eq!(replayed.sends.worker_shutdowns.len(), 0);
    assert_eq!(replayed.sends.diagnostics.len(), 1);
    let replay_diagnostics = replayed.sends.diagnostics.into_requests();
    drop(replay_diagnostics);
}
