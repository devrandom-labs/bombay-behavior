#![no_main]

//! Combined supervision attack surface: byte sequences interleave dynamic
//! births (user messages that create a fresh child) with child-stopped
//! deaths under a OneForOne/Permanent supervisor with a small restart
//! budget and no window pruning. An inline reference model tracks the slot
//! table (alive flags, birth order), the restart-stamp count, and the
//! budget, and every step's creates/observe-sends/replacement-sends/alive
//! state must agree.

use behavior::{
    Acted, Actions, Activate, Crash, Create, CreationKind, Delivery, MailAddr, Never,
    RestartPolicy, Step, Strategy, SupervisionEvent, Supervisor, UserEvent, WorkerStopped,
};
use libfuzzer_sys::fuzz_target;
use std::time::Instant;
use tokio::runtime::Builder;

const FLEET: usize = 2;
const BUDGET: u32 = 2;

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

struct BirthingParent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Worker>, error = Never)]
impl BirthingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Worker>, Never> {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::birth(nonce, worker(0))],
            become_: Step::Continue,
        })
    }
}

#[derive(Clone, Copy)]
struct Slot {
    nonce: u64,
    alive: bool,
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let behavior = Supervisor::new(
            BirthingParent,
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                FLEET,
                |index| Some(worker(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                BUDGET,
                std::time::Duration::MAX,
            ),
        )
        .unwrap();
        let initialized = (behavior).initialize().unwrap();
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        // Independent model: slot table in birth order, restart stamps.
        let mut slots: Vec<Slot> = (0..FLEET)
            .map(|index| Slot {
                nonce: u64::try_from(index).unwrap(),
                alive: true,
            })
            .collect();
        let mut births: u64 = u64::try_from(FLEET).unwrap();
        let mut restarts: Vec<u64> = Vec::new();

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                // Dynamic birth with a fresh nonce.
                let nonce = births;
                births += 1;
                slots.push(Slot { nonce, alive: true });
                let actions = behavior
                    .transition(SupervisionEvent::Behavior(UserEvent::user(
                        MailAddr(0),
                        nonce,
                    )))
                    .unwrap();
                assert_eq!(actions.creates.len(), 1, "birth create at byte {index}");
                assert_eq!(actions.creates[0].nonce, nonce);
                assert_eq!(actions.creates[0].kind, CreationKind::Birth);
                assert_eq!(
                    actions.sends.owned.child_observations.len(),
                    1,
                    "observe request at byte {index}"
                );
                assert_eq!(actions.sends.owned.child_observations[0].nonce, nonce);
            } else {
                // Death of the slot selected by the byte.
                let dead = slots[usize::from(byte) % slots.len()];
                let expected_restart = {
                    if restarts.len() + 1 <= BUDGET as usize {
                        restarts.push(u64::try_from(index).unwrap());
                        true
                    } else {
                        false
                    }
                };
                let actions = behavior
                    .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                        proxy: dead.nonce,
                        worker: dead.nonce,
                        outcome: Err(Crash::Failed),
                        at: base + std::time::Duration::from_nanos(u64::try_from(index).unwrap()),
                    }))
                    .unwrap();
                assert_eq!(
                    actions.sends.owned.replacement_commands.len(),
                    usize::from(expected_restart),
                    "replacement count at byte {index}"
                );
                let idx = slots
                    .iter()
                    .position(|slot| slot.nonce == dead.nonce)
                    .unwrap();
                slots[idx].alive = expected_restart;
            }

            for slot in &slots {
                assert_eq!(
                    behavior.is_alive(slot.nonce).unwrap(),
                    slot.alive,
                    "alive mismatch at byte {index} for nonce {}",
                    slot.nonce
                );
            }
            assert_eq!(
                behavior.child_count(),
                slots.len(),
                "child count at byte {index}"
            );
            assert_eq!(
                behavior.restarts_in_window(),
                restarts.len(),
                "restart stamps at byte {index}"
            );
        }
    });
});
