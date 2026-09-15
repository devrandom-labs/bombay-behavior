use std::convert::Infallible;
use std::hint::black_box;
use std::time::{Duration, Instant};

use behavior::{
    Actions, Address, ChildCreationOutcome, ChildHead, ChildReport, CreationSettlement,
    CreationsSettled, EndpointAddress, EstablishedCreation, EstablishedRecipient, ItemSettlement,
    MessageProtocol, Never, Protocol, Recipient, SettledItem, Step,
};
use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, Assignment, BacklogCapacity, DiagnosticDisposition,
    FifoCommand, FifoEvent, FifoOutcome, ImmediateActivation, Interruption, OrderedRoles,
    PoolFailureReaction, PoolRecovery, SubmissionId, WorkerSubmission, fifo, pool_worker,
};
use behavior_actors::{Activate, StopOnShutdown};

const DEFAULT_ITERATIONS: usize = 100_000;

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
enum SearchRole {
    Primary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchJob(u64);

#[derive(Debug, Eq, PartialEq)]
struct SearchResult(u64);

struct SearchWorker;

#[pool_worker(addr = RuntimeAddr, result = SearchResult)]
impl SearchWorker {
    fn process(&mut self, job: &SearchJob) -> SearchResult {
        SearchResult(job.0.wrapping_add(100))
    }

    fn transition(&mut self, assignment: Assignment<SearchJob>) -> WorkerActed<Self> {
        let result = self.process(assignment.payload());
        Ok(Actions::cont().with_send(assignment.complete(result)))
    }
}

fn prepare_worker(
    _: &SearchRole,
) -> Result<WorkerSubmission<SearchWorker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(SearchWorker))
}

fn iterations() -> usize {
    std::env::var("BOMBAY_FIFO_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap_or_else(|error| panic!("benchmark runtime construction failed: {error}"));
    runtime.block_on(measure());
}

async fn measure() {
    let roles = OrderedRoles::new(SearchRole::Primary, core::iter::empty())
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = fifo(
        prepare_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(0),
        Interruption::Fail,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .unwrap_or_else(|_| panic!("the worker declaration constructs a FIFO pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|error| panic!("FIFO initialization failed: {error}"));
    let mut pool = initialized.behavior;
    let creation = initialized
        .actions
        .creates
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("one worker creation is required"));
    let (worker, _, kind) = creation.into_parts();
    let created = CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::<StopOnShutdown<SearchWorker>, ChildHead>::Established {
                established: EstablishedCreation::installed(
                    worker.clone(),
                    kind,
                    EstablishedRecipient::issued(Endpoint(41)),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ));
    let created = pool
        .on(created)
        .unwrap_or_else(|error| panic!("worker creation failed: {error}"));
    let initialization = created
        .sends
        .worker_initializations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("created worker requires initialization"));
    let initialized = pool
        .on(initialization
            .resolve(behavior_actors::atomic::WorkerInitializationOutcome::ReadyForActivation))
        .unwrap_or_else(|error| panic!("worker initialization failed: {error}"));
    let activation = initialized
        .sends
        .worker_activations
        .into_requests()
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("initialized worker requires activation"));
    let started = activation.started();
    let ready = activation.activate().await;
    let admitted = pool
        .on(started)
        .unwrap_or_else(|error| panic!("activation admission failed: {error}"));
    if !admitted.sends.worker_assignments.is_empty()
        || !admitted.sends.customer_outcomes.as_slice().is_empty()
        || !admitted.creates.is_empty()
        || !matches!(admitted.become_, Step::Continue)
    {
        panic!("activation admission must not emit application work");
    }
    let ready = pool
        .on(ready)
        .unwrap_or_else(|error| panic!("worker readiness failed: {error}"));
    if !ready.sends.worker_assignments.is_empty()
        || !ready.sends.customer_outcomes.as_slice().is_empty()
        || !ready.creates.is_empty()
        || !matches!(ready.become_, Step::Continue)
    {
        panic!("worker readiness must not emit application work");
    }

    let customer = Recipient::<
        MessageProtocol<RuntimeAddr, FifoOutcome<SearchRole, SearchJob, SearchResult>>,
    >::global(RuntimeAddr(88));
    let iterations = iterations();
    let started = Instant::now();

    for index in 0..iterations {
        let ordinal = u64::try_from(index)
            .unwrap_or_else(|_| panic!("benchmark iteration fits the submission identifier"));
        let submitted = pool
            .receive(
                RuntimeAddr(7),
                FifoCommand::submit(
                    SubmissionId::new(ordinal),
                    SearchJob(black_box(ordinal)),
                    customer,
                ),
            )
            .unwrap_or_else(|error| panic!("FIFO submission failed: {error}"));
        if submitted.sends.customer_outcomes.as_slice().len() != 1 {
            panic!("accepted submission must emit one customer receipt");
        }
        let assignment = submitted
            .sends
            .worker_assignments
            .into_items()
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("accepted submission must emit one assignment"));
        let receipt = assignment.receipt();
        let (_, assignment, _) = assignment.into_parts();
        let accepted = pool
            .transition(FifoEvent::AssignmentSettled(SettledItem::Attempted(
                ItemSettlement::Accepted(receipt),
            )))
            .unwrap_or_else(|error| panic!("assignment receipt failed: {error}"));
        if !accepted.sends.customer_outcomes.as_slice().is_empty() {
            panic!("assignment receipt cannot terminate customer work");
        }
        let result = SearchResult(assignment.payload().0.wrapping_add(100));
        let completed = pool
            .on(ChildReport::new(
                worker.clone(),
                assignment.complete(result).into_inner(),
            ))
            .unwrap_or_else(|error| panic!("worker completion failed: {error}"));
        if completed.sends.customer_outcomes.as_slice().len() != 1 {
            panic!("one completed assignment must emit one terminal outcome");
        }
        black_box((
            completed.sends.customer_outcomes.as_slice().len(),
            completed.sends.worker_assignments.len(),
            completed.sends.worker_observations.len(),
            completed.sends.worker_initializations.len(),
            completed.sends.worker_activations.len(),
            completed.sends.worker_preparations.len(),
            completed.sends.restart_schedules.len(),
            completed.sends.worker_shutdowns.len(),
            completed.sends.diagnostics.len(),
            completed.creates.len(),
            matches!(completed.become_, Step::Continue),
        ));
    }

    let elapsed = started.elapsed();
    let rate = iterations as f64 / elapsed.as_secs_f64();
    println!("METRIC fifo_complete_cycles_per_s={rate:.0}");
    println!("METRIC fifo_iterations={iterations}");
    println!("METRIC fifo_elapsed_ns={}", elapsed.as_nanos());
    let end_delay = std::env::var("BOMBAY_FIFO_BENCH_END_DELAY_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if end_delay != 0 {
        std::thread::sleep(Duration::from_millis(end_delay));
    }
}
