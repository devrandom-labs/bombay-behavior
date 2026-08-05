//! Model-based supervision attacks: arbitrary child-stopped sequences
//! (strategy x policy x budget x window x timestamp orderings, including
//! equal and backwards timestamps) and mixed sequences interleaving dynamic
//! births with deaths — all checked against the independent reference model
//! in `behaviorpass_autoresearch::model`.

use std::time::Duration;

use behaviorpass::{
    Acted, Actions, Base, Behavior, ChildStopped, Crash, Create, Delivery, MailAddr, Never,
    RestartPolicy, Route, State, Step, Strategy, Supervising, SupervisionEvent, UserEvent,
};
use behaviorpass_autoresearch::model::{Model, Outcome};
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

/// Quiet parent for the static-fleet property.
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

/// Parent that creates one dynamic child (nonce = message value) per user
/// message; the generator guarantees distinct nonces.
struct BirthingParent {
    births: Vec<u64>,
}

impl State<Never, Child, Never> for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, Child, Never> {
        self.births.push(nonce);
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
    Base::new(Echo)
}

fn supervisor<B>(
    inner: B,
    strategy: Strategy,
    policy: RestartPolicy,
    maximum: u32,
    window: Duration,
    count: usize,
) -> Supervising<B, Child>
where
    B: Behavior<Offspring = Child, Addr = MailAddr>,
{
    Supervising::new(
        inner,
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
        let mut behavior = supervisor(Base::new(Parent), strategy, policy, maximum, window_duration, count);
        let base = Instant::now();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(behavior.init()).unwrap();

        for (dead_seed, outcome_tag, at) in events {
            let dead = dead_seed % u64::try_from(count).unwrap();
            let outcome = Outcome::from_tag(outcome_tag);
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
        events in vec((0_u8..4, 0_u8..8, 0_u64..100), 0..40),
    ) {
        let strategy = match strategy_tag {
            0 => Strategy::OneForOne,
            1 => Strategy::OneForAll,
            _ => Strategy::RestForOne,
        };
        // Window MAX (no pruning) keeps the model focused on birth-order
        // and candidate-set semantics; the static-fleet property covers
        // window pruning exhaustively elsewhere.
        let mut model = Model::new(count);
        let mut behavior = supervisor(
            Base::new(BirthingParent { births: Vec::new() }),
            strategy,
            RestartPolicy::Permanent,
            maximum,
            Duration::MAX,
            count,
        );
        let base = Instant::now();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(behavior.init()).unwrap();

        let mut births: u64 = u64::try_from(count).unwrap();
        for (tag, arg, at) in events {
            if tag == 0 {
                // Dynamic birth with a fresh nonce.
                let nonce = births;
                births += 1;
                model.birth(nonce);
                let actions = runtime
                    .block_on(behavior.step(SupervisionEvent::Inner(UserEvent::user(
                        MailAddr(0),
                        nonce,
                    ))))
                    .unwrap();
                prop_assert_eq!(actions.creates.len(), 1);
                prop_assert_eq!(actions.creates[0].nonce, nonce);
                // The born child is observed exactly once.
                prop_assert_eq!(actions.sends.own.inner.len(), 1);
                prop_assert_eq!(actions.sends.own.inner[0].message.nonce, nonce);
            } else {
                // Child-stopped for an existing slot.
                let known = model.slot_count();
                let nonce = u64::from(arg) % u64::try_from(known).unwrap();
                let outcome = Outcome::from_tag(tag);
                let expected =
                    model.apply(nonce, outcome, at, strategy, RestartPolicy::Permanent, maximum, None);
                let actions = runtime
                    .block_on(behavior.step(SupervisionEvent::ChildStopped(ChildStopped {
                        nonce,
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
            }

            for slot in model.slots() {
                prop_assert_eq!(
                    behavior.is_alive(slot.nonce),
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
    let mut behavior = supervisor(
        Base::new(Parent),
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
