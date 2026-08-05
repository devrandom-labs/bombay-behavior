//! Deterministic boundary and edge-case attacks: initialization guards,
//! empty fleets, unknown/duplicate nonces, duplicate death redelivery,
//! restart-window pruning with future stamps, FSM mid-drain reordering,
//! nested-schedule collisions, and stash+stop interaction.

use std::time::Duration;

use behaviorpass::{
    Acted, Actions, At, AtEvent, AtId, Base, Behavior, ChildStopped, Crash, Create, Delivery, Exit,
    Fsm, MailAddr, Move, Never, Proxy, Recipient, RestartPolicy, Route, Spec,
    StashRoute, State, Step, Strategy, Supervising, SupervisionEvent, TimeReached, User, UserEvent,
};
use tokio::time::Instant;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

impl State<u8, Never, Never> for Recorder {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
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

impl State<u8, Never, Never> for StopOnZero {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: Vec::new(),
            creates: Vec::new(),
            become_: if message == 0 {
                Step::Stop(Exit::Normal)
            } else {
                Step::Continue
            },
        })
    }
}

type Child = Base<Recorder, u8>;

struct Parent;

impl State<Never, Child, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Child, Never> {
        Ok(Actions::cont())
    }
}

/// Creates one dynamic child with the message value as its nonce, once.
struct BirthingParent {
    born: bool,
}

impl State<Never, Child, Never> for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Child, Never> {
        if self.born {
            return Ok(Actions::cont());
        }
        self.born = true;
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create {
                nonce,
                child: child(0),
            }],
            become_: Step::Continue,
        })
    }
}

fn child(_index: usize) -> Child {
    Base::new(Recorder::default())
}

fn supervisor(
    strategy: Strategy,
    policy: RestartPolicy,
    maximum: u32,
    window: Duration,
    count: usize,
) -> Supervising<Base<Parent, Never, Child>, Child> {
    Supervising::new(
        Base::new(Parent),
        |index| u64::try_from(index).unwrap(),
        count,
        child,
        strategy,
        policy,
        maximum,
        window,
    )
}

type SupervisorEvent = SupervisionEvent<User<MailAddr, u64>, MailAddr>;

fn stopped(nonce: u64, outcome: Result<Exit<MailAddr>, Crash>, at: Instant) -> SupervisorEvent {
    SupervisionEvent::ChildStopped(ChildStopped {
        nonce,
        outcome,
        at,
    })
}

#[tokio::test]
#[should_panic(expected = "a proxy initializes once")]
async fn proxy_double_init_is_rejected() {
    let mut proxy = Proxy::new(child(0));
    proxy.init().await.unwrap();
    proxy.init().await.unwrap();
}

#[tokio::test]
async fn empty_fleet_supervisor_initializes_and_steps_cleanly() {
    let mut supervisor = Spec::new(Parent).children((0, child));
    let initial = supervisor.init().await.unwrap();
    assert!(initial.creates.is_empty());
    assert!(initial.sends.own.inner.is_empty());

    let actions = supervisor
        .step(UserEvent::user(MailAddr(0), 3))
        .await
        .unwrap();
    assert!(actions.creates.is_empty());
    assert!(actions.sends.own.inner.is_empty());
    assert_eq!(supervisor.behavior().child_count(), 0);
}

#[tokio::test]
#[should_panic(expected = "unknown supervised nonce")]
async fn child_stopped_for_unknown_nonce_panics() {
    let mut supervisor = Spec::new(Parent).children((1, child));
    supervisor.init().await.unwrap();
    supervisor
        .step(stopped(42, Err(Crash::Failed), Instant::now()))
        .await
        .unwrap();
}

/// A redelivered `ChildStopped` (duplicate environmental event) triggers a
/// second replacement: supervision does NOT deduplicate death notices. Each
/// duplicate consumes budget and emits a fresh replacement send.
#[tokio::test]
async fn duplicate_child_stopped_triggers_a_second_restart() {
    let at = Instant::now();
    let mut supervisor = supervisor(Strategy::OneForOne, RestartPolicy::Permanent, 5, Duration::MAX, 1);
    supervisor.init().await.unwrap();

    let first = supervisor.step(stopped(0, Err(Crash::Failed), at)).await.unwrap();
    assert_eq!(first.sends.own.own.len(), 1);
    assert_eq!(supervisor.restarts_in_window(), 1);

    let duplicate = supervisor
        .step(stopped(0, Err(Crash::Failed), at))
        .await
        .unwrap();
    assert_eq!(duplicate.sends.own.own.len(), 1);
    assert_eq!(supervisor.restarts_in_window(), 2);
}

