#![no_main]
//! Unexpected worker exit through both retained-service policies.

use std::time::Instant;

use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, CancellationReceipt, DiagnosticAction,
    DiagnosticDisposition, DynamicCommand, DynamicDiagnostic, DynamicLifecycle, DynamicStatus,
    EntryCapacity, EntryRetirement, ImmediateActivation, InitialWorkerOutcome, ProxyInputReceipt,
    ProxyInputResult, ProxyOperationId, ProxyOutcome, StartRejection, UnexpectedExit,
    WorkerChangeRejection, WorkerStartResult, WorkerSubmission, dynamic,
};
use behavior_actors::{Activate as _, ChildStopped, Exit, ReplyDelivery, ReplyRoute};
use behavior_core::{
    ChildCreationOutcome, ChildReport, CreationId, CreationSettlement, CreationsSettled,
    EstablishedActor, EstablishedCreation, EstablishedRecipient, ItemSettlement, MessageProtocol,
    Recipient, SettledItem, Step,
};
use libfuzzer_sys::fuzz_target;

mod stable_proxy;

use stable_proxy::{RuntimeAddress, Worker, WorkerEndpoint, drive_ready_proxy, worker_stopped};

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum ServiceKey {
    Primary,
    CapacityProbe,
}

const fn exit_policy(byte: u8) -> UnexpectedExit {
    match byte % 2 {
        0 => UnexpectedExit::KeepEmpty,
        _ => UnexpectedExit::Retire,
    }
}

#[derive(Clone, Copy)]
enum RetirementArrival {
    Shutdown,
    ProxyExit,
}

const RETIREMENT_ORDERS: [[RetirementArrival; 2]; 2] = [
    [RetirementArrival::Shutdown, RetirementArrival::ProxyExit],
    [RetirementArrival::ProxyExit, RetirementArrival::Shutdown],
];

fn accepted_proxy_input(
    creation: CreationId,
    operation: ProxyOperationId,
) -> ProxyInputResult<behavior_core::Here, Worker, ImmediateActivation> {
    SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
        creation,
        EstablishedActor::issued(WorkerEndpoint),
        operation,
    )))
}

