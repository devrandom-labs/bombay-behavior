use std::time::Instant;

use behavior::{
    ChildCreationOutcome, ChildHead, ChildReport, CreateChild, CreationSettlement,
    CreationsSettled, EstablishedCreation, EstablishedRecipient, ItemSettlement, MessageProtocol,
    Recipient, SettledItem,
};
use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, BacklogCapacity, BindingCapacity, CustomerDelivery,
    DiagnosticDisposition, Interruption, KeyedAssignedReturnReason, KeyedCommand, KeyedEvent,
    KeyedOutcome, OrderedRoles, PoolFailureReaction, PoolRecovery, SubmissionId,
    WorkerInitializationOutcome, keyed,
};
use behavior_actors::{Activate, ChildStopped, Exit, StopOnShutdown};

use super::direct_pool_customer::{
    CustomerDesk, DeskInput, DeskJob, DeskNotice, DeskReturn, WorkEnding, input_orders,
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

fn keyed_customer_notice(
    delivery: CustomerDelivery<
        MessageProtocol<RuntimeAddr, KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>>,
    >,
) -> (DeskNotice, behavior_actors::atomic::BindingGeneration) {
    let delivery = match delivery {
        CustomerDelivery::Logical { delivery } => delivery,
        CustomerDelivery::Established { .. }
        | CustomerDelivery::RejectedLogical { .. }
        | CustomerDelivery::RejectedEstablished { .. } => {
            panic!("the model customer has one logical route")
        }
    };
    let customer = delivery.to.address().0;
    match delivery.message {
        KeyedOutcome::Accepted {
            submission,
            job,
            binding,
        } => {
            assert_eq!(binding.role(), &SearchRole::Primary);
            (
                DeskNotice::Accepted {
                    customer,
                    submission: submission.get(),
                    job: job.get(),
                },
                binding.generation().clone(),
            )
        }
        KeyedOutcome::Completed {
            job,
            binding,
            worker_result,
        } => {
            assert_eq!(binding.role(), &SearchRole::Primary);
            (
                DeskNotice::Completed {
                    customer,
                    job: job.get(),
                    worker_result: worker_result.0,
                },
                binding.generation().clone(),
            )
        }
        KeyedOutcome::ReturnedAssigned {
            job,
            binding,
            payload,
            reason,
        } => {
            assert_eq!(binding.role(), &SearchRole::Primary);
            let reason = match reason {
                KeyedAssignedReturnReason::WorkerStopped => DeskReturn::WorkerExited,
                KeyedAssignedReturnReason::PoolShutdown => DeskReturn::PoolClosed,
                KeyedAssignedReturnReason::RolePermanentlyUnavailable
                | KeyedAssignedReturnReason::AssignmentReturned
                | KeyedAssignedReturnReason::RetryPreparationRejected
                | KeyedAssignedReturnReason::ContradictoryAssignmentSettlement => {
                    panic!("the selected scenario cannot produce this return")
                }
            };
            (
                DeskNotice::Returned {
                    customer,
                    job: job.get(),
                    payload: payload.0,
                    reason,
                },
                binding.generation().clone(),
            )
        }
        KeyedOutcome::Rejected { .. } | KeyedOutcome::ReturnedQueued { .. } => {
            panic!("one ready keyed worker accepts and assigns the model job")
        }
    }
}

#[tokio::test]
async fn customer_desk_matches_every_keyed_assignment_exit_and_shutdown_order() {
    for order in input_orders() {
        let roles = OrderedRoles::new(SearchRole::Primary, [])
            .unwrap_or_else(|_| panic!("one role is a valid keyed roster"));
        let pool = keyed(
            prepare_worker,
            roles,
            |_: &Account| SearchRole::Primary,
            ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
            PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            BacklogCapacity::new(0),
            BindingCapacity::new(1).unwrap_or_else(|_| panic!("one binding is valid")),
            Interruption::Fail,
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
            .unwrap_or_else(|| panic!("one role creates one worker"));
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
        assert!(started.sends.worker_assignments.is_empty());
        let ready = pool
            .on(activation.activate().await)
            .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
        assert!(ready.sends.worker_assignments.is_empty());

        let customer = Recipient::<
            MessageProtocol<
                RuntimeAddr,
                KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>,
            >,
        >::global(RuntimeAddr(88));
        let submitted = pool
            .receive(
                RuntimeAddr(7),
                KeyedCommand::submit(SubmissionId::new(40), Account(2), SearchJob(29), customer),
            )
            .unwrap_or_else(|error| panic!("keyed submission failed: {error}"));
        let (initial, binding) = submitted
            .sends
            .customer_outcomes
            .into_requests()
            .pop()
            .map(keyed_customer_notice)
            .unwrap_or_else(|| panic!("accepted job emits one receipt"));
        let assignment = submitted
            .sends
            .worker_assignments
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("ready worker receives the accepted job"));
        let receipt = assignment.receipt();
        let (_, assignment, _) = assignment.into_parts();
        let completion =
            ChildReport::new(worker, assignment.complete(SearchResult(58)).into_inner());
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
                        .transition(KeyedEvent::AssignmentSettled(SettledItem::Attempted(
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
                        .receive(RuntimeAddr(7), KeyedCommand::shutdown())
                        .unwrap_or_else(|error| panic!("KeyedPool shutdown failed: {error}")),
                };
            let deliveries = acted.sends.customer_outcomes.into_requests();
            for delivery in deliveries {
                let (notice, current_binding) = keyed_customer_notice(delivery);
                assert_eq!(current_binding, binding);
                observed.push(notice);
            }
            assert_eq!(observed, expected, "customer trace differs after {order:?}");
        }
    }
}
