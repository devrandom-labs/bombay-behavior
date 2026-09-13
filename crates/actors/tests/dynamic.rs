use std::collections::VecDeque;
use std::time::{Duration, Instant};

use proptest::collection::vec;
use proptest::proptest;

use behavior_actors::atomic::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, CancellationOutcome, CancellationReceipt,
    DiagnosticAction, DiagnosticDisposition, DynamicCommand, DynamicDiagnostic, DynamicLifecycle,
    DynamicStatus, DynamicSupervisor, DynamicSupervisorEvent, EntryCapacity, EntryRetirement,
    EntryStopFailureReason, InitialWorkerOutcome, InterruptedWorker, ProxyControl, ProxyDrain,
    ProxyInputReceipt, ProxyInputResult, ProxyOperationId, ProxyOutcome, ProxyPhase, QueryReply,
    ReplacementFailure, ReplacementOutcome, StableProxy, StartRejection, UnexpectedExit,
    WorkerChange, WorkerChangeInterruption, WorkerChangeReceipt, WorkerChangeRejection,
    WorkerCreationRejection, WorkerInitializationOutcome, WorkerStartResult, WorkerSubmission,
    ZeroCapacity, dynamic,
};
use behavior_actors::{
    Activate, Active, ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome,
    ChildInputReason, ChildReport, ChildStopped, CreateChild, CreationId, CreationKind,
    CreationRejection, CreationSequence, CreationSettlement, CreationsSettled, EndpointAddress,
    EstablishedActor, EstablishedCreation, EstablishedRecipient, Exit, Here, InterpreterFault,
    ItemSettlement, MessageProtocol, Never, NoBirths, NoSends, Protocol, Recipient, ReplyDelivery,
    ReplyRoute, RoutedCreation, ScheduleAfterRejection, SettledItem, ShutdownRequested,
    TimerElapsed, TimerGeneration, TimerId, TimerScheduled, User,
};

#[expect(
    dead_code,
    reason = "compile-only proof of the complete explicit-stop rejection sum"
)]
fn explicit_stop_rejection_is_an_interpreter_result(reason: EntryStopFailureReason) {
    match reason {
        EntryStopFailureReason::ControlRejected(_)
        | EntryStopFailureReason::InterpreterCorrupt(_)
        | EntryStopFailureReason::InterpretationSkipped => {}
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SearchAddress(u64);

impl Address for SearchAddress {
    type Nonce = u64;
}

#[derive(Clone, Eq, PartialEq)]
struct SearchEndpoint;

impl EndpointAddress for SearchAddress {
    type Established<P>
        = SearchEndpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
struct SearchKey(&'static str);

#[derive(Debug, Eq, PartialEq)]
struct SearchWorker;

impl Behavior for SearchWorker {
    type Protocol = MessageProtocol<SearchAddress, Never>;
    type Event = User<SearchAddress, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SearchActivation;

#[derive(Clone, Copy)]
enum CancellationOrder {
    ReportFirst,
    ReportLast,
}

#[derive(Clone, Copy)]
enum StopOrder {
    ReceiptFirst,
    ProxyFirst,
}

#[derive(Clone, Copy)]
enum PendingProxyCreation {
    Committed,
    Rejected,
}

#[derive(Clone, Copy)]
enum DeadlineResult {
    Rejected,
    Elapsed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ServiceName {
    Search,
    Cache,
}

impl ServiceName {
    const fn key(self) -> SearchKey {
        match self {
            Self::Search => SearchKey("search"),
            Self::Cache => SearchKey("cache"),
        }
    }
}

enum CustomerCatalogue {
    Empty,
    Pending {
        service: ServiceName,
        creation: CreateChild<SearchAddress, StableProxy<SearchWorker, SearchActivation>>,
        authority: behavior_actors::atomic::CancelAuthority<SearchKey>,
    },
}

fn overlapping_initial_report(
    creation: behavior_actors::CreationId,
) -> ChildReport<ProxyOutcome<SearchWorker, SearchActivation>> {
    ChildReport::new(
        creation,
        ProxyOutcome::Initial {
            outcome: InitialWorkerOutcome::Overlap {
                worker: SearchWorker,
                activation: SearchActivation,
                phase: ProxyPhase::Ready,
            },
        },
    )
}

fn committed_proxy(
    created: CreateChild<SearchAddress, StableProxy<SearchWorker, SearchActivation>>,
) -> (
    CreationId,
    CreationsSettled<SearchAddress, StableProxy<SearchWorker, SearchActivation>>,
) {
    let (creation, _proxy, kind) = created.into_parts();
    let settled = SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::Established {
            established: EstablishedCreation::installed(
                creation,
                kind,
                EstablishedRecipient::issued(SearchEndpoint),
            ),
        },
    ));
    (
        creation,
        CreationsSettled::new(CreationSettlement::Settled([settled].into_iter().collect())),
    )
}

fn accepted_search_input(
    creation: CreationId,
    operation: ProxyOperationId,
) -> ProxyInputResult<Here, SearchWorker, SearchActivation> {
    SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
        creation,
        EstablishedActor::issued(SearchEndpoint),
        operation,
    )))
}

async fn ready_search_proxy(
    control: ProxyControl<SearchWorker, SearchActivation>,
) -> (
    Active<StableProxy<SearchWorker, SearchActivation>>,
    ProxyOutcome<SearchWorker, SearchActivation>,
) {
    let initialized = StableProxy::<SearchWorker, SearchActivation>::activated()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized.behavior;
    let worker = proxy
        .on(control)
        .unwrap_or_else(|_| panic!("the proxy accepts its owner input"))
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the proxy creates one worker"));
    let (creation, _worker, kind) = worker.into_parts();
    let initializing = proxy
        .on(CreationsSettled::new(CreationSettlement::Settled(
            [SettledItem::Attempted(ItemSettlement::Accepted(
                ChildCreationOutcome::Established {
                    established: EstablishedCreation::installed(
                        creation,
                        kind,
                        EstablishedRecipient::issued(SearchEndpoint),
                    ),
                },
            ))]
            .into_iter()
            .collect(),
        )))
        .unwrap_or_else(|_| panic!("the worker creation commits"));
    let initialization = initializing
        .sends
        .worker_initializations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the worker initialization is requested"));
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("worker initialization accepts activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one activation is requested"));
    let started = proxy
        .on(activation.started())
        .unwrap_or_else(|_| panic!("activation starts"));
    assert!(started.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .unwrap_or_else(|_| panic!("activation completes"));
    let outcome = ready
        .sends
        .owner_outcomes
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the proxy reports readiness"))
        .into_inner();
    (proxy, outcome)
}

impl ActivationPlan for SearchActivation {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

struct RootPeers {
    lifecycle: Recipient<
        MessageProtocol<SearchAddress, DynamicLifecycle<SearchKey, SearchWorker, SearchActivation>>,
    >,
    diagnostics: Recipient<
        MessageProtocol<
            SearchAddress,
            DynamicDiagnostic<SearchKey, SearchWorker, SearchActivation>,
        >,
    >,
}

fn search_supervisor(
    entries: EntryCapacity,
    activation: ActivationPolicy,
    actor_drain: ActorDrainPolicy,
    peers: RootPeers,
) -> Active<
    DynamicSupervisor<
        SearchKey,
        SearchWorker,
        SearchActivation,
        Recipient<
            MessageProtocol<
                SearchAddress,
                DynamicLifecycle<SearchKey, SearchWorker, SearchActivation>,
            >,
        >,
        Recipient<
            MessageProtocol<
                SearchAddress,
                DynamicDiagnostic<SearchKey, SearchWorker, SearchActivation>,
            >,
        >,
    >,
> {
    dynamic(
        entries,
        activation,
        UnexpectedExit::KeepEmpty,
        actor_drain,
        peers.lifecycle,
        DiagnosticDisposition::deliver_to(peers.diagnostics),
    )
    .initialize()
    .unwrap_or_else(|_| panic!("dynamic supervisor initialization is pure"))
    .behavior
}

fn admit_search_replacement(
    supervisor: &mut Active<
        DynamicSupervisor<
            SearchKey,
            SearchWorker,
            SearchActivation,
            Recipient<
                MessageProtocol<
                    SearchAddress,
                    DynamicLifecycle<SearchKey, SearchWorker, SearchActivation>,
                >,
            >,
            Recipient<
                MessageProtocol<
                    SearchAddress,
                    DynamicDiagnostic<SearchKey, SearchWorker, SearchActivation>,
                >,
            >,
        >,
    >,
) {
    let replacing = supervisor
        .receive(
            SearchAddress(90),
            DynamicCommand::Replace {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(91))),
            },
        )
        .unwrap_or_else(|_| panic!("the current service accepts replacement"));
    let replacement = replacing
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one replacement input is emitted"));
    let (creation, _control, operation) = replacement.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(creation, operation))
        .unwrap_or_else(|_| panic!("the replacement input is accepted"));
    assert!(accepted.creates.is_empty());
    assert!(accepted.sends.lifecycle.is_empty());
}

#[test]
fn capacities_reject_zero_before_supervisor_construction() {
    assert_eq!(EntryCapacity::new(0), Err(ZeroCapacity));
    assert_eq!(ActivationPolicy::new(0), Err(ZeroCapacity));
}

#[test]
fn role_first_construction_returns_the_complete_behavior() {
    let peers = RootPeers {
        lifecycle: Recipient::global(SearchAddress(1)),
        diagnostics: Recipient::global(SearchAddress(2)),
    };
    let supervisor = dynamic(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        UnexpectedExit::KeepEmpty,
        ActorDrainPolicy::WaitForActorGraph,
        peers.lifecycle,
        DiagnosticDisposition::deliver_to(peers.diagnostics),
    )
    .initialize()
    .unwrap_or_else(|_| panic!("dynamic supervisor initialization is pure"))
    .behavior;
    drop(supervisor);
}

#[test]
fn empty_global_shutdown_stops_without_runtime_work() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(1)),
            diagnostics: Recipient::global(SearchAddress(2)),
        },
    );

    let shutdown = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("global shutdown is a total control transition"));
    assert!(shutdown.sends.proxy_observations.is_empty());
    assert!(shutdown.sends.proxy_operations.is_empty());
    assert!(shutdown.sends.shutdown_schedules.is_empty());
    assert!(shutdown.sends.start_replies.as_slice().is_empty());
    assert!(shutdown.sends.replace_replies.as_slice().is_empty());
    assert!(shutdown.sends.stop_replies.as_slice().is_empty());
    assert!(shutdown.sends.query_replies.as_slice().is_empty());
    assert!(shutdown.sends.cancel_replies.as_slice().is_empty());
    assert!(shutdown.sends.lifecycle.is_empty());
    assert!(shutdown.sends.diagnostics.is_empty());
    assert!(shutdown.creates.is_empty());
    assert!(matches!(shutdown.become_, behavior_actors::Step::Stop(_)));
}

#[test]
fn global_shutdown_drains_a_pending_proxy_creation_and_closes_management() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(2).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(1)),
            diagnostics: Recipient::global(SearchAddress(2)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(3),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(4))),
            },
        )
        .unwrap_or_else(|_| panic!("the vacant supervisor accepts its first service"));
    let authority = match starting.sends.start_replies.into_deliveries().pop() {
        Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
            Ok(receipt) => receipt.cancel,
            Err(_) => panic!("start returns its cancellation authority"),
        },
        _ => panic!("start retains the logical reply route"),
    };
    let created = starting
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("start emits one proxy creation"));

    let shutdown = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown accepts the pending proxy creation"));
    assert!(matches!(shutdown.become_, behavior_actors::Step::Continue));
    assert!(shutdown.sends.proxy_operations.is_empty());
    assert_eq!(shutdown.sends.lifecycle.len(), 1);
    assert!(matches!(
        &shutdown.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Start
            },
            worker: InterruptedWorker::Submission(_),
            ..
        }
    ));

    let repeated = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("repeated shutdown is idempotent"));
    assert!(repeated.sends.proxy_operations.is_empty());
    assert!(repeated.sends.shutdown_schedules.is_empty());
    assert!(repeated.sends.lifecycle.is_empty());

    let queried = supervisor
        .receive(
            SearchAddress(5),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(6))),
            },
        )
        .unwrap_or_else(|_| panic!("query remains available during shutdown"));
    assert!(matches!(
        &queried.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, QueryReply::Known { status: DynamicStatus::Draining, .. })
    ));
    let cancelled = supervisor
        .receive(
            SearchAddress(7),
            DynamicCommand::Cancel {
                authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(8))),
            },
        )
        .unwrap_or_else(|_| panic!("cancellation reports shutdown ownership"));
    assert!(matches!(
        &cancelled.sends.cancel_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, CancellationReceipt::Draining { .. })
    ));
    let rejected = supervisor
        .receive(
            SearchAddress(9),
            DynamicCommand::Start {
                key: SearchKey("cache"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(10))),
            },
        )
        .unwrap_or_else(|_| panic!("closed start returns its complete submission"));
    assert!(matches!(
        &rejected.sends.start_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(
                delivery.message,
                Err(WorkerChangeRejection {
                    reason: StartRejection::ShuttingDown,
                    ..
                })
            )
    ));

    let (creation, committed) = committed_proxy(created);
    let late_birth = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the exact late proxy birth is retained"));
    assert_eq!(late_birth.sends.proxy_operations.len(), 1);
    let shutdown = late_birth
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the committed late proxy is shut down once"));
    let (shutdown_creation, _control, shutdown_operation) = shutdown.into_parts();
    let settled = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the shutdown receipt waits for exact proxy exit"));
    assert!(matches!(settled.become_, behavior_actors::Step::Continue));
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit completes aggregate shutdown"));
    assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Shutdown,
            ..
        }
    ));
}