fuzz_target!(|input: &[u8]| {
    for byte in input.iter().copied().take(64) {
        let policy = exit_policy(byte);
        let lifecycle = Recipient::<
            MessageProtocol<
                RuntimeAddress,
                DynamicLifecycle<ServiceKey, Worker, ImmediateActivation>,
            >,
        >::global(RuntimeAddress);
        let diagnostics = Recipient::<
            MessageProtocol<
                RuntimeAddress,
                DynamicDiagnostic<ServiceKey, Worker, ImmediateActivation>,
            >,
        >::global(RuntimeAddress);
        let initialized = dynamic(
            EntryCapacity::new(1).expect("one entry is valid"),
            ActivationPolicy::new(1).expect("one activation is valid"),
            policy,
            ActorDrainPolicy::WaitForActorGraph,
            lifecycle,
            DiagnosticDisposition::deliver_to(diagnostics),
        )
        .initialize()
        .unwrap_or_else(|_| panic!("dynamic supervisor initialization is pure"));
        let mut supervisor = initialized.behavior;

        let starting = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Start {
                    key: ServiceKey::Primary,
                    submission: WorkerSubmission::immediate(Worker::new(byte)),
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("the primary service is admitted"));
        let authority = match starting
            .sends
            .start_replies
            .into_deliveries()
            .pop()
            .expect("accepted start returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                Ok(receipt) => receipt.cancel,
                Err(_) => panic!("the primary service is accepted"),
            },
            ReplyDelivery::Established(_) => panic!("logical start route remains logical"),
        };
        let created = starting
            .creates
            .into_iter()
            .next()
            .expect("the primary service creates one proxy");
        let (proxy_creation, proxy, proxy_kind) = created.into_parts();
        let proxy_settlement = SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    proxy_creation,
                    proxy_kind,
                    EstablishedRecipient::issued(WorkerEndpoint),
                ),
            },
        ));
        let committed = CreationsSettled::new(CreationSettlement::Settled(
            [proxy_settlement].into_iter().collect(),
        ));
        let proxy_input = supervisor
            .on(committed)
            .unwrap_or_else(|_| panic!("the committed proxy receives one worker input"))
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .expect("one worker input is emitted");
        let (input_creation, control, operation) = proxy_input.into_parts();
        assert_eq!(input_creation, proxy_creation);
        let input_accepted = supervisor
            .on(accepted_proxy_input(input_creation, operation))
            .unwrap_or_else(|_| panic!("the worker input is accepted"));
        assert!(input_accepted.creates.is_empty());
        assert!(input_accepted.sends.proxy_observations.is_empty());
        assert!(input_accepted.sends.proxy_operations.is_empty());
        assert!(input_accepted.sends.shutdown_schedules.is_empty());
        assert!(input_accepted.sends.start_replies.as_slice().is_empty());
        assert!(input_accepted.sends.replace_replies.as_slice().is_empty());
        assert!(input_accepted.sends.stop_replies.as_slice().is_empty());
        assert!(input_accepted.sends.query_replies.as_slice().is_empty());
        assert!(input_accepted.sends.cancel_replies.as_slice().is_empty());
        assert!(input_accepted.sends.lifecycle.is_empty());
        assert!(input_accepted.sends.diagnostics.is_empty());

        let (mut proxy, worker_creation, ready_outcome) = drive_ready_proxy(proxy, control);
        let worker = match &ready_outcome {
            ProxyOutcome::Initial {
                outcome:
                    InitialWorkerOutcome::Resolved {
                        result: WorkerStartResult::Ready { attempt, .. },
                    },
            } => attempt.clone(),
            _ => panic!("the actual proxy produces a ready initial worker"),
        };
        let ready = supervisor
            .on(ChildReport::new(proxy_creation, ready_outcome))
            .unwrap_or_else(|_| panic!("the exact ready outcome is accepted"));
        assert!(matches!(
            &ready.sends.lifecycle[0].message,
            DynamicLifecycle::Started {
                key: ServiceKey::Primary,
                generation: 1,
                ..
            }
        ));

        let proxy_stopped = proxy
            .on(worker_stopped(worker_creation))
            .unwrap_or_else(|_| panic!("the exact worker stop reaches its proxy"));
        let worker_stopped_outcome = proxy_stopped
            .sends
            .owner_outcomes
            .into_requests()
            .pop()
            .expect("the proxy reports one exact worker stop")
            .into_inner();
        let stopped = supervisor
            .on(ChildReport::new(proxy_creation, worker_stopped_outcome))
            .unwrap_or_else(|_| panic!("the supervisor accepts the exact worker stop"));
        assert_eq!(stopped.sends.lifecycle.len(), 1);
        assert!(matches!(
            &stopped.sends.lifecycle[0].message,
            DynamicLifecycle::UnexpectedWorkerStopped {
                key: ServiceKey::Primary,
                generation: 1,
                disposition,
                ..
            } if *disposition == policy
        ));
        assert!(stopped.creates.is_empty());

        let capacity_probe = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Start {
                    key: ServiceKey::CapacityProbe,
                    submission: WorkerSubmission::immediate(Worker::new(byte.wrapping_add(1))),
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("capacity probing is total"));
        let returned = match capacity_probe
            .sends
            .start_replies
            .into_deliveries()
            .pop()
            .expect("the capacity probe receives one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                Err(WorkerChangeRejection {
                    key: ServiceKey::CapacityProbe,
                    submission,
                    reason: StartRejection::AtCapacity,
                }) => submission,
                Ok(_) | Err(_) => panic!("both policies retain their entry at capacity"),
            },
            ReplyDelivery::Established(_) => panic!("logical start route remains logical"),
        };
        assert_eq!(
            returned,
            WorkerSubmission::immediate(Worker::new(byte.wrapping_add(1)))
        );
        assert!(capacity_probe.creates.is_empty());

        match policy {
            UnexpectedExit::KeepEmpty => {
                assert!(stopped.sends.proxy_operations.is_empty());
                let queried = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Query {
                            key: ServiceKey::Primary,
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("the retained service is queryable"));
                assert!(matches!(
                    &queried.sends.query_replies.as_slice()[0],
                    ReplyDelivery::Logical(delivery)
                        if matches!(delivery.message, behavior_actors::atomic::QueryReply::Known {
                            status: DynamicStatus::Empty,
                            ..
                        })
                ));

                let duplicate = supervisor
                    .on(ChildReport::new(
                        proxy_creation,
                        ProxyOutcome::WorkerStopped {
                            worker,
                            stopped: worker_stopped(worker_creation),
                        },
                    ))
                    .unwrap_or_else(|_| panic!("duplicate worker stop is returned to diagnostics"));
                assert!(duplicate.sends.lifecycle.is_empty());
                assert!(matches!(
                    &duplicate.sends.diagnostics[0],
                    DiagnosticAction::Deliver {
                        diagnostic: DynamicDiagnostic::RejectedProxyOutcome { .. },
                        ..
                    }
                ));
                let _retained_authority = authority;
            }
            UnexpectedExit::Retire => {
                let shutdown = stopped
                    .sends
                    .proxy_operations
                    .into_items()
                    .pop()
                    .expect("retirement shuts down the exact proxy");
                let (shutdown_creation, shutdown_control, shutdown_operation) =
                    shutdown.into_parts();
                assert_eq!(shutdown_creation, proxy_creation);
                let proxy_retired = proxy
                    .on(shutdown_control)
                    .unwrap_or_else(|_| panic!("the empty proxy accepts shutdown"));
                assert!(matches!(proxy_retired.become_, Step::Stop(_)));

                let mut shutdown =
                    Some(accepted_proxy_input(shutdown_creation, shutdown_operation));
                let mut proxy_exit = Some(ChildStopped::new(
                    proxy_creation,
                    Ok(Exit::Normal),
                    Instant::now(),
                ));
                let order = RETIREMENT_ORDERS[usize::from(byte / 2) % RETIREMENT_ORDERS.len()];
                let mut retired = None;
                for arrival in order {
                    let acted = match arrival {
                        RetirementArrival::Shutdown => supervisor
                            .on(shutdown.take().expect("shutdown settles once"))
                            .unwrap_or_else(|_| panic!("shutdown settlement is retained")),
                        RetirementArrival::ProxyExit => supervisor
                            .on(proxy_exit.take().expect("proxy exit arrives once"))
                            .unwrap_or_else(|_| panic!("proxy exit is retained")),
                    };
                    assert!(acted.sends.proxy_operations.is_empty());
                    if shutdown.is_none() && proxy_exit.is_none() {
                        assert_eq!(acted.sends.lifecycle.len(), 1);
                        retired = Some(acted.sends.lifecycle);
                    } else {
                        assert!(acted.sends.lifecycle.is_empty());
                    }
                }
                let lifecycle = retired.expect("the complete proxy retirement closes");
                assert!(matches!(
                    &lifecycle[0].message,
                    DynamicLifecycle::EntryRetired {
                        key: ServiceKey::Primary,
                        generation: 1,
                        cause: EntryRetirement::UnexpectedWorkerStopped,
                    }
                ));

                let restarted = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Start {
                            key: ServiceKey::Primary,
                            submission: WorkerSubmission::immediate(Worker::new(
                                byte.wrapping_add(2),
                            )),
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("retirement releases entry capacity"));
                let _current_authority = match restarted
                    .sends
                    .start_replies
                    .into_deliveries()
                    .pop()
                    .expect("the fresh start receives one reply")
                {
                    ReplyDelivery::Logical(delivery) => match delivery.message {
                        Ok(receipt) => receipt.cancel,
                        Err(_) => panic!("the removed key accepts a fresh start"),
                    },
                    ReplyDelivery::Established(_) => {
                        panic!("logical start route remains logical")
                    }
                };
                assert_eq!(restarted.creates.len(), 1);

                let stale = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Cancel {
                            authority,
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("old cancellation authority is returned"));
                let _old_authority = match stale
                    .sends
                    .cancel_replies
                    .into_deliveries()
                    .pop()
                    .expect("old authority receives one reply")
                {
                    ReplyDelivery::Logical(delivery) => match delivery.message {
                        CancellationReceipt::Stale { authority } => authority,
                        _ => panic!("same-key reuse cannot revive old authority"),
                    },
                    ReplyDelivery::Established(_) => {
                        panic!("logical cancel route remains logical")
                    }
                };

                let old_worker = supervisor
                    .on(ChildReport::new(
                        proxy_creation,
                        ProxyOutcome::WorkerStopped {
                            worker,
                            stopped: worker_stopped(worker_creation),
                        },
                    ))
                    .unwrap_or_else(|_| panic!("old worker report is returned to diagnostics"));
                assert!(old_worker.sends.lifecycle.is_empty());
                assert!(matches!(
                    &old_worker.sends.diagnostics[0],
                    DiagnosticAction::Deliver {
                        diagnostic: DynamicDiagnostic::RejectedProxyOutcome { .. },
                        ..
                    }
                ));
                let queried = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Query {
                            key: ServiceKey::Primary,
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("the fresh service remains queryable"));
                assert!(matches!(
                    &queried.sends.query_replies.as_slice()[0],
                    ReplyDelivery::Logical(delivery)
                        if matches!(delivery.message, behavior_actors::atomic::QueryReply::Known {
                            status: DynamicStatus::CreatingProxy,
                            ..
                        })
                ));
            }
        }
    }
});
