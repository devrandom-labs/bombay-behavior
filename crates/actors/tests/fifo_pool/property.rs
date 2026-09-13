use std::collections::VecDeque;
use std::convert::Infallible;

use behavior_actors::atomic::{
    AdmissionRejection, AssignWorker, Assignment, AssignmentReceipt, BacklogCapacity, FifoCommand,
    FifoEvent, FifoOutcomeKind, FifoPool, ImmediateActivation, Interruption, PoolFailureReaction,
    PoolRecovery, SubmissionId,
};
use behavior_actors::{
    ActionItemResult, Active, ChildReport, CreationId, ItemSettlement, MessageProtocol, Never,
    Recipient, ReplyDelivery, SettledItem, Step,
};
use proptest::collection::vec;
use proptest::prelude::{Just, Strategy, any};
use proptest::prop_assert;
use proptest::prop_assert_eq;
use proptest::prop_oneof;
use proptest::proptest;
use proptest::test_runner::{Config, TestCaseError};
use tokio::runtime::Builder;

use super::{ReadySearchPool, Role, RuntimeAddr, SearchWorker, ready_search_pool};

const BACKLOG_MAXIMUM: usize = 3;
const CUSTOMER_ADDRESS: u64 = 88;

#[derive(Clone, Copy, Debug)]
enum DeskInstruction {
    Submit(u8),
    AcceptDelivery,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeskJob {
    id: u64,
    payload: u8,
}

#[derive(Debug, Eq, PartialEq)]
enum DeskWorker {
    Available,
    AwaitingDelivery(DeskJob),
    Delivered(DeskJob),
    CompletionBeforeDelivery(DeskJob),
}

#[derive(Debug, Eq, PartialEq)]
struct QueueDesk {
    worker: DeskWorker,
    waiting: VecDeque<DeskJob>,
    next_job: u64,
}

#[derive(Debug, Eq, PartialEq)]
enum QueueNotice {
    Accepted {
        customer: u64,
        submission: u64,
        job: u64,
    },
    Rejected {
        customer: u64,
        submission: u64,
        payload: u8,
        reason: AdmissionRejection,
    },
    Completed {
        customer: u64,
        job: u64,
        worker_result: u16,
    },
}

#[derive(Debug, Eq, PartialEq)]
struct QueueTrace {
    notice: Option<QueueNotice>,
    assigned_payload: Option<u8>,
}

impl QueueTrace {
    const fn quiet() -> Self {
        Self {
            notice: None,
            assigned_payload: None,
        }
    }

    const fn notice(notice: QueueNotice) -> Self {
        Self {
            notice: Some(notice),
            assigned_payload: None,
        }
    }

    const fn assigned(notice: QueueNotice, payload: u8) -> Self {
        Self {
            notice: Some(notice),
            assigned_payload: Some(payload),
        }
    }
}

impl QueueDesk {
    fn new() -> Self {
        Self {
            worker: DeskWorker::Available,
            waiting: VecDeque::new(),
            next_job: 1,
        }
    }

    fn apply(self, instruction: DeskInstruction, submission: u64) -> (Self, Option<QueueTrace>) {
        let Self {
            worker,
            mut waiting,
            next_job,
        } = self;
        match (worker, instruction) {
            (DeskWorker::Available, DeskInstruction::Submit(payload)) => {
                let job = DeskJob {
                    id: next_job,
                    payload,
                };
                (
                    Self {
                        worker: DeskWorker::AwaitingDelivery(job),
                        waiting,
                        next_job: next_job + 1,
                    },
                    Some(QueueTrace::assigned(
                        QueueNotice::Accepted {
                            customer: CUSTOMER_ADDRESS,
                            submission,
                            job: job.id,
                        },
                        payload,
                    )),
                )
            }
            (worker, DeskInstruction::Submit(payload)) => {
                match waiting.len().cmp(&BACKLOG_MAXIMUM) {
                    core::cmp::Ordering::Less => {
                        let job = DeskJob {
                            id: next_job,
                            payload,
                        };
                        waiting.push_back(job);
                        (
                            Self {
                                worker,
                                waiting,
                                next_job: next_job + 1,
                            },
                            Some(QueueTrace::notice(QueueNotice::Accepted {
                                customer: CUSTOMER_ADDRESS,
                                submission,
                                job: job.id,
                            })),
                        )
                    }
                    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => (
                        Self {
                            worker,
                            waiting,
                            next_job,
                        },
                        Some(QueueTrace::notice(QueueNotice::Rejected {
                            customer: CUSTOMER_ADDRESS,
                            submission,
                            payload,
                            reason: AdmissionRejection::BacklogFull,
                        })),
                    ),
                }
            }
            (DeskWorker::AwaitingDelivery(job), DeskInstruction::AcceptDelivery) => (
                Self {
                    worker: DeskWorker::Delivered(job),
                    waiting,
                    next_job,
                },
                Some(QueueTrace::quiet()),
            ),
            (DeskWorker::CompletionBeforeDelivery(job), DeskInstruction::AcceptDelivery) => {
                Self::complete(job, waiting, next_job)
            }
            (DeskWorker::AwaitingDelivery(job), DeskInstruction::Complete) => (
                Self {
                    worker: DeskWorker::CompletionBeforeDelivery(job),
                    waiting,
                    next_job,
                },
                Some(QueueTrace::quiet()),
            ),
            (DeskWorker::Delivered(job), DeskInstruction::Complete) => {
                Self::complete(job, waiting, next_job)
            }
            (worker, _) => (
                Self {
                    worker,
                    waiting,
                    next_job,
                },
                None,
            ),
        }
    }

