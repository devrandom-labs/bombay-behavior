use core::ops::ControlFlow;

use behavior_actors::atomic::{
    AssignWorker, BacklogCapacity, DiagnosticAction, DiagnosticDisposition, FifoCommand, FifoEvent,
    FifoOutcome, Interruption, OrderedRoles, PoolFailureReaction, PoolRecovery, RestartLimit,
    RestartRelease, SubmissionId, WorkerSubmission, fifo,
};
use behavior_actors::{
    ActionItemResult, Activate as _, ChildNamespaceExhausted, ChildReport, ChildStopped,
    CreationSettlement, EstablishedRecipient, EstablishedShutdownResolved, ExactDeliveryReason,
    Exit, ItemSettlement, MessageProtocol, Never, Recipient, ReplyDelivery, SettledItem,
    ShutdownId, ShutdownRejection, Step, TimerElapsed, TimerScheduled,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use super::{
    Endpoint, Role, RuntimeAddr, SearchWorker, created_worker, prepare_search_worker,
    ready_search_pool,
};

#[tokio::test]
async fn creation_batch_requires_declared_worker_order() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        super::ActivationPolicy::new(2)
            .unwrap_or_else(|_| panic!("two activations are valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut ordered_pool = initialized.behavior;
    let mut creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let CreationSettlement::Settled(first) = created_worker(creations.remove(0)).into_settlement()
    else {
        panic!("the test runtime commits the first worker")
    };
    let CreationSettlement::Settled(second) = created_worker(creations.remove(0)).into_settlement()
    else {
        panic!("the test runtime commits the second worker")
    };
    let first = first
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the first worker settlement exists"));
    let second = second
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the second worker settlement exists"));
    let accepted = ordered_pool
        .on(super::CreationsSettled::new(CreationSettlement::Settled(
            [first, second].into_iter().collect(),
        )))
        .unwrap_or_else(|error| panic!("declared creation order failed: {error}"));
    assert_eq!(accepted.sends.worker_initializations.len(), 2);
    assert!(accepted.sends.diagnostics.is_empty());

    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        super::ActivationPolicy::new(2)
            .unwrap_or_else(|_| panic!("two activations are valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs the pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut reversed_pool = initialized.behavior;
    let mut creations: Vec<_> = initialized.actions.creates.into_iter().collect();
    let CreationSettlement::Settled(first) = created_worker(creations.remove(0)).into_settlement()
    else {
        panic!("the test runtime commits the first worker")
    };
    let CreationSettlement::Settled(second) = created_worker(creations.remove(0)).into_settlement()
    else {
        panic!("the test runtime commits the second worker")
    };
    let first = first
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the first worker settlement exists"));
    let second = second
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the second worker settlement exists"));
    let rejected = reversed_pool
        .on(super::CreationsSettled::new(CreationSettlement::Settled(
            [second, first].into_iter().collect(),
        )))
        .unwrap_or_else(|error| panic!("reversed creation input failed: {error}"));
    assert!(rejected.sends.worker_initializations.is_empty());
    assert_eq!(rejected.sends.diagnostics.len(), 1);
}

#[tokio::test]
async fn completed_work_advances_and_wraps_worker_selection() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let super::ReadySearchPool { mut pool, workers } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );

    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(1), 11, customer),
        )
        .unwrap_or_else(|error| panic!("first FIFO submission failed: {error}"));
    let first = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the first worker receives the first job"));
    assert_eq!(first.target(), EstablishedRecipient::issued(Endpoint(41)));
    let receipt = first.receipt();
    let (_, assignment, _) = first.into_parts();
    let accepted = pool
        .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(receipt),
        )))
        .unwrap_or_else(|error| panic!("first assignment settlement failed: {error}"));
    assert!(accepted.sends.customer_outcomes.as_slice().is_empty());
    let completed = pool
        .on(ChildReport::new(
            workers[0],
            assignment.complete(101).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("first completion failed: {error}"));
    assert_eq!(completed.sends.customer_outcomes.as_slice().len(), 1);

    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(2), 12, customer),
        )
        .unwrap_or_else(|error| panic!("second FIFO submission failed: {error}"));
    let second = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the second worker receives the second job"));
    assert_eq!(second.target(), EstablishedRecipient::issued(Endpoint(42)));
    let receipt = second.receipt();
    let (_, assignment, _) = second.into_parts();
    let accepted = pool
        .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(receipt),
        )))
        .unwrap_or_else(|error| panic!("second assignment settlement failed: {error}"));
    assert!(accepted.sends.customer_outcomes.as_slice().is_empty());
    let completed = pool
        .on(ChildReport::new(
            workers[1],
            assignment.complete(102).into_inner(),
        ))
        .unwrap_or_else(|error| panic!("second completion failed: {error}"));
    assert_eq!(completed.sends.customer_outcomes.as_slice().len(), 1);

    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(3), 13, customer),
        )
        .unwrap_or_else(|error| panic!("third FIFO submission failed: {error}"));
    let third = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("worker selection wraps to the first worker"));
    assert_eq!(third.target(), EstablishedRecipient::issued(Endpoint(41)));
}