#[test]
fn global_shutdown_preserves_a_locally_cancelled_start() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(103)),
            diagnostics: Recipient::global(SearchAddress(104)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(105),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(106))),
            },
        )
        .unwrap_or_else(|_| panic!("start is admitted before shutdown"));
    let creation = starting
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("start retains one pending proxy creation"));
    let authority = match starting.sends.start_replies.into_deliveries().pop() {
        Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
            Ok(receipt) => receipt.cancel,
            Err(_) => panic!("accepted start returns cancellation authority"),
        },
        _ => panic!("start retains its logical reply route"),
    };
    let cancelled = supervisor
        .receive(
            SearchAddress(107),
            DynamicCommand::Cancel {
                authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(108))),
            },
        )
        .unwrap_or_else(|_| panic!("pending creation permits local cancellation"));
    let (authority, submission) = match cancelled.sends.cancel_replies.into_deliveries().pop() {
        Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
            CancellationReceipt::Returned {
                authority,
                submission,
            } => (authority, submission),
            _ => panic!("local cancellation returns worker custody"),
        },
        _ => panic!("cancellation retains its logical reply route"),
    };

    let shutdown = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown retains the unresolved proxy creation"));
    assert!(matches!(shutdown.become_, behavior_actors::Step::Continue));
    assert!(shutdown.sends.proxy_operations.is_empty());
    assert!(shutdown.sends.lifecycle.is_empty());
    let draining = supervisor
        .receive(
            SearchAddress(109),
            DynamicCommand::Cancel {
                authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(110))),
            },
        )
        .unwrap_or_else(|_| panic!("shutdown owns cancellation admission"));
    let draining_authority = match draining.sends.cancel_replies.into_deliveries().pop() {
        Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
            CancellationReceipt::Draining { authority } => authority,
            _ => panic!("cancellation remains closed during shutdown"),
        },
        _ => panic!("shutdown retains the cancellation reply route"),
    };

    let (creation, committed) = committed_proxy(creation);
    let late_birth = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("late proxy creation remains owned"));
    assert_eq!(late_birth.sends.proxy_operations.len(), 1);
    assert!(late_birth.sends.lifecycle.is_empty());
    let shutdown = late_birth
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("late proxy is shut down once"));
    let (shutdown_creation, _control, operation) = shutdown.into_parts();
    let awaiting_exit = supervisor
        .on(accepted_search_input(shutdown_creation, operation))
        .unwrap_or_else(|_| panic!("shutdown settlement waits for exact exit"));
    assert!(matches!(
        awaiting_exit.become_,
        behavior_actors::Step::Continue
    ));
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("exact proxy exit closes global shutdown"));
    assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::OperationCancelled {
            outcome: CancellationOutcome::WorkerReturned,
            ..
        }
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Cancellation,
            ..
        }
    ));
    assert_eq!(draining_authority.key().0, "search");
    assert_eq!(
        submission,
        WorkerSubmission::activated(SearchWorker, SearchActivation)
    );
}

#[test]
fn shutdown_deadline_rejection_and_elapsed_force_retirement() {
    for result in [DeadlineResult::Rejected, DeadlineResult::Elapsed] {
        let mut supervisor = search_supervisor(
            EntryCapacity::new(1).expect("entry capacity is positive"),
            ActivationPolicy::new(1).expect("activation capacity is positive"),
            ActorDrainPolicy::RetireActorGraphAfter {
                deadline: Duration::from_secs(5),
            },
            RootPeers {
                lifecycle: Recipient::global(SearchAddress(11)),
                diagnostics: Recipient::global(SearchAddress(12)),
            },
        );
        let starting = supervisor
            .receive(
                SearchAddress(13),
                DynamicCommand::Start {
                    key: SearchKey("search"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(14))),
                },
            )
            .unwrap_or_else(|_| panic!("one unresolved proxy creation keeps shutdown live"));
        assert_eq!(starting.creates.len(), 1);
        let shutdown = supervisor
            .on(ShutdownRequested)
            .unwrap_or_else(|_| panic!("shutdown reserves one exact deadline"));
        let schedule = shutdown
            .sends
            .shutdown_schedules
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("timed drain emits one schedule"));
        assert_eq!(schedule.after, Duration::from_secs(5));
        assert!(matches!(shutdown.become_, behavior_actors::Step::Continue));

        let forced = match result {
            DeadlineResult::Rejected => supervisor
                .transition(DynamicSupervisorEvent::ShutdownScheduleSettled(
                    SettledItem::Attempted(ItemSettlement::Rejected {
                        item: schedule,
                        reason: ScheduleAfterRejection::DeadlineOverflow,
                    }),
                ))
                .unwrap_or_else(|_| panic!("exact schedule rejection forces retirement")),
            DeadlineResult::Elapsed => {
                let scheduled = supervisor
                    .transition(DynamicSupervisorEvent::ShutdownScheduleSettled(
                        SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                            id: schedule.id,
                            generation: schedule.generation,
                        })),
                    ))
                    .unwrap_or_else(|_| panic!("exact schedule receipt starts the deadline"));
                assert!(matches!(scheduled.become_, behavior_actors::Step::Continue));
                let foreign = TimerElapsed::new(TimerId(9), TimerGeneration(9));
                assert!(matches!(
                    supervisor.on(foreign),
                    Err(DynamicSupervisorEvent::ShutdownElapsed(elapsed)) if elapsed == foreign
                ));
                supervisor
                    .on(TimerElapsed::new(schedule.id, schedule.generation))
                    .unwrap_or_else(|_| panic!("exact elapsed deadline forces retirement"))
            }
        };
        assert!(matches!(forced.become_, behavior_actors::Step::Stop(_)));
        assert!(forced.sends.lifecycle.is_empty());
        assert!(forced.creates.is_empty());
    }
}

#[test]
fn stop_waits_for_pending_proxy_creation() {
    for creation_result in [
        PendingProxyCreation::Committed,
        PendingProxyCreation::Rejected,
    ] {
        let mut supervisor = search_supervisor(
            EntryCapacity::new(1).expect("entry capacity is positive"),
            ActivationPolicy::new(1).expect("activation capacity is positive"),
            ActorDrainPolicy::WaitForActorGraph,
            RootPeers {
                lifecycle: Recipient::global(SearchAddress(55)),
                diagnostics: Recipient::global(SearchAddress(56)),
            },
        );
        let starting = supervisor
            .receive(
                SearchAddress(57),
                DynamicCommand::Start {
                    key: SearchKey("search"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(58))),
                },
            )
            .unwrap_or_else(|_| panic!("the keyed start is accepted"));
        assert_eq!(starting.creates.len(), 1);

        let stopping = supervisor
            .receive(
                SearchAddress(59),
                DynamicCommand::Stop {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(60))),
                },
            )
            .unwrap_or_else(|_| panic!("stop is total while proxy creation is pending"));
        assert!(stopping.sends.proxy_operations.is_empty());
        assert!(matches!(
            &stopping.sends.stop_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery) if matches!(delivery.message, Ok(SearchKey("search")))
        ));

        let draining = supervisor
            .on(ShutdownRequested)
            .unwrap_or_else(|_| panic!("global shutdown preserves the admitted stop"));
        assert!(matches!(
            &draining.sends.lifecycle[0].message,
            DynamicLifecycle::WorkerChangeInterrupted {
                operation: 1,
                interruption: WorkerChangeInterruption::ExplicitStop { operation: 2 },
                ..
            }
        ));

        let queried = supervisor
            .receive(
                SearchAddress(61),
                DynamicCommand::Query {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(62))),
                },
            )
            .unwrap_or_else(|_| panic!("the pending stop remains queryable"));
        assert!(matches!(
            &queried.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(
                    delivery.message,
                    QueryReply::Known {
                        status: DynamicStatus::Draining,
                        ..
                    }
                )
        ));

        let created = starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the pending proxy creation remains owned"));
        match creation_result {
            PendingProxyCreation::Committed => {
                let (creation, committed) = committed_proxy(created);
                let committed = supervisor
                    .on(committed)
                    .unwrap_or_else(|_| panic!("the committed proxy is drained"));
                assert!(committed.sends.lifecycle.is_empty());
                let shutdown = committed
                    .sends
                    .proxy_operations
                    .into_items()
                    .pop()
                    .unwrap_or_else(|| panic!("the committed proxy receives one shutdown"));
                let stopped_before_settlement = supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("the exact proxy may stop before rejection"));
                assert!(stopped_before_settlement.sends.lifecycle.is_empty());
                let failed = supervisor
                    .on(SettledItem::Attempted(ItemSettlement::Rejected {
                        item: shutdown,
                        reason: ChildInputReason::ClosedControlLane,
                    }))
                    .unwrap_or_else(|_| panic!("shutdown rejection reunites with the proxy stop"));
                assert_eq!(failed.sends.lifecycle.len(), 2);
                let mut lifecycle = failed.sends.lifecycle;
                let DynamicLifecycle::StopFinished {
                    result: Err(failure),
                    ..
                } = lifecycle.remove(0).message
                else {
                    panic!("the lifecycle value reports the failed stop")
                };
                assert_eq!(
                    failure.stopped().map(|stopped| stopped.child),
                    Some(creation)
                );
                let (reason, stopped) = failure.into_parts();
                assert!(matches!(
                    reason,
                    EntryStopFailureReason::ControlRejected(ChildInputReason::ClosedControlLane)
                ));
                assert!(matches!(stopped, Some(ChildStopped { child, .. }) if child == creation));
                assert!(matches!(
                    &lifecycle[0].message,
                    DynamicLifecycle::EntryRetired {
                        cause: EntryRetirement::Stop,
                        ..
                    }
                ));
            }
            PendingProxyCreation::Rejected => {
                let (creation, proxy, kind) = created.into_parts();
                let request = match kind {
                    CreationKind::Birth => CreateChild::birth(creation, proxy),
                    CreationKind::Replacement { previous } => {
                        CreateChild::replacement(creation, previous, proxy)
                    }
                };
                let routed = RoutedCreation::new(request, creation.get());
                let rejected = supervisor
                    .on(CreationsSettled::new(CreationSettlement::Settled(
                        [SettledItem::Attempted(ItemSettlement::Rejected {
                            item: routed,
                            reason: CreationRejection::EnvironmentFailed,
                        })]
                        .into_iter()
                        .collect(),
                    )))
                    .unwrap_or_else(|_| panic!("creation rejection retires without a proxy"));
                assert_eq!(rejected.sends.lifecycle.len(), 1);
                assert!(matches!(
                    &rejected.sends.lifecycle[0].message,
                    DynamicLifecycle::EntryRetired {
                        cause: EntryRetirement::Stop,
                        ..
                    }
                ));
                assert_eq!(rejected.sends.diagnostics.len(), 1);
            }
        }

        let retired = supervisor
            .receive(
                SearchAddress(63),
                DynamicCommand::Query {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(64))),
                },
            )
            .unwrap_or_else(|_| panic!("the retired key is queryable"));
        assert!(matches!(
            &retired.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, QueryReply::Unknown { .. })
        ));
    }
}

#[test]
fn stop_interrupts_a_worker_waiting_for_activation() {
    for order in [StopOrder::ReceiptFirst, StopOrder::ProxyFirst] {
        let mut supervisor = search_supervisor(
            EntryCapacity::new(2).expect("entry capacity is positive"),
            ActivationPolicy::new(1).expect("activation capacity is positive"),
            ActorDrainPolicy::WaitForActorGraph,
            RootPeers {
                lifecycle: Recipient::global(SearchAddress(65)),
                diagnostics: Recipient::global(SearchAddress(66)),
            },
        );
        let occupied = supervisor
            .receive(
                SearchAddress(67),
                DynamicCommand::Start {
                    key: SearchKey("occupied"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(68))),
                },
            )
            .unwrap_or_else(|_| panic!("the first start occupies activation capacity"));
        let (_, occupied_created) = committed_proxy(
            occupied
                .creates
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("the first start creates its proxy")),
        );
        let occupied_input = supervisor
            .on(occupied_created)
            .unwrap_or_else(|_| panic!("the first proxy receives its worker"))
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("the first worker input is emitted"));
        let (occupied_creation, _, occupied_operation) = occupied_input.into_parts();
        let occupied = supervisor
            .on(accepted_search_input(occupied_creation, occupied_operation))
            .unwrap_or_else(|_| panic!("the first worker input occupies capacity"));
        assert!(occupied.sends.proxy_operations.is_empty());
        let waiting = supervisor
            .receive(
                SearchAddress(69),
                DynamicCommand::Start {
                    key: SearchKey("waiting"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(70))),
                },
            )
            .unwrap_or_else(|_| panic!("the second start is admitted"));
        let (waiting_creation, waiting_created) = committed_proxy(
            waiting
                .creates
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("the second start creates its proxy")),
        );
        let waiting = supervisor
            .on(waiting_created)
            .unwrap_or_else(|_| panic!("the second worker waits for activation capacity"));
        assert!(waiting.sends.proxy_operations.is_empty());
        let stopping = supervisor
            .receive(
                SearchAddress(71),
                DynamicCommand::Stop {
                    key: SearchKey("waiting"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(72))),
                },
            )
            .unwrap_or_else(|_| panic!("the waiting start accepts stop"));
        assert!(matches!(
            &stopping.sends.stop_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery) if matches!(delivery.message, Ok(SearchKey("waiting")))
        ));
        assert!(matches!(
            &stopping.sends.lifecycle[0].message,
            DynamicLifecycle::WorkerChangeInterrupted {
                operation: 2,
                interruption: WorkerChangeInterruption::ExplicitStop { operation: 3 },
                ..
            }
        ));
        let shutdown = stopping
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("the waiting proxy receives one shutdown"));
        let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
        let lifecycle = match order {
            StopOrder::ReceiptFirst => {
                let settled = supervisor
                    .on(accepted_search_input(shutdown_creation, shutdown_operation))
                    .unwrap_or_else(|_| panic!("the shutdown input is accepted"));
                assert!(settled.sends.lifecycle.is_empty());
                supervisor
                    .on(ChildStopped::new(
                        waiting_creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("the exact proxy exit closes stop"))
                    .sends
                    .lifecycle
            }
            StopOrder::ProxyFirst => {
                let stopped = supervisor
                    .on(ChildStopped::new(
                        waiting_creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("the exact proxy may stop first"));
                assert!(stopped.sends.lifecycle.is_empty());
                supervisor
                    .on(accepted_search_input(shutdown_creation, shutdown_operation))
                    .unwrap_or_else(|_| panic!("the shutdown receipt closes stop"))
                    .sends
                    .lifecycle
            }
        };
        assert_eq!(lifecycle.len(), 2);
        assert!(matches!(
            &lifecycle[0].message,
            DynamicLifecycle::StopFinished { result: Ok(_), .. }
        ));
        assert!(matches!(
            &lifecycle[1].message,
            DynamicLifecycle::EntryRetired {
                cause: EntryRetirement::Stop,
                ..
            }
        ));
    }
}

