use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use behavior_actors::Activate as _;

use super::{
    ActivationPolicy, ActorDrainPolicy, BacklogCapacity, ChildStopped, DiagnosticDisposition,
    EstablishedShutdownResolved, Exit, FifoCommand, FifoEvent, Interruption, ItemSettlement, Never,
    OrderedRoles, PoolFailureReaction, PoolRecovery, RuntimeAddr, ScheduleAfterRejection,
    SearchWorker, SettledItem, Step, TimerElapsed, TimerScheduled, WorkerInitializationOutcome,
    WorkerSubmission, created_worker, fifo,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeskInput {
    WorkerStopped,
    ShutdownReturned,
    DeadlineAccepted,
    DeadlineRejected,
    DeadlineElapsed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReturnProgress {
    Expected,
    Received,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkerRetirement {
    stopped: ReturnProgress,
    shutdown: ReturnProgress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeadlineProgress {
    Scheduling,
    Waiting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RetirementDesk {
    Draining {
        worker: WorkerRetirement,
        deadline: DeadlineProgress,
    },
    Retired,
    Forced,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputDisposition {
    Applied,
    Unmatched,
    Late,
}

impl RetirementDesk {
    const fn new() -> Self {
        Self::Draining {
            worker: WorkerRetirement {
                stopped: ReturnProgress::Expected,
                shutdown: ReturnProgress::Expected,
            },
            deadline: DeadlineProgress::Scheduling,
        }
    }

    const fn apply(self, input: DeskInput) -> (Self, InputDisposition) {
        match (self, input) {
            (Self::Retired, _) => (Self::Retired, InputDisposition::Late),
            (Self::Forced, _) => (Self::Forced, InputDisposition::Late),
            (
                Self::Draining {
                    worker:
                        WorkerRetirement {
                            stopped: ReturnProgress::Expected,
                            shutdown: ReturnProgress::Expected,
                        },
                    deadline,
                },
                DeskInput::WorkerStopped,
            ) => (
                Self::Draining {
                    worker: WorkerRetirement {
                        stopped: ReturnProgress::Received,
                        shutdown: ReturnProgress::Expected,
                    },
                    deadline,
                },
                InputDisposition::Applied,
            ),
            (
                Self::Draining {
                    worker:
                        WorkerRetirement {
                            stopped: ReturnProgress::Expected,
                            shutdown: ReturnProgress::Received,
                        },
                    ..
                },
                DeskInput::WorkerStopped,
            ) => (Self::Retired, InputDisposition::Applied),
            (state @ Self::Draining { .. }, DeskInput::WorkerStopped) => {
                (state, InputDisposition::Unmatched)
            }
            (
                Self::Draining {
                    worker:
                        WorkerRetirement {
                            stopped: ReturnProgress::Expected,
                            shutdown: ReturnProgress::Expected,
                        },
                    deadline,
                },
                DeskInput::ShutdownReturned,
            ) => (
                Self::Draining {
                    worker: WorkerRetirement {
                        stopped: ReturnProgress::Expected,
                        shutdown: ReturnProgress::Received,
                    },
                    deadline,
                },
                InputDisposition::Applied,
            ),
            (
                Self::Draining {
                    worker:
                        WorkerRetirement {
                            stopped: ReturnProgress::Received,
                            shutdown: ReturnProgress::Expected,
                        },
                    ..
                },
                DeskInput::ShutdownReturned,
            ) => (Self::Retired, InputDisposition::Applied),
            (state @ Self::Draining { .. }, DeskInput::ShutdownReturned) => {
                (state, InputDisposition::Unmatched)
            }
            (
                Self::Draining {
                    worker,
                    deadline: DeadlineProgress::Scheduling,
                },
                DeskInput::DeadlineAccepted,
            ) => (
                Self::Draining {
                    worker,
                    deadline: DeadlineProgress::Waiting,
                },
                InputDisposition::Applied,
            ),
            (state @ Self::Draining { .. }, DeskInput::DeadlineAccepted) => {
                (state, InputDisposition::Unmatched)
            }
            (Self::Draining { .. }, DeskInput::DeadlineRejected) => {
                (Self::Forced, InputDisposition::Applied)
            }
            (
                Self::Draining {
                    deadline: DeadlineProgress::Waiting,
                    ..
                },
                DeskInput::DeadlineElapsed,
            ) => (Self::Forced, InputDisposition::Applied),
            (state @ Self::Draining { .. }, DeskInput::DeadlineElapsed) => {
                (state, InputDisposition::Unmatched)
            }
        }
    }
}

const REJECTION_ORDERS: [[DeskInput; 3]; 6] = [
    [
        DeskInput::WorkerStopped,
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineRejected,
    ],
    [
        DeskInput::WorkerStopped,
        DeskInput::DeadlineRejected,
        DeskInput::ShutdownReturned,
    ],
    [
        DeskInput::ShutdownReturned,
        DeskInput::WorkerStopped,
        DeskInput::DeadlineRejected,
    ],
    [
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineRejected,
        DeskInput::WorkerStopped,
    ],
    [
        DeskInput::DeadlineRejected,
        DeskInput::WorkerStopped,
        DeskInput::ShutdownReturned,
    ],
    [
        DeskInput::DeadlineRejected,
        DeskInput::ShutdownReturned,
        DeskInput::WorkerStopped,
    ],
];

const FIRING_ORDERS: [[DeskInput; 4]; 12] = [
    [
        DeskInput::DeadlineAccepted,
        DeskInput::DeadlineElapsed,
        DeskInput::WorkerStopped,
        DeskInput::ShutdownReturned,
    ],
    [
        DeskInput::DeadlineAccepted,
        DeskInput::DeadlineElapsed,
        DeskInput::ShutdownReturned,
        DeskInput::WorkerStopped,
    ],
    [
        DeskInput::DeadlineAccepted,
        DeskInput::WorkerStopped,
        DeskInput::DeadlineElapsed,
        DeskInput::ShutdownReturned,
    ],
    [
        DeskInput::DeadlineAccepted,
        DeskInput::WorkerStopped,
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineElapsed,
    ],
    [
        DeskInput::DeadlineAccepted,
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineElapsed,
        DeskInput::WorkerStopped,
    ],
    [
        DeskInput::DeadlineAccepted,
        DeskInput::ShutdownReturned,
        DeskInput::WorkerStopped,
        DeskInput::DeadlineElapsed,
    ],
    [
        DeskInput::WorkerStopped,
        DeskInput::DeadlineAccepted,
        DeskInput::DeadlineElapsed,
        DeskInput::ShutdownReturned,
    ],
    [
        DeskInput::WorkerStopped,
        DeskInput::DeadlineAccepted,
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineElapsed,
    ],
    [
        DeskInput::WorkerStopped,
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineAccepted,
        DeskInput::DeadlineElapsed,
    ],
    [
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineAccepted,
        DeskInput::DeadlineElapsed,
        DeskInput::WorkerStopped,
    ],
    [
        DeskInput::ShutdownReturned,
        DeskInput::DeadlineAccepted,
        DeskInput::WorkerStopped,
        DeskInput::DeadlineElapsed,
    ],
    [
        DeskInput::ShutdownReturned,
        DeskInput::WorkerStopped,
        DeskInput::DeadlineAccepted,
        DeskInput::DeadlineElapsed,
    ],
];

async fn compare_sequence(sequence: &[DeskInput]) {
    let role_owner = Arc::new(());
    let roles = OrderedRoles::new(Arc::clone(&role_owner), [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        |_: &Arc<()>| Ok::<_, Never>(WorkerSubmission::immediate(SearchWorker)),
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::from_secs(2),
        },
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialization emits one worker creation"));
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("created worker awaits initialization"));
    let mut worker = Some(initialization.worker().creation());
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("initialized worker begins activation"));
    let activation_started = pool
        .on(activation.started())
        .unwrap_or_else(|error| panic!("activation start failed: {error}"));
    assert!(activation_started.sends.worker_assignments.is_empty());
    let ready = pool
        .on(activation.activate().await)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    assert!(ready.sends.worker_assignments.is_empty());

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
    assert_eq!(draining.sends.restart_schedules.len(), 1);
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    assert!(draining.sends.diagnostics.is_empty());
    let shutdown_id = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("shutdown emits one exact worker request"))
        .id;
    let mut deadline = Some(
        draining
            .sends
            .restart_schedules
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("bounded drain schedules one deadline")),
    );
    let deadline_id = deadline
        .as_ref()
        .unwrap_or_else(|| panic!("deadline remains outside the pool"))
        .id;
    let deadline_generation = deadline
        .as_ref()
        .unwrap_or_else(|| panic!("deadline remains outside the pool"))
        .generation;
    let mut desk = RetirementDesk::new();
    assert_eq!(Arc::strong_count(&role_owner), 2);

    for input in sequence {
        let acted = match input {
            DeskInput::WorkerStopped => pool
                .on(ChildStopped::new(
                    worker
                        .take()
                        .unwrap_or_else(|| panic!("sequence stops the worker once")),
                    Ok(Exit::Normal),
                    Instant::now(),
                ))
                .unwrap_or_else(|error| panic!("worker exit input failed: {error}")),
            DeskInput::ShutdownReturned => pool
                .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
                    shutdown_id,
                ))
                .unwrap_or_else(|error| panic!("shutdown return failed: {error}")),
            DeskInput::DeadlineAccepted => {
                let _accepted_request = deadline
                    .take()
                    .unwrap_or_else(|| panic!("sequence settles the deadline once"));
                pool.transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
                    ItemSettlement::Accepted(TimerScheduled {
                        id: deadline_id,
                        generation: deadline_generation,
                    }),
                )))
                .unwrap_or_else(|error| panic!("deadline acceptance failed: {error}"))
            }
            DeskInput::DeadlineRejected => pool
                .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
                    ItemSettlement::Rejected {
                        item: deadline
                            .take()
                            .unwrap_or_else(|| panic!("sequence settles the deadline once")),
                        reason: ScheduleAfterRejection::DeadlineOverflow,
                    },
                )))
                .unwrap_or_else(|error| panic!("deadline rejection failed: {error}")),
            DeskInput::DeadlineElapsed => pool
                .on(TimerElapsed::new(deadline_id, deadline_generation))
                .unwrap_or_else(|error| panic!("deadline input failed: {error}")),
        };
        let (next, disposition) = desk.apply(*input);
        desk = next;

        assert!(acted.creates.is_empty());
        assert!(acted.sends.worker_observations.is_empty());
        assert!(acted.sends.worker_initializations.is_empty());
        assert!(acted.sends.worker_activations.is_empty());
        assert!(acted.sends.customer_outcomes.as_slice().is_empty());
        assert!(acted.sends.worker_assignments.is_empty());
        assert!(acted.sends.worker_preparations.is_empty());
        assert!(acted.sends.restart_schedules.is_empty());
        assert!(acted.sends.worker_shutdowns.is_empty());
        match disposition {
            InputDisposition::Applied => assert!(acted.sends.diagnostics.is_empty()),
            InputDisposition::Unmatched | InputDisposition::Late => {
                assert_eq!(acted.sends.diagnostics.len(), 1)
            }
        }
        match desk {
            RetirementDesk::Draining { .. } => {
                assert!(matches!(acted.become_, Step::Continue))
            }
            RetirementDesk::Retired | RetirementDesk::Forced => {
                assert!(matches!(acted.become_, Step::Stop(_)))
            }
        }
        drop(acted);
        match desk {
            RetirementDesk::Draining { .. } | RetirementDesk::Forced => {
                assert_eq!(Arc::strong_count(&role_owner), 2)
            }
            RetirementDesk::Retired => assert_eq!(Arc::strong_count(&role_owner), 1),
        }
    }

    drop(pool);
    assert_eq!(Arc::strong_count(&role_owner), 1);
}

#[tokio::test]
async fn retirement_desk_matches_every_worker_and_deadline_order() {
    for sequence in REJECTION_ORDERS {
        compare_sequence(&sequence).await;
    }
    for sequence in FIRING_ORDERS {
        compare_sequence(&sequence).await;
    }
}
