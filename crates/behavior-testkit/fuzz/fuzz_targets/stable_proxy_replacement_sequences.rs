#![no_main]
//! Arbitrary overlap, correlation, and cancellation during proxy replacement.
mod stable_proxy;
mod worker_return;
use behavior::atomic::{ProxyControl, ProxyOutcome, ProxyPhase, ReplacementOutcome, StableProxy};
use behavior::{EstablishedShutdownResolved, ShutdownId, Step};
use libfuzzer_sys::fuzz_target;
use stable_proxy::{Worker, drive_ready_proxy, worker_stopped};
use worker_return::{WorkerReturn, WorkerReturnDecision, WorkerReturnInput, foreign_worker};

#[derive(Clone, Copy)]
enum ReplacementInput {
    WorkerReturn(WorkerReturnInput),
    Replace(u8),
    Shutdown,
}

impl ReplacementInput {
    const fn from_byte(byte: u8) -> Self {
        match byte % 6 {
            0 => Self::WorkerReturn(WorkerReturnInput::ExactWorkerStopped),
            1 => Self::WorkerReturn(WorkerReturnInput::ExactShutdownReturned),
            2 => Self::WorkerReturn(WorkerReturnInput::ForeignWorkerStopped),
            3 => Self::WorkerReturn(WorkerReturnInput::ForeignShutdownReturned),
            4 => Self::Replace(byte),
            _ => Self::Shutdown,
        }
    }
}

#[derive(Clone, Copy)]
enum PredecessorReceipt {
    Awaiting,
    Returned,
}

impl PredecessorReceipt {
    const fn cancel(self) -> WorkerReturn {
        match self {
            Self::Awaiting => WorkerReturn::AwaitingStopAndReceipt,
            Self::Returned => WorkerReturn::AwaitingStop,
        }
    }
}

#[derive(Clone, Copy)]
enum ReplacementState {
    Returning(PredecessorReceipt),
    Cancelling(WorkerReturn),
    SuccessorCreated,
    Stopped,
}

enum ReplacementDecision {
    Waiting,
    InputRejected,
    OverlapReturned(u8),
    SuccessorCancelled,
    SuccessorCreated,
    ShutdownRepeated,
    Stopped,
}

impl ReplacementState {
    fn accept(self, input: ReplacementInput) -> (Self, ReplacementDecision) {
        match (self, input) {
            (Self::SuccessorCreated, _) => (
                Self::SuccessorCreated,
                ReplacementDecision::SuccessorCreated,
            ),
            (Self::Stopped, _) => (Self::Stopped, ReplacementDecision::Stopped),
            (
                Self::Returning(_),
                ReplacementInput::WorkerReturn(WorkerReturnInput::ExactWorkerStopped),
            ) => (
                Self::SuccessorCreated,
                ReplacementDecision::SuccessorCreated,
            ),
            (
                Self::Returning(PredecessorReceipt::Awaiting),
                ReplacementInput::WorkerReturn(WorkerReturnInput::ExactShutdownReturned),
            ) => (
                Self::Returning(PredecessorReceipt::Returned),
                ReplacementDecision::Waiting,
            ),
            (Self::Returning(receipt), ReplacementInput::WorkerReturn(_)) => {
                (Self::Returning(receipt), ReplacementDecision::InputRejected)
            }
            (Self::Returning(receipt), ReplacementInput::Replace(worker)) => (
                Self::Returning(receipt),
                ReplacementDecision::OverlapReturned(worker),
            ),
            (Self::Returning(receipt), ReplacementInput::Shutdown) => (
                Self::Cancelling(receipt.cancel()),
                ReplacementDecision::SuccessorCancelled,
            ),
            (Self::Cancelling(worker_return), ReplacementInput::WorkerReturn(input)) => {
                let (worker_return, decision) = worker_return.accept(input);
                match decision {
                    WorkerReturnDecision::Waiting => (
                        Self::Cancelling(worker_return),
                        ReplacementDecision::Waiting,
                    ),
                    WorkerReturnDecision::InputRejected => (
                        Self::Cancelling(worker_return),
                        ReplacementDecision::InputRejected,
                    ),
                    WorkerReturnDecision::Returned => (Self::Stopped, ReplacementDecision::Stopped),
                }
            }
            (Self::Cancelling(worker_return), ReplacementInput::Replace(worker)) => (
                Self::Cancelling(worker_return),
                ReplacementDecision::OverlapReturned(worker),
            ),
            (Self::Cancelling(worker_return), ReplacementInput::Shutdown) => (
                Self::Cancelling(worker_return),
                ReplacementDecision::ShutdownRepeated,
            ),
        }
    }

    const fn phase(self) -> ProxyPhase {
        match self {
            Self::Returning(_) => ProxyPhase::Replacing,
            Self::Cancelling(_) => ProxyPhase::ShuttingDown,
            Self::SuccessorCreated => ProxyPhase::Creating,
            Self::Stopped => ProxyPhase::Stopped,
        }
    }
}

