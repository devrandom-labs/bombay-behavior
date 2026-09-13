#![no_main]
//! Cancellation, proxy retirement, and same-key reuse through DynamicSupervisor.

use std::time::Instant;

use behavior::atomic::{
    ActivationPolicy, ActorDrainPolicy, CancelAuthority, CancellationOutcome, CancellationReceipt,
    DiagnosticDisposition, DynamicCommand, DynamicDiagnostic, DynamicLifecycle, DynamicStatus,
    EntryCapacity, EntryRetirement, ImmediateActivation, StartRejection, UnexpectedExit,
    WorkerChange, WorkerSubmission, dynamic,
};
use behavior::{
    Activate as _, ChildStopped, Exit, MessageProtocol, Recipient, ReplyDelivery, ReplyRoute, Step,
};
use libfuzzer_sys::fuzz_target;

mod dynamic_supervisor;
mod dynamic_supervisor_rejection;

use dynamic_supervisor::{RuntimeAddress, Worker, accepted_proxy_input, committed_proxy};
use dynamic_supervisor_rejection::rejected_proxy;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum ServiceKey {
    Primary,
    CapacityProbe,
}

#[derive(Clone, Copy)]
enum CreationResult {
    Rejected,
    ShutdownReceiptFirst,
    ProxyExitFirst,
}

