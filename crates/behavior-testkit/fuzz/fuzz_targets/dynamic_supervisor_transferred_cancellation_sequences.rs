#![no_main]
//! Post-transfer cancellation through every worker/shutdown/proxy arrival order.

use std::time::Instant;

use behavior::atomic::{
    ActivationPolicy, ActorDrainPolicy, CancellationOutcome, CancellationReceipt,
    DiagnosticDisposition, DynamicCommand, DynamicDiagnostic, DynamicLifecycle, EntryCapacity,
    EntryRetirement, ImmediateActivation, InitialWorkerOutcome, ProxyOperation, ProxyOutcome,
    ProxyPhase, UnexpectedExit, WorkerChange, WorkerSubmission, dynamic,
};
use behavior::{
    Activate as _, ChildInputReason, ChildReport, ChildStopped, Exit, ItemSettlement,
    MessageProtocol, Recipient, ReplyDelivery, ReplyRoute, SettledItem,
};
use libfuzzer_sys::fuzz_target;

mod dynamic_supervisor;

use dynamic_supervisor::{RuntimeAddress, Worker, accepted_proxy_input, committed_proxy};

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
enum ServiceKey {
    Primary,
    Waiting,
}

#[derive(Clone, Copy)]
enum WorkerDisposition {
    InputRejected,
    ProxyReported,
}

impl WorkerDisposition {
    const fn from_byte(byte: u8) -> Self {
        match byte % 2 {
            0 => Self::InputRejected,
            _ => Self::ProxyReported,
        }
    }
}

enum WorkerArrival {
    InputRejected(ProxyOperation<behavior::Here, Worker, ImmediateActivation>),
    ProxyReported(ChildReport<ProxyOutcome<Worker, ImmediateActivation>>),
}

#[derive(Clone, Copy)]
enum Arrival {
    Worker,
    Shutdown,
    ProxyExit,
}

const ARRIVAL_ORDERS: [[Arrival; 3]; 6] = [
    [Arrival::Worker, Arrival::Shutdown, Arrival::ProxyExit],
    [Arrival::Worker, Arrival::ProxyExit, Arrival::Shutdown],
    [Arrival::Shutdown, Arrival::Worker, Arrival::ProxyExit],
    [Arrival::Shutdown, Arrival::ProxyExit, Arrival::Worker],
    [Arrival::ProxyExit, Arrival::Worker, Arrival::Shutdown],
    [Arrival::ProxyExit, Arrival::Shutdown, Arrival::Worker],
];

