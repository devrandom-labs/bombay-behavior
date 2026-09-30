use core::convert::Infallible;
use std::time::Instant;

use behavior::{
    ActionItemResult, Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase,
    ChildCreationOutcome, ChildHead, ChildReport, CreateChild, CreationId, CreationSettlement,
    CreationsSettled, EndpointAddress, EstablishedCreation, EstablishedDelivery,
    EstablishedRecipient, ExactDeliveryReason, Here, InterpretEstablished, InterpretItem,
    InterpreterRequests, ItemSettlement, MessageProtocol, Never, NoBirths, Protocol, Recipient,
    ReportToParent, SettledItem, User,
};
use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, AssignWorker, Assignment, BacklogCapacity, Completion,
    DiagnosticDisposition, FifoCommand, FifoEvent, FifoOutcome, FifoOutcomeKind, FifoPool,
    ImmediateActivation, Interruption, OrderedRoles, PoolFailureReaction, PoolRecovery,
    QueuedReturnReason, SubmissionId, WorkerInitializationOutcome, WorkerSubmission, fifo,
};
use behavior_actors::{
    Activate, Active, ChildStopped, EstablishedShutdownResolved, Exit, ReplyDelivery,
    StopOnShutdown,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Worker,
}

#[derive(Debug, Eq, PartialEq)]
struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = Assignment<Box<str>>;
}

impl BehaviorBase for Worker {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Assignment<Box<str>>>;
    type Sends = InterpreterRequests<ReportToParent<Completion<u16>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont().with_send(input.message.complete(23)))
    }
}

type Pool = FifoPool<Role, Worker, ImmediateActivation, Never, Infallible, Box<str>, u16>;
type Customer = MessageProtocol<RuntimeAddr, FifoOutcome<Role, Box<str>, u16>>;

fn prepare_worker(_: &Role) -> Result<WorkerSubmission<Worker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(Worker))
}

