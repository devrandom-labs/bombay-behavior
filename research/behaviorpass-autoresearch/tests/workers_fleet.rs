//! `workers!` sum attacks: the macro-produced Crew must dispatch every fleet
//! index to its declared concrete variant (protocol-preserving), and a
//! supervised mixed fleet must route replacements by birth sequence without
//! crossing variants.

use std::marker::PhantomData;
use std::time::Duration;

use behaviorpass::{
    Acted, Actions, Base, Behavior, ChildStopped, Crash, Delivery, MailAddr, Never, Recipient,
    RestartPolicy, Route, State, Step, Strategy, Supervising, SupervisionEvent, User, UserEvent,
    workers,
};
use tokio::time::Instant;

struct WorkerA;

impl State<u8, Never, Never> for WorkerA {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), 1)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

struct WorkerB;

impl State<u8, Never, Never> for WorkerB {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), 2)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

fn worker_a(_index: usize) -> Base<WorkerA, u8> {
    Base::new(WorkerA)
}

fn worker_b(_index: usize) -> Base<WorkerB, u8> {
    Base::new(WorkerB)
}

/// The sum total and per-index variant dispatch are exact: slots 0..2 are
/// `WorkerA` (tag 1), slot 2 is `WorkerB` (tag 2).
#[tokio::test]
async fn workers_sum_preserves_the_concrete_variant_per_index() {
    let (count, build) = workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
    assert_eq!(count, 3);

    for index in 0..3 {
        let mut worker = build(index);
        let actions = worker.step(User::user(MailAddr(0), 7)).await.unwrap();
        let expected = if index < 2 { 1 } else { 2 };
        assert_eq!(actions.sends[0].message, expected);
    }
}

#[tokio::test]
#[should_panic(expected = "workers!: fleet index out of range")]
async fn workers_build_out_of_range_index_panics() {
    let (count, build) = workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
    assert_eq!(count, 3);
    let _ = build(3);
}

/// A supervised mixed fleet: every slot is wrapped in its own proxy; a
/// `RestForOne` replacement of a `WorkerA` slot (sequence 1) restarts exactly
/// the later-born slots (1 = `WorkerA`, 2 = `WorkerB`) — never the earlier
/// `WorkerA` slot 0 — and each replacement keeps its declared variant.
#[tokio::test]
async fn supervised_mixed_fleet_routes_replacements_by_birth_sequence() {
    // The macro's `Crew` type is block-scoped, so the supervising parent is
    // generic over its offspring; instantiating it with the concrete `Crew`
    // type happens implicitly at the `build` call site.
    struct GenericParent<C>(PhantomData<C>);

    impl<C> State<Never, C, Never> for GenericParent<C>
    where
        C: Behavior<Ph = Never, Addr = MailAddr>,
    {
        type Addr = MailAddr;
        type Msg = u64;

        fn handle(
            &mut self,
            _from: MailAddr,
            _message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, C, Never> {
            Ok(Actions::cont())
        }
    }

    fn supervise_with<C>(
        count: usize,
        build: fn(usize) -> C,
    ) -> Supervising<Base<GenericParent<C>, Never, C>, C>
    where
        C: Behavior<Ph = Never, Addr = MailAddr> + Send,
    {
        Supervising::new(
            Base::new(GenericParent(PhantomData)),
            |index| u64::try_from(index).unwrap(),
            count,
            build,
            Strategy::RestForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
        )
    }

    let (count, build) = workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
    let mut supervisor = supervise_with(count, build);
    let initial = supervisor.init().await.unwrap();
    assert_eq!(initial.creates.len(), 3);
    assert_eq!(initial.sends.own.inner.len(), 3);

    let at = Instant::now();
    let wide = supervisor
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 1,
            outcome: Err(Crash::Failed),
            at,
        }))
        .await
        .unwrap();
    let routes: Vec<Route<MailAddr>> = wide.sends.own.own.iter().map(|d| d.to.route()).collect();
    assert_eq!(routes.len(), 2);
    assert!(routes.contains(&Route::Child(1)));
    assert!(routes.contains(&Route::Child(2)));

    let narrow = supervisor
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 2,
            outcome: Err(Crash::Failed),
            at,
        }))
        .await
        .unwrap();
    assert_eq!(narrow.sends.own.own.len(), 1);
    assert_eq!(narrow.sends.own.own[0].to.route(), Route::Child(2));
}
