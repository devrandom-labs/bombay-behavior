//! Model-based supervision attacks: arbitrary child-stopped sequences
//! (strategy x policy x budget x window x timestamp orderings, including
//! equal and backwards timestamps) and mixed sequences interleaving dynamic
//! births with deaths — all checked against the independent reference model
//! in `behavior_testkit::model`.

use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, ChildStopped, Crash, Create, CreationKind, CreationResolved,
    Delivery, MailAddr, Never, Proxy, ProxyCommand, ProxyEvent, RestartPolicy, Step, Strategy,
    SupervisionEvent, Supervisor, User, UserEvent, WorkerStopped, stop_on_supervision_failure,
};
use behavior_testkit::model::{
    ExpectedCreation, ExpectedIncarnation, IncarnationModel, Model, Outcome,
};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;
use tokio::runtime::Builder;

#[derive(Default)]
struct Echo;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Echo {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }
}

type Child = Echo;

/// Quiet parent for the static-fleet property.
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

/// Parent that creates one dynamic child (nonce = message value) per user
/// message; the generator guarantees distinct nonces.
struct BirthingParent {
    births: Vec<u64>,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl BirthingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        self.births.push(nonce);
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::birth(nonce, child(0))],
            become_: Step::Continue,
        })
    }
}

fn child(_index: usize) -> Child {
    Echo
}

fn supervisor<B>(
    inner: B,
    strategy: Strategy,
    policy: RestartPolicy,
    maximum: u32,
    window: Duration,
    count: usize,
) -> Supervisor<B, Child>
where
    B: Behavior<Birth = behavior::Births<Child>, Addr = MailAddr>,
{
    Supervisor::new(
        inner,
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            count,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(strategy, policy, maximum, window),
    )
    .unwrap()
}

fn assert_expected_creation(expected: ExpectedIncarnation, actual: &Create<MailAddr, Child>) {
    assert_eq!(actual.nonce, expected.nonce);
    assert_eq!(
        actual.kind,
        match expected.role {
            ExpectedCreation::Initial | ExpectedCreation::Ordinary => CreationKind::Birth,
            ExpectedCreation::Successor => CreationKind::ReplacementIncarnation {
                replaces: expected.nonce - 1,
            },
        }
    );
}

#[tokio::test]
async fn creation_provenance_matches_the_independent_incarnation_model() {
    let mut model = IncarnationModel::new();
    let proxy = Proxy::new(child(0));

    let expected_initial = model.initialize();
    let initialized = proxy.initialize().unwrap();
    let initial = initialized.actions;
    let mut proxy = initialized.behavior;
    assert_expected_creation(expected_initial, &initial.creates[0]);
    proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(()),
        }))
        .unwrap();

    let expected_ordinary = IncarnationModel::ordinary(9);
    let ordinary = Create::birth(9, child(0));
    assert_expected_creation(expected_ordinary, &ordinary);

    assert_eq!(model.request_successor(), None);
    let requested = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .unwrap();
    assert!(requested.creates.is_empty());

    let expected_deferred = model.stopped(0).unwrap();
    let deferred = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_expected_creation(expected_deferred, &deferred.creates[0]);
}

