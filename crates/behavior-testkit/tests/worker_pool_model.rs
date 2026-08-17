use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use behavior::{
    Actions, AssignmentId, Behavior, CreationKind, CreationRejection, Delivery, InterruptionPolicy,
    JobId, MailAddr, Never, NoBirths, PoolAssignment, PoolConfigError, PoolError, PoolMessage,
    PoolResponse, Proxy, ProxyCommand, Recipient, RestartPolicy, Step, User,
    WorkerCreationResolved, WorkerPhase, WorkerPool, WorkerStopped,
};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Worker;

struct Reply;

impl Behavior for Reply {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, u16, MailAddr>;
    type Event = User<MailAddr, Self::Msg>;
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

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

struct PanicPayload {
    panic_on_clone: Arc<AtomicBool>,
}

impl Clone for PanicPayload {
    fn clone(&self) -> Self {
        assert!(
            !self.panic_on_clone.load(Ordering::SeqCst),
            "adversarial clone"
        );
        Self {
            panic_on_clone: self.panic_on_clone.clone(),
        }
    }
}

struct PanicWorker;

struct PanicReply;

impl Behavior for PanicReply {
    type Addr = MailAddr;
    type Msg = PoolResponse<PanicPayload, (), MailAddr>;
    type Event = User<MailAddr, Self::Msg>;
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

impl Behavior for PanicWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<PanicPayload>;
    type Event = User<MailAddr, PoolAssignment<PanicPayload>>;
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn panic_worker(_: usize) -> PanicWorker {
    PanicWorker
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

fn worker(_index: usize) -> Worker {
    Worker
}

fn pool(
    workers: usize,
    capacity: usize,
    interruption: InterruptionPolicy,
) -> WorkerPool<MailAddr, Reply, u8, u16, Worker> {
    WorkerPool::new(
        behavior::ChildTopology::indexed(nonce, workers, |index| Some(worker(index))),
        behavior::PoolConfiguration::new(
            capacity,
            interruption,
            RestartPolicy::Permanent,
            64,
            Duration::from_secs(60),
        ),
    )
    .unwrap()
}

fn install(
    pool: &mut behavior::Active<WorkerPool<MailAddr, Reply, u8, u16, Worker>>,
    slot: u64,
) -> behavior::PoolActions<MailAddr, Reply, u8, u16, Worker> {
    pool.on(WorkerCreationResolved::new(
        slot,
        0,
        CreationKind::Birth,
        Ok(()),
    ))
    .unwrap()
}

fn submit(
    pool: &mut behavior::Active<WorkerPool<MailAddr, Reply, u8, u16, Worker>>,
    id: u64,
    payload: u8,
) -> behavior::PoolActions<MailAddr, Reply, u8, u16, Worker> {
    pool.receive(
        MailAddr(90),
        PoolMessage::Submit {
            job: JobId(id),
            payload,
            reply_to: Recipient::global(MailAddr(91)),
        },
    )
    .unwrap()
}

fn assignments(
    actions: &behavior::PoolActions<MailAddr, Reply, u8, u16, Worker>,
) -> &[Delivery<Proxy<Worker>>] {
    &actions.sends.behavior.assignments
}

fn responses(
    actions: &behavior::PoolActions<MailAddr, Reply, u8, u16, Worker>,
) -> &[Delivery<Reply>] {
    &actions.sends.behavior.responses
}

#[test]
fn initialization_stages_and_observes_every_stable_proxy_before_dispatch() {
    let pool = pool(2, 4, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let actions = initialized.actions;
    let pool = initialized.behavior;
    assert_eq!(actions.creates.len(), 2);
    assert_eq!(actions.sends.child_observations.len(), 2);
    assert_eq!(actions.sends.creation_observations.len(), 2);
    for (creation, observation) in actions
        .creates
        .iter()
        .zip(actions.sends.creation_observations.iter())
    {
        assert_eq!(creation.nonce, observation.nonce);
    }
    assert!(assignments(&actions).is_empty());
    assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Installing));
    assert_eq!(pool.worker_phase(1), Some(WorkerPhase::Installing));
}

#[test]
fn accepted_job_is_recorded_before_one_exact_dispatch() {
    let pool = pool(1, 0, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install(&mut pool, 0);

    let actions = submit(&mut pool, 7, 42);
    assert!(matches!(
        responses(&actions)[0].message,
        PoolResponse::Accepted { job: JobId(7) }
    ));
    let ProxyCommand::Forward(assignment) = &assignments(&actions)[0].message else {
        panic!("pool dispatches with Forward");
    };
    assert_eq!(assignment.assignment, AssignmentId(0));
    assert_eq!(assignment.job, JobId(7));
    assert_eq!(assignment.payload, 42);
    assert_eq!(
        assignments(&actions)[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Assigned {
            assignment: AssignmentId(0),
            job: JobId(7),
        })
    );
}

#[test]
fn full_backlog_returns_the_unaccepted_owned_job() {
    let pool = pool(1, 1, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    let accepted = submit(&mut pool, 1, 10);
    assert!(matches!(
        responses(&accepted)[0].message,
        PoolResponse::Accepted { .. }
    ));
    let rejected = submit(&mut pool, 2, 20);
    assert!(matches!(
        responses(&rejected)[0].message,
        PoolResponse::Rejected {
            job: JobId(2),
            payload: 20,
            reason: behavior::PoolRejection::BacklogFull,
        }
    ));
    assert_eq!(pool.backlog_len(), 1);
}

#[test]
fn matching_completion_releases_slot_and_dispatches_fifo_successor() {
    let pool = pool(1, 2, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install(&mut pool, 0);
    submit(&mut pool, 1, 10);
    submit(&mut pool, 2, 20);

    let actions = pool
        .receive(
            MailAddr(0),
            PoolMessage::Completed {
                worker: 0,
                assignment: AssignmentId(0),
                result: 99,
            },
        )
        .unwrap();
    assert!(matches!(
        responses(&actions)[0].message,
        PoolResponse::Completed {
            job: JobId(1),
            result: 99,
        }
    ));
    let ProxyCommand::Forward(next) = &assignments(&actions)[0].message else {
        panic!("queued successor is forwarded");
    };
    assert_eq!(next.job, JobId(2));
    assert_eq!(next.assignment, AssignmentId(1));
}

#[test]
fn stale_completion_is_typed_and_preserves_current_ownership() {
    let pool = pool(1, 0, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install(&mut pool, 0);
    submit(&mut pool, 1, 10);
    let error = match pool.receive(
        MailAddr(0),
        PoolMessage::Completed {
            worker: 0,
            assignment: AssignmentId(9),
            result: 0,
        },
    ) {
        Ok(_) => panic!("stale completion must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        PoolError::StaleCompletion {
            worker: 0,
            expected: AssignmentId(0),
            received: AssignmentId(9),
        }
    );
    assert!(matches!(
        pool.worker_phase(0),
        Some(WorkerPhase::Assigned {
            assignment: AssignmentId(0),
            ..
        })
    ));
}

#[test]
fn interruption_policy_distinguishes_failure_from_at_least_once_retry() {
    for policy in [InterruptionPolicy::Fail, InterruptionPolicy::Retry] {
        let pool = pool(1, 1, policy);
        let initialized = pool.initialize().unwrap();
        let mut pool = initialized.behavior;
        install(&mut pool, 0);
        submit(&mut pool, 1, 10);
        let actions = pool
            .on(WorkerStopped::new(
                0,
                0,
                Err(behavior::Crash::Panicked),
                Instant::now(),
            ))
            .unwrap();
        assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Installing));
        match policy {
            InterruptionPolicy::Fail => assert!(matches!(
                responses(&actions)[0].message,
                PoolResponse::Interrupted {
                    job: JobId(1),
                    payload: 10,
                    ..
                }
            )),
            InterruptionPolicy::Retry => {
                assert!(responses(&actions).is_empty());
                assert_eq!(pool.backlog_len(), 1);
                let replacement = install(&mut pool, 0);
                let ProxyCommand::Forward(retried) = &assignments(&replacement)[0].message else {
                    panic!("retry is forwarded after installation");
                };
                assert_eq!(retried.job, JobId(1));
                assert_eq!(retried.assignment, AssignmentId(1));
            }
        }
    }
}

#[test]
fn rejected_worker_creation_never_dispatches() {
    let pool = pool(1, 1, InterruptionPolicy::Retry);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    submit(&mut pool, 1, 10);
    let actions = pool
        .on(WorkerCreationResolved::new(
            0,
            0,
            CreationKind::Birth,
            Err(CreationRejection::InitializationFailed),
        ))
        .unwrap();
    assert!(assignments(&actions).is_empty());
    assert!(matches!(
        responses(&actions)[0].message,
        PoolResponse::Interrupted {
            job: JobId(1),
            payload: 10,
            reason: behavior::PoolInterruption::NoRecoverableWorkers,
        }
    ));
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Retired {
            reason: behavior::WorkerRetirement::CreationRejected(
                CreationRejection::InitializationFailed
            ),
        })
    );
    assert_eq!(pool.backlog_len(), 0);
}

#[test]
fn duplicate_configured_routes_are_rejected_before_initialization() {
    fn duplicate(_index: usize) -> u64 {
        7
    }
    let result = WorkerPool::<MailAddr, Reply, u8, u16, Worker>::new(
        behavior::ChildTopology::indexed(duplicate, 2, |index| Some(worker(index))),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        ),
    );
    assert!(matches!(result, Err(PoolConfigError::DuplicateWorker(7))));
}

#[test]
fn zero_worker_pool_is_rejected_before_it_can_accept_owned_work() {
    let result = WorkerPool::<MailAddr, Reply, u8, u16, Worker>::new(
        behavior::ChildTopology::indexed(nonce, 0, |index| Some(worker(index))),
        behavior::PoolConfiguration::new(
            8,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        ),
    );
    assert!(matches!(result, Err(PoolConfigError::NoWorkers)));
}

#[test]
fn panicking_payload_clone_occurs_before_admission_state_is_committed() {
    let pool = WorkerPool::<MailAddr, PanicReply, PanicPayload, (), PanicWorker>::new(
        behavior::ChildTopology::indexed(nonce, 1, |index| Some(panic_worker(index))),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        ),
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    pool.on(WorkerCreationResolved::new(
        0,
        0,
        CreationKind::Birth,
        Ok(()),
    ))
    .unwrap();
    let panic_on_clone = Arc::new(AtomicBool::new(true));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = pool.receive(
            MailAddr(90),
            PoolMessage::Submit {
                job: JobId(1),
                payload: PanicPayload {
                    panic_on_clone: panic_on_clone.clone(),
                },
                reply_to: Recipient::global(MailAddr(91)),
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(pool.backlog_len(), 0);
    assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Idle));

    panic_on_clone.store(false, Ordering::SeqCst);
    let actions = pool
        .receive(
            MailAddr(90),
            PoolMessage::Submit {
                job: JobId(2),
                payload: PanicPayload { panic_on_clone },
                reply_to: Recipient::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.behavior.assignments.len(), 1);
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Assigned {
            assignment: AssignmentId(0),
            job: JobId(2),
        })
    );
}