#[test]
fn rejected_proxy_shutdown_still_retires_an_interrupted_start_as_stop() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(2).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(107)),
            diagnostics: Recipient::global(SearchAddress(108)),
        },
    );
    let occupying_start = supervisor
        .receive(
            SearchAddress(109),
            DynamicCommand::Start {
                key: SearchKey("occupied"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(110))),
            },
        )
        .unwrap_or_else(|_| panic!("the first start occupies activation capacity"));
    let (_, occupying_proxy) = committed_proxy(
        occupying_start
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the first start creates its proxy")),
    );
    let occupying_input = supervisor
        .on(occupying_proxy)
        .unwrap_or_else(|_| panic!("the first proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the first worker input is emitted"));
    let (occupying_creation, _occupying_control, occupying_operation) =
        occupying_input.into_parts();
    let occupied = supervisor
        .on(accepted_search_input(
            occupying_creation,
            occupying_operation,
        ))
        .unwrap_or_else(|_| panic!("the first worker input occupies activation capacity"));
    assert!(occupied.sends.proxy_operations.is_empty());

    let waiting_start = supervisor
        .receive(
            SearchAddress(111),
            DynamicCommand::Start {
                key: SearchKey("waiting"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(112))),
            },
        )
        .unwrap_or_else(|_| panic!("the second start is admitted"));
    let (waiting_creation, waiting_proxy) = committed_proxy(
        waiting_start
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the second start creates its proxy")),
    );
    let waiting = supervisor
        .on(waiting_proxy)
        .unwrap_or_else(|_| panic!("the second worker waits for activation capacity"));
    assert!(waiting.sends.proxy_operations.is_empty());
    let stopping = supervisor
        .receive(
            SearchAddress(113),
            DynamicCommand::Stop {
                key: SearchKey("waiting"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(114))),
            },
        )
        .unwrap_or_else(|_| panic!("the waiting start accepts Stop"));
    assert!(matches!(
        &stopping.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            interruption: WorkerChangeInterruption::ExplicitStop { .. },
            ..
        }
    ));
    let shutdown = stopping
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the waiting proxy receives one shutdown"));
    let rejected = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: shutdown,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the exact shutdown rejection completes Stop delivery"));
    assert!(matches!(
        &rejected.sends.lifecycle[0].message,
        DynamicLifecycle::StopFinished {
            result: Err(failure),
            ..
        } if matches!(
            failure.reason(),
            EntryStopFailureReason::ControlRejected(ChildInputReason::ClosedControlLane)
        )
    ));
    let retired = supervisor
        .on(ChildStopped::new(
            waiting_creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit closes the accepted Stop"));
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::EntryRetired {
            key: SearchKey("waiting"),
            cause: EntryRetirement::Stop,
            ..
        }
    ));
    let queried = supervisor
        .receive(
            SearchAddress(115),
            DynamicCommand::Query {
                key: SearchKey("waiting"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(116))),
            },
        )
        .unwrap_or_else(|_| panic!("the completed Stop releases its key"));
    assert!(matches!(
        &queried.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, QueryReply::Unknown { .. })
    ));
}

#[test]
fn global_shutdown_retains_a_worker_rejected_after_proxy_input_transfer() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(73)),
            diagnostics: Recipient::global(SearchAddress(74)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(75),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(76))),
            },
        )
        .unwrap_or_else(|_| panic!("the initial service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the start creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown retains the transferred proxy input"));
    assert!(draining.sends.lifecycle.is_empty());
    let shutdown = draining
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("shutdown stops the exact proxy once"));
    let rejected = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: initial,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the rejected worker input completes interrupted custody"));
    assert!(rejected.sends.lifecycle.is_empty());
    assert!(matches!(
        &rejected.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyInputRejected {
                input: SettledItem::Attempted(ItemSettlement::Rejected { item, reason }),
                ..
            },
            ..
        } if item.creation() == creation && *reason == ChildInputReason::ClosedControlLane
    ));
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the shutdown input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit completes shutdown"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            operation: 1,
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Start
            },
            worker: InterruptedWorker::ProxyInputRejected,
            ..
        }
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Shutdown,
            ..
        }
    ));
}

#[test]
fn global_shutdown_retains_the_exact_late_proxy_result() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(79)),
            diagnostics: Recipient::global(SearchAddress(80)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(81),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(82))),
            },
        )
        .unwrap_or_else(|_| panic!("the initial service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the start creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let (input_creation, _, input_operation) = initial.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(input_creation, input_operation))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown retains an accepted proxy input"));
    assert!(draining.sends.lifecycle.is_empty());
    let shutdown = draining
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the proxy receives one shutdown"));
    let reported = supervisor
        .on(overlapping_initial_report(creation))
        .unwrap_or_else(|_| panic!("the exact late proxy result is retained"));
    assert!(reported.sends.lifecycle.is_empty());
    let exited = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy may exit before shutdown settlement"));
    assert!(exited.sends.lifecycle.is_empty());
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let retired = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("shutdown settlement completes the drained proxy"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            operation: 1,
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Start
            },
            worker: InterruptedWorker::ProxyReported(ProxyOutcome::Initial {
                outcome: InitialWorkerOutcome::Overlap {
                    phase: ProxyPhase::Ready,
                    ..
                },
            }),
            ..
        }
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Shutdown,
            ..
        }
    ));
}

#[tokio::test]
async fn global_shutdown_retains_a_replacement_awaiting_input_settlement() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(201)),
            diagnostics: Recipient::global(SearchAddress(202)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(203),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(204))),
            },
        )
        .unwrap_or_else(|_| panic!("the initial service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the initial service creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let (input_creation, control, input_operation) = initial.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(input_creation, input_operation))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let (_proxy, outcome) = ready_search_proxy(control).await;
    let started = supervisor
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("the initial worker makes the service ready"));
    assert!(matches!(
        &started.sends.lifecycle[0].message,
        DynamicLifecycle::Started { .. }
    ));

    let replacing = supervisor
        .receive(
            SearchAddress(205),
            DynamicCommand::Replace {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(206))),
            },
        )
        .unwrap_or_else(|_| panic!("the ready service accepts replacement"));
    let replacement = replacing
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one replacement input is emitted"));
    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown retains the unsettled replacement input"));
    assert!(draining.sends.lifecycle.is_empty());
    let shutdown = draining
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("shutdown stops the exact proxy once"));

    let rejected = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: replacement,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the rejected replacement input remains correlated"));
    assert!(rejected.sends.lifecycle.is_empty());
    assert!(matches!(
        &rejected.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyInputRejected {
                input: SettledItem::Attempted(ItemSettlement::Rejected {
                    item,
                    reason: ChildInputReason::ClosedControlLane,
                }),
                ..
            },
            ..
        } if item.creation() == creation
    ));
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let settled = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the exact shutdown input is accepted"));
    assert!(settled.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit completes shutdown"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            operation: 2,
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Replacement
            },
            worker: InterruptedWorker::ProxyInputRejected,
            ..
        }
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Shutdown,
            ..
        }
    ));
    assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
}

#[tokio::test]
async fn global_shutdown_retains_an_accepted_replacement_report() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(211)),
            diagnostics: Recipient::global(SearchAddress(212)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(213),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(214))),
            },
        )
        .unwrap_or_else(|_| panic!("the initial service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the initial service creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let (input_creation, control, input_operation) = initial.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(input_creation, input_operation))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let (_proxy, outcome) = ready_search_proxy(control).await;
    let previous = match &outcome {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        _ => panic!("the proxy reports its ready worker"),
    };
    let previous_creation = previous.creation();
    let started = supervisor
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("the initial worker makes the service ready"));
    assert!(matches!(
        &started.sends.lifecycle[0].message,
        DynamicLifecycle::Started { .. }
    ));

    let replacing = supervisor
        .receive(
            SearchAddress(215),
            DynamicCommand::Replace {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(216))),
            },
        )
        .unwrap_or_else(|_| panic!("the ready service accepts replacement"));
    let replacement = replacing
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one replacement input is emitted"));
    let (replacement_creation, _, replacement_operation) = replacement.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(
            replacement_creation,
            replacement_operation,
        ))
        .unwrap_or_else(|_| panic!("the replacement input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown retains the accepted replacement"));
    assert!(draining.sends.lifecycle.is_empty());
    let shutdown = draining
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("shutdown stops the exact proxy once"));

    let reported = supervisor
        .on(ChildReport::new(
            creation,
            ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                    replaces: previous,
                    worker: SearchWorker,
                    activation: SearchActivation,
                },
            },
        ))
        .unwrap_or_else(|_| panic!("the exact late replacement report is retained"));
    assert!(reported.sends.lifecycle.is_empty());
    let exited = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("proxy exit waits for shutdown settlement"));
    assert!(exited.sends.lifecycle.is_empty());
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let retired = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("shutdown settlement closes exact retirement"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            operation: 2,
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Replacement
            },
            worker: InterruptedWorker::ProxyReported(ProxyOutcome::Replacement {
                outcome: ReplacementOutcome::WorkerAttemptsExhausted { replaces, .. }
            }),
            ..
        } if replaces.creation() == previous_creation
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Shutdown,
            ..
        }
    ));
    assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
}

#[tokio::test]
async fn global_shutdown_retires_an_available_service_once() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(221)),
            diagnostics: Recipient::global(SearchAddress(222)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(223),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(224))),
            },
        )
        .unwrap_or_else(|_| panic!("the service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy")),
    );
    let input = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let (input_creation, control, input_operation) = input.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(input_creation, input_operation))
        .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let (_proxy, outcome) = ready_search_proxy(control).await;
    let started = supervisor
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("the service becomes available"));
    assert!(matches!(
        &started.sends.lifecycle[0].message,
        DynamicLifecycle::Started { .. }
    ));

    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("global shutdown drains the available service"));
    assert!(draining.sends.lifecycle.is_empty());
    let shutdown = draining
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the available proxy receives one shutdown"));
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the shutdown input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit completes shutdown"));
    assert_eq!(retired.sends.lifecycle.len(), 1);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Shutdown,
            ..
        }
    ));
    assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
}

#[test]
fn global_shutdown_preserves_start_failure_retirement() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(231)),
            diagnostics: Recipient::global(SearchAddress(232)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(233),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(234))),
            },
        )
        .unwrap_or_else(|_| panic!("the service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy")),
    );
    let input = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let failed = supervisor
        .on(SettledItem::Unattempted(input))
        .unwrap_or_else(|_| panic!("the unattempted input begins Start retirement"));
    assert_eq!(failed.sends.lifecycle.len(), 1);
    assert_eq!(failed.sends.diagnostics.len(), 1);
    let shutdown = failed
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("Start failure emits one proxy shutdown"));
    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("global shutdown retains Start retirement"));
    assert!(draining.sends.proxy_operations.is_empty());
    assert!(draining.sends.lifecycle.is_empty());
    let (shutdown_creation, _, shutdown_operation) = shutdown.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the existing shutdown input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit closes Start retirement"));
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::StartFailed,
            ..
        }
    ));
    assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
}

