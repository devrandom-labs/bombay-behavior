//! Explicit heterogeneous-worker sum checks: each fleet index dispatches to
//! its declared concrete variant, and supervision routes replacements by birth
//! sequence without crossing variants.

use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, BehaviorActed, BehaviorBase, Crash, CreationKind, Delivery,
    EventIngress, Here, MailAddr, Never, Recipient, RestartPolicy, Step, Strategy, Supervise,
    SupervisionEvent, SupervisionLifecycle, User, UserEvent, WorkerCreationResolved, WorkerStopped,
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

struct FleetRoot;

enum FleetEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    User(User<MailAddr, ()>),
}

impl UserEvent for FleetEvent {
    type Addr = MailAddr;
    type Message = ();

    fn user(from: MailAddr, message: ()) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<MailAddr, ()>, Self> {
        match self {
            Self::User(user) => Ok(user),
            lifecycle => Err(lifecycle),
        }
    }
}

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for FleetEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl behavior::Protocol for FleetRoot {
    type Addr = MailAddr;
    type Msg = ();
}

impl BehaviorBase for FleetRoot {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for FleetRoot {
    type Protocol = Self;
    type Event = FleetEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = behavior::NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            FleetEvent::Lifecycle(_lifecycle) => {}
            FleetEvent::User(_user) => {}
        }
        Ok(Actions::cont())
    }
}

macro_rules! supervise_with {
    ($count:expr, $build:expr, $strategy:expr) => {
        Supervise::new(
            FleetRoot,
            behavior::ChildTopology::indexed(|index| u64::try_from(index).unwrap(), $count, $build),
            behavior::RestartConfiguration::new(
                $strategy,
                RestartPolicy::Permanent,
                u32::MAX,
                Duration::MAX,
                behavior::RestartTiming::Immediate,
            ),
            behavior::Proxy::new,
        )
        .unwrap()
    };
}

macro_rules! assert_quiet_supervision {
    ($actions:expr) => {{
        let actions = &$actions;
        assert!(actions.sends.owned.child_observations.is_empty());
        assert!(actions.sends.owned.creation_observations.is_empty());
        assert!(actions.sends.owned.schedules.is_empty());
        assert!(actions.sends.owned.replacement_inputs.is_empty());
        assert!(actions.sends.owned.failure_reports.is_empty());
        assert!(actions.sends.owned.shutdowns.is_empty());
        assert!(actions.sends.inner.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }};
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
    let supervisor = supervise_with!(count, build, Strategy::RestForOne);
    let initialized = supervisor.initialize().unwrap();
    let initial = initialized.actions;
    let mut supervisor = initialized.behavior;
    assert_eq!(initial.creates.len(), 3);
    assert_eq!(initial.sends.owned.child_observations.len(), 3);
    for proxy in 0..3 {
        let joined = supervisor
            .transition(SupervisionEvent::WorkerCreationResolved(
                WorkerCreationResolved::new(proxy, proxy, CreationKind::Birth, Ok(())),
            ))
            .unwrap();
        assert_quiet_supervision!(joined);
    }

    let at = Instant::now();
    let wide = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            worker: 1,
            outcome: Err(Crash::Failed),
            at,
        }))
        .unwrap();
    let routes: Vec<u64> = wide
        .sends
        .owned
        .replacement_inputs
        .iter()
        .map(|d| d.nonce)
        .collect();
    assert_eq!(routes.len(), 2);
    assert!(routes.contains(&1));
    assert!(routes.contains(&2));

    let duplicate_stop = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 2,
            worker: 2,
            outcome: Err(Crash::Cancelled),
            at,
        }))
        .unwrap();
    assert_quiet_supervision!(duplicate_stop);
    for proxy in [1, 2] {
        let joined = supervisor
            .transition(SupervisionEvent::WorkerCreationResolved(
                WorkerCreationResolved::new(
                    proxy,
                    proxy + 10,
                    CreationKind::ReplacementIncarnation { replaces: proxy },
                    Ok(()),
                ),
            ))
            .unwrap();
        assert_quiet_supervision!(joined);
    }

    let narrow = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 2,
            worker: 12,
            outcome: Err(Crash::Failed),
            at,
        }))
        .unwrap();
    assert_eq!(narrow.sends.owned.replacement_inputs.len(), 1);
    assert_eq!(narrow.sends.owned.replacement_inputs[0].nonce, 2);
}

/// A heterogeneous fleet under `OneForAll`: one death replaces every alive slot,
/// each routed to its own declared variant's nonce.
#[tokio::test]
async fn workers_one_for_all_replaces_every_slot() {
    let (count, build) = (3, build_worker as fn(usize) -> Option<Worker>);
    let supervisor = supervise_with!(count, build, Strategy::OneForAll);
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    for proxy in 0..3 {
        let joined = supervisor
            .transition(SupervisionEvent::WorkerCreationResolved(
                WorkerCreationResolved::new(proxy, proxy, CreationKind::Birth, Ok(())),
            ))
            .unwrap();
        assert_quiet_supervision!(joined);
    }
    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    let routes: Vec<u64> = actions
        .sends
        .owned
        .replacement_inputs
        .iter()
        .map(|d| d.nonce)
        .collect();
    assert_eq!(routes.len(), 3);
    for nonce in 0..3 {
        assert!(routes.contains(&nonce));
    }
    assert!(supervisor.is_restartable(0).unwrap());
    assert!(supervisor.is_restartable(1).unwrap());
    assert!(supervisor.is_restartable(2).unwrap());
}
use behavior_testkit::InitializeTest;