/// `children_with_nonces` accepts duplicate configured nonces: two slots
/// share one route and a death notice addresses only the first. The fresh-
/// nonce guard exists only for dynamic births.
#[tokio::test]
async fn duplicate_configured_nonces_create_ambiguous_routes() {
    let mut supervisor = Supervising::new(
        Base::new(Parent),
        |_| 7,
        2,
        child,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    let initial = supervisor.init().await.unwrap();
    assert_eq!(initial.creates.len(), 2);
    assert_eq!(initial.creates[0].nonce, 7);
    assert_eq!(initial.creates[1].nonce, 7);
    assert_eq!(initial.sends.own.inner.len(), 2);

    let actions = supervisor
        .step(stopped(7, Err(Crash::Failed), Instant::now()))
        .await
        .unwrap();
    assert_eq!(actions.sends.own.own.len(), 1);
    assert_eq!(actions.sends.own.own[0].to.route(), Route::Child(7));
    assert_eq!(supervisor.child_count(), 2);
}

#[tokio::test]
async fn transient_policy_restarts_only_abnormal_outcomes() {
    let at = Instant::now();
    let mut supervisor = supervisor(Strategy::OneForOne, RestartPolicy::Transient, u32::MAX, Duration::MAX, 3);
    supervisor.init().await.unwrap();

    let normal = supervisor.step(stopped(0, Ok(Exit::Normal), at)).await.unwrap();
    assert!(normal.sends.own.own.is_empty());
    assert!(!supervisor.is_alive(0));

    let collected = supervisor.step(stopped(0, Ok(Exit::Collected), at)).await.unwrap();
    assert!(collected.sends.own.own.is_empty());

    let link = supervisor
        .step(stopped(1, Ok(Exit::LinkDied(MailAddr(9))), at))
        .await
        .unwrap();
    assert_eq!(link.sends.own.own.len(), 1);
    assert!(supervisor.is_alive(1));

    let failed = supervisor.step(stopped(2, Err(Crash::Failed), at)).await.unwrap();
    assert_eq!(failed.sends.own.own.len(), 1);
    assert!(supervisor.is_alive(2));
}

/// `OneForAll` candidate set excludes slots previously denied; the dead slot
/// itself is never resurrected by another child's death. Note the budget
/// counts every replacement — the first `OneForAll` restart of 3 children
/// consumes 3 stamps.
#[tokio::test]
async fn one_for_all_skips_dead_slots_and_respects_budget() {
    let at = Instant::now();
    let mut supervisor = supervisor(Strategy::OneForAll, RestartPolicy::Permanent, 5, Duration::MAX, 3);
    supervisor.init().await.unwrap();

    let first = supervisor.step(stopped(0, Err(Crash::Failed), at)).await.unwrap();
    assert_eq!(first.sends.own.own.len(), 3);

    // Second death: 3 alive candidates + 3 prior stamps = 6 > budget 5.
    let denied = supervisor.step(stopped(1, Err(Crash::Failed), at)).await.unwrap();
    assert!(denied.sends.own.own.is_empty());
    assert!(!supervisor.is_alive(1));

    // Third death: candidates are now only {0, 2} — the dead slot 1 is
    // excluded — so 3 + 2 = 5 <= budget: BOTH alive slots restart, and the
    // dead slot 1 is not resurrected. (Had slot 1 been included, 3 + 3 = 6
    // > 5 would have denied everything.)
    let third = supervisor.step(stopped(2, Err(Crash::Failed), at)).await.unwrap();
    assert_eq!(third.sends.own.own.len(), 2);
    let routes: Vec<_> = third.sends.own.own.iter().map(|d| d.to.route()).collect();
    assert!(routes.contains(&Route::Child(0)));
    assert!(routes.contains(&Route::Child(2)));
    assert!(!routes.contains(&Route::Child(1)));
    assert!(supervisor.is_alive(2));
    assert!(supervisor.is_alive(0));
    assert!(!supervisor.is_alive(1));
}

/// `RestForOne` candidates are birth-sequence ordered, not index ordered: a
/// dynamic birth (sequence 3) is only ever replaced alongside later births.
#[tokio::test]
async fn rest_for_one_uses_birth_sequence_not_index() {
    let at = Instant::now();
    let mut supervisor = Supervising::new(
        Base::new(BirthingParent { born: false }),
        |index| u64::try_from(index).unwrap(),
        3,
        child,
        Strategy::RestForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    supervisor.init().await.unwrap();
    supervisor
        .step(UserEvent::user(MailAddr(0), 9))
        .await
        .unwrap();
    assert_eq!(supervisor.child_count(), 4);

    // Death of configured slot 0 (sequence 0): every alive slot restarts.
    let wide = supervisor.step(stopped(0, Err(Crash::Failed), at)).await.unwrap();
    assert_eq!(wide.sends.own.own.len(), 4);
    let routes: Vec<_> = wide.sends.own.own.iter().map(|d| d.to.route()).collect();
    for nonce in 0..3 {
        assert!(routes.contains(&Route::Child(nonce)));
    }
    assert!(routes.contains(&Route::Child(9)));

    // Death of dynamic slot 9 (sequence 3): only later-born slots restart —
    // the configured slots (sequences 0..3) are untouched even though they
    // sit at lower indexes.
    let narrow = supervisor
        .step(stopped(9, Err(Crash::Failed), at))
        .await
        .unwrap();
    assert_eq!(narrow.sends.own.own.len(), 1);
    assert_eq!(narrow.sends.own.own[0].to.route(), Route::Child(9));
}

/// Window pruning is lazy (evaluated at each death) and inclusive at the
/// edge; stamps in the future relative to the death timestamp survive.
#[tokio::test]
async fn restart_window_prunes_aged_stamps_but_keeps_future_ones() {
    let start = Instant::now();
    let window = Duration::from_nanos(50);
    let mut supervisor = supervisor(Strategy::OneForOne, RestartPolicy::Permanent, u32::MAX, window, 1);
    supervisor.init().await.unwrap();

    supervisor
        .step(stopped(0, Err(Crash::Failed), start))
        .await
        .unwrap();
    assert_eq!(supervisor.restarts_in_window(), 1);

    // 100ns later: the earlier stamp aged out before the budget check.
    supervisor
        .step(stopped(0, Err(Crash::Failed), start + Duration::from_nanos(100)))
        .await
        .unwrap();
    assert_eq!(supervisor.restarts_in_window(), 1);

    // A death stamped BEFORE the previous one keeps the future stamp (age
    // computation underflows to "keep") and adds a new one.
    supervisor
        .step(stopped(0, Err(Crash::Failed), start + Duration::from_nanos(60)))
        .await
        .unwrap();
    assert_eq!(supervisor.restarts_in_window(), 2);
}

/// A stash layer that holds everything: the release trigger's own processing
/// stops the fold, the drain is skipped, and held messages survive the stop.
#[tokio::test]
async fn stash_stop_skips_drain_and_preserves_held_messages() {
    let mut behavior = Spec::new(StopOnZero::default()).stash(|message| match message {
        0 => StashRoute::Release,
        _ => StashRoute::Stash,
    });
    behavior.step(UserEvent::user(MailAddr(1), 5)).await.unwrap();
    assert_eq!(behavior.behavior().held(), 1);

    let actions = behavior.step(UserEvent::user(MailAddr(9), 0)).await.unwrap();
    assert_eq!(actions.become_, Step::Stop(Exit::Normal));
    assert_eq!(behavior.behavior().inner().state().seen, [(MailAddr(9), 0)]);
    assert_eq!(behavior.behavior().held(), 1);
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
    let mut machine = Fsm::new(
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
    for message in [Message::A, Message::B, Message::C] {
        machine.step(User::user(MailAddr(0), message)).await.unwrap();
    }
    machine.step(User::user(MailAddr(0), Message::Open)).await.unwrap();

    // Original FIFO was A, B, C; B was the phase-changer (consumed in P1),
    // A deferred in P1 and replayed in P2 after C.
    assert_eq!(machine.state(), &['c', 'a']);
    assert_eq!(machine.held(), 0);
}

/// Two At layers scheduled with the SAME (id, at) collapse onto one event:
/// the outermost layer claims the first delivery, so the inner schedule
/// fires only on a second (duplicate) delivery. Deterministic, but the two
/// schedules are not independently observable.
#[tokio::test]
async fn nested_at_identical_schedules_collapse_onto_the_outer_layer() {
    let due = Instant::now() + Duration::from_secs(1);
    let inner = At::new(Base::new(Recorder::default()), Some(due), |_| {
        Ok(Step::Stop(Exit::Normal))
    });
    let mut outer = At::new(inner, Some(due), |_| Ok(Step::Continue));
    outer.init().await.unwrap();

    let event = AtEvent::Reached(TimeReached {
        id: AtId(0),
        at: due,
    });
    let first = outer.step(event.clone()).await.unwrap();
    assert_eq!(first.become_, Step::Continue); // outer fired (Continue)

    let second = outer.step(event).await.unwrap();
    assert_eq!(second.become_, Step::Stop(Exit::Normal)); // inner fired on redelivery
}

/// `Spec::children` defaults are Transient policy with a budget of one
/// restart per 5-second window: a second abnormal death inside the window
/// is denied, even under a strategy that would otherwise restart. The
/// builder's explicit `when`/`within` calls are required for unbounded
/// restart semantics.
#[tokio::test]
async fn spec_children_defaults_to_transient_with_budget_one() {
    let at = Instant::now();
    let mut supervisor = Spec::new(Parent).children((1, child));
    supervisor.init().await.unwrap();

    let first = supervisor
        .step(stopped(0, Err(Crash::Failed), at))
        .await
        .unwrap();
    assert_eq!(first.sends.own.own.len(), 1);

    let second = supervisor
        .step(stopped(0, Err(Crash::Failed), at + Duration::from_secs(1)))
        .await
        .unwrap();
    assert!(second.sends.own.own.is_empty());
    assert!(!supervisor.behavior().is_alive(0));

    // A normal exit is never restarted under the default Transient policy.
    let normal = supervisor
        .step(stopped(0, Ok(Exit::Normal), at))
        .await
        .unwrap();
    assert!(normal.sends.own.own.is_empty());
}
