//! Accepted customer custody and exact worker-assignment reunion.

use core::num::NonZeroU64;
use std::sync::Arc;

use behavior::{
    ActionItem, EndpointAddress, EstablishedRecipient, ExactDeliveryReason, Never, Protocol,
    ReportToParent, SourceAction,
};

use super::super::WorkerAttempt;

/// Customer-authored correlation echoed by the admission outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SubmissionId(u64);

impl SubmissionId {
    /// Name one customer submission.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Inspect the customer-authored value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Pool-issued correlation for one accepted job.
///
/// Applications may inspect a received identifier but cannot mint one:
///
/// ```compile_fail,E0599
/// use behavior_actors::atomic::JobId;
/// let _ = JobId::new(1);
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct JobId(NonZeroU64);

impl JobId {
    pub(in crate::atomic) const fn issued(value: NonZeroU64) -> Self {
        Self(value)
    }

    /// Inspect the opaque correlation for observation and persistence.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// One worker execution payload with affine completion authority.
///
/// Only a pool can issue an assignment:
///
/// ```compile_fail,E0599
/// use behavior_actors::atomic::Assignment;
/// let _ = Assignment::new(String::from("job"));
/// ```
///
/// Completing consumes the affine authority, so the same assignment cannot
/// complete twice:
///
/// ```compile_fail,E0382
/// use behavior_actors::atomic::Assignment;
/// fn duplicate(assignment: Assignment<u8>) {
///     let _first = assignment.complete(10_u16);
///     let _second = assignment.complete(11_u16);
/// }
/// ```
#[must_use = "an assignment must be completed, returned by delivery settlement, or transferred"]
pub struct Assignment<Job> {
    payload: Job,
    authority: CompletionAuthority,
}

impl<Job> Assignment<Job> {
    pub(in crate::atomic) const fn issued(payload: Job, authority: CompletionAuthority) -> Self {
        Self { payload, authority }
    }

    /// Borrow the execution payload without exposing pool correlation.
    #[must_use]
    pub const fn payload(&self) -> &Job {
        &self.payload
    }

    /// Consume the assignment and return one opaque completion to its pool.
    #[must_use]
    pub fn complete<WorkerResult>(
        self,
        worker_result: WorkerResult,
    ) -> ReportToParent<Completion<WorkerResult>> {
        ReportToParent::new(Completion {
            worker_result,
            authority: self.authority,
        })
    }

    pub(in crate::atomic) fn into_parts(self) -> (Job, CompletionAuthority) {
        (self.payload, self.authority)
    }
}

impl<Job> core::fmt::Debug for Assignment<Job>
where
    Job: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Assignment")
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}

/// Opaque worker result carrying the consumed authority of one exact assignment.
///
/// A result without assignment authority cannot be forged:
///
/// ```compile_fail,E0599
/// use behavior_actors::atomic::Completion;
/// let _ = Completion::new(10_u16);
/// ```
#[must_use = "a completion must return to its pool or remain in terminal custody"]
pub struct Completion<WorkerResult> {
    worker_result: WorkerResult,
    authority: CompletionAuthority,
}

impl<WorkerResult> Completion<WorkerResult> {
    pub(in crate::atomic) const fn authority(&self) -> &CompletionAuthority {
        &self.authority
    }