#[test]
fn rejected_shutdown_finishes_stop_before_exact_proxy_exit() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(85)),
            diagnostics: Recipient::global(SearchAddress(86)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(87),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(88))),
            },
        )
        .unwrap_or_else(|_| panic!("the initial service is admitted"));
    let (creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the start creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let stopping = supervisor
        .receive(
            SearchAddress(89),
            DynamicCommand::Stop {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(88))),
            },
        )
        .unwrap_or_else(|_| panic!("stop is accepted while proxy input is pending"));
    let shutdown = stopping
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the proxy receives one shutdown"));
    let worker_rejected = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: initial,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the exact rejected worker input is retained"));
    assert!(worker_rejected.sends.lifecycle.is_empty());
    assert!(matches!(
        &worker_rejected.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyInputRejected { .. },
            ..
        }
    ));
    let failed = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: shutdown,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("shutdown rejection completes the admitted stop"));
    assert!(matches!(
        &failed.sends.lifecycle[0].message,
        DynamicLifecycle::StopFinished {
            result: Err(failure),
            ..
        } if matches!(
            (failure.reason(), failure.stopped()),
            (
                EntryStopFailureReason::ControlRejected(ChildInputReason::ClosedControlLane),
                None,
            )
        )
    ));
    assert!(matches!(
        &failed.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyShutdownRejected { .. },
            ..
        }
    ));
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("exact proxy exit retires the failed stop"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            operation: 1,
            interruption: WorkerChangeInterruption::ExplicitStop { operation: 2 },
            worker: InterruptedWorker::ProxyInputRejected,
            ..
        }
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            cause: EntryRetirement::Stop,
            ..
        }
    ));
}

#[tokio::test]
async fn ready_service_accepts_one_replacement_on_its_current_proxy() {
    for order in [StopOrder::ReceiptFirst, StopOrder::ProxyFirst] {
        let mut supervisor = search_supervisor(
            EntryCapacity::new(1).expect("entry capacity is positive"),
            ActivationPolicy::new(1).expect("activation capacity is positive"),
            ActorDrainPolicy::WaitForActorGraph,
            RootPeers {
                lifecycle: Recipient::global(SearchAddress(31)),
                diagnostics: Recipient::global(SearchAddress(32)),
            },
        );
        let starting = supervisor
            .receive(
                SearchAddress(33),
                DynamicCommand::Start {
                    key: SearchKey("search"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(34))),
                },
            )
            .unwrap_or_else(|_| panic!("the initial service is admitted"));
        let authority = match starting.sends.start_replies.into_deliveries().pop() {
            Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
                Ok(receipt) => receipt.cancel,
                Err(_) => panic!("the initial service is accepted"),
            },
            _ => panic!("one logical start reply is retained"),
        };
        drop(authority);
        let created = starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy"));
        let (creation, committed) = committed_proxy(created);
        let initial = supervisor
            .on(committed)
            .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"))
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("one initial input is emitted"));
        let (input_creation, control, operation) = initial.into_parts();
        let accepted = supervisor
            .on(accepted_search_input(input_creation, operation))
            .unwrap_or_else(|_| panic!("the initial input is accepted"));
        assert!(accepted.sends.proxy_operations.is_empty());
        let (proxy, outcome) = ready_search_proxy(control).await;
        let previous = match &outcome {
            ProxyOutcome::Initial {
                outcome:
                    InitialWorkerOutcome::Resolved {
                        result: WorkerStartResult::Ready { attempt, .. },
                    },
            } => attempt.clone(),
            _ => panic!("the proxy reports its ready worker"),
        };
        let started = supervisor
            .on(ChildReport::new(creation, outcome))
            .unwrap_or_else(|_| panic!("the proxy readiness makes the service available"));
        assert!(matches!(
            &started.sends.lifecycle[0].message,
            DynamicLifecycle::Started { .. }
        ));

        let replacing = supervisor
            .receive(
                SearchAddress(35),
                DynamicCommand::Replace {
                    key: SearchKey("search"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(36))),
                },
            )
            .unwrap_or_else(|_| panic!("a ready service accepts replacement"));

        assert!(replacing.creates.is_empty());
        assert_eq!(replacing.sends.proxy_operations.len(), 1);
        let authority = match replacing.sends.replace_replies.into_deliveries().pop() {
            Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
                Ok(WorkerChangeReceipt { key, cancel }) => {
                    assert_eq!(key.0, "search");
                    assert_eq!(cancel.key().0, "search");
                    cancel
                }
                Err(_) => panic!("replacement returns its accepted receipt"),
            },
            _ => panic!("replacement reply retains its logical route"),
        };
        let replacement = replacing
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("one replacement input is emitted"));
        let (input_creation, _control, operation) = replacement.into_parts();
        let accepted = supervisor
            .on(accepted_search_input(input_creation, operation))
            .unwrap_or_else(|_| panic!("the replacement input is accepted"));
        assert!(accepted.sends.lifecycle.is_empty());

        let (_foreign_proxy, foreign_outcome) =
            ready_search_proxy(ProxyControl::start_with(SearchWorker, SearchActivation)).await;
        let foreign = match foreign_outcome {
            ProxyOutcome::Initial {
                outcome:
                    InitialWorkerOutcome::Resolved {
                        result: WorkerStartResult::Ready { attempt, .. },
                    },
            } => attempt,
            _ => panic!("the independent proxy reports its ready worker"),
        };
        let rejected = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                        replaces: foreign,
                        worker: SearchWorker,
                        activation: SearchActivation,
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("contradictory replacement evidence is diagnosed"));
        assert!(rejected.sends.lifecycle.is_empty());
        assert!(matches!(
            &rejected.sends.diagnostics[0],
            DiagnosticAction::Deliver { diagnostic, .. }
                if matches!(
                    diagnostic,
                    DynamicDiagnostic::RejectedProxyOutcome {
                        report: ChildReport {
                            report: ProxyOutcome::Replacement {
                                outcome: ReplacementOutcome::WorkerAttemptsExhausted { .. }
                            },
                            ..
                        }
                    }
                )
        ));

        let failed = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                        replaces: previous.clone(),
                        worker: SearchWorker,
                        activation: SearchActivation,
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("the completed replacement failure is accepted"));
        assert!(matches!(
            &failed.sends.lifecycle[0].message,
            DynamicLifecycle::ReplacementFailed {
                failure: ReplacementFailure::WorkerAttemptsExhausted { .. },
                ..
            }
        ));

        let committed = supervisor
            .receive(
                SearchAddress(37),
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(38))),
                },
            )
            .unwrap_or_else(|_| panic!("the completed replacement remains correlated"));
        assert!(matches!(
            &committed.sends.cancel_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, CancellationReceipt::Committed { .. })
        ));

        let replacing = supervisor
            .receive(
                SearchAddress(39),
                DynamicCommand::Replace {
                    key: SearchKey("search"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(40))),
                },
            )
            .unwrap_or_else(|_| panic!("the retained ready service accepts another replacement"));
        let authority = match replacing.sends.replace_replies.into_deliveries().pop() {
            Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
                Ok(receipt) => receipt.cancel,
                Err(_) => panic!("the replacement is accepted before proxy input rejection"),
            },
            _ => panic!("the replacement returns one logical reply"),
        };
        let returned = replacing
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("the replacement input is returned intact"));
        let rejected = supervisor
            .on(SettledItem::Unattempted(returned))
            .unwrap_or_else(|_| panic!("input rejection restores the current service"));
        assert!(matches!(
            &rejected.sends.lifecycle[0].message,
            DynamicLifecycle::ReplacementInputRejected { .. }
        ));
        assert!(matches!(
            &rejected.sends.diagnostics[0],
            DiagnosticAction::Deliver {
                diagnostic: DynamicDiagnostic::ProxyInputRejected { .. },
                ..
            }
        ));
        let committed = supervisor
            .receive(
                SearchAddress(41),
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(42))),
                },
            )
            .unwrap_or_else(|_| panic!("the rejected input leaves a terminal operation result"));
        assert!(matches!(
            &committed.sends.cancel_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, CancellationReceipt::Committed { .. })
        ));

        admit_search_replacement(&mut supervisor);
        let rejected_creation = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::Resolved {
                        replaces: previous.clone(),
                        result: WorkerStartResult::CreationRejected {
                            rejection: WorkerCreationRejection::NamespaceExhausted {
                                worker: SearchWorker,
                            },
                            activation: SearchActivation,
                            stopped: None,
                        },
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("the pre-birth failure is accepted"));
        assert!(matches!(
            &rejected_creation.sends.lifecycle[0].message,
            DynamicLifecycle::ReplacementFailed {
                failure: ReplacementFailure::WorkerCreationRejected { .. },
                ..
            }
        ));
        let queried = supervisor
            .receive(
                SearchAddress(43),
                DynamicCommand::Query {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(44))),
                },
            )
            .unwrap_or_else(|_| panic!("the pre-birth failure remains queryable"));
        assert!(matches!(
            &queried.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, QueryReply::Known { status: DynamicStatus::Empty, .. })
        ));

        admit_search_replacement(&mut supervisor);
        let (_successor_proxy, successor_outcome) =
            ready_search_proxy(ProxyControl::start_with(SearchWorker, SearchActivation)).await;
        let successor = match successor_outcome {
            ProxyOutcome::Initial {
                outcome:
                    InitialWorkerOutcome::Resolved {
                        result: WorkerStartResult::Ready { attempt, .. },
                    },
            } => attempt,
            _ => panic!("the independent proxy supplies a distinct ready worker"),
        };
        let replaced = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::Resolved {
                        replaces: previous.clone(),
                        result: WorkerStartResult::Ready {
                            attempt: successor.clone(),
                            readiness: (),
                        },
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("the exact ready replacement is accepted"));
        assert!(matches!(
            &replaced.sends.lifecycle[0].message,
            DynamicLifecycle::Replaced {
                key: SearchKey("search"),
                generation: 1,
                ..
            }
        ));

        let empty_worker = successor.clone();
        let successor_creation = successor.creation();
        let stopped = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::WorkerStopped {
                    worker: successor,
                    stopped: ChildStopped::new(
                        successor_creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ),
                },
            ))
            .unwrap_or_else(|_| panic!("the exact successor stop is accepted"));
        assert!(matches!(
            &stopped.sends.lifecycle[0].message,
            DynamicLifecycle::UnexpectedWorkerStopped { .. }
        ));

        admit_search_replacement(&mut supervisor);
        let (_post_birth_proxy, post_birth_outcome) =
            ready_search_proxy(ProxyControl::start_with(SearchWorker, SearchActivation)).await;
        let post_birth_worker = match post_birth_outcome {
            ProxyOutcome::Initial {
                outcome:
                    InitialWorkerOutcome::Resolved {
                        result: WorkerStartResult::Ready { attempt, .. },
                    },
            } => attempt,
            _ => panic!("the independent proxy supplies a post-birth worker"),
        };
        let post_birth_creation = post_birth_worker.creation();
        let unavailable = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::Resolved {
                        replaces: empty_worker.clone(),
                        result: WorkerStartResult::Unavailable {
                            attempt: post_birth_worker.clone(),
                            drain: ProxyDrain::InitializationStopped {
                                activation: SearchActivation,
                                observed: None,
                                returned: ChildStopped::new(
                                    post_birth_creation,
                                    Ok(Exit::Normal),
                                    Instant::now(),
                                ),
                            },
                        },
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("the post-birth failure is accepted"));
        assert!(matches!(
            &unavailable.sends.lifecycle[0].message,
            DynamicLifecycle::ReplacementFailed {
                failure: ReplacementFailure::WorkerUnavailable { .. },
                ..
            }
        ));

        admit_search_replacement(&mut supervisor);
        let stale = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                        replaces: empty_worker,
                        worker: SearchWorker,
                        activation: SearchActivation,
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("the pre-birth predecessor is now stale"));
        assert!(stale.sends.lifecycle.is_empty());
        assert_eq!(stale.sends.diagnostics.len(), 1);
        let exact = supervisor
            .on(ChildReport::new(
                creation,
                ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                        replaces: post_birth_worker,
                        worker: SearchWorker,
                        activation: SearchActivation,
                    },
                },
            ))
            .unwrap_or_else(|_| panic!("the post-birth worker remains exact"));
        assert!(matches!(
            &exact.sends.lifecycle[0].message,
            DynamicLifecycle::ReplacementFailed {
                failure: ReplacementFailure::WorkerAttemptsExhausted { .. },
                ..
            }
        ));

        let stopping = supervisor
            .receive(
                SearchAddress(49),
                DynamicCommand::Stop {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(50))),
                },
            )
            .unwrap_or_else(|_| panic!("an empty service accepts explicit stop"));
        assert_eq!(stopping.sends.proxy_operations.len(), 1);
        assert!(matches!(
            &stopping.sends.stop_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery) if matches!(delivery.message, Ok(SearchKey("search")))
        ));
        let shutdown = stopping
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("the exact proxy receives one shutdown request"));
        let rejected = supervisor
            .on(SettledItem::Attempted(ItemSettlement::Rejected {
                item: shutdown,
                reason: ChildInputReason::ClosedControlLane,
            }))
            .unwrap_or_else(|_| panic!("a rejected shutdown restores the empty service"));
        assert!(matches!(
            &rejected.sends.lifecycle[0].message,
            DynamicLifecycle::StopFinished {
                result: Err(failure),
                ..
            } if matches!(
                (failure.reason(), failure.stopped()),
                (
                    EntryStopFailureReason::ControlRejected(ChildInputReason::ClosedControlLane),
                    None,
                )
            )
        ));
        assert_eq!(rejected.sends.diagnostics.len(), 1);

        let stopping = supervisor
            .receive(
                SearchAddress(51),
                DynamicCommand::Stop {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(52))),
                },
            )
            .unwrap_or_else(|_| panic!("the restored service accepts another stop"));
        let shutdown = stopping
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("the second stop emits one shutdown request"));
        let draining = supervisor
            .on(ShutdownRequested)
            .unwrap_or_else(|_| panic!("global shutdown retains the accepted stop"));
        assert!(draining.sends.proxy_operations.is_empty());
        assert!(draining.sends.lifecycle.is_empty());
        assert!(matches!(draining.become_, behavior_actors::Step::Continue));
        let mut foreign_creations = CreationSequence::new();
        let first_foreign = foreign_creations.issue();
        assert_eq!(first_foreign.map(CreationId::get), Some(1));
        let foreign_creation = foreign_creations
            .issue()
            .unwrap_or_else(|| panic!("the foreign creation ID exists"));
        let foreign = supervisor
            .on(ChildStopped::new(
                foreign_creation,
                Ok(Exit::Normal),
                Instant::now(),
            ))
            .unwrap_or_else(|_| panic!("a foreign proxy stop is diagnosed"));
        assert_eq!(foreign.sends.diagnostics.len(), 1);
        let (shutdown_creation, _control, shutdown_operation) = shutdown.into_parts();
        let retired = match order {
            StopOrder::ReceiptFirst => {
                let accepted = supervisor
                    .on(accepted_search_input(shutdown_creation, shutdown_operation))
                    .unwrap_or_else(|_| {
                        panic!("shutdown acceptance waits for the exact proxy stop")
                    });
                assert!(accepted.sends.lifecycle.is_empty());
                supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("the exact proxy stop completes explicit stop"))
            }
            StopOrder::ProxyFirst => {
                let stopped = supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| {
                        panic!("the exact proxy may stop before shutdown acceptance")
                    });
                assert!(stopped.sends.lifecycle.is_empty());
                supervisor
                    .on(accepted_search_input(shutdown_creation, shutdown_operation))
                    .unwrap_or_else(|_| panic!("shutdown acceptance reunites with the proxy stop"))
            }
        };
        assert_eq!(retired.sends.lifecycle.len(), 2);
        assert!(matches!(
            &retired.sends.lifecycle[0].message,
            DynamicLifecycle::StopFinished {
                result: Ok(ChildStopped { child, .. }),
                ..
            } if *child == creation
        ));
        assert!(matches!(
            &retired.sends.lifecycle[1].message,
            DynamicLifecycle::EntryRetired {
                cause: EntryRetirement::Stop,
                ..
            }
        ));
        assert!(matches!(retired.become_, behavior_actors::Step::Stop(_)));
        let duplicate = supervisor
            .on(ChildStopped::new(
                creation,
                Ok(Exit::Normal),
                Instant::now(),
            ))
            .unwrap_or_else(|_| panic!("the duplicate proxy stop is diagnosed"));
        assert_eq!(duplicate.sends.diagnostics.len(), 1);

        let queried = supervisor
            .receive(
                SearchAddress(53),
                DynamicCommand::Query {
                    key: SearchKey("search"),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(54))),
                },
            )
            .unwrap_or_else(|_| panic!("retirement releases the keyed entry"));
        assert!(matches!(
            &queried.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, QueryReply::Unknown { .. })
        ));
        drop(proxy);
    }
}

