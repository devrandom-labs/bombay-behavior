//! `workers!` sum attacks: the macro-produced Crew must dispatch every fleet
//! index to its declared concrete variant (protocol-preserving), and a
//! supervised mixed fleet must route replacements by birth sequence without
//! crossing variants.

use std::marker::PhantomData;
use std::time::Duration;

use behavior::{
    Acted, Actions, Base, Behavior, Crash, Delivery, MailAddr, Never, Recipient, RestartPolicy,
    Route, State, Step, Strategy, Supervising, SupervisionEvent, User, UserEvent, WorkerStopped,
    workers,
};
use tokio::time::Instant;

struct WorkerA;

impl State<u8, behavior::NoBirths, Never> for WorkerA {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), 1)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

struct WorkerB;

impl State<u8, behavior::NoBirths, Never> for WorkerB {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
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

/// The supervising parent is generic over its offspring; instantiating it
/// with the concrete `Crew` type happens implicitly at each `build` call
/// site (the macro's `Crew` is block-scoped).
struct GenericParent<C>(PhantomData<C>);

impl<C> State<Never, behavior::Births<C>, Never> for GenericParent<C>
where
    C: Behavior<Ph = Never, Addr = MailAddr>,
{
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<C>, Never> {
        Ok(Actions::cont())
    }
}

fn supervise_with<C>(
    count: usize,
    build: fn(usize) -> C,
    strategy: Strategy,
) -> Supervising<Base<GenericParent<C>, Never, behavior::Births<C>>, C>
where
    C: Behavior<Ph = Never, Addr = MailAddr> + Send,
{
    Supervising::new(
        Base::new(GenericParent(PhantomData)),
        |index| u64::try_from(index).unwrap(),
        count,
        build,
        strategy,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    )
}

/// The sum total and per-index variant dispatch are exact: slots 0..2 are
/// `WorkerA` (tag 1), slot 2 is `WorkerB` (tag 2).
#[tokio::test]
async fn workers_sum_preserves_the_concrete_variant_per_index() {
    let (count, build) =
        workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
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
    let (count, build) =
        workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
    assert_eq!(count, 3);
    let _ = build(3);
}

/// A supervised mixed fleet: every slot is wrapped in its own proxy; a
/// `RestForOne` replacement of a `WorkerA` slot (sequence 1) restarts exactly
/// the later-born slots (1 = `WorkerA`, 2 = `WorkerB`) — never the earlier
/// `WorkerA` slot 0 — and each replacement keeps its declared variant.
#[tokio::test]
async fn supervised_mixed_fleet_routes_replacements_by_birth_sequence() {
    let (count, build) =
        workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
    let mut supervisor = supervise_with(count, build, Strategy::RestForOne);
    let initial = supervisor.init().await.unwrap();
    assert_eq!(initial.creates.len(), 3);
    assert_eq!(initial.sends.own.inner.len(), 3);

    let at = Instant::now();
    let wide = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
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
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 2,
            outcome: Err(Crash::Failed),
            at,
        }))
        .await
        .unwrap();
    assert_eq!(narrow.sends.own.own.len(), 1);
    assert_eq!(narrow.sends.own.own[0].to.route(), Route::Child(2));
}

/// Three kinds: each kind's slots route to its own variant, exactly at the
/// kind boundaries.
#[tokio::test]
async fn workers_three_kinds_boundaries_route_exactly() {
    struct WorkerC;
    impl State<u8, behavior::NoBirths, Never> for WorkerC {
        type Addr = MailAddr;
        type Msg = u8;

        fn handle(
            &mut self,
            from: MailAddr,
            _message: u8,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never>
        {
            Ok(Actions {
                sends: vec![Delivery::new(Recipient::global(from), 3)],
                creates: Vec::new(),
                become_: Step::Continue,
            })
        }
    }
    fn worker_c(_index: usize) -> Base<WorkerC, u8> {
        Base::new(WorkerC)
    }

    let (count, build) = workers![
        (1, Base<WorkerA, u8>, worker_a),
        (1, Base<WorkerB, u8>, worker_b),
        (1, Base<WorkerC, u8>, worker_c)
    ];
    assert_eq!(count, 3);
    let expected = [1, 2, 3];
    for (index, tag) in expected.into_iter().enumerate() {
        let mut worker = build(index);
        let actions = worker.step(User::user(MailAddr(0), 7)).await.unwrap();
        assert_eq!(actions.sends[0].message, tag);
    }
}

/// A `workers!` fleet under `OneForAll`: one death replaces every alive slot,
/// each routed to its own declared variant's nonce.
#[tokio::test]
async fn workers_one_for_all_replaces_every_slot() {
    let (count, build) =
        workers![(2, Base<WorkerA, u8>, worker_a), (1, Base<WorkerB, u8>, worker_b)];
    let mut supervisor = supervise_with(count, build, Strategy::OneForAll);
    supervisor.init().await.unwrap();
    let actions = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .await
        .unwrap();
    let routes: Vec<Route<MailAddr>> = actions.sends.own.own.iter().map(|d| d.to.route()).collect();
    assert_eq!(routes.len(), 3);
    for nonce in 0..3 {
        assert!(routes.contains(&Route::Child(nonce)));
    }
    assert!(supervisor.is_alive(0));
    assert!(supervisor.is_alive(1));
    assert!(supervisor.is_alive(2));
}
