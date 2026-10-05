//! Accepted customer custody and exact worker-assignment reunion.

use core::num::NonZeroU64;
use std::sync::Arc;

use behavior::{
    ActionItem, EstablishedDelivery, EstablishedRecipient, ExactDeliveryReason, InterpretItem,
    Interpretation, InterpretationProgress, ItemSettlement, Never, Protocol, RecipientAddress,
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
/// let _ = behavior_actors::atomic::JobId::new(1);
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
/// let _ = behavior_actors::atomic::Assignment::new(String::from("job"));
/// ```
///
/// Completing consumes the affine authority, so the same assignment cannot
/// complete twice:
///
/// ```compile_fail,E0382
/// fn duplicate(assignment: behavior_actors::atomic::Assignment<u8>) {
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
/// let _ = behavior_actors::atomic::Completion::new(10_u16);
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
#[must_use = "worker assignment delivery must settle or remain in lifecycle custody"]
pub struct AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: RecipientAddress,
{
    target: EstablishedRecipient<P>,
    assignment: Assignment<Job>,
    receipt: AssignmentReceipt,
}

impl<P, Job> AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: RecipientAddress,
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

    /// Return a clone of the exact worker recipient selected by the pool.
    /// The assignment and its accepted receipt remain inside this request.
    #[must_use]
    pub fn target(&self) -> EstablishedRecipient<P> {
        self.target.clone()
    }

    /// Borrow the opaque accepted receipt inside the owning atomic module.
    #[must_use]
    pub(in crate::atomic) fn receipt(&self) -> AssignmentReceipt {
        AssignmentReceipt {
            assignment: self.receipt.assignment,
            worker: self.receipt.worker.clone(),
        }
    }

    /// Recover the exact delivery values inside the owning atomic module.
    #[must_use]
    pub(in crate::atomic) fn into_parts(
        self,
    ) -> (EstablishedRecipient<P>, Assignment<Job>, AssignmentReceipt) {
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

    /// Attempt the lower worker delivery while the complete request or partial
    /// assignment receipt remains in the caller's original interpretation slot.
    pub async fn settle<Host, RootEvent, Path>(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                <Self as ActionItem>::Custody,
                ItemSettlement<Self, AssignmentReceipt, ExactDeliveryReason, Never>,
            >,
        >,
        host: &mut Host,
    ) where
        Host: InterpretItem<EstablishedDelivery<P>, RootEvent, Path>,
        <P::Addr as RecipientAddress>::Established<P>: Send,
        Job: Send,
    {
        <Self as ActionItem>::prepare_interpretation(progress);
        if let Some(InterpretationProgress::Interpreting(custody)) = progress {
            if let Some((input, received)) = <Self as ActionItem>::interpretation_input(custody) {
                <Host as InterpretItem<EstablishedDelivery<P>, RootEvent, Path>>::interpret_item(
                    host, input, received,
                )
                .await;
            }
        }
        <Self as ActionItem>::finish_interpretation(progress);
    }
}

impl<P, Job> ActionItem for AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: Send,
    Job: Send,
{
    type Custody = (
        AssignmentReceipt,
        (Option<EstablishedDelivery<P>>, Option<Self::Reply>),
    );
    type Input<'a>
        = &'a mut Option<EstablishedDelivery<P>>
    where
        Self: 'a;
    type Reply = ItemSettlement<EstablishedDelivery<P>, (), ExactDeliveryReason, Never>;

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                Self::Custody,
                ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>,
            >,
        >,
    ) {
        if !matches!(progress, Some(InterpretationProgress::Original(_))) {
            return;
        }
        match progress.take() {
            Some(InterpretationProgress::Original(Self {
                target,
                assignment,
                receipt,
            })) => {
                *progress = Some(InterpretationProgress::Interpreting((
                    receipt,
                    (Some(EstablishedDelivery::new(target, assignment)), None),
                )));
            }
            retained => *progress = retained,
        }
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (_, (input, received)) = custody;
        if input.is_some() && received.is_none() {
            Some((input, received))
        } else {
            None
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<
                Self,
                Self::Custody,
                ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>,
            >,
        >,
    ) {
        if !matches!(
            progress,
            Some(InterpretationProgress::Interpreting((_, (None, Some(_)))))
        ) {
            return;
        }
        match progress.take() {
            Some(InterpretationProgress::Interpreting((receipt, (None, Some(reply))))) => {
                let received = match reply {
                    ItemSettlement::Accepted(()) => ItemSettlement::Accepted(receipt),
                    ItemSettlement::Rejected {
                        item: EstablishedDelivery { to, message },
                        reason,
                    } => ItemSettlement::Rejected {
                        item: Self {
                            target: to,
                            assignment: message,
                            receipt,
                        },
                        reason,
                    },
                    ItemSettlement::Corrupt {
                        item: EstablishedDelivery { to, message },
                        fault,
                    } => ItemSettlement::Corrupt {
                        item: Self {
                            target: to,
                            assignment: message,
                            receipt,
                        },
                        fault,
                    },
                    ItemSettlement::Blocked { prerequisite, .. } => match prerequisite {},
                };
                let interpretation = match received {
                    received @ ItemSettlement::Corrupt { .. } => Interpretation::Corrupt(received),
                    received => Interpretation::Complete(received),
                };
                *progress = Some(InterpretationProgress::Completed(interpretation));
            }
            retained => *progress = retained,
        }
    }

    type Accepted = AssignmentReceipt;
    type Rejection = ExactDeliveryReason;
    type Prerequisite = Never;
}

impl<P, Job> SourceAction for AssignWorker<P, Job>
where
    P: Protocol<Msg = Assignment<Job>>,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: Send,
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
    use core::future::Future;
    use std::collections::VecDeque;
    use std::sync::Arc;
    use std::time::Instant;

    use behavior::{
        ActionItem, Address, CreationSequence, EstablishedDelivery, EstablishedRecipient,
        ExactDeliveryReason, Here, InterpretItem, InterpretationProgress, ItemSettlement, MailAddr,
        MessageProtocol, Protocol, RecipientAddress,
    };

    use super::{
        AcceptedJobSequence, AssignWorker, AssignedJob, Assignment, AssignmentReceipt,
        AssignmentReceiptOutcome, AssignmentRejectionOutcome, AssignmentSequence,
        CompletionAuthority, CorrelationMatch, CustomerJob, WorkerCompletionOutcome,
        WorkerExitOutcome,
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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct DeliveryAddr(u64);

    impl Address for DeliveryAddr {
        type Nonce = u64;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct DeliveryEndpoint(u64);

    impl RecipientAddress for DeliveryAddr {
        type Established<P>
            = DeliveryEndpoint
        where
            P: Protocol<Addr = Self>;
    }

    struct MoveJob(Box<str>);

    type AssignmentProtocol = MessageProtocol<DeliveryAddr, Assignment<MoveJob>>;
    type ExactAssignment = EstablishedDelivery<AssignmentProtocol>;

    enum DeliveryAdmission {
        Accept,
        Reject,
    }

    struct AssignmentDeliveryHost {
        decisions: VecDeque<DeliveryAdmission>,
        observed_payloads: Vec<usize>,
    }

    impl InterpretItem<ExactAssignment, (), Here> for AssignmentDeliveryHost {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<ExactAssignment>,
            received: &'a mut Option<<ExactAssignment as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            ExactAssignment: 'a,
        {
            async move {
                if received.is_some() {
                    return;
                }
                let Some(delivery) = input.take() else {
                    return;
                };
                *received = Some({
                    self.observed_payloads
                        .push(delivery.message.payload().0.as_ptr() as usize);
                    match self
                        .decisions
                        .pop_front()
                        .expect("one decision per delivery")
                    {
                        DeliveryAdmission::Accept => ItemSettlement::Accepted(()),
                        DeliveryAdmission::Reject => ItemSettlement::Rejected {
                            item: delivery,
                            reason: ExactDeliveryReason::ClosedRecipient,
                        },
                    }
                });
            }
        }
    }

    #[tokio::test]
    async fn exact_assignment_settlement_preserves_original_correlation_after_transfer() {
        let worker = worker();
        let mut assignments = AssignmentSequence::new();
        let (first_correlation, first_assignment) = assignments
            .assign(&worker, MoveJob(Box::from("first")))
            .expect("first assignment correlation");
        let (second_correlation, second_assignment) = assignments
            .assign(&worker, MoveJob(Box::from("second")))
            .expect("second assignment correlation");
        let first_payload = first_assignment.payload().0.as_ptr();
        let second_payload = second_assignment.payload().0.as_ptr();
        let first_target = EstablishedRecipient::issued(DeliveryEndpoint(41));
        let second_target = EstablishedRecipient::issued(DeliveryEndpoint(42));
        let first = AssignWorker::<AssignmentProtocol, _>::new(
            first_target.clone(),
            &first_correlation,
            first_assignment,
        );
        let second = AssignWorker::<AssignmentProtocol, _>::new(
            second_target,
            &second_correlation,
            second_assignment,
        );
        let mut host = AssignmentDeliveryHost {
            decisions: VecDeque::from([DeliveryAdmission::Accept, DeliveryAdmission::Reject]),
            observed_payloads: Vec::new(),
        };

        let ItemSettlement::Accepted(second_receipt) = ({
            let mut progress = Some(InterpretationProgress::Original(second));
            AssignWorker::<AssignmentProtocol, MoveJob>::settle::<_, (), Here>(
                &mut progress,
                &mut host,
            )
            .await;
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        }) else {
            panic!("second assignment admission returns its held receipt");
        };
        assert!(matches!(
            second_receipt.compare(&second_correlation),
            CorrelationMatch::Exact
        ));
        let ItemSettlement::Rejected {
            item: first_returned,
            reason: ExactDeliveryReason::ClosedRecipient,
        } = ({
            let mut progress = Some(InterpretationProgress::Original(first));
            AssignWorker::<AssignmentProtocol, MoveJob>::settle::<_, (), Here>(
                &mut progress,
                &mut host,
            )
            .await;
            let Some(InterpretationProgress::Completed(settlement)) = progress else {
                panic!("the exact host returns its complete original settlement");
            };
            settlement.into_settlement()
        })
        else {
            panic!("first assignment returns its actual rejected delivery");
        };
        assert_eq!(first_returned.target(), first_target);
        let (_, returned_assignment, first_receipt) = first_returned.into_parts();
        assert_eq!(returned_assignment.payload().0.as_ptr(), first_payload);
        assert_eq!(&*returned_assignment.payload().0, "first");
        assert!(matches!(
            first_receipt.compare(&first_correlation),
            CorrelationMatch::Exact
        ));
        assert_eq!(
            host.observed_payloads,
            [second_payload as usize, first_payload as usize]
        );
        assert!(host.decisions.is_empty());
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