impl CreationResult {
    const fn from_byte(byte: u8) -> Self {
        match byte % 3 {
            0 => Self::Rejected,
            1 => Self::ShutdownReceiptFirst,
            _ => Self::ProxyExitFirst,
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
    let mut old_authority: Option<CancelAuthority<ServiceKey>> = None;
    let mut last_generation: Option<u64> = None;

    for byte in input.iter().copied().take(64) {
        let started = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Start {
                    key: ServiceKey::Primary,
                    submission: WorkerSubmission::immediate(Worker),
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("retired entry releases its key and capacity"));
        assert_eq!(started.creates.len(), 1);
        assert_eq!(started.sends.proxy_observations.len(), 1);
        assert!(started.sends.proxy_operations.is_empty());
        assert_eq!(started.sends.start_replies.as_slice().len(), 1);
        assert!(started.sends.lifecycle.is_empty());
        let created = started
            .creates
            .into_iter()
            .next()
            .expect("accepted start creates one stable proxy");
        let authority = match started
            .sends
            .start_replies
            .into_deliveries()
            .pop()
            .expect("accepted start returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                Ok(receipt) => receipt.cancel,
                Err(_) => panic!("accepted start returns cancellation authority"),
            },
            ReplyDelivery::Established(_) => panic!("logical start route remains logical"),
        };

        if let Some(authority) = old_authority.take() {
            let stale = supervisor
                .receive(
                    RuntimeAddress,
                    DynamicCommand::Cancel {
                        authority,
                        reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                    },
                )
                .unwrap_or_else(|_| panic!("old generation cancellation is a total reply"));
            assert!(stale.creates.is_empty());
            assert!(stale.sends.proxy_operations.is_empty());
            assert!(stale.sends.lifecycle.is_empty());
            match stale
                .sends
                .cancel_replies
                .into_deliveries()
                .pop()
                .expect("stale cancellation returns one reply")
            {
                ReplyDelivery::Logical(delivery) => match delivery.message {
                    CancellationReceipt::Stale { authority } => {
                        drop(authority);
                    }
                    _ => panic!("same-key reuse cannot revive an old authority"),
                },
                ReplyDelivery::Established(_) => {
                    panic!("logical cancellation route remains logical")
                }
            }
            let queried = supervisor
                .receive(
                    RuntimeAddress,
                    DynamicCommand::Query {
                        key: ServiceKey::Primary,
                        reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                    },
                )
                .unwrap_or_else(|_| panic!("query remains total after stale cancellation"));
            assert!(matches!(
                &queried.sends.query_replies.as_slice()[0],
                ReplyDelivery::Logical(delivery)
                    if matches!(
                        delivery.message,
                        behavior::atomic::QueryReply::Known {
                            status: DynamicStatus::CreatingProxy,
                            ..
                        }
                    )
            ));
        }

        let cancelled = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("local worker cancellation returns its submission"));
        assert!(cancelled.creates.is_empty());
        assert!(cancelled.sends.proxy_operations.is_empty());
        assert!(cancelled.sends.lifecycle.is_empty());
        let authority = match cancelled
            .sends
            .cancel_replies
            .into_deliveries()
            .pop()
            .expect("cancellation returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                CancellationReceipt::Returned { authority, .. } => authority,
                _ => panic!("pre-transfer cancellation returns the worker"),
            },
            ReplyDelivery::Established(_) => panic!("logical cancellation route remains logical"),
        };
        let replayed = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("current cancellation replay is total"));
        assert!(replayed.creates.is_empty());
        assert!(replayed.sends.proxy_operations.is_empty());
        assert!(replayed.sends.lifecycle.is_empty());
        let authority = match replayed
            .sends
            .cancel_replies
            .into_deliveries()
            .pop()
            .expect("current cancellation returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                CancellationReceipt::Cancelled { authority } => authority,
                _ => panic!("current cancelled operation stays cancelled"),
            },
            ReplyDelivery::Established(_) => panic!("logical cancellation route remains logical"),
        };

        let blocked = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Start {
                    key: ServiceKey::CapacityProbe,
                    submission: WorkerSubmission::immediate(Worker),
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("capacity rejection is a total reply"));
        assert!(blocked.creates.is_empty());
        assert!(blocked.sends.proxy_observations.is_empty());
        assert!(blocked.sends.proxy_operations.is_empty());
        assert!(blocked.sends.shutdown_schedules.is_empty());
        assert!(blocked.sends.replace_replies.as_slice().is_empty());
        assert!(blocked.sends.stop_replies.as_slice().is_empty());
        assert!(blocked.sends.query_replies.as_slice().is_empty());
        assert!(blocked.sends.cancel_replies.as_slice().is_empty());
        assert!(blocked.sends.lifecycle.is_empty());
        assert!(blocked.sends.diagnostics.is_empty());
        match blocked
            .sends
            .start_replies
            .into_deliveries()
            .pop()
            .expect("rejected start returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                Err(rejection) => {
                    assert!(matches!(rejection.key, ServiceKey::CapacityProbe));
                    assert_eq!(rejection.reason, StartRejection::AtCapacity);
                    assert_eq!(rejection.submission, WorkerSubmission::immediate(Worker));
                }
                Ok(_) => panic!("cancelling entry retains the only capacity slot"),
            },
            ReplyDelivery::Established(_) => panic!("logical start route remains logical"),
        }

        let terminal = match CreationResult::from_byte(byte) {
            CreationResult::Rejected => supervisor
                .on(rejected_proxy(created))
                .unwrap_or_else(|_| panic!("creation rejection retires the cancelled entry")),
            CreationResult::ShutdownReceiptFirst => {
                let (creation, committed) = committed_proxy(created);
                let shutdown = supervisor
                    .on(committed)
                    .unwrap_or_else(|_| panic!("late proxy commit begins exact shutdown"))
                    .sends
                    .proxy_operations
                    .into_items()
                    .pop()
                    .expect("late proxy receives one shutdown");
                let (shutdown_creation, _request, operation) = shutdown.into_parts();
                assert_eq!(shutdown_creation, creation);
                let settled = supervisor
                    .on(accepted_proxy_input(shutdown_creation, operation))
                    .unwrap_or_else(|_| panic!("shutdown receipt waits for exact proxy exit"));
                assert!(settled.sends.lifecycle.is_empty());
                supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("exact proxy exit closes retirement"))
            }
            CreationResult::ProxyExitFirst => {
                let (creation, committed) = committed_proxy(created);
                let shutdown = supervisor
                    .on(committed)
                    .unwrap_or_else(|_| panic!("late proxy commit begins exact shutdown"))
                    .sends
                    .proxy_operations
                    .into_items()
                    .pop()
                    .expect("late proxy receives one shutdown");
                let exited = supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("proxy exit may precede shutdown receipt"));
                assert!(exited.sends.lifecycle.is_empty());
                let (shutdown_creation, _request, operation) = shutdown.into_parts();
                assert_eq!(shutdown_creation, creation);
                supervisor
                    .on(accepted_proxy_input(shutdown_creation, operation))
                    .unwrap_or_else(|_| panic!("shutdown receipt closes reverse-order retirement"))
            }
        };
        assert!(matches!(terminal.become_, Step::Continue));
        assert_eq!(terminal.sends.lifecycle.len(), 2);
        assert!(matches!(
            &terminal.sends.lifecycle[0].message,
            DynamicLifecycle::OperationCancelled {
                change: WorkerChange::Start,
                outcome: CancellationOutcome::WorkerReturned,
                ..
            }
        ));
        let generation = match &terminal.sends.lifecycle[1].message {
            DynamicLifecycle::EntryRetired {
                generation,
                cause: EntryRetirement::Cancellation,
                ..
            } => *generation,
            _ => panic!("cancelled entry retires once with its exact cause"),
        };
        if let Some(previous) = last_generation.replace(generation) {
            assert_ne!(generation, previous);
        }

        let queried = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Query {
                    key: ServiceKey::Primary,
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("query remains total after retirement"));
        assert!(matches!(
            &queried.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, behavior::atomic::QueryReply::Unknown { .. })
        ));

        let stale = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("retired cancellation is a total stale reply"));
        assert!(stale.sends.lifecycle.is_empty());
        old_authority = match stale
            .sends
            .cancel_replies
            .into_deliveries()
            .pop()
            .expect("retired cancellation returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                CancellationReceipt::Stale { authority } => Some(authority),
                _ => panic!("retirement makes cancellation authority stale"),
            },
            ReplyDelivery::Established(_) => panic!("logical cancellation route remains logical"),
        };
    }
});
