//! Typed worker preparation outside the pure supervisor transition.

use core::marker::PhantomData;
use core::ops::ControlFlow;
use std::sync::Arc;

use behavior::{
    ActionItem, ActionItemResult, Behavior, InterpretationProgress, ItemSettlement, Never,
    SettledItem, SourceAction, finish_item, prepare_item,
};

use crate::atomic::RoleName;

use super::{ActivationPlan, PreparedWorker, WorkerSubmission};

pub(in super::super) struct PreparationTicket {
    token: Arc<()>,
}

impl PreparationTicket {
    fn reserve() -> (Self, Self) {
        let token = Arc::new(());
        (
            Self {
                token: Arc::clone(&token),
            },
            Self { token },
        )
    }

    pub(in super::super) fn matches(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.token, &other.token)
    }
}

/// The one preparation correlation retained by an actor across a source task.
///
/// An issued request can accept its start settlement. Only after that exact
/// receipt may the actor accept the late source return. Keeping the phase
/// beside the ticket denies duplicate starts and early completion without an
/// arrival-history flag.
pub(in super::super) enum WorkerPreparationExpectation {
    Issued(PreparationTicket),
    Started(PreparationTicket),
}

impl WorkerPreparationExpectation {
    pub(in super::super) fn issued(ticket: PreparationTicket) -> Self {
        Self::Issued(ticket)
    }

    pub(in super::super) fn accept_start(
        self,
        receipt: &WorkerPreparationStarted,
    ) -> Result<Self, Self> {
        match self {
            Self::Issued(ticket) if receipt.accepts(&ticket) => Ok(Self::Started(ticket)),
            expectation => Err(expectation),
        }
    }

    pub(in super::super) fn accepts_issued<Source, Role, Worker, Plan>(
        &self,
        input: &ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    ) -> bool
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Role: Send + Sync,
        Worker: Behavior + Send,
        Plan: ActivationPlan,
    {
        match self {
            Self::Issued(ticket) => preparation_start_accepts(input, ticket),
            Self::Started(_) => false,
        }
    }

    pub(in super::super) fn accepts_return<Source, Role, Worker, Plan>(
        &self,
        returned: &WorkerPreparation<Source, Role, Worker, Plan>,
    ) -> bool
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Worker: Behavior + Send,
        Plan: ActivationPlan,
    {
        match self {
            Self::Issued(_) => false,
            Self::Started(ticket) => returned.accepts(ticket),
        }
    }
}

/// Static declaration implemented by one concrete Bombay worker-source
/// capability.
///
/// This trait declares types only. It deliberately has no method: the source is
/// an affine capability interpreted by Bombay, not an application callback
/// invoked by `FixedSupervisor`.
///
/// A source declared for another worker cannot prepare this supervisor:
///
/// ```compile_fail,E0277
/// struct DeclaredWorker;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl DeclaredWorker {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never)
///         -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct OtherWorker;
/// #[behavior::behavior(addr = behavior::MailAddr, message = behavior::Never)]
/// impl OtherWorker {
///     fn receive(&mut self, _: behavior::MailAddr, message: behavior::Never)
///         -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Source;
/// impl behavior_actors::atomic::WorkerSource<(), DeclaredWorker,
///     behavior_actors::atomic::ImmediateActivation> for Source
/// {
///     type WorkerRejection = behavior::Never;
///     type SourceRejection = behavior::Never;
/// }
/// fn requires_declared<Source>()
/// where
///     Source: behavior_actors::atomic::WorkerSource<
///         (), OtherWorker, behavior_actors::atomic::ImmediateActivation,
///     >,
/// {}
/// requires_declared::<Source>();
/// ```
pub trait WorkerSource<Role, Worker, Plan>: Send
where
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    /// Exact rejection while preparing one selected role.
    type WorkerRejection: Send;
    /// Exact rejection before the first worker submission is prepared.
    type SourceRejection: Send;
}

impl<Role, Worker, Plan> WorkerSource<Role, Worker, Plan> for Never
where
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

