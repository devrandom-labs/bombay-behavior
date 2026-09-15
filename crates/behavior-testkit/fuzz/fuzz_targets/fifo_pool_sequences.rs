#![no_main]
//! Arbitrary direct-worker assignment, rejection, exit, and shutdown order.

use core::future::Future;
use core::task::{Context, Poll, Waker};
use std::collections::{BTreeSet, VecDeque};
use std::convert::Infallible;
use std::time::Instant;

use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, AssignWorker, AssignedReturnReason, Assignment,
    AssignmentReceipt, BacklogCapacity, Completion, DiagnosticDisposition, FifoCommand, FifoEvent,
    FifoOutcome, FifoOutcomeKind, FifoPool, ImmediateActivation, Interruption, OrderedRoles,
    PoolFailureReaction, PoolRecovery, QueuedReturnReason, SubmissionId,
    WorkerInitializationOutcome, WorkerSubmission, fifo,
};
use behavior_actors::{
    Activate as _, Active, ChildStopped, EstablishedShutdownResolved, Exit, ReplyDelivery,
    ShutdownId, ShutdownRejection, StopOnShutdown,
};
use behavior_core::{
    ActionItemResult, Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase,
    ChildCreationOutcome, ChildHead, ChildReport, CreateChild, CreationId, CreationSequence,
    CreationSettlement, CreationsSettled, EndpointAddress, EstablishedCreation,
    EstablishedRecipient, ExactDeliveryReason, InterpreterRequests, ItemSettlement,
    MessageProtocol, Never, NoBirths, Protocol, Recipient, ReportToParent, SettledItem, Step, User,
};
use libfuzzer_sys::fuzz_target;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddress(u64);

impl Address for RuntimeAddress {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkerEndpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = WorkerEndpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Search,
}

#[derive(Debug, Eq, PartialEq)]
struct SearchWorker;

type SearchPool =
    Active<FifoPool<Role, SearchWorker, ImmediateActivation, Never, Infallible, u8, u16>>;

impl Protocol for SearchWorker {
    type Addr = RuntimeAddress;
    type Msg = Assignment<u8>;
}

impl BehaviorBase for SearchWorker {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for SearchWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, Assignment<u8>>;
    type Sends = InterpreterRequests<ReportToParent<Completion<u16>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, assignment: Self::Event) -> BehaviorActed<Self> {
        let result = u16::from(*assignment.message.payload()) + 100;
        Ok(Actions::cont().with_send(assignment.message.complete(result)))
    }
}

fn prepare_worker(_: &Role) -> Result<WorkerSubmission<SearchWorker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(SearchWorker))
}

