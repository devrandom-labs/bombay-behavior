use std::time::Duration;

use behavior::{
    Actions, AffinitySelector, AssignmentId, Behavior, CreationKind, Delivery, InterruptionPolicy,
    JobId, KeyedPoolMessage, KeyedWorkerPool, MailAddr, Never, NoBirths, PoolAssignment,
    PoolBehaviorSends, PoolError, PoolResponse, Proxy, ProxyCommand, Recipient, RestartPolicy,
    SendAlgebra, User, WorkerCreationResolved, WorkerPhase, WorkerStopped,
};
use proptest::prelude::*;
use std::time::Instant;

#[derive(Clone, Copy)]
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
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

fn worker(_: usize) -> Worker {
    Worker
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
type PoolDefinition = KeyedWorkerPool<MailAddr, Reply, u8, u8, u16, Worker, Selector>;
type Pool = behavior::Active<PoolDefinition>;

fn pool_definition(selector: Selector) -> PoolDefinition {
    KeyedWorkerPool::<MailAddr, Reply, u8, u8, u16, Worker, _>::new(
        nonce,
        2,
        |index| Some(worker(index)),
        8,
        InterruptionPolicy::Retry,
        RestartPolicy::Permanent,
        64,
        Duration::from_secs(60),
        selector,
    )
    .unwrap()
}

fn pool(selector: Selector) -> Pool {
    let pool = pool_definition(selector);
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    for slot in 0..2 {
        pool.on(WorkerCreationResolved::new(
            slot,
            slot,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    }
    pool
}

fn submit(
    pool: &mut Pool,
    key: u8,
    job: u64,
) -> behavior::PoolActions<MailAddr, Reply, u8, u16, Worker> {
    pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Submit {
            key,
            job: JobId(job),
            payload: key,
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

#[test]
fn targeted_submission_rejects_when_its_busy_workers_backlog_is_full() {
    let pool = KeyedWorkerPool::<MailAddr, Reply, u8, u8, u16, Worker, _>::new(
        nonce,
        1,
        |index| Some(worker(index)),
        1,
        InterruptionPolicy::Retry,
        RestartPolicy::Permanent,
        64,
        Duration::from_secs(60),
        Selector::Parity,
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

    assert_eq!(assignments(&submit(&mut pool, 0, 1)).len(), 1);
    assert!(assignments(&submit(&mut pool, 0, 2)).is_empty());
    assert_eq!(pool.backlog_len(), 1);

    let rejected = submit(&mut pool, 0, 3);
    assert!(assignments(&rejected).is_empty());
    assert!(matches!(
        rejected.sends.behavior.responses[0].message,
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
    let mut pool = pool(Selector::Parity);
    submit(&mut pool, 4, 1);
    assert_eq!(pool.affinity(&4), Some(0));

    let stopped = pool
        .on(WorkerStopped::new(
            0,
            0,
            Err(behavior::Crash::Panicked),
            Instant::now(),
        ))
        .unwrap();
    assert!(!stopped.sends.replacement_commands.is_empty());
    assert_eq!(pool.affinity(&4), Some(0));
    assert_eq!(pool.worker_phase(0), Some(WorkerPhase::Installing));

    let installed = pool
        .on(WorkerCreationResolved::new(
            0,
            2,
            CreationKind::replacement_of(0),
            Ok(()),
        ))
        .unwrap();
    assert_eq!(assignments(&installed).len(), 1);
    assert_eq!(
        assignments(&installed)[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
    assert_eq!(pool.affinity(&4), Some(0));
}

#[test]
fn explicit_rebalance_changes_future_admission_but_not_accepted_jobs() {
    let mut pool = pool(Selector::Parity);
    submit(&mut pool, 2, 1);
    let queued = submit(&mut pool, 2, 2);
    assert!(assignments(&queued).is_empty());

    pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Rebalance { key: 2, worker: 1 },
    )
    .unwrap();
    assert_eq!(pool.affinity(&2), Some(1));

    let future = submit(&mut pool, 2, 3);
    assert_eq!(
        assignments(&future)[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 1)
    );

    let prior = pool
        .receive(
            MailAddr(90),
            KeyedPoolMessage::Completed {
                worker: 0,
                assignment: AssignmentId(0),
                result: 10,
            },
        )
        .unwrap();
    let ProxyCommand::Forward(assignment) = &assignments(&prior)[0].message else {
        panic!("accepted job is forwarded");
    };
    assert_eq!(assignment.job, JobId(2));
    assert_eq!(
        assignments(&prior)[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
}

#[test]
fn unavailable_route_refuses_owned_payload_without_creating_a_binding() {
    let mut pool = pool(Selector::Invalid);
    let actions = submit(&mut pool, 7, 1);
    assert_eq!(pool.affinity(&7), None);
    assert!(assignments(&actions).is_empty());
    assert!(matches!(
        actions.sends.behavior.responses[0].message,
        PoolResponse::Rejected {
            job: JobId(1),
            payload: 7,
            reason: behavior::PoolRejection::AffinityUnavailable,
        }
    ));
}

#[test]
fn rebalance_rejects_unknown_worker_without_changing_the_binding() {
    let mut pool = pool(Selector::Parity);
    submit(&mut pool, 2, 1);
    let result = pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Rebalance { key: 2, worker: 9 },
    );
    assert!(matches!(result, Err(PoolError::UnknownWorker(9))));
    assert_eq!(pool.affinity(&2), Some(0));
}

#[test]
fn retired_affinity_refuses_new_work_until_explicit_valid_rebalance() {
    let mut pool = pool(Selector::Parity);
    submit(&mut pool, 2, 1);
    pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Completed {
            worker: 0,
            assignment: AssignmentId(0),
            result: 0,
        },
    )
    .unwrap();
    pool.on(WorkerStopped::new(
        0,
        0,
        Err(behavior::Crash::Panicked),
        Instant::now(),
    ))
    .unwrap();
    pool.on(WorkerCreationResolved::new(
        0,
        2,
        CreationKind::replacement_of(0),
        Err(behavior::CreationRejection::InitializationFailed),
    ))
    .unwrap();

    let refused = submit(&mut pool, 2, 2);
    assert!(matches!(
        refused.sends.behavior.responses[0].message,
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
        Err(PoolError::RebalanceToRetiredWorker { worker: 0, .. })
    ));
    pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Rebalance { key: 2, worker: 1 },
    )
    .unwrap();
    assert_eq!(pool.affinity(&2), Some(1));
    let admitted = submit(&mut pool, 2, 3);
    assert_eq!(
        assignments(&admitted)[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 1)
    );
}

#[test]
fn retiring_one_affinity_slot_terminates_its_queue_while_other_slots_live() {
    let mut pool = pool(Selector::Parity);
    submit(&mut pool, 2, 1);
    submit(&mut pool, 2, 2);
    assert_eq!(pool.backlog_len(), 1);

    pool.on(WorkerStopped::new(
        0,
        0,
        Err(behavior::Crash::Panicked),
        Instant::now(),
    ))
    .unwrap();
    assert_eq!(pool.backlog_len(), 2);
    let rejected = pool
        .on(WorkerCreationResolved::new(
            0,
            2,
            CreationKind::replacement_of(0),
            Err(behavior::CreationRejection::InitializationFailed),
        ))
        .unwrap();

    assert_eq!(pool.backlog_len(), 0);
    assert_eq!(rejected.sends.behavior.responses.len(), 2);
    assert!(
        rejected
            .sends
            .behavior
            .responses
            .iter()
            .any(|delivery| matches!(
                delivery.message,
                PoolResponse::Interrupted {
                    job: JobId(2),
                    reason: behavior::PoolInterruption::AffinityRetired { worker: 0, .. },
                    ..
                }
            ))
    );
    assert_eq!(pool.worker_phase(1), Some(WorkerPhase::Idle));
}

#[test]
fn unbound_rebalance_explicitly_establishes_affinity() {
    let mut pool = pool(Selector::Parity);
    assert_eq!(pool.affinity(&9), None);
    pool.receive(
        MailAddr(90),
        KeyedPoolMessage::Rebalance { key: 9, worker: 0 },
    )
    .unwrap();
    assert_eq!(pool.affinity(&9), Some(0));
    let actions = submit(&mut pool, 9, 1);
    assert_eq!(
        assignments(&actions)[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
}

#[test]
fn captured_selector_state_is_statically_dispatched() {
    let selected = 1_u64;
    let pool = KeyedWorkerPool::<MailAddr, Reply, u8, u8, u16, Worker, _>::new(
        nonce,
        2,
        |index| Some(worker(index)),
        1,
        InterruptionPolicy::Fail,
        RestartPolicy::Permanent,
        1,
        Duration::from_secs(1),
        move |_: &u8| selected,
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let mut pool = initialized.behavior;
    for slot in 0..2 {
        pool.on(WorkerCreationResolved::new(
            slot,
            slot,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
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
    assert_eq!(
        actions.sends.behavior.assignments[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 1)
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn bindings_change_iff_an_explicit_valid_rebalance_occurs(
        key in any::<u8>(),
        rebalances in prop::collection::vec(0_u64..3, 0..64),
    ) {
        let mut pool = pool(Selector::Parity);
        submit(&mut pool, key, 0);
        let mut model = u64::from(key % 2);
        for worker in rebalances {
            let result = pool.receive(
                MailAddr(90),
                KeyedPoolMessage::Rebalance { key, worker },
            );
            if worker < 2 {
                result.unwrap();
                model = worker;
            } else {
                prop_assert!(matches!(result, Err(PoolError::UnknownWorker(2))));
            }
            prop_assert_eq!(pool.affinity(&key), Some(model));
        }
    }
}

#[test]
fn short_rebalance_sequences_exhaustively_match_the_binding_model() {
    for first in 0..3_u64 {
        for second in 0..3_u64 {
            let mut pool = pool(Selector::Parity);
            submit(&mut pool, 3, 0);
            let mut expected = 1;
            for worker in [first, second] {
                let result =
                    pool.receive(MailAddr(90), KeyedPoolMessage::Rebalance { key: 3, worker });
                if worker < 2 {
                    result.unwrap();
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
    let behavior = behavior::Compose::new(pool_definition(Selector::Parity))
        .stop_on_shutdown()
        .initialize()
        .unwrap();
    let mut behavior = behavior.behavior;
    for slot in 0..2 {
        behavior
            .on(WorkerCreationResolved::new(
                slot,
                slot,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
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
    assert_eq!(actions.sends.behavior.responses.len(), 1);
    assert_eq!(actions.sends.behavior.assignments.len(), 1);
    assert_eq!(
        actions.sends.behavior.assignments[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 1)
    );
}

#[test]
fn named_pool_send_product_appends_each_lane_once_in_order() {
    type Sends = PoolBehaviorSends<MailAddr, Reply, u8, u16, Worker>;
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
    later.assignments.push(Delivery::new(
        Recipient::child(0),
        ProxyCommand::Forward(PoolAssignment {
            assignment: AssignmentId(0),
            job: JobId(1),
            payload: 7,
        }),
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
}
use behavior_testkit::InitializeTest;
