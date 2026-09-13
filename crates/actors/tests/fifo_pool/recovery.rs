use core::ops::ControlFlow;
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

                let input = match ending {
                    PreparationEnding::Prepared => {
                        let ControlFlow::Break(preparation) =
                            request.accept(WorkerSubmission::immediate(SearchWorker))
                        else {
                            panic!("one selected role completes one preparation")
                        };
                        SettledItem::Attempted(ItemSettlement::Accepted(preparation))
                    }
                    PreparationEnding::WorkerRejected => {
                        SettledItem::Attempted(ItemSettlement::Accepted(
                            request.reject(SearchWorkerRejection::Unavailable),
                        ))
                    }
                    PreparationEnding::SourceRejected => {
                        SettledItem::Attempted(ItemSettlement::Rejected {
                            item: request,
                            reason: SearchSourceRejection::Closed,
                        })
                    }
                    PreparationEnding::Corrupt => SettledItem::Attempted(ItemSettlement::Corrupt {
                        item: request,
                        fault: InterpreterFault::CorruptTraversal,
                    }),
                    PreparationEnding::Unattempted => SettledItem::Unattempted(request),
                };
                let acted = pool
                    .transition(FifoEvent::WorkerPreparationSettled(input))
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
