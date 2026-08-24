#![no_main]

//! Supervision attack surface: arbitrary child-stopped byte sequences drive
//! a OneForOne/Permanent supervisor with a small restart budget and a small
//! window, while an inline reference model (documented semantics: lazy
//! window pruning with inclusive edge + future-stamp survival, budget counts
//! every replacement, denial kills the dead slot) tracks the expected
//! replacement sends, alive flags, and restart-stamp count. The impl must
//! agree on every byte.

use behavior::{
    Acted, Actions, Activate, Backoff, BackoffSupervisor, Crash, CreationKind, CreationRejection,
    CreationResolved, Delivery, DynamicSupervisor, DynamicSupervisorMessage,
    DynamicSupervisorOutcome, MailAddr, Never, Recipient, RestartDenial, RestartPolicy,
    ShutdownRequested, Step, Strategy, SupervisionEvent, SupervisionFailureReason, Supervisor,
    SupervisorError, TimerElapsed, TimerGeneration, TimerId, TopologyFailurePolicy,
    WorkerCreationResolved, WorkerStopped,
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

fn timer(nonce: u64) -> TimerId {
    TimerId(nonce)
}

type DynamicReply = bombay_behavior_fuzz::TestRecipient<DynamicSupervisorOutcome<MailAddr, Worker>>;

fn fuzz_dynamic_initial_join(bytes: &[u8]) {
    for byte in bytes.iter().copied() {
        let worker_first = byte & 1 != 0;
        let worker_rejected = byte & 2 != 0;
        let shutdown_point = (byte >> 2) & 3;
        let mut subject =
            DynamicSupervisor::<MailAddr, Worker, Recipient<DynamicReply>>::new()
                .initialize()
                .unwrap()
                .behavior;
        subject
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: Worker,
                    reply_to: Recipient::global(MailAddr(99)),
                },
            )
            .unwrap();

        let proxy = CreationResolved::birth(7, MailAddr(70));
        let worker = WorkerCreationResolved::new(
            7,
            0,
            CreationKind::Birth,
            if worker_rejected {
                Err(CreationRejection::EnvironmentFailed)
            } else {
                Ok(())
            },
        );
        let mut outcomes = 0;
        let mut shutdowns = 0;

        if shutdown_point == 0 {
            let actions = subject.on_path(ShutdownRequested).unwrap();
            outcomes += actions.sends.outcomes.len();
            shutdowns += actions.sends.shutdowns.len();
        }
        let first = if worker_first {
            subject.on_path(worker).unwrap()
        } else {
            subject.on_path(proxy).unwrap()
        };
        outcomes += first.sends.outcomes.len();
        shutdowns += first.sends.shutdowns.len();

        if shutdown_point == 1 {
            let actions = subject.on_path(ShutdownRequested).unwrap();
            outcomes += actions.sends.outcomes.len();
            shutdowns += actions.sends.shutdowns.len();
        }
        let second = if worker_first {
            subject.on_path(proxy).unwrap()
        } else {
            subject.on_path(worker).unwrap()
        };
        outcomes += second.sends.outcomes.len();
        shutdowns += second.sends.shutdowns.len();

        if shutdown_point >= 2 {
            let actions = subject.on_path(ShutdownRequested).unwrap();
            outcomes += actions.sends.outcomes.len();
            shutdowns += actions.sends.shutdowns.len();
        }

        assert_eq!(outcomes, 1);
        assert_eq!(shutdowns, 1);
        assert_eq!(second.sends.outcomes.len(), 1);
        if worker_rejected {
            assert!(matches!(
                second.sends.outcomes[0].message,
                DynamicSupervisorOutcome::StartFailed {
                    nonce: 7,
                    reason: CreationRejection::EnvironmentFailed,
                }
            ));
        } else {
            assert!(matches!(
                second.sends.outcomes[0].message,
                DynamicSupervisorOutcome::Started { nonce: 7, child }
                    if child.address() == MailAddr(70)
            ));
        }
    }
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        fuzz_dynamic_initial_join(bytes);
        let behavior = Supervisor::new(
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                FLEET,
                |index| Some(worker(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                BUDGET,
                std::time::Duration::from_nanos(WINDOW_NANOS),
            ),
        )
        .unwrap()
        .with_failure_policy(TopologyFailurePolicy::Stop);
        let initialized = (behavior).initialize().unwrap();
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        // Independent budget/window model over the same event stream.
        let mut restarts: Vec<u64> = Vec::new();
        let mut alive = [true; FLEET];
        let mut workers = [0_u64, 1, 2, 3];
        let mut next_worker = u64::try_from(FLEET).unwrap();

        for (index, byte) in bytes.iter().copied().enumerate() {
            let nonce = usize::from(byte) % FLEET;
            // Deliberately non-monotone, equal, and backwards timestamps:
            // (index * 37) % 200 cycles 0..200 with duplicates and drops.
            let at = u64::try_from((index * 37) % 200).unwrap();

            let was_alive = alive[nonce];
            let expected_restart = was_alive && {
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

            let observed = WorkerStopped {
                proxy: u64::try_from(nonce).unwrap(),
                worker: workers[nonce],
                outcome: Err(Crash::Failed),
                at: base + std::time::Duration::from_nanos(at),
            };
            let result = behavior.transition(SupervisionEvent::WorkerStopped(observed.clone()));

            if !was_alive {
                assert!(matches!(
                    result,
                    Err(SupervisorError::UnexpectedWorkerStopped(returned))
                        if returned == observed
                ));
                continue;
            }
            let actions = result.unwrap();

            assert_eq!(
                actions.sends.replacement_commands.len(),
                usize::from(expected_restart),
                "replacement count mismatch at byte {index}"
            );
            if expected_restart {
                assert_eq!(actions.become_, Step::Continue);
            } else if was_alive {
                assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
                assert_eq!(actions.sends.failure_reports.len(), 1);
                assert_eq!(
                    actions.sends.failure_reports[0].failure.reason(),
                    SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                        restarts_in_window: restarts.len(),
                        replacements_requested: 1,
                        maximum_restarts: BUDGET,
                    })
                );
            } else {
                assert_eq!(actions.become_, Step::Continue);
                assert!(actions.sends.failure_reports.is_empty());
            }
            if expected_restart {
                let proxy = u64::try_from(nonce).unwrap();
                let previous = workers[nonce];
                behavior
                    .on_path(WorkerCreationResolved::new(
                        proxy,
                        next_worker,
                        CreationKind::ReplacementIncarnation { replaces: previous },
                        Ok(()),
                    ))
                    .unwrap();
                workers[nonce] = next_worker;
                next_worker = next_worker.checked_add(1).unwrap();
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

        // Independent generation/pending model for the standalone backoff
        // adapter. Each input selects worker failure, exact timer, stale
        // timer, or shutdown; every step checks delayed release, cancellation,
        // and stale-timer rejection.
        let supervisor = Supervisor::new(
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                FLEET,
                |index| Some(worker(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                u32::MAX,
                std::time::Duration::MAX,
            ),
        )
        .unwrap();
        let initialized = BackoffSupervisor::new(
            supervisor,
            Backoff::exponential(
                std::time::Duration::from_nanos(1),
                std::time::Duration::from_nanos(8),
            )
            .unwrap(),
            timer,
        )
        .initialize()
        .unwrap();
        let mut backoff = initialized.behavior;
        let mut pending = [None; FLEET];
        let mut next_generation = [0_u64; FLEET];
        let mut backoff_workers = [0_u64, 1, 2, 3];
        let mut next_backoff_worker = u64::try_from(FLEET).unwrap();
        let mut shutting_down = false;
        let mut backoff_stopped = [false; FLEET];

        for (index, byte) in bytes.iter().copied().enumerate() {
            let slot = (usize::from(byte) / 4) % FLEET;
            let nonce = u64::try_from(slot).unwrap();
            match byte % 4 {
                0 => {
                    let observed = WorkerStopped::new(
                        nonce,
                        backoff_workers[slot],
                        Err(Crash::Failed),
                        base + std::time::Duration::from_nanos(
                            u64::try_from(index).unwrap(),
                        ),
                    );
                    let result = backoff.on_path(observed.clone());
                    let accepts_fact = !backoff_stopped[slot];
                    if !accepts_fact {
                        assert!(matches!(
                            result,
                            Err(behavior::BackoffSupervisorError::Supervision(
                                SupervisorError::UnexpectedWorkerStopped(returned)
                            )) if returned == observed
                        ));
                        continue;
                    }
                    let actions = result.unwrap();
                    backoff_stopped[slot] = true;
                    let scheduled = !shutting_down && pending[slot].is_none();
                    assert_eq!(actions.sends.schedules.len(), usize::from(scheduled));
                    assert!(actions.sends.supervision.replacement_commands.is_empty());
                    if scheduled {
                        let generation = next_generation[slot];
                        assert_eq!(
                            actions.sends.schedules.as_slice()[0].generation,
                            TimerGeneration(generation)
                        );
                        pending[slot] = Some(generation);
                        next_generation[slot] = generation.checked_add(1).unwrap();
                    }
                }
                1 => {
                    let generation = pending[slot].unwrap_or(u64::MAX);
                    let actions = backoff
                        .on_path(TimerElapsed::new(TimerId(nonce), TimerGeneration(generation)))
                        .unwrap();
                    let released = pending[slot].take().is_some() && !shutting_down;
                    assert_eq!(
                        actions.sends.supervision.replacement_commands.len(),
                        usize::from(released)
                    );
                    assert!(actions.sends.schedules.is_empty());
                    if released {
                        let previous = backoff_workers[slot];
                        backoff
                            .on_path(WorkerCreationResolved::new(
                                nonce,
                                next_backoff_worker,
                                CreationKind::ReplacementIncarnation { replaces: previous },
                                Ok(()),
                            ))
                            .unwrap();
                        backoff_workers[slot] = next_backoff_worker;
                        backoff_stopped[slot] = false;
                        next_backoff_worker = next_backoff_worker.checked_add(1).unwrap();
                    }
                }
                2 => {
                    let generation = pending[slot].map_or(0, |generation| generation + 1);
                    let actions = backoff
                        .on_path(TimerElapsed::new(TimerId(nonce), TimerGeneration(generation)))
                        .unwrap();
                    assert!(actions.sends.supervision.replacement_commands.is_empty());
                    assert!(actions.sends.schedules.is_empty());
                }
                _ => {
                    let actions = backoff.on_path(ShutdownRequested).unwrap();
                    pending.fill(None);
                    shutting_down = true;
                    assert!(actions.sends.schedules.is_empty());
                }
            }
            assert_eq!(
                backoff.pending_restarts(),
                pending.iter().filter(|generation| generation.is_some()).count()
            );
        }
    });
});
