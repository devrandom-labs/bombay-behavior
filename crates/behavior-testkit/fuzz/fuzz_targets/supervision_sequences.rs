#![no_main]

//! Supervision attack surface: arbitrary child-stopped byte sequences drive
//! a OneForOne/Permanent supervisor with a small restart budget and a small
//! window, while an inline reference model (documented semantics: lazy
//! window pruning with inclusive edge + future-stamp survival, budget counts
//! every replacement, denial kills the dead slot) tracks the expected
//! replacement sends, alive flags, and restart-stamp count. The impl must
//! agree on every byte.

use behavior::{
    Acted, Actions, Crash, Delivery, MailAddr, Never, RestartDenial, RestartPolicy, Step, Strategy,
    SupervisionEvent, SupervisionFailureReason, Supervisor, WorkerStopped,
    stop_on_supervision_failure,
};
use libfuzzer_sys::fuzz_target;
use std::time::Instant;
use tokio::runtime::Builder;

const FLEET: usize = 4;
const BUDGET: u32 = 2;
const WINDOW_NANOS: u64 = 100;

struct Worker;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Worker {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }
}

fn worker(_index: usize) -> Worker {
    Worker
}

struct Parent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Worker>, error = Never)]
impl Parent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Worker>, Never> {
        Ok(Actions::cont())
    }
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let behavior = Supervisor::new(
            Parent,
            |index| u64::try_from(index).unwrap(),
            FLEET,
            |index| Some(worker(index)),
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            BUDGET,
            std::time::Duration::from_nanos(WINDOW_NANOS),
        )
        .unwrap()
        .with_failure_reaction(stop_on_supervision_failure);
        let initialized = behavior::Compose::new(behavior).initialize().unwrap();
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        // Independent budget/window model over the same event stream.
        let mut restarts: Vec<u64> = Vec::new();
        let mut alive = [true; FLEET];

        for (index, byte) in bytes.iter().copied().enumerate() {
            let nonce = usize::from(byte) % FLEET;
            // Deliberately non-monotone, equal, and backwards timestamps:
            // (index * 37) % 200 cycles 0..200 with duplicates and drops.
            let at = u64::try_from((index * 37) % 200).unwrap();

            let expected_restart = {
                restarts.retain(|stamp| *stamp > at || at - stamp <= WINDOW_NANOS);
                if restarts.len() + 1 <= BUDGET as usize {
                    restarts.push(at);
                    alive[nonce] = true;
                    true
                } else {
                    alive[nonce] = false;
                    false
                }
            };

            let actions = behavior
                .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                    proxy: u64::try_from(nonce).unwrap(),
                    worker: u64::try_from(nonce).unwrap(),
                    outcome: Err(Crash::Failed),
                    at: base + std::time::Duration::from_nanos(at),
                }))
                .unwrap();

            assert_eq!(
                actions.sends.replacement_commands.len(),
                usize::from(expected_restart),
                "replacement count mismatch at byte {index}"
            );
            if expected_restart {
                assert_eq!(actions.become_, Step::Continue);
            } else {
                assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
                assert_eq!(actions.sends.failure_reports.len(), 1);
                assert_eq!(
                    actions.sends.failure_reports[0].failure.reason,
                    SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                        restarts_in_window: restarts.len(),
                        replacements_requested: 1,
                        maximum_restarts: BUDGET,
                    })
                );
            }
            for slot in 0..FLEET {
                assert_eq!(
                    behavior.is_alive(u64::try_from(slot).unwrap()).unwrap(),
                    alive[slot],
                    "alive mismatch at byte {index}"
                );
            }
            assert_eq!(
                behavior.restarts_in_window(),
                restarts.len(),
                "restart-stamp count mismatch at byte {index}"
            );
        }
    });
});
