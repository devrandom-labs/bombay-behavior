#![no_main]
#![allow(
    clippy::type_complexity,
    reason = "the target keeps the actual inferred FixedSupervisor composition visible"
)]
//! Three simultaneous recoveries preserve exact role ownership under noisy returns.

#[path = "fixed_supervisor/roster.rs"]
mod roster;
mod stable_proxy;

use core::convert::Infallible;
use core::ops::ControlFlow;
use std::collections::BTreeMap;
use std::time::Duration;

use behavior::atomic::{
    CapabilityResult, DiagnosticAction, FixedCommand, FixedDiagnostic, FixedSupervisor,
    FixedSupervisorEvent, ImmediateActivation, ProxyControl, ProxyInputReceipt, ProxyOperationId,
    ProxyOutcome, Recovery, RestartLimit, RestartRelease, StableProxy, Strategy, UnavailablePhase,
    WorkerAttempt, WorkerSource, WorkerSubmission,
};
use behavior::{
    Active, ChildReport, CreationId, EstablishedActor, EstablishedRecipient, ItemSettlement,
    MessageProtocol, Never, Recipient, ReplyDelivery, SendSettlements, SettledItem, Step,
};
use libfuzzer_sys::fuzz_target;
use roster::{ReadyMember, Role, ready_three_role_roster};
use stable_proxy::{RuntimeAddress, Worker, WorkerEndpoint, start_ready_worker, worker_stopped};

const ROLES: [Role; 3] = [Role::Search, Role::Index, Role::Spellcheck];

struct Workshop;