#[tokio::test]
async fn immediate_and_denied_replacements_match_the_independent_models() {
    let mut incarnation = IncarnationModel::new();
    let _initial = incarnation.initialize();
    assert_eq!(incarnation.stopped(0), None);

    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let mut proxy = initialized.behavior;
    proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(()),
        }))
        .unwrap();
    proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    let expected_immediate = incarnation.request_successor().unwrap();
    let immediate = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .unwrap();
    assert_expected_creation(expected_immediate, &immediate.creates[0]);

    let mut policy = Model::new(1);
    let denied = policy.apply(
        0,
        Outcome::Failed,
        0,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        0,
        None,
    );
    assert!(denied.is_empty());

    let behavior = supervisor(
        Parent,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        0,
        Duration::MAX,
        1,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let denied = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert!(denied.creates.is_empty());
    assert!(denied.sends.replacement_commands.is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn supervision_matches_the_reference_model(
        count in 1_usize..8,
        strategy_tag in 0_u8..3,
        policy_tag in 0_u8..3,
        maximum in 0_u32..5,
        window_nanos in 0_u64..400,
        no_window in any::<bool>(),
        events in vec((0_u64..8, 0_u8..4, 0_u64..400), 0..60),
    ) {
        let strategy = match strategy_tag {
            0 => Strategy::OneForOne,
            1 => Strategy::OneForAll,
            _ => Strategy::RestForOne,
        };
        let policy = match policy_tag {
            0 => RestartPolicy::Permanent,
            1 => RestartPolicy::Transient,
            _ => RestartPolicy::Temporary,
        };
        let window = if no_window {
            None
        } else {
            Some(window_nanos)
        };
        let window_duration = window.map_or(Duration::MAX, Duration::from_nanos);

        let mut model = Model::new(count);
        let behavior = supervisor(Parent, strategy, policy, maximum, window_duration, count)
            .with_failure_reaction(stop_on_supervision_failure);
        let base = Instant::now();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;

        for (dead_seed, outcome_tag, at) in events {
            let dead = dead_seed % u64::try_from(count).unwrap();
            let outcome = Outcome::from_tag(outcome_tag);
            let expected = model.apply(dead, outcome, at, strategy, policy, maximum, window);
            let actions = runtime
                .block_on(async { behavior.transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                    proxy: dead,
                    worker: dead,
            outcome: outcome.into_result(),
                    at: base + Duration::from_nanos(at),
                })) })
                .unwrap();

            let sends: Vec<MailAddr> = actions
                .sends.replacement_commands
                .iter()
                .map(|delivery| delivery.to.resolve(MailAddr(17)))
                .collect();
            let expected: Vec<MailAddr> = expected
                .into_iter()
                .map(|nonce| behavior::Address::birth(MailAddr(17), nonce))
                .collect();
            prop_assert_eq!(sends, expected);
            prop_assert!(actions.creates.is_empty());
            if model.last_restart_denied() {
                prop_assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
            } else {
                prop_assert_eq!(actions.become_, Step::Continue);
            }

            for nonce in 0..count {
                prop_assert_eq!(
                    behavior.is_alive(u64::try_from(nonce).unwrap()).unwrap(),
                    model.alive(u64::try_from(nonce).unwrap()).unwrap(),
                    "alive mismatch nonce={} dead={} outcome={} at={}",
                    nonce,
                    dead,
                    outcome_tag,
                    at
                );
            }
            prop_assert_eq!(behavior.restarts_in_window(), model.restarts());
        }
    }

    #[test]
    fn mixed_dynamic_births_and_deaths_match_the_reference_model(
        count in 1_usize..4,
        strategy_tag in 0_u8..3,
        maximum in 0_u32..4,
        window_nanos in 0_u64..120,
        events in vec((0_u8..4, 0_u8..8, 0_u64..100), 0..40),
    ) {
        let strategy = match strategy_tag {
            0 => Strategy::OneForOne,
            1 => Strategy::OneForAll,
            _ => Strategy::RestForOne,
        };
        // Finite window: births interleave with deaths under budget AND
        // window pruning — the cross product the static-fleet property and
        // the MAX-window mixed property each cover only partially.
        let mut model = Model::new(count);
        let behavior = supervisor(
            BirthingParent { births: Vec::new() },
            strategy,
            RestartPolicy::Permanent,
            maximum,
            Duration::from_nanos(window_nanos),
            count,
        );
        let base = Instant::now();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;

        let mut births: u64 = u64::try_from(count).unwrap();
        for (tag, arg, at) in events {
            if tag == 0 {
                // Dynamic birth with a fresh nonce.
                let nonce = births;
                births += 1;
                model.birth(nonce);
                let actions = runtime
                    .block_on(async { behavior.transition(SupervisionEvent::Behavior(UserEvent::user(
                        MailAddr(0),
                        nonce,
                    ))) })
                    .unwrap();
                prop_assert_eq!(actions.creates.len(), 1);
                prop_assert_eq!(actions.creates[0].nonce, nonce);
                // The born child is observed exactly once.
                prop_assert_eq!(actions.sends.child_observations.len(), 1);
                prop_assert_eq!(actions.sends.child_observations[0].nonce, nonce);
                prop_assert_eq!(actions.sends.creation_observations.len(), 1);
                prop_assert_eq!(actions.sends.creation_observations[0].nonce, nonce);
            } else {
                // Child-stopped for an existing slot.
                let known = model.slot_count();
                let nonce = u64::from(arg) % u64::try_from(known).unwrap();
                let outcome = Outcome::from_tag(tag);
                let expected = model.apply(
                    nonce,
                    outcome,
                    at,
                    strategy,
                    RestartPolicy::Permanent,
                    maximum,
                    Some(window_nanos),
                );
                let actions = runtime
                    .block_on(async { behavior.transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                        proxy: nonce,
                        worker: nonce,
            outcome: outcome.into_result(),
                        at: base + Duration::from_nanos(at),
                    })) })
                    .unwrap();
                let sends: Vec<MailAddr> = actions
                    .sends.replacement_commands
                    .iter()
                    .map(|delivery| delivery.to.resolve(MailAddr(17)))
                    .collect();
                let expected: Vec<MailAddr> = expected
                    .into_iter()
                    .map(|child| behavior::Address::birth(MailAddr(17), child))
                    .collect();
                prop_assert_eq!(sends, expected);
            }

            for slot in model.slots() {
                prop_assert_eq!(
                    behavior.is_alive(slot.nonce).unwrap(),
                    slot.alive,
                    "alive mismatch for nonce {}",
                    slot.nonce
                );
            }
            prop_assert_eq!(behavior.child_count(), model.slot_count());
            prop_assert_eq!(behavior.restarts_in_window(), model.restarts());
        }
    }
}
/// Deterministic window-rolling: the budget is per window, so an aged-out
/// restart frees budget for a later one.
#[tokio::test]
async fn budget_recovers_after_stamps_age_out_of_the_window() {
    let base = Instant::now();
    let behavior = supervisor(
        Parent,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        3,
        Duration::from_nanos(100),
        1,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    for offset in 0..3 {
        behavior
            .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                proxy: 0,
                worker: 0,
                outcome: Err(Crash::Failed),
                at: base + Duration::from_nanos(offset),
            }))
            .unwrap();
    }
    assert_eq!(behavior.restarts_in_window(), 3);

    // Deadline 3ns: 3 stamps + 1 candidate = 4 > 3, denied.
    let denied = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(3),
        }))
        .unwrap();
    assert!(denied.sends.replacement_commands.is_empty());
    assert!(!behavior.is_alive(0).unwrap());

    // Deadline 100ns: all three stamps still inside the inclusive window; denied.
    let edge = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(100),
        }))
        .unwrap();
    assert!(edge.sends.replacement_commands.is_empty());

    // Deadline 101ns: the stamp at 0ns aged out (age 101 > 100); budget recovers.
    let recovered = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(101),
        }))
        .unwrap();
    assert_eq!(recovered.sends.replacement_commands.len(), 1);
    assert!(behavior.is_alive(0).unwrap());
}

/// With a finite window, the restart-stamp vector stays bounded by the
/// window's time density: 1000 deaths at 1ns spacing under a 50ns window
/// never hold more than 51 stamps (memory bounded by window, not by total
/// emitted events).
#[tokio::test]
async fn restart_stamps_stay_bounded_by_the_window() {
    let base = Instant::now();
    let behavior = supervisor(
        Parent,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::from_nanos(50),
        1,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let mut peak = 0_usize;
    for offset in 0..1000 {
        behavior
            .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                proxy: 0,
                worker: 0,
                outcome: Err(Crash::Failed),
                at: base + Duration::from_nanos(offset),
            }))
            .unwrap();
        peak = peak.max(behavior.restarts_in_window());
    }
    // Deadline 1ns spacing, at most 51 distinct stamps fit inside a 50ns window.
    assert!(
        peak <= 51,
        "peak restart stamps {peak} exceed the window bound"
    );
    assert!(peak >= 51, "window never filled: peak {peak}");
}
use behavior_testkit::InitializeTest;
