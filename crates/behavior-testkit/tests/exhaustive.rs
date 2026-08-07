//! Exhaustive small-state enumeration of the supervision fold: every
//! sequence of up to three child-stopped events over a two-slot fleet,
//! across strategies, policies, budgets, and window edges, compared against
//! the independent reference model. This is the exhaustive counterpart to
//! the random model property: the whole small state space, not a sample.

use std::time::Duration;

use behavior::{
    Acted, Actions, Base, Behavior, Delivery, MailAddr, Never, RestartPolicy, Route, State,
    Strategy, Supervising, SupervisionEvent, WorkerStopped,
};
use behavior_testkit::model::{Model, Outcome};
use tokio::runtime::Builder;
use tokio::time::Instant;

#[derive(Default)]
struct Echo;

impl State<u8, behavior::NoBirths, Never> for Echo {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
    }
}

type Child = Base<Echo, u8>;

struct Parent;

impl State<Never, behavior::Births<Child>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Never>
    {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Child {
    Base::new(Echo)
}

const FLEET: usize = 2;
const NONCES: [u64; 2] = [0, 1];
const AT_VALUES: [u64; 2] = [0, 2];
const OUTCOMES: [Outcome; 7] = [
    Outcome::Normal,
    Outcome::Collected,
    Outcome::LinkDied,
    Outcome::Failed,
    Outcome::EnvironmentFailed,
    Outcome::Panicked,
    Outcome::Cancelled,
];
const MAX_LENGTH: usize = 3;

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive enumeration over 54 configurations is inherently a straight-line walk"
)]
fn exhaustive_supervision_sequences_match_the_reference_model() {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let base = Instant::now();

    let mut checked = 0_usize;
    for strategy in [
        Strategy::OneForOne,
        Strategy::OneForAll,
        Strategy::RestForOne,
    ] {
        for policy in [
            RestartPolicy::Permanent,
            RestartPolicy::Transient,
            RestartPolicy::Temporary,
        ] {
            for maximum in 0_u32..3 {
                for window in [None, Some(2_u64)] {
                    let window_duration = window.map_or(Duration::MAX, Duration::from_nanos);

                    // Enumerate every sequence of length 0..=MAX_LENGTH over
                    // the event alphabet (nonce x outcome x timestamp).
                    let alphabet = NONCES.len() * OUTCOMES.len() * AT_VALUES.len();
                    let mut length = 0_usize;
                    while length <= MAX_LENGTH {
                        let total = alphabet.pow(u32::try_from(length).unwrap());
                        for code in 0..total {
                            let mut events = Vec::with_capacity(length);
                            let mut rest = code;
                            for _ in 0..length {
                                let slot = rest % alphabet;
                                rest /= alphabet;
                                let nonce = NONCES[slot % NONCES.len()];
                                let outcome = OUTCOMES[(slot / NONCES.len()) % OUTCOMES.len()];
                                let at = AT_VALUES[slot / (NONCES.len() * OUTCOMES.len())];
                                events.push((nonce, outcome, at));
                            }

                            let mut model = Model::new(FLEET);
                            let mut behavior = Supervising::new(
                                Base::new(Parent),
                                |index| u64::try_from(index).unwrap(),
                                FLEET,
                                child,
                                strategy,
                                policy,
                                maximum,
                                window_duration,
                            );
                            runtime.block_on(behavior.init()).unwrap();

                            for (nonce, outcome, at) in events {
                                let expected = model
                                    .apply(nonce, outcome, at, strategy, policy, maximum, window);
                                let actions = runtime
                                    .block_on(behavior.step(SupervisionEvent::WorkerStopped(
                                        WorkerStopped {
                                            proxy: nonce,
                                            outcome: outcome.into_result(),
                                            at: base + Duration::from_nanos(at),
                                        },
                                    )))
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
                                assert_eq!(
                                    sends, expected,
                                    "strategy={strategy:?} policy={policy:?} maximum={maximum} window={window:?}"
                                );
                                for slot in model.slots() {
                                    assert_eq!(
                                        behavior.is_alive(slot.nonce),
                                        slot.alive,
                                        "alive mismatch nonce={} strategy={strategy:?} policy={policy:?} maximum={maximum} window={window:?}",
                                        slot.nonce
                                    );
                                }
                                assert_eq!(
                                    behavior.restarts_in_window(),
                                    model.restarts(),
                                    "restart count mismatch"
                                );
                            }
                            checked += 1;
                        }
                        length += 1;
                    }
                }
            }
        }
    }
    // 3 strategies x 3 policies x 3 budgets x 2 windows x (28^0 + 28 + 28^2
    // + 28^3) sequences.
    assert_eq!(checked, 3 * 3 * 3 * 2 * (1 + 28 + 28 * 28 + 28 * 28 * 28));
}
