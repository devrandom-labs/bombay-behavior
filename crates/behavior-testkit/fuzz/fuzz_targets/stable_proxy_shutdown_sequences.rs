#![no_main]
//! Arbitrary exact, duplicate, and foreign inputs during ready-proxy shutdown.

mod stable_proxy;
mod worker_return;

use behavior::atomic::{ProxyControl, ProxyPhase, StableProxy};
use behavior::{EstablishedShutdownResolved, ShutdownId, Step};
use libfuzzer_sys::fuzz_target;
use stable_proxy::{Worker, drive_ready_proxy, worker_stopped};
use worker_return::{WorkerReturn, WorkerReturnDecision, WorkerReturnInput, foreign_worker};

#[derive(Clone, Copy)]
enum ShutdownInput {
    WorkerReturn(WorkerReturnInput),
    ShutdownRepeated,
}

impl ShutdownInput {
    const fn from_byte(byte: u8) -> Self {
        match byte % 5 {
            0 => Self::WorkerReturn(WorkerReturnInput::ExactWorkerStopped),
            1 => Self::WorkerReturn(WorkerReturnInput::ExactShutdownReturned),
            2 => Self::WorkerReturn(WorkerReturnInput::ForeignWorkerStopped),
            3 => Self::WorkerReturn(WorkerReturnInput::ForeignShutdownReturned),
            _ => Self::ShutdownRepeated,
        }
    }
}

fn exercise(inputs: &[u8]) {
    let (mut proxy, worker, _outcome) = drive_ready_proxy(
        StableProxy::immediate(),
        ProxyControl::start(Worker::new(0)),
    );
    let foreign_worker = foreign_worker(worker);
    let shutting_down = proxy
        .on(ProxyControl::shutdown())
        .expect("ready shutdown starts worker return");
    let shutdown = shutting_down
        .sends
        .worker_shutdowns
        .into_requests()
        .pop()
        .expect("ready shutdown emits one exact request")
        .id;
    let foreign_shutdown = ShutdownId(foreign_worker.get());
    assert_ne!(foreign_shutdown, shutdown);
    let mut worker_return = WorkerReturn::AwaitingStopAndReceipt;

    for byte in inputs {
        if matches!(worker_return, WorkerReturn::Returned) {
            break;
        }
        let input = ShutdownInput::from_byte(*byte);
        let (next, decision) = match input {
            ShutdownInput::WorkerReturn(input) => worker_return.accept(input),
            ShutdownInput::ShutdownRepeated => (worker_return, WorkerReturnDecision::Waiting),
        };
        worker_return = next;
        let actions = match input {
            ShutdownInput::WorkerReturn(WorkerReturnInput::ExactWorkerStopped) => {
                proxy.on(worker_stopped(worker))
            }
            ShutdownInput::WorkerReturn(WorkerReturnInput::ExactShutdownReturned) => {
                proxy.on(EstablishedShutdownResolved::<Worker>::accepted(shutdown))
            }
            ShutdownInput::WorkerReturn(WorkerReturnInput::ForeignWorkerStopped) => {
                proxy.on(worker_stopped(foreign_worker))
            }
            ShutdownInput::WorkerReturn(WorkerReturnInput::ForeignShutdownReturned) => proxy.on(
                EstablishedShutdownResolved::<Worker>::accepted(foreign_shutdown),
            ),
            ShutdownInput::ShutdownRepeated => proxy.on(ProxyControl::shutdown()),
        }
        .expect("every modeled shutdown input is total");

        match decision {
            WorkerReturnDecision::Waiting => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert!(actions.sends.diagnostics.is_empty());
            }
            WorkerReturnDecision::InputRejected => {
                assert_eq!(proxy.phase(), ProxyPhase::ShuttingDown);
                assert!(matches!(actions.become_, Step::Continue));
                assert_eq!(actions.sends.diagnostics.len(), 1);
            }
            WorkerReturnDecision::Returned => {
                assert_eq!(proxy.phase(), ProxyPhase::Stopped);
                assert!(matches!(actions.become_, Step::Stop(_)));
                assert!(actions.sends.diagnostics.is_empty());
            }
        }
        assert!(actions.creates.is_empty());
        assert!(actions.sends.worker_observations.is_empty());
        assert!(actions.sends.worker_initializations.is_empty());
        assert!(actions.sends.worker_activations.is_empty());
        assert!(actions.sends.worker_shutdowns.is_empty());
        assert!(actions.sends.worker_deliveries.is_empty());
        assert!(actions.sends.owner_outcomes.is_empty());
    }
}

fuzz_target!(|inputs: &[u8]| {
    exercise(inputs);
});
