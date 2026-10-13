//! Current one-role FixedSupervisor setup backed by its actual StableProxy.

use core::convert::Infallible;

use behavior_actors::diagnostic::DiagnosticDisposition;
use behavior_actors::lifecycle::ActorDrainPolicy;
use behavior_actors::proxy::{InitialWorkerOutcome, ProxyOutcome, StableProxy, WorkerStartResult};
use behavior_actors::supervisor::roster;
use behavior_actors::supervisor::roster::{FailureReaction, FixedSupervisor, Recovery};
use behavior_actors::worker::{WorkerSource};
use behavior_actors::worker::{
    ImmediateStartup, OrderedRoles, StartupLimit, WorkerAttempt, WorkerSubmission,
};
use behavior_actors::{Activate as _, Active};
use behavior_core::{
    ActionItemResult, ChildCreationOutcome, ChildReport, CreateChild, CreationId,
    CreationSettlement, Creations, CreationsSettled, EstablishedActor, ItemSettlement, SettledItem,
    Step,
};

use crate::proxy_control::admit_proxy_operation;
use crate::stable_proxy::{RuntimeAddress, Worker, WorkerEndpoint, drive_ready_proxy};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Search,
}

pub(crate) struct ReadySupervisor<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateStartup>,
{
    pub(crate) supervisor:
        Active<FixedSupervisor<Role, Worker, ImmediateStartup, Source, Infallible, Infallible>>,
    pub(crate) proxy: Active<StableProxy<Worker, ImmediateStartup>>,
    pub(crate) proxy_id: CreationId,
    pub(crate) worker: WorkerAttempt,
}

fn initial_worker(_: &Role) -> WorkerSubmission<Worker, ImmediateStartup> {
    WorkerSubmission::immediate(Worker::new(0))
}

fn committed_proxy(
    creations: Creations<CreateChild<RuntimeAddress, StableProxy<Worker, ImmediateStartup>>>,
) -> CreationsSettled<RuntimeAddress, StableProxy<Worker, ImmediateStartup>> {
    let creation = creations
        .into_iter()
        .next()
        .expect("one declared role creates one proxy");
    let (proxy, actor, kind) = creation.into_parts();
    drop(actor);
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established(behavior_core::CommittedChild::new(
                proxy,
                kind,
                behavior_core::EstablishedActor::issued(
                    crate::stable_proxy::installed_control::InstalledControl::new(WorkerEndpoint),
                ),
            )),
        ))]
        .into_iter()
        .collect(),
    ))
}

pub(crate) fn ready_supervisor<Source>(recovery: Recovery<Source>) -> ReadySupervisor<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateStartup>,
{
    let initialized = roster::new(
        initial_worker,
        OrderedRoles::new(Role::Search, core::iter::empty()).expect("one role is a valid roster"),
        roster::Config {
            startup: StartupLimit::new(1).expect("startup capacity is positive"),
            recovery: recovery,
            failure: FailureReaction::StopSupervisor,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::<Infallible>::terminate(),
        },
    )
    .build()
    .unwrap_or_else(|_| panic!("the initial worker is prepared"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed-supervisor initialization is pure"));
    let mut supervisor = initialized.behavior;
    let proxy_created = supervisor
        .on(committed_proxy(initialized.actions.creates))
        .unwrap_or_else(|_| panic!("the committed proxy receives its initial input"));
    let initial = match proxy_created
        .sends
        .proxy_operations
        .into_items()
        .into_iter()
        .map(ActionItemResult::<_>::Unattempted)
        .collect::<Vec<_>>()
        .pop()
        .expect("one initial proxy operation is emitted")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the operation is not interpreted by the fixture"),
    };
    let (proxy_id, control, receipt) = admit_proxy_operation(
        initial,
        EstablishedActor::<StableProxy<Worker, ImmediateStartup>>::issued(
            crate::stable_proxy::installed_control::InstalledControl::new(WorkerEndpoint),
        ),
    );
    let (proxy, _, ready) = drive_ready_proxy(StableProxy::immediate(), control);
    let worker = match &ready {
        ProxyOutcome::Initial {
            outcome:
                InitialWorkerOutcome::Resolved {
                    result: WorkerStartResult::Ready { attempt, .. },
                },
        } => attempt.clone(),
        ProxyOutcome::Initial { .. }
        | ProxyOutcome::Replacement { .. }
        | ProxyOutcome::WorkerStopped { .. }
        | ProxyOutcome::Unavailable { .. } => panic!("the proxy reaches ready"),
    };
    let accepted = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Accepted(receipt)))
        .unwrap_or_else(|_| panic!("the exact initial proxy input is accepted"));
    assert!(matches!(accepted.become_, Step::Continue));
    let opened = supervisor
        .on(ChildReport::new(proxy_id, ready))
        .unwrap_or_else(|_| panic!("the exact proxy outcome opens the roster"));
    assert!(matches!(opened.become_, Step::Continue));

    ReadySupervisor {
        supervisor,
        proxy,
        proxy_id,
        worker,
    }
}
