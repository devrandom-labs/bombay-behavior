mod installed_control;

use core::time::Duration;
use std::cell::Cell;
use std::rc::Rc;

use behavior::{
    ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, ChildInputReason,
    CreationId, CreationKind, CreationSettlement, CreationsSettled, EndpointAddress,
    EstablishedActor, ItemSettlement, MessageProtocol, Never, NoBirths, NoSends, Protocol,
    SettledItem, User,
};
use behavior_actors::atomic::{
    ActivationPlan, ActivationPolicy, ActorDrainPolicy, DiagnosticDisposition, FailureReaction,
    OrderedRoles, ProxyControl, ProxyControlAdmission, ProxyPhase, Recovery, RestartLimit,
    RestartRelease, StableProxy, Strategy, WorkerSource, WorkerSubmission, fixed,
};
use behavior_actors::{Activate, Active};

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
        =
        installed_control::InstalledControl<B, <Self as EndpointAddress>::Established<B::Protocol>>
    where
        B: behavior::Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: behavior::Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
}

#[derive(Debug, Eq, PartialEq)]
enum SearchRole {
    Search,
    Index,
    Spellcheck,
}

#[derive(Debug, Eq, PartialEq)]
enum SearchWorker {
    Search,
    Index,
    Spellcheck,
}

impl Behavior for SearchWorker {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SearchActivation(SearchWorker);

impl ActivationPlan for SearchActivation {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

#[derive(Debug, Eq, PartialEq)]
enum PreparationRejection {
    IndexUnavailable,
}

#[derive(Debug, Eq, PartialEq)]
struct DiagnosticRoute;

#[derive(Debug, Eq, PartialEq)]
struct LifecycleRoute;

#[derive(Debug, Eq, PartialEq)]
struct SearchWorkerSource {
    authority: u8,
}

impl WorkerSource<SearchRole, SearchWorker, SearchActivation> for SearchWorkerSource {
    type WorkerRejection = PreparationRejection;
    type SourceRejection = Never;
}

fn worker_for(role: &SearchRole) -> SearchWorker {
    match role {
        SearchRole::Search => SearchWorker::Search,
        SearchRole::Index => SearchWorker::Index,
        SearchRole::Spellcheck => SearchWorker::Spellcheck,
    }
}

fn submission_for(role: &SearchRole) -> WorkerSubmission<SearchWorker, SearchActivation> {
    let worker = worker_for(role);
    WorkerSubmission::activated(worker, SearchActivation(worker_for(role)))
}

fn restart_limit() -> RestartLimit {
    RestartLimit::new(3, Duration::from_secs(60))
}

fn recovery() -> Recovery<SearchWorkerSource> {
    Recovery::permanent(
        SearchWorkerSource { authority: 17 },
        Strategy::RestForOne,
        restart_limit(),
        RestartRelease::linear(Duration::from_secs(2), Duration::from_secs(30))
            .expect("release durations are ordered and positive"),
    )
}

#[test]
fn canonical_construction_prepares_every_declared_role() {
    let prepared = Rc::new(Cell::new(0_usize));
    let observed = Rc::clone(&prepared);
    let roles = OrderedRoles::new(
        SearchRole::Search,
        [SearchRole::Index, SearchRole::Spellcheck],
    )
    .expect("search roles are unique");
    let activation = ActivationPolicy::new(2).expect("activation capacity is positive");

    let _supervisor = fixed(
        move |role: &SearchRole| {
            observed.set(observed.get() + 1);
            Ok::<_, PreparationRejection>(submission_for(role))
        },
        roles,
        activation,
        recovery(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::deliver_to(DiagnosticRoute),
    )
    .publish_lifecycle(LifecycleRoute)
    .build()
    .unwrap_or_else(|_| panic!("every initial worker prepares"));

    assert_eq!(prepared.get(), 3);
}

#[test]
fn route_free_diagnostic_termination_needs_no_annotation() {
    let roles = OrderedRoles::new(SearchRole::Search, []).expect("one role is non-empty");

    let _supervisor = fixed(
        |role: &SearchRole| Ok::<_, PreparationRejection>(submission_for(role)),
        roles,
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::RetireMember,
        ActorDrainPolicy::RetireActorGraphAfter {
            deadline: Duration::ZERO,
        },
        DiagnosticDisposition::terminate(),
    )
    .build()
    .unwrap_or_else(|_| panic!("the worker prepares"));
}

#[test]
fn transient_recovery_infers_its_worker_source() {
    let roles = OrderedRoles::new(SearchRole::Search, []).expect("one role is non-empty");

    let _supervisor = fixed(
        |role: &SearchRole| Ok::<_, PreparationRejection>(submission_for(role)),
        roles,
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::transient(
            SearchWorkerSource { authority: 29 },
            Strategy::OneForOne,
            restart_limit(),
            RestartRelease::immediate(),
        ),
        FailureReaction::RetireMember,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build()
    .unwrap_or_else(|_| panic!("the worker prepares"));
}

struct SearchProxyHost {
    creation: CreationId,
    proxy: Active<StableProxy<SearchWorker, SearchActivation>>,
    worker_creations: usize,
}

impl ProxyControlAdmission<SearchWorker, SearchActivation> for SearchProxyHost {
    fn admit_proxy_control(
        &mut self,
        creation: CreationId,
        control: ProxyControl<SearchWorker, SearchActivation>,
    ) -> ItemSettlement<
        ProxyControl<SearchWorker, SearchActivation>,
        EstablishedActor<StableProxy<SearchWorker, SearchActivation>>,
        ChildInputReason,
        Never,
    > {
        if creation != self.creation {
            return ItemSettlement::Rejected {
                item: control,
                reason: ChildInputReason::MissingBinding,
            };
        }
        let actions = self
            .proxy
            .on(control)
            .unwrap_or_else(|_| panic!("the exact proxy accepts its initial control"));
        self.worker_creations += actions.creates.len();
        ItemSettlement::Accepted(EstablishedActor::issued(
            installed_control::InstalledControl::new(Endpoint(100)),
        ))
    }
}

#[test]
fn initialization_creates_and_admits_one_ordered_proxy_batch() {
    let roles = OrderedRoles::new(SearchRole::Search, [SearchRole::Index])
        .expect("search roles are unique");
    let supervisor = fixed(
        |role: &SearchRole| Ok::<_, PreparationRejection>(submission_for(role)),
        roles,
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        Recovery::temporary(),
        FailureReaction::RetireMember,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build()
    .unwrap_or_else(|_| panic!("both workers prepare"));
    let initialized = supervisor
        .initialize()
        .unwrap_or_else(|_| panic!("initialization emits one proxy batch"));
    assert_eq!(initialized.actions.creates.len(), 2);
    assert_eq!(initialized.actions.sends.proxy_observations.len(), 2);
    let creation_ids: Vec<_> = initialized
        .actions
        .creates
        .iter()
        .map(|creation| creation.id().get())
        .collect();
    assert_eq!(creation_ids, [1, 2]);
    assert!(
        initialized
            .actions
            .creates
            .iter()
            .all(|creation| creation.kind() == CreationKind::Birth
                && creation.child().phase() == ProxyPhase::Dormant)
    );
    let observed_ids: Vec<_> = initialized
        .actions
        .sends
        .proxy_observations
        .iter()
        .map(|observation| observation.child.get())
        .collect();
    assert_eq!(observed_ids, creation_ids);

    let settlements = initialized
        .actions
        .creates
        .into_iter()
        .enumerate()
        .map(|(position, creation)| {
            let (id, proxy, kind) = creation.into_parts();
            drop(proxy);
            SettledItem::Attempted(ItemSettlement::Accepted(ChildCreationOutcome::Established(
                behavior::CommittedChild::new(
                    id,
                    kind,
                    behavior::EstablishedActor::issued(installed_control::InstalledControl::new(
                        Endpoint(100 + position as u64),
                    )),
                ),
            )))
        })
        .collect();
    let mut supervisor = initialized.behavior;
    let actions = supervisor
        .on(CreationsSettled::new(CreationSettlement::Settled(
            settlements,
        )))
        .unwrap_or_else(|_| panic!("the exact ordered proxy batch is admitted"));

    assert_eq!(actions.sends.proxy_operations.len(), 1);
    assert_eq!(actions.creates.len(), 0);

    let operation = actions
        .sends
        .proxy_operations
        .into_items()
        .pop()
        .unwrap_or_else(|| panic!("the first proxy operation is issued"));
    let proxy = StableProxy::<SearchWorker, SearchActivation>::activated()
        .initialize()
        .unwrap_or_else(|_| panic!("proxy initialization is pure"))
        .behavior;
    let mut host = SearchProxyHost {
        creation: operation.creation(),
        proxy,
        worker_creations: 0,
    };
    let settlement = operation.settle(&mut host);
    assert!(matches!(settlement, ItemSettlement::Accepted(_)));
    assert_eq!(host.worker_creations, 1);
    let settled = supervisor
        .on(SettledItem::Attempted(settlement))
        .unwrap_or_else(|_| panic!("the fixed supervisor accepts its exact proxy receipt"));
    assert_eq!(settled.sends.proxy_operations.len(), 0);
}

#[test]
fn preparation_rejection_returns_every_owned_input() {
    let roles = OrderedRoles::new(
        SearchRole::Search,
        [SearchRole::Index, SearchRole::Spellcheck],
    )
    .expect("search roles are unique");
    let activation = ActivationPolicy::new(2).expect("activation capacity is positive");

    let rejection = match fixed(
        |role: &SearchRole| match role {
            SearchRole::Index => Err(PreparationRejection::IndexUnavailable),
            SearchRole::Search | SearchRole::Spellcheck => Ok(submission_for(role)),
        },
        roles,
        activation,
        recovery(),
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .build()
    {
        Ok(_) => panic!("index preparation must be rejected"),
        Err(rejection) => rejection,
    };

    assert_eq!(rejection.workers.prepared.len(), 1);
    assert_eq!(rejection.workers.prepared[0].role, SearchRole::Search);
    assert_eq!(
        rejection.workers.prepared[0].submission,
        WorkerSubmission::activated(SearchWorker::Search, SearchActivation(SearchWorker::Search))
    );
    assert_eq!(rejection.workers.role, SearchRole::Index);
    assert_eq!(
        rejection.workers.reason,
        PreparationRejection::IndexUnavailable
    );
    assert_eq!(rejection.workers.remaining, [SearchRole::Spellcheck]);
    assert_eq!(rejection.activation, activation);
    assert_eq!(rejection.recovery, recovery());
    assert_eq!(rejection.failure_reaction, FailureReaction::StopSupervisor);
    assert_eq!(rejection.actor_drain, ActorDrainPolicy::WaitForActorGraph);
    assert_eq!(rejection.diagnostics, DiagnosticDisposition::terminate());
    assert_eq!(rejection.lifecycle, None);
    let returned = (rejection.workers.factory)(&SearchRole::Spellcheck);
    assert_eq!(
        returned,
        Ok(WorkerSubmission::activated(
            SearchWorker::Spellcheck,
            SearchActivation(SearchWorker::Spellcheck)
        ))
    );
}
