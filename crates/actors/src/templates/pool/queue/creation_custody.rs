#[path = "../../../../tests/installed_control/mod.rs"]
mod installed_control;
use crate::diagnostic::{DiagnosticAction, DiagnosticDisposition};
use crate::lifecycle::ActorDrainPolicy;
use crate::pool::queue;
use crate::pool::queue::{FifoCommand, FifoOutcome};
use crate::pool::{
    Assignment, BacklogCapacity, Completion, Interruption, PoolFailureReaction, PoolRecovery,
    SubmissionId,
};
use crate::worker::{
    OrderedRoles, StartupLimit, StartupPlan, WorkerCreationRejection, WorkerInitializationOutcome,
    WorkerSubmission,
};
use crate::{Activate as _, ChildStopped, Exit, StopOnShutdown};
use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase, ChildCreationOutcome,
    ChildHead, CreationId, CreationKind, CreationSequence, CreationSettlement, CreationsSettled,
    EndpointAddress, InterpreterRequests, ItemSettlement, MessageProtocol, Never, NoBirths,
    Protocol, Recipient, ReportToParent, SettledItem, Step, User,
};
use std::convert::Infallible;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Instant;

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
    type Installed<B>
        = installed_control::InstalledControl<B, Endpoint>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;
    fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        *installed.endpoint()
    }
}
#[derive(Debug)]
struct SearchWorker;
impl Protocol for SearchWorker {
    type Addr = RuntimeAddr;
    type Msg = Assignment<u8>;
}
impl BehaviorBase for SearchWorker {
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl Behavior for SearchWorker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Assignment<u8>>;
    type Sends = InterpreterRequests<ReportToParent<Completion<u16>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: ActiveTurn, assignment: Self::Event) -> BehaviorActed<Self> {
        let result = u16::from(*assignment.message.payload());
        Ok(Actions::cont().with_send(assignment.message.complete(result)))
    }
}
#[derive(Debug)]
struct Hydrate {
    value: Box<u8>,
    retired: Arc<AtomicUsize>,
}
impl Drop for Hydrate {
    fn drop(&mut self) {
        self.retired.fetch_add(1, Ordering::SeqCst);
    }
}
impl StartupPlan for Hydrate {
    type Ready = u8;
    type Rejection = Never;
    fn start(self) -> impl core::future::Future<Output = Result<u8, Never>> + Send {
        async move { Ok(*self.value + 1) }
    }
}
fn unavailable<C>(id: CreationId, kind: CreationKind) -> CreationsSettled<RuntimeAddr, C>
where
    C: Behavior<Protocol: Protocol<Addr = RuntimeAddr>>,
{
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::<C, ChildHead>::Unavailable { id, kind },
        ))]
        .into_iter()
        .collect(),
    ))
}
#[derive(Debug, Eq, PartialEq)]
enum Role {
    Search,
    Index,
}
#[tokio::test]
async fn fifo_unavailable_diagnostic_preserves_exact_receipt_startup_and_prior_stop() {
    for reaction in [
        PoolFailureReaction::RetireRole,
        PoolFailureReaction::StopPool,
    ] {
        let retired = Arc::new(AtomicUsize::new(0));
        let plans = retired.clone();
        let original_plan = Arc::new(AtomicUsize::new(0));
        let issued_plan = original_plan.clone();
        let declared = queue::new(
            move |role: &Role| {
                let value = Box::new(match role {
                    Role::Search => 1,
                    Role::Index => 2,
                });
                if *role == Role::Search {
                    issued_plan.store((&*value as *const u8) as usize, Ordering::SeqCst);
                }
                Ok::<_, Never>(WorkerSubmission::with_startup(
                    SearchWorker,
                    Hydrate {
                        value,
                        retired: plans.clone(),
                    },
                ))
            },
            OrderedRoles::new(Role::Search, [Role::Index]).unwrap(),
            queue::Config {
                startup: StartupLimit::new(2).unwrap(),
                recovery: PoolRecovery::temporary(reaction),
                backlog: BacklogCapacity::new(4),
                interruption: Interruption::Fail,
                shutdown: ActorDrainPolicy::WaitForActorGraph,
                diagnostics: DiagnosticDisposition::<Infallible>::terminate(),
            },
        )
        .logical_replies()
        .build()
        .unwrap_or_else(|_| panic!("valid role declarations construct FIFO"));
        let initialized = declared.initialize().unwrap();
        let mut pool = initialized.behavior;
        let mut creations = initialized.actions.creates.into_iter();
        let failed = creations.next().unwrap();
        let (id, _, kind) = failed.into_parts();
        let sibling = creations.next().unwrap();
        let (sibling_id, _, sibling_kind) = sibling.into_parts();
        let committed = pool
            .on(CreationsSettled::new(CreationSettlement::Settled(
                [SettledItem::Attempted(ItemSettlement::Accepted(
                    ChildCreationOutcome::<StopOnShutdown<SearchWorker>, ChildHead>::Established(
                        behavior::CommittedChild::new(
                            sibling_id,
                            sibling_kind,
                            behavior::EstablishedActor::issued(
                                installed_control::InstalledControl::new(Endpoint(72)),
                            ),
                        ),
                    ),
                ))]
                .into_iter()
                .collect(),
            )))
            .unwrap();
        let init = committed
            .sends
            .worker_initializations
            .into_requests()
            .pop()
            .unwrap();
        let activating = pool
            .on(init.resolve(WorkerInitializationOutcome::ReadyForStartup))
            .unwrap();
        let startup = activating
            .sends
            .worker_startups
            .into_requests()
            .pop()
            .unwrap();
        let started = pool.on(startup.started()).unwrap();
        let ready = pool.on(startup.start().await).unwrap();
        for actions in [started, ready] {
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
            assert!(actions.sends.worker_observations.is_empty());
            assert!(actions.sends.worker_initializations.is_empty());
            assert!(actions.sends.worker_startups.is_empty());
            assert!(actions.sends.customer_outcomes.as_slice().is_empty());
            assert!(actions.sends.worker_assignments.is_empty());
            assert!(actions.sends.worker_preparations.is_empty());
            assert!(actions.sends.restart_schedules.is_empty());
            assert!(actions.sends.worker_shutdowns.is_empty());
            assert!(actions.sends.diagnostics.is_empty());
        }
        assert_eq!(retired.load(Ordering::SeqCst), 1);
        let mut foreign_ids = CreationSequence::new();
        let _ = foreign_ids.issue().unwrap();
        let _ = foreign_ids.issue().unwrap();
        let foreign = foreign_ids.issue().unwrap();
        let unrelated = pool
            .on(unavailable::<StopOnShutdown<SearchWorker>>(foreign, kind))
            .unwrap();
        assert!(unrelated.sends.worker_shutdowns.is_empty());
        assert!(unrelated.sends.worker_preparations.is_empty());
        let original = match unrelated.sends.diagnostics.into_requests().pop().unwrap() {
            DiagnosticAction::Terminal { diagnostic } => diagnostic,
            DiagnosticAction::Deliver { .. } => panic!("terminal policy retains foreign input"),
        };
        let super::FifoDiagnosticCause::Unexpected(
            crate::pool::queue::FifoEvent::WorkerCreationsSettled(original),
        ) = original.cause
        else {
            panic!("foreign input preserves its complete original event")
        };
        let CreationSettlement::Settled(rows) = original.into_settlement() else {
            panic!("foreign receipt remains accepted")
        };
        let row = rows.into_iter().next().unwrap();
        assert!(
            matches!(row,SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Unavailable{id,kind:returned_kind})) if id==foreign && returned_kind==kind)
        );
        assert_eq!(retired.load(Ordering::SeqCst), 1);
        let at = Instant::now();
        let waiting = pool
            .on(ChildStopped::new(id, Ok(Exit::Normal), at))
            .unwrap();
        assert!(waiting.sends.diagnostics.is_empty());
        let returned = pool
            .on(unavailable::<StopOnShutdown<SearchWorker>>(id, kind))
            .unwrap();
        assert!(returned.creates.is_empty());
        assert!(returned.sends.worker_preparations.is_empty());
        assert!(returned.sends.restart_schedules.is_empty());
        assert!(returned.sends.worker_startups.is_empty());
        assert_eq!(retired.load(Ordering::SeqCst), 1);
        let diagnostic = match returned.sends.diagnostics.into_requests().pop().unwrap() {
            DiagnosticAction::Terminal { diagnostic } => diagnostic,
            DiagnosticAction::Deliver { .. } => panic!("terminal policy retains diagnostic"),
        };
        assert_eq!(diagnostic.role(), Some(&Role::Search));
        match &diagnostic.cause {
            super::FifoDiagnosticCause::WorkerCreationRejected {
                rejection:
                    WorkerCreationRejection::Unavailable {
                        id: returned_id,
                        kind: returned_kind,
                    },
                startup,
                stopped: Some(stopped),
                role,
            } => {
                assert_eq!(*returned_id, id);
                assert_eq!(*returned_kind, kind);
                assert_eq!(*startup.value, 1);
                assert_eq!(
                    (&*startup.value as *const u8) as usize,
                    original_plan.load(Ordering::SeqCst)
                );
                assert_eq!(role.role(), &Role::Search);
                assert_eq!(stopped.child, id);
                assert_eq!(stopped.outcome, Ok(Exit::Normal));
                assert_eq!(stopped.at, at);
            }
            _ => panic!("exact unavailable receipt, startup and prior stop remain together"),
        }
        drop(diagnostic);
        assert_eq!(retired.load(Ordering::SeqCst), 2);
        match reaction {
            PoolFailureReaction::RetireRole => {
                assert!(returned.sends.worker_shutdowns.is_empty());
                assert!(matches!(returned.become_, Step::Continue));
                let accepted=pool.receive(RuntimeAddr(7),FifoCommand::submit(SubmissionId::new(9),55,Recipient::<MessageProtocol<RuntimeAddr,FifoOutcome<Role,u8,u16>>>::global(RuntimeAddr(90)))).unwrap();
                assert_eq!(accepted.sends.worker_assignments.len(), 1);
            }
            PoolFailureReaction::StopPool => {
                assert_eq!(returned.sends.worker_shutdowns.len(), 1);
                assert!(matches!(returned.become_, Step::Continue));
            }
        }
        let replay = pool
            .on(unavailable::<StopOnShutdown<SearchWorker>>(id, kind))
            .unwrap();
        assert!(!replay.sends.diagnostics.is_empty());
        assert!(replay.sends.worker_preparations.is_empty());
        assert!(replay.creates.is_empty());
        let diagnostic = match replay.sends.diagnostics.into_requests().pop().unwrap() {
            DiagnosticAction::Terminal { diagnostic } => diagnostic,
            DiagnosticAction::Deliver { .. } => panic!("terminal policy retains replay"),
        };
        let super::FifoDiagnosticCause::Unexpected(
            crate::pool::queue::FifoEvent::WorkerCreationsSettled(original),
        ) = diagnostic.cause
        else {
            panic!("replay preserves its complete original event")
        };
        let CreationSettlement::Settled(rows) = original.into_settlement() else {
            panic!("replayed receipt remains accepted")
        };
        let row = rows.into_iter().next().unwrap();
        assert!(
            matches!(row,SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Unavailable{id:returned_id,kind:returned_kind})) if returned_id==id && returned_kind==kind)
        );
    }
}
