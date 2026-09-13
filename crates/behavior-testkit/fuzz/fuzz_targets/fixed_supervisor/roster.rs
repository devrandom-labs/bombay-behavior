//! Ready three-role FixedSupervisor setup backed by actual StableProxy actors.

use core::convert::Infallible;

use behavior::atomic::{
    ActivationPolicy, ActorDrainPolicy, DiagnosticDisposition, FailureReaction, FixedDiagnostic,
    FixedSupervisor, ImmediateActivation, InitialWorkerOutcome, OrderedRoles, ProxyInputReceipt,
    ProxyOutcome, Recovery, StableProxy, WorkerAttempt, WorkerSource, WorkerStartResult,
    WorkerSubmission, fixed,
};
use behavior::{
    Activate as _, Active, ChildCreationOutcome, ChildReport, CreateChild, CreationId,
    CreationSettlement, Creations, CreationsSettled, EstablishedActor, EstablishedCreation,
    EstablishedRecipient, ItemSettlement, MessageProtocol, Never, SendSettlements, SettledItem,
    Step,
};

use crate::stable_proxy::{RuntimeAddress, Worker, WorkerEndpoint, drive_ready_proxy};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Role {
    Search,
    Index,
    Spellcheck,
}

pub(crate) struct ReadyMember {
    pub(crate) role: Role,
    pub(crate) proxy: Active<StableProxy<Worker, ImmediateActivation>>,
    pub(crate) proxy_id: CreationId,
    pub(crate) worker: WorkerAttempt,
}

pub(crate) struct ReadyRoster<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    pub(crate) supervisor: Active<
        FixedSupervisor<
            Role,
            Worker,
            ImmediateActivation,
            Source,
            EstablishedRecipient<
                MessageProtocol<
                    RuntimeAddress,
                    FixedDiagnostic<Role, Worker, ImmediateActivation, Source>,
                >,
            >,
            Infallible,
        >,
    >,
    pub(crate) members: Vec<ReadyMember>,
}

fn initial_worker(role: &Role) -> Result<WorkerSubmission<Worker, ImmediateActivation>, Never> {
    let worker = match role {
        Role::Search => Worker::new(1),
        Role::Index => Worker::new(2),
        Role::Spellcheck => Worker::new(3),
    };
    Ok(WorkerSubmission::immediate(worker))
}

fn committed_proxies(
    creations: Creations<CreateChild<RuntimeAddress, StableProxy<Worker, ImmediateActivation>>>,
) -> CreationsSettled<RuntimeAddress, StableProxy<Worker, ImmediateActivation>> {
    CreationsSettled::new(CreationSettlement::Settled(
        creations
            .into_iter()
            .map(|creation| {
                let (proxy, actor, kind) = creation.into_parts();
                drop(actor);
                SettledItem::Attempted(ItemSettlement::Accepted(
                    ChildCreationOutcome::Established {
                        established: EstablishedCreation::installed(
                            proxy,
                            kind,
                            EstablishedRecipient::issued(WorkerEndpoint),
                        ),
                    },
                ))
            })
            .collect(),
    ))
}

pub(crate) fn ready_three_role_roster<Source>(recovery: Recovery<Source>) -> ReadyRoster<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    let initialized = fixed(
        initial_worker,
        OrderedRoles::new(Role::Search, [Role::Index, Role::Spellcheck])
            .expect("three distinct roles form one fixed roster"),
        ActivationPolicy::new(3).expect("three workers may activate together"),
        recovery,
        FailureReaction::StopSupervisor,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::deliver_to(EstablishedRecipient::issued(WorkerEndpoint)),
    )
    .build::<Worker, ImmediateActivation, Never>()
    .unwrap_or_else(|_| panic!("each declared role has an initial worker"))
    .initialize()
    .unwrap_or_else(|_| panic!("fixed-supervisor initialization is pure"));
    let mut supervisor = initialized.behavior;
    let proxy_created = supervisor
        .on(committed_proxies(initialized.actions.creates))
        .unwrap_or_else(|_| panic!("the committed proxies receive their initial inputs"));
    let operations = proxy_created
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs();
    assert_eq!(operations.len(), 3);
    let mut members = Vec::with_capacity(3);

    for (role, operation) in [Role::Search, Role::Index, Role::Spellcheck]
        .into_iter()
        .zip(operations)
    {
        let operation = match operation {
            SettledItem::Unattempted(operation) => operation,
            SettledItem::Attempted(_) => {
                panic!("the fixture intercepts each uninterpreted initial operation")
            }
        };
        let (proxy_id, control, operation) = operation.into_parts();
        let (proxy, _, outcome) = drive_ready_proxy(StableProxy::immediate(), control);
        let worker = match &outcome {
            ProxyOutcome::Initial {
                outcome:
                    InitialWorkerOutcome::Resolved {
                        result: WorkerStartResult::Ready { attempt, .. },
                    },
            } => attempt.clone(),
            ProxyOutcome::Initial { .. }
            | ProxyOutcome::Replacement { .. }
            | ProxyOutcome::WorkerStopped { .. }
            | ProxyOutcome::Unavailable { .. } => panic!("each initial proxy reaches ready"),
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
            .on(ChildReport::new(proxy_id, outcome))
            .unwrap_or_else(|_| panic!("the exact proxy outcome opens its role"));
        assert!(matches!(opened.become_, Step::Continue));
        members.push(ReadyMember {
            role,
            proxy,
            proxy_id,
            worker,
        });
    }

    ReadyRoster {
        supervisor,
        members,
    }
}
