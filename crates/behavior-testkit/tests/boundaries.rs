//! Deterministic boundary and edge-case attacks: initialization guards,
//! empty fleets, unknown/duplicate nonces, duplicate death redelivery,
//! restart-window pruning with future stamps, FSM mid-drain reordering,
//! nested-schedule collisions, and stash+stop interaction.

use std::time::Duration;

use behavior::EventLayer;
use behavior::{
    Acted, Actions, Crash, Create, Delivery, Exit, Machine, MailAddr, Move, Never, Proxy,
    Recipient, RestartPolicy, StashRoute, Step, Strategy, SupervisionEvent, Supervisor,
    TimerElapsed, TimerGeneration, TimerId, User, UserEvent, WorkerStopped,
};
use std::time::Instant;

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
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        Ok(Actions::cont())
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

fn supervisor(
    strategy: Strategy,
    policy: RestartPolicy,
    maximum: u32,
    window: Duration,
    count: usize,
) -> Supervisor<Parent, Child> {
    Supervisor::new(
        Parent,
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            count,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(strategy, policy, maximum, window),
    )
    .unwrap()
}

type SupervisorEvent = SupervisionEvent<User<MailAddr, u64>>;

fn stopped(nonce: u64, outcome: Result<Exit<MailAddr>, Crash>, at: Instant) -> SupervisorEvent {
    SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: nonce,
        worker: nonce,
        outcome,
        at,
    })
}

#[tokio::test]
async fn proxy_initialization_is_explicit() {
    let initialized = Proxy::new(child(0)).initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 1);
}