/// One non-empty ordered request for replacement worker submissions.
///
/// The request cannot report a prepared worker before its exact start is
/// committed:
///
/// ```compile_fail,E0599
/// fn premature_submission<Source, Role, Worker, Plan>(
///     request: behavior_actors::atomic::PrepareWorkers<Source, Role, Worker, Plan>,
///     submission: behavior_actors::atomic::WorkerSubmission<Worker, Plan>,
/// )
/// where
///     Source: behavior_actors::atomic::WorkerSource<Role, Worker, Plan>,
///     Worker: behavior::Behavior + Send,
///     Plan: behavior_actors::atomic::ActivationPlan,
/// {
///     let _ = request.accept(submission);
/// }
/// ```
#[must_use = "worker preparation must settle or transfer outward"]
pub struct PrepareWorkers<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    ticket: PreparationTicket,
    source: Source,
    first: RoleName<Role>,
    remaining: Vec<RoleName<Role>>,
    worker: PhantomData<fn() -> Worker>,
    plan: PhantomData<fn() -> Plan>,
}

/// Receipt that the runtime committed one exact worker-preparation start.
///
/// This proves neither a completed source nor permission to create a worker.
/// Its private ticket names the issued preparation without exposing a runtime
/// address or allowing the actor to forge a receipt for another request.
#[must_use = "worker preparation start must return to its source"]
pub struct WorkerPreparationStarted {
    ticket: PreparationTicket,
}

impl WorkerPreparationStarted {
    pub(in super::super) fn accepts(&self, expected: &PreparationTicket) -> bool {
        self.ticket.matches(expected)
    }
}

/// Source work after one accepted start and before its first submission.
///
/// Only this phase can return a source rejection. Once it accepts a first
/// submission, the returned pending request has a nonempty prepared prefix
/// and can report only worker-specific rejection for later roles.
#[must_use = "started worker preparation must settle or transfer outward"]
pub struct StartingWorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    ticket: PreparationTicket,
    source: Source,
    first: RoleName<Role>,
    remaining: Vec<RoleName<Role>>,
    worker: PhantomData<fn() -> Worker>,
    plan: PhantomData<fn() -> Plan>,
}

impl<Source, Role, Worker, Plan> StartingWorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    /// Borrow the source and first selected application role.
    #[must_use]
    pub fn source_and_role(&mut self) -> (&mut Source, &Role) {
        (&mut self.source, self.first.role())
    }

    /// Accept the first submission and advance or complete the ordered group.
    #[must_use]
    pub fn accept(
        self,
        submission: WorkerSubmission<Worker, Plan>,
    ) -> ControlFlow<
        WorkerPreparation<Source, Role, Worker, Plan>,
        PendingWorkerPreparation<Source, Role, Worker, Plan>,
    > {
        advance_preparation(
            self.ticket,
            self.source,
            Vec::new(),
            self.first,
            self.remaining,
            submission,
        )
    }

    /// Return an exact worker rejection for the first selected role.
    #[must_use]
    pub fn reject(
        self,
        reason: Source::WorkerRejection,
    ) -> WorkerPreparation<Source, Role, Worker, Plan> {
        rejected_preparation(
            self.ticket,
            self.source,
            Vec::new(),
            self.first,
            reason,
            self.remaining,
        )
    }

    /// Return the source and its exact rejection before any worker submission.
    #[must_use]
    pub fn reject_source(
        self,
        reason: Source::SourceRejection,
    ) -> WorkerPreparation<Source, Role, Worker, Plan> {
        WorkerPreparation {
            ticket: self.ticket,
            outcome: WorkerPreparationOutcome::SourceRejected {
                source: self.source,
                failed_role: self.first,
                reason,
                remaining: self.remaining,
            },
        }
    }
}

