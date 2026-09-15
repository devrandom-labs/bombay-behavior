#![no_main]
//! Closed management and late proxy resolution during global shutdown.

use std::time::Instant;

use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, CancellationReceipt, DiagnosticAction,
    DiagnosticDisposition, DynamicCommand, DynamicDiagnostic, DynamicLifecycle, DynamicStatus,
    EntryCapacity, EntryRetirement, ImmediateActivation, InterruptedWorker, ReplaceRejection,
    StartRejection, StopRejection, UnexpectedExit, WorkerChange, WorkerChangeInterruption,
    WorkerSubmission, dynamic,
};
use behavior_actors::{
    Activate as _, ChildStopped, Exit, ReplyDelivery, ReplyRoute, ShutdownRequested,
};
use behavior_core::{MessageProtocol, Recipient, Step};
use libfuzzer_sys::fuzz_target;

mod dynamic_supervisor;
mod dynamic_supervisor_rejection;

use dynamic_supervisor::{RuntimeAddress, Worker, accepted_proxy_input, committed_proxy};
use dynamic_supervisor_rejection::rejected_proxy;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum ServiceKey {
    Primary,
    Probe,
}

#[derive(Clone, Copy)]
enum ClosedCommand {
    Shutdown,
    Query,
    Start,
    Replace,
    Stop,
    Cancel,
}

impl ClosedCommand {
    const fn from_byte(byte: u8) -> Self {
        match byte % 6 {
            0 => Self::Shutdown,
            1 => Self::Query,
            2 => Self::Start,
            3 => Self::Replace,
            4 => Self::Stop,
            _ => Self::Cancel,
        }
    }
}

#[derive(Clone, Copy)]
enum CreationResolution {
    Rejected,
    Committed([RetirementArrival; 2]),
}

#[derive(Clone, Copy)]
enum RetirementArrival {
    Shutdown,
    ProxyExit,
}

impl CreationResolution {
    const fn from_byte(byte: u8) -> Self {
        match byte % 3 {
            0 => Self::Rejected,
            1 => Self::Committed([RetirementArrival::Shutdown, RetirementArrival::ProxyExit]),
            _ => Self::Committed([RetirementArrival::ProxyExit, RetirementArrival::Shutdown]),
        }
    }
}

