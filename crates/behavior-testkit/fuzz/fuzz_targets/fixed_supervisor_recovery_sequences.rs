#![no_main]
//! Stateful one-role recovery through the actual stable proxy.

mod fixed_supervisor;
mod fixed_supervisor_recovery;
mod stable_proxy;

use core::ops::ControlFlow;
use std::time::Duration;

use behavior_actors::atomic::{
    CapabilityResult, FixedSupervisorEvent, ImmediateActivation, ProxyInputReceipt, ProxyOutcome,
    Recovery, RestartLimit, RestartRelease, StableProxy, Strategy, UnavailablePhase, WorkerSource,
    WorkerSubmission,
};
use behavior_core::{
    ChildReport, CreationSequence, EstablishedActor, ItemSettlement, Never, NoSends,
    SendSettlements, SettledItem, Step,
};
use fixed_supervisor::Role;
use fixed_supervisor_recovery::{search_capability, search_recovery};
use libfuzzer_sys::fuzz_target;
use stable_proxy::{Worker, WorkerEndpoint, start_ready_worker, worker_stopped};

struct Workshop;

impl WorkerSource<Role, Worker, ImmediateActivation> for Workshop {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

#[derive(Clone, Copy)]
enum RecoveryInput {
    ReceiptReturned,
    OutcomeReturned,
    WrongProxyOutcome,
    DuplicateWorkerStop,
}

impl RecoveryInput {
    const fn from_byte(byte: u8) -> Self {
        match byte % 4 {
            0 => Self::ReceiptReturned,
            1 => Self::OutcomeReturned,
            2 => Self::WrongProxyOutcome,
            _ => Self::DuplicateWorkerStop,
        }
    }
}

#[derive(Clone, Copy)]
enum ExpectedService {
    ProxyReceiptRequired,
    ProxyOutcomeRequired,
    Available,
    SupervisorStopped,
}

enum ObservedReturn {
    ProxyReceipt,
    ProxyOutcome,
    UnknownProxyOutcome,
    RepeatedWorkerStop,
}

enum ExpectedReply {
    Unavailable,
    SupervisorStops,
    Available,
}

impl ExpectedService {
    const fn observe(self, returned: ObservedReturn) -> (Self, ExpectedReply) {
        match (self, returned) {
            (Self::ProxyReceiptRequired, ObservedReturn::ProxyReceipt) => {
                (Self::ProxyOutcomeRequired, ExpectedReply::Unavailable)
            }
            (Self::ProxyOutcomeRequired, ObservedReturn::ProxyOutcome) => {
                (Self::Available, ExpectedReply::Available)
            }
            (Self::Available, _) => (Self::Available, ExpectedReply::Available),
            (Self::SupervisorStopped, _) => {
                (Self::SupervisorStopped, ExpectedReply::SupervisorStops)
            }
            (Self::ProxyReceiptRequired, ObservedReturn::ProxyOutcome)
            | (Self::ProxyReceiptRequired, ObservedReturn::UnknownProxyOutcome)
            | (Self::ProxyReceiptRequired, ObservedReturn::RepeatedWorkerStop)
            | (Self::ProxyOutcomeRequired, ObservedReturn::ProxyReceipt)
            | (Self::ProxyOutcomeRequired, ObservedReturn::UnknownProxyOutcome)
            | (Self::ProxyOutcomeRequired, ObservedReturn::RepeatedWorkerStop) => {
                (Self::SupervisorStopped, ExpectedReply::SupervisorStops)
            }
        }
    }
}

fn exercise(inputs: &[u8]) {
    let recovering = search_recovery(Recovery::permanent(
        Workshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::immediate(),
    ));
    let mut supervisor = recovering.supervisor;
    let mut proxy = recovering.proxy;
    let proxy_id = recovering.proxy_id;
    let previous = recovering.previous;
    let preparation = match recovering
        .preparation
        .accept(WorkerSubmission::immediate(Worker::new(1)))
    {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => panic!("one role needs one worker submission"),
    };
    let prepared = supervisor
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("the exact preparation returns to its recovery"));
    let replacement = match prepared
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
    let (route, control, operation_id) = replacement.into_parts();
    assert_eq!(route, proxy_id);
    let (_successor, replacement_outcome) = start_ready_worker(&mut proxy, control);
    let mut operation = Some((route, operation_id));
    let mut outcome = Some(replacement_outcome);
    let mut sequence = CreationSequence::new();
    let _occupied = sequence
        .issue()
        .expect("a distinct sequence issues an occupied proxy ID");
    let wrong_proxy = sequence
        .issue()
        .expect("the distinct sequence issues a nonmatching proxy ID");
    assert_ne!(wrong_proxy, proxy_id);
    let mut expected_service = ExpectedService::ProxyReceiptRequired;

