//! Durable lifecycle messages owned by one configured actor.

use behavior::{
    Address, Behavior, BehaviorAddr, ChildInputReason, EndpointAddress, EstablishedActor,
    InterpreterFault, Protocol,
};

use crate::{
    ActivationPlan, ChildStopped, InitialWorkerOutcome, ProxyOutcome, ProxyPhase, StableProxy,
    WorkerAttempt, WorkerCreationRejection, WorkerSubmission,
};

use crate::atomic::proxy_creation::StableProxyCreationSettlement;

/// Why one keyed service is being removed after its proxy drains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryRetirement {
    /// The accepted worker change was cancelled.
    Cancellation,
    /// The initial worker could not become ready.
    StartFailed,
    /// Policy selected retirement after the ready worker stopped.
    UnexpectedWorkerStopped,
    /// An admitted explicit stop observed the stable proxy's exact exit.
    Stop,
    /// Global supervisor shutdown drained or proved absent this entry.
    Shutdown,
}

/// Exact reason an admitted keyed-service stop could not complete normally.
pub enum EntryStopFailureReason {
    /// The child input capability rejected the shutdown request.
    ControlRejected(ChildInputReason),
    /// The interpreter violated its child-input contract.
    InterpreterCorrupt(InterpreterFault),
    /// Product traversal ended before attempting the shutdown request.
    InterpretationSkipped,
}

/// Typed failure of one admitted explicit keyed-service stop.
///
/// The settlement reason and optional exact proxy stop are independent current
/// values. A missing stop permits restoration; a present stop requires entry
/// retirement.
pub struct EntryStopFailure<A>
where
    A: Address,
{
    pub(super) reason: EntryStopFailureReason,
    pub(super) stopped: Option<ChildStopped<A>>,
}

impl<A> EntryStopFailure<A>
where
    A: Address,
{
    /// Borrow the exact shutdown-settlement reason.
    #[must_use]
    pub const fn reason(&self) -> &EntryStopFailureReason {
        &self.reason
    }

    /// Borrow the exact proxy stop when it arrived before settlement failed.
    #[must_use]
    pub const fn stopped(&self) -> Option<&ChildStopped<A>> {
        self.stopped.as_ref()
    }

    /// Transfer the complete independent reason and proxy-stop values.
    #[must_use]
    pub fn into_parts(self) -> (EntryStopFailureReason, Option<ChildStopped<A>>) {
        (self.reason, self.stopped)
    }
}

/// Management operation whose cancellation is durably completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerChange {
    /// Creation of the entry's first worker.
    Start,
    /// Replacement of an existing or unavailable worker.
    Replacement,
}

/// Why an accepted worker change cannot publish availability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerChangeInterruption {
    /// An admitted explicit Stop interrupted an initial start.
    ExplicitStop {
        /// Exact stop operation, distinct from the interrupted start operation.
        operation: u64,
    },
    /// Supervisor shutdown interrupted the named worker-change kind.
    SupervisorShutdown { change: WorkerChange },
}

/// Complete worker-custody result of one cancelled management operation.
pub enum CancellationOutcome<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// The complete worker submission returned through the cancellation reply.
    WorkerReturned,
    /// The emitted proxy input was rejected and moved to diagnostic custody.
    ProxyInputRejected,
    /// The proxy produced a late result retained by the lifecycle owner.
    ProxyReported { outcome: ProxyOutcome<Worker, Plan> },
}

/// Complete worker disposition when a start is interrupted before availability.
pub enum InterruptedWorker<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// The worker never left the supervisor's local submission.
    Submission(WorkerSubmission<Worker, Plan>),
    /// The emitted proxy input was rejected and moved to diagnostic custody.
    ProxyInputRejected,
    /// The proxy returned the exact result of the interrupted input.
    ProxyReported(ProxyOutcome<Worker, Plan>),
}

/// Exact replacement failure values not retained by the next service state.
pub enum ReplacementFailure<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// StableProxy refused the input without changing its current worker.
    ProxyRefused {
        worker: Worker,
        activation: Plan,
        phase: ProxyPhase,
    },
    /// StableProxy returned the worker after exhausting fresh attempt IDs.
    WorkerAttemptsExhausted { worker: Worker, activation: Plan },
    /// Successor creation failed before a worker was committed.
    WorkerCreationRejected {
        rejection: WorkerCreationRejection<Worker>,
        activation: Plan,
        stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
    },
    /// A committed successor could not become available and was drained.
    WorkerUnavailable {
        drain: crate::ProxyDrain<Worker, Plan>,
    },
}

/// Durable dynamic-supervisor lifecycle message.
pub enum DynamicLifecycle<Key, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Started {
        key: Key,
        generation: u64,
        operation: u64,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    },
    StartCreationRejected {
        key: Key,
        generation: u64,
        submission: WorkerSubmission<Worker, Plan>,
        creation: StableProxyCreationSettlement<Worker, Plan>,
    },
    WorkerChangeInterrupted {
        key: Key,
        generation: u64,
        operation: u64,
        interruption: WorkerChangeInterruption,
        worker: InterruptedWorker<Worker, Plan>,
    },
    StartInputRejected {
        key: Key,
        generation: u64,
    },
    ReplacementInputRejected {
        key: Key,
        generation: u64,
        operation: u64,
    },
    StartOutcomeRejected {
        key: Key,
        generation: u64,
        operation: u64,
        outcome: InitialWorkerOutcome<Worker, Plan>,
    },
    Replaced {
        key: Key,
        generation: u64,
        operation: u64,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    },
    ReplacementFailed {
        key: Key,
        generation: u64,
        operation: u64,
        failure: ReplacementFailure<Worker, Plan>,
    },
    StopFinished {
        key: Key,
        generation: u64,
        operation: u64,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        result: Result<ChildStopped<BehaviorAddr<Worker>>, EntryStopFailure<BehaviorAddr<Worker>>>,
    },
    UnexpectedWorkerStopped {
        key: Key,
        generation: u64,
        worker: WorkerAttempt,
        readiness: Plan::Ready,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
        disposition: super::UnexpectedExit,
    },
    CommandUnavailable {
        key: Key,
        generation: u64,
        sender: BehaviorAddr<Worker>,
        proxy_phase: ProxyPhase,
        command: <Worker::Protocol as Protocol>::Msg,
    },
    OperationCancelled {
        key: Key,
        generation: u64,
        operation: u64,
        change: WorkerChange,
        outcome: CancellationOutcome<Worker, Plan>,
    },
    EntryRetired {
        key: Key,
        generation: u64,
        cause: EntryRetirement,
    },
}
