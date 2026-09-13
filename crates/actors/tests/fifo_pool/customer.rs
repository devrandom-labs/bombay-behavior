use std::time::Instant;

use behavior_actors::{Delivery, EstablishedDelivery};

use super::direct_pool_customer::{
    CustomerDesk, DeskInput, DeskJob, DeskNotice, DeskReturn, WorkEnding, input_orders,
};
use super::{
    AssignedReturnReason, BacklogCapacity, ChildReport, ChildStopped, Exit, FifoCommand, FifoEvent,
    FifoOutcome, FifoOutcomeKind, Interruption, ItemSettlement, MessageProtocol, Never,
    OrderedRoles, PoolFailureReaction, PoolRecovery, Recipient, ReplyDelivery, Role, RuntimeAddr,
    SettledItem, SubmissionId, ready_search_pool,
};

fn fifo_customer_notice(
    delivery: ReplyDelivery<
        Delivery<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>,
        EstablishedDelivery<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>,
    >,
) -> DeskNotice {
    let delivery = match delivery {
        ReplyDelivery::Logical(delivery) => delivery,
        ReplyDelivery::Established(_) => panic!("the model customer has one logical route"),
    };
    let customer = delivery.to.address().0;
    let outcome = delivery.message;
    match outcome.kind() {
        FifoOutcomeKind::Accepted => {
            let (submission, job) = outcome
                .into_accepted()
                .unwrap_or_else(|_| panic!("accepted outcome retains its identifiers"));
            DeskNotice::Accepted {
                customer,
                submission: submission.get(),
                job: job.get(),
            }
        }
        FifoOutcomeKind::Completed => {
            assert_eq!(outcome.role(), Some(&Role::Search));
            let (job, worker_result) = outcome
                .into_completed()
                .unwrap_or_else(|_| panic!("completed outcome retains its result"));
            DeskNotice::Completed {
                customer,
                job: job.get(),
                worker_result,
            }
        }
        FifoOutcomeKind::ReturnedAssigned => {
            assert_eq!(outcome.role(), Some(&Role::Search));
            let (job, payload, reason) = outcome
                .into_returned_assigned()
                .unwrap_or_else(|_| panic!("assigned return retains its job"));
            DeskNotice::Returned {
                customer,
                job: job.get(),
                payload,
                reason: match reason {
                    AssignedReturnReason::WorkerStopped => DeskReturn::WorkerExited,
                    AssignedReturnReason::PoolShutdown => DeskReturn::PoolClosed,
                    AssignedReturnReason::RetryPreparationRejected
                    | AssignedReturnReason::ContradictoryAssignmentSettlement => {
                        panic!("the selected scenario cannot produce this return")
                    }
                },
            }
        }
        FifoOutcomeKind::Rejected | FifoOutcomeKind::ReturnedQueued => {
            panic!("one ready worker accepts and assigns the model job")
        }
    }
}

#[tokio::test]
async fn customer_desk_matches_every_assignment_exit_and_shutdown_order() {
    for order in input_orders() {
        let roles = OrderedRoles::new(Role::Search, [])
            .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
        let ready = ready_search_pool(
            roles,
            BacklogCapacity::new(0),
            Interruption::Fail,
            PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole),
        )
        .await;
        let mut pool = ready.pool;
        let worker = ready.workers[0];
        let customer =
            Recipient::<MessageProtocol<RuntimeAddr, FifoOutcome<Role, u8, u16>>>::global(
                RuntimeAddr(88),
            );
        let submitted = pool
            .receive(
                RuntimeAddr(7),
                FifoCommand::submit(SubmissionId::new(40), 29, customer),
            )
            .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
        let initial = submitted
            .sends
            .customer_outcomes
            .into_deliveries()
            .pop()
            .map(fifo_customer_notice)
            .unwrap_or_else(|| panic!("accepted job emits one receipt"));
        let assignment = submitted
            .sends
            .worker_assignments
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
        let receipt = assignment.receipt();
        let (_, assignment, _) = assignment.into_parts();
        let completion = ChildReport::new(worker, assignment.complete(58).into_inner());
        let job = DeskJob {
            customer: 88,
            submission: 40,
            job: 1,
            payload: 29,
        };
        let (mut desk, accepted) = CustomerDesk::accepted(job);
        let mut expected = vec![accepted];
        let mut observed = vec![initial];
        let mut receipt = Some(receipt);
        let mut completion = Some(completion);

        for input in order {
            let (next, notice) = desk.apply(input);
            desk = next;
            if let Some(notice) = notice {
                expected.push(notice);
            }
            let acted =
                match input {
                    DeskInput::DeliveryAccepted => pool
                        .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
                            ItemSettlement::Accepted(receipt.take().unwrap_or_else(|| {
                                panic!("each order accepts delivery exactly once")
                            })),
                        )))
                        .unwrap_or_else(|error| panic!("assignment settlement failed: {error}")),
                    DeskInput::WorkEnded(WorkEnding::Completed) => pool
                        .on(completion.take().unwrap_or_else(|| {
                            panic!("each order completes the assignment exactly once")
                        }))
                        .unwrap_or_else(|error| panic!("completion input failed: {error}")),
                    DeskInput::WorkEnded(WorkEnding::WorkerExited) => pool
                        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
                        .unwrap_or_else(|error| panic!("worker exit input failed: {error}")),
                    DeskInput::Shutdown => pool
                        .receive(RuntimeAddr(7), FifoCommand::shutdown())
                        .unwrap_or_else(|error| panic!("FIFO shutdown failed: {error}")),
                };
            let deliveries = acted.sends.customer_outcomes.into_deliveries();
            observed.extend(deliveries.into_iter().map(fifo_customer_notice));
            assert_eq!(observed, expected, "customer trace differs after {order:?}");
        }
    }
}