#[tokio::test]
async fn public_fifo_values_reveal_their_domain_payloads() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let super::ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(9), 27, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the ready worker receives the job"));
    let receipt = assignment.receipt();
    let (_, assignment, _) = assignment.into_parts();
    assert!(format!("{assignment:?}").contains("payload: 27"));
    let worker = workers
        .pop()
        .unwrap_or_else(|| panic!("the ready worker exists"));
    let mut foreign_creations = super::CreationSequence::new();
    let _occupied = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("the worker creation exists"));
    let foreign_worker = foreign_creations
        .issue()
        .unwrap_or_else(|| panic!("a foreign creation exists"));
    let foreign = pool
        .on(ChildStopped::new(
            foreign_worker,
            Ok(Exit::Normal),
            std::time::Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("foreign worker stop failed: {error}"));
    let diagnostic = foreign
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("foreign input emits one diagnostic"));
    let DiagnosticAction::Terminal { diagnostic } = diagnostic;
    assert_eq!(format!("{diagnostic:?}"), "FifoDiagnostic { .. }");

    let completion = assignment.complete(103).into_inner();
    assert!(format!("{completion:?}").contains("worker_result: 103"));
    let accepted: ActionItemResult<super::AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
    let waiting = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .unwrap_or_else(|error| panic!("assignment settlement failed: {error}"));
    assert!(waiting.sends.customer_outcomes.as_slice().is_empty());
    let completed = pool
        .on(ChildReport::new(worker, completion))
        .unwrap_or_else(|error| panic!("completion failed: {error}"));
    assert_eq!(completed.sends.customer_outcomes.as_slice().len(), 1);

    let stopped = pool
        .on(ChildStopped::new(
            worker,
            Ok(Exit::Normal),
            std::time::Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("worker stop failed: {error}"));
    assert!(matches!(stopped.become_, Step::Continue));
    let rejected = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(10), 29, customer),
        )
        .unwrap_or_else(|error| panic!("unavailable FIFO submission failed: {error}"));
    let outcome = match rejected
        .sends
        .customer_outcomes
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("unavailable pool rejects the submission"))
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("the test customer route is logical"),
    };
    assert_eq!(outcome.payload(), Some(&29));
}

