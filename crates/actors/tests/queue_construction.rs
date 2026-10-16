use behavior::{Actions, Address, CreationKind, EndpointAddress, Protocol};
use std::cell::Cell;
mod installed_control;
use behavior_actors::Activate as _;
use behavior_actors::diagnostic::DiagnosticDisposition;
use behavior_actors::lifecycle::ActorDrainPolicy;
use behavior_actors::pool::queue;
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

#[test]
fn initial_workers_preserve_roster_order_and_fresh_creation() {
    let prepared = Cell::new(0_usize);
    let declaration = queue::new(
        |_| {
            prepared.set(prepared.get() + 1);
            WorkerSubmission::immediate(SearchWorker)
        },
        OrderedRoles::new(SearchRole::Primary, [SearchRole::Reserve])
            .expect("two distinct search roles"),
        queue::Config {
            startup: StartupLimit::new(1).expect("one activation"),
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(4),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::terminate(),
        },
    );
    assert_eq!(prepared.get(), 0);
    let pool = declaration
        .build()
        .unwrap_or_else(|_| panic!("both workers are prepared"));
    assert_eq!(prepared.get(), 2);
    let initialized = pool
        .initialize()
        .unwrap_or_else(|_| panic!("initialization emits the complete creation batch"));
    assert_eq!(initialized.actions.creates.len(), 2);
    assert_eq!(initialized.actions.sends.worker_observations.len(), 2);
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
}

#[test]
fn preparation_refusal_returns_all_workers_and_original_configuration() {
    let route = Box::new(vec![173_u8, 179]);
    let route_address = core::ptr::from_ref(route.as_ref());
    let activation = StartupLimit::new(2).expect("two activations");
    let mut rejection = match queue::new(
        |role| match role {
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
        queue::Config {
            startup: activation,
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(4),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::deliver_to(route),
        },
    )
    .build()
    {
        Ok(_) => panic!("secondary preparation must return its refusal"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.workers.prepared.len(), 1);
    assert_eq!(rejection.workers.prepared[0].role, SearchRole::Primary);
    assert_eq!(
        rejection.workers.prepared[0].submission,
        WorkerSubmission::immediate(SearchWorker)
    );
    assert_eq!(rejection.workers.role, SearchRole::Secondary);
    assert_eq!(*rejection.workers.reason, [181_u8, 191]);
    assert_eq!(rejection.workers.remaining, [SearchRole::Reserve]);
    assert_eq!(rejection.config.startup, activation);
    assert_eq!(
        rejection.config.recovery,
        PoolRecovery::temporary(PoolFailureReaction::RetireRole)
    );
    assert_eq!(rejection.config.backlog, BacklogCapacity::new(4));
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
