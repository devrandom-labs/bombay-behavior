//! Current one-role FixedSupervisor setup backed by its actual StableProxy.

use core::convert::Infallible;

use behavior::atomic::{
    ActivationPolicy, ActorDrainPolicy, DiagnosticDisposition, FailureReaction, FixedSupervisor,
    ImmediateActivation, InitialWorkerOutcome, OrderedRoles, ProxyInputReceipt, ProxyOutcome,
    Recovery, StableProxy, WorkerAttempt, WorkerSource, WorkerStartResult, WorkerSubmission, fixed,
};
use behavior::{
    Activate as _, Active, ChildCreationOutcome, ChildReport, CreateChild, CreationId,
    CreationSettlement, Creations, CreationsSettled, EstablishedActor, EstablishedCreation,
    EstablishedRecipient, ItemSettlement, Never, SendSettlements, SettledItem, Step,
};

use crate::stable_proxy::{RuntimeAddress, Worker, WorkerEndpoint, drive_ready_proxy};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Search,
}

pub(crate) struct ReadySupervisor<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    pub(crate) supervisor:
        Active<FixedSupervisor<Role, Worker, ImmediateActivation, Source, Infallible, Infallible>>,
    pub(crate) proxy: Active<StableProxy<Worker, ImmediateActivation>>,
    pub(crate) proxy_id: CreationId,
    pub(crate) worker: WorkerAttempt,
}

fn initial_worker(_: &Role) -> Result<WorkerSubmission<Worker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(Worker::new(0)))
}

fn committed_proxy(
    creations: Creations<CreateChild<RuntimeAddress, StableProxy<Worker, ImmediateActivation>>>,
) -> CreationsSettled<RuntimeAddress, StableProxy<Worker, ImmediateActivation>> {
    let creation = creations
        .into_iter()
        .next()
        .expect("one declared role creates one proxy");
    let (proxy, actor, kind) = creation.into_parts();
    drop(actor);
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    proxy,
                    kind,
                    EstablishedRecipient::issued(WorkerEndpoint),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

pub(crate) fn ready_supervisor<Source>(recovery: Recovery<Source>) -> ReadySupervisor<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(Role::Search, core::iter::empty()).expect("one role is a valid roster"),
        ActivationPolicy::new(1).expect("activation capacity is positive"),
        recovery,
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::<Infallible>::terminate(),
    )
    .build::<Worker, ImmediateActivation, Never>()
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
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one initial proxy operation is emitted")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the operation is not interpreted by the fixture"),
    };
    let (proxy_id, control, operation) = initial.into_parts();
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
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                proxy_id,
                EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
                    WorkerEndpoint,
                ),
                operation,
            ),
        )))
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
