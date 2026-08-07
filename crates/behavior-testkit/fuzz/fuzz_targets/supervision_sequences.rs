#![no_main]

//! Supervision attack surface: arbitrary child-stopped byte sequences drive
//! a OneForOne/Permanent supervisor with a small restart budget and a small
//! window, while an inline reference model (documented semantics: lazy
//! window pruning with inclusive edge + future-stamp survival, budget counts
//! every replacement, denial kills the dead slot) tracks the expected
//! replacement sends, alive flags, and restart-stamp count. The impl must
//! agree on every byte.

use behavior::{
    Acted, Actions, Base, Behavior, WorkerStopped, Crash, Delivery, MailAddr, Never, RestartPolicy,
    State, Strategy, Supervising, SupervisionEvent,
};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;
use tokio::time::Instant;

const FLEET: usize = 4;
const BUDGET: u32 = 2;
const WINDOW_NANOS: u64 = 100;

struct Worker;

impl State<u8> for Worker {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
    }
}

fn worker(_index: usize) -> Base<Worker, u8> {
    Base::new(Worker)
}

struct Parent;

impl State<Never, behavior::Births<Base<Worker, u8>>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Base<Worker, u8>>, Never> {
        Ok(Actions::cont())
    }
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let mut behavior = Supervising::new(
            Base::new(Parent),
            |index| u64::try_from(index).unwrap(),
            FLEET,
            worker,
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            BUDGET,
            std::time::Duration::from_nanos(WINDOW_NANOS),
        );
        behavior.init().await.unwrap();
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
                .step(SupervisionEvent::WorkerStopped(WorkerStopped {
                    proxy: u64::try_from(nonce).unwrap(),
                    outcome: Err(Crash::Failed),
                    at: base + std::time::Duration::from_nanos(at),
                }))
                .await
                .unwrap();

            assert_eq!(
                actions.sends.own.own.len(),
                usize::from(expected_restart),
                "replacement count mismatch at byte {index}"
            );
            for slot in 0..FLEET {
                assert_eq!(
                    behavior.is_alive(u64::try_from(slot).unwrap()),
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