    fn complete(
        job: DeskJob,
        mut waiting: VecDeque<DeskJob>,
        next_job: u64,
    ) -> (Self, Option<QueueTrace>) {
        let next = waiting.pop_front();
        let assigned_payload = next.map(|assigned| assigned.payload);
        let worker = match next {
            Some(assigned) => DeskWorker::AwaitingDelivery(assigned),
            None => DeskWorker::Available,
        };
        (
            Self {
                worker,
                waiting,
                next_job,
            },
            Some(QueueTrace {
                notice: Some(QueueNotice::Completed {
                    customer: CUSTOMER_ADDRESS,
                    job: job.id,
                    worker_result: u16::from(job.payload) + 100,
                }),
                assigned_payload,
            }),
        )
    }
}

enum DeliveryCustody {
    Available,
    Awaiting {
        assignment: Assignment<u8>,
        receipt: AssignmentReceipt,
    },
    Delivered(Assignment<u8>),
    CompletionBeforeDelivery(AssignmentReceipt),
}

struct PoolWitness {
    pool: Active<FifoPool<Role, SearchWorker, ImmediateActivation, Never, Infallible, u8, u16>>,
    worker: CreationId,
    delivery: DeliveryCustody,
}

impl PoolWitness {
    fn new(ready: ReadySearchPool<Never>) -> Self {
        Self {
            pool: ready.pool,
            worker: ready.workers[0],
            delivery: DeliveryCustody::Available,
        }
    }