#[tokio::test]
async fn delayed_recoveries_select_the_exact_role_and_timer() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let super::ReadySearchPool {
        mut pool,
        mut workers,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::permanent(
            super::SearchSource,
            RestartLimit::new(3, Duration::from_secs(10)),
            RestartRelease::constant(Duration::from_secs(1))
                .unwrap_or_else(|error| panic!("positive restart delay rejected: {error}")),
            PoolFailureReaction::RetireRole,
        ),
    )
    .await;

    let first_preparation = super::stop_search_worker(&mut pool, workers.remove(0));
    let ControlFlow::Break(first_prepared) =
        first_preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let first_scheduling = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(first_prepared),
        )))
        .unwrap_or_else(|error| panic!("first worker preparation failed: {error}"));
    let first_schedule = first_scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("first delayed replacement requests a timer"));

    let second_preparation = super::stop_search_worker(&mut pool, workers.remove(0));
    let ControlFlow::Break(second_prepared) =
        second_preparation.accept(WorkerSubmission::immediate(SearchWorker))
    else {
        panic!("one selected role completes in one preparation")
    };
    let second_scheduling = pool
        .transition(FifoEvent::WorkerPreparationSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(second_prepared),
        )))
        .unwrap_or_else(|error| panic!("second worker preparation failed: {error}"));
    let second_schedule = second_scheduling
        .sends
        .restart_schedules
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("second delayed replacement requests a timer"));

    let second_waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: second_schedule.id,
                generation: second_schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("second restart schedule failed: {error}"));
    assert!(second_waiting.sends.diagnostics.is_empty());
    let first_waiting = pool
        .transition(FifoEvent::RestartScheduleSettled(SettledItem::Attempted(
            ItemSettlement::Accepted(TimerScheduled {
                id: first_schedule.id,
                generation: first_schedule.generation,
            }),
        )))
        .unwrap_or_else(|error| panic!("first restart schedule failed: {error}"));
    assert!(first_waiting.sends.diagnostics.is_empty());

    let second_restart = pool
        .on(TimerElapsed::new(
            second_schedule.id,
            second_schedule.generation,
        ))
        .unwrap_or_else(|error| panic!("second restart timer failed: {error}"));
    assert_eq!(second_restart.creates.len(), 1);
    assert!(second_restart.sends.diagnostics.is_empty());
    let first_restart = pool
        .on(TimerElapsed::new(
            first_schedule.id,
            first_schedule.generation,
        ))
        .unwrap_or_else(|error| panic!("first restart timer failed: {error}"));
    assert_eq!(first_restart.creates.len(), 1);
    assert!(first_restart.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn quarantined_workers_accept_their_own_shutdown_receipts() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let super::ReadySearchPool {
        mut pool,
        workers: _,
    } = ready_search_pool(
        roles,
        BacklogCapacity::new(1),
        Interruption::Retry,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(20), 31, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let first_assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the first worker receives the job"));
    let first_rejected: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: first_assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let first_quarantined = pool
        .transition(FifoEvent::AssignmentSettled(first_rejected))
        .unwrap_or_else(|error| panic!("first assignment rejection failed: {error}"));
    let _first_shutdown = first_quarantined
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the first worker is quarantined"));
    let second_assignment = first_quarantined
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the retried job reaches the second worker"));
    let second_rejected: ActionItemResult<AssignWorker<SearchWorker, u8>> =
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: second_assignment,
            reason: ExactDeliveryReason::ClosedRecipient,
        });
    let second_quarantined = pool
        .transition(FifoEvent::AssignmentSettled(second_rejected))
        .unwrap_or_else(|error| panic!("second assignment rejection failed: {error}"));
    let second_shutdown = second_quarantined
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the second worker is quarantined"));

    let accepted = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            second_shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("second worker shutdown settlement failed: {error}"));
    assert!(accepted.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn draining_workers_accept_receipts_for_the_exact_worker() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let super::ReadySearchPool { mut pool, workers } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let mut shutdowns = draining.sends.worker_shutdowns.into_requests();
    let first_shutdown = shutdowns.remove(0);
    let second_shutdown = shutdowns.remove(0);

    let second_accepted = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            second_shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("second shutdown settlement failed: {error}"));
    assert!(second_accepted.sends.diagnostics.is_empty());
    let second_stopped = pool
        .on(ChildStopped::new(
            workers[1],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("second worker stop failed: {error}"));
    assert!(second_stopped.sends.diagnostics.is_empty());
    assert!(matches!(second_stopped.become_, Step::Continue));

    let first_rejected = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::rejected(
            first_shutdown.id,
            ShutdownRejection::AlreadyStopping,
        ))
        .unwrap_or_else(|error| panic!("first shutdown rejection failed: {error}"));
    assert!(first_rejected.sends.diagnostics.is_empty());
    let retired = pool
        .on(ChildStopped::new(
            workers[0],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("first worker stop failed: {error}"));
    assert_eq!(retired.sends.diagnostics.len(), 1);
    assert!(matches!(retired.become_, Step::Stop(_)));
}

