use std::time::Duration;

use behavior::{
    Actions, AffinitySelector, AssignmentId, Behavior, ChildDelivery, ChildHead, ChildReport,
    ChildRoute, ChildStopped, CreationKind, CreationResolved, Delivery, Exit, InterpreterRequests,
    InterruptionPolicy, JobId, KeyedPoolMessage, KeyedWorkerPool, KeyedWorkerPoolEvent,
    KeyedWorkerPoolProtocol, MailAddr, Never, NoBirths, PoolAssignment, PoolCompletion,
    PoolFailure, PoolInterruption, PoolResponse, PoolSends, Proxy, RebalanceRejection, Recipient,
    RestartPolicy, ScheduleAfter, SendEffects, ShutdownRequested, Step, SupervisionEvent,
    TimerGeneration, TimerId, User, WorkerCreationResolved, WorkerPhase, WorkerStopped,
};
use proptest::prelude::*;
use std::time::Instant;

#[derive(Clone, Copy)]
struct Worker;

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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

#[derive(Clone, Copy)]
enum Selector {
    Parity,
    Invalid,
}

impl AffinitySelector<u8, u64> for Selector {
    fn select(&self, key: &u8) -> u64 {
        match self {
            Self::Parity => u64::from(key % 2),
            Self::Invalid => 9,
        }
    }
}

type Reply = behavior_testkit::TestRecipient<PoolResponse<u8, u16, MailAddr>>;
type ReplyRoute = Recipient<Reply>;

fn owned_pool<B>(behavior: B) -> B
where
    B: Behavior<Protocol = KeyedWorkerPoolProtocol<MailAddr, u8, u8, u16, ReplyRoute>>,
{
    behavior
}

macro_rules! pool_definition {
    ($selector:expr) => {
        owned_pool(
            KeyedWorkerPool::new(
                behavior::ChildTopology::indexed(nonce, 2, |_| Some(Worker)),
                behavior::PoolConfiguration::new(
                    8,
                    InterruptionPolicy::Retry,
                    RestartPolicy::Permanent,
                    64,
                    Duration::from_secs(60),
                    behavior::RestartTiming::Immediate,
                ),
                $selector,
                Proxy::new,
            )
            .unwrap(),
        )
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
        assert_eq!(actions.sends.responses.len(), $responses, "responses");
        assert_eq!(actions.sends.assignments.len(), $assignments, "assignments");
        assert_eq!(
            actions.sends.supervision.child_observations.len(),
            $child_observations,
            "child observations"
        );
        assert_eq!(
            actions.sends.supervision.creation_observations.len(),
            $creation_observations,
            "creation observations"
        );
        assert_eq!(
            actions.sends.supervision.schedules.len(),
            $schedules,
            "schedules"
        );
        assert_eq!(
            actions.sends.supervision.replacement_inputs.len(),
            $replacement_inputs,
            "replacement inputs"
        );
        assert_eq!(
            actions.sends.supervision.failure_reports.len(),
            $failure_reports,
            "failure reports"
        );
        assert_eq!(
            actions.sends.supervision.shutdowns.len(),
            $shutdowns,
            "shutdowns"
        );
        assert_eq!(actions.creates.len(), $creates, "creates");
        assert!(matches!(&actions.become_, $become), "become");
    }};
}

