#![no_main]

use std::time::Duration;

use behavior::{
    Actions, Activate, Behavior, ChildReport, CreationKind, Delivery, IncarnationPhase,
    InterruptionPolicy, JobId, KeyedPoolMessage, KeyedWorkerPool, KeyedWorkerPoolEvent, MailAddr,
    Never, NoBirths, PoolAssignment, PoolCompletion, PoolError, PoolFailure, PoolMessage,
    PoolResponse, Proxy, ProxyUnavailable, RebalanceRejection, Recipient, RestartPolicy,
    SupervisionEvent, User, WorkerCreationResolved, WorkerPhase, WorkerPool, WorkerPoolEvent,
    WorkerStopped,
};
use libfuzzer_sys::fuzz_target;
use std::time::Instant;

type Reply = bombay_behavior_fuzz::TestRecipient<PoolResponse<u8, u8, MailAddr>>;
struct Worker;
struct KeyedWorker;

impl behavior::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
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
    type Msg = PoolAssignment<u8>;
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

macro_rules! assert_common_pool_effects {
    ($actions:expr) => {{
        let actions = &$actions;
        assert!(actions.sends.supervision.child_observations.is_empty());
        assert!(actions.sends.supervision.creation_observations.is_empty());
        assert!(actions.sends.supervision.schedules.is_empty());
        assert!(actions.sends.supervision.shutdowns.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, behavior::Step::Continue));
    }};
}

