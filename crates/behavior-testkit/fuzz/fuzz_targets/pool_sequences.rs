#![no_main]

use std::time::Duration;

use behavior::{
    Actions, Activate, Behavior, CreationKind, Delivery, IncarnationPhase, InterruptionPolicy,
    JobId, KeyedPoolMessage, KeyedWorkerPool, KeyedWorkerPoolProtocol, MailAddr, Never, NoBirths,
    PoolAssignment, PoolError, PoolFailure, PoolMessage, PoolResponse, ProxyCommand,
    ProxyUnavailable, RebalanceRejection, Recipient, RestartPolicy, User,
    WorkerCreationResolved, WorkerPhase, WorkerPool, WorkerPoolProtocol, WorkerStopped,
};
use libfuzzer_sys::fuzz_target;
use std::time::Instant;

type Reply = bombay_behavior_fuzz::TestRecipient<PoolResponse<u8, u8, MailAddr>>;
struct Worker;
struct KeyedWorker;

impl behavior::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<
        WorkerPoolProtocol<MailAddr, Reply, u8, u8, Recipient<Reply>>,
    >;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
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

impl behavior::Protocol for KeyedWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<
        KeyedWorkerPoolProtocol<MailAddr, Reply, u8, u8, u8, Recipient<Reply>>,
    >;
}

