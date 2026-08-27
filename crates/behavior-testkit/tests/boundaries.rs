//! Deterministic boundary and edge-case attacks: initialization guards,
//! empty fleets, unknown/duplicate nonces, duplicate death redelivery,
//! restart-window pruning with future stamps, FSM mid-drain reordering,
//! nested-schedule collisions, and stash+stop interaction.

use std::time::Duration;

use behavior::EventLayer;
use behavior::{
    Acted, Actions, Crash, Create, CreationKind, Delivery, Exit, Machine, MailAddr, Move, Never,
    Proxy, Recipient, RestartPolicy, StashRoute, Step, Strategy, Supervise, SupervisionEvent,
    TimerElapsed, TimerGeneration, TimerId, User, UserEvent, WorkerCreationResolved, WorkerStopped,
};
use std::time::Instant;

macro_rules! assert_supervision_counts {
    ($actions:expr, replacement_inputs = $replacement_inputs:expr) => {{
        let actions = &$actions;
        assert!(actions.sends.owned.child_observations.is_empty());
        assert!(actions.sends.owned.creation_observations.is_empty());
        assert!(actions.sends.owned.schedules.is_empty());
        assert_eq!(
            actions.sends.owned.replacement_inputs.len(),
            $replacement_inputs
        );
        assert!(actions.sends.owned.failure_reports.is_empty());
        assert!(actions.sends.owned.shutdowns.is_empty());
        assert!(actions.sends.inner.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }};
}

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

/// Stops (Normal) on message 0, records otherwise.
#[derive(Default)]
struct StopOnZero {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl StopOnZero {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        self.seen.push((from, message));
        Ok(Actions {
            sends: Vec::new(),
            creates: Vec::new(),
            become_: if message == 0 {
                Step::Stop(behavior::Stopped)
            } else {
                Step::Continue
            },
        })
    }
}

type Child = Recorder;

struct Parent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl Parent {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        if message == u64::MAX {
            Ok(Actions::create(vec![Create::birth(message, child(0))]))
        } else {
            Ok(Actions::cont())
        }
    }
}

/// Creates one dynamic child with the message value as its nonce, once.
struct BirthingParent {
    born: bool,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl BirthingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        if self.born {
            return Ok(Actions::cont());
        }
        self.born = true;
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::birth(nonce, child(0))],
            become_: Step::Continue,
        })
    }
}

fn child(_index: usize) -> Child {
    Recorder::default()
}

macro_rules! supervisor {
    ($strategy:expr, $policy:expr, $maximum:expr, $window:expr, $count:expr $(,)?) => {
        Supervise::new(
            Parent,
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                $count,
                |index| Some(child(index)),
            ),
            behavior::RestartConfiguration::new(
                $strategy,
                $policy,
                $maximum,
                $window,
                behavior::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap()
    };
}

type SupervisorEvent = SupervisionEvent<User<MailAddr, u64>>;

fn stopped(nonce: u64, outcome: Result<Exit<MailAddr>, Crash>, at: Instant) -> SupervisorEvent {
    stopped_worker(nonce, nonce, outcome, at)
}

fn stopped_worker(
    proxy: u64,
    worker: u64,
    outcome: Result<Exit<MailAddr>, Crash>,
    at: Instant,
) -> SupervisorEvent {
    SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy,
        worker,
        outcome,
        at,
    })
}

fn worker_created(proxy: u64, worker: u64, kind: CreationKind<u64>) -> SupervisorEvent {
    SupervisionEvent::WorkerCreationResolved(WorkerCreationResolved::new(
        proxy,
        worker,
        kind,
        Ok(()),
    ))
}

#[tokio::test]
async fn proxy_initialization_is_explicit() {
    let initialized = Proxy::new(child(0)).initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 1);
}

