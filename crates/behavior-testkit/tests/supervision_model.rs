//! Model-based supervision attacks: arbitrary child-stopped sequences
//! (strategy x policy x budget x window x timestamp orderings, including
//! equal and backwards timestamps) and mixed sequences interleaving dynamic
//! births with deaths — all checked against the independent reference model
//! in `behavior_testkit::model`.

use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, BehaviorActed, BehaviorBase, Births, ChildStopped, Crash, Create,
    CreationKind, CreationResolved, Delivery, MailAddr, Never, Proxy, ProxyEvent,
    ReplacementRequested, RestartPolicy, Step, Strategy, Supervise, SuperviseError,
    SupervisionEvent, User, UserEvent, WorkerCreationResolved, WorkerStopped,
    stop_on_supervision_failure,
};
use behavior_testkit::model::{
    ExpectedCreation, ExpectedIncarnation, IncarnationModel, Model, Outcome, SupervisionModelError,
};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;

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

/// Parent that creates one dynamic child (nonce = message value) per user
/// message; the generator guarantees distinct nonces.
struct BirthingParent {
    births: Vec<u64>,
}

type ParentEvent = User<MailAddr, u64>;

impl behavior::Protocol for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;
}

impl BehaviorBase for BirthingParent {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for BirthingParent {
    type Protocol = Self;
    type Event = ParentEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        self.births.push(event.message);
        Ok(Actions::create(vec![Create::birth(
            event.message,
            child(0),
        )]))
    }
}

fn child(_index: usize) -> Child {
    Echo
}