    pub(in crate::atomic) fn into_parts(self) -> (WorkerResult, CompletionAuthority) {
        (self.worker_result, self.authority)
    }
}

impl<WorkerResult> core::fmt::Debug for Completion<WorkerResult>
where
    WorkerResult: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Completion")
            .field("worker_result", &self.worker_result)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::atomic) struct AdmissionOrdinal(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AssignmentId(NonZeroU64);

#[derive(Debug, Eq, PartialEq)]
pub(in crate::atomic) struct AcceptedJobSequence {
    next: Option<NonZeroU64>,
}

impl AcceptedJobSequence {
    pub(in crate::atomic) const fn new() -> Self {
        Self {
            next: Some(NonZeroU64::MIN),
        }
    }

    pub(in crate::atomic) fn issue(&mut self) -> Option<(JobId, AdmissionOrdinal)> {
        let value = self.next?;
        self.next = value.checked_add(1);
        Some((JobId::issued(value), AdmissionOrdinal(value)))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::atomic) struct AssignmentSequence {
    next: Option<NonZeroU64>,
}

impl AssignmentSequence {
    pub(in crate::atomic) const fn new() -> Self {
        Self {
            next: Some(NonZeroU64::MIN),
        }
    }

    fn issue(&mut self) -> Option<AssignmentId> {
        let value = self.next?;
        self.next = value.checked_add(1);
        Some(AssignmentId(value))
    }

    pub(in crate::atomic) fn assign<Job>(
        &mut self,
        worker: &WorkerAttempt,
        payload: Job,
    ) -> Option<(CompletionCorrelation, Assignment<Job>)> {
        let assignment = self.issue()?;
        let token = Arc::new(());
        let correlation = CompletionCorrelation {
            assignment,
            worker: worker.clone(),
            token: Arc::clone(&token),
        };
        let authority = CompletionAuthority {
            assignment,
            worker: worker.clone(),
            token,
        };
        Some((correlation, Assignment::issued(payload, authority)))
    }
}

pub(in crate::atomic) struct CompletionAuthority {
    assignment: AssignmentId,
    worker: WorkerAttempt,
    token: Arc<()>,
}

#[derive(Clone)]
pub(in crate::atomic) struct CompletionCorrelation {
    assignment: AssignmentId,
    worker: WorkerAttempt,
    token: Arc<()>,
}

impl CompletionCorrelation {
    fn compare(&self, authority: &CompletionAuthority) -> CorrelationMatch {
        match (
            self.assignment == authority.assignment,
            self.worker == authority.worker,
            Arc::ptr_eq(&self.token, &authority.token),
        ) {
            (true, true, true) => CorrelationMatch::Exact,
            _ => CorrelationMatch::Foreign,
        }
    }
}

pub(in crate::atomic) enum CorrelationMatch {
    Exact,
    Foreign,
}

/// Accepted result proving which exact assignment delivery settled.
#[doc(hidden)]
pub struct AssignmentReceipt {
    assignment: AssignmentId,
    worker: WorkerAttempt,
}

impl AssignmentReceipt {
    fn issued(correlation: &CompletionCorrelation) -> Self {
        Self {
            assignment: correlation.assignment,
            worker: correlation.worker.clone(),
        }
    }

    fn compare(&self, correlation: &CompletionCorrelation) -> CorrelationMatch {
        match (
            self.assignment == correlation.assignment,
            self.worker == correlation.worker,
        ) {
            (true, true) => CorrelationMatch::Exact,
            _ => CorrelationMatch::Foreign,
        }
    }
}

/// One exact direct-worker delivery whose complete settlement returns to its pool.
#[doc(hidden)]
#[must_use = "worker assignment delivery must settle or remain in lifecycle custody"]
pub struct AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
{
    target: EstablishedRecipient<P>,
    assignment: Assignment<Job>,
    receipt: AssignmentReceipt,
}

impl<P, Job> AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
{
    pub(in crate::atomic) fn new(
        target: EstablishedRecipient<P>,
        correlation: &CompletionCorrelation,
        assignment: Assignment<Job>,
    ) -> Self {
        Self {
            target,
            assignment,
            receipt: AssignmentReceipt::issued(correlation),
        }
    }

    /// Borrow the exact worker recipient selected by the pool.
    #[doc(hidden)]
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<P> {
        self.target.clone()
    }

    /// Borrow the opaque accepted receipt Bombay must return after delivery.
    #[doc(hidden)]
    #[must_use]
    pub fn receipt(&self) -> AssignmentReceipt {
        AssignmentReceipt {
            assignment: self.receipt.assignment,
            worker: self.receipt.worker.clone(),
        }
    }

    /// Recover the exact delivery values when Communication rejects them.
    #[doc(hidden)]
    #[must_use]
    pub fn into_parts(self) -> (EstablishedRecipient<P>, Assignment<Job>, AssignmentReceipt) {
        (self.target, self.assignment, self.receipt)
    }

    pub(in crate::atomic) fn returned(
        target: EstablishedRecipient<P>,
        assignment: Assignment<Job>,
        receipt: AssignmentReceipt,
    ) -> Self {
        Self {
            target,
            assignment,
            receipt,
        }
    }
}

impl<P, Job> ActionItem for AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
{
    type Accepted = AssignmentReceipt;
    type Rejection = ExactDeliveryReason;
    type Prerequisite = Never;
}

impl<P, Job> SourceAction for AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    Job: Send,
{
    type Source = Self;
}

pub(in crate::atomic) struct CustomerJob<Job, Customer> {
    pub(in crate::atomic) id: JobId,
    pub(in crate::atomic) admitted: AdmissionOrdinal,
    pub(in crate::atomic) payload: Job,
    pub(in crate::atomic) customer: Customer,
}

enum AssignmentDelivery<WorkerResult, A>
where
    A: behavior::Address,
{
    AwaitingReceipt,
    Accepted,
    CompletionHasPriority {
        completion: Completion<WorkerResult>,
        later_exit: Option<crate::ChildStopped<A>>,
    },
    WorkerExitHasPriority {
        stopped: crate::ChildStopped<A>,
        later_completion: Option<Completion<WorkerResult>>,
    },
}

pub(in crate::atomic) struct AssignmentShutdown<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    pub(in crate::atomic) customer: CustomerJob<Job, Customer>,
    pub(in crate::atomic) worker: WorkerAttempt,
    pub(in crate::atomic) stopped: Option<crate::ChildStopped<A>>,
    pub(in crate::atomic) completion: Option<Completion<WorkerResult>>,
}

pub(in crate::atomic) struct AssignedJob<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    pub(in crate::atomic) customer: CustomerJob<Job, Customer>,
    correlation: CompletionCorrelation,
    delivery: AssignmentDelivery<WorkerResult, A>,
}

