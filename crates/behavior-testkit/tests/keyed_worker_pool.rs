use std::time::Duration;

use behavior::{
    Actions, AssignmentId, Behavior, CreationKind, Delivery, InterruptionPolicy, JobId,
    KeyedPoolMessage, KeyedWorkerPool, MailAddr, Never, NoBirths, PoolAssignment, PoolError,
    PoolResponse, ProxyCommand, Recipient, RestartPolicy, Route, User, WorkerCreationResolved,
    WorkerPhase, WorkerStopped,
};
use proptest::prelude::*;
use tokio::time::Instant;

#[derive(Clone, Copy)]
struct Worker;

impl Behavior for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
    type Event = User<MailAddr, PoolAssignment<u8>>;
    type Sends = Vec<Delivery<MailAddr, Never>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

fn worker(_: usize) -> Worker {
    Worker
}

fn parity(key: &u8) -> u64 {
    u64::from(key % 2)
}

fn invalid(_: &u8) -> u64 {
    9
}

fn pool(route: fn(&u8) -> u64) -> KeyedWorkerPool<MailAddr, u8, u8, u16, Worker> {
    let mut pool = KeyedWorkerPool::new(
        nonce,
        2,
        worker,
        8,
        InterruptionPolicy::Retry,
        RestartPolicy::Permanent,
        64,
        Duration::from_secs(60),
        route,
    )
    .unwrap();
    pool.init().unwrap();
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
    pool: &mut KeyedWorkerPool<MailAddr, u8, u8, u16, Worker>,
    key: u8,
    job: u64,
) -> behavior::PoolActions<MailAddr, u8, u16, Worker> {
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
    actions: &behavior::PoolActions<MailAddr, u8, u16, Worker>,
) -> &[Delivery<MailAddr, ProxyCommand<Worker>>] {
    &actions.sends.behavior.own
}

#[test]
fn affinity_survives_fresh_worker_incarnation_replacement() {
    let mut pool = pool(parity);
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
    assert_eq!(assignments(&installed)[0].to.route(), Route::Child(0));
    assert_eq!(pool.affinity(&4), Some(0));
}

#[test]
fn explicit_rebalance_changes_future_admission_but_not_accepted_jobs() {
    let mut pool = pool(parity);
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
    assert_eq!(assignments(&future)[0].to.route(), Route::Child(1));

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
    assert_eq!(assignments(&prior)[0].to.route(), Route::Child(0));
}

#[test]
fn unavailable_route_refuses_owned_payload_without_creating_a_binding() {
    let mut pool = pool(invalid);
    let actions = submit(&mut pool, 7, 1);
    assert_eq!(pool.affinity(&7), None);
    assert!(assignments(&actions).is_empty());
    assert!(matches!(
        actions.sends.behavior.inner[0].message,
        PoolResponse::Rejected {
            job: JobId(1),
            payload: 7,
            reason: behavior::PoolRejection::AffinityUnavailable,
        }
    ));
}

#[test]
fn rebalance_rejects_unknown_worker_without_changing_the_binding() {
    let mut pool = pool(parity);
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
    let mut pool = pool(parity);
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
        refused.sends.behavior.inner[0].message,
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
    assert_eq!(assignments(&admitted)[0].to.route(), Route::Child(1));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn bindings_change_iff_an_explicit_valid_rebalance_occurs(
        key in any::<u8>(),
        rebalances in prop::collection::vec(0_u64..3, 0..64),
    ) {
        let mut pool = pool(parity);
        submit(&mut pool, key, 0);
        let mut model = parity(&key);
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
            let mut pool = pool(parity);
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
    let mut behavior = behavior::Compose::from_behavior(pool(parity))
        .stop_on_shutdown()
        .build();
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
    assert_eq!(actions.sends.behavior.inner.len(), 1);
    assert_eq!(actions.sends.behavior.own.len(), 1);
    assert_eq!(actions.sends.behavior.own[0].to.route(), Route::Child(1));
}