    fn apply(
        self,
        instruction: DeskInstruction,
        submission: u64,
    ) -> Result<(Self, Option<QueueTrace>), TestCaseError> {
        let Self {
            mut pool,
            worker,
            delivery,
        } = self;
        let (delivery, acted) = match (delivery, instruction) {
            (delivery, DeskInstruction::Submit(payload)) => {
                let customer = Recipient::<
                    MessageProtocol<RuntimeAddr, super::FifoOutcome<Role, u8, u16>>,
                >::global(RuntimeAddr(CUSTOMER_ADDRESS));
                let acted = pool
                    .receive(
                        RuntimeAddr(7),
                        FifoCommand::submit(SubmissionId::new(submission), payload, customer),
                    )
                    .map_err(|error| TestCaseError::fail(format!("submission failed: {error}")))?;
                (delivery, acted)
            }
            (
                DeliveryCustody::Awaiting {
                    assignment,
                    receipt,
                },
                DeskInstruction::AcceptDelivery,
            ) => {
                let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                    SettledItem::Attempted(ItemSettlement::Accepted(receipt));
                let acted = pool
                    .transition(FifoEvent::AssignmentSettled(accepted))
                    .map_err(|error| {
                        TestCaseError::fail(format!("delivery acceptance failed: {error}"))
                    })?;
                (DeliveryCustody::Delivered(assignment), acted)
            }
            (
                DeliveryCustody::CompletionBeforeDelivery(receipt),
                DeskInstruction::AcceptDelivery,
            ) => {
                let accepted: ActionItemResult<AssignWorker<SearchWorker, u8>> =
                    SettledItem::Attempted(ItemSettlement::Accepted(receipt));
                let acted = pool
                    .transition(FifoEvent::AssignmentSettled(accepted))
                    .map_err(|error| {
                        TestCaseError::fail(format!("late delivery acceptance failed: {error}"))
                    })?;
                (DeliveryCustody::Available, acted)
            }
            (
                DeliveryCustody::Awaiting {
                    assignment,
                    receipt,
                },
                DeskInstruction::Complete,
            ) => {
                let result = u16::from(*assignment.payload()) + 100;
                let acted = pool
                    .on(ChildReport::new(
                        worker,
                        assignment.complete(result).into_inner(),
                    ))
                    .map_err(|error| {
                        TestCaseError::fail(format!("early completion failed: {error}"))
                    })?;
                (DeliveryCustody::CompletionBeforeDelivery(receipt), acted)
            }
            (DeliveryCustody::Delivered(assignment), DeskInstruction::Complete) => {
                let result = u16::from(*assignment.payload()) + 100;
                let acted = pool
                    .on(ChildReport::new(
                        worker,
                        assignment.complete(result).into_inner(),
                    ))
                    .map_err(|error| TestCaseError::fail(format!("completion failed: {error}")))?;
                (DeliveryCustody::Available, acted)
            }
            (delivery, _) => {
                return Ok((
                    Self {
                        pool,
                        worker,
                        delivery,
                    },
                    None,
                ));
            }
        };

        prop_assert!(matches!(&acted.become_, Step::Continue));
        prop_assert!(acted.creates.is_empty());
        prop_assert!(acted.sends.worker_observations.is_empty());
        prop_assert!(acted.sends.worker_initializations.is_empty());
        prop_assert!(acted.sends.worker_activations.is_empty());
        prop_assert!(acted.sends.worker_preparations.is_empty());
        prop_assert!(acted.sends.restart_schedules.is_empty());
        prop_assert!(acted.sends.worker_shutdowns.is_empty());
        prop_assert!(acted.sends.diagnostics.is_empty());

        let mut outcomes = acted.sends.customer_outcomes.into_deliveries();
        let notice = match outcomes.len() {
            0 => None,
            1 => {
                let outcome = outcomes
                    .pop()
                    .ok_or_else(|| TestCaseError::fail("one customer outcome disappeared"))?;
                let delivery = match outcome {
                    ReplyDelivery::Logical(delivery) => delivery,
                    ReplyDelivery::Established(_) => {
                        return Err(TestCaseError::fail("customer route became established"));
                    }
                };
                let customer = delivery.to.address().0;
                let outcome = delivery.message;
                let notice = match outcome.kind() {
                    FifoOutcomeKind::Accepted => {
                        let (submission, job) = outcome.into_accepted().map_err(|_| {
                            TestCaseError::fail("accepted notice lost its identifiers")
                        })?;
                        QueueNotice::Accepted {
                            customer,
                            submission: submission.get(),
                            job: job.get(),
                        }
                    }
                    FifoOutcomeKind::Rejected => {
                        let (submission, payload, reason) = outcome
                            .into_rejected()
                            .map_err(|_| TestCaseError::fail("rejected notice lost its payload"))?;
                        QueueNotice::Rejected {
                            customer,
                            submission: submission.get(),
                            payload,
                            reason,
                        }
                    }
                    FifoOutcomeKind::Completed => {
                        prop_assert_eq!(outcome.role(), Some(&Role::Search));
                        let (job, worker_result) = outcome.into_completed().map_err(|_| {
                            TestCaseError::fail("completed notice lost its worker result")
                        })?;
                        QueueNotice::Completed {
                            customer,
                            job: job.get(),
                            worker_result,
                        }
                    }
                    FifoOutcomeKind::ReturnedQueued | FifoOutcomeKind::ReturnedAssigned => {
                        return Err(TestCaseError::fail(
                            "operating queue sequence returned accepted work",
                        ));
                    }
                };
                Some(notice)
            }
            _ => {
                return Err(TestCaseError::fail(
                    "one input emitted multiple customer outcomes",
                ));
            }
        };

        let mut assignments = acted.sends.worker_assignments.into_items();
        let emitted = match assignments.len() {
            0 => None,
            1 => assignments.pop(),
            _ => {
                return Err(TestCaseError::fail(
                    "one worker received multiple assignments",
                ));
            }
        };
        let (delivery, assigned_payload) = match (delivery, emitted) {
            (DeliveryCustody::Available, Some(action)) => {
                let (_, assignment, receipt) = action.into_parts();
                let payload = *assignment.payload();
                (
                    DeliveryCustody::Awaiting {
                        assignment,
                        receipt,
                    },
                    Some(payload),
                )
            }
            (delivery, None) => (delivery, None),
            (_, Some(_)) => {
                return Err(TestCaseError::fail(
                    "pool assigned work before releasing its worker",
                ));
            }
        };
        Ok((
            Self {
                pool,
                worker,
                delivery,
            },
            Some(QueueTrace {
                notice,
                assigned_payload,
            }),
        ))
    }
}

proptest! {
    #![proptest_config(Config {
        cases: 192,
        max_shrink_iters: 100_000,
        ..Config::default()
    })]

    #[test]
    fn generated_fifo_sequences_match_one_admission_queue(
        instructions in vec(
            prop_oneof![
                4 => any::<u8>().prop_map(DeskInstruction::Submit),
                2 => Just(DeskInstruction::AcceptDelivery),
                2 => Just(DeskInstruction::Complete),
            ],
            0..129,
        )
    ) {
        let runtime = Builder::new_current_thread()
            .build()
            .unwrap_or_else(|error| panic!("test runtime construction failed: {error}"));
        let roles = super::OrderedRoles::new(Role::Search, [])
            .unwrap_or_else(|_| panic!("one role is a valid FIFO roster"));
        let ready = runtime.block_on(ready_search_pool(
            roles,
            BacklogCapacity::new(BACKLOG_MAXIMUM),
            Interruption::Fail,
            PoolRecovery::<Never>::temporary(PoolFailureReaction::RetireRole),
        ));
        let mut witness = PoolWitness::new(ready);
        let mut desk = QueueDesk::new();
        let mut submission = 0_u64;

        for instruction in instructions {
            let (next_desk, expected) = desk.apply(instruction, submission);
            let (next_witness, actual) = witness.apply(instruction, submission)?;
            prop_assert_eq!(actual, expected);
            desk = next_desk;
            witness = next_witness;
            match instruction {
                DeskInstruction::Submit(_) => submission += 1,
                DeskInstruction::AcceptDelivery | DeskInstruction::Complete => {}
            }
        }
    }
}
