use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use behavior::{
    Actions, AssignmentId, Backoff, Behavior, ChildReport, ChildStopped, CompletionRejection,
    Crash, CreationKind, CreationRejection, CreationResolved, Exit, InterruptionPolicy, JobId,
    MailAddr, Never, NoBirths, PoolAssignment, PoolCompletion, PoolConfigError, PoolError,
    PoolFailure, PoolInterruption, PoolMessage, PoolResponse, Proxy, ProxyUnavailable, Recipient,
    RestartPolicy, ShutdownRequested, Step, SupervisionEvent, TimerElapsed, User,
    WorkerCreationResolved, WorkerPhase, WorkerPool, WorkerPoolEvent, WorkerStopped,
};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Worker;

struct Reply;

type ReplyRoute = Recipient<Reply>;

impl behavior::Protocol for Reply {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, u16, MailAddr>;
}

impl Behavior for Reply {
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

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

#[derive(Debug)]
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

impl behavior::Protocol for PanicReply {
    type Addr = MailAddr;
    type Msg = PoolResponse<PanicPayload, (), MailAddr>;
}

impl Behavior for PanicReply {
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

impl behavior::Protocol for PanicWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<PanicPayload>;
}

impl Behavior for PanicWorker {
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

macro_rules! pool {
    ($workers:expr, $capacity:expr, $interruption:expr $(,)?) => {
        WorkerPool::<MailAddr, u8, u16, Worker, ReplyRoute, _>::new(
            behavior::ChildTopology::indexed(nonce, $workers, |_| Some(Worker)),
            behavior::PoolConfiguration::new(
                $capacity,
                $interruption,
                RestartPolicy::Permanent,
                64,
                Duration::from_secs(60),
                behavior::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap()
    };
}

macro_rules! assert_pool_effect_counts {
    (
        $actions:expr;
        responses = $responses:expr,
        assignments = $assignments:expr,
        child_observations = $child_observations:expr,
        creation_observations = $creation_observations:expr,
        schedules = $schedules:expr,
        replacement_inputs = $replacement_inputs:expr,
        failure_reports = $failure_reports:expr,
        shutdowns = $shutdowns:expr,
        creates = $creates:expr,
        become = $become:pat_param
    ) => {{
        let actions = &$actions;
        assert_eq!(actions.sends.responses.len(), $responses);
        assert_eq!(actions.sends.assignments.len(), $assignments);
        assert_eq!(
            actions.sends.supervision.child_observations.len(),
            $child_observations
        );
        assert_eq!(
            actions.sends.supervision.creation_observations.len(),
            $creation_observations
        );
        assert_eq!(actions.sends.supervision.schedules.len(), $schedules);
        assert_eq!(
            actions.sends.supervision.replacement_inputs.len(),
            $replacement_inputs
        );
        assert_eq!(
            actions.sends.supervision.failure_reports.len(),
            $failure_reports
        );
        assert_eq!(actions.sends.supervision.shutdowns.len(), $shutdowns);
        assert_eq!(actions.creates.len(), $creates);
        assert!(matches!(&actions.become_, $become));
    }};
}

macro_rules! install {
    ($pool:expr, $slot:expr $(,)?) => {{
        let installed = $pool
            .on_path(WorkerCreationResolved::new(
                $slot,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert_pool_effect_counts!(installed;
            responses = 0,
            assignments = 0,
            child_observations = 0,
            creation_observations = 0,
            schedules = 0,
            replacement_inputs = 0,
            failure_reports = 0,
            shutdowns = 0,
            creates = 0,
            become = Step::Continue
        );
    }};
}

macro_rules! submit {
    ($pool:expr, $id:expr, $payload:expr $(,)?) => {
        $pool
            .receive(
                MailAddr(90),
                PoolMessage::Submit {
                    job: JobId($id),
                    payload: $payload,
                    reply_to: Recipient::<Reply>::global(MailAddr(91)),
                },
            )
            .unwrap()
    };
}

macro_rules! assignments {
    ($actions:expr) => {
        &$actions.sends.assignments
    };
}

macro_rules! complete {
    ($pool:expr, $slot:expr, $worker:expr, $assignment:expr, $result:expr $(,)?) => {
        $pool.transition(SupervisionEvent::Behavior(WorkerPoolEvent::Completion(
            ChildReport::new(
                $slot,
                ChildReport::new(
                    $worker,
                    PoolCompletion {
                        assignment: AssignmentId($assignment),
                        result: $result,
                    },
                ),
            ),
        )))
    };
}

macro_rules! responses {
    ($actions:expr) => {
        &$actions.sends.responses
    };
}

#[test]
fn initialization_stages_and_observes_every_stable_proxy_before_dispatch() {
    let pool = pool!(2, 4, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let actions = initialized.actions;
    let pool = initialized.behavior;
    assert_eq!(actions.creates.len(), 2);
    assert_eq!(actions.sends.supervision.child_observations.len(), 2);
    assert_eq!(actions.sends.supervision.creation_observations.len(), 2);
    for (creation, observation) in actions
        .creates
        .iter()
        .zip(actions.sends.supervision.creation_observations.iter())
    {
        assert_eq!(creation.nonce, observation.nonce);
    }
    assert!(assignments!(&actions).is_empty());
    assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Installing));
    assert_eq!(pool.worker_phase(1), Some(WorkerPhase::Installing));
}

#[test]
fn shutdown_returns_all_jobs_and_waits_for_every_owned_proxy() {
    let mut pool = pool!(2, 2, InterruptionPolicy::Retry)
        .initialize()
        .unwrap()
        .behavior;
    for nonce in [0, 1] {
        let committed = pool
            .on_path(CreationResolved::birth(nonce, MailAddr(10 + nonce)))
            .unwrap();
        assert_pool_effect_counts!(committed;
            responses = 0,
            assignments = 0,
            child_observations = 0,
            creation_observations = 0,
            schedules = 0,
            replacement_inputs = 0,
            failure_reports = 0,
            shutdowns = 0,
            creates = 0,
            become = Step::Continue
        );
        install!(&mut pool, nonce);
    }
    let first = submit!(&mut pool, 1, 11);
    assert_pool_effect_counts!(first;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let second = submit!(&mut pool, 2, 22);
    assert_pool_effect_counts!(second;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let queued = submit!(&mut pool, 3, 33);
    assert_pool_effect_counts!(queued;
        responses = 1, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    let shutdown = pool.on_path(ShutdownRequested).unwrap();
    assert_eq!(shutdown.sends.supervision.shutdowns.len(), 2);
    assert_eq!(responses!(&shutdown).len(), 3);
    assert!(responses!(&shutdown).iter().all(|delivery| matches!(
        delivery.message,
        PoolResponse::Interrupted {
            reason: PoolInterruption::PoolShutdown,
            ..
        }
    )));
    assert!(matches!(shutdown.become_, Step::Continue));

    assert!(matches!(
        complete!(&mut pool, 1, 0, 77, 909),
        Err(PoolFailure::Completion(CompletionRejection::ShuttingDown {
            worker: 1,
            assignment: AssignmentId(77),
            result: 909,
        }))
    ));

    let first = pool
        .on_path(ChildStopped::new(0, Ok(Exit::Normal), Instant::now()))
        .unwrap();
    assert!(matches!(first.become_, Step::Continue));
    let last = pool
        .on_path(ChildStopped::new(1, Ok(Exit::Normal), Instant::now()))
        .unwrap();
    assert!(matches!(last.become_, Step::Stop(behavior::Stopped)));
}

#[test]
fn shutdown_resolves_pending_proxy_installation_without_duplicate_requests() {
    let mut pool = pool!(2, 0, InterruptionPolicy::Fail)
        .initialize()
        .unwrap()
        .behavior;
    let committed = pool
        .on_path(CreationResolved::birth(0, MailAddr(10)))
        .unwrap();
    assert_pool_effect_counts!(committed;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    let shutdown = pool.on_path(ShutdownRequested).unwrap();
    assert_eq!(shutdown.sends.supervision.shutdowns.as_slice().len(), 1);
    assert_eq!(shutdown.sends.supervision.shutdowns.as_slice()[0].nonce, 0);

    let pending_rejected = pool
        .on_path(CreationResolved::rejected(
            1,
            CreationKind::Birth,
            CreationRejection::EnvironmentFailed,
        ))
        .unwrap();
    assert!(pending_rejected.sends.supervision.shutdowns.is_empty());
    assert!(matches!(pending_rejected.become_, Step::Continue));

    let stale_resolution = CreationResolved::birth(0, MailAddr(99));
    assert!(matches!(
        pool.on_path(stale_resolution),
        Err(PoolFailure::Infrastructure(PoolError::UnexpectedCreation(returned)))
            if returned == stale_resolution
    ));
    let stopped = pool
        .on_path(ChildStopped::new(0, Ok(Exit::Normal), Instant::now()))
        .unwrap();
    assert!(matches!(stopped.become_, Step::Stop(behavior::Stopped)));
}

#[test]
fn accepted_job_is_recorded_before_one_exact_dispatch() {
    let pool = pool!(1, 0, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);

    let actions = submit!(&mut pool, 7, 42);
    assert!(matches!(
        responses!(&actions)[0].message,
        PoolResponse::Accepted { job: JobId(7) }
    ));
    let assignment = &assignments!(&actions)[0].message;
    assert_eq!(assignment.assignment, AssignmentId(0));
    assert_eq!(assignment.job, JobId(7));
    assert_eq!(assignment.payload, 42);
    assert_eq!(assignments!(&actions)[0].nonce, 0);
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
    let pool = pool!(1, 1, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    let accepted = submit!(&mut pool, 1, 10);
    assert!(matches!(
        responses!(&accepted)[0].message,
        PoolResponse::Accepted { .. }
    ));
    let rejected = submit!(&mut pool, 2, 20);
    assert!(matches!(
        responses!(&rejected)[0].message,
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
    let pool = pool!(1, 2, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let assigned = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(assigned;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let queued = submit!(&mut pool, 2, 20);
    assert_pool_effect_counts!(queued;
        responses = 1, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    let actions = complete!(&mut pool, 0, 0, 0, 99).unwrap();
    assert!(matches!(
        responses!(&actions)[0].message,
        PoolResponse::Completed {
            job: JobId(1),
            result: 99,
        }
    ));
    let next = &assignments!(&actions)[0].message;
    assert_eq!(next.job, JobId(2));
    assert_eq!(next.assignment, AssignmentId(1));
}

#[test]
fn stale_completion_is_typed_and_preserves_current_ownership() {
    let pool = pool!(1, 0, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let error = match complete!(&mut pool, 0, 0, 9, 0) {
        Ok(_) => panic!("stale completion must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        PoolFailure::Completion(CompletionRejection::StaleAssignment {
            worker: 0,
            expected: AssignmentId(0),
            received: AssignmentId(9),
            result: 0,
        })
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
fn stale_incarnation_completion_returns_the_result_without_consuming_the_assignment() {
    let pool = pool!(1, 0, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    let error = match complete!(&mut pool, 0, 99, 0, 71) {
        Err(error) => error,
        Ok(_) => panic!("a stale incarnation must not consume the assignment"),
    };
    assert_eq!(
        error,
        PoolFailure::Completion(CompletionRejection::StaleIncarnation {
            worker: 0,
            expected: 0,
            observed: 99,
            assignment: AssignmentId(0),
            result: 71,
        })
    );
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Assigned {
            assignment: AssignmentId(0),
            job: JobId(1),
        })
    );
}

#[test]
fn stale_worker_stop_is_rejected_before_pool_or_supervisor_state_changes() {
    let mut pool = pool!(1, 1, InterruptionPolicy::Retry)
        .initialize()
        .unwrap()
        .behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let stopped = WorkerStopped::new(0, 99, Err(Crash::Failed), Instant::now());

    assert!(matches!(
        pool.on_path(stopped.clone()),
        Err(PoolFailure::Infrastructure(PoolError::UnexpectedWorkerStopped(returned)))
            if returned == stopped
    ));
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
fn interruption_policy_distinguishes_failure_from_at_least_once_retry() {
    for policy in [InterruptionPolicy::Fail, InterruptionPolicy::Retry] {
        let pool = pool!(1, 1, policy);
        let initialized = pool.initialize().unwrap();
        let mut pool = initialized.behavior;
        install!(&mut pool, 0);
        let submitted = submit!(&mut pool, 1, 10);
        assert_pool_effect_counts!(submitted;
            responses = 1, assignments = 1, child_observations = 0,
            creation_observations = 0, schedules = 0, replacement_inputs = 0,
            failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
        );
        let actions = pool
            .on_path(WorkerStopped::new(
                0,
                0,
                Err(behavior::Crash::Panicked),
                Instant::now(),
            ))
            .unwrap();
        assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Installing));
        match policy {
            InterruptionPolicy::Fail => assert!(matches!(
                responses!(&actions)[0].message,
                PoolResponse::Interrupted {
                    job: JobId(1),
                    payload: 10,
                    ..
                }
            )),
            InterruptionPolicy::Retry => {
                assert!(responses!(&actions).is_empty());
                assert_eq!(pool.backlog_len(), 1);
                let replacement = pool
                    .on_path(WorkerCreationResolved::new(
                        0,
                        1,
                        CreationKind::ReplacementIncarnation { replaces: 0 },
                        Ok(()),
                    ))
                    .unwrap();
                let retried = &assignments!(&replacement)[0].message;
                assert_eq!(retried.job, JobId(1));
                assert_eq!(retried.assignment, AssignmentId(1));
            }
        }
    }
}

#[test]
fn delayed_pool_replacement_retains_retry_until_the_exact_timer_once() {
    let pool = WorkerPool::<MailAddr, u8, u16, Worker, ReplyRoute, _>::new(
        behavior::ChildTopology::indexed(nonce, 1, |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Retry,
            RestartPolicy::Permanent,
            8,
            Duration::MAX,
            behavior::RestartTiming::Delayed(Backoff::constant(Duration::from_millis(4)).unwrap()),
        ),
        Proxy::new,
    )
    .unwrap();
    let mut pool = pool.initialize().unwrap().behavior;
    let committed = pool
        .on_path(CreationResolved::birth(0, MailAddr(10)))
        .unwrap();
    assert_pool_effect_counts!(committed;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    let stopped = pool
        .on_path(WorkerStopped::new(0, 0, Err(Crash::Failed), Instant::now()))
        .unwrap();
    assert!(responses!(&stopped).is_empty());
    assert!(assignments!(&stopped).is_empty());
    assert!(stopped.sends.supervision.replacement_inputs.is_empty());
    assert_eq!(stopped.sends.supervision.schedules.len(), 1);
    assert_eq!(pool.backlog_len(), 1);

    let schedule = stopped.sends.supervision.schedules.as_slice()[0];
    let elapsed = TimerElapsed::new(schedule.id, schedule.generation);
    let released = pool.on_path(elapsed).unwrap();
    assert_eq!(released.sends.supervision.replacement_inputs.len(), 1);
    assert_eq!(released.sends.supervision.replacement_inputs[0].nonce, 0);
    assert!(assignments!(&released).is_empty());
    assert!(responses!(&released).is_empty());

    let duplicate = pool.on_path(elapsed).unwrap();
    assert!(duplicate.sends.supervision.replacement_inputs.is_empty());
    assert!(assignments!(&duplicate).is_empty());
    assert!(responses!(&duplicate).is_empty());

    let installed = pool
        .on_path(WorkerCreationResolved::new(
            0,
            1,
            CreationKind::ReplacementIncarnation { replaces: 0 },
            Ok(()),
        ))
        .unwrap();
    assert_eq!(assignments!(&installed).len(), 1);
    assert_eq!(assignments!(&installed)[0].message.job, JobId(1));
    assert_eq!(
        assignments!(&installed)[0].message.assignment,
        AssignmentId(1)
    );
    assert!(responses!(&installed).is_empty());
    assert_eq!(pool.backlog_len(), 0);
}

#[test]
fn worker_stop_before_proxy_return_joins_without_losing_or_failing_the_assignment() {
    let pool = pool!(1, 1, InterruptionPolicy::Retry);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    let assignment = assignments!(&submitted)[0].message.clone();

    let stopped = pool
        .on_path(WorkerStopped::new(0, 0, Err(Crash::Failed), Instant::now()))
        .unwrap();
    assert!(responses!(&stopped).is_empty());
    assert_eq!(pool.backlog_len(), 1);

    let returned = ProxyUnavailable {
        proxy: 0,
        from: MailAddr(90),
        phase: behavior::IncarnationPhase::Vacant {
            last_installed: Some(0),
        },
        command: assignment.clone(),
    };
    let joined = pool
        .transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        ))
        .expect("the two authoritative facts must join in either order");
    assert!(responses!(&joined).is_empty());
    assert!(assignments!(&joined).is_empty());
    assert_eq!(pool.backlog_len(), 1);

    assert!(matches!(
        pool.transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        )),
        Err(PoolFailure::UnexpectedAssignmentUnavailable(observed)) if observed == returned
    ));
    assert_eq!(pool.backlog_len(), 1);
}

#[test]
fn proxy_return_before_worker_stop_joins_once_and_preserves_the_retry() {
    let pool = pool!(1, 1, InterruptionPolicy::Retry);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    let assignment = assignments!(&submitted)[0].message.clone();
    let returned = ProxyUnavailable {
        proxy: 0,
        from: MailAddr(90),
        phase: behavior::IncarnationPhase::Vacant {
            last_installed: Some(0),
        },
        command: assignment,
    };

    let first = pool
        .transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        ))
        .unwrap();
    assert!(responses!(&first).is_empty());
    assert!(assignments!(&first).is_empty());
    assert_eq!(pool.backlog_len(), 0);

    assert!(matches!(
        pool.transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        )),
        Err(PoolFailure::UnexpectedAssignmentUnavailable(observed)) if observed == returned
    ));

    let stopped = pool
        .on_path(WorkerStopped::new(0, 0, Err(Crash::Failed), Instant::now()))
        .unwrap();
    assert!(responses!(&stopped).is_empty());
    assert_eq!(pool.backlog_len(), 1);
}

#[test]
fn returned_assignment_after_restart_exhaustion_is_consumed_without_a_second_outcome() {
    let pool = WorkerPool::<MailAddr, u8, u16, Worker, ReplyRoute, _>::new(
        behavior::ChildTopology::indexed(nonce, 1, |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            0,
            Duration::from_secs(1),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    let assignment = assignments!(&submitted)[0].message.clone();

    let exhausted = pool
        .on_path(WorkerStopped::new(0, 0, Err(Crash::Failed), Instant::now()))
        .unwrap();
    assert_eq!(responses!(&exhausted).len(), 1);
    assert!(matches!(
        responses!(&exhausted)[0].message,
        PoolResponse::Interrupted {
            job: JobId(1),
            payload: 10,
            ..
        }
    ));
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Retired {
            reason: behavior::WorkerRetirement::ReplacementUnavailable,
        })
    );

    let returned = ProxyUnavailable {
        proxy: 0,
        from: MailAddr(90),
        phase: behavior::IncarnationPhase::Vacant {
            last_installed: Some(0),
        },
        command: assignment,
    };
    let joined = pool
        .transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        ))
        .unwrap();
    assert!(responses!(&joined).is_empty());
    assert!(assignments!(&joined).is_empty());
    assert!(matches!(
        pool.transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        )),
        Err(PoolFailure::UnexpectedAssignmentUnavailable(observed)) if observed == returned
    ));
}

#[test]
fn returned_assignment_after_shutdown_is_consumed_without_a_second_outcome() {
    let pool = pool!(1, 1, InterruptionPolicy::Retry);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    let assignment = assignments!(&submitted)[0].message.clone();

    let shutdown = pool.on_path(ShutdownRequested).unwrap();
    assert_eq!(responses!(&shutdown).len(), 1);
    assert!(matches!(
        responses!(&shutdown)[0].message,
        PoolResponse::Interrupted {
            job: JobId(1),
            payload: 10,
            reason: PoolInterruption::PoolShutdown,
        }
    ));

    let returned = ProxyUnavailable {
        proxy: 0,
        from: MailAddr(90),
        phase: behavior::IncarnationPhase::ShuttingDown { incarnation: 0 },
        command: assignment,
    };
    let joined = pool
        .transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        ))
        .unwrap();
    assert!(responses!(&joined).is_empty());
    assert!(assignments!(&joined).is_empty());
    assert!(matches!(
        pool.transition(SupervisionEvent::Behavior(
            WorkerPoolEvent::AssignmentUnavailable(returned.clone()),
        )),
        Err(PoolFailure::UnexpectedAssignmentUnavailable(observed)) if observed == returned
    ));
}

#[test]
fn rejected_worker_creation_never_dispatches() {
    let pool = pool!(1, 1, InterruptionPolicy::Retry);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    let submitted = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let actions = pool
        .on_path(WorkerCreationResolved::new(
            0,
            0,
            CreationKind::Birth,
            Err(CreationRejection::InitializationFailed),
        ))
        .unwrap();
    assert!(assignments!(&actions).is_empty());
    assert!(matches!(
        responses!(&actions)[0].message,
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
    let result = WorkerPool::<MailAddr, u8, u16, Worker, ReplyRoute, _>::new(
        behavior::ChildTopology::indexed(duplicate, 2, |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    );
    assert!(matches!(result, Err(PoolConfigError::DuplicateWorker(7))));
}

#[test]
fn zero_worker_pool_is_rejected_before_it_can_accept_owned_work() {
    let result = WorkerPool::<MailAddr, u8, u16, Worker, ReplyRoute, _>::new(
        behavior::ChildTopology::indexed(nonce, 0, |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            8,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    );
    assert!(matches!(result, Err(PoolConfigError::NoWorkers)));
}

#[test]
fn panicking_payload_clone_occurs_before_admission_state_is_committed() {
    let pool =
        WorkerPool::<MailAddr, PanicPayload, (), PanicWorker, Recipient<PanicReply>, _>::new(
            behavior::ChildTopology::indexed(nonce, 1, |_| Some(PanicWorker)),
            behavior::PoolConfiguration::new(
                1,
                InterruptionPolicy::Retry,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
                behavior::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let panic_on_clone = Arc::new(AtomicBool::new(true));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match pool.receive(
            MailAddr(90),
            PoolMessage::Submit {
                job: JobId(1),
                payload: PanicPayload {
                    panic_on_clone: panic_on_clone.clone(),
                },
                reply_to: Recipient::<PanicReply>::global(MailAddr(91)),
            },
        ) {
            Ok(_) | Err(_) => {}
        }
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
                reply_to: Recipient::<PanicReply>::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.assignments.len(), 1);
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
    let pool =
        WorkerPool::<MailAddr, PanicPayload, (), PanicWorker, Recipient<PanicReply>, _>::new(
            behavior::ChildTopology::indexed(nonce, 1, |_| Some(PanicWorker)),
            behavior::PoolConfiguration::new(
                1,
                InterruptionPolicy::Retry,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
                behavior::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let panic_on_clone = Arc::new(AtomicBool::new(false));
    let submitted = pool
        .receive(
            MailAddr(90),
            PoolMessage::Submit {
                job: JobId(1),
                payload: PanicPayload {
                    panic_on_clone: panic_on_clone.clone(),
                },
                reply_to: Recipient::<PanicReply>::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    panic_on_clone.store(true, Ordering::SeqCst);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match pool.on_path(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        )) {
            Ok(_) | Err(_) => {}
        }
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
    let pool = WorkerPool::<MailAddr, u8, u16, Worker, ReplyRoute, _>::new(
        behavior::ChildTopology::indexed(nonce, 1, |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            1,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            0,
            Duration::from_secs(1),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let submitted = submit!(&mut pool, 1, 10);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let actions = pool
        .on_path(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ))
        .unwrap();
    assert!(actions.sends.supervision.replacement_inputs.is_empty());
    assert_eq!(
        pool.worker_phase(0),
        Some(WorkerPhase::Retired {
            reason: behavior::WorkerRetirement::ReplacementUnavailable,
        })
    );
}

#[test]
fn duplicate_creation_resolution_cannot_revive_or_overwrite_an_available_slot() {
    let pool = pool!(1, 1, InterruptionPolicy::Fail);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    install!(&mut pool, 0);
    let observed = WorkerCreationResolved::new(0, 0, CreationKind::Birth, Ok(()));
    let result = pool.on_path(observed);
    assert!(matches!(
        result,
        Err(PoolFailure::Infrastructure(PoolError::UnexpectedWorkerCreation(returned)))
            if returned == observed
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
        let pool = pool!(1, 8, InterruptionPolicy::Fail);
        let initialized = pool.initialize().unwrap();
        let mut pool = initialized.behavior;
        install!(&mut pool, 0);
        let mut model = Model { slot: ModelSlot::Idle, queue: VecDeque::new(), next_assignment: 0 };
        let mut job = 0_u64;

        for command in commands {
            match command {
                Command::Submit(payload) => {
                    let model_can_accept = matches!(model.slot, ModelSlot::Idle) || model.queue.len() < 8;
                    let actions = submit!(&mut pool, job, payload);
                    if model_can_accept {
                        model.queue.push_back((job, payload));
                        if matches!(model.slot, ModelSlot::Idle) {
                            let (id, _) = model.queue.pop_front().unwrap();
                            model.slot = ModelSlot::Busy { assignment: model.next_assignment, job: id };
                            model.next_assignment += 1;
                        }
                        prop_assert!(matches!(responses!(&actions)[0].message, PoolResponse::Accepted { .. }), "accepted response");
                    } else {
                        prop_assert!(matches!(responses!(&actions)[0].message, PoolResponse::Rejected { .. }), "rejected response");
                    }
                    job += 1;
                }
                Command::Complete => {
                    let ModelSlot::Busy { assignment, .. } = model.slot else { continue };
                    let expected_assignment = usize::from(!model.queue.is_empty());
                    let completed = complete!(&mut pool, 0, 0, assignment, 0).unwrap();
                    assert_pool_effect_counts!(completed;
                        responses = 1, assignments = expected_assignment,
                        child_observations = 0, creation_observations = 0,
                        schedules = 0, replacement_inputs = 0, failure_reports = 0,
                        shutdowns = 0, creates = 0, become = Step::Continue
                    );
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
    let behavior = behavior::StopOnShutdown::new(pool!(1, 0, InterruptionPolicy::Fail));
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let joined = behavior
        .on_path(WorkerCreationResolved::new(
            0,
            0,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert!(matches!(joined.sends.owned, behavior::NoSends));
    assert!(joined.sends.inner.responses.is_empty());
    assert!(joined.sends.inner.assignments.is_empty());
    assert!(joined.sends.inner.supervision.child_observations.is_empty());
    assert!(
        joined
            .sends
            .inner
            .supervision
            .creation_observations
            .is_empty()
    );
    assert!(joined.sends.inner.supervision.schedules.is_empty());
    assert!(joined.sends.inner.supervision.replacement_inputs.is_empty());
    assert!(joined.sends.inner.supervision.failure_reports.is_empty());
    assert!(joined.sends.inner.supervision.shutdowns.is_empty());
    assert!(joined.creates.is_empty());
    assert!(matches!(joined.become_, Step::Continue));
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
    assert_eq!(actions.sends.inner.responses.len(), 1);
    assert_eq!(actions.sends.inner.assignments.len(), 1);
    assert!(matches!(actions.become_, Step::Continue));
}
use behavior_testkit::InitializeTest;
