//! Model-based supervision attack: arbitrary child-stopped sequences
//! (strategy x policy x budget x window x timestamp orderings, including
//! equal and backwards timestamps) checked against an independent reference
//! model of the documented semantics — never against implementation
//! branches.

use std::time::Duration;

use behaviorpass::{
    Acted, Actions, Base, Behavior, ChildStopped, Crash, Delivery, Exit, MailAddr, Never,
    RestartPolicy, Route, State, Strategy, Supervising, SupervisionEvent,
};
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;
use tokio::time::Instant;

#[derive(Default)]
struct Echo;

impl State<u8, Never, Never> for Echo {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        Ok(Actions::cont())
    }
}

type Child = Base<Echo, u8>;

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

fn child(_index: usize) -> Child {
    Base::new(Echo)
}

/// Independent reference model of the supervision contract:
/// - eligibility by policy (Transient restarts only abnormal outcomes);
/// - lazy window pruning, inclusive at the edge, future stamps survive;
/// - candidate sets by strategy over ALIVE slots (`RestForOne` uses birth
///   sequence, not index);
/// - budget counts every replacement; denial kills the dead slot.
struct Model {
    slots: Vec<Slot>,
    restarts: Vec<u64>,
}

struct Slot {
    alive: bool,
    sequence: u64,
}

impl Model {
    fn new(count: usize) -> Self {
        Self {
            slots: (0..count)
                .map(|index| Slot {
                    alive: true,
                    sequence: u64::try_from(index).unwrap(),
                })
                .collect(),
            restarts: Vec::new(),
        }
    }

    /// Returns the nonces of the replacement sends the contract demands.
    #[allow(
        clippy::too_many_arguments,
        reason = "the model mirrors the supervisor's full parameter surface"
    )]
    fn apply(
        &mut self,
        dead: u64,
        outcome: Outcome,
        at: u64,
        strategy: Strategy,
        policy: RestartPolicy,
        maximum: u32,
        window: Option<u64>,
    ) -> Vec<u64> {
        // The fleet uses identity nonces (nonce == slot index), so the dead
        // nonce IS the slot position; the first-match position semantics
        // only diverge for duplicate configured nonces, which this model
        // never generates.
        let dead = usize::try_from(dead).unwrap();
        let eligible = match policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => !matches!(outcome, Outcome::Normal | Outcome::Collected),
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            self.slots[dead].alive = false;
            return Vec::new();
        }
        if let Some(window) = window {
            self.restarts
                .retain(|stamp| *stamp > at || at - stamp <= window);
        }
        let sequence = self.slots[dead].sequence;
        let candidates: Vec<usize> = match strategy {
            Strategy::OneForOne => vec![dead],
            Strategy::OneForAll => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| slot.alive.then_some(index))
                .collect(),
            Strategy::RestForOne => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| {
                    (slot.alive && slot.sequence >= sequence).then_some(index)
                })
                .collect(),
        };
        if self.restarts.len() + candidates.len() > maximum as usize {
            self.slots[dead].alive = false;
            return Vec::new();
        }
        self.restarts
            .resize(self.restarts.len() + candidates.len(), at);
        for index in &candidates {
            self.slots[*index].alive = true;
        }
        candidates
            .into_iter()
            .map(|index| u64::try_from(index).unwrap())
            .collect()
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    Normal,
    Collected,
    LinkDied,
    Failed,
}

impl Outcome {
    fn into_result(self) -> Result<Exit<MailAddr>, Crash> {
        match self {
            Self::Normal => Ok(Exit::Normal),
            Self::Collected => Ok(Exit::Collected),
            Self::LinkDied => Ok(Exit::LinkDied(MailAddr(9))),
            Self::Failed => Err(Crash::Failed),
        }
    }
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
        let mut behavior = supervisor(strategy, policy, maximum, window_duration, count);
        let base = Instant::now();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(behavior.init()).unwrap();

        for (dead_seed, outcome_tag, at) in events {
            let dead = dead_seed % u64::try_from(count).unwrap();
            let outcome = match outcome_tag {
                0 => Outcome::Normal,
                1 => Outcome::Collected,
                2 => Outcome::LinkDied,
                _ => Outcome::Failed,
            };
            let expected = model.apply(dead, outcome, at, strategy, policy, maximum, window);
            let actions = runtime
                .block_on(behavior.step(SupervisionEvent::ChildStopped(ChildStopped {
                    nonce: dead,
                    outcome: outcome.into_result(),
                    at: base + Duration::from_nanos(at),
                })))
                .unwrap();

            let sends: Vec<u64> = actions
                .sends
                .own
                .own
                .iter()
                .map(|delivery| match delivery.to.route() {
                    Route::Child(nonce) => nonce,
                    other => panic!("unexpected route {other:?}"),
                })
                .collect();
            prop_assert_eq!(sends, expected);
            prop_assert!(actions.creates.is_empty());

            for nonce in 0..count {
                prop_assert_eq!(
                    behavior.is_alive(u64::try_from(nonce).unwrap()),
                    model.slots[nonce].alive,
                    "alive mismatch nonce={} dead={} outcome={} at={}",
                    nonce,
                    dead,
                    outcome_tag,
                    at
                );
            }
            prop_assert_eq!(behavior.restarts_in_window(), model.restarts.len());
        }
    }
}

/// Deterministic window-rolling: the budget is per window, so an aged-out
/// restart frees budget for a later one.
#[tokio::test]
async fn budget_recovers_after_stamps_age_out_of_the_window() {
    let base = Instant::now();
    let mut behavior = supervisor(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        3,
        Duration::from_nanos(100),
        1,
    );
    behavior.init().await.unwrap();

    for offset in 0..3 {
        behavior
            .step(SupervisionEvent::ChildStopped(ChildStopped {
                nonce: 0,
                outcome: Err(Crash::Failed),
                at: base + Duration::from_nanos(offset),
            }))
            .await
            .unwrap();
    }
    assert_eq!(behavior.restarts_in_window(), 3);

    // At 3ns: 3 stamps + 1 candidate = 4 > 3, denied.
    let denied = behavior
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(3),
        }))
        .await
        .unwrap();
    assert!(denied.sends.own.own.is_empty());
    assert!(!behavior.is_alive(0));

    // At 100ns: all three stamps still inside the inclusive window; denied.
    let edge = behavior
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(100),
        }))
        .await
        .unwrap();
    assert!(edge.sends.own.own.is_empty());

    // At 101ns: the stamp at 0ns aged out (age 101 > 100); budget recovers.
    let recovered = behavior
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: base + Duration::from_nanos(101),
        }))
        .await
        .unwrap();
    assert_eq!(recovered.sends.own.own.len(), 1);
    assert!(behavior.is_alive(0));
}