#[test]
fn panicking_retry_clone_preserves_the_exact_assigned_state() {
    let pool = WorkerPool::<MailAddr, PanicReply, PanicPayload, (), PanicWorker>::new(
        behavior::ChildTopology::indexed(nonce, 1, |index| Some(panic_worker(index))),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        ),
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    pool.on(WorkerCreationResolved::new(
        0,
        0,
        CreationKind::Birth,
        Ok(()),
    ))
    .unwrap();
    let panic_on_clone = Arc::new(AtomicBool::new(false));
    pool.receive(
        MailAddr(90),
        PoolMessage::Submit {
            job: JobId(1),
            payload: PanicPayload {
                panic_on_clone: panic_on_clone.clone(),
            },
            reply_to: Recipient::global(MailAddr(91)),
        },
    )
    .unwrap();
    panic_on_clone.store(true, Ordering::SeqCst);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = pool.on(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ));
    }));
    assert!(result.is_err());
    assert_eq!(pool.backlog_len(), 0);
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Assigned {
            assignment: AssignmentId(0),
            job: JobId(1),
        })
    );
}

#[test]
fn denied_replacement_retires_slot_instead_of_stranding_installation() {
    let pool = WorkerPool::new(
        behavior::ChildTopology::indexed(nonce, 1, |index| Some(worker(index))),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            0,
            Duration::from_secs(1),
        ),
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install(&mut pool, 0);
    submit(&mut pool, 1, 10);
    let actions = pool
        .on(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ))
        .unwrap();
    assert!(actions.sends.replacement_commands.is_empty());
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Retired {
            reason: behavior::WorkerRetirement::ReplacementUnavailable,
        })
    );
}

