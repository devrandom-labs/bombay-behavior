//! Independent model of the exact worker-stop and shutdown-receipt join.

use behavior_core::{CreationId, CreationSequence};

#[derive(Clone, Copy)]
pub(crate) enum WorkerReturnInput {
    ExactWorkerStopped,
    ExactShutdownReturned,
    ForeignWorkerStopped,
    ForeignShutdownReturned,
}

#[derive(Clone, Copy)]
pub(crate) enum WorkerReturn {
    AwaitingStopAndReceipt,
    AwaitingStop,
    AwaitingReceipt,
    Returned,
}

pub(crate) enum WorkerReturnDecision {
    Waiting,
    InputRejected,
    Returned,
}

impl WorkerReturn {
    pub(crate) const fn accept(self, input: WorkerReturnInput) -> (Self, WorkerReturnDecision) {
        match (self, input) {
            (Self::Returned, _) => (Self::Returned, WorkerReturnDecision::Returned),
            (Self::AwaitingStopAndReceipt, WorkerReturnInput::ExactWorkerStopped) => {
                (Self::AwaitingReceipt, WorkerReturnDecision::Waiting)
            }
            (Self::AwaitingStopAndReceipt, WorkerReturnInput::ExactShutdownReturned) => {
                (Self::AwaitingStop, WorkerReturnDecision::Waiting)
            }
            (Self::AwaitingStop, WorkerReturnInput::ExactWorkerStopped)
            | (Self::AwaitingReceipt, WorkerReturnInput::ExactShutdownReturned) => {
                (Self::Returned, WorkerReturnDecision::Returned)
            }
            (current, _) => (current, WorkerReturnDecision::InputRejected),
        }
    }
}

pub(crate) fn foreign_worker(expected: CreationId) -> CreationId {
    let mut workers = CreationSequence::new();
    let _occupied = workers
        .issue()
        .expect("a new sequence issues one worker ID");
    let foreign = workers
        .issue()
        .expect("the sequence issues another worker ID");
    assert_ne!(foreign, expected);
    foreign
}