#[tokio::test]
async fn retire_policy_drains_an_unexpected_worker_stop_and_releases_its_key() {
    let initialized = dynamic(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        UnexpectedExit::Retire,
        ActorDrainPolicy::WaitForActorGraph,
        Recipient::<
            MessageProtocol<
                SearchAddress,
                DynamicLifecycle<SearchKey, SearchWorker, SearchActivation>,
            >,
        >::global(SearchAddress(101)),
        DiagnosticDisposition::deliver_to(Recipient::<
            MessageProtocol<
                SearchAddress,
                DynamicDiagnostic<SearchKey, SearchWorker, SearchActivation>,
            >,
        >::global(SearchAddress(102))),
    )
    .initialize()
    .unwrap_or_else(|_| panic!("dynamic supervisor initialization is pure"));
    let mut supervisor = initialized.behavior;
    let starting = supervisor
        .receive(
            SearchAddress(103),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(104))),
            },
        )
        .unwrap_or_else(|_| panic!("the service start is admitted"));
    let (proxy_creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let (input_creation, control, input_operation) = initial.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(input_creation, input_operation))
        .unwrap_or_else(|_| panic!("the proxy accepts its initial worker"));
    assert!(accepted.sends.lifecycle.is_empty());
    let (worker_proxy, ready) = ready_search_proxy(control).await;
    let worker = match &ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        _ => panic!("the proxy reports one ready worker"),
    };
    let available = supervisor
        .on(ChildReport::new(proxy_creation, ready))
        .unwrap_or_else(|_| panic!("the exact ready worker makes the service available"));
    assert!(matches!(
        &available.sends.lifecycle[0].message,
        DynamicLifecycle::Started { .. }
    ));

    let worker_creation = worker.creation();
    let retiring = supervisor
        .on(ChildReport::new(
            proxy_creation,
            ProxyOutcome::WorkerStopped {
                worker,
                stopped: ChildStopped::new(worker_creation, Ok(Exit::Normal), Instant::now()),
            },
        ))
        .unwrap_or_else(|_| panic!("retire policy accepts the exact unexpected worker stop"));
    assert!(matches!(
        &retiring.sends.lifecycle[0].message,
        DynamicLifecycle::UnexpectedWorkerStopped {
            disposition: UnexpectedExit::Retire,
            ..
        }
    ));
    let shutdown = retiring
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("retire policy shuts down the exact stable proxy"));
    let (shutdown_creation, _shutdown_control, shutdown_operation) = shutdown.into_parts();
    let awaiting_proxy_exit = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the exact proxy shutdown is accepted"));
    assert!(awaiting_proxy_exit.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            proxy_creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit closes unexpected-stop retirement"));
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::EntryRetired {
            key: SearchKey("search"),
            cause: EntryRetirement::UnexpectedWorkerStopped,
            ..
        }
    ));
    let queried = supervisor
        .receive(
            SearchAddress(105),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(106))),
            },
        )
        .unwrap_or_else(|_| panic!("retirement releases the keyed service"));
    assert!(matches!(
        &queried.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, QueryReply::Unknown { .. })
    ));
    drop(worker_proxy);
}

#[tokio::test]
async fn corrupt_and_unattempted_proxy_shutdowns_restore_the_ready_service() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(111)),
            diagnostics: Recipient::global(SearchAddress(112)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(113),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(114))),
            },
        )
        .unwrap_or_else(|_| panic!("the service start is admitted"));
    let (proxy_creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
    let (input_creation, control, input_operation) = initial.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(input_creation, input_operation))
        .unwrap_or_else(|_| panic!("the proxy accepts its initial worker"));
    assert!(accepted.sends.lifecycle.is_empty());
    let (worker_proxy, ready) = ready_search_proxy(control).await;
    let started = supervisor
        .on(ChildReport::new(proxy_creation, ready))
        .unwrap_or_else(|_| panic!("the ready worker makes the service available"));
    assert!(matches!(
        &started.sends.lifecycle[0].message,
        DynamicLifecycle::Started { .. }
    ));

    let stopping = supervisor
        .receive(
            SearchAddress(115),
            DynamicCommand::Stop {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(116))),
            },
        )
        .unwrap_or_else(|_| panic!("the ready service accepts explicit stop"));
    let shutdown = stopping
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("explicit stop emits one proxy shutdown"));
    let corrupt = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Corrupt {
            item: shutdown,
            fault: InterpreterFault::CorruptTraversal,
        }))
        .unwrap_or_else(|_| panic!("corrupt interpretation restores the ready service"));
    assert!(matches!(
        &corrupt.sends.lifecycle[0].message,
        DynamicLifecycle::StopFinished {
            result: Err(failure),
            ..
        } if matches!(
            (failure.reason(), failure.stopped()),
            (
                EntryStopFailureReason::InterpreterCorrupt(
                    InterpreterFault::CorruptTraversal
                ),
                None,
            )
        )
    ));
    assert!(matches!(
        &corrupt.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyShutdownRejected {
                input: SettledItem::Attempted(ItemSettlement::Corrupt {
                    fault: InterpreterFault::CorruptTraversal,
                    ..
                }),
                ..
            },
            ..
        }
    ));
    let ready_after_corruption = supervisor
        .receive(
            SearchAddress(117),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(118))),
            },
        )
        .unwrap_or_else(|_| panic!("the restored service remains queryable"));
    assert!(matches!(
        &ready_after_corruption.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, QueryReply::Known {
                status: DynamicStatus::Ready { .. },
                ..
            })
    ));

    let stopping = supervisor
        .receive(
            SearchAddress(119),
            DynamicCommand::Stop {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(120))),
            },
        )
        .unwrap_or_else(|_| panic!("the restored service accepts another stop"));
    let shutdown = stopping
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the second stop emits one proxy shutdown"));
    let unattempted = supervisor
        .on(SettledItem::Unattempted(shutdown))
        .unwrap_or_else(|_| panic!("unattempted interpretation restores the ready service"));
    assert!(matches!(
        &unattempted.sends.lifecycle[0].message,
        DynamicLifecycle::StopFinished {
            result: Err(failure),
            ..
        } if matches!(
            (failure.reason(), failure.stopped()),
            (EntryStopFailureReason::InterpretationSkipped, None)
        )
    ));
    assert!(matches!(
        &unattempted.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyShutdownRejected {
                input: SettledItem::Unattempted(_),
                ..
            },
            ..
        }
    ));
    let ready_after_skip = supervisor
        .receive(
            SearchAddress(121),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(122))),
            },
        )
        .unwrap_or_else(|_| panic!("the twice-restored service remains queryable"));
    assert!(matches!(
        &ready_after_skip.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, QueryReply::Known {
                status: DynamicStatus::Ready { .. },
                ..
            })
    ));
    drop(worker_proxy);
}

#[tokio::test]
async fn a_ready_service_rejects_another_services_worker_stop() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(2).expect("entry capacity admits both services"),
        ActivationPolicy::new(2).expect("both workers may activate"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(123)),
            diagnostics: Recipient::global(SearchAddress(124)),
        },
    );

    let search_start = supervisor
        .receive(
            SearchAddress(125),
            DynamicCommand::Start {
                key: ServiceName::Search.key(),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(126))),
            },
        )
        .unwrap_or_else(|_| panic!("the search service start is admitted"));
    let (search_proxy_creation, search_proxy_committed) = committed_proxy(
        search_start
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the search service creates one proxy")),
    );
    let search_input = supervisor
        .on(search_proxy_committed)
        .unwrap_or_else(|_| panic!("the search proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one search worker input is emitted"));
    let (search_input_creation, search_control, search_operation) = search_input.into_parts();
    let search_input_accepted = supervisor
        .on(accepted_search_input(
            search_input_creation,
            search_operation,
        ))
        .unwrap_or_else(|_| panic!("the search proxy accepts its worker"));
    assert!(search_input_accepted.sends.lifecycle.is_empty());
    let (search_proxy, search_ready) = ready_search_proxy(search_control).await;
    let search_started = supervisor
        .on(ChildReport::new(search_proxy_creation, search_ready))
        .unwrap_or_else(|_| panic!("the search worker makes its service ready"));
    assert!(matches!(
        &search_started.sends.lifecycle[0].message,
        DynamicLifecycle::Started {
            key: SearchKey("search"),
            ..
        }
    ));

    let cache_start = supervisor
        .receive(
            SearchAddress(127),
            DynamicCommand::Start {
                key: ServiceName::Cache.key(),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(128))),
            },
        )
        .unwrap_or_else(|_| panic!("the cache service start is admitted"));
    let (cache_proxy_creation, cache_proxy_committed) = committed_proxy(
        cache_start
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the cache service creates one proxy")),
    );
    let cache_input = supervisor
        .on(cache_proxy_committed)
        .unwrap_or_else(|_| panic!("the cache proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one cache worker input is emitted"));
    let (cache_input_creation, cache_control, cache_operation) = cache_input.into_parts();
    let cache_input_accepted = supervisor
        .on(accepted_search_input(cache_input_creation, cache_operation))
        .unwrap_or_else(|_| panic!("the cache proxy accepts its worker"));
    assert!(cache_input_accepted.sends.lifecycle.is_empty());
    let (cache_proxy, cache_ready) = ready_search_proxy(cache_control).await;
    let cache_worker = match &cache_ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        _ => panic!("the cache proxy reports its ready worker"),
    };
    let cache_worker_creation = cache_worker.creation();
    let cache_started = supervisor
        .on(ChildReport::new(cache_proxy_creation, cache_ready))
        .unwrap_or_else(|_| panic!("the cache worker makes its service ready"));
    assert!(matches!(
        &cache_started.sends.lifecycle[0].message,
        DynamicLifecycle::Started {
            key: SearchKey("cache"),
            ..
        }
    ));

    let rejected = supervisor
        .on(ChildReport::new(
            search_proxy_creation,
            ProxyOutcome::WorkerStopped {
                worker: cache_worker,
                stopped: ChildStopped::new(cache_worker_creation, Ok(Exit::Normal), Instant::now()),
            },
        ))
        .unwrap_or_else(|_| panic!("the foreign worker stop is returned as a diagnostic"));
    assert!(rejected.sends.lifecycle.is_empty());
    assert!(matches!(
        &rejected.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::RejectedProxyOutcome {
                report: ChildReport {
                    child,
                    report: ProxyOutcome::WorkerStopped { worker, stopped },
                },
            },
            ..
        } if *child == search_proxy_creation
            && worker.creation() == cache_worker_creation
            && stopped.child == cache_worker_creation
    ));

    for (service, sender, reply) in [
        (ServiceName::Search, SearchAddress(129), SearchAddress(130)),
        (ServiceName::Cache, SearchAddress(131), SearchAddress(132)),
    ] {
        let queried = supervisor
            .receive(
                sender,
                DynamicCommand::Query {
                    key: service.key(),
                    reply_to: ReplyRoute::logical(Recipient::global(reply)),
                },
            )
            .unwrap_or_else(|_| panic!("the unaffected service remains queryable"));
        assert!(matches!(
            &queried.sends.query_replies.as_slice()[0],
            ReplyDelivery::Logical(delivery)
                if matches!(delivery.message, QueryReply::Known {
                    status: DynamicStatus::Ready { .. },
                    ..
                })
        ));
    }
    drop((search_proxy, cache_proxy));
}