#[test]
fn duplicate_creation_resolution_cannot_revive_or_overwrite_an_available_slot() {
    let pool = pool(1, 1, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install(&mut pool, 0);
    let result = pool.on(WorkerCreationResolved::new(
        0,
        0,
        CreationKind::Birth,
        Ok(()),
    ));
    assert!(matches!(
        result,
        Err(PoolError::CreationResolvedWhileUnavailable {
            worker: 0,
            phase: WorkerPhase::Idle,
        })
    ));
    assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Idle));
}

#[derive(Clone, Copy, Debug)]
enum Command {
    Submit(u8),
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelSlot {
    Idle,
    Busy { assignment: u64, job: u64 },
}

struct Model {
    slot: ModelSlot,
    queue: VecDeque<(u64, u8)>,
    next_assignment: u64,
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn arbitrary_fifo_sequences_match_an_independent_ownership_model(
        commands in vec(prop_oneof![any::<u8>().prop_map(Command::Submit), Just(Command::Complete)], 0..128)
    ) {
        let pool = pool(1, 8, InterruptionPolicy::Fail);
        let initialized = pool.initialize().unwrap();
        let mut pool = initialized.behavior;
        install(&mut pool, 0);
        let mut model = Model { slot: ModelSlot::Idle, queue: VecDeque::new(), next_assignment: 0 };
        let mut job = 0_u64;

        for command in commands {
            match command {
                Command::Submit(payload) => {
                    let model_can_accept = matches!(model.slot, ModelSlot::Idle) || model.queue.len() < 8;
                    let actions = submit(&mut pool, job, payload);
                    if model_can_accept {
                        model.queue.push_back((job, payload));
                        if matches!(model.slot, ModelSlot::Idle) {
                            let (id, _) = model.queue.pop_front().unwrap();
                            model.slot = ModelSlot::Busy { assignment: model.next_assignment, job: id };
                            model.next_assignment += 1;
                        }
                        prop_assert!(matches!(responses(&actions)[0].message, PoolResponse::Accepted { .. }), "accepted response");
                    } else {
                        prop_assert!(matches!(responses(&actions)[0].message, PoolResponse::Rejected { .. }), "rejected response");
                    }
                    job += 1;
                }
                Command::Complete => {
                    let ModelSlot::Busy { assignment, .. } = model.slot else { continue };
                    pool.receive(MailAddr(0), PoolMessage::Completed {
                        worker: 0,
                        assignment: AssignmentId(assignment),
                        result: 0,
                    }).unwrap();
                    model.slot = if let Some((id, _)) = model.queue.pop_front() {
                        let assignment = model.next_assignment;
                        model.next_assignment += 1;
                        ModelSlot::Busy { assignment, job: id }
                    } else {
                        ModelSlot::Idle
                    };
                }
            }
            prop_assert_eq!(pool.backlog_len(), model.queue.len());
            match (pool.worker_phase(0).unwrap(), model.slot) {
                (WorkerPhase::Idle, ModelSlot::Idle) => {}
                (WorkerPhase::Assigned { assignment, job }, ModelSlot::Busy { assignment: expected_assignment, job: expected_job }) => {
                    prop_assert_eq!(assignment, AssignmentId(expected_assignment));
                    prop_assert_eq!(job, JobId(expected_job));
                }
                (actual, expected) => prop_assert!(false, "phase mismatch: {actual:?} != {expected:?}"),
            }
        }
    }
}

#[test]
fn assignment_and_response_lanes_survive_shutdown_composition() {
    let behavior = behavior::StopOnShutdown::new(pool(1, 0, InterruptionPolicy::Fail));
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    behavior
        .on(WorkerCreationResolved::new(
            0,
            0,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    let actions = behavior
        .receive(
            MailAddr(0),
            PoolMessage::Submit {
                job: JobId(1),
                payload: 1,
                reply_to: Recipient::global(MailAddr(2)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.behavior.responses.len(), 1);
    assert_eq!(actions.sends.behavior.assignments.len(), 1);
    assert!(matches!(actions.become_, Step::Continue));
}
use behavior_testkit::InitializeTest;