impl<Source, Role, Worker, Plan> PrepareWorkers<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    pub(in super::super) fn new(
        source: Source,
        first: RoleName<Role>,
        remaining: Vec<RoleName<Role>>,
    ) -> (PreparationTicket, Self) {
        let (expected, ticket) = PreparationTicket::reserve();
        (
            expected,
            Self {
                ticket,
                source,
                first,
                remaining,
                worker: PhantomData,
                plan: PhantomData,
            },
        )
    }

    pub(in super::super) fn accepts(&self, expected: &PreparationTicket) -> bool {
        expected.matches(&self.ticket)
    }

    /// Borrow the source and first selected role before committing the start.
    #[must_use]
    pub fn source_and_role(&mut self) -> (&mut Source, &Role) {
        (&mut self.source, self.first.role())
    }

    pub(in super::super) fn into_parts(self) -> (Source, RoleName<Role>, Vec<RoleName<Role>>) {
        (self.source, self.first, self.remaining)
    }

    /// Commit one start and transfer the affine source to its first attempt.
    ///
    /// The issued request is consumed, so another start receipt cannot be
    /// produced from the same source action.
    #[must_use]
    pub fn start(
        self,
    ) -> (
        WorkerPreparationStarted,
        StartingWorkerPreparation<Source, Role, Worker, Plan>,
    ) {
        let started = WorkerPreparationStarted {
            ticket: PreparationTicket {
                token: Arc::clone(&self.ticket.token),
            },
        };
        let starting = StartingWorkerPreparation {
            ticket: self.ticket,
            source: self.source,
            first: self.first,
            remaining: self.remaining,
            worker: PhantomData,
            plan: PhantomData,
        };
        (started, starting)
    }
}

pub(in super::super) fn preparation_start_accepts<Source, Role, Worker, Plan>(
    result: &ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    expected: &PreparationTicket,
) -> bool
where
    Source: WorkerSource<Role, Worker, Plan>,
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    match result {
        SettledItem::Attempted(ItemSettlement::Accepted(started)) => started.accepts(expected),
        SettledItem::Attempted(ItemSettlement::Rejected { item, .. })
        | SettledItem::Attempted(ItemSettlement::Blocked { item, .. })
        | SettledItem::Attempted(ItemSettlement::Corrupt { item, .. })
        | SettledItem::Unattempted(item) => item.accepts(expected),
    }
}

pub(in super::super) enum WorkerPreparationOutcome<
    Source,
    Role,
    Worker,
    Plan,
    WorkerRejection,
    SourceRejection,
> {
    Prepared {
        source: Source,
        members: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
    },
    WorkerRejected {
        source: Source,
        prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
        failed_role: RoleName<Role>,
        reason: WorkerRejection,
        remaining: Vec<RoleName<Role>>,
    },
    SourceRejected {
        source: Source,
        failed_role: RoleName<Role>,
        reason: SourceRejection,
        remaining: Vec<RoleName<Role>>,
    },
}

/// Non-empty remainder of one exact worker-preparation request.
///
/// Once the first worker submission has been accepted, a later failure is a
/// worker rejection; the source cannot be reclassified as rejected:
///
/// ```compile_fail,E0599
/// fn late_source_rejection<Source, Role, Worker, Plan>(
///     pending: behavior_actors::atomic::PendingWorkerPreparation<Source, Role, Worker, Plan>,
///     reason: Source::SourceRejection,
/// )
/// where
///     Source: behavior_actors::atomic::WorkerSource<Role, Worker, Plan>,
///     Worker: behavior::Behavior + Send,
///     Plan: behavior_actors::atomic::ActivationPlan,
/// {
///     let _ = pending.reject_source(reason);
/// }
/// ```
#[must_use = "worker preparation must advance, reject, or transfer outward"]
pub struct PendingWorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    ticket: PreparationTicket,
    source: Source,
    prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
    current: RoleName<Role>,
    remaining: Vec<RoleName<Role>>,
}