#[test]
fn accepted_start_cancellation_retires_before_fresh_key_reuse() {
    let initialized = dynamic(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        UnexpectedExit::KeepEmpty,
        ActorDrainPolicy::WaitForActorGraph,
        Recipient::<
            MessageProtocol<
                SearchAddress,
                DynamicLifecycle<SearchKey, SearchWorker, SearchActivation>,
            >,
        >::global(SearchAddress(1)),
        DiagnosticDisposition::deliver_to(Recipient::<
            MessageProtocol<
                SearchAddress,
                DynamicDiagnostic<SearchKey, SearchWorker, SearchActivation>,
            >,
        >::global(SearchAddress(2))),
    )
    .initialize()
    .unwrap_or_else(|_| panic!("dynamic supervisor initialization is pure"));
    assert!(initialized.actions.creates.is_empty());
    let mut supervisor = initialized.behavior;
    let reply = ReplyRoute::logical(Recipient::global(SearchAddress(3)));
    let acted = supervisor
        .receive(
            SearchAddress(4),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: reply,
            },
        )
        .unwrap_or_else(|_| panic!("a vacant bounded supervisor accepts its first service"));

    let rejected = supervisor.on(CreationsSettled::new(CreationSettlement::Settled(
        behavior_actors::Creations::empty(),
    )));
    assert!(
        rejected.is_err(),
        "an empty creation settlement is rejected"
    );
    let queried = supervisor
        .receive(
            SearchAddress(11),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(12))),
            },
        )
        .unwrap_or_else(|_| panic!("the retained service remains queryable"));
    assert!(matches!(
        &queried.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(
                delivery.message,
                QueryReply::Known {
                    status: DynamicStatus::CreatingProxy,
                    ..
                }
            )
    ));

    assert_eq!(acted.creates.len(), 1);
    assert_eq!(acted.sends.proxy_observations.len(), 1);
    assert!(acted.sends.proxy_operations.is_empty());
    assert_eq!(acted.sends.start_replies.as_slice().len(), 1);
    let proxy_creation = acted
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one exact proxy creation is retained"));
    match &acted.sends.start_replies.as_slice()[0] {
        ReplyDelivery::Logical(delivery) => match &delivery.message {
            Ok(WorkerChangeReceipt { key, cancel }) => {
                assert_eq!(key.0, "search");
                assert_eq!(cancel.key().0, "search");
            }
            Err(_) => panic!("accepted start returns a receipt"),
        },
        ReplyDelivery::Established(_) => panic!("the reply retains its submitted logical route"),
    }
    let authority = match acted
        .sends
        .start_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("one start reply is retained"))
    {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            Ok(receipt) => receipt.cancel,
            Err(_) => panic!("accepted start returns cancellation authority"),
        },
        ReplyDelivery::Established(_) => panic!("the logical route stays logical"),
    };
    let cancelled = supervisor
        .receive(
            SearchAddress(5),
            DynamicCommand::Cancel {
                authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(6))),
            },
        )
        .unwrap_or_else(|_| panic!("pre-transfer cancellation is accepted"));
    assert!(cancelled.creates.is_empty());
    assert!(cancelled.sends.proxy_operations.is_empty());
    let returned_authority = match cancelled
        .sends
        .cancel_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("cancellation returns one reply"))
    {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            CancellationReceipt::Returned { authority, .. } => {
                assert_eq!(authority.key().0, "search");
                authority
            }
            _ => panic!("pre-transfer cancellation returns the worker submission"),
        },
        ReplyDelivery::Established(_) => panic!("the cancellation route stays logical"),
    };

    let cancelled_again = supervisor
        .receive(
            SearchAddress(61),
            DynamicCommand::Cancel {
                authority: returned_authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(62))),
            },
        )
        .unwrap_or_else(|_| panic!("the current cancelled operation remains correlated"));
    assert!(cancelled_again.creates.is_empty());
    assert!(cancelled_again.sends.proxy_observations.is_empty());
    assert!(cancelled_again.sends.proxy_operations.is_empty());
    assert!(cancelled_again.sends.shutdown_schedules.is_empty());
    assert!(cancelled_again.sends.start_replies.as_slice().is_empty());
    assert!(cancelled_again.sends.replace_replies.as_slice().is_empty());
    assert!(cancelled_again.sends.stop_replies.as_slice().is_empty());
    assert!(cancelled_again.sends.query_replies.as_slice().is_empty());
    assert!(cancelled_again.sends.lifecycle.is_empty());
    assert!(cancelled_again.sends.diagnostics.is_empty());
    assert!(matches!(
        cancelled_again.become_,
        behavior_actors::Step::Continue
    ));
    let cancelled_authority = match cancelled_again
        .sends
        .cancel_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("replayed cancellation returns one reply"))
    {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            CancellationReceipt::Cancelled { authority } => authority,
            _ => panic!("the current cancellation is already cancelled"),
        },
        ReplyDelivery::Established(_) => panic!("the cancellation route stays logical"),
    };

    let (_creation, committed) = committed_proxy(proxy_creation);
    let shutting_down = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("a late proxy birth begins exact retirement"));
    let shutdown = shutting_down
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("late birth emits one proxy shutdown"));
    let (creation, _shutdown, operation) = shutdown.into_parts();
    let waiting_for_exit = supervisor
        .on(accepted_search_input(creation, operation))
        .unwrap_or_else(|_| panic!("shutdown acceptance waits for the exact proxy stop"));
    assert!(waiting_for_exit.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy stop closes retirement"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    match &retired.sends.lifecycle[1].message {
        DynamicLifecycle::EntryRetired { key, cause, .. } => {
            assert_eq!(key.0, "search");
            assert_eq!(*cause, EntryRetirement::Cancellation);
        }
        _ => panic!("retirement publishes its domain cause"),
    }

    let stale = supervisor
        .receive(
            SearchAddress(63),
            DynamicCommand::Cancel {
                authority: cancelled_authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(64))),
            },
        )
        .unwrap_or_else(|_| panic!("a retired operation returns its authority as stale"));
    assert!(stale.creates.is_empty());
    assert!(stale.sends.proxy_observations.is_empty());
    assert!(stale.sends.proxy_operations.is_empty());
    assert!(stale.sends.shutdown_schedules.is_empty());
    assert!(stale.sends.start_replies.as_slice().is_empty());
    assert!(stale.sends.replace_replies.as_slice().is_empty());
    assert!(stale.sends.stop_replies.as_slice().is_empty());
    assert!(stale.sends.query_replies.as_slice().is_empty());
    assert!(stale.sends.lifecycle.is_empty());
    assert!(stale.sends.diagnostics.is_empty());
    assert!(matches!(stale.become_, behavior_actors::Step::Continue));
    let stale_authority = match stale
        .sends
        .cancel_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("stale cancellation returns one reply"))
    {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            CancellationReceipt::Stale { authority } => authority,
            _ => panic!("retirement makes the old cancellation authority stale"),
        },
        ReplyDelivery::Established(_) => panic!("the cancellation route stays logical"),
    };

    let restarted = supervisor
        .receive(
            SearchAddress(7),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(8))),
            },
        )
        .unwrap_or_else(|_| panic!("complete retirement releases entry capacity"));
    let second_creation = restarted
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("reused key owns a fresh proxy creation"));
    let second_authority = match restarted
        .sends
        .start_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("reused key returns cancellation authority"))
    {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            Ok(receipt) => receipt.cancel,
            Err(_) => panic!("reused key is accepted"),
        },
        ReplyDelivery::Established(_) => panic!("the logical route stays logical"),
    };

    let stale_after_reuse = supervisor
        .receive(
            SearchAddress(65),
            DynamicCommand::Cancel {
                authority: stale_authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(66))),
            },
        )
        .unwrap_or_else(|_| panic!("same-key reuse cannot revive an old authority"));
    assert!(stale_after_reuse.creates.is_empty());
    assert!(stale_after_reuse.sends.proxy_observations.is_empty());
    assert!(stale_after_reuse.sends.proxy_operations.is_empty());
    assert!(stale_after_reuse.sends.shutdown_schedules.is_empty());
    assert!(stale_after_reuse.sends.start_replies.as_slice().is_empty());
    assert!(
        stale_after_reuse
            .sends
            .replace_replies
            .as_slice()
            .is_empty()
    );
    assert!(stale_after_reuse.sends.stop_replies.as_slice().is_empty());
    assert!(stale_after_reuse.sends.query_replies.as_slice().is_empty());
    assert!(stale_after_reuse.sends.lifecycle.is_empty());
    assert!(stale_after_reuse.sends.diagnostics.is_empty());
    assert!(matches!(
        stale_after_reuse.become_,
        behavior_actors::Step::Continue
    ));
    match stale_after_reuse
        .sends
        .cancel_replies
        .into_deliveries()
        .pop()
        .unwrap_or_else(|| panic!("same-key stale cancellation returns one reply"))
    {
        ReplyDelivery::Logical(delivery) => match delivery.message {
            CancellationReceipt::Stale { authority } => {
                assert_eq!(authority.key().0, "search");
            }
            _ => panic!("the old authority stays stale after same-key reuse"),
        },
        ReplyDelivery::Established(_) => panic!("the cancellation route stays logical"),
    }
    let reused_status = supervisor
        .receive(
            SearchAddress(67),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(68))),
            },
        )
        .unwrap_or_else(|_| panic!("stale cancellation leaves the reused entry unchanged"));
    match &reused_status.sends.query_replies.as_slice()[0] {
        ReplyDelivery::Logical(delivery) => match &delivery.message {
            QueryReply::Known {
                status: DynamicStatus::CreatingProxy,
                ..
            } => {}
            _ => panic!("the reused entry still owns its pending proxy creation"),
        },
        ReplyDelivery::Established(_) => panic!("the query route stays logical"),
    }
    let second_cancelled = supervisor
        .receive(
            SearchAddress(9),
            DynamicCommand::Cancel {
                authority: second_authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(10))),
            },
        )
        .unwrap_or_else(|_| panic!("the reused entry is cancelled"));
    assert!(second_cancelled.sends.proxy_operations.is_empty());
    assert_eq!(second_cancelled.sends.cancel_replies.as_slice().len(), 1);
    let (second_creation, committed) = committed_proxy(second_creation);
    let second_shutdown = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the second late birth begins retirement"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the second proxy receives one shutdown"));
    let stopped_first = supervisor
        .on(ChildStopped::new(
            second_creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("proxy stop may precede shutdown settlement"));
    assert!(stopped_first.sends.lifecycle.is_empty());
    let (second_creation, _shutdown, second_operation) = second_shutdown.into_parts();
    let second_retired = supervisor
        .on(accepted_search_input(second_creation, second_operation))
        .unwrap_or_else(|_| panic!("shutdown settlement closes the reverse order"));
    assert_eq!(second_retired.sends.lifecycle.len(), 2);
    match &second_retired.sends.lifecycle[1].message {
        DynamicLifecycle::EntryRetired {
            key,
            generation,
            cause,
        } => {
            assert_eq!(key.0, "search");
            assert_eq!(*generation, 2);
            assert_eq!(*cause, EntryRetirement::Cancellation);
        }
        _ => panic!("reverse order publishes one retirement"),
    }
}