fuzz_target!(|input: &[u8]| {
    for byte in input.iter().copied().take(64) {
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
            EntryCapacity::new(2).expect("two entries are valid"),
            ActivationPolicy::new(1).expect("one activation is valid"),
            UnexpectedExit::KeepEmpty,
            ActorDrainPolicy::WaitForActorGraph,
            lifecycle,
            DiagnosticDisposition::deliver_to(diagnostics),
        )
        .initialize()
        .unwrap_or_else(|_| panic!("dynamic supervisor initialization is pure"));
        let mut supervisor = initialized.behavior;

        let primary = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Start {
                    key: ServiceKey::Primary,
                    submission: WorkerSubmission::immediate(Worker),
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("the primary service is admitted"));
        let authority = match primary
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
        let primary_proxy = primary
            .creates
            .into_iter()
            .next()
            .expect("the primary service creates one proxy");
        let (primary_creation, primary_committed) = committed_proxy(primary_proxy);
        let primary_input = supervisor
            .on(primary_committed)
            .unwrap_or_else(|_| panic!("the primary proxy receives one worker input"))
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .expect("the primary worker input is emitted");

        let disposition = WorkerDisposition::from_byte(byte);
        let worker = match disposition {
            WorkerDisposition::InputRejected => WorkerArrival::InputRejected(primary_input),
            WorkerDisposition::ProxyReported => {
                let (creation, _input, operation) = primary_input.into_parts();
                assert_eq!(creation, primary_creation);
                let accepted = supervisor
                    .on(accepted_proxy_input(creation, operation))
                    .unwrap_or_else(|_| panic!("the primary input is accepted"));
                assert!(accepted.sends.proxy_operations.is_empty());
                WorkerArrival::ProxyReported(ChildReport::new(
                    primary_creation,
                    ProxyOutcome::Initial {
                        outcome: InitialWorkerOutcome::Overlap {
                            worker: Worker,
                            activation: ImmediateActivation,
                            phase: ProxyPhase::Ready,
                        },
                    },
                ))
            }
        };

        let waiting = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Start {
                    key: ServiceKey::Waiting,
                    submission: WorkerSubmission::immediate(Worker),
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("the waiting service is admitted"));
        let _waiting_authority = match waiting
            .sends
            .start_replies
            .into_deliveries()
            .pop()
            .expect("the waiting start returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                Ok(receipt) => receipt.cancel,
                Err(_) => panic!("the waiting service is accepted"),
            },
            ReplyDelivery::Established(_) => panic!("logical start route remains logical"),
        };
        let waiting_proxy = waiting
            .creates
            .into_iter()
            .next()
            .expect("the waiting service creates one proxy");
        let (waiting_creation, waiting_committed) = committed_proxy(waiting_proxy);
        let waiting = supervisor
            .on(waiting_committed)
            .unwrap_or_else(|_| panic!("the second proxy waits for activation capacity"));
        assert!(waiting.sends.proxy_operations.is_empty());

        let cancelling = supervisor
            .receive(
                RuntimeAddress,
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(RuntimeAddress)),
                },
            )
            .unwrap_or_else(|_| panic!("post-transfer cancellation is admitted"));
        let _cancelled_authority = match cancelling
            .sends
            .cancel_replies
            .into_deliveries()
            .pop()
            .expect("cancellation returns one reply")
        {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                CancellationReceipt::Pending { authority, .. } => authority,
                _ => panic!("a transferred worker is never returned"),
            },
            ReplyDelivery::Established(_) => panic!("logical cancel route remains logical"),
        };
        let shutdown = cancelling
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .expect("cancellation shuts down the exact proxy");
        let (shutdown_creation, _request, shutdown_operation) = shutdown.into_parts();
        assert_eq!(shutdown_creation, primary_creation);

        let mut worker = Some(worker);
        let mut shutdown = Some(accepted_proxy_input(shutdown_creation, shutdown_operation));
        let mut proxy_exit = Some(ChildStopped::new(
            primary_creation,
            Ok(Exit::Normal),
            Instant::now(),
        ));
        let order = ARRIVAL_ORDERS[usize::from(byte / 2) % ARRIVAL_ORDERS.len()];
        let mut terminal_lifecycle = None;

        for arrival in order {
            let acted = match arrival {
                Arrival::Worker => match worker.take().expect("worker result arrives once") {
                    WorkerArrival::InputRejected(input) => supervisor
                        .on(SettledItem::Attempted(ItemSettlement::Rejected {
                            item: input,
                            reason: ChildInputReason::ClosedControlLane,
                        }))
                        .unwrap_or_else(|_| panic!("rejected worker input is retained")),
                    WorkerArrival::ProxyReported(report) => supervisor
                        .on(report)
                        .unwrap_or_else(|_| panic!("late proxy report is retained")),
                },
                Arrival::Shutdown => supervisor
                    .on(shutdown.take().expect("shutdown receipt arrives once"))
                    .unwrap_or_else(|_| panic!("shutdown receipt is retained")),
                Arrival::ProxyExit => supervisor
                    .on(proxy_exit.take().expect("proxy exit arrives once"))
                    .unwrap_or_else(|_| panic!("proxy exit is retained")),
            };
            assert!(acted.creates.is_empty());
            assert!(acted.sends.proxy_observations.is_empty());
            assert!(acted.sends.shutdown_schedules.is_empty());
            assert!(acted.sends.start_replies.as_slice().is_empty());
            assert!(acted.sends.replace_replies.as_slice().is_empty());
            assert!(acted.sends.stop_replies.as_slice().is_empty());
            assert!(acted.sends.query_replies.as_slice().is_empty());
            assert!(acted.sends.cancel_replies.as_slice().is_empty());

            let authorized = acted.sends.proxy_operations.into_items();
            match arrival {
                Arrival::Worker => {
                    assert_eq!(authorized.len(), 1);
                    assert_eq!(authorized[0].creation(), waiting_creation);
                }
                Arrival::Shutdown | Arrival::ProxyExit => assert!(authorized.is_empty()),
            }
            match (arrival, disposition) {
                (Arrival::Worker, WorkerDisposition::InputRejected) => {
                    assert_eq!(acted.sends.diagnostics.len(), 1);
                    assert!(matches!(
                        &acted.sends.diagnostics[0],
                        behavior::atomic::DiagnosticAction::Deliver {
                            diagnostic: DynamicDiagnostic::ProxyInputRejected { .. },
                            ..
                        }
                    ));
                }
                (Arrival::Worker, WorkerDisposition::ProxyReported)
                | (Arrival::Shutdown, _)
                | (Arrival::ProxyExit, _) => assert!(acted.sends.diagnostics.is_empty()),
            }
            if worker.is_none() && shutdown.is_none() && proxy_exit.is_none() {
                assert_eq!(acted.sends.lifecycle.len(), 2);
                terminal_lifecycle = Some(acted.sends.lifecycle);
            } else {
                assert!(acted.sends.lifecycle.is_empty());
            }
        }

        let lifecycle = terminal_lifecycle.expect("the third arrival closes cancellation");
        assert_eq!(lifecycle.len(), 2);
        match (&lifecycle[0].message, disposition) {
            (
                DynamicLifecycle::OperationCancelled {
                    change: WorkerChange::Start,
                    outcome: CancellationOutcome::ProxyInputRejected,
                    ..
                },
                WorkerDisposition::InputRejected,
            ) => {}
            (
                DynamicLifecycle::OperationCancelled {
                    change: WorkerChange::Start,
                    outcome:
                        CancellationOutcome::ProxyReported {
                            outcome: ProxyOutcome::Initial { .. },
                        },
                    ..
                },
                WorkerDisposition::ProxyReported,
            ) => {}
            _ => panic!("cancellation publishes the exact worker disposition"),
        }
        assert!(matches!(
            &lifecycle[1].message,
            DynamicLifecycle::EntryRetired {
                cause: EntryRetirement::Cancellation,
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
            .unwrap_or_else(|_| panic!("retired service query is total"));
        assert!(matches!(
            &queried.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, behavior::atomic::QueryReply::Unknown { .. })
        ));
    }
});
