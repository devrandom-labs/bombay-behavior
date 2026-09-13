//! Typed worker preparation outside the pure supervisor transition.

use core::marker::PhantomData;
use core::ops::ControlFlow;
use std::sync::Arc;

use behavior::{ActionItem, Behavior, ItemSettlement, Never, SettledItem, SourceAction};

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
    /// Exact rejection before the complete source action is accepted.
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
#[doc(hidden)]
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

    pub(in super::super) fn into_parts(self) -> (Source, RoleName<Role>, Vec<RoleName<Role>>) {
        (self.source, self.first, self.remaining)
    }

    /// Borrow the source and current application role for one Bombay-owned
    /// preparation attempt.
    #[doc(hidden)]
    #[must_use]
    pub fn source_and_role(&mut self) -> (&mut Source, &Role) {
        (&mut self.source, self.first.role())
    }

    /// Accept one submission for the current role.
    #[doc(hidden)]
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

    /// Return an exact rejection for the current role.
    #[doc(hidden)]
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
}

pub(in super::super) fn preparation_result_accepts<Source, Role, Worker, Plan>(
    result: &SettledItem<
        PrepareWorkers<Source, Role, Worker, Plan>,
        ItemSettlement<
            PrepareWorkers<Source, Role, Worker, Plan>,
            WorkerPreparation<Source, Role, Worker, Plan>,
            Source::SourceRejection,
            Never,
        >,
    >,
    expected: &PreparationTicket,
) -> bool
where
    Source: WorkerSource<Role, Worker, Plan>,
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    match result {
        SettledItem::Attempted(ItemSettlement::Accepted(preparation)) => {
            preparation.accepts(expected)
        }
        SettledItem::Attempted(ItemSettlement::Rejected { item, .. })
        | SettledItem::Attempted(ItemSettlement::Blocked { item, .. })
        | SettledItem::Attempted(ItemSettlement::Corrupt { item, .. })
        | SettledItem::Unattempted(item) => item.accepts(expected),
    }
}

pub(in super::super) enum WorkerPreparationOutcome<Source, Role, Worker, Plan, Rejection> {
    Prepared {
        source: Source,
        members: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
    },
    WorkerRejected {
        source: Source,
        prepared: Vec<PreparedWorker<RoleName<Role>, Worker, Plan>>,
        failed_role: RoleName<Role>,
        reason: Rejection,
        remaining: Vec<RoleName<Role>>,
    },
}

/// Non-empty remainder of one exact worker-preparation request.
#[doc(hidden)]
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
    #[doc(hidden)]
    #[must_use]
    pub fn source_and_role(&mut self) -> (&mut Source, &Role) {
        (&mut self.source, self.current.role())
    }

    /// Accept one submission for the current role.
    #[doc(hidden)]
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
    #[doc(hidden)]
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

/// Complete accepted result of one worker-source action.
#[doc(hidden)]
#[must_use = "worker preparation must return to its supervisor or retire outward"]
pub struct WorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    ticket: PreparationTicket,
    outcome: WorkerPreparationOutcome<Source, Role, Worker, Plan, Source::WorkerRejection>,
}

impl<Source, Role, Worker, Plan> WorkerPreparation<Source, Role, Worker, Plan>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
{
    pub(in super::super) fn from_parts(
        ticket: PreparationTicket,
        outcome: WorkerPreparationOutcome<Source, Role, Worker, Plan, Source::WorkerRejection>,
    ) -> Self {
        Self { ticket, outcome }
    }

    pub(in super::super) fn into_parts(
        self,
    ) -> (
        PreparationTicket,
        WorkerPreparationOutcome<Source, Role, Worker, Plan, Source::WorkerRejection>,
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
    type Accepted = WorkerPreparation<Source, Role, Worker, Plan>;
    type Rejection = Source::SourceRejection;
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

    use behavior::{
        ActionItemResult, ActiveTurn, Behavior, BehaviorActed, ItemSettlement, MailAddr,
        MessageProtocol, Never, NoBirths, NoSends, SettledItem, User,
    };

    use super::{PrepareWorkers, WorkerPreparationOutcome, WorkerSource};
    use crate::ActivationPlan;
    use crate::WorkerSubmission;
    use crate::atomic::RoleName;

    #[derive(Debug, Eq, PartialEq)]
    enum Role {
        Api,
        Storage,
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
        let (ticket, mut request) = Request::new(Source(3), api_name, vec![storage_name]);
        let (source, role) = request.source_and_role();
        assert_eq!(source, &mut Source(3));
        assert!(core::ptr::eq(role, api.role()));

        let ControlFlow::Continue(mut pending) = request.accept(submission(11)) else {
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
        let ControlFlow::Continue(progress) = request.accept(submission(17)) else {
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
    fn source_rejection_and_no_attempt_return_the_complete_request() {
        let api = RoleName::new(Role::Api);
        let storage = RoleName::new(Role::Storage);
        let (rejected_ticket, rejected_request) =
            Request::new(Source(19), api.clone(), vec![storage.clone()]);
        let rejected: ActionItemResult<Request> =
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: rejected_request,
                reason: SourceRejection::Unavailable,
            });
        let SettledItem::Attempted(ItemSettlement::Rejected { mut item, reason }) = rejected else {
            panic!("source rejection must remain the generic rejected settlement");
        };
        let (source, role) = item.source_and_role();
        assert_eq!(source, &mut Source(19));
        assert!(core::ptr::eq(role, api.role()));
        assert!(item.accepts(&rejected_ticket));
        assert_eq!(reason, SourceRejection::Unavailable);

        let (unattempted_ticket, unattempted_request) =
            Request::new(Source(23), storage.clone(), Vec::new());
        let unattempted: ActionItemResult<Request> = SettledItem::Unattempted(unattempted_request);
        let SettledItem::Unattempted(mut item) = unattempted else {
            panic!("no-attempt must retain the complete request");
        };
        let (source, role) = item.source_and_role();
        assert_eq!(source, &mut Source(23));
        assert!(core::ptr::eq(role, storage.role()));
        assert!(item.accepts(&unattempted_ticket));

        let (foreign_ticket, _) = Request::new(Source(29), api, Vec::new());
        assert!(!item.accepts(&foreign_ticket));
    }
}