#[tokio::test]
async fn committed_proxy_creation_emits_the_exact_initial_worker_input() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(3).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(11)),
            diagnostics: Recipient::global(SearchAddress(12)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(13),
            DynamicCommand::Start {
                key: SearchKey("index"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(14))),
            },
        )
        .unwrap_or_else(|_| panic!("start is admitted"));
    let creation = starting
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one proxy creation is emitted"));
    let (creation, committed) = committed_proxy(creation);

    let advancing = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the exact proxy creation advances its service"));

    assert_eq!(advancing.sends.proxy_operations.len(), 1);
    let operation = advancing
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial worker input is emitted"));
    assert_eq!(operation.creation(), creation);

    let mut waiting_creations = VecDeque::new();
    for (key, sender, reply) in [
        (SearchKey("cache"), SearchAddress(17), SearchAddress(18)),
        (SearchKey("billing"), SearchAddress(19), SearchAddress(20)),
    ] {
        let start = supervisor
            .receive(
                sender,
                DynamicCommand::Start {
                    key,
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(reply)),
                },
            )
            .unwrap_or_else(|_| panic!("entry capacity accepts the waiting service"));
        let created = start
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the waiting service creates one proxy"));
        let (waiting_creation, committed) = committed_proxy(created);
        let waiting = supervisor
            .on(committed)
            .unwrap_or_else(|_| panic!("the committed proxy waits for activation capacity"));
        assert!(waiting.sends.proxy_operations.is_empty());
        waiting_creations.push_back(waiting_creation);
    }
    let waiting_creation = waiting_creations
        .pop_front()
        .unwrap_or_else(|| panic!("the first waiting proxy is retained"));
    let last_creation = waiting_creations
        .pop_front()
        .unwrap_or_else(|| panic!("the last waiting proxy is retained"));

    let (operation_creation, control, operation_id) = operation.into_parts();
    let settled = supervisor
        .on(accepted_search_input(operation_creation, operation_id))
        .unwrap_or_else(|_| panic!("the exact input receipt advances its service"));
    assert!(settled.creates.is_empty());
    assert!(settled.sends.proxy_operations.is_empty());

    let initialized_proxy = StableProxy::<SearchWorker, SearchActivation>::activated()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"));
    let mut proxy = initialized_proxy.behavior;
    let worker_creation = proxy
        .on(control)
        .unwrap_or_else(|_| panic!("the proxy accepts its owner input"))
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the proxy creates one worker"));
    let (worker_creation, _worker, worker_kind) = worker_creation.into_parts();
    let initializing = proxy
        .on(behavior_actors::CreationsSettled::new(
            CreationSettlement::Settled(
                [SettledItem::Attempted(ItemSettlement::Accepted(
                    ChildCreationOutcome::Established {
                        established: EstablishedCreation::installed(
                            worker_creation,
                            worker_kind,
                            EstablishedRecipient::issued(SearchEndpoint),
                        ),
                    },
                ))]
                .into_iter()
                .collect(),
            ),
        ))
        .unwrap_or_else(|_| panic!("the worker creation commits"));
    let initialization = initializing
        .sends
        .worker_initializations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the worker initialization is requested"));
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("worker initialization accepts activation"));
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one activation is requested"));
    let activation_started = proxy
        .on(activation.started())
        .unwrap_or_else(|_| panic!("activation starts"));
    assert!(activation_started.creates.is_empty());
    assert!(activation_started.sends.owner_outcomes.is_empty());
    let ready = proxy
        .on(activation.activate().await)
        .unwrap_or_else(|_| panic!("activation completes"));
    let outcome = ready
        .sends
        .owner_outcomes
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("the proxy reports readiness"))
        .into_inner();
    let started = supervisor
        .on(ChildReport::new(creation, outcome))
        .unwrap_or_else(|_| panic!("the exact proxy readiness advances its service"));
    assert_eq!(started.sends.lifecycle.len(), 1);
    match &started.sends.lifecycle[0].message {
        DynamicLifecycle::Started { key, .. } => assert_eq!(key.0, "index"),
        _ => panic!("readiness publishes the started lifecycle"),
    }
    let authorized = started
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("released capacity authorizes the waiting service"));
    assert_eq!(authorized.creation(), waiting_creation);

    let next = supervisor
        .on(SettledItem::Unattempted(authorized))
        .unwrap_or_else(|_| panic!("returned input releases its activation capacity"));
    let next_operations = next.sends.proxy_operations.into_items();
    assert_eq!(next_operations.len(), 2);
    assert_eq!(next_operations[1].creation(), last_creation);

    let queried = supervisor
        .receive(
            SearchAddress(15),
            DynamicCommand::Query {
                key: SearchKey("index"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(16))),
            },
        )
        .unwrap_or_else(|_| panic!("query remains total"));
    match &queried.sends.query_replies.as_slice()[0] {
        ReplyDelivery::Logical(delivery) => match &delivery.message {
            QueryReply::Known {
                status: DynamicStatus::Ready { .. },
                ..
            } => {}
            _ => panic!("the committed worker is publicly ready"),
        },
        ReplyDelivery::Established(_) => panic!("query retains its submitted logical route"),
    }
}

#[test]
fn activation_capacity_two_preserves_admission_order() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(3).expect("entry capacity is positive"),
        ActivationPolicy::new(2).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(301)),
            diagnostics: Recipient::global(SearchAddress(302)),
        },
    );
    let mut creations = VecDeque::new();
    let mut occupied = VecDeque::new();

    for (key, sender) in [
        (SearchKey("search"), SearchAddress(303)),
        (SearchKey("cache"), SearchAddress(304)),
        (SearchKey("billing"), SearchAddress(305)),
    ] {
        let started = supervisor
            .receive(
                sender,
                DynamicCommand::Start {
                    key,
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(sender)),
                },
            )
            .unwrap_or_else(|_| panic!("entry capacity accepts three services"));
        assert_eq!(started.creates.len(), 1);
        assert_eq!(started.sends.proxy_observations.len(), 1);
        assert_eq!(started.sends.start_replies.as_slice().len(), 1);
        assert!(started.sends.proxy_operations.is_empty());
        assert!(started.sends.shutdown_schedules.is_empty());
        assert!(started.sends.replace_replies.as_slice().is_empty());
        assert!(started.sends.stop_replies.as_slice().is_empty());
        assert!(started.sends.query_replies.as_slice().is_empty());
        assert!(started.sends.cancel_replies.as_slice().is_empty());
        assert!(started.sends.lifecycle.is_empty());
        assert!(started.sends.diagnostics.is_empty());
        assert!(matches!(started.become_, behavior_actors::Step::Continue));
        let created = started
            .creates
            .into_iter()
            .next()
            .expect("accepted start creates one proxy");
        let (creation, committed) = committed_proxy(created);
        creations.push_back(creation);

        let advanced = supervisor
            .on(committed)
            .unwrap_or_else(|_| panic!("proxy creation is exact"));
        assert!(advanced.creates.is_empty());
        assert!(advanced.sends.proxy_observations.is_empty());
        assert!(advanced.sends.shutdown_schedules.is_empty());
        assert!(advanced.sends.start_replies.as_slice().is_empty());
        assert!(advanced.sends.replace_replies.as_slice().is_empty());
        assert!(advanced.sends.stop_replies.as_slice().is_empty());
        assert!(advanced.sends.query_replies.as_slice().is_empty());
        assert!(advanced.sends.cancel_replies.as_slice().is_empty());
        assert!(advanced.sends.lifecycle.is_empty());
        assert!(advanced.sends.diagnostics.is_empty());
        assert!(matches!(advanced.become_, behavior_actors::Step::Continue));

        let operations = advanced.sends.proxy_operations.into_items();
        match occupied.len() {
            0 | 1 => {
                assert_eq!(operations.len(), 1);
                assert_eq!(operations[0].creation(), creation);
                occupied.push_back(
                    operations
                        .into_iter()
                        .next()
                        .expect("available authorization emits one input"),
                );
            }
            _ => assert!(operations.is_empty()),
        }
    }

    let first = occupied
        .pop_front()
        .expect("the first service occupies authorization");
    assert_eq!(first.creation(), creations[0]);
    assert_eq!(occupied[0].creation(), creations[1]);
    let released = supervisor
        .on(SettledItem::Unattempted(first))
        .unwrap_or_else(|_| panic!("returned input releases one authorization"));
    assert!(released.creates.is_empty());
    assert!(released.sends.proxy_observations.is_empty());
    assert!(released.sends.shutdown_schedules.is_empty());
    assert!(released.sends.start_replies.as_slice().is_empty());
    assert!(released.sends.replace_replies.as_slice().is_empty());
    assert!(released.sends.stop_replies.as_slice().is_empty());
    assert!(released.sends.query_replies.as_slice().is_empty());
    assert!(released.sends.cancel_replies.as_slice().is_empty());
    assert_eq!(released.sends.lifecycle.len(), 1);
    assert!(matches!(
        &released.sends.lifecycle[0].message,
        DynamicLifecycle::StartInputRejected { key, .. } if key.0 == "search"
    ));
    assert_eq!(released.sends.diagnostics.len(), 1);
    assert!(matches!(
        &released.sends.diagnostics[0],
        DiagnosticAction::Deliver { diagnostic, .. }
            if matches!(
                diagnostic,
                DynamicDiagnostic::ProxyInputRejected { key, .. } if key.0 == "search"
            )
    ));
    assert!(matches!(released.become_, behavior_actors::Step::Continue));
    let next = released.sends.proxy_operations.into_items();
    assert_eq!(next.len(), 2);
    assert_eq!(next[0].creation(), creations[0]);
    assert_eq!(next[1].creation(), creations[2]);
}

#[tokio::test]
async fn global_shutdown_orders_local_start_and_replacement_by_key() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(3).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(93)),
            diagnostics: Recipient::global(SearchAddress(94)),
        },
    );
    let service = supervisor
        .receive(
            SearchAddress(95),
            DynamicCommand::Start {
                key: SearchKey("service"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(96))),
            },
        )
        .unwrap_or_else(|_| panic!("the service start is admitted"));
    let (service_creation, service_created) = committed_proxy(
        service
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy")),
    );
    let service_input = supervisor
        .on(service_created)
        .unwrap_or_else(|_| panic!("the service proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one service input is emitted"));
    let (creation, control, operation) = service_input.into_parts();
    let accepted = supervisor
        .on(accepted_search_input(creation, operation))
        .unwrap_or_else(|_| panic!("the service input is accepted"));
    assert!(accepted.sends.lifecycle.is_empty());
    let (proxy, outcome) = ready_search_proxy(control).await;
    let started = supervisor
        .on(ChildReport::new(service_creation, outcome))
        .unwrap_or_else(|_| panic!("the service becomes available"));
    assert!(matches!(
        &started.sends.lifecycle[0].message,
        DynamicLifecycle::Started { .. }
    ));

    let occupied = supervisor
        .receive(
            SearchAddress(97),
            DynamicCommand::Start {
                key: SearchKey("occupied"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(98))),
            },
        )
        .unwrap_or_else(|_| panic!("the occupied start is admitted"));
    let (occupied_creation, occupied_created) = committed_proxy(
        occupied
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the occupied service creates one proxy")),
    );
    let occupied_input = supervisor
        .on(occupied_created)
        .unwrap_or_else(|_| panic!("the occupied proxy receives its worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the occupied input is emitted"));
    let (creation, _, operation) = occupied_input.into_parts();
    let occupied = supervisor
        .on(accepted_search_input(creation, operation))
        .unwrap_or_else(|_| panic!("the occupied input takes activation capacity"));
    assert!(occupied.sends.proxy_operations.is_empty());

    let waiting = supervisor
        .receive(
            SearchAddress(99),
            DynamicCommand::Start {
                key: SearchKey("waiting"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(100))),
            },
        )
        .unwrap_or_else(|_| panic!("the waiting start is admitted"));
    let (waiting_creation, waiting_created) = committed_proxy(
        waiting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the waiting service creates one proxy")),
    );
    let waiting = supervisor
        .on(waiting_created)
        .unwrap_or_else(|_| panic!("the waiting worker remains local"));
    assert!(waiting.sends.proxy_operations.is_empty());
    let replacing = supervisor
        .receive(
            SearchAddress(101),
            DynamicCommand::Replace {
                key: SearchKey("service"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(102))),
            },
        )
        .unwrap_or_else(|_| panic!("replacement waits behind occupied capacity"));
    assert!(replacing.sends.proxy_operations.is_empty());

    let draining = supervisor
        .on(ShutdownRequested)
        .unwrap_or_else(|_| panic!("shutdown consumes both local workers"));
    let shutdowns = draining.sends.proxy_operations.into_items();
    assert_eq!(shutdowns.len(), 3);
    assert_eq!(shutdowns[0].creation(), occupied_creation);
    assert_eq!(shutdowns[1].creation(), service_creation);
    assert_eq!(shutdowns[2].creation(), waiting_creation);
    assert!(matches!(
        &draining.sends.lifecycle[0].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            key: SearchKey("service"),
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Replacement
            },
            worker: InterruptedWorker::Submission(_),
            ..
        }
    ));
    assert!(matches!(
        &draining.sends.lifecycle[1].message,
        DynamicLifecycle::WorkerChangeInterrupted {
            key: SearchKey("waiting"),
            interruption: WorkerChangeInterruption::SupervisorShutdown {
                change: WorkerChange::Start
            },
            worker: InterruptedWorker::Submission(_),
            ..
        }
    ));
    assert!(matches!(draining.become_, behavior_actors::Step::Continue));
    drop(proxy);
}

#[test]
fn cancelled_transferred_start_retains_its_rejected_input_outcome() {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(117)),
            diagnostics: Recipient::global(SearchAddress(118)),
        },
    );
    let starting = supervisor
        .receive(
            SearchAddress(119),
            DynamicCommand::Start {
                key: SearchKey("search"),
                submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(120))),
            },
        )
        .unwrap_or_else(|_| panic!("the service start is admitted"));
    let authority = match starting.sends.start_replies.into_deliveries().pop() {
        Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
            Ok(receipt) => receipt.cancel,
            Err(_) => panic!("the accepted start returns cancellation authority"),
        },
        _ => panic!("the accepted start returns one logical reply"),
    };
    let (proxy_creation, committed) = committed_proxy(
        starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the service creates one proxy")),
    );
    let initial = supervisor
        .on(committed)
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial worker"))
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("one initial proxy input is transferred"));
    let cancelling = supervisor
        .receive(
            SearchAddress(121),
            DynamicCommand::Cancel {
                authority,
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(122))),
            },
        )
        .unwrap_or_else(|_| panic!("post-transfer cancellation is admitted"));
    assert!(matches!(
        &cancelling.sends.cancel_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, CancellationReceipt::Pending { .. })
    ));
    let shutdown = cancelling
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("cancellation shuts down the exact stable proxy"));
    let rejected = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Rejected {
            item: initial,
            reason: ChildInputReason::ClosedControlLane,
        }))
        .unwrap_or_else(|_| panic!("the exact rejected input enters diagnostic custody"));
    assert!(rejected.sends.lifecycle.is_empty());
    assert!(matches!(
        &rejected.sends.diagnostics[0],
        DiagnosticAction::Deliver {
            diagnostic: DynamicDiagnostic::ProxyInputRejected { .. },
            ..
        }
    ));
    let (shutdown_creation, _shutdown_control, shutdown_operation) = shutdown.into_parts();
    let awaiting_proxy_exit = supervisor
        .on(accepted_search_input(shutdown_creation, shutdown_operation))
        .unwrap_or_else(|_| panic!("the exact proxy shutdown is accepted"));
    assert!(awaiting_proxy_exit.sends.lifecycle.is_empty());
    let retired = supervisor
        .on(ChildStopped::new(
            proxy_creation,
            Ok(Exit::Normal),
            Instant::now(),
        ))
        .unwrap_or_else(|_| panic!("the exact proxy exit closes cancellation"));
    assert_eq!(retired.sends.lifecycle.len(), 2);
    assert!(matches!(
        &retired.sends.lifecycle[0].message,
        DynamicLifecycle::OperationCancelled {
            change: WorkerChange::Start,
            outcome: CancellationOutcome::ProxyInputRejected,
            ..
        }
    ));
    assert!(matches!(
        &retired.sends.lifecycle[1].message,
        DynamicLifecycle::EntryRetired {
            key: SearchKey("search"),
            cause: EntryRetirement::Cancellation,
            ..
        }
    ));
    let queried = supervisor
        .receive(
            SearchAddress(123),
            DynamicCommand::Query {
                key: SearchKey("search"),
                reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(124))),
            },
        )
        .unwrap_or_else(|_| panic!("completed cancellation releases its key"));
    assert!(matches!(
        &queried.sends.query_replies.as_slice()[0],
        ReplyDelivery::Logical(delivery)
            if matches!(delivery.message, QueryReply::Unknown { .. })
    ));
}

