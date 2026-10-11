mod installed_control;
use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorBase, ChildCreationOutcome,
    ChildHead, CreationId, CreationKind, CreationSequence, CreationSettlement, CreationsSettled,
    EndpointAddress, InterpreterRequests, ItemSettlement, MessageProtocol, Never, NoBirths,
    Protocol, Recipient, ReportToParent, SettledItem, Step, User,
};
use behavior_actors::atomic::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, Assignment, BacklogCapacity, Completion,
    DiagnosticAction, DiagnosticDisposition, FifoCommand, FifoOutcome, InitialWorkerOutcome,
    Interruption, OrderedRoles, PoolFailureReaction, PoolRecovery, ProxyControl, ProxyDiagnostic,
    ProxyOutcome, ProxyPhase, StableProxy, SubmissionId, WorkerCreationRejection,
    WorkerInitializationOutcome, WorkerStartResult, WorkerSubmission, fifo,
};
use behavior_actors::{Activate as _, ChildStopped, Exit, StopOnShutdown};
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
impl ActivationPlan for Hydrate {
    type Ready = u8;
    type Rejection = Never;
    fn activate(self) -> impl core::future::Future<Output = Result<u8, Never>> + Send {
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
fn assert_rejected(
    outcome: ProxyOutcome<SearchWorker, Hydrate>,
    id: CreationId,
    kind: CreationKind,
    pointer: *const u8,
    at: Instant,
) {
    match outcome {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result:
                        WorkerStartResult::CreationRejected {
                            rejection:
                                WorkerCreationRejection::Unavailable {
                                    id: returned,
                                    kind: returned_kind,
                                },
                            activation,
                            stopped: Some(stopped),
                        },
                },
        } => {
            assert_eq!(returned, id);
            assert_eq!(returned_kind, kind);
            assert_eq!(&*activation.value as *const u8, pointer);
            assert_eq!(stopped.child, id);
            assert_eq!(stopped.outcome, Ok(Exit::Normal));
            assert_eq!(stopped.at, at);
        }
        _ => panic!("complete original receipt, activation, and prior stop must coexist"),
    }
}
#[test]
fn proxy_unavailable_preserves_receipt_activation_prior_stop_and_replays() {
    let retired = Arc::new(AtomicUsize::new(0));
    let plan = Hydrate {
        value: Box::new(8),
        retired: retired.clone(),
    };
    let pointer = &*plan.value as *const u8;
    let mut proxy = StableProxy::<SearchWorker, Hydrate>::activated()
        .initialize()
        .unwrap()
        .behavior;
    let started = proxy
        .on(ProxyControl::start_with(SearchWorker, plan))
        .unwrap();
    let creation = started.creates.into_iter().next().unwrap();
    let (id, _, kind) = creation.into_parts();
    let mut sequence = CreationSequence::new();
    let _ = sequence.issue().unwrap();
    let foreign = sequence.issue().unwrap();
    let rejected = proxy
        .on(unavailable::<StopOnShutdown<SearchWorker>>(foreign, kind))
        .unwrap();
    assert_eq!(proxy.phase(), ProxyPhase::Creating);
    assert!(rejected.sends.owner_outcomes.is_empty());
    let diagnostic = rejected
        .sends
        .diagnostics
        .into_requests()
        .pop()
        .unwrap()
        .into_inner();
    let ProxyDiagnostic::UnexpectedWorkerStart { workers, .. } = diagnostic else {
        panic!("foreign receipt is returned")
    };
    let CreationSettlement::Settled(rows) = workers.into_settlement() else {
        panic!("accepted result remains accepted")
    };
    assert!(
        matches!(rows.into_iter().next().unwrap(),SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Unavailable{id,kind:returned})) if id==foreign && returned==kind)
    );
    let at = Instant::now();
    let waiting = proxy
        .on(ChildStopped::new(id, Ok(Exit::Normal), at))
        .unwrap();
    assert!(waiting.sends.owner_outcomes.is_empty());
    let returned = proxy
        .on(unavailable::<StopOnShutdown<SearchWorker>>(id, kind))
        .unwrap();
    assert_eq!(proxy.phase(), ProxyPhase::EmptyInitial);
    assert!(returned.creates.is_empty());
    assert!(returned.sends.worker_initializations.is_empty());
    assert!(returned.sends.worker_activations.is_empty());
    assert!(returned.sends.worker_shutdowns.is_empty());
    assert_eq!(retired.load(Ordering::SeqCst), 0);
    assert_rejected(
        returned
            .sends
            .owner_outcomes
            .into_requests()
            .pop()
            .unwrap()
            .into_inner(),
        id,
        kind,
        pointer,
        at,
    );
    assert_eq!(retired.load(Ordering::SeqCst), 1);
    let replay = proxy
        .on(unavailable::<StopOnShutdown<SearchWorker>>(id, kind))
        .unwrap();
    assert_eq!(proxy.phase(), ProxyPhase::EmptyInitial);
    assert!(replay.sends.owner_outcomes.is_empty());
    assert_eq!(replay.sends.diagnostics.len(), 1);
}
#[test]
fn shutdown_outside_proxy_preserves_unavailable_inner_lane() {
    let retired = Arc::new(AtomicUsize::new(0));
    let mut proxy = StopOnShutdown::new(StableProxy::<SearchWorker, Hydrate>::activated())
        .initialize()
        .unwrap()
        .behavior;
    let started = proxy
        .on_path(ProxyControl::start_with(
            SearchWorker,
            Hydrate {
                value: Box::new(9),
                retired: retired.clone(),
            },
        ))
        .unwrap();
    let creation = started.creates.into_iter().next().unwrap();
    let (id, _, kind) = creation.into_parts();
    let returned = proxy
        .on_path(unavailable::<StopOnShutdown<SearchWorker>>(id, kind))
        .unwrap();
    assert_eq!(proxy.base().phase(), ProxyPhase::EmptyInitial);
    assert_eq!(returned.sends.inner.owner_outcomes.len(), 1);
    assert_eq!(retired.load(Ordering::SeqCst), 0);
    drop(returned);
    assert_eq!(retired.load(Ordering::SeqCst), 1);
}
#[test]
fn shutdown_inside_proxy_preserves_unavailable_child_lane() {
    let retired = Arc::new(AtomicUsize::new(0));
    let mut proxy = StableProxy::<StopOnShutdown<SearchWorker>, Hydrate>::activated()
        .initialize()
        .unwrap()
        .behavior;
    let started = proxy
        .on(ProxyControl::start_with(
            StopOnShutdown::new(SearchWorker),
            Hydrate {
                value: Box::new(10),
                retired: retired.clone(),
            },
        ))
        .unwrap();
    let creation = started.creates.into_iter().next().unwrap();
    let (id, _, kind) = creation.into_parts();
    let returned = proxy
        .on(unavailable::<StopOnShutdown<StopOnShutdown<SearchWorker>>>(
            id, kind,
        ))
        .unwrap();
    assert_eq!(proxy.phase(), ProxyPhase::EmptyInitial);
    assert_eq!(returned.sends.owner_outcomes.len(), 1);
    assert_eq!(retired.load(Ordering::SeqCst), 0);
    drop(returned);
    assert_eq!(retired.load(Ordering::SeqCst), 1);
}
#[derive(Debug, Eq, PartialEq)]
enum Role {
    Search,
    Index,
}
#[tokio::test]
async fn fifo_unavailable_uses_only_configured_failure_reaction() {
    for reaction in [
        PoolFailureReaction::RetireRole,
        PoolFailureReaction::StopPool,
    ] {
        let retired = Arc::new(AtomicUsize::new(0));
        let plans = retired.clone();
        let declared = fifo(
            move |role: &Role| {
                Ok::<_, Never>(WorkerSubmission::activated(
                    SearchWorker,
                    Hydrate {
                        value: Box::new(match role {
                            Role::Search => 1,
                            Role::Index => 2,
                        }),
                        retired: plans.clone(),
                    },
                ))
            },
            OrderedRoles::new(Role::Search, [Role::Index]).unwrap(),
            ActivationPolicy::new(2).unwrap(),
            PoolRecovery::temporary(reaction),
            BacklogCapacity::new(4),
            Interruption::Fail,
            ActorDrainPolicy::WaitForActorGraph,
            DiagnosticDisposition::<Infallible>::terminate(),
        )
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
            .on(init.resolve(WorkerInitializationOutcome::ReadyForActivation))
            .unwrap();
        let activation = activating
            .sends
            .worker_activations
            .into_requests()
            .pop()
            .unwrap();
        let started = pool.on(activation.started()).unwrap();
        let ready = pool.on(activation.activate().await).unwrap();
        for actions in [started, ready] {
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
            assert!(actions.sends.worker_observations.is_empty());
            assert!(actions.sends.worker_initializations.is_empty());
            assert!(actions.sends.worker_activations.is_empty());
            assert!(actions.sends.customer_outcomes.as_slice().is_empty());
            assert!(actions.sends.worker_assignments.is_empty());
            assert!(actions.sends.worker_preparations.is_empty());
            assert!(actions.sends.restart_schedules.is_empty());
            assert!(actions.sends.worker_shutdowns.is_empty());
            assert!(actions.sends.diagnostics.is_empty());
        }
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
        assert!(returned.sends.worker_activations.is_empty());
        assert_eq!(retired.load(Ordering::SeqCst), 1);
        let diagnostic = match returned.sends.diagnostics.into_requests().pop().unwrap() {
            DiagnosticAction::Terminal { diagnostic } => diagnostic,
            DiagnosticAction::Deliver { .. } => panic!("terminal policy retains diagnostic"),
        };
        assert_eq!(diagnostic.role(), Some(&Role::Search));
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
    }
}

#[test]
fn unavailable_receipt_cannot_become_committed_authority() {
    let id = CreationSequence::new().issue().unwrap();
    let kind = CreationKind::Replacement { previous: id };
    let receipt =
        ChildCreationOutcome::<StopOnShutdown<SearchWorker>, ChildHead>::Unavailable { id, kind };
    let Err(ChildCreationOutcome::Unavailable {
        id: returned,
        kind: returned_kind,
    }) = receipt.into_committed()
    else {
        panic!("unavailability must return the complete original receipt");
    };
    assert_eq!(returned, id);
    assert_eq!(returned_kind, kind);
}
