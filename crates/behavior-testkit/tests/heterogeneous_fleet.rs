//! Explicit heterogeneous-worker sum checks: each fleet index dispatches to
//! its declared concrete variant, and supervision routes replacements by birth
//! sequence without crossing variants.

use std::marker::PhantomData;
use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, Crash, Delivery, MailAddr, Never, Recipient, RestartPolicy, Step,
    Strategy, SupervisionEvent, Supervisor, User, UserEvent, WorkerStopped,
};
use std::time::Instant;

struct WorkerA;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl WorkerA {
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

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl WorkerB {
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

fn worker_a(_index: usize) -> WorkerA {
    WorkerA
}

fn worker_b(_index: usize) -> WorkerB {
    WorkerB
}

enum Worker {
    A(WorkerA),
    B(WorkerB),
}

impl behavior::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = behavior::NoBirths;

    fn init(&mut self, turn: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        match self {
            Self::A(worker) => worker.init(turn),
            Self::B(worker) => worker.init(turn),
        }
    }

    fn transition(
        &mut self,
        turn: behavior::ActiveTurn,
        event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        match self {
            Self::A(worker) => worker.transition(turn, event),
            Self::B(worker) => worker.transition(turn, event),
        }
    }
}

fn build_worker(index: usize) -> Option<Worker> {
    match index {
        0..2 => Some(Worker::A(worker_a(index))),
        2 => Some(Worker::B(worker_b(index))),
        _ => None,
    }
}

/// The supervising parent is generic over its offspring; instantiating it
/// with the concrete `Worker` type happens at each `build` call site.
struct GenericParent<C>(PhantomData<C>);

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<C>, error = Never)]
impl<C> GenericParent<C>
where
    C: Behavior<Ph = Never>,
    C::Protocol: behavior::Protocol<Addr = MailAddr>,
{
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
    build: fn(usize) -> Option<C>,
    strategy: Strategy,
) -> Supervisor<GenericParent<C>, C>
where
    C: Behavior<Ph = Never> + Send,
    C::Protocol: behavior::Protocol<Addr = MailAddr>,
{
    Supervisor::new(
        GenericParent(PhantomData),
        behavior::ChildTopology::indexed(|index| u64::try_from(index).unwrap(), count, build),
        behavior::RestartConfiguration::new(
            strategy,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
        ),
    )
    .unwrap()
}

/// The sum total and per-index variant dispatch are exact: slots 0..2 are
/// `WorkerA` (tag 1), slot 2 is `WorkerB` (tag 2).
#[tokio::test]
async fn workers_sum_preserves_the_concrete_variant_per_index() {
    let (count, build) = (3, build_worker as fn(usize) -> Option<Worker>);
    assert_eq!(count, 3);

    for index in 0..3 {
        let mut worker = build(index).unwrap().initialize().unwrap().behavior;
        let actions = worker.transition(User::user(MailAddr(0), 7)).unwrap();
        let expected = if index < 2 { 1 } else { 2 };
        assert_eq!(actions.sends[0].message, expected);
    }
}

#[tokio::test]
async fn workers_build_out_of_range_index_is_absent() {
    let (count, build) = (3, build_worker as fn(usize) -> Option<Worker>);
    assert_eq!(count, 3);
    assert!(build(3).is_none());
}

/// A supervised mixed fleet: every slot is wrapped in its own proxy; a
/// `RestForOne` replacement of a `WorkerA` slot (sequence 1) restarts exactly
/// the later-born slots (1 = `WorkerA`, 2 = `WorkerB`) — never the earlier
/// `WorkerA` slot 0 — and each replacement keeps its declared variant.
#[tokio::test]
async fn supervised_mixed_fleet_routes_replacements_by_birth_sequence() {
    let (count, build) = (3, build_worker as fn(usize) -> Option<Worker>);
    let supervisor = supervise_with(count, build, Strategy::RestForOne);
    let initialized = supervisor.initialize().unwrap();
    let initial = initialized.actions;
    let mut supervisor = initialized.behavior;
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

/// A heterogeneous fleet under `OneForAll`: one death replaces every alive slot,
/// each routed to its own declared variant's nonce.
#[tokio::test]
async fn workers_one_for_all_replaces_every_slot() {
    let (count, build) = (3, build_worker as fn(usize) -> Option<Worker>);
    let supervisor = supervise_with(count, build, Strategy::OneForAll);
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
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
    assert!(supervisor.is_alive(0).unwrap());
    assert!(supervisor.is_alive(1).unwrap());
    assert!(supervisor.is_alive(2).unwrap());
}
use behavior_testkit::InitializeTest;