#[tokio::test]
async fn empty_fleet_supervisor_initializes_and_steps_cleanly() {
    let supervisor = behavior::Supervisor::new(
        Parent,
        behavior::ChildTopology::new((0..0).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
        ),
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
async fn stale_child_stopped_is_inert_at_its_selected_supervisor_owner() {
    let supervisor = behavior::Supervisor::new(
        Parent,
        behavior::ChildTopology::new((0..1).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
        ),
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let actions = supervisor
        .transition(stopped(42, Err(Crash::Failed), Instant::now()))
        .unwrap();

    assert!(actions.sends.owned.replacement_commands.is_empty());
    assert!(actions.sends.owned.failure_reports.is_empty());
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(supervisor.child_count(), 1);
}

/// A redelivered `ChildStopped` (duplicate environmental event) triggers a
/// second replacement: supervision does NOT deduplicate death notices. Each
/// duplicate consumes budget and emits a fresh replacement send.
#[tokio::test]
async fn duplicate_child_stopped_triggers_a_second_restart() {
    let at = Instant::now();
    let supervisor = supervisor(
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
    assert_eq!(first.sends.owned.replacement_commands.len(), 1);
    assert_eq!(supervisor.restarts_in_window(), 1);

    let duplicate = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(duplicate.sends.owned.replacement_commands.len(), 1);
    assert_eq!(supervisor.restarts_in_window(), 2);
}

/// Configured nonces obey the same creator-local uniqueness law as dynamic
/// births; an ambiguous topology is rejected before it can emit creations.
#[test]
fn duplicate_configured_nonces_are_rejected() {
    let result = Supervisor::new(
        Parent,
        behavior::ChildTopology::indexed(|_| 7, 2, |index| Some(child(index))),
        behavior::RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
        ),
    );
    assert!(matches!(
        result,
        Err(behavior::FleetError::DuplicateChild(7))
    ));
}

#[tokio::test]
async fn transient_policy_restarts_only_abnormal_outcomes() {
    let at = Instant::now();
    let supervisor = supervisor(
        Strategy::OneForOne,
        RestartPolicy::Transient,
        u32::MAX,
        Duration::MAX,
        3,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let normal = supervisor
        .transition(stopped(0, Ok(Exit::Normal), at))
        .unwrap();
    assert!(normal.sends.owned.replacement_commands.is_empty());
    assert!(!supervisor.is_alive(0).unwrap());

    let collected = supervisor
        .transition(stopped(0, Ok(Exit::Collected), at))
        .unwrap();
    assert!(collected.sends.owned.replacement_commands.is_empty());

    let link = supervisor
        .transition(stopped(1, Ok(Exit::LinkDied(MailAddr(9))), at))
        .unwrap();
    assert_eq!(link.sends.owned.replacement_commands.len(), 1);
    assert!(supervisor.is_alive(1).unwrap());

    for crash in [
        Crash::Failed,
        Crash::EnvironmentFailed,
        Crash::Panicked,
        Crash::Cancelled,
    ] {
        let crashed = supervisor.transition(stopped(2, Err(crash), at)).unwrap();
        assert_eq!(crashed.sends.owned.replacement_commands.len(), 1);
        assert!(supervisor.is_alive(2).unwrap());
    }
}

/// `OneForAll` candidate set excludes slots previously denied; the dead slot
/// itself is never resurrected by another child's death. Note the budget
/// counts every replacement — the first `OneForAll` restart of 3 children
/// consumes 3 stamps.
#[tokio::test]
async fn one_for_all_skips_dead_slots_and_respects_budget() {
    let at = Instant::now();
    let supervisor = supervisor(
        Strategy::OneForAll,
        RestartPolicy::Permanent,
        5,
        Duration::MAX,
        3,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let first = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(first.sends.owned.replacement_commands.len(), 3);

    // Second death: 3 alive candidates + 3 prior stamps = 6 > budget 5.
    let denied = supervisor
        .transition(stopped(1, Err(Crash::Failed), at))
        .unwrap();
    assert!(denied.sends.owned.replacement_commands.is_empty());
    assert!(!supervisor.is_alive(1).unwrap());

    // Third death: candidates are now only {0, 2} — the dead slot 1 is
    // excluded — so 3 + 2 = 5 <= budget: BOTH alive slots restart, and the
    // dead slot 1 is not resurrected. (Had slot 1 been included, 3 + 3 = 6
    // > 5 would have denied everything.)
    let third = supervisor
        .transition(stopped(2, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(third.sends.owned.replacement_commands.len(), 2);
    let routes: Vec<_> = third
        .sends
        .owned
        .replacement_commands
        .iter()
        .map(|d| d.to.resolve(MailAddr(17)))
        .collect();
    assert!(routes.contains(&behavior::Address::birth(MailAddr(17), 0)));
    assert!(routes.contains(&behavior::Address::birth(MailAddr(17), 2)));
    assert!(!routes.contains(&behavior::Address::birth(MailAddr(17), 1)));
    assert!(supervisor.is_alive(2).unwrap());
    assert!(supervisor.is_alive(0).unwrap());
    assert!(!supervisor.is_alive(1).unwrap());
}

/// `RestForOne` candidates are birth-sequence ordered, not index ordered: a
/// dynamic birth (sequence 3) is only ever replaced alongside later births.
#[tokio::test]
async fn rest_for_one_uses_birth_sequence_not_index() {
    let at = Instant::now();
    let supervisor = Supervisor::new(
        BirthingParent { born: false },
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            3,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(
            Strategy::RestForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
        ),
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    supervisor
        .transition(UserEvent::user(MailAddr(0), 9))
        .unwrap();
    assert_eq!(supervisor.child_count(), 4);

    // Death of configured slot 0 (sequence 0): every alive slot restarts.
    let wide = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(wide.sends.owned.replacement_commands.len(), 4);
    let routes: Vec<_> = wide
        .sends
        .owned
        .replacement_commands
        .iter()
        .map(|d| d.to.resolve(MailAddr(17)))
        .collect();
    for nonce in 0..3 {
        assert!(routes.contains(&behavior::Address::birth(MailAddr(17), nonce)));
    }
    assert!(routes.contains(&behavior::Address::birth(MailAddr(17), 9)));

    // Death of dynamic slot 9 (sequence 3): only later-born slots restart —
    // the configured slots (sequences 0..3) are untouched even though they
    // sit at lower indexes.
    let narrow = supervisor
        .transition(stopped(9, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(narrow.sends.owned.replacement_commands.len(), 1);
    assert_eq!(
        narrow.sends.owned.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 9)
    );
}

/// Window pruning is lazy (evaluated at each death) and inclusive at the
/// edge; stamps in the future relative to the death timestamp survive.
#[tokio::test]
async fn restart_window_prunes_aged_stamps_but_keeps_future_ones() {
    let start = Instant::now();
    let window = Duration::from_nanos(50);
    let supervisor = supervisor(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        window,
        1,
    );
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    supervisor
        .transition(stopped(0, Err(Crash::Failed), start))
        .unwrap();
    assert_eq!(supervisor.restarts_in_window(), 1);

    // 100ns later: the earlier stamp aged out before the budget check.
    supervisor
        .transition(stopped(
            0,
            Err(Crash::Failed),
            start + Duration::from_nanos(100),
        ))
        .unwrap();
    assert_eq!(supervisor.restarts_in_window(), 1);

    // A death stamped BEFORE the previous one keeps the future stamp (age
    // computation underflows to "keep") and adds a new one.
    supervisor
        .transition(stopped(
            0,
            Err(Crash::Failed),
            start + Duration::from_nanos(60),
        ))
        .unwrap();
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
    behavior
        .transition(UserEvent::user(MailAddr(1), 5))
        .unwrap();
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
        machine
            .transition(User::user(MailAddr(0), message))
            .unwrap();
    }
    machine
        .transition(User::user(MailAddr(0), Message::Open))
        .unwrap();

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
            Ok(Step::Stop(behavior::Stopped))
        }),
        TimerId(1),
        Some(due),
        |_| Ok(Step::Continue),
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
            Ok(Step::Stop(behavior::Stopped))
        }),
        TimerId(0),
        Some(due),
        |_| Ok(Step::Continue),
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
    let supervisor = behavior::Supervisor::new(
        Parent,
        behavior::ChildTopology::new((0..1).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
        ),
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let first = supervisor
        .transition(stopped(0, Err(Crash::Failed), at))
        .unwrap();
    assert_eq!(first.sends.owned.replacement_commands.len(), 1);

    let second = supervisor
        .transition(stopped(0, Err(Crash::Failed), at + Duration::from_secs(1)))
        .unwrap();
    assert!(second.sends.owned.replacement_commands.is_empty());
    assert!(!supervisor.is_alive(0).unwrap());

    // A normal exit is never restarted under the selected Transient policy.
    let normal = supervisor
        .transition(stopped(0, Ok(Exit::Normal), at))
        .unwrap();
    assert!(normal.sends.owned.replacement_commands.is_empty());
}
use behavior_testkit::InitializeTest;