impl<Source, Role, Worker, Plan> PendingWorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    /// Borrow the source and current application role for one Bombay-owned
    /// preparation attempt.
    #[must_use]
    pub fn source_and_role(&mut self) -> (&mut Source, &Role) {
        (&mut self.source, self.current.role())
    }

    /// Borrow each accepted role, original worker, and activation plan in order.
    ///
    /// The complete pending request, including its original preparation ticket,
    /// remains owned by the caller. No submission is cloned or transferred.
    #[must_use]
    pub fn prepared_workers(&self) -> impl Iterator<Item = (&Role, &Worker, &Plan)> {
        self.prepared.iter().map(|prepared| {
            (
                prepared.role.role(),
                &prepared.submission.worker,
                &prepared.submission.activation,
            )
        })
    }

    /// Borrow the untouched roles after the current role in their original order.
    ///
    /// The existing `source_and_role` method supplies the source and current role.
    #[must_use]
    pub fn remaining_roles(&self) -> impl Iterator<Item = &Role> {
        self.remaining.iter().map(RoleName::role)
    }

    /// Accept one submission for the current role.
    #[must_use]
    pub fn accept(
        self,
        submission: WorkerSubmission<Worker, Plan>,
    ) -> ControlFlow<
        WorkerPreparation<Source, Role, Worker, Plan>,
        PendingWorkerPreparation<Source, Role, Worker, Plan>,
    > {
        advance_preparation(
            self.ticket,
            self.source,
            self.prepared,
            self.current,
            self.remaining,
            submission,
        )
    }

    /// Return an exact rejection for the current role.
    #[must_use]
    pub fn reject(
        self,
        reason: Source::WorkerRejection,
    ) -> WorkerPreparation<Source, Role, Worker, Plan> {
        rejected_preparation(
            self.ticket,
            self.source,
            self.prepared,
            self.current,
            reason,
            self.remaining,
        )
    }
}

/// Complete result of source work after one accepted preparation start.
#[must_use = "worker preparation must return to its supervisor or retire outward"]
pub struct WorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    ticket: PreparationTicket,
    outcome: WorkerPreparationOutcome<
        Source,
        Role,
        Worker,
        Plan,
        Source::WorkerRejection,
        Source::SourceRejection,
    >,
}

impl<Source, Role, Worker, Plan> WorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    pub(in super::super) fn from_parts(
        ticket: PreparationTicket,
        outcome: WorkerPreparationOutcome<
            Source,
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
    ) -> Self {
        Self { ticket, outcome }
    }

    pub(in super::super) fn into_parts(
        self,
    ) -> (
        PreparationTicket,
        WorkerPreparationOutcome<
            Source,
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
    ) {
        (self.ticket, self.outcome)
    }

    pub(in super::super) fn accepts(&self, expected: &PreparationTicket) -> bool {
        expected.matches(&self.ticket)
    }
}

fn advance_preparation<Source, Role, Worker, Plan>(
    ticket: PreparationTicket,
    source: Source,
    mut prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
    current: RoleName<Role>,
    remaining: Vec<RoleName<Role>>,
    submission: WorkerSubmission<Worker, Plan>,
) -> ControlFlow<
    WorkerPreparation<Source, Role, Worker, Plan>,
    PendingWorkerPreparation<Source, Role, Worker, Plan>,
>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    prepared.push(PreparedWorker {
        role: current,
        submission,
    });
    let mut remaining = remaining.into_iter();
    match remaining.next() {
        None => ControlFlow::Break(WorkerPreparation {
            ticket,
            outcome: WorkerPreparationOutcome::Prepared {
                source,
                members: prepared,
            },
        }),
        Some(current) => ControlFlow::Continue(PendingWorkerPreparation {
            ticket,
            source,
            prepared,
            current,
            remaining: remaining.collect(),
        }),
    }
}

fn rejected_preparation<Source, Role, Worker, Plan>(
    ticket: PreparationTicket,
    source: Source,
    prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
    failed_role: RoleName<Role>,
    reason: Source::WorkerRejection,
    remaining: Vec<RoleName<Role>>,
) -> WorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    WorkerPreparation {
        ticket,
        outcome: WorkerPreparationOutcome::WorkerRejected {
            source,
            prepared,
            failed_role,
            reason,
            remaining,
        },
    }
}

impl<Source, Role, Worker, Plan> ActionItem for PrepareWorkers<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;

    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }

    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        match custody {
            (input @ Some(_), received @ None) => Some((input, received)),
            _ => None,
        }
    }

    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = WorkerPreparationStarted;
    type Rejection = Never;
    type Prerequisite = Never;
}

impl<Source, Role, Worker, Plan> SourceAction for PrepareWorkers<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    type Source = Self;
}

#[cfg(test)]
mod tests {
    use core::ops::ControlFlow;
    use core::ptr;
    use std::sync::Arc;

    use behavior::{
        ActionItemResult, ActiveTurn, Behavior, BehaviorActed, MailAddr, MessageProtocol, Never,
        NoBirths, NoSends, SettledItem, User,
    };

