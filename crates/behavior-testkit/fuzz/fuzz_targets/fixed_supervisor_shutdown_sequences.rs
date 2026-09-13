#![no_main]
//! Stateful shutdown-order exploration for one ready fixed roster.

mod fixed_supervisor;
mod stable_proxy;

use behavior::atomic::{
    FixedCommand, ImmediateActivation, ProxyInputReceipt, Recovery, StableProxy,
};
use behavior::{
    CreationSequence, EstablishedActor, ItemSettlement, NoSends, SendSettlements, SettledItem, Step,
};
use fixed_supervisor::ready_supervisor;
use libfuzzer_sys::fuzz_target;
use stable_proxy::{RuntimeAddress, Worker, WorkerEndpoint, worker_stopped};

#[derive(Clone, Copy)]
enum ShutdownInput {
    OperationAccepted,
    OperationRejected,
    ProxyExited,
    WrongProxyExited,
    ShutdownRepeated,
}

impl ShutdownInput {
    const fn from_byte(byte: u8) -> Self {
        match byte % 5 {
            0 => Self::OperationAccepted,
            1 => Self::OperationRejected,
            2 => Self::ProxyExited,
            3 => Self::WrongProxyExited,
            _ => Self::ShutdownRepeated,
        }
    }
}

#[derive(Clone, Copy)]
enum ProxyRetirement {
    AwaitingOperationAndExit,
    AwaitingOperation,
    AwaitingExit,
    Retired,
}

enum ProxyRetirementInput {
    OperationReturned,
    ProxyExited,
    WrongProxyExited,
    ShutdownRepeated,
}

enum ProxyRetirementDecision {
    Waiting,
    Rejected,
    Retired,
}

impl ProxyRetirement {
    const fn accept(self, input: ProxyRetirementInput) -> (Self, ProxyRetirementDecision) {
        match (self, input) {
            (Self::AwaitingOperationAndExit, ProxyRetirementInput::OperationReturned) => {
                (Self::AwaitingExit, ProxyRetirementDecision::Waiting)
            }
            (Self::AwaitingOperationAndExit, ProxyRetirementInput::ProxyExited) => {
                (Self::AwaitingOperation, ProxyRetirementDecision::Waiting)
            }
            (Self::AwaitingOperation, ProxyRetirementInput::OperationReturned)
            | (Self::AwaitingExit, ProxyRetirementInput::ProxyExited) => {
                (Self::Retired, ProxyRetirementDecision::Retired)
            }
            (Self::Retired, _) => (Self::Retired, ProxyRetirementDecision::Retired),
            (current, ProxyRetirementInput::ShutdownRepeated) => {
                (current, ProxyRetirementDecision::Waiting)
            }
            (current, ProxyRetirementInput::WrongProxyExited)
            | (current, ProxyRetirementInput::ProxyExited)
            | (current, ProxyRetirementInput::OperationReturned) => {
                (current, ProxyRetirementDecision::Rejected)
            }
        }
    }
}

fn exercise(inputs: &[u8]) {
    let ready = ready_supervisor(Recovery::temporary());
    let mut supervisor = ready.supervisor;
    let proxy = ready.proxy_id;
    drop(ready.proxy);
    drop(ready.worker);

    let shutting_down = supervisor
        .receive(RuntimeAddress, FixedCommand::shutdown())
        .unwrap_or_else(|_| panic!("a ready roster accepts shutdown"));
    assert!(matches!(shutting_down.become_, Step::Continue));
    let mut operation = Some(
        match shutting_down
            .sends
            .proxy_operations
            .unattempted()
            .into_inputs()
            .pop()
            .expect("one exact proxy shutdown is emitted")
        {
            SettledItem::Unattempted(operation) => operation,
            SettledItem::Attempted(_) => panic!("the shutdown has not been interpreted"),
        },
    );
    let mut sequence = CreationSequence::new();
    let _occupied = sequence
        .issue()
        .expect("a distinct sequence issues an occupied proxy ID");
    let wrong_proxy = sequence
        .issue()
        .expect("the distinct sequence issues a nonmatching ID");
    assert_ne!(wrong_proxy, proxy);
    let mut retirement = ProxyRetirement::AwaitingOperationAndExit;

    for byte in inputs {
        match retirement {
            ProxyRetirement::Retired => break,
            ProxyRetirement::AwaitingOperationAndExit
            | ProxyRetirement::AwaitingOperation
            | ProxyRetirement::AwaitingExit => {}
        }
        let input = ShutdownInput::from_byte(*byte);
        let modeled = match input {
            ShutdownInput::OperationAccepted | ShutdownInput::OperationRejected => {
                ProxyRetirementInput::OperationReturned
            }
            ShutdownInput::ProxyExited => ProxyRetirementInput::ProxyExited,
            ShutdownInput::WrongProxyExited => ProxyRetirementInput::WrongProxyExited,
            ShutdownInput::ShutdownRepeated => ProxyRetirementInput::ShutdownRepeated,
        };
        let action = match input {
            ShutdownInput::OperationAccepted => match operation.take() {
                Some(operation) => {
                    let (route, _, operation) = operation.into_parts();
                    supervisor.on(SettledItem::Attempted(ItemSettlement::Accepted(
                        ProxyInputReceipt::new(
                            route,
                            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(
                                WorkerEndpoint,
                            ),
                            operation,
                        ),
                    )))
                }
                None => continue,
            },
            ShutdownInput::OperationRejected => match operation.take() {
                Some(operation) => supervisor.on(SettledItem::Unattempted(operation)),
                None => continue,
            },
            ShutdownInput::ProxyExited => supervisor.on(worker_stopped(proxy)),
            ShutdownInput::WrongProxyExited => supervisor.on(worker_stopped(wrong_proxy)),
            ShutdownInput::ShutdownRepeated => {
                supervisor.receive(RuntimeAddress, FixedCommand::shutdown())
            }
        }
        .unwrap_or_else(|_| panic!("every modeled shutdown input is total"));
        let (next, decision) = retirement.accept(modeled);
        retirement = next;

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

        match decision {
            ProxyRetirementDecision::Waiting => {
                assert!(matches!(action.become_, Step::Continue));
                assert!(action.sends.diagnostics.is_empty());
            }
            ProxyRetirementDecision::Rejected => {
                assert!(matches!(action.become_, Step::Stop(_)));
                assert_eq!(action.sends.diagnostics.len(), 1);
                break;
            }
            ProxyRetirementDecision::Retired => {
                assert!(matches!(action.become_, Step::Stop(_)));
                assert!(action.sends.diagnostics.is_empty());
            }
        }
    }
}

fuzz_target!(|inputs: &[u8]| {
    exercise(inputs);
});