impl WorkerSource<Role, Worker, ImmediateActivation> for Workshop {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

struct PendingRecovery {
    proxy_id: CreationId,
    proxy: Active<StableProxy<Worker, ImmediateActivation>>,
    previous: WorkerAttempt,
    work: ReplacementWork,
}

enum ReplacementWork {
    AwaitingDelivery {
        route: CreationId,
        control: ProxyControl<Worker, ImmediateActivation>,
        operation: ProxyOperationId,
    },
    AwaitingReport(ProxyOutcome<Worker, ImmediateActivation>),
    Ready,
}

#[derive(Clone, Copy)]
enum ReportSource {
    OwnProxy,
    OtherProxy(Role),
}

#[derive(Clone, Copy)]
enum RecoveryInput {
    ReturnReceipt(Role),
    ReturnReport { role: Role, source: ReportSource },
    RepeatWorkerStop(Role),
    QueryCapabilities,
}

enum ExpectedCapability {
    Recovering,
    Ready,
}

impl RecoveryInput {
    fn from_bytes(bytes: &[u8]) -> Self {
        let role = role_from_byte(bytes[1]);
        match bytes[0] % 4 {
            0 => Self::ReturnReceipt(role),
            1 => Self::ReturnReport {
                role,
                source: match bytes[2] % 3 {
                    0 => ReportSource::OwnProxy,
                    1 => ReportSource::OtherProxy(next_role(role)),
                    _ => ReportSource::OtherProxy(previous_role(role)),
                },
            },
            2 => Self::RepeatWorkerStop(role),
            _ => Self::QueryCapabilities,
        }
    }
}

fn role_from_byte(byte: u8) -> Role {
    match byte % 3 {
        0 => Role::Search,
        1 => Role::Index,
        _ => Role::Spellcheck,
    }
}

const fn next_role(role: Role) -> Role {
    match role {
        Role::Search => Role::Index,
        Role::Index => Role::Spellcheck,
        Role::Spellcheck => Role::Search,
    }
}

const fn previous_role(role: Role) -> Role {
    match role {
        Role::Search => Role::Spellcheck,
        Role::Index => Role::Search,
        Role::Spellcheck => Role::Index,
    }
}

fn begin_recovery(
    supervisor: &mut Active<
        FixedSupervisor<
            Role,
            Worker,
            ImmediateActivation,
            Workshop,
            EstablishedRecipient<
                MessageProtocol<
                    RuntimeAddress,
                    FixedDiagnostic<Role, Worker, ImmediateActivation, Workshop>,
                >,
            >,
            Infallible,
        >,
    >,
    mut member: ReadyMember,
    successor: u8,
) -> PendingRecovery {
    let stopped = member
        .proxy
        .on(worker_stopped(member.worker.creation()))
        .expect("the actual worker stop reaches its stable proxy");
    let report = stopped
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the stable proxy reports its exact worker stop")
        .into_inner();
    let preparing = supervisor
        .on(ChildReport::new(member.proxy_id, report))
        .unwrap_or_else(|_| panic!("the exact worker stop starts one-role recovery"));
    let preparation = match preparing
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one-role recovery emits one preparation")
    {
        SettledItem::Unattempted(preparation) => preparation,
        SettledItem::Attempted(_) => panic!("the preparation has not been interpreted"),
    };
    let preparation = match preparation.accept(WorkerSubmission::immediate(Worker::new(successor)))
    {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => panic!("one selected role needs one worker submission"),
    };
    let admitted = supervisor
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact preparation admits one replacement"));
    let operation = match admitted
        .sends
        .proxy_operations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("immediate release emits one replacement")
    {
        SettledItem::Unattempted(operation) => operation,
        SettledItem::Attempted(_) => panic!("the replacement has not been interpreted"),
    };
    let (route, control, operation) = operation.into_parts();
    assert_eq!(route, member.proxy_id);
    PendingRecovery {
        proxy_id: member.proxy_id,
        proxy: member.proxy,
        previous: member.worker,
        work: ReplacementWork::AwaitingDelivery {
            route,
            control,
            operation,
        },
    }
}

fn proxy_for(recoveries: &BTreeMap<Role, PendingRecovery>, role: Role) -> CreationId {
    recoveries
        .get(&role)
        .map(|recovery| recovery.proxy_id)
        .expect("every modeled role belongs to the roster")
}

fn expected_capability(recovery: &PendingRecovery) -> ExpectedCapability {
    match recovery.work {
        ReplacementWork::AwaitingDelivery { .. } | ReplacementWork::AwaitingReport(_) => {
            ExpectedCapability::Recovering
        }
        ReplacementWork::Ready => ExpectedCapability::Ready,
    }
}

fn assert_capabilities(
    supervisor: &mut Active<
        FixedSupervisor<
            Role,
            Worker,
            ImmediateActivation,
            Workshop,
            EstablishedRecipient<
                MessageProtocol<
                    RuntimeAddress,
                    FixedDiagnostic<Role, Worker, ImmediateActivation, Workshop>,
                >,
            >,
            Infallible,
        >,
    >,
    recoveries: &BTreeMap<Role, PendingRecovery>,
) {
    for role in ROLES {
        let queried = supervisor
            .receive(
                RuntimeAddress,
                FixedCommand::capability(
                    role,
                    Recipient::<MessageProtocol<
                        RuntimeAddress,
                        CapabilityResult<Role, Worker>,
                    >>::global(RuntimeAddress),
                ),
            )
            .unwrap_or_else(|_| panic!("capability lookup is total for each declared role"));
        assert!(matches!(queried.become_, Step::Continue));
        assert!(queried.sends.diagnostics.is_empty());
        let reply = queried
            .sends
            .capability_replies
            .into_deliveries()
            .pop()
            .expect("one capability reply is emitted");
        let ReplyDelivery::Logical(reply) = reply else {
            panic!("the logical query route stays logical")
        };
        match (
            expected_capability(
                recoveries
                    .get(&role)
                    .expect("every declared role has one recovery record"),
            ),
            reply.message,
        ) {
            (
                ExpectedCapability::Recovering,
                CapabilityResult::Unavailable {
                    role: returned,
                    phase: UnavailablePhase::Recovering,
                },
            )
            | (ExpectedCapability::Ready, CapabilityResult::Ready { role: returned, .. }) => {
                assert_eq!(returned, role)
            }
            (ExpectedCapability::Recovering, CapabilityResult::Ready { .. })
            | (ExpectedCapability::Recovering, CapabilityResult::Unavailable { .. })
            | (ExpectedCapability::Recovering, CapabilityResult::UnknownRole { .. })
            | (ExpectedCapability::Ready, CapabilityResult::Unavailable { .. })
            | (ExpectedCapability::Ready, CapabilityResult::UnknownRole { .. }) => {
                panic!("capability disagrees with the role's outstanding recovery values")
            }
        }
    }
}

fn return_receipt(
    supervisor: &mut Active<
        FixedSupervisor<
            Role,
            Worker,
            ImmediateActivation,
            Workshop,
            EstablishedRecipient<
                MessageProtocol<
                    RuntimeAddress,
                    FixedDiagnostic<Role, Worker, ImmediateActivation, Workshop>,
                >,
            >,
            Infallible,
        >,
    >,
    mut recovery: PendingRecovery,
) -> PendingRecovery {
    let (route, control, operation) = match recovery.work {
        ReplacementWork::AwaitingDelivery {
            route,
            control,
            operation,
        } => (route, control, operation),
        work @ (ReplacementWork::AwaitingReport(_) | ReplacementWork::Ready) => {
            recovery.work = work;
            return recovery;
        }
    };
    let accepted = supervisor
        .on(SettledItem::Attempted(ItemSettlement::Accepted(
            ProxyInputReceipt::new(
                route,
                EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
                    WorkerEndpoint,
                ),
                operation,
            ),
        )))
        .unwrap_or_else(|_| panic!("the exact proxy input return advances its recovery"));
    assert!(matches!(accepted.become_, Step::Continue));
    assert!(accepted.sends.diagnostics.is_empty());
    let (_, report) = start_ready_worker(&mut recovery.proxy, control);
    recovery.work = ReplacementWork::AwaitingReport(report);
    recovery
}