    use super::{PrepareWorkers, WorkerPreparationOutcome, WorkerSource};
    use crate::ActivationPlan;
    use crate::WorkerSubmission;
    use crate::atomic::RoleName;

    #[derive(Debug, Eq, PartialEq)]
    enum Role {
        Api,
        Storage,
        Search,
        Queue,
        Index,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Worker(u8);

    impl Behavior for Worker {
        type Protocol = MessageProtocol<MailAddr, Never>;
        type Event = User<MailAddr, Never>;
        type Sends = NoSends;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Plan(u8);

    impl ActivationPlan for Plan {
        type Ready = ();
        type Rejection = Never;

        fn activate(
            self,
        ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send
        {
            core::future::ready(Ok(()))
        }
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Source(u8);

    #[derive(Debug, Eq, PartialEq)]
    enum WorkerRejection {
        Unsupported,
    }

    #[derive(Debug, Eq, PartialEq)]
    enum SourceRejection {
        Unavailable,
    }

    impl WorkerSource<Role, Worker, Plan> for Source {
        type WorkerRejection = WorkerRejection;
        type SourceRejection = SourceRejection;
    }

    type Request = PrepareWorkers<Source, Role, Worker, Plan>;

    fn submission(worker: u8) -> WorkerSubmission<Worker, Plan> {
        WorkerSubmission::activated(Worker(worker), Plan(worker))
    }

    #[test]
    fn two_role_request_advances_once_then_completes() {
        let api = RoleName::new(Role::Api);
        let storage = RoleName::new(Role::Storage);
        let api_name = api.clone();
        let storage_name = storage.clone();
        let (ticket, request) = Request::new(Source(3), api_name, vec![storage_name]);
        let (_started, mut starting) = request.start();
        let (source, role) = starting.source_and_role();
        assert_eq!(source, &mut Source(3));
        assert!(core::ptr::eq(role, api.role()));

        let ControlFlow::Continue(mut pending) = starting.accept(submission(11)) else {
            panic!("one remaining role cannot complete preparation");
        };
        let (source, role) = pending.source_and_role();
        assert_eq!(source, &mut Source(3));
        assert!(core::ptr::eq(role, storage.role()));
        let ControlFlow::Break(preparation) = pending.accept(submission(13)) else {
            panic!("the final role completes preparation");
        };
        let (returned_ticket, outcome) = preparation.into_parts();
        assert!(ticket.matches(&returned_ticket));
        let WorkerPreparationOutcome::Prepared { source, members } = outcome else {
            panic!("accepted roles produce the prepared outcome");
        };

        assert_eq!(source, Source(3));
        assert_eq!(members.len(), 2);
        assert!(core::ptr::eq(members[0].role.role(), api.role()));
        assert!(core::ptr::eq(members[1].role.role(), storage.role()));
    }

    #[test]
    fn rejection_derives_failed_name_and_untouched_suffix() {
        let api = RoleName::new(Role::Api);
        let storage = RoleName::new(Role::Storage);
        let later = RoleName::new(Role::Api);
        let (ticket, request) =
            Request::new(Source(7), api.clone(), vec![storage.clone(), later.clone()]);
        let (_started, starting) = request.start();
        let ControlFlow::Continue(progress) = starting.accept(submission(17)) else {
            panic!("two names remain after the first submission");
        };
        let preparation = progress.reject(WorkerRejection::Unsupported);
        let (returned_ticket, outcome) = preparation.into_parts();
        assert!(ticket.matches(&returned_ticket));
        let WorkerPreparationOutcome::WorkerRejected {
            source,
            prepared,
            failed_role,
            reason,
            remaining,
        } = outcome
        else {
            panic!("rejected construction must select the worker rejection outcome");
        };

        assert_eq!(source, Source(7));
        assert_eq!(prepared.len(), 1);
        assert!(core::ptr::eq(prepared[0].role.role(), api.role()));
        assert!(core::ptr::eq(failed_role.role(), storage.role()));
        assert_eq!(reason, WorkerRejection::Unsupported);
        assert_eq!(remaining.len(), 1);
        assert!(core::ptr::eq(remaining[0].role(), later.role()));
    }

    #[test]
    fn source_rejection_after_start_and_no_attempt_return_the_complete_source() {
        let api = RoleName::new(Role::Api);
        let storage = RoleName::new(Role::Storage);
        let (rejected_ticket, rejected_request) =
            Request::new(Source(19), api.clone(), vec![storage.clone()]);
        let (_started, mut starting) = rejected_request.start();
        let (source, role) = starting.source_and_role();
        assert_eq!(source, &mut Source(19));
        assert!(core::ptr::eq(role, api.role()));
        let rejected = starting.reject_source(SourceRejection::Unavailable);
        assert!(rejected.accepts(&rejected_ticket));
        let (_, outcome) = rejected.into_parts();
        let WorkerPreparationOutcome::SourceRejected { source, reason, .. } = outcome else {
            panic!("late source rejection returns the exact source");
        };
        assert_eq!(source, Source(19));
        assert_eq!(reason, SourceRejection::Unavailable);

        let (unattempted_ticket, unattempted_request) =
            Request::new(Source(23), storage.clone(), Vec::new());
        let unattempted: ActionItemResult<Request> = SettledItem::Unattempted(unattempted_request);
        let SettledItem::Unattempted(item) = unattempted else {
            panic!("no-attempt must retain the complete request");
        };
        assert!(item.accepts(&unattempted_ticket));
        let (foreign_ticket, _) = Request::new(Source(29), api, Vec::new());
        assert!(!item.accepts(&foreign_ticket));
        let (source, role, remaining) = item.into_parts();
        assert_eq!(source, Source(23));
        assert!(core::ptr::eq(role.role(), storage.role()));
        assert!(remaining.is_empty());
    }

    #[test]
    fn pending_observation_preserves_original_workers_roles_and_ticket() {
        for roles in [
            [
                Role::Api,
                Role::Storage,
                Role::Search,
                Role::Queue,
                Role::Index,
            ],
            [
                Role::Storage,
                Role::Api,
                Role::Index,
                Role::Search,
                Role::Queue,
            ],
        ] {
            let [first, second, current, later, last] = roles;
            let first = RoleName::new(first);
            let second = RoleName::new(second);
            let current = RoleName::new(current);
            let later = RoleName::new(later);
            let last = RoleName::new(last);
            let first_role = first.clone().into_role();
            let second_role = second.clone().into_role();
            let current_role = current.clone().into_role();
            let later_role = later.clone().into_role();
            let last_role = last.clone().into_role();
            let (issued, request) =
                Request::new(Source(31), first, vec![second, current, later, last]);
            let ticket_allocation = Arc::downgrade(&issued.token);
            let (started, starting) = request.start();
            let ControlFlow::Continue(pending) = starting.accept(submission(11)) else {
                panic!("four remaining roles cannot complete preparation");
            };
            let ControlFlow::Continue(mut pending) = pending.accept(submission(13)) else {
                panic!("three remaining roles cannot complete preparation");
            };
            let prepared: Vec<_> = pending
                .prepared_workers()
                .map(|(role, worker, plan)| (ptr::from_ref(role), worker.0, plan.0))
                .collect();
            let expected_prepared = [
                (Arc::as_ptr(&first_role), 11, 11),
                (Arc::as_ptr(&second_role), 13, 13),
            ];
            assert_eq!(prepared, expected_prepared);
            let remaining: Vec<_> = pending.remaining_roles().map(ptr::from_ref).collect();
            let expected_remaining = [Arc::as_ptr(&later_role), Arc::as_ptr(&last_role)];
            assert_eq!(remaining, expected_remaining);
            let source_current = {
                let (source, role) = pending.source_and_role();
                (source.0, ptr::from_ref(role))
            };
            assert_eq!(source_current, (31, Arc::as_ptr(&current_role)));
            assert!(started.accepts(&pending.ticket));
            assert!(issued.matches(&pending.ticket));
            assert_eq!(ticket_allocation.strong_count(), 3);
            drop((pending, started, issued));
            assert_eq!(ticket_allocation.strong_count(), 0);
            let role_counts = [
                Arc::strong_count(&first_role),
                Arc::strong_count(&second_role),
                Arc::strong_count(&current_role),
                Arc::strong_count(&later_role),
                Arc::strong_count(&last_role),
            ];
            assert_eq!(role_counts, [1, 1, 1, 1, 1]);
        }
    }
}