pub(in crate::atomic) enum AssignmentReceiptOutcome<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    AwaitingCompletion(AssignedJob<Job, Customer, WorkerResult, A>),
    JobCompleted {
        customer: CustomerJob<Job, Customer>,
        result: WorkerResult,
        stopped: Option<crate::ChildStopped<A>>,
    },
    JobInterrupted {
        customer: CustomerJob<Job, Customer>,
        stopped: crate::ChildStopped<A>,
        late_completion: Option<Completion<WorkerResult>>,
    },
}

pub(in crate::atomic) enum WorkerCompletionOutcome<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    AwaitingReceipt(AssignedJob<Job, Customer, WorkerResult, A>),
    JobCompleted {
        customer: CustomerJob<Job, Customer>,
        result: WorkerResult,
        stopped: Option<crate::ChildStopped<A>>,
    },
}

pub(in crate::atomic) enum AssignmentRejectionOutcome<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    JobReturned {
        customer: CustomerJob<Job, Customer>,
        stopped: Option<crate::ChildStopped<A>>,
    },
    ConflictingCompletion {
        customer: CustomerJob<Job, Customer>,
        assignment: Assignment<Job>,
        completion: Completion<WorkerResult>,
        stopped: Option<crate::ChildStopped<A>>,
    },
}

pub(in crate::atomic) enum WorkerExitOutcome<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    AwaitingReceipt(AssignedJob<Job, Customer, WorkerResult, A>),
    JobInterrupted {
        customer: CustomerJob<Job, Customer>,
        stopped: crate::ChildStopped<A>,
        late_completion: Option<Completion<WorkerResult>>,
    },
}