fn committed_worker(
    creation: CreateChild<RuntimeAddr, StopOnShutdown<Worker>>,
) -> CreationsSettled<RuntimeAddr, StopOnShutdown<Worker>> {
    let (id, _, kind) = creation.into_parts();
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::<StopOnShutdown<Worker>, ChildHead>::Established {
                established: EstablishedCreation::installed(
                    id,
                    kind,
                    EstablishedRecipient::issued(Endpoint(40 + id.get())),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

async fn ready_pool() -> (Active<Pool>, CreationId) {
    let roles = OrderedRoles::new(Role::Worker, []).expect("one worker role");
    let pool: Pool = fifo(
        prepare_worker,
        roles,
        ActivationPolicy::new(1).expect("one worker may activate"),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the declared worker constructs the pool"));
    let initialized = pool.initialize().expect("pool initialization");
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .expect("one worker creation");
    let committed = pool.on(committed_worker(creation)).expect("worker birth");
    let initialization = committed
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("worker initialization request");
    let worker = initialization.worker().creation();
    let initialized_worker = pool
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("worker initialization settlement");
    let activation = initialized_worker
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("worker activation request");
    let started = pool.on(activation.started()).expect("activation starts");
    assert!(started.sends.worker_assignments.is_empty());
    let ready = pool.on(activation.activate().await).expect("worker ready");
    assert!(ready.sends.worker_assignments.is_empty());
    (pool, worker)
}

fn submitted_assignment(
    pool: &mut Active<Pool>,
    payload: Box<str>,
) -> AssignWorker<Worker, Box<str>> {
    let customer = Recipient::<Customer>::global(RuntimeAddr(88));
    let submitted = pool
        .receive(
            RuntimeAddr(7),
            FifoCommand::submit(SubmissionId::new(9), payload, customer),
        )
        .expect("ready pool accepts customer work");
    submitted
        .sends
        .worker_assignments
        .into_items()
        .pop()
        .expect("ready worker gets exact assignment")
}

enum DeliveryAdmission {
    Accept,
    Reject,
}

struct EndpointReader;

impl InterpretEstablished<Worker> for EndpointReader {
    type Output = Endpoint;

    fn interpret_established(&mut self, endpoint: Endpoint) -> Self::Output {
        endpoint
    }
}

struct DeliveryHost {
    admission: DeliveryAdmission,
    endpoints: Vec<Endpoint>,
    payloads: Vec<usize>,
    payload_texts: Vec<String>,
    completions: Vec<Completion<u16>>,
}

impl DeliveryHost {
    fn new(admission: DeliveryAdmission) -> Self {
        Self {
            admission,
            endpoints: Vec::new(),
            payloads: Vec::new(),
            payload_texts: Vec::new(),
            completions: Vec::new(),
        }
    }
}

impl InterpretItem<EstablishedDelivery<Worker>, (), Here> for DeliveryHost {
    async fn interpret_item(
        &mut self,
        delivery: EstablishedDelivery<Worker>,
    ) -> ItemSettlement<EstablishedDelivery<Worker>, (), ExactDeliveryReason, Never> {
        self.endpoints
            .push(delivery.to.clone().interpret(&mut EndpointReader));
        self.payloads
            .push(delivery.message.payload().as_ref().as_ptr() as usize);
        self.payload_texts
            .push(delivery.message.payload().as_ref().to_owned());
        match self.admission {
            DeliveryAdmission::Accept => {
                self.completions
                    .push(delivery.message.complete(23).into_inner());
                ItemSettlement::Accepted(())
            }
            DeliveryAdmission::Reject => ItemSettlement::Rejected {
                item: delivery,
                reason: ExactDeliveryReason::ClosedRecipient,
            },
        }
    }
}

#[tokio::test]
async fn accepted_assignment_returns_its_receipt_before_completion() {
    let (mut pool, worker) = ready_pool().await;
    let payload: Box<str> = Box::from("accepted");
    let identity = payload.as_ptr() as usize;
    let request = submitted_assignment(&mut pool, payload);
    let mut host = DeliveryHost::new(DeliveryAdmission::Accept);

    let settlement = request.settle(&mut host).await;
    assert_eq!(host.endpoints, [Endpoint(40 + worker.get())]);
    assert_eq!(host.payloads.len(), 1);
    assert_ne!(host.payloads[0], identity);
    assert_eq!(host.payload_texts, ["accepted"]);
    let accepted: ActionItemResult<AssignWorker<Worker, Box<str>>> =
        SettledItem::Attempted(settlement);
    let awaiting_completion = pool
        .transition(FifoEvent::AssignmentSettled(accepted))
        .expect("the exact receipt returns to its pool");
    assert!(
        awaiting_completion
            .sends
            .customer_outcomes
            .as_slice()
            .is_empty()
    );
    let [completion]: [Completion<u16>; 1] = host
        .completions
        .try_into()
        .unwrap_or_else(|_| panic!("one accepted delivery transfers the assignment to one worker"));
    let completed = pool
        .on(ChildReport::new(worker, completion))
        .expect("separate completion reaches the pool");
    let [outcome]: [_; 1] = completed
        .sends
        .customer_outcomes
        .into_deliveries()
        .try_into()
        .unwrap_or_else(|_| panic!("one customer outcome"));
    let ReplyDelivery::Logical(delivery) = outcome else {
        panic!("the customer route is logical");
    };
    assert_eq!(delivery.message.kind(), FifoOutcomeKind::Completed);
    assert_eq!(delivery.message.worker_result(), Some(&23));
}

#[tokio::test]
async fn concurrent_same_typed_assignments_return_receipts_to_their_own_pools() {
    let (mut first, first_worker) = ready_pool().await;
    let (mut second, second_worker) = ready_pool().await;
    assert_eq!(first_worker, second_worker);
    let first_payload: Box<str> = Box::from("first");
    let second_payload: Box<str> = Box::from("second");
    let first_identity = first_payload.as_ptr() as usize;
    let second_identity = second_payload.as_ptr() as usize;
    let first_request = submitted_assignment(&mut first, first_payload);
    let second_request = submitted_assignment(&mut second, second_payload);
    let mut host = DeliveryHost::new(DeliveryAdmission::Accept);

    let second_receipt = second_request.settle(&mut host).await;
    let first_receipt = first_request.settle(&mut host).await;
    assert_eq!(host.endpoints.len(), 2);
    assert_ne!(host.payloads[0], second_identity);
    assert_ne!(host.payloads[1], first_identity);
    assert_eq!(host.payload_texts, ["second", "first"]);
    let [second_completion, first_completion]: [_; 2] = host
        .completions
        .try_into()
        .unwrap_or_else(|_| panic!("both exact worker deliveries completed once"));

    let first_accepted = first
        .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
            first_receipt,
        )))
        .expect("first pool receives its original opaque receipt");
    let second_accepted = second
        .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
            second_receipt,
        )))
        .expect("second pool receives its original opaque receipt");
    for accepted in [first_accepted, second_accepted] {
        assert!(accepted.sends.diagnostics.is_empty());
        assert!(accepted.sends.customer_outcomes.as_slice().is_empty());
    }

    for finished in [
        first
            .on(ChildReport::new(first_worker, first_completion))
            .expect("first worker completion"),
        second
            .on(ChildReport::new(second_worker, second_completion))
            .expect("second worker completion"),
    ] {
        assert!(finished.sends.diagnostics.is_empty());
        let [outcome]: [_; 1] = finished
            .sends
            .customer_outcomes
            .into_deliveries()
            .try_into()
            .unwrap_or_else(|_| panic!("one customer completion per pool"));
        let ReplyDelivery::Logical(delivery) = outcome else {
            panic!("the customer route remains logical");
        };
        assert_eq!(delivery.message.kind(), FifoOutcomeKind::Completed);
        assert_eq!(delivery.message.worker_result(), Some(&23));
    }
}

