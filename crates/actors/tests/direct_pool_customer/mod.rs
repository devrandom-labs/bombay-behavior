//! Customer custody shared by FIFO and keyed direct workers.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeskInput {
    DeliveryAccepted,
    WorkEnded(WorkEnding),
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkEnding {
    Completed,
    WorkerExited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeskReturn {
    WorkerExited,
    PoolClosed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DeskState {
    AwaitingDelivery(VecDeque<WorkEnding>),
    Delivered,
    Completed,
    Returned(DeskReturn),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeskJob {
    pub(crate) customer: u64,
    pub(crate) submission: u64,
    pub(crate) job: u64,
    pub(crate) payload: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeskNotice {
    Accepted {
        customer: u64,
        submission: u64,
        job: u64,
    },
    Completed {
        customer: u64,
        job: u64,
        worker_result: u16,
    },
    Returned {
        customer: u64,
        job: u64,
        payload: u8,
        reason: DeskReturn,
    },
}

pub(crate) struct CustomerDesk {
    job: DeskJob,
    state: DeskState,
}

impl CustomerDesk {
    pub(crate) fn accepted(job: DeskJob) -> (Self, DeskNotice) {
        let notice = DeskNotice::Accepted {
            customer: job.customer,
            submission: job.submission,
            job: job.job,
        };
        (
            Self {
                job,
                state: DeskState::AwaitingDelivery(VecDeque::new()),
            },
            notice,
        )
    }

    pub(crate) fn apply(self, input: DeskInput) -> (Self, Option<DeskNotice>) {
        let Self { job, state } = self;
        let (next, notice) = match (state, input) {
            (DeskState::AwaitingDelivery(mut observations), DeskInput::DeliveryAccepted) => {
                match observations.pop_front() {
                    Some(ending) => Self::settle(job, ending),
                    None => (DeskState::Delivered, None),
                }
            }
            (DeskState::AwaitingDelivery(mut observations), DeskInput::WorkEnded(ending)) => {
                observations.push_back(ending);
                (DeskState::AwaitingDelivery(observations), None)
            }
            (DeskState::Delivered, DeskInput::WorkEnded(ending)) => Self::settle(job, ending),
            (DeskState::AwaitingDelivery(_) | DeskState::Delivered, DeskInput::Shutdown) => {
                Self::return_job(job, DeskReturn::PoolClosed)
            }
            (state, _) => (state, None),
        };
        (Self { job, state: next }, notice)
    }

    fn settle(job: DeskJob, ending: WorkEnding) -> (DeskState, Option<DeskNotice>) {
        match ending {
            WorkEnding::Completed => (
                DeskState::Completed,
                Some(DeskNotice::Completed {
                    customer: job.customer,
                    job: job.job,
                    worker_result: 58,
                }),
            ),
            WorkEnding::WorkerExited => Self::return_job(job, DeskReturn::WorkerExited),
        }
    }

    fn return_job(job: DeskJob, reason: DeskReturn) -> (DeskState, Option<DeskNotice>) {
        (
            DeskState::Returned(reason),
            Some(DeskNotice::Returned {
                customer: job.customer,
                job: job.job,
                payload: job.payload,
                reason,
            }),
        )
    }
}

pub(crate) const fn input_orders() -> [[DeskInput; 4]; 24] {
    let delivery = DeskInput::DeliveryAccepted;
    let completed = DeskInput::WorkEnded(WorkEnding::Completed);
    let exited = DeskInput::WorkEnded(WorkEnding::WorkerExited);
    let shutdown = DeskInput::Shutdown;

    [
        [delivery, completed, exited, shutdown],
        [delivery, completed, shutdown, exited],
        [delivery, exited, completed, shutdown],
        [delivery, exited, shutdown, completed],
        [delivery, shutdown, completed, exited],
        [delivery, shutdown, exited, completed],
        [completed, delivery, exited, shutdown],
        [completed, delivery, shutdown, exited],
        [completed, exited, delivery, shutdown],
        [completed, exited, shutdown, delivery],
        [completed, shutdown, delivery, exited],
        [completed, shutdown, exited, delivery],
        [exited, delivery, completed, shutdown],
        [exited, delivery, shutdown, completed],
        [exited, completed, delivery, shutdown],
        [exited, completed, shutdown, delivery],
        [exited, shutdown, delivery, completed],
        [exited, shutdown, completed, delivery],
        [shutdown, delivery, completed, exited],
        [shutdown, delivery, exited, completed],
        [shutdown, completed, delivery, exited],
        [shutdown, completed, exited, delivery],
        [shutdown, exited, delivery, completed],
        [shutdown, exited, completed, delivery],
    ]
}