impl<Job, Customer, WorkerResult, A> AssignedJob<Job, Customer, WorkerResult, A>
where
    A: behavior::Address,
{
    pub(in crate::atomic) fn new(
        customer: CustomerJob<Job, Customer>,
        correlation: CompletionCorrelation,
    ) -> Self {
        Self {
            customer,
            correlation,
            delivery: AssignmentDelivery::AwaitingReceipt,
        }
    }

    pub(in crate::atomic) fn compare_receipt(
        &self,
        receipt: &AssignmentReceipt,
    ) -> CorrelationMatch {
        receipt.compare(&self.correlation)
    }

    pub(in crate::atomic) fn compare_completion(
        &self,
        completion: &Completion<WorkerResult>,
    ) -> CorrelationMatch {
        self.correlation.compare(completion.authority())
    }

    pub(in crate::atomic) fn shutdown(self) -> AssignmentShutdown<Job, Customer, WorkerResult, A> {
        let Self {
            customer,
            correlation,
            delivery,
        } = self;
        let worker = correlation.worker;
        match delivery {
            AssignmentDelivery::AwaitingReceipt | AssignmentDelivery::Accepted => {
                AssignmentShutdown {
                    customer,
                    worker,
                    stopped: None,
                    completion: None,
                }
            }
            AssignmentDelivery::CompletionHasPriority {
                completion,
                later_exit,
            } => AssignmentShutdown {
                customer,
                worker,
                stopped: later_exit,
                completion: Some(completion),
            },
            AssignmentDelivery::WorkerExitHasPriority {
                stopped,
                later_completion,
            } => AssignmentShutdown {
                customer,
                worker,
                stopped: Some(stopped),
                completion: later_completion,
            },
        }
    }

    pub(in crate::atomic) fn accept_receipt(
        self,
        receipt: AssignmentReceipt,
    ) -> Result<AssignmentReceiptOutcome<Job, Customer, WorkerResult, A>, (Self, AssignmentReceipt)>
    {
        if let CorrelationMatch::Foreign = receipt.compare(&self.correlation) {
            return Err((self, receipt));
        }
        let Self {
            customer,
            correlation,
            delivery,
        } = self;
        Ok(match delivery {
            AssignmentDelivery::AwaitingReceipt => {
                AssignmentReceiptOutcome::AwaitingCompletion(Self {
                    customer,
                    correlation,
                    delivery: AssignmentDelivery::Accepted,
                })
            }
            AssignmentDelivery::CompletionHasPriority {
                completion,
                later_exit,
            } => {
                let (result, _) = completion.into_parts();
                AssignmentReceiptOutcome::JobCompleted {
                    customer,
                    result,
                    stopped: later_exit,
                }
            }
            AssignmentDelivery::WorkerExitHasPriority {
                stopped,
                later_completion,
            } => AssignmentReceiptOutcome::JobInterrupted {
                customer,
                stopped,
                late_completion: later_completion,
            },
            AssignmentDelivery::Accepted => {
                return Err((
                    Self {
                        customer,
                        correlation,
                        delivery: AssignmentDelivery::Accepted,
                    },
                    receipt,
                ));
            }
        })
    }

    pub(in crate::atomic) fn accept_rejection(
        self,
        assignment: Assignment<Job>,
    ) -> Result<AssignmentRejectionOutcome<Job, Customer, WorkerResult, A>, (Self, Assignment<Job>)>
    {
        let (payload, authority) = assignment.into_parts();
        match self.correlation.compare(&authority) {
            CorrelationMatch::Foreign => {
                return Err((self, Assignment::issued(payload, authority)));
            }
            CorrelationMatch::Exact => {}
        }
        let assignment = Assignment::issued(payload, authority);
        Ok(match self.delivery {
            AssignmentDelivery::AwaitingReceipt => AssignmentRejectionOutcome::JobReturned {
                customer: self.customer,
                stopped: None,
            },
            AssignmentDelivery::CompletionHasPriority {
                completion,
                later_exit,
            } => AssignmentRejectionOutcome::ConflictingCompletion {
                customer: self.customer,
                assignment,
                completion,
                stopped: later_exit,
            },
            AssignmentDelivery::WorkerExitHasPriority {
                stopped,
                later_completion: None,
            } => AssignmentRejectionOutcome::JobReturned {
                customer: self.customer,
                stopped: Some(stopped),
            },
            AssignmentDelivery::WorkerExitHasPriority {
                stopped,
                later_completion: Some(completion),
            } => AssignmentRejectionOutcome::ConflictingCompletion {
                customer: self.customer,
                assignment,
                completion,
                stopped: Some(stopped),
            },
            AssignmentDelivery::Accepted => {
                return Err((
                    Self {
                        customer: self.customer,
                        correlation: self.correlation,
                        delivery: AssignmentDelivery::Accepted,
                    },
                    assignment,
                ));
            }
        })
    }

    pub(in crate::atomic) fn accept_completion(
        self,
        completion: Completion<WorkerResult>,
    ) -> Result<
        WorkerCompletionOutcome<Job, Customer, WorkerResult, A>,
        (Self, Completion<WorkerResult>),
    > {
        match self.correlation.compare(completion.authority()) {
            CorrelationMatch::Foreign => return Err((self, completion)),
            CorrelationMatch::Exact => {}
        }
        let Self {
            customer,
            correlation,
            delivery,
        } = self;
        Ok(match delivery {
            AssignmentDelivery::AwaitingReceipt => WorkerCompletionOutcome::AwaitingReceipt(Self {
                customer,
                correlation,
                delivery: AssignmentDelivery::CompletionHasPriority {
                    completion,
                    later_exit: None,
                },
            }),
            AssignmentDelivery::Accepted => {
                let (result, _) = completion.into_parts();
                WorkerCompletionOutcome::JobCompleted {
                    customer,
                    result,
                    stopped: None,
                }
            }
            AssignmentDelivery::WorkerExitHasPriority {
                stopped,
                later_completion: None,
            } => WorkerCompletionOutcome::AwaitingReceipt(Self {
                customer,
                correlation,
                delivery: AssignmentDelivery::WorkerExitHasPriority {
                    stopped,
                    later_completion: Some(completion),
                },
            }),
            delivery @ (AssignmentDelivery::CompletionHasPriority { .. }
            | AssignmentDelivery::WorkerExitHasPriority {
                later_completion: Some(_),
                ..
            }) => {
                return Err((
                    Self {
                        customer,
                        correlation,
                        delivery,
                    },
                    completion,
                ));
            }
        })
    }

    pub(in crate::atomic) fn accept_worker_exit(
        self,
        stopped: crate::ChildStopped<A>,
    ) -> Result<WorkerExitOutcome<Job, Customer, WorkerResult, A>, (Self, crate::ChildStopped<A>)>
    {
        if stopped.child != self.correlation.worker.creation() {
            return Err((self, stopped));
        }
        let Self {
            customer,
            correlation,
            delivery,
        } = self;
        Ok(match delivery {
            AssignmentDelivery::AwaitingReceipt => WorkerExitOutcome::AwaitingReceipt(Self {
                customer,
                correlation,
                delivery: AssignmentDelivery::WorkerExitHasPriority {
                    stopped,
                    later_completion: None,
                },
            }),
            AssignmentDelivery::Accepted => WorkerExitOutcome::JobInterrupted {
                customer,
                stopped,
                late_completion: None,
            },
            AssignmentDelivery::CompletionHasPriority {
                completion,
                later_exit: None,
            } => WorkerExitOutcome::AwaitingReceipt(Self {
                customer,
                correlation,
                delivery: AssignmentDelivery::CompletionHasPriority {
                    completion,
                    later_exit: Some(stopped),
                },
            }),
            delivery @ (AssignmentDelivery::WorkerExitHasPriority { .. }
            | AssignmentDelivery::CompletionHasPriority {
                later_exit: Some(_),
                ..
            }) => {
                return Err((
                    Self {
                        customer,
                        correlation,
                        delivery,
                    },
                    stopped,
                ));
            }
        })
    }

    pub(in crate::atomic) fn observed_stop(&self) -> Option<&crate::ChildStopped<A>> {
        match &self.delivery {
            AssignmentDelivery::CompletionHasPriority {
                later_exit: Some(stopped),
                ..
            }
            | AssignmentDelivery::WorkerExitHasPriority { stopped, .. } => Some(stopped),
            AssignmentDelivery::AwaitingReceipt
            | AssignmentDelivery::Accepted
            | AssignmentDelivery::CompletionHasPriority {
                later_exit: None, ..
            } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use behavior::{CreationSequence, MailAddr};

    use super::{
        AcceptedJobSequence, AssignedJob, AssignmentReceipt, AssignmentReceiptOutcome,
        AssignmentRejectionOutcome, AssignmentSequence, CompletionAuthority, CustomerJob,
        WorkerCompletionOutcome, WorkerExitOutcome,
    };
    use crate::atomic::WorkerAttempt;
    use crate::{ChildStopped, Crash};

    fn worker() -> WorkerAttempt {
        let mut creations = CreationSequence::new();
        let creation = creations
            .issue()
            .unwrap_or_else(|| panic!("test creation correlation is available"));
        WorkerAttempt::issued(creation)
    }

    fn customer() -> CustomerJob<u8, u8> {
        let mut jobs = AcceptedJobSequence::new();
        let (id, admitted) = jobs
            .issue()
            .unwrap_or_else(|| panic!("test job correlation is available"));
        CustomerJob {
            id,
            admitted,
            payload: 7,
            customer: 9,
        }
    }

    #[test]
    fn completion_waits_for_delivery_acceptance() {
        let worker = worker();
        let mut assignments = AssignmentSequence::new();
        let (correlation, execution) = assignments
            .assign(&worker, 7)
            .unwrap_or_else(|| panic!("test assignment correlation is available"));
        let receipt = AssignmentReceipt::issued(&correlation);
        let completion = execution.complete(21).into_inner();
        let assigned: AssignedJob<_, _, _, MailAddr> = AssignedJob::new(customer(), correlation);

        let waiting = assigned
            .accept_completion(completion)
            .unwrap_or_else(|_| panic!("exact completion is admitted"));
        let WorkerCompletionOutcome::AwaitingReceipt(waiting) = waiting else {
            panic!("completion alone cannot resolve customer custody")
        };
        let completed = waiting
            .accept_receipt(receipt)
            .unwrap_or_else(|_| panic!("exact delivery receipt is admitted"));
        let AssignmentReceiptOutcome::JobCompleted {
            customer,
            result,
            stopped,
        } = completed
        else {
            panic!("accepted delivery releases the retained completion")
        };
        assert_eq!(customer.payload, 7);
        assert_eq!(result, 21);
        assert_eq!(stopped, None);
    }

    #[test]
    fn stop_waits_for_delivery_acceptance_and_wins_terminal_order() {
        let worker = worker();
        let mut assignments = AssignmentSequence::new();
        let (correlation, _) = assignments
            .assign(&worker, 7)
            .unwrap_or_else(|| panic!("test assignment correlation is available"));
        let receipt = AssignmentReceipt::issued(&correlation);
        let stop = ChildStopped::new(worker.creation(), Err(Crash::Failed), Instant::now());
        let assigned: AssignedJob<u8, u8, u8, MailAddr> = AssignedJob::new(customer(), correlation);

        let waiting = assigned
            .accept_worker_exit(stop)
            .unwrap_or_else(|_| panic!("exact stop is admitted"));
        let WorkerExitOutcome::AwaitingReceipt(waiting) = waiting else {
            panic!("stop alone cannot resolve customer custody")
        };
        let interrupted = waiting
            .accept_receipt(receipt)
            .unwrap_or_else(|_| panic!("exact delivery receipt is admitted"));
        let AssignmentReceiptOutcome::JobInterrupted {
            customer,
            stopped,
            late_completion,
        } = interrupted
        else {
            panic!("accepted delivery releases the retained stop")
        };
        assert_eq!(customer.payload, 7);
        assert_eq!(stopped.child, worker.creation());
        match late_completion {
            None => {}
            Some(_) => panic!("no later completion was observed"),
        }
    }

    #[test]
    fn rejected_delivery_returns_the_execution_and_customer_obligation() {
        let worker = worker();
        let mut assignments = AssignmentSequence::new();
        let (correlation, execution) = assignments
            .assign(&worker, 7)
            .unwrap_or_else(|| panic!("test assignment correlation is available"));
        let assigned: AssignedJob<u8, u8, u8, MailAddr> = AssignedJob::new(customer(), correlation);

        let returned = assigned
            .accept_rejection(execution)
            .unwrap_or_else(|_| panic!("exact returned execution reunites authority"));
        let AssignmentRejectionOutcome::JobReturned { customer, stopped } = returned else {
            panic!("rejected delivery reunites authority and returns customer custody")
        };
        assert_eq!(customer.payload, 7);
        assert_eq!(stopped, None);
    }

    #[test]
    fn foreign_delivery_rejection_preserves_both_assignments() {
        let worker = worker();
        let mut assignments = AssignmentSequence::new();
        let (correlation, _) = assignments
            .assign(&worker, 7)
            .unwrap_or_else(|| panic!("first assignment correlation is available"));
        let receipt = AssignmentReceipt::issued(&correlation);
        let (_, foreign) = assignments
            .assign(&worker, 11)
            .unwrap_or_else(|| panic!("second assignment correlation is available"));
        let assigned: AssignedJob<u8, u8, u8, MailAddr> = AssignedJob::new(customer(), correlation);

        let (assigned, foreign) = match assigned.accept_rejection(foreign) {
            Err(returned) => returned,
            Ok(_) => panic!("a foreign assignment cannot reunite authority"),
        };
        assert_eq!(foreign.payload(), &11);
        let accepted = assigned
            .accept_receipt(receipt)
            .unwrap_or_else(|_| panic!("the original assignment remains current"));
        assert!(matches!(
            accepted,
            AssignmentReceiptOutcome::AwaitingCompletion(_)
        ));
    }

    #[test]
    fn returned_authority_after_completion_is_a_contradiction() {
        let worker = worker();
        let mut assignments = AssignmentSequence::new();
        let (correlation, execution) = assignments
            .assign(&worker, 7)
            .unwrap_or_else(|| panic!("test assignment correlation is available"));
        let returned = crate::atomic::Assignment::issued(
            7,
            CompletionAuthority {
                assignment: correlation.assignment,
                worker: correlation.worker.clone(),
                token: Arc::clone(&correlation.token),
            },
        );
        let completion = execution.complete(21).into_inner();
        let assigned: AssignedJob<u8, u8, u8, MailAddr> = AssignedJob::new(customer(), correlation);
        let waiting = assigned
            .accept_completion(completion)
            .unwrap_or_else(|_| panic!("exact completion is admitted"));
        let WorkerCompletionOutcome::AwaitingReceipt(waiting) = waiting else {
            panic!("completion must remain pending before delivery settlement")
        };

        let contradiction = waiting
            .accept_rejection(returned)
            .unwrap_or_else(|_| panic!("the returned authority has exact correlation"));
        let AssignmentRejectionOutcome::ConflictingCompletion {
            customer,
            assignment,
            completion,
            stopped,
        } = contradiction
        else {
            panic!("returned authority contradicts its retained completion")
        };
        assert_eq!(customer.payload, 7);
        assert_eq!(assignment.payload(), &7);
        let (result, _) = completion.into_parts();
        assert_eq!(result, 21);
        assert_eq!(stopped, None);
    }
}