#[test]
fn pending_creation_drain_accepts_only_the_exact_worker_stop() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        super::ActivationPolicy::new(1)
            .unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
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
    let exact_worker = creation.id();
    let mut creations = super::CreationSequence::new();
    let _occupied = creations
        .issue()
        .unwrap_or_else(|| panic!("the exact creation exists"));
    let foreign_worker = creations
        .issue()
        .unwrap_or_else(|| panic!("a foreign creation exists"));
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(draining.become_, Step::Continue));

    let foreign = pool
        .on(ChildStopped::new(
            foreign_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("foreign worker stop failed: {error}"));
    assert_eq!(foreign.sends.diagnostics.len(), 1);
    let exact = pool
        .on(ChildStopped::new(
            exact_worker,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("exact worker stop failed: {error}"));
    assert!(exact.sends.diagnostics.is_empty());
    assert!(matches!(exact.become_, Step::Continue));

    let retired = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("exact worker creation settlement failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert!(retired.sends.worker_initializations.is_empty());
    assert!(retired.sends.worker_shutdowns.is_empty());
    assert_eq!(retired.sends.diagnostics.len(), 1);
}

#[test]
fn creation_returned_during_drain_stops_the_uninitialized_worker() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        super::ActivationPolicy::new(1)
            .unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
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
    let worker = creation.id();
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(draining.become_, Step::Continue));

    let created = pool
        .on(created_worker(creation))
        .unwrap_or_else(|error| panic!("exact worker creation settlement failed: {error}"));
    assert!(matches!(created.become_, Step::Continue));
    assert!(created.sends.worker_initializations.is_empty());
    assert!(created.sends.diagnostics.is_empty());
    let shutdown = created
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the uninitialized worker receives exact shutdown"));

    let accepted = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("worker shutdown settlement failed: {error}"));
    assert!(accepted.sends.diagnostics.is_empty());
    let retired = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .unwrap_or_else(|error| panic!("worker stop failed: {error}"));
    assert!(matches!(retired.become_, Step::Stop(_)));
    assert_eq!(retired.sends.diagnostics.len(), 1);
}

#[test]
fn rejected_creation_during_drain_returns_the_activation_to_diagnostics() {
    let activation_drops = Arc::new(AtomicUsize::new(0));
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let mut activation = Some(super::HeldActivation(Arc::clone(&activation_drops)));
    let pool = fifo(
        move |_: &Role| {
            Ok::<_, Never>(WorkerSubmission::activated(
                SearchWorker,
                activation
                    .take()
                    .unwrap_or_else(|| panic!("one-role fixture prepares one worker")),
            ))
        },
        roles,
        super::ActivationPolicy::new(1)
            .unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
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
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert!(matches!(draining.become_, Step::Continue));

    let rejected = pool
        .on(super::CreationsSettled::new(CreationSettlement::Rejected {
            creations: [creation].into_iter().collect(),
            reason: ChildNamespaceExhausted,
        }))
        .unwrap_or_else(|error| panic!("worker creation rejection failed: {error}"));
    assert!(matches!(rejected.become_, Step::Stop(_)));
    assert_eq!(rejected.sends.diagnostics.len(), 1);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 0);
    drop(rejected);
    assert_eq!(activation_drops.load(Ordering::SeqCst), 1);
}

#[test]
fn initialization_stop_must_name_its_exact_worker() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        super::ActivationPolicy::new(1)
            .unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
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
    let mut creations = super::CreationSequence::new();
    let _occupied = creations
        .issue()
        .unwrap_or_else(|| panic!("the exact creation exists"));
    let foreign_worker = creations
        .issue()
        .unwrap_or_else(|| panic!("a foreign creation exists"));

    let foreign = pool
        .on(
            initialization.resolve(super::WorkerInitializationOutcome::Stopped(
                ChildStopped::new(foreign_worker, Ok(Exit::Normal), Instant::now()),
            )),
        )
        .unwrap_or_else(|error| panic!("foreign initialization stop failed: {error}"));
    assert_eq!(foreign.sends.diagnostics.len(), 1);
    assert!(foreign.sends.worker_activations.is_empty());
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    assert_eq!(draining.sends.worker_shutdowns.len(), 1);
    assert!(matches!(draining.become_, Step::Continue));
}

