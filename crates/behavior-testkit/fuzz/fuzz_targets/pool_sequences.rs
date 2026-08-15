#![no_main]

use std::time::Duration;

use behavior::{
    Actions, Activate, Behavior, CreationKind, InterruptionPolicy, JobId, KeyedPoolMessage,
    KeyedWorkerPool, MailAddr, Never, NoBirths, PoolAssignment, PoolMessage, PoolResponse, Recipient,
    RestartPolicy, User, WorkerCreationResolved, WorkerPhase, WorkerPool, WorkerStopped,
};
use libfuzzer_sys::fuzz_target;
use std::time::Instant;

type Reply = bombay_behavior_fuzz::TestRecipient<PoolResponse<u8, u8, MailAddr>>;

struct Worker;

impl Behavior for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
    type Event = User<MailAddr, PoolAssignment<u8>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

fn worker(_index: usize) -> Worker {
    Worker
}

fn affinity(key: &u8) -> u64 {
    u64::from(key & 1)
}

fuzz_target!(|bytes: &[u8]| {
    let pool = WorkerPool::new(
        nonce,
        2,
        |index| Some(worker(index)),
        4,
        if bytes.first().is_some_and(|byte| byte & 1 == 0) {
            InterruptionPolicy::Fail
        } else {
            InterruptionPolicy::Retry
        },
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::from_secs(1),
    )
    .unwrap();
    let initialized = (pool).initialize().unwrap();
    let mut pool = initialized.behavior;
    for slot in 0..2 {
        pool.on(WorkerCreationResolved::new(
            slot,
            0,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    }

    let mut next_job = 0_u64;
    for byte in bytes.iter().copied().take(512) {
        let slot = u64::from((byte >> 2) & 1);
        match byte & 3 {
            0 | 1 => {
                pool.receive(
                    MailAddr(9),
                    PoolMessage::Submit {
                        job: JobId(next_job),
                        payload: byte,
                        reply_to: Recipient::<Reply>::global(MailAddr(10)),
                    },
                )
                .unwrap();
                next_job = next_job.wrapping_add(1);
            }
            2 => {
                if let Some(WorkerPhase::Assigned { assignment, .. }) = pool.worker_phase(slot) {
                    pool.receive(
                        MailAddr(9),
                        PoolMessage::Completed {
                            worker: slot,
                            assignment,
                            result: byte,
                        },
                    )
                    .unwrap();
                }
            }
            3 => {
                if matches!(
                    pool.worker_phase(slot),
                    Some(WorkerPhase::Idle | WorkerPhase::Assigned { .. })
                ) {
                    let actions = pool
                        .on(WorkerStopped::new(
                            slot,
                            0,
                            Err(behavior::Crash::Panicked),
                            Instant::now(),
                        ))
                        .unwrap();
                    if !actions.sends.replacement_commands.is_empty() {
                        pool.on(WorkerCreationResolved::new(
                            slot,
                            1,
                            CreationKind::replacement_of(0),
                            Ok(()),
                        ))
                        .unwrap();
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    let keyed = KeyedWorkerPool::new(
        nonce,
        2,
        |index| Some(worker(index)),
        4,
        InterruptionPolicy::Retry,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::from_secs(1),
        affinity,
    )
    .unwrap();
    let initialized = (keyed).initialize().unwrap();
    let mut keyed = initialized.behavior;
    for slot in 0..2 {
        keyed
            .on(WorkerCreationResolved::new(
                slot,
                slot,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
    }
    let mut incarnations = [0_u64, 1_u64];
    for (job, byte) in bytes.iter().copied().take(512).enumerate() {
        match byte % 4 {
            0 => {
                keyed
                    .receive(
                        MailAddr(9),
                        KeyedPoolMessage::Submit {
                            key: byte >> 2,
                            job: JobId(u64::try_from(job).unwrap()),
                            payload: byte,
                            reply_to: Recipient::<Reply>::global(MailAddr(10)),
                        },
                    )
                    .unwrap();
            }
            1 => {
                let _ = keyed.receive(
                    MailAddr(9),
                    KeyedPoolMessage::Rebalance {
                        key: byte >> 2,
                        worker: u64::from((byte >> 1) & 1),
                    },
                );
            }
            2 => {
                let slot = u64::from((byte >> 1) & 1);
                if let Some(WorkerPhase::Assigned { assignment, .. }) = keyed.worker_phase(slot) {
                    keyed
                        .receive(
                            MailAddr(9),
                            KeyedPoolMessage::Completed {
                                worker: slot,
                                assignment,
                                result: byte,
                            },
                        )
                        .unwrap();
                }
            }
            3 => {
                let slot = usize::from((byte >> 1) & 1);
                let nonce = u64::try_from(slot).unwrap();
                if matches!(
                    keyed.worker_phase(nonce),
                    Some(WorkerPhase::Idle | WorkerPhase::Assigned { .. })
                ) {
                    let stopped = incarnations[slot];
                    let actions = keyed
                        .on(WorkerStopped::new(
                            nonce,
                            stopped,
                            Err(behavior::Crash::Panicked),
                            Instant::now(),
                        ))
                        .unwrap();
                    if !actions.sends.replacement_commands.is_empty() {
                        let replacement = stopped.wrapping_add(2);
                        let result = if byte & 0x80 == 0 {
                            Ok(())
                        } else {
                            Err(behavior::CreationRejection::EnvironmentFailed)
                        };
                        keyed
                            .on(WorkerCreationResolved::new(
                                nonce,
                                replacement,
                                CreationKind::replacement_of(stopped),
                                result,
                            ))
                            .unwrap();
                        if result.is_ok() {
                            incarnations[slot] = replacement;
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }
});