fn return_report(
    supervisor: &mut Active<
        FixedSupervisor<
            Role,
            Worker,
            ImmediateActivation,
            Workshop,
            EstablishedRecipient<
                MessageProtocol<
                    RuntimeAddress,
                    FixedDiagnostic<Role, Worker, ImmediateActivation, Workshop>,
                >,
            >,
            Infallible,
        >,
    >,
    mut recovery: PendingRecovery,
) -> PendingRecovery {
    let report = match recovery.work {
        ReplacementWork::AwaitingReport(report) => report,
        work @ (ReplacementWork::AwaitingDelivery { .. } | ReplacementWork::Ready) => {
            recovery.work = work;
            return recovery;
        }
    };
    let accepted = supervisor
        .on(ChildReport::new(recovery.proxy_id, report))
        .unwrap_or_else(|_| panic!("the exact proxy report advances its recovery"));
    assert!(matches!(accepted.become_, Step::Continue));
    assert!(accepted.sends.diagnostics.is_empty());
    recovery.work = ReplacementWork::Ready;
    recovery
}

fn return_cross_report(
    supervisor: &mut Active<
        FixedSupervisor<
            Role,
            Worker,
            ImmediateActivation,
            Workshop,
            EstablishedRecipient<
                MessageProtocol<
                    RuntimeAddress,
                    FixedDiagnostic<Role, Worker, ImmediateActivation, Workshop>,
                >,
            >,
            Infallible,
        >,
    >,
    source_proxy: CreationId,
    mut recovery: PendingRecovery,
) -> PendingRecovery {
    let report = match recovery.work {
        ReplacementWork::AwaitingReport(report) => report,
        work @ (ReplacementWork::AwaitingDelivery { .. } | ReplacementWork::Ready) => {
            recovery.work = work;
            return recovery;
        }
    };
    let rejected = supervisor
        .on(ChildReport::new(source_proxy, report))
        .unwrap_or_else(|_| panic!("a cross-sourced report becomes a diagnostic"));
    assert!(matches!(rejected.become_, Step::Continue));
    let mut diagnostics = rejected.sends.diagnostics.into_requests();
    let DiagnosticAction::Deliver {
        diagnostic:
            FixedDiagnostic::UnexpectedInput {
                input: FixedSupervisorEvent::ProxyReported(returned),
            },
        ..
    } = diagnostics
        .pop()
        .expect("the complete cross-sourced report is returned")
    else {
        panic!("cross-sourced replacement keeps its diagnostic meaning")
    };
    assert!(diagnostics.is_empty());
    assert_eq!(returned.child, source_proxy);
    recovery.work = ReplacementWork::AwaitingReport(returned.report);
    recovery
}