impl Behavior for KeyedWorker {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
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

fn keyed_worker(_index: usize) -> KeyedWorker {
    KeyedWorker
}

fn affinity(key: &u8) -> u64 {
    u64::from(key & 1)
}

fn terminal_responses(responses: &[Delivery<Reply>]) -> usize {
    responses
        .iter()
        .filter(|delivery| {
            matches!(
                delivery.message,
                PoolResponse::Completed { .. } | PoolResponse::Interrupted { .. }
            )
        })
        .count()
}

fn fuzz_unavailability_join(control: u8) {
    let interruption = if control & 1 == 0 {
        InterruptionPolicy::Fail
    } else {
        InterruptionPolicy::Retry
    };
    let replacement_expected = control & 2 == 0;
    let initialized = WorkerPool::new(
        behavior::ChildTopology::new([0], |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            1,
            interruption,
            RestartPolicy::Permanent,
            u32::from(replacement_expected),
            Duration::from_secs(1),
        ),
        Recipient::global(MailAddr(9)),
    )
    .unwrap()
    .initialize()
    .unwrap();
    let mut pool = initialized.behavior;
    pool.on_path(WorkerCreationResolved::new(
        0,
        0,
        CreationKind::Birth,
        Ok(()),
    ))
    .unwrap();
    let submitted = pool
        .receive(
            MailAddr(9),
            PoolMessage::Submit {
                job: JobId(1),
                payload: control,
                reply_to: Recipient::global(MailAddr(10)),
            },
        )
        .unwrap();
    let assignment = submitted
        .sends
        .inner
        .assignments
        .into_iter()
        .next()
        .unwrap();
    let ProxyCommand::Forward { command, .. } = assignment.message else {
        panic!("pool dispatch did not use the proxy forwarding protocol");
    };
    let returned = ProxyUnavailable {
        phase: IncarnationPhase::Vacant {
            last_installed: Some(0),
        },
        command,
    };
    let stopped = WorkerStopped::new(
        0,
        0,
        Err(behavior::Crash::Failed),
        Instant::now(),
    );

    let mut terminal = 0;
    let mut redispatched = 0;
    if control & 4 == 0 {
        let first = pool.on_path(User::new(MailAddr(0), returned)).unwrap();
        terminal += terminal_responses(&first.sends.inner.responses);
        redispatched += first.sends.inner.assignments.len();
        let second = pool.on_path(stopped).unwrap();
        terminal += terminal_responses(&second.sends.inner.responses);
        redispatched += second.sends.inner.assignments.len();
        if replacement_expected {
            let ready = pool
                .on_path(WorkerCreationResolved::new(
                    0,
                    1,
                    CreationKind::replacement_of(0),
                    Ok(()),
                ))
                .unwrap();
            terminal += terminal_responses(&ready.sends.inner.responses);
            redispatched += ready.sends.inner.assignments.len();
        }
    } else {
        let first = pool.on_path(stopped).unwrap();
        terminal += terminal_responses(&first.sends.inner.responses);
        redispatched += first.sends.inner.assignments.len();
        if replacement_expected && control & 8 != 0 {
            let ready = pool
                .on_path(WorkerCreationResolved::new(
                    0,
                    1,
                    CreationKind::replacement_of(0),
                    Ok(()),
                ))
                .unwrap();
            terminal += terminal_responses(&ready.sends.inner.responses);
            redispatched += ready.sends.inner.assignments.len();
        }
        let second = pool.on_path(User::new(MailAddr(0), returned)).unwrap();
        terminal += terminal_responses(&second.sends.inner.responses);
        redispatched += second.sends.inner.assignments.len();
        if replacement_expected && control & 8 == 0 {
            let ready = pool
                .on_path(WorkerCreationResolved::new(
                    0,
                    1,
                    CreationKind::replacement_of(0),
                    Ok(()),
                ))
                .unwrap();
            terminal += terminal_responses(&ready.sends.inner.responses);
            redispatched += ready.sends.inner.assignments.len();
        }
    }

    match (interruption, replacement_expected) {
        (InterruptionPolicy::Fail, _) | (InterruptionPolicy::Retry, false) => {
            assert_eq!(terminal, 1);
            assert_eq!(redispatched, 0);
        }
        (InterruptionPolicy::Retry, true) => {
            assert_eq!(terminal, 0);
            assert_eq!(redispatched, 1);
        }
    }
}

fuzz_target!(|bytes: &[u8]| {
    fuzz_unavailability_join(bytes.first().copied().unwrap_or(0));
    let pool = WorkerPool::new(
        behavior::ChildTopology::indexed(nonce, 2, |index| Some(worker(index))),
        behavior::PoolConfiguration::new(
            4,
            if bytes.first().is_some_and(|byte| byte & 1 == 0) {
                InterruptionPolicy::Fail
            } else {
                InterruptionPolicy::Retry
            },
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::from_secs(1),
        ),
        Recipient::global(MailAddr(9)),
    )
    .unwrap();
    let initialized = (pool).initialize().unwrap();
    let mut pool = initialized.behavior;
    for slot in 0..2 {
        pool.on_path(WorkerCreationResolved::new(
            slot,
            0,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    }

    let mut next_job = 0_u64;
    let mut incarnations = [0_u64; 2];
    let mut next_incarnations = [1_u64; 2];
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
                    let position = usize::try_from(slot).unwrap();
                    let stopped = WorkerStopped::new(
                        slot,
                        incarnations[position],
                        Err(behavior::Crash::Panicked),
                        Instant::now(),
                    );
                    let actions = pool.on_path(stopped.clone()).unwrap();
                    if !actions.sends.owned.replacement_commands.is_empty() {
                        let replacement = next_incarnations[position];
                        pool.on_path(WorkerCreationResolved::new(
                            slot,
                            replacement,
                            CreationKind::replacement_of(incarnations[position]),
                            Ok(()),
                        ))
                        .unwrap();
                        incarnations[position] = replacement;
                        next_incarnations[position] = replacement.checked_add(1).unwrap();
                        if byte & 0x80 != 0 {
                            let duplicate = pool.on_path(stopped.clone());
                            assert!(matches!(
                                duplicate,
                                Err(PoolFailure::Infrastructure(
                                    PoolError::UnexpectedWorkerStopped(returned)
                                )) if returned == stopped
                            ));
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    let keyed = KeyedWorkerPool::new(
        behavior::ChildTopology::indexed(nonce, 2, |index| Some(keyed_worker(index))),
        behavior::PoolConfiguration::new(
            4,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::from_secs(1),
        ),
        affinity,
        Recipient::global(MailAddr(9)),
    )
    .unwrap();
    let initialized = (keyed).initialize().unwrap();
    let mut keyed = initialized.behavior;
    for slot in 0..2 {
        keyed
            .on_path(WorkerCreationResolved::new(
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
                let key = byte >> 2;
                let worker = u64::from((byte >> 1) & 1);
                match keyed.receive(
                    MailAddr(9),
                    KeyedPoolMessage::Rebalance { key, worker },
                ) {
                    Ok(_) => {}
                    Err(PoolFailure::Rebalance(
                        RebalanceRejection::UnknownWorker {
                            key: returned_key,
                            worker: returned_worker,
                        }
                        | RebalanceRejection::RetiredWorker {
                            key: returned_key,
                            worker: returned_worker,
                            ..
                        }
                        | RebalanceRejection::ShuttingDown {
                            key: returned_key,
                            worker: returned_worker,
                        },
                    )) => {
                        assert_eq!(returned_key, key);
                        assert_eq!(returned_worker, worker);
                    }
                    Err(other) => panic!("rebalance produced the wrong rejection: {other:?}"),
                }
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
                    let stopped_fact = WorkerStopped::new(
                        nonce,
                        stopped,
                        Err(behavior::Crash::Panicked),
                        Instant::now(),
                    );
                    let actions = keyed.on_path(stopped_fact.clone()).unwrap();
                    if !actions.sends.owned.replacement_commands.is_empty() {
                        let replacement = stopped.wrapping_add(2);
                        let result = if byte & 0x80 == 0 {
                            Ok(())
                        } else {
                            Err(behavior::CreationRejection::EnvironmentFailed)
                        };
                        keyed
                            .on_path(WorkerCreationResolved::new(
                                nonce,
                                replacement,
                                CreationKind::replacement_of(stopped),
                                result,
                            ))
                            .unwrap();
                        if result.is_ok() {
                            incarnations[slot] = replacement;
                            if byte & 0x40 != 0 {
                                let duplicate = keyed.on_path(stopped_fact.clone());
                                assert!(matches!(
                                    duplicate,
                                    Err(PoolFailure::Infrastructure(
                                        PoolError::UnexpectedWorkerStopped(returned)
                                    )) if returned == stopped_fact
                                ));
                            }
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }
});
