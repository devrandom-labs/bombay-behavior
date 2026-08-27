#![no_main]

//! Combined supervision attack surface: byte sequences interleave inner
//! application births with worker deaths in a fixed OneForOne/Permanent fleet.
//! The independent model keeps application births outside the supervisor-owned
//! topology and tracks the fixed slot table, incarnations, restart stamps, and
//! budget. Every step's complete transition effects and ownership state must
//! agree.

use behavior::{
    Acted, Actions, Activate, Crash, Create, CreationKind, CreationResolved, Delivery, MailAddr,
    Never, RestartDenial, RestartPolicy, Step, Strategy, Supervise, SuperviseError,
    SupervisionEvent, SupervisionFailure, UserEvent, WorkerCreationResolved, WorkerStopped,
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
    worker: u64,
    next_worker: u64,
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let behavior = Supervise::new(
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
                std::time::Duration::MAX, behavior::RestartTiming::Immediate
            ),
            behavior::Proxy::new,
        )
        .unwrap();
        let initialized = (behavior).initialize().unwrap();
        assert_eq!(
            initialized
                .actions
                .creates
                .iter()
                .map(|create| create.nonce)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(
            initialized
                .actions
                .creates
                .iter()
                .all(|create| create.kind == CreationKind::Birth)
        );
        assert_eq!(
            initialized.actions.sends.owned.child_observations.len(),
            FLEET
        );
        assert_eq!(
            initialized.actions.sends.owned.creation_observations.len(),
            FLEET
        );
        assert!(initialized.actions.sends.owned.schedules.is_empty());
        assert!(
            initialized
                .actions
                .sends
                .owned
                .replacement_inputs
                .is_empty()
        );
        assert!(initialized.actions.sends.owned.failure_reports.is_empty());
        assert!(initialized.actions.sends.owned.shutdowns.is_empty());
        assert!(initialized.actions.sends.inner.is_empty());
        assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        // Independent model: slot table in birth order, restart stamps.
        let mut slots: Vec<Slot> = (0..FLEET)
            .map(|index| Slot {
                nonce: u64::try_from(index).unwrap(),
                alive: true,
                worker: 0,
                next_worker: 1,
            })
            .collect();
        for slot in &slots {
            let stable = behavior
                .transition(SupervisionEvent::CreationResolved(CreationResolved::birth(
                    slot.nonce,
                    MailAddr(slot.nonce + 100),
                )))
                .unwrap();
            assert!(stable.sends.owned.child_observations.is_empty());
            assert!(stable.sends.owned.creation_observations.is_empty());
            assert!(stable.sends.owned.schedules.is_empty());
            assert!(stable.sends.owned.replacement_inputs.is_empty());
            assert!(stable.sends.owned.failure_reports.is_empty());
            assert!(stable.sends.owned.shutdowns.is_empty());
            assert!(stable.sends.inner.is_empty());
            assert!(stable.creates.is_empty());
            assert!(matches!(stable.become_, Step::Continue));
            let worker = behavior
                .transition(SupervisionEvent::WorkerCreationResolved(
                    WorkerCreationResolved::new(
                        slot.nonce,
                        slot.worker,
                        CreationKind::Birth,
                        Ok(()),
                    ),
                ))
                .unwrap();
            assert!(worker.sends.owned.child_observations.is_empty());
            assert!(worker.sends.owned.creation_observations.is_empty());
            assert!(worker.sends.owned.schedules.is_empty());
            assert!(worker.sends.owned.replacement_inputs.is_empty());
            assert!(worker.sends.owned.failure_reports.is_empty());
            assert!(worker.sends.owned.shutdowns.is_empty());
            assert!(worker.sends.inner.is_empty());
            assert!(worker.creates.is_empty());
            assert!(matches!(worker.become_, Step::Continue));
        }
        let mut births: u64 = u64::try_from(FLEET).unwrap();
        let mut restarts: Vec<u64> = Vec::new();

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                // Dynamic birth with a fresh nonce.
                let nonce = births;
                births += 1;
                let actions = behavior
                    .transition(SupervisionEvent::Behavior(UserEvent::user(
                        MailAddr(0),
                        nonce,
                    )))
                    .unwrap();
                assert_eq!(actions.creates.len(), 1, "birth create at byte {index}");
                assert_eq!(actions.creates[0].nonce, nonce);
                assert_eq!(actions.creates[0].kind, CreationKind::Birth);
                assert!(actions.sends.owned.child_observations.is_empty());
                assert!(actions.sends.owned.creation_observations.is_empty());
                assert!(actions.sends.owned.schedules.is_empty());
                assert!(actions.sends.owned.replacement_inputs.is_empty());
                assert!(actions.sends.owned.failure_reports.is_empty());
                assert!(actions.sends.owned.shutdowns.is_empty());
                assert!(actions.sends.inner.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
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
                let stopped = WorkerStopped {
                    proxy: dead.nonce,
                    worker: dead.worker,
                    outcome: Err(Crash::Failed),
                    at: base + std::time::Duration::from_nanos(u64::try_from(index).unwrap()),
                };
                let actions = behavior
                    .transition(SupervisionEvent::WorkerStopped(stopped.clone()))
                    .unwrap();
                assert_eq!(
                    actions.sends.owned.replacement_inputs.len(),
                    usize::from(expected_restart),
                    "replacement count at byte {index}"
                );
                assert!(actions.sends.owned.child_observations.is_empty());
                assert!(actions.sends.owned.creation_observations.is_empty());
                assert!(actions.sends.owned.schedules.is_empty());
                assert_eq!(
                    actions.sends.owned.failure_reports.len(),
                    usize::from(!expected_restart),
                    "failure report count at byte {index}"
                );
                if !expected_restart {
                    assert_eq!(
                        actions.sends.owned.failure_reports.as_slice()[0].failure,
                        SupervisionFailure::restart_denied(
                            dead.nonce,
                            Err(Crash::Failed),
                            RestartDenial::BudgetExceeded {
                                restarts_in_window: BUDGET as usize,
                                replacements_requested: 1,
                                maximum_restarts: BUDGET,
                            },
                        )
                    );
                }
                assert!(actions.sends.owned.shutdowns.is_empty());
                assert!(actions.sends.inner.is_empty());
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
                let idx = slots
                    .iter()
                    .position(|slot| slot.nonce == dead.nonce)
                    .unwrap();
                slots[idx].alive = expected_restart;
                if expected_restart {
                    let replacement = slots[idx].next_worker;
                    let installed = behavior
                        .transition(SupervisionEvent::WorkerCreationResolved(
                            WorkerCreationResolved::new(
                                dead.nonce,
                                replacement,
                                CreationKind::replacement_of(dead.worker),
                                Ok(()),
                            ),
                        ))
                        .unwrap();
                    assert!(installed.sends.owned.child_observations.is_empty());
                    assert!(installed.sends.owned.creation_observations.is_empty());
                    assert!(installed.sends.owned.schedules.is_empty());
                    assert!(installed.sends.owned.replacement_inputs.is_empty());
                    assert!(installed.sends.owned.failure_reports.is_empty());
                    assert!(installed.sends.owned.shutdowns.is_empty());
                    assert!(installed.sends.inner.is_empty());
                    assert!(installed.creates.is_empty());
                    assert!(matches!(installed.become_, Step::Continue));
                    slots[idx].worker = replacement;
                    slots[idx].next_worker = replacement.checked_add(1).unwrap();

                    if byte & 2 != 0 {
                        let duplicate =
                            behavior.transition(SupervisionEvent::WorkerStopped(stopped.clone()));
                        assert!(matches!(
                            duplicate,
                            Err(SuperviseError::UnexpectedWorkerStopped(returned))
                                if returned == stopped
                        ));
                    }
                }
            }

            for slot in &slots {
                assert_eq!(
                    behavior.is_restartable(slot.nonce).unwrap(),
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
            if !behavior
                .is_restartable(slots[usize::from(byte) % slots.len()].nonce)
                .unwrap()
            {
                break;
            }
        }
    });
});