fn created_worker(
    creation: CreateChild<RuntimeAddress, StopOnShutdown<SearchWorker>>,
) -> CreationsSettled<RuntimeAddress, StopOnShutdown<SearchWorker>> {
    let (worker, _, kind) = creation.into_parts();
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::<StopOnShutdown<SearchWorker>, ChildHead>::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(WorkerEndpoint),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

fn ready_pool() -> (SearchPool, CreationId) {
    let pool = fifo(
        prepare_worker,
        OrderedRoles::new(Role::Search, core::iter::empty()).expect("one role is a valid roster"),
        ActivationPolicy::new(1).expect("one activation is valid"),
        PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs a pool"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|_| panic!("FIFO initialization is pure"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .expect("initialization creates one worker");
    let committed = pool
        .on(created_worker(creation))
        .unwrap_or_else(|_| panic!("worker creation commits"));
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("created worker awaits initialization");
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|_| panic!("worker initialization commits"));
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("initialized worker begins activation");
    let started = pool
        .on(activation.started())
        .unwrap_or_else(|_| panic!("worker activation starts"));
    assert!(started.sends.worker_assignments.is_empty());
    let mut future = core::pin::pin!(activation.activate());
    let mut context = Context::from_waker(Waker::noop());
    let ready = match future.as_mut().poll(&mut context) {
        Poll::Ready(ready) => ready,
        Poll::Pending => panic!("immediate activation cannot remain pending"),
    };
    let activated = pool
        .on(ready)
        .unwrap_or_else(|_| panic!("worker activation commits"));
    assert!(activated.sends.worker_assignments.is_empty());
    (pool, worker)
}

struct AcceptedWork {
    job: u64,
    assignment: AssignWorker<SearchWorker, u8>,
}

fn submit_work(pool: &mut SearchPool) -> AcceptedWork {
    let customer = Recipient::<MessageProtocol<RuntimeAddress, FifoOutcome<Role, u8, u16>>>::global(
        RuntimeAddress(88),
    );
    let acted = pool
        .receive(
            RuntimeAddress(7),
            FifoCommand::submit(SubmissionId::new(41), 37, customer),
        )
        .unwrap_or_else(|_| panic!("ready worker accepts one job"));
    let outcome = match acted
        .sends
        .customer_outcomes
        .into_deliveries()
        .pop()
        .expect("accepted work emits one admission outcome")
    {
        ReplyDelivery::Logical(delivery) => delivery.message,
        ReplyDelivery::Established(_) => panic!("logical customer route remains logical"),
    };
    let (_, job) = outcome
        .into_accepted()
        .unwrap_or_else(|_| panic!("ready admission is accepted"));
    let assignment = acted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .expect("ready worker receives the accepted job");
    AcceptedWork {
        job: job.get(),
        assignment,
    }
}

enum DeliveryCustody {
    Emitted(AssignWorker<SearchWorker, u8>),
    Delivered(Assignment<u8>),
    CompletionBeforeDelivery {
        receipt: AssignmentReceipt,
        replay: AssignmentReceipt,
    },
    Settled,
}

#[derive(Clone, Copy)]
enum FuzzInput {
    AcceptDelivery,
    Complete,
    RejectDelivery,
    ReplayReceipt,
    ForeignReceipt,
    ForeignCompletion,
    WorkerStopped,
    ForeignWorkerStopped,
    Shutdown,
    ShutdownAccepted,
    ShutdownRejected,
    ReplayShutdown,
}

impl FuzzInput {
    const fn from_byte(byte: u8) -> Self {
        match byte % 12 {
            0 => Self::AcceptDelivery,
            1 => Self::Complete,
            2 => Self::RejectDelivery,
            3 => Self::ReplayReceipt,
            4 => Self::ForeignReceipt,
            5 => Self::ForeignCompletion,
            6 => Self::WorkerStopped,
            7 => Self::ForeignWorkerStopped,
            8 => Self::Shutdown,
            9 => Self::ShutdownAccepted,
            10 => Self::ShutdownRejected,
            _ => Self::ReplayShutdown,
        }
    }
}

#[derive(Clone, Copy)]
enum CustomerEmission {
    MustWait,
    MayFinish,
}

#[derive(Clone, Copy)]
enum PoolProgress {
    WorkerPresent,
    WorkerStopped,
    PoolStopped,
}

struct Scenario {
    pool: SearchPool,
    worker: CreationId,
    foreign_worker: CreationId,
    job: u64,
    delivery: DeliveryCustody,
    stale_receipts: VecDeque<AssignmentReceipt>,
    foreign_receipts: VecDeque<AssignmentReceipt>,
    foreign_completion: Option<Completion<u16>>,
    pending_shutdowns: VecDeque<ShutdownId>,
    stale_shutdowns: VecDeque<ShutdownId>,
    terminal_jobs: BTreeSet<u64>,
    progress: PoolProgress,
}

impl Scenario {
    fn apply(self, input: FuzzInput) -> Self {
        let Self {
            mut pool,
            worker,
            foreign_worker,
            job,
            delivery,
            mut stale_receipts,
            mut foreign_receipts,
            mut foreign_completion,
            mut pending_shutdowns,
            mut stale_shutdowns,
            mut terminal_jobs,
            progress,
        } = self;
        let (delivery, selected) = match (delivery, input) {
            (DeliveryCustody::Emitted(action), FuzzInput::AcceptDelivery) => {
                let receipt = action.receipt();
                stale_receipts.push_back(action.receipt());
                let (_, assignment, _) = action.into_parts();
                let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                    SettledItem::Attempted(ItemSettlement::Accepted(receipt));
                let expected = match progress {
                    PoolProgress::WorkerPresent => CustomerEmission::MustWait,
                    PoolProgress::WorkerStopped | PoolProgress::PoolStopped => {
                        CustomerEmission::MayFinish
                    }
                };
                (
                    DeliveryCustody::Delivered(assignment),
                    Some((
                        expected,
                        pool.transition(FifoEvent::AssignmentSettled(accepted)),
                    )),
                )
            }
            (DeliveryCustody::Emitted(action), FuzzInput::Complete) => {
                let receipt = action.receipt();
                let replay = action.receipt();
                let (_, assignment, _) = action.into_parts();
                (
                    DeliveryCustody::CompletionBeforeDelivery { receipt, replay },
                    Some((
                        CustomerEmission::MustWait,
                        pool.on(ChildReport::new(
                            worker,
                            assignment.complete(137).into_inner(),
                        )),
                    )),
                )
            }
            (DeliveryCustody::Emitted(action), FuzzInput::RejectDelivery) => {
                let rejected: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                    SettledItem::Attempted(ItemSettlement::Rejected {
                        item: action,
                        reason: ExactDeliveryReason::ClosedRecipient,
                    });
                let expected = match progress {
                    PoolProgress::WorkerPresent => CustomerEmission::MustWait,
                    PoolProgress::WorkerStopped | PoolProgress::PoolStopped => {
                        CustomerEmission::MayFinish
                    }
                };
                (
                    DeliveryCustody::Settled,
                    Some((
                        expected,
                        pool.transition(FifoEvent::AssignmentSettled(rejected)),
                    )),
                )
            }
            (DeliveryCustody::Delivered(assignment), FuzzInput::Complete) => (
                DeliveryCustody::Settled,
                Some((
                    CustomerEmission::MayFinish,
                    pool.on(ChildReport::new(
                        worker,
                        assignment.complete(137).into_inner(),
                    )),
                )),
            ),
            (
                DeliveryCustody::CompletionBeforeDelivery { receipt, replay },
                FuzzInput::AcceptDelivery,
            ) => {
                stale_receipts.push_back(replay);
                let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                    SettledItem::Attempted(ItemSettlement::Accepted(receipt));
                (
                    DeliveryCustody::Settled,
                    Some((
                        CustomerEmission::MayFinish,
                        pool.transition(FifoEvent::AssignmentSettled(accepted)),
                    )),
                )
            }
            (delivery, FuzzInput::ReplayReceipt) => match stale_receipts.pop_front() {
                Some(receipt) => {
                    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
                    (
                        delivery,
                        Some((
                            CustomerEmission::MustWait,
                            pool.transition(FifoEvent::AssignmentSettled(accepted)),
                        )),
                    )
                }
                None => (delivery, None),
            },
            (delivery, FuzzInput::ForeignReceipt) => match foreign_receipts.pop_front() {
                Some(receipt) => {
                    let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                        SettledItem::Attempted(ItemSettlement::Accepted(receipt));
                    (
                        delivery,
                        Some((
                            CustomerEmission::MustWait,
                            pool.transition(FifoEvent::AssignmentSettled(accepted)),
                        )),
                    )
                }
                None => (delivery, None),
            },
            (delivery, FuzzInput::ForeignCompletion) => match foreign_completion.take() {
                Some(completion) => (
                    delivery,
                    Some((
                        CustomerEmission::MustWait,
                        pool.on(ChildReport::new(worker, completion)),
                    )),
                ),
                None => (delivery, None),
            },
            (delivery, FuzzInput::WorkerStopped) => (
                delivery,
                Some((
                    CustomerEmission::MayFinish,
                    pool.on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now())),
                )),
            ),
            (delivery, FuzzInput::ForeignWorkerStopped) => (
                delivery,
                Some((
                    CustomerEmission::MustWait,
                    pool.on(ChildStopped::new(
                        foreign_worker,
                        Ok(Exit::Normal),
                        Instant::now(),
                    )),
                )),
            ),
            (delivery, FuzzInput::Shutdown) => (
                delivery,
                Some((
                    CustomerEmission::MayFinish,
                    pool.receive(RuntimeAddress(7), FifoCommand::shutdown()),
                )),
            ),
            (delivery, FuzzInput::ShutdownAccepted) => match pending_shutdowns.pop_front() {
                Some(shutdown) => {
                    stale_shutdowns.push_back(shutdown);
                    (
                        delivery,
                        Some((
                            CustomerEmission::MayFinish,
                            pool.on(EstablishedShutdownResolved::<SearchWorker>::accepted(
                                shutdown,
                            )),
                        )),
                    )
                }
                None => (delivery, None),
            },
            (delivery, FuzzInput::ShutdownRejected) => match pending_shutdowns.pop_front() {
                Some(shutdown) => {
                    stale_shutdowns.push_back(shutdown);
                    (
                        delivery,
                        Some((
                            CustomerEmission::MayFinish,
                            pool.on(EstablishedShutdownResolved::<SearchWorker>::rejected(
                                shutdown,
                                ShutdownRejection::AlreadyStopping,
                            )),
                        )),
                    )
                }
                None => (delivery, None),
            },
            (delivery, FuzzInput::ReplayShutdown) => match stale_shutdowns.pop_front() {
                Some(shutdown) => (
                    delivery,
                    Some((
                        CustomerEmission::MustWait,
                        pool.on(EstablishedShutdownResolved::<SearchWorker>::accepted(
                            shutdown,
                        )),
                    )),
                ),
                None => (delivery, None),
            },
            (delivery, _) => (delivery, None),
        };
        let Some((expected, acted)) = selected else {
            return Self {
                pool,
                worker,
                foreign_worker,
                job,
                delivery,
                stale_receipts,
                foreign_receipts,
                foreign_completion,
                pending_shutdowns,
                stale_shutdowns,
                terminal_jobs,
                progress,
            };
        };
        let acted = acted.unwrap_or_else(|_| panic!("every selected FIFO input is total"));
        assert!(acted.creates.is_empty());
        assert!(acted.sends.worker_observations.is_empty());
        assert!(acted.sends.worker_initializations.is_empty());
        assert!(acted.sends.worker_activations.is_empty());
        assert!(acted.sends.worker_assignments.is_empty());
        assert!(acted.sends.worker_preparations.is_empty());
        assert!(acted.sends.restart_schedules.is_empty());
        assert!(acted.sends.diagnostics.len() <= 1);

        let outcomes = acted.sends.customer_outcomes.into_deliveries();
        match expected {
            CustomerEmission::MustWait => assert!(outcomes.is_empty()),
            CustomerEmission::MayFinish => assert!(outcomes.len() <= 1),
        }
        for outcome in outcomes {
            let outcome = match outcome {
                ReplyDelivery::Logical(delivery) => {
                    assert_eq!(delivery.to.address(), RuntimeAddress(88));
                    delivery.message
                }
                ReplyDelivery::Established(_) => panic!("logical customer route changed"),
            };
            let terminal = match outcome.kind() {
                FifoOutcomeKind::Completed => {
                    let (completed, result) = outcome
                        .into_completed()
                        .unwrap_or_else(|_| panic!("completion retains its result"));
                    assert_eq!(result, 137);
                    completed.get()
                }
                FifoOutcomeKind::ReturnedQueued => {
                    let (returned, payload, reason) = outcome
                        .into_returned_queued()
                        .unwrap_or_else(|_| panic!("queued return retains its payload"));
                    assert_eq!(payload, 37);
                    assert!(matches!(
                        reason,
                        QueuedReturnReason::NoRecoverableWorkers | QueuedReturnReason::PoolShutdown
                    ));
                    returned.get()
                }
                FifoOutcomeKind::ReturnedAssigned => {
                    let (returned, payload, reason) = outcome
                        .into_returned_assigned()
                        .unwrap_or_else(|_| panic!("assigned return retains its payload"));
                    assert_eq!(payload, 37);
                    assert!(matches!(
                        reason,
                        AssignedReturnReason::WorkerStopped | AssignedReturnReason::PoolShutdown
                    ));
                    returned.get()
                }
                FifoOutcomeKind::Accepted | FifoOutcomeKind::Rejected => {
                    panic!("no later admission command was sent")
                }
            };
            assert_eq!(terminal, job);
            assert!(terminal_jobs.insert(terminal));
        }
        for shutdown in acted.sends.worker_shutdowns.into_requests() {
            pending_shutdowns.push_back(shutdown.id);
        }
        assert!(terminal_jobs.len() <= 1);
        let progress = match (progress, input, acted.become_) {
            (_, _, Step::Stop(_)) => PoolProgress::PoolStopped,
            (PoolProgress::WorkerPresent, FuzzInput::WorkerStopped, Step::Continue) => {
                PoolProgress::WorkerStopped
            }
            (PoolProgress::WorkerPresent, _, Step::Continue) => PoolProgress::WorkerPresent,
            (PoolProgress::WorkerStopped, _, Step::Continue) => PoolProgress::WorkerStopped,
            (PoolProgress::PoolStopped, _, Step::Continue) => panic!("stopped FIFO pool reopened"),
        };
        Self {
            pool,
            worker,
            foreign_worker,
            job,
            delivery,
            stale_receipts,
            foreign_receipts,
            foreign_completion,
            pending_shutdowns,
            stale_shutdowns,
            terminal_jobs,
            progress,
        }
    }
}

