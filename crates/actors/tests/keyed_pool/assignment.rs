use std::time::Instant;

use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, AssignWorker, BacklogCapacity, BindingCapacity,
    CustomerDelivery, DiagnosticDisposition, Interruption, KeyedCommand, KeyedEvent, KeyedOutcome,
    KeyedQueuedReturnReason, OrderedRoles, PoolFailureReaction, PoolRecovery, SubmissionId,
    WorkerInitializationOutcome, keyed,
};
use behavior_actors::{
    ActionItemResult, Activate, ChildCreationOutcome, ChildHead, ChildStopped, CreateChild,
    CreationSettlement, CreationsSettled, EstablishedCreation, EstablishedRecipient,
    EstablishedShutdownResolved, ExactDeliveryReason, Exit, ItemSettlement, MessageProtocol,
    Recipient, SettledItem, Step, StopOnShutdown,
};

use super::domain::{
    Account, Endpoint, RuntimeAddr, SearchJob, SearchResult, SearchRole, SearchWorker,
    prepare_worker,
};

fn commit_worker_creation(
    creation: CreateChild<RuntimeAddr, StopOnShutdown<SearchWorker>>,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<SearchWorker>> {
    let (worker, _, kind) = creation.into_parts();
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
async fn rejected_assignments_wait_for_worker_shutdown_before_returning_to_customers() {
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
        assert!(started.sends.worker_assignments.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.sends.worker_assignments.is_empty());
    }

    let primary_customer = Recipient::<
        MessageProtocol<RuntimeAddr, KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>>,
    >::global(RuntimeAddr(90));
    let primary_assigned = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(1),
                Account(2),
                SearchJob(10),
                primary_customer,
            ),
        )
        .unwrap_or_else(|error| panic!("primary assignment failed: {error}"));
    let primary_assignment = primary_assigned
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle primary receives its job"));

    let primary_queued = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(2),
                Account(2),
                SearchJob(11),
                Recipient::global(RuntimeAddr(91)),
            ),
        )
        .unwrap_or_else(|error| panic!("primary queue admission failed: {error}"));
    assert!(primary_queued.sends.worker_assignments.is_empty());

    let replica_assigned = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(3),
                Account(3),
                SearchJob(20),
                Recipient::global(RuntimeAddr(92)),
            ),
        )
        .unwrap_or_else(|error| panic!("replica assignment failed: {error}"));
    let replica_assignment = replica_assigned
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle replica receives its job"));

    let primary_rejected: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: primary_assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let primary_quarantined = pool
        .transition(KeyedEvent::AssignmentSettled(primary_rejected))
        .unwrap_or_else(|error| panic!("primary assignment rejection failed: {error}"));
    assert!(primary_quarantined.sends.customer_outcomes.is_empty());
    let primary_shutdown = primary_quarantined
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("rejected delivery quarantines the primary worker"));

    let replica_rejected: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: replica_assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let replica_quarantined = pool
        .transition(KeyedEvent::AssignmentSettled(replica_rejected))
        .unwrap_or_else(|error| panic!("replica assignment rejection failed: {error}"));
    assert!(replica_quarantined.sends.customer_outcomes.is_empty());
    let replica_shutdown = replica_quarantined
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("rejected delivery quarantines the replica worker"));

    let primary_waiting = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            primary_shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("primary shutdown settlement failed: {error}"));
    assert!(primary_waiting.sends.customer_outcomes.is_empty());
    assert!(matches!(primary_waiting.become_, Step::Continue));
    let primary_retired = pool
        .on(ChildStopped::new(
            workers[0],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("primary worker exit failed: {error}"));
    let primary_returns = primary_retired.sends.customer_outcomes.into_requests();
    assert_eq!(primary_returns.len(), 2);
    for (returned, expected) in primary_returns
        .into_iter()
        .zip([SearchJob(10), SearchJob(11)])
    {
        match returned {
            CustomerDelivery::Logical { delivery } => match delivery.message {
                KeyedOutcome::ReturnedQueued {
                    binding,
                    payload,
                    reason,
                    ..
                } => {
                    assert_eq!(binding.role(), &SearchRole::Primary);
                    assert_eq!(payload, expected);
                    assert_eq!(reason, KeyedQueuedReturnReason::RolePermanentlyUnavailable);
                }
                _ => panic!("retired primary returns queued work"),
            },
            CustomerDelivery::Established { .. }
            | CustomerDelivery::RejectedLogical { .. }
            | CustomerDelivery::RejectedEstablished { .. } => {
                panic!("primary jobs retain their logical customers")
            }
        }
    }

    let replica_waiting = pool
        .on(ChildStopped::new(
            workers[1],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("replica worker exit failed: {error}"));
    assert!(replica_waiting.sends.customer_outcomes.is_empty());
    assert!(matches!(replica_waiting.become_, Step::Continue));
    let replica_retired = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            replica_shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("replica shutdown settlement failed: {error}"));
    let returned = replica_retired
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("retired replica returns its queued job"));
    match returned {
        CustomerDelivery::Logical { delivery } => match delivery.message {
            KeyedOutcome::ReturnedQueued {
                binding,
                payload,
                reason,
                ..
            } => {
                assert_eq!(binding.role(), &SearchRole::Replica);
                assert_eq!(payload, SearchJob(20));
                assert_eq!(reason, KeyedQueuedReturnReason::RolePermanentlyUnavailable);
            }
            _ => panic!("retired replica returns queued work"),
        },
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the replica job retains its logical customer")
        }
    }
}