#[test]
fn cancelled_transferred_start_reunites_its_late_report_with_proxy_retirement() {
    for order in [
        CancellationOrder::ReportFirst,
        CancellationOrder::ReportLast,
    ] {
        let mut supervisor = search_supervisor(
            EntryCapacity::new(2).expect("entry capacity is positive"),
            ActivationPolicy::new(1).expect("activation capacity is positive"),
            ActorDrainPolicy::WaitForActorGraph,
            RootPeers {
                lifecycle: Recipient::global(SearchAddress(21)),
                diagnostics: Recipient::global(SearchAddress(22)),
            },
        );
        let starting = supervisor
            .receive(
                SearchAddress(23),
                DynamicCommand::Start {
                    key: SearchKey("search"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(24))),
                },
            )
            .unwrap_or_else(|_| panic!("the initial service is admitted"));
        let authority = match starting.sends.start_replies.into_deliveries().pop() {
            Some(ReplyDelivery::Logical(delivery)) => match delivery.message {
                Ok(receipt) => receipt.cancel,
                Err(_) => panic!("the initial service is accepted"),
            },
            _ => panic!("one logical start reply is retained"),
        };
        let created = starting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the initial service creates one proxy"));
        let (creation, committed) = committed_proxy(created);
        let emitted = supervisor
            .on(committed)
            .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"))
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("one initial proxy input is emitted"));
        let (operation_creation, _control, operation) = emitted.into_parts();
        let accepted = supervisor
            .on(accepted_search_input(operation_creation, operation))
            .unwrap_or_else(|_| panic!("the initial proxy input is accepted"));
        assert!(accepted.sends.proxy_operations.is_empty());

        let waiting = supervisor
            .receive(
                SearchAddress(25),
                DynamicCommand::Start {
                    key: SearchKey("cache"),
                    submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(26))),
                },
            )
            .unwrap_or_else(|_| panic!("the waiting service is admitted"));
        let waiting = waiting
            .creates
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("the waiting service creates one proxy"));
        let (waiting_creation, committed) = committed_proxy(waiting);
        let waiting = supervisor
            .on(committed)
            .unwrap_or_else(|_| panic!("the second proxy waits for capacity"));
        assert!(waiting.sends.proxy_operations.is_empty());

        let cancelling = supervisor
            .receive(
                SearchAddress(27),
                DynamicCommand::Cancel {
                    authority,
                    reply_to: ReplyRoute::logical(Recipient::global(SearchAddress(28))),
                },
            )
            .unwrap_or_else(|_| panic!("post-transfer cancellation is admitted"));
        match &cancelling.sends.cancel_replies.as_slice()[0] {
            ReplyDelivery::Logical(delivery) => match delivery.message {
                CancellationReceipt::Pending { .. } => {}
                _ => panic!("a transferred worker cannot be returned"),
            },
            ReplyDelivery::Established(_) => panic!("the cancellation reply route stays logical"),
        }
        let shutdown = cancelling
            .sends
            .proxy_operations
            .into_items()
            .pop()
            .unwrap_or_else(|| panic!("cancellation shuts down the exact proxy"));
        let (shutdown_creation, _control, shutdown_operation) = shutdown.into_parts();
        let (authorized, lifecycle) = match order {
            CancellationOrder::ReportFirst => {
                let reported = supervisor
                    .on(overlapping_initial_report(creation))
                    .unwrap_or_else(|_| panic!("the exact late report settles cancelled work"));
                assert!(reported.sends.lifecycle.is_empty());
                let stopped = supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("proxy exit waits for shutdown settlement"));
                assert!(stopped.sends.lifecycle.is_empty());
                let retired = supervisor
                    .on(accepted_search_input(shutdown_creation, shutdown_operation))
                    .unwrap_or_else(|_| panic!("shutdown settlement closes cancellation"));
                (
                    reported.sends.proxy_operations.into_items(),
                    retired.sends.lifecycle,
                )
            }
            CancellationOrder::ReportLast => {
                let stopped = supervisor
                    .on(ChildStopped::new(
                        creation,
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap_or_else(|_| panic!("proxy exit may precede both settlements"));
                assert!(stopped.sends.lifecycle.is_empty());
                let settled = supervisor
                    .on(accepted_search_input(shutdown_creation, shutdown_operation))
                    .unwrap_or_else(|_| panic!("shutdown may settle before the late report"));
                assert!(settled.sends.lifecycle.is_empty());
                let reported = supervisor
                    .on(overlapping_initial_report(creation))
                    .unwrap_or_else(|_| panic!("the late report closes cancellation"));
                (
                    reported.sends.proxy_operations.into_items(),
                    reported.sends.lifecycle,
                )
            }
        };
        assert_eq!(authorized.len(), 1);
        assert_eq!(authorized[0].creation(), waiting_creation);
        assert_eq!(lifecycle.len(), 2);
        match &lifecycle[0].message {
            DynamicLifecycle::OperationCancelled {
                change,
                outcome:
                    CancellationOutcome::ProxyReported {
                        outcome: ProxyOutcome::Initial { .. },
                    },
                ..
            } => assert_eq!(*change, WorkerChange::Start),
            _ => panic!("cancellation owns the exact late initial report"),
        }
        match &lifecycle[1].message {
            DynamicLifecycle::EntryRetired { cause, .. } => {
                assert_eq!(*cause, EntryRetirement::Cancellation);
            }
            _ => panic!("the drained cancelled entry retires once"),
        }
    }
}

fn admission_queries_match_one_customer_catalogue(bytes: Vec<u8>) {
    let mut supervisor = search_supervisor(
        EntryCapacity::new(1).expect("entry capacity is positive"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        ActorDrainPolicy::WaitForActorGraph,
        RootPeers {
            lifecycle: Recipient::global(SearchAddress(201)),
            diagnostics: Recipient::global(SearchAddress(202)),
        },
    );
    let mut catalogue = CustomerCatalogue::Empty;

    for (position, byte) in bytes.into_iter().enumerate() {
        let sender = SearchAddress(
            1_000 + u64::try_from(position).expect("generated sequence length fits u64"),
        );
        let service = match byte % 2 {
            0 => ServiceName::Search,
            _ => ServiceName::Cache,
        };
        match byte % 4 {
            0 | 1 => {
                let expected = match &catalogue {
                    CustomerCatalogue::Empty => Ok(()),
                    CustomerCatalogue::Pending {
                        service: current, ..
                    } if *current == service => Err(StartRejection::AlreadyExists),
                    CustomerCatalogue::Pending { .. } => Err(StartRejection::AtCapacity),
                };
                let acted = supervisor
                    .receive(
                        sender,
                        DynamicCommand::Start {
                            key: service.key(),
                            submission: WorkerSubmission::activated(SearchWorker, SearchActivation),
                            reply_to: ReplyRoute::logical(Recipient::global(sender)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("start admission is total"));

                assert!(acted.sends.proxy_operations.is_empty());
                assert!(acted.sends.shutdown_schedules.is_empty());
                assert!(acted.sends.replace_replies.as_slice().is_empty());
                assert!(acted.sends.stop_replies.as_slice().is_empty());
                assert!(acted.sends.query_replies.as_slice().is_empty());
                assert!(acted.sends.cancel_replies.as_slice().is_empty());
                assert!(acted.sends.lifecycle.is_empty());
                assert!(acted.sends.diagnostics.is_empty());
                assert!(matches!(acted.become_, behavior_actors::Step::Continue));

                let mut replies = acted.sends.start_replies.into_deliveries().into_iter();
                let reply = replies.next().expect("every start receives one reply");
                assert!(replies.next().is_none());
                let ReplyDelivery::Logical(delivery) = reply else {
                    panic!("the customer supplied a logical reply route");
                };
                match (expected, delivery.message) {
                    (Ok(()), Ok(receipt)) => {
                        assert_eq!(receipt.key.0, service.key().0);
                        assert_eq!(receipt.cancel.key().0, service.key().0);
                        assert_eq!(acted.sends.proxy_observations.len(), 1);
                        let mut creations = acted.creates.into_iter();
                        let creation = creations.next().expect("accepted start creates one proxy");
                        assert!(creations.next().is_none());
                        catalogue = CustomerCatalogue::Pending {
                            service,
                            creation,
                            authority: receipt.cancel,
                        };
                    }
                    (Err(reason), Err(rejection)) => {
                        assert_eq!(rejection.key.0, service.key().0);
                        assert_eq!(rejection.reason, reason);
                        assert_eq!(
                            rejection.submission,
                            WorkerSubmission::activated(SearchWorker, SearchActivation),
                        );
                        assert!(acted.creates.is_empty());
                        assert!(acted.sends.proxy_observations.is_empty());
                    }
                    (Ok(()), Err(_)) => panic!("the empty catalogue accepts one service"),
                    (Err(_), Ok(_)) => panic!("the retained service occupies its entry"),
                }
            }
            _ => {
                let acted = supervisor
                    .receive(
                        sender,
                        DynamicCommand::Query {
                            key: service.key(),
                            reply_to: ReplyRoute::logical(Recipient::global(sender)),
                        },
                    )
                    .unwrap_or_else(|_| panic!("query is total"));

                assert!(acted.creates.is_empty());
                assert!(acted.sends.proxy_observations.is_empty());
                assert!(acted.sends.proxy_operations.is_empty());
                assert!(acted.sends.shutdown_schedules.is_empty());
                assert!(acted.sends.start_replies.as_slice().is_empty());
                assert!(acted.sends.replace_replies.as_slice().is_empty());
                assert!(acted.sends.stop_replies.as_slice().is_empty());
                assert!(acted.sends.cancel_replies.as_slice().is_empty());
                assert!(acted.sends.lifecycle.is_empty());
                assert!(acted.sends.diagnostics.is_empty());
                assert!(matches!(acted.become_, behavior_actors::Step::Continue));

                let mut replies = acted.sends.query_replies.into_deliveries().into_iter();
                let reply = replies.next().expect("every query receives one reply");
                assert!(replies.next().is_none());
                let ReplyDelivery::Logical(delivery) = reply else {
                    panic!("the customer supplied a logical reply route");
                };
                match (&catalogue, delivery.message) {
                    (
                        CustomerCatalogue::Pending {
                            service: current, ..
                        },
                        QueryReply::Known { key, status },
                    ) if *current == service => {
                        assert_eq!(key.0, service.key().0);
                        assert!(matches!(status, DynamicStatus::CreatingProxy));
                    }
                    (
                        CustomerCatalogue::Pending {
                            service: current, ..
                        },
                        QueryReply::Unknown { key },
                    ) if *current != service => assert_eq!(key.0, service.key().0),
                    (CustomerCatalogue::Empty, QueryReply::Unknown { key }) => {
                        assert_eq!(key.0, service.key().0);
                    }
                    _ => panic!("query diverged from the customer catalogue"),
                }
            }
        }
    }

    if let CustomerCatalogue::Pending {
        service,
        creation,
        authority,
    } = catalogue
    {
        assert_eq!(authority.key().0, service.key().0);
        let _externally_owned = (creation, authority);
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        max_shrink_iters: 100_000,
        ..proptest::test_runner::Config::default()
    })]

    #[test]
    fn arbitrary_admission_queries_match_one_customer_catalogue(
        bytes in vec(0_u8..4, 0..256),
    ) {
        admission_queries_match_one_customer_catalogue(bytes);
    }
}

#[test]
fn short_admission_query_sequences_are_exhaustive() {
    let mut sequence_count = 0_usize;
    for length in 0_u32..=5 {
        for encoded in 0..4_usize.pow(length) {
            let mut remainder = encoded;
            let mut bytes =
                Vec::with_capacity(usize::try_from(length).expect("exhaustive length fits usize"));
            for _ in 0..length {
                bytes.push(u8::try_from(remainder % 4).expect("base-four digit fits u8"));
                remainder /= 4;
            }
            admission_queries_match_one_customer_catalogue(bytes);
            sequence_count += 1;
        }
    }
    assert_eq!(sequence_count, 1_365);
}