#[tokio::test]
async fn empty_fleet_supervisor_initializes_and_steps_cleanly() {
    let supervisor = behavior::Supervise::new(
        Parent,
        behavior::ChildTopology::new((0..0).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let initial = initialized.actions;
    let mut supervisor = initialized.behavior;
    assert!(initial.creates.is_empty());
    assert!(initial.sends.owned.child_observations.is_empty());

    let actions = supervisor
        .transition(UserEvent::user(MailAddr(0), 3))
        .unwrap();
    assert!(actions.creates.is_empty());
    assert!(actions.sends.owned.child_observations.is_empty());
    assert_eq!(supervisor.child_count(), 0);
}

#[tokio::test]
async fn foreign_worker_stop_is_returned_exactly_without_changing_the_fleet() {
    let supervisor = behavior::Supervise::new(
        Parent,
        behavior::ChildTopology::new((0..1).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let observed = WorkerStopped::new(42, 42, Err(Crash::Failed), Instant::now());
    assert!(matches!(
        supervisor.transition(SupervisionEvent::WorkerStopped(observed.clone())),
        Err(behavior::SuperviseError::UnexpectedWorkerStopped(returned)) if returned == observed
    ));
    assert_eq!(supervisor.child_count(), 1);
}

/// A redelivered worker termination fact is stale after replacement begins;
/// it cannot consume budget or emit a second replacement.
#[tokio::test]
async fn duplicate_worker_stopped_is_returned_during_replacement() {
    let at = Instant::now();
    let supervisor = supervisor!(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        5,
        Duration::MAX,
        1,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let first = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(first.sends.owned.replacement_inputs.len(), 1);
    assert_eq!(supervisor.restarts_in_window(), 1);

    let duplicate = WorkerStopped::new(0, 0, Err(Crash::Failed), at);
    assert!(matches!(
        supervisor.transition(SupervisionEvent::WorkerStopped(duplicate.clone())),
        Err(behavior::SuperviseError::UnexpectedWorkerStopped(returned))
            if returned == duplicate
    ));
    assert_eq!(supervisor.restarts_in_window(), 1);
}

/// Configured nonces obey the same creator-local uniqueness law as dynamic
/// births; an ambiguous topology is rejected before it can emit creations.
#[test]
fn duplicate_configured_nonces_are_rejected() {
    let result = Supervise::new(
        Parent,
        behavior::ChildTopology::indexed(|_| 7, 2, |index| Some(child(index))),
        behavior::RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    );
    assert!(matches!(
        result,
        Err(behavior::FleetError::DuplicateChild(7))
    ));
}

#[tokio::test]
async fn transient_policy_restarts_only_abnormal_outcomes() {
    let at = Instant::now();
    let supervisor = supervisor!(
        Strategy::OneForOne,
        RestartPolicy::Transient,
        u32::MAX,
        Duration::MAX,
        4,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    for proxy in 0..4 {
        let joined = supervisor
            .transition(worker_created(proxy, proxy, CreationKind::Birth))
            .unwrap();
        assert_supervision_counts!(joined, replacement_inputs = 0);
    }

    let normal = supervisor
        .transition(stopped(0, Ok(Exit::Normal), at))
        .unwrap();
    assert!(normal.sends.owned.replacement_inputs.is_empty());
    assert!(!supervisor.is_restartable(0).unwrap());

    let collected = supervisor
        .transition(stopped(1, Ok(Exit::Collected), at))
        .unwrap();
    assert!(collected.sends.owned.replacement_inputs.is_empty());

    let link = supervisor
        .transition(stopped(2, Ok(Exit::LinkDied(MailAddr(9))), at))
        .unwrap();
    assert_eq!(link.sends.owned.replacement_inputs.len(), 1);
    assert!(supervisor.is_restartable(2).unwrap());

    let mut worker = 3;
    for (generation, crash) in [
        Crash::Failed,
        Crash::EnvironmentFailed,
        Crash::Panicked,
        Crash::Cancelled,
    ]
    .into_iter()
    .enumerate()
    {
        let crashed = supervisor
            .transition(stopped_worker(3, worker, Err(crash), at))
            .unwrap();
        assert_eq!(crashed.sends.owned.replacement_inputs.len(), 1);
        assert!(supervisor.is_restartable(3).unwrap());
        let replacement = 102 + u64::try_from(generation).unwrap() * 100;
        let joined = supervisor
            .transition(worker_created(
                3,
                replacement,
                CreationKind::ReplacementIncarnation { replaces: worker },
            ))
            .unwrap();
        assert_supervision_counts!(joined, replacement_inputs = 0);
        worker = replacement;
    }
}

/// `OneForAll` candidate set excludes slots previously denied; the dead slot
/// itself is never resurrected by another child's death. Note the budget
/// counts every replacement — the first `OneForAll` restart of 3 children
/// consumes 3 stamps.
#[tokio::test]
async fn one_for_all_skips_dead_slots_and_respects_budget() {
    let at = Instant::now();
    let supervisor = supervisor!(
        Strategy::OneForAll,
        RestartPolicy::Permanent,
        5,
        Duration::MAX,
        3,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    for proxy in 0..3 {
        let joined = supervisor
            .transition(worker_created(proxy, proxy, CreationKind::Birth))
            .unwrap();
        assert_supervision_counts!(joined, replacement_inputs = 0);
    }

    let first = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(first.sends.owned.replacement_inputs.len(), 3);

    for proxy in 1..3 {
        let duplicate_stop = supervisor
            .transition(stopped(proxy, Err(Crash::Cancelled), at))
            .unwrap();
        assert_supervision_counts!(duplicate_stop, replacement_inputs = 0);
    }
    for proxy in 0..3 {
        let joined = supervisor
            .transition(worker_created(
                proxy,
                proxy + 100,
                CreationKind::ReplacementIncarnation { replaces: proxy },
            ))
            .unwrap();
        assert_supervision_counts!(joined, replacement_inputs = 0);
    }

    // Second death: 3 alive candidates + 3 prior stamps = 6 > budget 5.
    let denied = supervisor
        .transition(stopped_worker(1, 101, Err(Crash::Failed), at))
        .unwrap();
    assert!(denied.sends.owned.replacement_inputs.is_empty());
    assert!(!supervisor.is_restartable(1).unwrap());

    // Third death: candidates are now only {0, 2} — the dead slot 1 is
    // excluded — so 3 + 2 = 5 <= budget: BOTH alive slots restart, and the
    // dead slot 1 is not resurrected. (Had slot 1 been included, 3 + 3 = 6
    // > 5 would have denied everything.)
    let third = supervisor
        .transition(stopped_worker(2, 102, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(third.sends.owned.replacement_inputs.len(), 2);
    let routes: Vec<_> = third
        .sends
        .owned
        .replacement_inputs
        .iter()
        .map(|d| d.nonce)
        .collect();
    assert!(routes.contains(&0));
    assert!(routes.contains(&2));
    assert!(!routes.contains(&1));
    assert!(supervisor.is_restartable(2).unwrap());
    assert!(supervisor.is_restartable(0).unwrap());
    assert!(!supervisor.is_restartable(1).unwrap());
}

/// `RestForOne` follows declared topology order rather than sorting by nonce.
#[tokio::test]
async fn rest_for_one_uses_declared_topology_order_not_nonce_order() {
    let at = Instant::now();
    let supervisor = Supervise::new(
        BirthingParent { born: false },
        behavior::ChildTopology::new([9, 0, 5], |index| Some(child(index))),
        behavior::RestartConfiguration::new(
            Strategy::RestForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    assert_eq!(supervisor.child_count(), 3);
    for proxy in [9, 0, 5] {
        let joined = supervisor
            .transition(worker_created(proxy, proxy, CreationKind::Birth))
            .unwrap();
        assert_supervision_counts!(joined, replacement_inputs = 0);
    }

    // Nonce 0 is the second declared slot, so only it and later nonce 5 are
    // selected. Numeric nonce ordering would select a different set.
    let actions = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    let routes: Vec<_> = actions
        .sends
        .owned
        .replacement_inputs
        .iter()
        .map(|d| d.nonce)
        .collect();
    assert_eq!(routes, [0, 5]);
}

/// Window pruning is lazy (evaluated at each death) and inclusive at the
/// edge; stamps in the future relative to the death timestamp survive.
#[tokio::test]
async fn restart_window_prunes_aged_stamps_but_keeps_future_ones() {
    let start = Instant::now();
    let window = Duration::from_nanos(50);
    let supervisor = supervisor!(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        window,
        1,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let joined = supervisor
        .transition(worker_created(0, 0, CreationKind::Birth))
        .unwrap();
    assert_supervision_counts!(joined, replacement_inputs = 0);

    let requested = supervisor
        .transition(stopped(0, Err(Crash::Failed), start))
        .unwrap();
    assert_supervision_counts!(requested, replacement_inputs = 1);
    assert_eq!(supervisor.restarts_in_window(), 1);
    let joined = supervisor
        .transition(worker_created(
            0,
            100,
            CreationKind::ReplacementIncarnation { replaces: 0 },
        ))
        .unwrap();
    assert_supervision_counts!(joined, replacement_inputs = 0);

    // 100ns later: the earlier stamp aged out before the budget check.
    let requested = supervisor
        .transition(stopped_worker(
            0,
            100,
            Err(Crash::Failed),
            start + Duration::from_nanos(100),
        ))
        .unwrap();
    assert_supervision_counts!(requested, replacement_inputs = 1);
    assert_eq!(supervisor.restarts_in_window(), 1);
    let joined = supervisor
        .transition(worker_created(
            0,
            200,
            CreationKind::ReplacementIncarnation { replaces: 100 },
        ))
        .unwrap();
    assert_supervision_counts!(joined, replacement_inputs = 0);

    // A death stamped BEFORE the previous one keeps the future stamp (age
    // computation underflows to "keep") and adds a new one.
    let requested = supervisor
        .transition(stopped_worker(
            0,
            200,
            Err(Crash::Failed),
            start + Duration::from_nanos(60),
        ))
        .unwrap();
    assert_supervision_counts!(requested, replacement_inputs = 1);
    assert_eq!(supervisor.restarts_in_window(), 2);
}

/// A stash layer that holds everything: the release trigger's own processing
/// stops the fold, the drain is skipped, and held messages survive the stop.
#[tokio::test]
async fn stash_stop_skips_drain_and_preserves_held_messages() {
    let behavior = behavior::Stash::new(StopOnZero::default(), |message| match message {
        0 => StashRoute::Release,
        _ => StashRoute::Stash,
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stashed = behavior
        .transition(UserEvent::user(MailAddr(1), 5))
        .unwrap();
    assert!(stashed.sends.is_empty());
    assert!(stashed.creates.is_empty());
    assert!(matches!(stashed.become_, Step::Continue));
    assert_eq!(behavior.held(), 1);

    let actions = behavior
        .transition(UserEvent::user(MailAddr(9), 0))
        .unwrap();
    assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
    assert_eq!(behavior.base().seen, [(MailAddr(9), 0)]);
    assert_eq!(behavior.held(), 1);
}

/// A message deferred mid-drain, then replayed after a phase change inside
/// the same drain, is processed AFTER messages that were held behind it: the
/// drain merges newly deferred messages at the batch tail. Deterministic and
/// lossless, but not FIFO for this interleaving.
#[tokio::test]
async fn fsm_mid_drain_deferral_reorders_relative_to_fifo() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        P0,
        P1,
        P2,
    }
    #[derive(Clone, Copy)]
    enum Message {
        A,
        B,
        C,
        Open,
    }
    let machine = Machine::new(
        Vec::new(),
        Phase::P0,
        |phase, seen: &mut Vec<char>, message| {
            Ok::<Move<Phase>, Never>(match (phase, message) {
                (Phase::P0, Message::Open) => Move::Goto(Phase::P1),
                (Phase::P0, _) | (Phase::P1, Message::A) => Move::Defer,
                (Phase::P1, Message::B) => Move::Goto(Phase::P2),
                (Phase::P2, Message::A) => {
                    seen.push('a');
                    Move::Stay
                }
                (Phase::P2, Message::C) => {
                    seen.push('c');
                    Move::Stay
                }
                (_, _) => Move::Stay,
            })
        },
    );
    let mut machine = machine.initialize().unwrap().behavior;
    for message in [Message::A, Message::B, Message::C] {
        let deferred = machine
            .transition(User::user(MailAddr(0), message))
            .unwrap();
        assert!(deferred.sends.is_empty());
        assert!(deferred.creates.is_empty());
        assert!(matches!(deferred.become_, Step::Continue));
    }
    let opened = machine
        .transition(User::user(MailAddr(0), Message::Open))
        .unwrap();
    assert!(opened.sends.is_empty());
    assert!(opened.creates.is_empty());
    assert!(matches!(opened.become_, Step::Continue));

    // Original FIFO was A, B, C; B was the phase-changer (consumed in P1),
    // A deferred in P1 and replayed in P2 after C.
    assert_eq!(machine.state(), &['c', 'a']);
    assert_eq!(machine.held(), 0);
}

/// Two Deadline layers at the same instant retain distinct structural owners,
/// so a fact reaches exactly the layer selected by its schedule request.
#[tokio::test]
async fn nested_at_identical_schedules_are_distinguished_by_identity() {
    let due = Instant::now() + Duration::from_secs(1);
    let outer = behavior::Deadline::new(
        behavior::Deadline::new(Recorder::default(), TimerId(0), Some(due), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(1),
        Some(due),
        |_| Step::Continue,
    );
    let initialized = outer.initialize().unwrap();
    let mut outer = initialized.behavior;

    let event = EventLayer::Inner(EventLayer::Owned(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    }));
    let first = outer.transition(event).unwrap();
    assert_eq!(first.become_, Step::Stop(behavior::Stopped));
}

/// Equal timer identities at nested layers remain distinct destinations.
#[tokio::test]
async fn duplicate_nested_timer_identity_remains_addressable_at_both_paths() {
    let due = Instant::now() + Duration::from_secs(1);
    let outer = behavior::Deadline::new(
        behavior::Deadline::new(Recorder::default(), TimerId(0), Some(due), |_| {
            Step::Stop(behavior::Stopped)
        }),
        TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let mut outer = outer.initialize().unwrap().behavior;

    let outer_actions = outer
        .transition(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(outer_actions.become_, Step::Continue);

    let inner_actions = outer
        .transition(EventLayer::Inner(EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })))
        .unwrap();
    assert_eq!(inner_actions.become_, Step::Stop(behavior::Stopped));
}

/// One explicit restart configuration applies its strategy, eligibility, and
/// budget as a correlated product: a second abnormal death inside the window
/// is denied and a normal exit is ineligible under `Transient` policy.
#[tokio::test]
async fn restart_configuration_controls_eligibility_and_budget_together() {
    let at = Instant::now();
    let supervisor = behavior::Supervise::new(
        Parent,
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let first_joined = supervisor
        .transition(worker_created(0, 0, CreationKind::Birth))
        .unwrap();
    assert_supervision_counts!(first_joined, replacement_inputs = 0);
    let second_joined = supervisor
        .transition(worker_created(1, 1, CreationKind::Birth))
        .unwrap();
    assert_supervision_counts!(second_joined, replacement_inputs = 0);

    let first = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(first.sends.owned.replacement_inputs.len(), 1);
    let replacement_joined = supervisor
        .transition(worker_created(
            0,
            100,
            CreationKind::ReplacementIncarnation { replaces: 0 },
        ))
        .unwrap();
    assert_supervision_counts!(replacement_joined, replacement_inputs = 0);

    let second = supervisor
        .transition(stopped_worker(
            0,
            100,
            Err(Crash::Failed),
            at + Duration::from_secs(1),
        ))
        .unwrap();
    assert!(second.sends.owned.replacement_inputs.is_empty());
    assert!(!supervisor.is_restartable(0).unwrap());

    // A normal exit is never restarted under the selected Transient policy.
    let normal = supervisor
        .transition(stopped(1, Ok(Exit::Normal), at))
        .unwrap();
    assert!(normal.sends.owned.replacement_inputs.is_empty());
}
use behavior_testkit::InitializeTest;