fn exercise(inputs: &[u8]) {
    let (mut proxy, predecessor, _outcome) = drive_ready_proxy(
        StableProxy::immediate(),
        ProxyControl::start(Worker::new(0)),
    );
    let foreign_worker = foreign_worker(predecessor);
    let replacing = proxy
        .on(ProxyControl::replace(Worker::new(1)))
        .expect("replacement begins exact predecessor return");
    let shutdown = replacing
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("replacement emits one predecessor shutdown")
        .id;
    let foreign_shutdown = ShutdownId(foreign_worker.get());
    assert_ne!(foreign_shutdown, shutdown);
    let mut state = ReplacementState::Returning(PredecessorReceipt::Awaiting);

    for byte in inputs {
        if matches!(
            state,
            ReplacementState::SuccessorCreated | ReplacementState::Stopped
        ) {
            break;
        }
        let input = ReplacementInput::from_byte(*byte);
        let (next, decision) = state.accept(input);
        state = next;
        let actions = match input {
            ReplacementInput::WorkerReturn(WorkerReturnInput::ExactWorkerStopped) => {
                proxy.on(worker_stopped(predecessor))
            }
            ReplacementInput::WorkerReturn(WorkerReturnInput::ExactShutdownReturned) => {
                proxy.on(EstablishedShutdownResolved::<Worker>::accepted(shutdown))
            }
            ReplacementInput::WorkerReturn(WorkerReturnInput::ForeignWorkerStopped) => {
                proxy.on(worker_stopped(foreign_worker))
            }
            ReplacementInput::WorkerReturn(WorkerReturnInput::ForeignShutdownReturned) => proxy.on(
                EstablishedShutdownResolved::<Worker>::accepted(foreign_shutdown),
            ),
            ReplacementInput::Replace(worker) => {
                proxy.on(ProxyControl::replace(Worker::new(worker)))
            }
            ReplacementInput::Shutdown => proxy.on(ProxyControl::shutdown()),
        }
        .expect("every modeled replacement input is total");

        assert_eq!(proxy.phase(), state.phase());
        let observations = match &decision {
            ReplacementDecision::SuccessorCreated => 1,
            _ => 0,
        };
        assert_eq!(actions.sends.worker_observations.len(), observations);
        assert!(actions.sends.worker_initializations.is_empty());
        assert!(actions.sends.worker_activations.is_empty());
        assert!(actions.sends.worker_shutdowns.is_empty());
        assert!(actions.sends.worker_deliveries.is_empty());
        match decision {
            ReplacementDecision::Waiting | ReplacementDecision::ShutdownRepeated => {
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.creates.is_empty());
                assert!(actions.sends.owner_outcomes.is_empty());
                assert!(actions.sends.diagnostics.is_empty());
            }
            ReplacementDecision::InputRejected => {
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.creates.is_empty());
                assert!(actions.sends.owner_outcomes.is_empty());
                assert_eq!(actions.sends.diagnostics.len(), 1);
            }
            ReplacementDecision::OverlapReturned(worker) => {
                assert!(actions.creates.is_empty());
                assert!(actions.sends.diagnostics.is_empty());
                let outcome = actions
                    .sends
                    .owner_outcomes
                    .into_requests()
                    .pop()
                    .expect("overlap returns one complete successor")
                    .into_inner();
                match outcome {
                    ProxyOutcome::Replacement {
                        outcome: ReplacementOutcome::NotReplaceable { worker: actual, .. },
                    } => assert_eq!(actual, Worker::new(worker)),
                    _ => panic!("overlap changed its domain outcome"),
                }
            }
            ReplacementDecision::SuccessorCancelled => {
                assert!(actions.creates.is_empty());
                assert!(actions.sends.diagnostics.is_empty());
                let outcome = actions
                    .sends
                    .owner_outcomes
                    .into_requests()
                    .pop()
                    .expect("shutdown returns one retained successor")
                    .into_inner();
                match outcome {
                    ProxyOutcome::Replacement {
                        outcome: ReplacementOutcome::CancelledBeforeBirth { worker, .. },
                    } => assert_eq!(worker, Worker::new(1)),
                    _ => panic!("shutdown changed successor cancellation"),
                }
            }
            ReplacementDecision::SuccessorCreated => {
                assert!(matches!(actions.become_, Step::Continue));
                assert_eq!(actions.creates.len(), 1);
                assert_eq!(actions.sends.owner_outcomes.len(), 1);
                assert!(actions.sends.diagnostics.is_empty());
            }
            ReplacementDecision::Stopped => {
                assert!(matches!(actions.become_, Step::Stop(_)));
                assert!(actions.creates.is_empty());
                assert!(actions.sends.owner_outcomes.is_empty());
                assert!(actions.sends.diagnostics.is_empty());
            }
        }
    }
}

fuzz_target!(|inputs: &[u8]| {
    exercise(inputs);
});