#[test]
fn initialization_stop_during_drain_must_name_its_exact_worker() {
    let roles = OrderedRoles::new(Role::Search, [])
        .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
    let pool = fifo(
        prepare_search_worker,
        roles,
        super::ActivationPolicy::new(1)
            .unwrap_or_else(|_| panic!("one activation is valid policy")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        super::ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<core::convert::Infallible>::terminate(),
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
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("the initializing worker receives shutdown"));
    let mut creations = super::CreationSequence::new();
    let _occupied = creations
        .issue()
        .unwrap_or_else(|| panic!("the exact creation exists"));
    let foreign_worker = creations
        .issue()
        .unwrap_or_else(|| panic!("a foreign creation exists"));

    let foreign = pool
        .on(
            initialization.resolve(super::WorkerInitializationOutcome::Stopped(
                ChildStopped::new(foreign_worker, Ok(Exit::Normal), Instant::now()),
            )),
        )
        .unwrap_or_else(|error| panic!("foreign initialization stop failed: {error}"));
    assert_eq!(foreign.sends.diagnostics.len(), 1);
    assert!(foreign.sends.worker_activations.is_empty());
    let settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("worker shutdown settlement failed: {error}"));
    assert!(settled.sends.diagnostics.is_empty());
}

#[tokio::test]
async fn stop_then_shutdown_receipt_still_selects_the_exact_worker() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let super::ReadySearchPool { mut pool, workers } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Fail,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let mut shutdowns = draining.sends.worker_shutdowns.into_requests();
    let first_shutdown = shutdowns.remove(0);
    let second_shutdown = shutdowns.remove(0);

    let first_stopped = pool
        .on(ChildStopped::new(
            workers[0],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("first worker stop failed: {error}"));
    assert!(first_stopped.sends.diagnostics.is_empty());
    let foreign = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            ShutdownId(99),
        ))
        .unwrap_or_else(|error| panic!("foreign shutdown settlement failed: {error}"));
    assert_eq!(foreign.sends.diagnostics.len(), 1);
    let second_settled = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            second_shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("second shutdown settlement failed: {error}"));
    assert!(second_settled.sends.diagnostics.is_empty());
    let second_stopped = pool
        .on(ChildStopped::new(
            workers[1],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("second worker stop failed: {error}"));
    assert!(second_stopped.sends.diagnostics.is_empty());
    let retired = pool
        .on(EstablishedShutdownResolved::<SearchWorker>::accepted(
            first_shutdown.id,
        ))
        .unwrap_or_else(|error| panic!("first shutdown settlement failed: {error}"));
    assert!(retired.sends.diagnostics.is_empty());
    assert!(matches!(retired.become_, Step::Stop(_)));
}

#[tokio::test]
async fn stopped_busy_worker_consumes_no_shutdown_identifier() {
    let roles = OrderedRoles::new(Role::Search, [Role::Index])
        .unwrap_or_else(|_| panic!("two distinct roles are a valid FIFO roster"));
    let super::ReadySearchPool { mut pool, workers } = ready_search_pool(
        roles,
        BacklogCapacity::new(0),
        Interruption::Retry,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
    )
    .await;
    let customer = Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddr(88),
    );
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(30), 41, customer),
        )
        .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
    let assignment = submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the first worker receives the job"));
    let stopped = pool
        .on(ChildStopped::new(
            workers[0],
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|error| panic!("busy worker stop failed: {error}"));
    assert!(stopped.sends.diagnostics.is_empty());

    let draining = pool
        .receive(RuntimeAddr(7), FifoCommand::shutdown())
        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}"));
    let shutdown = draining
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .unwrap_or_else(|| panic!("only the still-running worker receives shutdown"));
    assert_eq!(shutdown.id, ShutdownId(0));
    drop(assignment);
}