    for byte in inputs {
        match expected_service {
            ExpectedService::Available | ExpectedService::SupervisorStopped => break,
            ExpectedService::ProxyReceiptRequired | ExpectedService::ProxyOutcomeRequired => {}
        }
        let input = RecoveryInput::from_byte(*byte);
        let action = match input {
            RecoveryInput::ReceiptReturned => match operation.take() {
                Some((route, operation_id)) => supervisor.on(SettledItem::Attempted(
                    ItemSettlement::Accepted(ProxyInputReceipt::new(
                        route,
                        EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
                            WorkerEndpoint,
                        ),
                        operation_id,
                    )),
                )),
                None => continue,
            },
            RecoveryInput::OutcomeReturned => match outcome.take() {
                Some(outcome) => supervisor.on(ChildReport::new(proxy_id, outcome)),
                None => continue,
            },
            RecoveryInput::WrongProxyOutcome => match outcome.take() {
                Some(outcome) => supervisor.on(ChildReport::new(wrong_proxy, outcome)),
                None => continue,
            },
            RecoveryInput::DuplicateWorkerStop => supervisor.on(ChildReport::new(
                proxy_id,
                ProxyOutcome::WorkerStopped {
                    worker: previous.clone(),
                    stopped: worker_stopped(previous.creation()),
                },
            )),
        }
        .unwrap_or_else(|_| panic!("every modeled recovery input is total"));
        let observed = match input {
            RecoveryInput::ReceiptReturned => ObservedReturn::ProxyReceipt,
            RecoveryInput::OutcomeReturned => ObservedReturn::ProxyOutcome,
            RecoveryInput::WrongProxyOutcome => ObservedReturn::UnknownProxyOutcome,
            RecoveryInput::DuplicateWorkerStop => ObservedReturn::RepeatedWorkerStop,
        };
        let (next, expected_reply) = expected_service.observe(observed);
        expected_service = next;

        assert!(action.creates.is_empty());
        assert!(action.sends.proxy_observations.into_requests().is_empty());
        assert!(
            action
                .sends
                .worker_preparations
                .unattempted()
                .into_inputs()
                .is_empty()
        );
        assert!(
            action
                .sends
                .proxy_operations
                .unattempted()
                .into_inputs()
                .is_empty()
        );
        assert!(
            action
                .sends
                .restart_schedules
                .unattempted()
                .into_inputs()
                .is_empty()
        );
        let NoSends = action.sends.lifecycle;
        assert!(action.sends.status_replies.into_deliveries().is_empty());
        assert!(action.sends.capability_replies.into_deliveries().is_empty());

        match expected_reply {
            ExpectedReply::SupervisorStops => {
                assert!(matches!(action.become_, Step::Stop(_)));
                assert_eq!(action.sends.diagnostics.len(), 1);
                break;
            }
            ExpectedReply::Unavailable | ExpectedReply::Available => {
                assert!(matches!(action.become_, Step::Continue));
                assert!(action.sends.diagnostics.is_empty());
            }
        }

        match (expected_reply, search_capability(&mut supervisor)) {
            (
                ExpectedReply::Unavailable,
                CapabilityResult::Unavailable {
                    phase: UnavailablePhase::Recovering,
                    ..
                },
            )
            | (ExpectedReply::Available, CapabilityResult::Ready { .. }) => {}
            (ExpectedReply::SupervisorStops, _)
            | (ExpectedReply::Unavailable, _)
            | (ExpectedReply::Available, _) => {
                panic!("capability reply disagrees with recovery sequence")
            }
        }
    }
}

fuzz_target!(|inputs: &[u8]| {
    exercise(inputs);
});