fuzz_target!(|input: &[u8]| {
    let lifecycle = Recipient::<
        MessageProtocol<RuntimeAddress, DynamicLifecycle<ServiceKey, Worker, ImmediateActivation>>,
    >::global(RuntimeAddress);
    let diagnostics = Recipient::<
        MessageProtocol<RuntimeAddress, DynamicDiagnostic<ServiceKey, Worker, ImmediateActivation>>,
    >::global(RuntimeAddress);
    let initialized = dynamic(
        EntryCapacity::new(1).expect("one entry is valid"),
        ActivationPolicy::new(1).expect("one activation is valid"),
        UnexpectedExit::KeepEmpty,
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
                submission: WorkerSubmission::immediate(Worker),
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
            Err(_) => panic!("the pending service is accepted"),
        },
        ReplyDelivery::Established(_) => panic!("logical start route remains logical"),
    };
    let created = starting
        .creates
        .into_iter()
        .next()
        .expect("start emits one pending proxy creation");

    let shutdown = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("global shutdown is total"));
    assert!(matches!(shutdown.become_, Step::Continue));
    assert_eq!(shutdown.sends.lifecycle.len(), 1);
    assert!(matches!(
        &shutdown.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            key: ServiceKey::Primary,
            generation: 1,
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Start,
            },
            worker: InterruptedWorker::Submission(_),
            ..
        }
    ));
    assert!(shutdown.creates.is_empty());
    assert!(shutdown.sends.proxy_operations.is_empty());
    let mut authority = Some(authority);

    for byte in input.iter().copied().skip(1).take(32) {
        match ClosedCommand::from_byte(byte) {
            ClosedCommand::Shutdown => {
                let repeated = supervisor
                    .on(ShutdownRequested)
                    .unwrap_or_else(|_| panic!("repeated shutdown is idempotent"));
                assert!(matches!(repeated.become_, Step::Continue));
                assert!(repeated.creates.is_empty());
                assert!(repeated.sends.proxy_operations.is_empty());
                assert!(repeated.sends.lifecycle.is_empty());
            }
            ClosedCommand::Query => {
                let queried = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Query {
                            key: ServiceKey::Primary,
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("query remains available while draining"));
                assert!(matches!(
                    &queried.sends.query_replies.as_slice()[0],
                    ReplyDelivery::Logical(delivery)
                        if matches!(delivery.message, behavior_actors::atomic::QueryReply::Known {
                            status: DynamicStatus::Draining,
                            ..
                        })
                ));
                assert!(queried.creates.is_empty());
            }
            ClosedCommand::Start => {
                let rejected = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Start {
                            key: ServiceKey::Probe,
                            submission: WorkerSubmission::immediate(Worker),
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("closed start returns its worker"));
                assert!(matches!(
                    &rejected.sends.start_replies.as_slice()[0],
                    ReplyDelivery::Logical(delivery)
                        if matches!(&delivery.message, Err(rejection)
                            if rejection.reason == StartRejection::ShuttingDown
                                && rejection.key == ServiceKey::Probe
                                && rejection.submission == WorkerSubmission::immediate(Worker))
                ));
                assert!(rejected.creates.is_empty());
            }
            ClosedCommand::Replace => {
                let rejected = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Replace {
                            key: ServiceKey::Primary,
                            submission: WorkerSubmission::immediate(Worker),
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("closed replacement returns its worker"));
                assert!(matches!(
                    &rejected.sends.replace_replies.as_slice()[0],
                    ReplyDelivery::Logical(delivery)
                        if matches!(&delivery.message, Err(rejection)
                            if rejection.reason == ReplaceRejection::ShuttingDown
                                && rejection.key == ServiceKey::Primary
                                && rejection.submission == WorkerSubmission::immediate(Worker))
                ));
                assert!(rejected.creates.is_empty());
            }
            ClosedCommand::Stop => {
                let rejected = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Stop {
                            key: ServiceKey::Primary,
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("closed stop returns its key"));
                assert!(matches!(
                    &rejected.sends.stop_replies.as_slice()[0],
                    ReplyDelivery::Logical(delivery)
                        if matches!(&delivery.message, Err(StopRejection::ShuttingDown {
                            key: ServiceKey::Primary,
                        }))
                ));
                assert!(rejected.creates.is_empty());
            }
            ClosedCommand::Cancel => {
                let cancelled = supervisor
                    .receive(
                        RuntimeAddress,
                        DynamicCommand::Cancel {
                            authority: authority.take().expect("caller retains the authority"),
                            reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("cancellation reports drain ownership"));
                authority = Some(
                    match cancelled
                        .sends
                        .cancel_replies
                        .into_deliveries()
                        .pop()
                        .expect("cancellation receives one reply")
                    {
                        ReplyDelivery::Logical(delivery) => match delivery.message {
                            CancellationReceipt::Draining { authority } => authority,
                            _ => panic!("shutdown retains the matching operation"),
                        },
                        ReplyDelivery::Established(_) => {
                            panic!("logical cancel route remains logical")
                        }
                    },
                );
                assert!(cancelled.creates.is_empty());
            }
        }
    }
    let _authority = authority.expect("the caller still owns cancellation authority");

    match CreationResolution::from_byte(input.first().copied().unwrap_or(0)) {
        CreationResolution::Rejected => {
            let retired = supervisor
                .on(rejected_proxy(created))
                .unwrap_or_else(|_| panic!("late creation rejection is retained"));
            assert!(matches!(retired.become_, Step::Stop(_)));
            assert!(matches!(
                &retired.sends.lifecycle[0].message,
                DynamicLifecycle::EntryRetired {
                    cause: EntryRetirement::Shutdown,
                    ..
                }
            ));
            assert!(matches!(
                &retired.sends.diagnostics[0],
                DiagnosticAction::Deliver {
                    diagnostic: DynamicDiagnostic::ProxyCreationRejected { .. },
                    ..
                }
            ));
        }
        CreationResolution::Committed(order) => {
            let (creation, committed) = committed_proxy(created);
            let late_birth = supervisor
                .on(committed)
                .unwrap_or_else(|_| panic!("late proxy creation is retained"));
            assert!(matches!(late_birth.become_, Step::Continue));
            let shutdown = late_birth
                .sends
                .proxy_operations
                .into_items()
                .pop()
                .expect("late proxy is shut down once");
            let (shutdown_creation, _control, shutdown_operation) = shutdown.into_parts();
            assert_eq!(shutdown_creation, creation);
            let mut settled = Some(accepted_proxy_input(shutdown_creation, shutdown_operation));
            let mut exited = Some(ChildStopped::new(
                creation,
                Ok(Exit::Normal),
                Instant::now(),
            ));
            let mut retired = None;
            for arrival in order {
                let acted = match arrival {
                    RetirementArrival::Shutdown => supervisor
                        .on(settled.take().expect("shutdown settles once"))
                        .unwrap_or_else(|_| panic!("shutdown settlement is retained")),
                    RetirementArrival::ProxyExit => supervisor
                        .on(exited.take().expect("proxy exit arrives once"))
                        .unwrap_or_else(|_| panic!("proxy exit is retained")),
                };
                if settled.is_none() && exited.is_none() {
                    assert!(matches!(acted.become_, Step::Stop(_)));
                    retired = Some(acted.sends.lifecycle);
                } else {
                    assert!(matches!(acted.become_, Step::Continue));
                    assert!(acted.sends.lifecycle.is_empty());
                }
            }
            let retired = retired.expect("exact proxy retirement closes shutdown");
            assert!(matches!(
                &retired[0].message,
                DynamicLifecycle::EntryRetired {
                    cause: EntryRetirement::Shutdown,
                    ..
                }
            ));
        }
    }
});
