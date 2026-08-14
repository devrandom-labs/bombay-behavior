//! `workers!` sum attacks: the macro-produced Crew must dispatch every fleet
//! index to its declared concrete variant (protocol-preserving), and a
//! supervised mixed fleet must route replacements by birth sequence without
//! crossing variants.

use std::marker::PhantomData;
use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, Crash, Delivery, Handler, MailAddr, Never, Pure, Recipient,
    RestartPolicy, Step, Strategy, SupervisionEvent, Supervisor, User, UserEvent, WorkerStopped,
    workers,
};
use tokio::time::Instant;

struct WorkerA;

impl Handler<Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, behavior::NoBirths, Never>
    for WorkerA
{
    type Addr = MailAddr;
    type Msg = u8;

    fn receive(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), 1)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

struct WorkerB;

impl Handler<Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, behavior::NoBirths, Never>
    for WorkerB
{
    type Addr = MailAddr;
    type Msg = u8;

    fn receive(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), 2)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

fn worker_a(_index: usize) -> Pure<WorkerA, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>> {
    Pure::new(WorkerA)
}

fn worker_b(_index: usize) -> Pure<WorkerB, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>> {
    Pure::new(WorkerB)
}

/// The supervising parent is generic over its offspring; instantiating it
/// with the concrete `Crew` type happens implicitly at each `build` call
/// site (the macro's `Crew` is block-scoped).
struct GenericParent<C>(PhantomData<C>);

impl<C> Handler<Vec<Never>, behavior::Births<C>, Never> for GenericParent<C>
where
    C: Behavior<Ph = Never, Addr = MailAddr>,
{
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<C>, Never> {
        Ok(Actions::cont())
    }
}

fn supervise_with<C>(
    count: usize,
    build: fn(usize) -> C,
    strategy: Strategy,
) -> Supervisor<Pure<GenericParent<C>, Vec<Never>, behavior::Births<C>>, C>
where
    C: Behavior<Ph = Never, Addr = MailAddr> + Send,
{
    Supervisor::new(
        Pure::new(GenericParent(PhantomData)),
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
    let (count, build) = workers![(2, Pure<WorkerA, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_a), (1, Pure<WorkerB, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_b)];
    assert_eq!(count, 3);

    for index in 0..3 {
        let mut worker = build(index);
        let actions = worker.transition(User::user(MailAddr(0), 7)).unwrap();
        let expected = if index < 2 { 1 } else { 2 };
        assert_eq!(actions.sends[0].message, expected);
    }
}

#[tokio::test]
#[should_panic(expected = "workers!: fleet index out of range")]
async fn workers_build_out_of_range_index_panics() {
    let (count, build) = workers![(2, Pure<WorkerA, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_a), (1, Pure<WorkerB, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_b)];
    assert_eq!(count, 3);
    let _ = build(3);
}

/// A supervised mixed fleet: every slot is wrapped in its own proxy; a
/// `RestForOne` replacement of a `WorkerA` slot (sequence 1) restarts exactly
/// the later-born slots (1 = `WorkerA`, 2 = `WorkerB`) — never the earlier
/// `WorkerA` slot 0 — and each replacement keeps its declared variant.
#[tokio::test]
async fn supervised_mixed_fleet_routes_replacements_by_birth_sequence() {
    let (count, build) = workers![(2, Pure<WorkerA, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_a), (1, Pure<WorkerB, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_b)];
    let mut supervisor = supervise_with(count, build, Strategy::RestForOne);
    let initial = supervisor.init().unwrap();
    assert_eq!(initial.creates.len(), 3);
    assert_eq!(initial.sends.child_observations.len(), 3);

    let at = Instant::now();
    let wide = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            worker: 1,
            outcome: Err(Crash::Failed),
            at,
        }))
        .unwrap();
    let routes: Vec<MailAddr> = wide
        .sends
        .replacement_commands
        .iter()
        .map(|d| d.to.resolve(MailAddr(17)))
        .collect();
    assert_eq!(routes.len(), 2);
    assert!(routes.contains(&behavior::Address::birth(MailAddr(17), 1)));
    assert!(routes.contains(&behavior::Address::birth(MailAddr(17), 2)));

    let narrow = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 2,
            worker: 2,
            outcome: Err(Crash::Failed),
            at,
        }))
        .unwrap();
    assert_eq!(narrow.sends.replacement_commands.len(), 1);
    assert_eq!(
        narrow.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 2)
    );
}

/// Three kinds: each kind's slots route to its own variant, exactly at the
/// kind boundaries.
#[tokio::test]
async fn workers_three_kinds_boundaries_route_exactly() {
    struct WorkerC;
    impl Handler<Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, behavior::NoBirths, Never>
        for WorkerC
    {
        type Addr = MailAddr;
        type Msg = u8;

        fn receive(
            &mut self,
            from: MailAddr,
            _message: u8,
        ) -> Acted<
            MailAddr,
            Never,
            Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
            behavior::NoBirths,
            Never,
        > {
            Ok(Actions {
                sends: vec![Delivery::new(Recipient::global(from), 3)],
                creates: Vec::new(),
                become_: Step::Continue,
            })
        }
    }
    fn worker_c(
        _index: usize,
    ) -> Pure<WorkerC, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>> {
        Pure::new(WorkerC)
    }

    let (count, build) = workers![
        (1, Pure<WorkerA, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_a),
        (1, Pure<WorkerB, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_b),
        (1, Pure<WorkerC, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_c)
    ];
    assert_eq!(count, 3);
    let expected = [1, 2, 3];
    for (index, tag) in expected.into_iter().enumerate() {
        let mut worker = build(index);
        let actions = worker.transition(User::user(MailAddr(0), 7)).unwrap();
        assert_eq!(actions.sends[0].message, tag);
    }
}

/// A `workers!` fleet under `OneForAll`: one death replaces every alive slot,
/// each routed to its own declared variant's nonce.
#[tokio::test]
async fn workers_one_for_all_replaces_every_slot() {
    let (count, build) = workers![(2, Pure<WorkerA, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_a), (1, Pure<WorkerB, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>>, worker_b)];
    let mut supervisor = supervise_with(count, build, Strategy::OneForAll);
    supervisor.init().unwrap();
    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    let routes: Vec<MailAddr> = actions
        .sends
        .replacement_commands
        .iter()
        .map(|d| d.to.resolve(MailAddr(17)))
        .collect();
    assert_eq!(routes.len(), 3);
    for nonce in 0..3 {
        assert!(routes.contains(&behavior::Address::birth(MailAddr(17), nonce)));
    }
    assert!(supervisor.is_alive(0));
    assert!(supervisor.is_alive(1));
    assert!(supervisor.is_alive(2));
}
