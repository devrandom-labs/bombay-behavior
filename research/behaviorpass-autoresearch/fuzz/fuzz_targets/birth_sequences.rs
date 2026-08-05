#![no_main]

//! Combined supervision attack surface: byte sequences interleave dynamic
//! births (user messages that create a fresh child) with child-stopped
//! deaths under a OneForOne/Permanent supervisor with a small restart
//! budget and no window pruning. An inline reference model tracks the slot
//! table (alive flags, birth order), the restart-stamp count, and the
//! budget, and every step's creates/observe-sends/replacement-sends/alive
//! state must agree.

use behaviorpass::{
    Acted, Actions, Base, Behavior, ChildStopped, Crash, Create, Delivery, MailAddr, Never,
    RestartPolicy, State, Step, Strategy, Supervising, SupervisionEvent, UserEvent,
};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;
use tokio::time::Instant;

const FLEET: usize = 2;
const BUDGET: u32 = 2;

struct Worker;

impl State<u8> for Worker {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        Ok(Actions::cont())
    }
}

fn worker(_index: usize) -> Base<Worker, u8> {
    Base::new(Worker)
}

struct BirthingParent;

impl State<Never, Base<Worker, u8>, Never> for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Base<Worker, u8>, Never> {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create {
                nonce,
                child: worker(0),
            }],
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
        let mut behavior = Supervising::new(
            Base::new(BirthingParent),
            |index| u64::try_from(index).unwrap(),
            FLEET,
            worker,
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            BUDGET,
            std::time::Duration::MAX,
        );
        behavior.init().await.unwrap();
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
                    .step(SupervisionEvent::Inner(UserEvent::user(MailAddr(0), nonce)))
                    .await
                    .unwrap();
                assert_eq!(actions.creates.len(), 1, "birth create at byte {index}");
                assert_eq!(actions.creates[0].nonce, nonce);
                assert_eq!(
                    actions.sends.own.inner.len(),
                    1,
                    "observe send at byte {index}"
                );
                assert_eq!(actions.sends.own.inner[0].message.nonce, nonce);
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
                    .step(SupervisionEvent::ChildStopped(ChildStopped {
                        nonce: dead.nonce,
                        outcome: Err(Crash::Failed),
                        at: base + std::time::Duration::from_nanos(u64::try_from(index).unwrap()),
                    }))
                    .await
                    .unwrap();
                assert_eq!(
                    actions.sends.own.own.len(),
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
                    behavior.is_alive(slot.nonce),
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