macro_rules! pool {
    ($selector:expr) => {{
        let initialized = pool_definition!($selector).initialize().unwrap();
        let mut pool = initialized.behavior;
        for slot in 0..2 {
            let joined = pool.on_path(WorkerCreationResolved::new(
                slot,
                slot,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            assert_pool_effect_counts!(joined;
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
        }
        pool
    }};
}

macro_rules! submit {
    ($pool:expr, $key:expr, $job:expr) => {
        $pool
            .receive(
                MailAddr(90),
                KeyedPoolMessage::Submit {
                    key: $key,
                    job: JobId($job),
                    payload: $key,
                    reply_to: Recipient::global(MailAddr(91)),
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
    ($pool:expr, $slot:expr, $worker:expr, $assignment:expr, $result:expr) => {
        $pool
            .transition(SupervisionEvent::Behavior(
                KeyedWorkerPoolEvent::Completion(ChildReport::new(
                    $slot,
                    ChildReport::new(
                        $worker,
                        PoolCompletion {
                            assignment: AssignmentId($assignment),
                            result: $result,
                        },
                    ),
                )),
            ))
            .unwrap()
    };
}

#[test]
fn targeted_submission_rejects_when_its_busy_workers_backlog_is_full() {
    let pool = owned_pool(
        KeyedWorkerPool::new(
            behavior::ChildTopology::indexed(nonce, 1, |_| Some(Worker)),
            behavior::PoolConfiguration::new(
                1,
                InterruptionPolicy::Retry,
                RestartPolicy::Permanent,
                64,
                Duration::from_secs(60),
                behavior::RestartTiming::Immediate,
            ),
            Selector::Parity,
            Proxy::new,
        )
        .unwrap(),
    );
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    let joined = pool
        .on_path(WorkerCreationResolved::new(
            0,
            0,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_pool_effect_counts!(joined;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    assert_eq!(assignments!(&submit!(&mut pool, 0, 1)).len(), 1);
    assert!(assignments!(&submit!(&mut pool, 0, 2)).is_empty());
    assert_eq!(pool.backlog_len(), 1);

    let rejected = submit!(&mut pool, 0, 3);
    assert!(assignments!(&rejected).is_empty());
    assert!(matches!(
        rejected.sends.responses[0].message,
        PoolResponse::Rejected {
            job: JobId(3),
            payload: 0,
            reason: behavior::PoolRejection::BacklogFull,
        }
    ));
    assert_eq!(pool.backlog_len(), 1);
}

#[test]
fn affinity_survives_fresh_worker_incarnation_replacement() {
    let mut pool = pool!(Selector::Parity);
    let submitted = submit!(&mut pool, 4, 1);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(pool.affinity(&4), Some(0));

    let stopped = pool
        .on_path(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ))
        .unwrap();
    assert!(!stopped.sends.supervision.replacement_inputs.is_empty());
    assert_eq!(pool.affinity(&4), Some(0));
    assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Installing));

    let installed = pool
        .on_path(WorkerCreationResolved::new(
            0,
            2,
            CreationKind::replacement_of(0),
            Ok(()),
        ))
        .unwrap();
    assert_eq!(assignments!(&installed).len(), 1);
    assert_eq!(assignments!(&installed)[0].nonce, 0);
    assert_eq!(pool.affinity(&4), Some(0));
}

#[test]
fn explicit_rebalance_changes_future_admission_but_not_accepted_jobs() {
    let mut pool = pool!(Selector::Parity);
    let submitted = submit!(&mut pool, 2, 1);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let queued = submit!(&mut pool, 2, 2);
    assert!(assignments!(&queued).is_empty());

    let rebalanced = pool
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Rebalance { key: 2, worker: 1 },
        )
        .unwrap();
    assert_pool_effect_counts!(rebalanced;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(pool.affinity(&2), Some(1));

    let future = submit!(&mut pool, 2, 3);
    assert_eq!(assignments!(&future)[0].nonce, 1);

    let prior = complete!(&mut pool, 0, 0, 0, 10);
    let assignment = &assignments!(&prior)[0].message;
    assert_eq!(assignment.job, JobId(2));
    assert_eq!(assignments!(&prior)[0].nonce, 0);
}

#[test]
fn unavailable_route_refuses_owned_payload_without_creating_a_binding() {
    let mut pool = pool!(Selector::Invalid);
    let actions = submit!(&mut pool, 7, 1);
    assert_eq!(pool.affinity(&7), None);
    assert!(assignments!(&actions).is_empty());
    assert!(matches!(
        actions.sends.responses[0].message,
        PoolResponse::Rejected {
            job: JobId(1),
            payload: 7,
            reason: behavior::PoolRejection::AffinityUnavailable,
        }
    ));
}

#[test]
fn rebalance_rejects_unknown_worker_without_changing_the_binding() {
    let mut pool = pool!(Selector::Parity);
    let submitted = submit!(&mut pool, 2, 1);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let result = pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Rebalance { key: 2, worker: 9 },
    );
    assert!(matches!(
        result,
        Err(PoolFailure::Rebalance(RebalanceRejection::UnknownWorker {
            key: 2,
            worker: 9,
        }))
    ));
    assert_eq!(pool.affinity(&2), Some(0));
}

#[test]
fn retired_affinity_refuses_new_work_until_explicit_valid_rebalance() {
    let mut pool = pool!(Selector::Parity);
    let submitted = submit!(&mut pool, 2, 1);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let completed = complete!(&mut pool, 0, 0, 0, 0);
    assert_pool_effect_counts!(completed;
        responses = 1, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let stopped = pool
        .on_path(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ))
        .unwrap();
    assert_pool_effect_counts!(stopped;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 1,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let rejected = pool
        .on_path(WorkerCreationResolved::new(
            0,
            2,
            CreationKind::replacement_of(0),
            Err(behavior::CreationRejection::InitializationFailed),
        ))
        .unwrap();
    assert_pool_effect_counts!(rejected;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 1, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(
        rejected.sends.supervision.failure_reports.as_slice()[0].failure,
        behavior::SupervisionFailure::worker_creation_rejected(
            0,
            2,
            CreationKind::replacement_of(0),
            behavior::CreationRejection::InitializationFailed,
        )
    );

    let refused = submit!(&mut pool, 2, 2);
    assert!(matches!(
        refused.sends.responses[0].message,
        PoolResponse::Rejected {
            reason: behavior::PoolRejection::AffinityUnavailable,
            ..
        }
    ));
    assert!(matches!(
        pool.receive(
            MailAddr(90),
            KeyedPoolMessage::Rebalance { key: 2, worker: 0 },
        ),
        Err(PoolFailure::Rebalance(RebalanceRejection::RetiredWorker {
            key: 2,
            worker: 0,
            ..
        }))
    ));
    let rebalanced = pool
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Rebalance { key: 2, worker: 1 },
        )
        .unwrap();
    assert_pool_effect_counts!(rebalanced;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(pool.affinity(&2), Some(1));
    let admitted = submit!(&mut pool, 2, 3);
    assert_eq!(assignments!(&admitted)[0].nonce, 1);
}

#[test]
fn retiring_one_affinity_slot_terminates_its_queue_while_other_slots_live() {
    let mut pool = pool!(Selector::Parity);
    let submitted = submit!(&mut pool, 2, 1);
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    let queued = submit!(&mut pool, 2, 2);
    assert_pool_effect_counts!(queued;
        responses = 1, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(pool.backlog_len(), 1);

    let stopped = pool
        .on_path(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ))
        .unwrap();
    assert_pool_effect_counts!(stopped;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 1,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(pool.backlog_len(), 2);
    let rejected = pool
        .on_path(WorkerCreationResolved::new(
            0,
            2,
            CreationKind::replacement_of(0),
            Err(behavior::CreationRejection::InitializationFailed),
        ))
        .unwrap();

    assert_eq!(pool.backlog_len(), 0);
    assert_eq!(rejected.sends.responses.len(), 2);
    assert!(rejected.sends.responses.iter().any(|delivery| matches!(
        delivery.message,
        PoolResponse::Interrupted {
            job: JobId(2),
            reason: behavior::PoolInterruption::AffinityRetired { worker: 0, .. },
            ..
        }
    )));
    assert_eq!(pool.worker_phase(1), Some(WorkerPhase::Idle));
}

#[test]
fn unbound_rebalance_explicitly_establishes_affinity() {
    let mut pool = pool!(Selector::Parity);
    assert_eq!(pool.affinity(&9), None);
    let rebalanced = pool
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Rebalance { key: 9, worker: 0 },
        )
        .unwrap();
    assert_pool_effect_counts!(rebalanced;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert_eq!(pool.affinity(&9), Some(0));
    let actions = submit!(&mut pool, 9, 1);
    assert_eq!(assignments!(&actions)[0].nonce, 0);
}

#[test]
fn captured_selector_state_is_statically_dispatched() {
    let selected = 1_u64;
    let pool = owned_pool(
        KeyedWorkerPool::new(
            behavior::ChildTopology::indexed(nonce, 2, |_| Some(Worker)),
            behavior::PoolConfiguration::new(
                1,
                InterruptionPolicy::Fail,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
                behavior::RestartTiming::Immediate,
            ),
            move |_: &u8| selected,
            Proxy::new,
        )
        .unwrap(),
    );
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    for slot in 0..2 {
        let joined = pool
            .on_path(WorkerCreationResolved::new(
                slot,
                slot,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert_pool_effect_counts!(joined;
            responses = 0, assignments = 0, child_observations = 0,
            creation_observations = 0, schedules = 0, replacement_inputs = 0,
            failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
        );
    }
    let actions = pool
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Submit {
                key: 4,
                job: JobId(1),
                payload: 4,
                reply_to: Recipient::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.assignments[0].nonce, 1);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn bindings_change_iff_an_explicit_valid_rebalance_occurs(
        key in any::<u8>(),
        rebalances in prop::collection::vec(0_u64..3, 0..64),
    ) {
        let mut pool = pool!(Selector::Parity);
        let submitted = submit!(&mut pool, key, 0);
        assert_pool_effect_counts!(submitted;
            responses = 1, assignments = 1, child_observations = 0,
            creation_observations = 0, schedules = 0, replacement_inputs = 0,
            failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
        );
        let mut model = u64::from(key % 2);
        for worker in rebalances {
            let result = pool.receive(
                MailAddr(90),
                KeyedPoolMessage::Rebalance { key, worker },
            );
            if worker < 2 {
                let rebalanced = result.unwrap();
                assert_pool_effect_counts!(rebalanced;
                    responses = 0, assignments = 0, child_observations = 0,
                    creation_observations = 0, schedules = 0, replacement_inputs = 0,
                    failure_reports = 0, shutdowns = 0, creates = 0,
                    become = Step::Continue
                );
                model = worker;
            } else {
                let rejected_exactly = matches!(
                    result,
                    Err(PoolFailure::Rebalance(RebalanceRejection::UnknownWorker {
                        key: rejected,
                        worker: 2,
                    })) if rejected == key
                );
                prop_assert!(rejected_exactly);
            }
            prop_assert_eq!(pool.affinity(&key), Some(model));
        }
    }
}

#[test]
fn short_rebalance_sequences_exhaustively_match_the_binding_model() {
    for first in 0..3_u64 {
        for second in 0..3_u64 {
            let mut pool = pool!(Selector::Parity);
            let submitted = submit!(&mut pool, 3, 0);
            assert_pool_effect_counts!(submitted;
                responses = 1, assignments = 1, child_observations = 0,
                creation_observations = 0, schedules = 0, replacement_inputs = 0,
                failure_reports = 0, shutdowns = 0, creates = 0,
                become = Step::Continue
            );
            let mut expected = 1;
            for worker in [first, second] {
                let result =
                    pool.receive(MailAddr(90), KeyedPoolMessage::Rebalance { key: 3, worker });
                if worker < 2 {
                    let rebalanced = result.unwrap();
                    assert_pool_effect_counts!(rebalanced;
                        responses = 0, assignments = 0, child_observations = 0,
                        creation_observations = 0, schedules = 0,
                        replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
                        creates = 0, become = Step::Continue
                    );
                    expected = worker;
                } else {
                    assert!(result.is_err());
                }
                assert_eq!(pool.affinity(&3), Some(expected));
            }
        }
    }
}

#[test]
fn keyed_assignment_lanes_survive_shutdown_composition() {
    let behavior = behavior::StopOnShutdown::new(pool_definition!(Selector::Parity))
        .initialize()
        .unwrap();
    let mut behavior = behavior.behavior;
    for slot in 0..2 {
        let joined = behavior
            .on_path(WorkerCreationResolved::new(
                slot,
                slot,
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
    }
    let actions = behavior
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Submit {
                key: 3,
                job: JobId(1),
                payload: 3,
                reply_to: Recipient::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.inner.responses.len(), 1);
    assert_eq!(actions.sends.inner.assignments.len(), 1);
    assert_eq!(actions.sends.inner.assignments[0].nonce, 1);
}

#[test]
fn named_pool_send_product_appends_each_lane_once_in_order() {
    type Sends = PoolSends<MailAddr, Worker, Proxy<Worker>, Vec<Delivery<Reply>>>;
    let mut sends = Sends::empty();
    sends.responses.push(Delivery::new(
        Recipient::global(MailAddr(1)),
        PoolResponse::Accepted { job: JobId(1) },
    ));
    let mut later = Sends::empty();
    later.responses.push(Delivery::new(
        Recipient::global(MailAddr(2)),
        PoolResponse::Accepted { job: JobId(2) },
    ));
    later.assignments.push(ChildDelivery::at(
        ChildRoute::<Proxy<Worker>, ChildHead>::new(0),
        PoolAssignment {
            assignment: AssignmentId(0),
            job: JobId(1),
            payload: 7,
        },
    ));
    later.supervision.schedules = InterpreterRequests::one(ScheduleAfter::new(
        TimerId(4),
        TimerGeneration(2),
        Duration::from_millis(5),
    ));

    sends.append(later);
    assert!(matches!(
        sends.responses[0].message,
        PoolResponse::Accepted { job: JobId(1) }
    ));
    assert!(matches!(
        sends.responses[1].message,
        PoolResponse::Accepted { job: JobId(2) }
    ));
    assert_eq!(sends.assignments.len(), 1);
    assert_eq!(sends.supervision.schedules.len(), 1);
}

#[test]
fn keyed_pool_returns_owned_jobs_and_drains_all_stable_proxies() {
    let mut pool = pool_definition!(Selector::Parity)
        .initialize()
        .unwrap()
        .behavior;
    for slot in 0..2 {
        let committed = pool
            .on_path(CreationResolved::birth(slot, MailAddr(20 + slot)))
            .unwrap();
        assert_pool_effect_counts!(committed;
            responses = 0, assignments = 0, child_observations = 0,
            creation_observations = 0, schedules = 0, replacement_inputs = 0,
            failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
        );
        let joined = pool
            .on_path(WorkerCreationResolved::new(
                slot,
                slot,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert_pool_effect_counts!(joined;
            responses = 0, assignments = 0, child_observations = 0,
            creation_observations = 0, schedules = 0, replacement_inputs = 0,
            failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
        );
    }
    let submitted = pool
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Submit {
                key: 0,
                job: JobId(1),
                payload: 7,
                reply_to: Recipient::global(MailAddr(91)),
            },
        )
        .unwrap();
    assert_pool_effect_counts!(submitted;
        responses = 1, assignments = 1, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );

    let shutdown = pool.on_path(ShutdownRequested).unwrap();
    assert_eq!(shutdown.sends.supervision.shutdowns.len(), 2);
    assert!(shutdown.sends.responses.iter().any(|delivery| matches!(
        delivery.message,
        PoolResponse::Interrupted {
            reason: PoolInterruption::PoolShutdown,
            ..
        }
    )));
    assert!(matches!(shutdown.become_, Step::Continue));
    assert!(matches!(
        pool.receive(
            MailAddr(90),
            KeyedPoolMessage::Rebalance { key: 7, worker: 1 },
        ),
        Err(PoolFailure::Rebalance(RebalanceRejection::ShuttingDown {
            key: 7,
            worker: 1
        }))
    ));
    let retained = pool
        .on_path(ChildStopped::new(0, Ok(Exit::Normal), Instant::now()))
        .unwrap();
    assert_pool_effect_counts!(retained;
        responses = 0, assignments = 0, child_observations = 0,
        creation_observations = 0, schedules = 0, replacement_inputs = 0,
        failure_reports = 0, shutdowns = 0, creates = 0, become = Step::Continue
    );
    assert!(matches!(
        pool.on_path(ChildStopped::new(1, Ok(Exit::Normal), Instant::now()))
            .unwrap()
            .become_,
        Step::Stop(behavior::Stopped)
    ));
}
use behavior_testkit::InitializeTest;