fn exercise(inputs: &[u8]) {
    let ready = ready_three_role_roster(Recovery::permanent(
        Workshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    ));
    let mut supervisor = ready.supervisor;
    let mut recoveries = BTreeMap::new();
    for (successor, member) in (10_u8..).zip(ready.members) {
        let role = member.role;
        let replaced = recoveries.insert(role, begin_recovery(&mut supervisor, member, successor));
        assert!(replaced.is_none());
    }
    assert_capabilities(&mut supervisor, &recoveries);

    for bytes in inputs.chunks_exact(3) {
        match RecoveryInput::from_bytes(bytes) {
            RecoveryInput::ReturnReceipt(role) => {
                let recovery = recoveries
                    .remove(&role)
                    .expect("every input role owns one recovery");
                let replaced = recoveries.insert(role, return_receipt(&mut supervisor, recovery));
                assert!(replaced.is_none());
            }
            RecoveryInput::ReturnReport {
                role,
                source: ReportSource::OwnProxy,
            } => {
                let recovery = recoveries
                    .remove(&role)
                    .expect("every input role owns one recovery");
                let replaced = recoveries.insert(role, return_report(&mut supervisor, recovery));
                assert!(replaced.is_none());
            }
            RecoveryInput::ReturnReport {
                role,
                source: ReportSource::OtherProxy(source),
            } => {
                let source_proxy = proxy_for(&recoveries, source);
                let recovery = recoveries
                    .remove(&role)
                    .expect("every input role owns one recovery");
                let recovery = return_cross_report(&mut supervisor, source_proxy, recovery);
                let replaced = recoveries.insert(role, recovery);
                assert!(replaced.is_none());
            }
            RecoveryInput::RepeatWorkerStop(role) => {
                let recovery = recoveries
                    .get(&role)
                    .expect("every input role owns one recovery");
                let repeated_stop = worker_stopped(recovery.previous.creation());
                let repeated = supervisor
                    .on(ChildReport::new(
                        recovery.proxy_id,
                        ProxyOutcome::WorkerStopped {
                            worker: recovery.previous.clone(),
                            stopped: repeated_stop,
                        },
                    ))
                    .unwrap_or_else(|_| panic!("a repeated predecessor stop becomes a diagnostic"));
                assert!(matches!(repeated.become_, Step::Continue));
                let mut diagnostics = repeated.sends.diagnostics.into_requests();
                let Some(DiagnosticAction::Deliver {
                    diagnostic:
                        FixedDiagnostic::UnexpectedInput {
                            input:
                                FixedSupervisorEvent::ProxyReported(ChildReport {
                                    child,
                                    report: ProxyOutcome::WorkerStopped { worker, stopped },
                                }),
                        },
                    ..
                }) = diagnostics.pop()
                else {
                    panic!("the repeated worker stop is returned complete")
                };
                assert!(diagnostics.is_empty());
                assert_eq!(child, recovery.proxy_id);
                assert_eq!(worker, recovery.previous);
                assert_eq!(stopped, repeated_stop);
            }
            RecoveryInput::QueryCapabilities => {}
        }
        assert_capabilities(&mut supervisor, &recoveries);
    }

    for role in ROLES {
        let recovery = recoveries
            .remove(&role)
            .expect("every declared role owns one recovery");
        let recovery = return_receipt(&mut supervisor, recovery);
        let recovery = return_report(&mut supervisor, recovery);
        let replaced = recoveries.insert(role, recovery);
        assert!(replaced.is_none());
    }
    assert_capabilities(&mut supervisor, &recoveries);
}

fuzz_target!(|inputs: &[u8]| {
    exercise(inputs);
});