macro_rules! supervise {
    ($inner:expr, $strategy:expr, $policy:expr, $maximum:expr, $window:expr, $count:expr $(,)?) => {
        Supervise::new(
            $inner,
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

macro_rules! supervisor {
    ($strategy:expr, $policy:expr, $maximum:expr, $window:expr, $count:expr $(,)?) => {
        supervise!(
            BirthingParent { births: Vec::new() },
            $strategy,
            $policy,
            $maximum,
            $window,
            $count
        )
    };
}

macro_rules! assert_supervision_counts {
    (
        $actions:expr;
        child_observations = $child_observations:expr,
        creation_observations = $creation_observations:expr,
        schedules = $schedules:expr,
        replacement_inputs = $replacement_inputs:expr,
        failure_reports = $failure_reports:expr,
        shutdowns = $shutdowns:expr,
        inner = $inner:expr,
        creates = $creates:expr,
        become = $become:pat_param
    ) => {{
        let actions = &$actions;
        assert_eq!(
            actions.sends.owned.child_observations.len(),
            $child_observations
        );
        assert_eq!(
            actions.sends.owned.creation_observations.len(),
            $creation_observations
        );
        assert_eq!(actions.sends.owned.schedules.len(), $schedules);
        assert_eq!(
            actions.sends.owned.replacement_inputs.len(),
            $replacement_inputs
        );
        assert_eq!(actions.sends.owned.failure_reports.len(), $failure_reports);
        assert_eq!(actions.sends.owned.shutdowns.len(), $shutdowns);
        assert_eq!(actions.sends.inner.len(), $inner);
        assert_eq!(actions.creates.len(), $creates);
        assert!(matches!(&actions.become_, $become));
    }};
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

#[test]
fn creation_provenance_matches_the_independent_incarnation_model() {
    let mut model = IncarnationModel::new();
    let proxy = Proxy::new(child(0));

    let expected_initial = model.initialize();
    let initialized = proxy.initialize().unwrap();
    let initial = initialized.actions;
    let mut proxy = initialized.behavior;
    assert_expected_creation(expected_initial, &initial.creates[0]);
    let installed = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(MailAddr(999)),
        }))
        .unwrap();
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable_reports.is_empty());
    assert!(installed.sends.child_observations.is_empty());
    assert!(installed.sends.creation_observations.is_empty());
    assert!(installed.sends.stopped_reports.is_empty());
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(installed.sends.shutdowns.is_empty());
    assert!(installed.creates.is_empty());
    assert!(matches!(installed.become_, Step::Continue));

    let expected_ordinary = IncarnationModel::ordinary(9);
    let ordinary = Create::birth(9, child(0));
    assert_expected_creation(expected_ordinary, &ordinary);

    assert_eq!(model.request_successor(), None);
    let requested = proxy
        .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
            child(0),
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

#[test]
fn immediate_and_denied_replacements_match_the_independent_models() {
    let mut incarnation = IncarnationModel::new();
    let _initial = incarnation.initialize();
    assert_eq!(incarnation.stopped(0), None);

    let proxy = Proxy::new(child(0));
    let initialized = proxy.initialize().unwrap();
    let mut proxy = initialized.behavior;
    let installed = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(MailAddr(999)),
        }))
        .unwrap();
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable_reports.is_empty());
    assert!(installed.sends.child_observations.is_empty());
    assert!(installed.sends.creation_observations.is_empty());
    assert!(installed.sends.stopped_reports.is_empty());
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(installed.sends.shutdowns.is_empty());
    assert!(installed.creates.is_empty());
    assert!(matches!(installed.become_, Step::Continue));
    let stopped = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert!(stopped.sends.deliveries.is_empty());
    assert!(stopped.sends.unavailable_reports.is_empty());
    assert!(stopped.sends.child_observations.is_empty());
    assert!(stopped.sends.creation_observations.is_empty());
    assert_eq!(stopped.sends.stopped_reports.len(), 1);
    assert!(stopped.sends.creation_reports.is_empty());
    assert!(stopped.sends.shutdowns.is_empty());
    assert!(stopped.creates.is_empty());
    assert!(matches!(stopped.become_, Step::Continue));
    let expected_immediate = incarnation.request_successor().unwrap();
    let immediate = proxy
        .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
            child(0),
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
    assert!(denied.unwrap().is_empty());

    let behavior = supervisor!(
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
    assert!(denied.sends.owned.replacement_inputs.is_empty());
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
        let behavior = supervisor!(strategy, policy, maximum, window_duration, count)
            .with_failure_reaction(stop_on_supervision_failure);
        let base = Instant::now();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;
        let mut workers: Vec<u64> = (0..u64::try_from(count).unwrap()).collect();
        let mut next_worker = u64::try_from(count).unwrap();
        for proxy in 0..u64::try_from(count).unwrap() {
            let committed = behavior
                .transition(SupervisionEvent::CreationResolved(CreationResolved::birth(
                    proxy,
                    MailAddr(10_000 + proxy),
                )))
                .unwrap();
            assert_supervision_counts!(committed;
                child_observations = 0, creation_observations = 0, schedules = 0,
                replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
                inner = 0, creates = 0, become = Step::Continue
            );
            let joined = behavior
                .transition(SupervisionEvent::WorkerCreationResolved(
                    WorkerCreationResolved::new(
                        proxy,
                        proxy,
                        CreationKind::Birth,
                        Ok(()),
                    ),
                ))
                .unwrap();
            assert_supervision_counts!(joined;
                child_observations = 0, creation_observations = 0, schedules = 0,
                replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
                inner = 0, creates = 0, become = Step::Continue
            );
        }

        for (dead_seed, outcome_tag, at) in events {
            let dead = dead_seed % u64::try_from(count).unwrap();
            let outcome = Outcome::from_tag(outcome_tag);
            let stopped = WorkerStopped {
                    proxy: dead,
                    worker: workers[usize::try_from(dead).unwrap()],
                    outcome: outcome.into_result(),
                    at: base + Duration::from_nanos(at),
                };
            let expected = model.apply(dead, outcome, at, strategy, policy, maximum, window);
            let actual = behavior.transition(SupervisionEvent::WorkerStopped(stopped.clone()));
            let expected = match expected {
                Ok(expected) => expected,
                Err(SupervisionModelError::AlreadyStopped { nonce }) => {
                    prop_assert_eq!(nonce, dead);
                    prop_assert!(matches!(
                        actual,
                        Err(SuperviseError::UnexpectedWorkerStopped(returned))
                            if returned == stopped
                    ));
                    continue;
                }
            };
            let actions = actual.unwrap();

            let sends: Vec<u64> = actions
                .sends.owned.replacement_inputs
                .iter()
                .map(|delivery| delivery.nonce)
                .collect();
            let expected_routes: Vec<u64> = expected.clone();
            prop_assert_eq!(sends, expected_routes);
            prop_assert!(actions.creates.is_empty());
            if model.last_restart_denied() {
                prop_assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
            } else {
                prop_assert_eq!(actions.become_, Step::Continue);
            }

            for proxy in expected {
                let index = usize::try_from(proxy).unwrap();
                let previous = workers[index];
                if proxy != dead {
                    let duplicate_stop = behavior.transition(SupervisionEvent::WorkerStopped(
                        WorkerStopped {
                            proxy,
                            worker: previous,
                            outcome: Err(Crash::Cancelled),
                            at: base + Duration::from_nanos(at),
                        },
                    )).unwrap();
                    assert_supervision_counts!(duplicate_stop;
                        child_observations = 0, creation_observations = 0, schedules = 0,
                        replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
                        inner = 0, creates = 0, become = Step::Continue
                    );
                }
                let replacement = next_worker;
                next_worker += 1;
                let joined = behavior.transition(
                    SupervisionEvent::WorkerCreationResolved(WorkerCreationResolved::new(
                        proxy,
                        replacement,
                        CreationKind::ReplacementIncarnation { replaces: previous },
                        Ok(()),
                    )),
                ).unwrap();
                assert_supervision_counts!(joined;
                    child_observations = 0, creation_observations = 0, schedules = 0,
                    replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
                    inner = 0, creates = 0, become = Step::Continue
                );
                workers[index] = replacement;
            }

            for nonce in 0..count {
                prop_assert_eq!(
                    behavior.is_restartable(u64::try_from(nonce).unwrap()).unwrap(),
                    model.alive(u64::try_from(nonce).unwrap()).unwrap(),
                    "restartability mismatch nonce={} dead={} outcome={} at={}",
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
    fn arbitrary_inner_births_remain_outside_fixed_supervision(
        count in 1_usize..4,
        births in vec(any::<u64>(), 0..40),
    ) {
        let behavior = supervise!(
            BirthingParent { births: Vec::new() },
            Strategy::OneForAll,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
            count,
        );
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;
        for nonce in births {
            let actions = behavior
                .transition(SupervisionEvent::Behavior(UserEvent::user(MailAddr(0), nonce)))
                .unwrap();
            prop_assert_eq!(actions.creates.len(), 1);
            prop_assert_eq!(actions.creates[0].nonce, nonce);
            prop_assert!(actions.sends.owned.child_observations.is_empty());
            prop_assert!(actions.sends.owned.creation_observations.is_empty());
            prop_assert!(actions.sends.owned.replacement_inputs.is_empty());
            prop_assert_eq!(behavior.child_count(), count);
            prop_assert_eq!(behavior.restarts_in_window(), 0);
        }
    }
}
/// Deterministic window-rolling: the budget is per window, so an aged-out
/// restart frees budget for a later one.
#[test]
fn budget_recovers_after_stamps_age_out_of_the_window() {
    let base = Instant::now();
    let behavior = supervisor!(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        3,
        Duration::from_nanos(100),
        1,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let mut worker = 0;
    let mut next_worker = 1;

    for offset in 0..3 {
        let requested = behavior
            .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                proxy: 0,
                worker,
                outcome: Err(Crash::Failed),
                at: base + Duration::from_nanos(offset),
            }))
            .unwrap();
        assert_supervision_counts!(requested;
            child_observations = 0, creation_observations = 0, schedules = 0,
            replacement_inputs = 1, failure_reports = 0, shutdowns = 0,
            inner = 0, creates = 0, become = Step::Continue
        );
        let joined = behavior
            .transition(SupervisionEvent::WorkerCreationResolved(
                WorkerCreationResolved::new(
                    0,
                    next_worker,
                    CreationKind::ReplacementIncarnation { replaces: worker },
                    Ok(()),
                ),
            ))
            .unwrap();
        assert_supervision_counts!(joined;
            child_observations = 0, creation_observations = 0, schedules = 0,
            replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
            inner = 0, creates = 0, become = Step::Continue
        );
        worker = next_worker;
        next_worker += 1;
    }
    assert_eq!(behavior.restarts_in_window(), 3);

    // Deadline 101ns: the stamp at 0ns aged out (age 101 > 100); budget recovers.
    let recovered = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(101),
        }))
        .unwrap();
    assert_eq!(recovered.sends.owned.replacement_inputs.len(), 1);
    assert!(behavior.is_restartable(0).unwrap());
}

