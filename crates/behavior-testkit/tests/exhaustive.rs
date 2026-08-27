//! Exhaustive small-state enumeration of the supervision fold: every
//! sequence of up to three child-stopped events over a two-slot fleet,
//! across strategies, policies, budgets, and window edges, compared against
//! the independent reference model. This is the exhaustive counterpart to
//! the random model property: the whole small state space, not a sample.

use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, BehaviorActed, BehaviorBase, Births, CreationKind, Delivery,
    EventIngress, Here, MailAddr, Never, RestartPolicy, Strategy, Supervise, SuperviseError,
    SupervisionEvent, SupervisionLifecycle, User, UserEvent, WorkerCreationResolved, WorkerStopped,
};
use behavior_testkit::model::{Model, Outcome, SupervisionModelError};
use std::time::Instant;
use tokio::runtime::Builder;

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
        assert!(matches!(actions.become_, behavior::Step::Continue));
    }};
}

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

fn child(_index: usize) -> Child {
    Echo
}

struct ExhaustiveApplication;

enum ExhaustiveEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    User(User<MailAddr, ()>),
}

impl UserEvent for ExhaustiveEvent {
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

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for ExhaustiveEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl behavior::Protocol for ExhaustiveApplication {
    type Addr = MailAddr;
    type Msg = ();
}

impl BehaviorBase for ExhaustiveApplication {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for ExhaustiveApplication {
    type Protocol = Self;
    type Event = ExhaustiveEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ExhaustiveEvent::Lifecycle(_lifecycle) => {}
            ExhaustiveEvent::User(_user) => {}
        }
        Ok(Actions::cont())
    }
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
                            let behavior = Supervise::new(
                                ExhaustiveApplication,
                                behavior::ChildTopology::indexed(
                                    |index| u64::try_from(index).unwrap(),
                                    FLEET,
                                    |index| Some(child(index)),
                                ),
                                behavior::RestartConfiguration::new(
                                    strategy,
                                    policy,
                                    maximum,
                                    window_duration,
                                    behavior::RestartTiming::Immediate,
                                ),
                                behavior::Proxy::new,
                            )
                            .unwrap();
                            let initialized = behavior.initialize().unwrap();
                            let mut behavior = initialized.behavior;
                            let mut workers = [0_u64, 1];
                            let mut next_worker = 2_u64;
                            for proxy in 0..FLEET {
                                let proxy = u64::try_from(proxy).unwrap();
                                let joined = runtime
                                    .block_on(async {
                                        behavior.transition(
                                            SupervisionEvent::WorkerCreationResolved(
                                                WorkerCreationResolved::new(
                                                    proxy,
                                                    proxy,
                                                    CreationKind::Birth,
                                                    Ok(()),
                                                ),
                                            ),
                                        )
                                    })
                                    .unwrap();
                                assert_quiet_supervision!(joined);
                            }

                            for (nonce, outcome, at) in events {
                                let stopped = WorkerStopped {
                                    proxy: nonce,
                                    worker: workers[usize::try_from(nonce).unwrap()],
                                    outcome: outcome.into_result(),
                                    at: base + Duration::from_nanos(at),
                                };
                                let expected = model
                                    .apply(nonce, outcome, at, strategy, policy, maximum, window);
                                let actual = runtime.block_on(async {
                                    behavior.transition(SupervisionEvent::WorkerStopped(
                                        stopped.clone(),
                                    ))
                                });
                                let expected = match expected {
                                    Ok(expected) => expected,
                                    Err(SupervisionModelError::AlreadyStopped {
                                        nonce: rejected,
                                    }) => {
                                        assert_eq!(rejected, nonce);
                                        assert!(matches!(
                                            actual,
                                            Err(SuperviseError::UnexpectedWorkerStopped(returned))
                                                if returned == stopped
                                        ));
                                        continue;
                                    }
                                };
                                let actions = actual.unwrap();
                                let sends: Vec<u64> = actions
                                    .sends
                                    .owned
                                    .replacement_inputs
                                    .iter()
                                    .map(|delivery| delivery.nonce)
                                    .collect();
                                let expected_routes: Vec<u64> = expected.clone();
                                assert_eq!(
                                    sends, expected_routes,
                                    "strategy={strategy:?} policy={policy:?} maximum={maximum} window={window:?}"
                                );
                                for proxy in expected {
                                    let index = usize::try_from(proxy).unwrap();
                                    let previous = workers[index];
                                    if proxy != nonce {
                                        let duplicate_stop = runtime
                                            .block_on(async {
                                                behavior.transition(
                                                    SupervisionEvent::WorkerStopped(
                                                        WorkerStopped {
                                                            proxy,
                                                            worker: previous,
                                                            outcome: Err(
                                                                behavior::Crash::Cancelled,
                                                            ),
                                                            at: base + Duration::from_nanos(at),
                                                        },
                                                    ),
                                                )
                                            })
                                            .unwrap();
                                        assert_quiet_supervision!(duplicate_stop);
                                    }
                                    let joined = runtime
                                        .block_on(async {
                                            behavior.transition(
                                                SupervisionEvent::WorkerCreationResolved(
                                                    WorkerCreationResolved::new(
                                                        proxy,
                                                        next_worker,
                                                        CreationKind::ReplacementIncarnation {
                                                            replaces: previous,
                                                        },
                                                        Ok(()),
                                                    ),
                                                ),
                                            )
                                        })
                                        .unwrap();
                                    assert_quiet_supervision!(joined);
                                    workers[index] = next_worker;
                                    next_worker += 1;
                                }
                                for slot in model.slots() {
                                    assert_eq!(
                                        behavior.is_restartable(slot.nonce).unwrap(),
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
use behavior_testkit::InitializeTest;