fn fuzz_unavailability_join(control: u8) {
    let interruption = if control & 1 == 0 {
        InterruptionPolicy::Fail
    } else {
        InterruptionPolicy::Retry
    };
    let replacement_expected = control & 2 == 0;
    let initialized = WorkerPool::<MailAddr, u8, u8, Worker, Recipient<Reply>, _>::new(
        behavior::ChildTopology::new([0], |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            1,
            interruption,
            RestartPolicy::Permanent,
            u32::from(replacement_expected),
            Duration::from_secs(1), behavior::RestartTiming::Immediate
        ),
        Proxy::new,
    )
    .unwrap()
    .initialize()
    .unwrap();
    assert!(initialized.actions.sends.responses.is_empty());
    assert!(initialized.actions.sends.assignments.is_empty());
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .child_observations
            .len(),
        1
    );
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .creation_observations
            .len(),
        1
    );
    assert!(initialized.actions.sends.supervision.schedules.is_empty());
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .replacement_inputs
            .is_empty()
    );
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .failure_reports
            .is_empty()
    );
    assert!(initialized.actions.sends.supervision.shutdowns.is_empty());
    assert_eq!(initialized.actions.creates.len(), 1);
    assert!(matches!(
        initialized.actions.become_,
        behavior::Step::Continue
    ));
    let mut pool = initialized.behavior;
    let joined = pool
        .on_path(WorkerCreationResolved::new(
            0,
            0,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert!(joined.sends.responses.is_empty());
    assert!(joined.sends.assignments.is_empty());
    assert!(joined.sends.supervision.replacement_inputs.is_empty());
    assert!(joined.sends.supervision.failure_reports.is_empty());
    assert_common_pool_effects!(joined);
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
    assert_eq!(submitted.sends.responses.len(), 1);
    assert_eq!(submitted.sends.assignments.len(), 1);
    assert!(submitted.sends.supervision.replacement_inputs.is_empty());
    assert!(submitted.sends.supervision.failure_reports.is_empty());
    assert_common_pool_effects!(submitted);
    let assignment = submitted.sends.assignments.into_iter().next().unwrap();
    let command = assignment.message;
    let returned = ProxyUnavailable {
        proxy: 0,
        from: MailAddr(9),
        phase: IncarnationPhase::Vacant {
            last_installed: Some(0),
        },
        command,
    };
    let stopped = WorkerStopped::new(0, 0, Err(behavior::Crash::Failed), Instant::now());

    let mut terminal = 0;
    let mut redispatched = 0;
    let mut replacements = 0;
    let mut failures = 0;
    macro_rules! record {
        ($actions:expr) => {{
            let actions = &$actions;
            terminal += terminal_responses(&actions.sends.responses);
            redispatched += actions.sends.assignments.len();
            replacements += actions.sends.supervision.replacement_inputs.len();
            failures += actions.sends.supervision.failure_reports.len();
            assert_common_pool_effects!(actions);
        }};
    }
    if control & 4 == 0 {
        let first = pool
            .transition(SupervisionEvent::Behavior(
                WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
            ))
            .unwrap();
        record!(first);
        let second = pool.on_path(stopped).unwrap();
        record!(second);
        if replacement_expected {
            let ready = pool
                .on_path(WorkerCreationResolved::new(
                    0,
                    1,
                    CreationKind::replacement_of(0),
                    Ok(()),
                ))
                .unwrap();
            record!(ready);
        }
    } else {
        let first = pool.on_path(stopped).unwrap();
        record!(first);
        if replacement_expected && control & 8 != 0 {
            let ready = pool
                .on_path(WorkerCreationResolved::new(
                    0,
                    1,
                    CreationKind::replacement_of(0),
                    Ok(()),
                ))
                .unwrap();
            record!(ready);
        }
        let second = pool
            .transition(SupervisionEvent::Behavior(
                WorkerPoolEvent::AssignmentUnavailable(returned),
            ))
            .unwrap();
        record!(second);
        if replacement_expected && control & 8 == 0 {
            let ready = pool
                .on_path(WorkerCreationResolved::new(
                    0,
                    1,
                    CreationKind::replacement_of(0),
                    Ok(()),
                ))
                .unwrap();
            record!(ready);
        }
    }
    assert_eq!(replacements, usize::from(replacement_expected));
    assert_eq!(failures, usize::from(!replacement_expected));

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
    let pool = WorkerPool::<MailAddr, u8, u8, Worker, Recipient<Reply>, _>::new(
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
            Duration::from_secs(1), behavior::RestartTiming::Immediate
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = (pool).initialize().unwrap();
    assert!(initialized.actions.sends.responses.is_empty());
    assert!(initialized.actions.sends.assignments.is_empty());
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .child_observations
            .len(),
        2
    );
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .creation_observations
            .len(),
        2
    );
    assert!(initialized.actions.sends.supervision.schedules.is_empty());
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .replacement_inputs
            .is_empty()
    );
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .failure_reports
            .is_empty()
    );
    assert!(initialized.actions.sends.supervision.shutdowns.is_empty());
    assert_eq!(initialized.actions.creates.len(), 2);
    assert!(matches!(
        initialized.actions.become_,
        behavior::Step::Continue
    ));
    let mut pool = initialized.behavior;
    for slot in 0..2 {
        let joined = pool
            .on_path(WorkerCreationResolved::new(
                slot,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert!(joined.sends.responses.is_empty());
        assert!(joined.sends.assignments.is_empty());
        assert!(joined.sends.supervision.replacement_inputs.is_empty());
        assert!(joined.sends.supervision.failure_reports.is_empty());
        assert_common_pool_effects!(joined);
    }

    let mut next_job = 0_u64;
    let mut incarnations = [0_u64; 2];
    let mut next_incarnations = [1_u64; 2];
    for byte in bytes.iter().copied().take(512) {
        let slot = u64::from((byte >> 2) & 1);
        match byte & 3 {
            0 | 1 => {
                let actions = pool
                    .receive(
                        MailAddr(9),
                        PoolMessage::Submit {
                            job: JobId(next_job),
                            payload: byte,
                            reply_to: Recipient::<Reply>::global(MailAddr(10)),
                        },
                    )
                    .unwrap();
                assert_eq!(actions.sends.responses.len(), 1);
                assert!(actions.sends.assignments.len() <= 1);
                assert!(actions.sends.supervision.replacement_inputs.is_empty());
                assert!(actions.sends.supervision.failure_reports.is_empty());
                assert_common_pool_effects!(actions);
                next_job = next_job.wrapping_add(1);
            }
            2 => {
                if let Some(WorkerPhase::Assigned { assignment, .. }) = pool.worker_phase(slot) {
                    let actions = pool
                        .transition(SupervisionEvent::Behavior(WorkerPoolEvent::Completion(
                            ChildReport::new(
                                slot,
                                ChildReport::new(
                                    incarnations[usize::try_from(slot).unwrap()],
                                    PoolCompletion {
                                        assignment,
                                        result: byte,
                                    },
                                ),
                            ),
                        )))
                        .unwrap();
                    assert_eq!(actions.sends.responses.len(), 1);
                    assert!(actions.sends.assignments.len() <= 1);
                    assert!(actions.sends.supervision.replacement_inputs.is_empty());
                    assert!(actions.sends.supervision.failure_reports.is_empty());
                    assert_common_pool_effects!(actions);
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
                    assert!(actions.sends.responses.len() <= 1);
                    assert!(actions.sends.assignments.len() <= 1);
                    assert_eq!(actions.sends.supervision.replacement_inputs.len(), 1);
                    assert!(actions.sends.supervision.failure_reports.is_empty());
                    assert_common_pool_effects!(actions);
                    if !actions.sends.supervision.replacement_inputs.is_empty() {
                        let replacement = next_incarnations[position];
                        let installed = pool
                            .on_path(WorkerCreationResolved::new(
                                slot,
                                replacement,
                                CreationKind::replacement_of(incarnations[position]),
                                Ok(()),
                            ))
                            .unwrap();
                        assert!(installed.sends.responses.is_empty());
                        assert!(installed.sends.assignments.len() <= 1);
                        assert!(installed.sends.supervision.replacement_inputs.is_empty());
                        assert!(installed.sends.supervision.failure_reports.is_empty());
                        assert_common_pool_effects!(installed);
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

    let keyed = KeyedWorkerPool::<MailAddr, u8, u8, u8, KeyedWorker, Recipient<Reply>, _, _>::new(
        behavior::ChildTopology::indexed(nonce, 2, |index| Some(keyed_worker(index))),
        behavior::PoolConfiguration::new(
            4,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::from_secs(1), behavior::RestartTiming::Immediate
        ),
        affinity,
        Proxy::new,
    )
    .unwrap();
    let initialized = (keyed).initialize().unwrap();
    assert!(initialized.actions.sends.responses.is_empty());
    assert!(initialized.actions.sends.assignments.is_empty());
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .child_observations
            .len(),
        2
    );
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .creation_observations
            .len(),
        2
    );
    assert!(initialized.actions.sends.supervision.schedules.is_empty());
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .replacement_inputs
            .is_empty()
    );
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .failure_reports
            .is_empty()
    );
    assert!(initialized.actions.sends.supervision.shutdowns.is_empty());
    assert_eq!(initialized.actions.creates.len(), 2);
    assert!(matches!(
        initialized.actions.become_,
        behavior::Step::Continue
    ));
    let mut keyed = initialized.behavior;
    for slot in 0..2 {
        let joined = keyed
            .on_path(WorkerCreationResolved::new(
                slot,
                slot,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert!(joined.sends.responses.is_empty());
        assert!(joined.sends.assignments.is_empty());
        assert!(joined.sends.supervision.replacement_inputs.is_empty());
        assert!(joined.sends.supervision.failure_reports.is_empty());
        assert_common_pool_effects!(joined);
    }
    let mut incarnations = [0_u64, 1_u64];
    for (job, byte) in bytes.iter().copied().take(512).enumerate() {
        match byte % 4 {
            0 => {
                let actions = keyed
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
                assert_eq!(actions.sends.responses.len(), 1);
                assert!(actions.sends.assignments.len() <= 1);
                assert!(actions.sends.supervision.replacement_inputs.is_empty());
                assert!(actions.sends.supervision.failure_reports.is_empty());
                assert_common_pool_effects!(actions);
            }
            1 => {
                let key = byte >> 2;
                let worker = u64::from((byte >> 1) & 1);
                match keyed.receive(MailAddr(9), KeyedPoolMessage::Rebalance { key, worker }) {
                    Ok(actions) => {
                        assert!(actions.sends.responses.is_empty());
                        assert!(actions.sends.assignments.len() <= 1);
                        assert!(actions.sends.supervision.replacement_inputs.is_empty());
                        assert!(actions.sends.supervision.failure_reports.is_empty());
                        assert_common_pool_effects!(actions);
                    }
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
                    let actions = keyed
                        .transition(SupervisionEvent::Behavior(
                            KeyedWorkerPoolEvent::Completion(ChildReport::new(
                                slot,
                                ChildReport::new(
                                    incarnations[usize::try_from(slot).unwrap()],
                                    PoolCompletion {
                                        assignment,
                                        result: byte,
                                    },
                                ),
                            )),
                        ))
                        .unwrap();
                    assert_eq!(actions.sends.responses.len(), 1);
                    assert!(actions.sends.assignments.len() <= 1);
                    assert!(actions.sends.supervision.replacement_inputs.is_empty());
                    assert!(actions.sends.supervision.failure_reports.is_empty());
                    assert_common_pool_effects!(actions);
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
                    assert!(actions.sends.responses.len() <= 1);
                    assert!(actions.sends.assignments.len() <= 1);
                    assert_eq!(actions.sends.supervision.replacement_inputs.len(), 1);
                    assert!(actions.sends.supervision.failure_reports.is_empty());
                    assert_common_pool_effects!(actions);
                    if !actions.sends.supervision.replacement_inputs.is_empty() {
                        let replacement = stopped.wrapping_add(2);
                        let result = if byte & 0x80 == 0 {
                            Ok(())
                        } else {
                            Err(behavior::CreationRejection::EnvironmentFailed)
                        };
                        let installed = keyed
                            .on_path(WorkerCreationResolved::new(
                                nonce,
                                replacement,
                                CreationKind::replacement_of(stopped),
                                result,
                            ))
                            .unwrap();
                        if result.is_ok() {
                            assert!(installed.sends.responses.is_empty());
                        } else {
                            assert!(installed.sends.responses.len() <= 4);
                            assert!(installed.sends.responses.iter().all(|delivery| matches!(
                                delivery.message,
                                PoolResponse::Interrupted {
                                    reason: behavior::PoolInterruption::AffinityRetired {
                                        worker,
                                        ..
                                    },
                                    ..
                                } | PoolResponse::Interrupted {
                                    reason: behavior::PoolInterruption::WorkerStopped {
                                        worker,
                                        ..
                                    },
                                    ..
                                } if worker == nonce
                            )));
                        }
                        assert!(installed.sends.assignments.len() <= 1);
                        assert!(installed.sends.supervision.replacement_inputs.is_empty());
                        assert_eq!(
                            installed.sends.supervision.failure_reports.len(),
                            usize::from(result.is_err())
                        );
                        assert_common_pool_effects!(installed);
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