#[tokio::test]
async fn exact_completion_selects_the_busy_nonzero_role() {
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
        assert!(started.sends.worker_assignments.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.sends.worker_assignments.is_empty());
    }

    let primary = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(1),
                Account(2),
                SearchJob(10),
                Recipient::global(RuntimeAddr(91)),
            ),
        )
        .unwrap_or_else(|error| panic!("primary assignment failed: {error}"));
    let primary_accepted = primary
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("primary customer observes admission"));
    let primary_job = match primary_accepted {
        CustomerDelivery::Logical { delivery } => {
            assert_eq!(delivery.to.address(), RuntimeAddr(91));
            match delivery.message {
                KeyedOutcome::Accepted { job, binding, .. } => {
                    assert_eq!(binding.role(), &SearchRole::Primary);
                    let generation_debug = format!("{:?}", binding.generation());
                    assert!(generation_debug.contains("BindingGeneration"));
                    assert!(generation_debug.contains(&binding.generation().get().to_string()));
                    let binding_debug = format!("{binding:?}");
                    assert!(binding_debug.contains("BindingEvidence"));
                    assert!(binding_debug.contains(&generation_debug));
                    assert!(binding_debug.contains("Primary"));
                    assert!(!binding_debug.contains("Account"));
                    assert!(!binding_debug.contains("token"));
                    job
                }
                _ => panic!("primary customer observes accepted work"),
            }
        }
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("primary customer retains its logical route")
        }
    };
    let primary_assignment = primary
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle primary receives its job"));
    let (_, primary_assignment, primary_receipt) = primary_assignment.into_parts();
    let primary_receipt: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Accepted(primary_receipt));
    let primary_busy = pool
        .transition(KeyedEvent::AssignmentSettled(primary_receipt))
        .unwrap_or_else(|error| panic!("primary receipt failed: {error}"));
    assert!(primary_busy.sends.customer_outcomes.is_empty());

    let replica = pool
        .receive(
            RuntimeAddr(7),
            KeyedCommand::submit(
                SubmissionId::new(2),
                Account(3),
                SearchJob(20),
                Recipient::global(RuntimeAddr(92)),
            ),
        )
        .unwrap_or_else(|error| panic!("replica assignment failed: {error}"));
    let replica_accepted = replica
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("replica customer observes admission"));
    let replica_job = match replica_accepted {
        CustomerDelivery::Logical { delivery } => {
            assert_eq!(delivery.to.address(), RuntimeAddr(92));
            match delivery.message {
                KeyedOutcome::Accepted { job, binding, .. } => {
                    assert_eq!(binding.role(), &SearchRole::Replica);
                    job
                }
                _ => panic!("replica customer observes accepted work"),
            }
        }
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("replica customer retains its logical route")
        }
    };
    let replica_assignment = replica
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("idle replica receives its job"));
    let (_, replica_assignment, replica_receipt) = replica_assignment.into_parts();
    let replica_receipt: ActionItemResult<AssignWorker<SearchWorker, SearchJob>> =
        SettledItem::Attempted(ItemSettlement::Accepted(replica_receipt));
    let both_busy = pool
        .transition(KeyedEvent::AssignmentSettled(replica_receipt))
        .unwrap_or_else(|error| panic!("replica receipt failed: {error}"));
    assert!(both_busy.sends.customer_outcomes.is_empty());

    let replica_completed = pool
        .on(behavior_actors::ChildReport::new(
            workers[1],
            replica_assignment.complete(SearchResult(120)).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("replica completion failed: {error}"));
    assert!(replica_completed.sends.diagnostics.is_empty());
    let replica_outcome = replica_completed
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("replica completion reaches its customer"));
    match replica_outcome {
        CustomerDelivery::Logical { delivery } => {
            assert_eq!(delivery.to.address(), RuntimeAddr(92));
            match delivery.message {
                KeyedOutcome::Completed {
                    job,
                    binding,
                    worker_result,
                } => {
                    assert_eq!(job, replica_job);
                    assert_eq!(binding.role(), &SearchRole::Replica);
                    assert_eq!(worker_result, SearchResult(120));
                }
                _ => panic!("replica customer observes completed work"),
            }
        }
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("replica completion retains its logical customer")
        }
    }

    let primary_completed = pool
        .on(behavior_actors::ChildReport::new(
            workers[0],
            primary_assignment.complete(SearchResult(110)).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("primary completion failed: {error}"));
    assert!(primary_completed.sends.diagnostics.is_empty());
    let primary_outcome = primary_completed
        .sends
        .customer_outcomes
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("primary completion reaches its customer"));
    match primary_outcome {
        CustomerDelivery::Logical { delivery } => {
            assert_eq!(delivery.to.address(), RuntimeAddr(91));
            match delivery.message {
                KeyedOutcome::Completed {
                    job,
                    binding,
                    worker_result,
                } => {
                    assert_eq!(job, primary_job);
                    assert_eq!(binding.role(), &SearchRole::Primary);
                    assert_eq!(worker_result, SearchResult(110));
                }
                _ => panic!("primary customer observes completed work"),
            }
        }
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("primary completion retains its logical customer")
        }
    }
}