#[tokio::test]
async fn rejected_assignment_returns_original_customer_job_after_quarantine() {
    let (mut pool, worker) = ready_pool().await;
    let payload: Box<str> = Box::from("rejected");
    let identity = payload.as_ptr() as usize;
    let request = submitted_assignment(&mut pool, payload);
    let mut host = DeliveryHost::new(DeliveryAdmission::Reject);

    let settlement = request.settle(&mut host).await;
    assert_eq!(host.endpoints, [Endpoint(40 + worker.get())]);
    assert_eq!(host.payloads.len(), 1);
    assert_ne!(host.payloads[0], identity);
    assert_eq!(host.payload_texts, ["rejected"]);
    assert!(host.completions.is_empty());
    let rejected: ActionItemResult<AssignWorker<Worker, Box<str>>> =
        SettledItem::Attempted(settlement);
    let quarantined = pool
        .transition(FifoEvent::AssignmentSettled(rejected))
        .expect("pool receives the exact rejected assignment");
    assert!(quarantined.sends.customer_outcomes.as_slice().is_empty());
    let shutdown = quarantined
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("rejected worker is quarantined");
    let stopped = pool
        .on(ChildStopped::new(worker, Ok(Exit::Normal), Instant::now()))
        .expect("quarantined worker exits");
    assert!(stopped.sends.customer_outcomes.as_slice().is_empty());
    let retired = pool
        .on(EstablishedShutdownResolved::<Worker>::accepted(shutdown.id))
        .expect("quarantine shutdown settles");
    let [outcome]: [_; 1] = retired
        .sends
        .customer_outcomes
        .into_deliveries()
        .try_into()
        .unwrap_or_else(|_| panic!("one returned job"));
    let ReplyDelivery::Logical(delivery) = outcome else {
        panic!("the customer route is logical");
    };
    let (_, returned, reason) = delivery
        .message
        .into_returned_queued()
        .unwrap_or_else(|_| panic!("the rejected exact delivery returns queued custody"));
    assert_eq!(reason, QueuedReturnReason::NoRecoverableWorkers);
    assert_eq!(returned.as_ptr() as usize, identity);
    assert_eq!(&*returned, "rejected");
}
