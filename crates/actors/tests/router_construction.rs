use behavior::{Actions, Address, CreationKind, EndpointAddress, Protocol, Step};
mod installed_control;
use behavior_actors::Activate as _;
use behavior_actors::diagnostic::DiagnosticDisposition;
use behavior_actors::lifecycle::ActorDrainPolicy;
use behavior_actors::pool::router;
use behavior_actors::pool::router::BindingCapacity;
use behavior_actors::pool::{
    Assignment, BacklogCapacity, Interruption, PoolFailureReaction, PoolRecovery, pool_worker,
};
use behavior_actors::worker::{OrderedRoles, StartupLimit, WorkerSubmission};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchRole {
    Primary,
    Secondary,
    Reserve,
}

#[derive(Debug, Eq, PartialEq)]
struct SearchWorker;

#[pool_worker(addr = RuntimeAddr, result = u16)]
#[allow(
    clippy::unused_self,
    clippy::unnecessary_wraps,
    reason = "the worker answers assignments through the owning Behavior contract"
)]
impl SearchWorker {
    fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
        let result = u16::from(*assignment.payload());
        Ok(Actions::cont().with_send(assignment.complete(result)))
    }
}

fn select_role(key: &u64) -> SearchRole {
    match *key {
        7 => SearchRole::Primary,
        11 => SearchRole::Secondary,
        _ => SearchRole::Reserve,
    }
}

#[test]
fn initial_workers_preserve_roster_order_and_fresh_creation() {
    let pool = router::new(
        |_: &SearchRole| WorkerSubmission::immediate(SearchWorker),
        OrderedRoles::new(SearchRole::Primary, [SearchRole::Reserve])
            .expect("two distinct search roles"),
        select_role,
        router::Config {
            startup: StartupLimit::new(1).expect("one startup"),
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(4),
            bindings: BindingCapacity::new(3).expect("three retained bindings"),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::terminate(),
        },
    )
    .unwrap_or_else(|_| panic!("both workers are prepared"));
    let initialized = pool
        .initialize()
        .unwrap_or_else(|_| panic!("initialization emits the complete creation batch"));
    assert_eq!(initialized.actions.creates.len(), 2);
    assert_eq!(initialized.actions.sends.worker_observations.len(), 2);
    assert!(initialized.actions.sends.worker_initializations.is_empty());
    assert!(initialized.actions.sends.worker_startups.is_empty());
    assert!(initialized.actions.sends.customer_outcomes.is_empty());
    assert!(
        initialized
            .actions
            .sends
            .binding_replies
            .as_slice()
            .is_empty()
    );
    assert!(initialized.actions.sends.worker_assignments.is_empty());
    assert!(initialized.actions.sends.worker_preparations.is_empty());
    assert!(initialized.actions.sends.restart_schedules.is_empty());
    assert!(initialized.actions.sends.worker_shutdowns.is_empty());
    assert!(initialized.actions.sends.diagnostics.is_empty());
    assert_eq!(initialized.actions.become_, Step::Continue);
    assert!(
        initialized
            .actions
            .creates
            .iter()
            .all(|creation| creation.kind() == CreationKind::Birth)
    );
    let creations: Vec<_> = initialized
        .actions
        .creates
        .iter()
        .map(|creation| creation.id())
        .collect();
    let observations: Vec<_> = initialized
        .actions
        .sends
        .worker_observations
        .iter()
        .map(|observation| observation.child)
        .collect();
    assert_eq!(creations, observations);
    assert_ne!(creations[0], creations[1]);
}

#[test]
fn preparation_refusal_returns_all_workers_and_original_configuration() {
    let selector = Box::new(select_role as fn(&u64) -> SearchRole);
    let selector_address = core::ptr::from_ref(selector.as_ref());
    let route = Box::new(vec![173_u8, 179]);
    let route_address = core::ptr::from_ref(route.as_ref());
    let startup = StartupLimit::new(2).expect("two startups");
    let mut rejection = match router::new(
        |role: &SearchRole| match role {
            SearchRole::Primary | SearchRole::Reserve => {
                Ok(WorkerSubmission::immediate(SearchWorker))
            }
            SearchRole::Secondary => Err(Box::new(vec![181_u8, 191])),
        },
        OrderedRoles::new(
            SearchRole::Primary,
            [SearchRole::Secondary, SearchRole::Reserve],
        )
        .expect("three distinct search roles"),
        selector,
        router::Config {
            startup,
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(4),
            bindings: BindingCapacity::new(3).expect("three retained bindings"),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::deliver_to(route),
        },
    ) {
        Ok(_) => panic!("secondary preparation must return its refusal"),
        Err(rejection) => rejection,
    };
    assert_eq!(
        core::ptr::from_ref(rejection.selector.as_ref()),
        selector_address
    );
    let selected = (rejection.selector)(&11);
    assert_eq!(selected, SearchRole::Secondary);
    assert_eq!(rejection.workers.prepared.len(), 1);
    assert_eq!(rejection.workers.prepared[0].role, SearchRole::Primary);
    assert_eq!(
        rejection.workers.prepared[0].submission,
        WorkerSubmission::immediate(SearchWorker)
    );
    assert_eq!(rejection.workers.role, SearchRole::Secondary);
    assert_eq!(*rejection.workers.reason, [181_u8, 191]);
    assert_eq!(rejection.workers.remaining, [SearchRole::Reserve]);
    assert_eq!(rejection.config.startup, startup);
    assert_eq!(
        rejection.config.recovery,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole)
    );
    assert_eq!(rejection.config.backlog, BacklogCapacity::new(4));
    assert_eq!(
        rejection.config.bindings,
        BindingCapacity::new(3).expect("three retained bindings")
    );
    assert_eq!(rejection.config.interruption, Interruption::Retry);
    assert_eq!(
        rejection.config.shutdown,
        ActorDrainPolicy::WaitForActorGraph
    );
    let DiagnosticDisposition::DeliverTo(route) = rejection.config.diagnostics else {
        panic!("the complete owned diagnostic route is returned");
    };
    assert_eq!(core::ptr::from_ref(route.as_ref()), route_address);
    assert_eq!(*route, [173_u8, 179]);
    let returned = (rejection.workers.factory)(&SearchRole::Reserve);
    assert_eq!(returned, Ok(WorkerSubmission::immediate(SearchWorker)));
}