/// With a finite window, the restart-stamp vector stays bounded by the
/// window's time density: 1000 deaths at 1ns spacing under a 50ns window
/// never hold more than 51 stamps (memory bounded by window, not by total
/// emitted events).
#[test]
fn restart_stamps_stay_bounded_by_the_window() {
    let base = Instant::now();
    let behavior = supervisor!(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::from_nanos(50),
        1,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let mut worker = 0;
    let mut next_worker = 1;

    let mut peak = 0_usize;
    for offset in 0..1000 {
        let requested = behavior
            .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                proxy: 0,
                worker,
                outcome: Err(Crash::Failed),
                at: base + Duration::from_nanos(offset),
            }))
            .unwrap();
        assert_supervision_counts!(requested;
            child_observations = 0, creation_observations = 0, schedules = 0,
            replacement_inputs = 1, failure_reports = 0, shutdowns = 0,
            inner = 0, creates = 0, become = Step::Continue
        );
        let joined = behavior
            .transition(SupervisionEvent::WorkerCreationResolved(
                WorkerCreationResolved::new(
                    0,
                    next_worker,
                    CreationKind::ReplacementIncarnation { replaces: worker },
                    Ok(()),
                ),
            ))
            .unwrap();
        assert_supervision_counts!(joined;
            child_observations = 0, creation_observations = 0, schedules = 0,
            replacement_inputs = 0, failure_reports = 0, shutdowns = 0,
            inner = 0, creates = 0, become = Step::Continue
        );
        worker = next_worker;
        next_worker += 1;
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