fn exercise(bytes: &[u8]) {
    let (mut pool, worker) = ready_pool();
    let accepted = submit_work(&mut pool);
    let (mut foreign_pool, _) = ready_pool();
    let foreign = submit_work(&mut foreign_pool);
    let mut foreign_receipts = VecDeque::new();
    for _ in 0..4 {
        foreign_receipts.push_back(foreign.assignment.receipt());
    }
    let (_, foreign_assignment, _) = foreign.assignment.into_parts();
    let foreign_completion = Some(foreign_assignment.complete(999).into_inner());
    let mut sequence = CreationSequence::new();
    let _occupied = sequence.issue().expect("one creation ID exists");
    let foreign_worker = sequence.issue().expect("a second creation ID exists");
    assert_ne!(foreign_worker, worker);
    let mut scenario = Scenario {
        pool,
        worker,
        foreign_worker,
        job: accepted.job,
        delivery: DeliveryCustody::Emitted(accepted.assignment),
        stale_receipts: VecDeque::new(),
        foreign_receipts,
        foreign_completion,
        pending_shutdowns: VecDeque::new(),
        stale_shutdowns: VecDeque::new(),
        terminal_jobs: BTreeSet::new(),
        progress: PoolProgress::WorkerPresent,
    };
    for byte in bytes.iter().copied().take(256) {
        scenario = scenario.apply(FuzzInput::from_byte(byte));
    }
}

fuzz_target!(|bytes: &[u8]| {
    exercise(bytes);
});
