//! Cross-lane isolation: in a supervised stack over a stash over a sending
//! parent, the user lane, the child-death lane, and the buffer never leak
//! into each other — user messages are routed by the stash (Deliver through,
//! Stash held, Release delivered), child deaths produce replacement sends
//! only, and neither produces the other's effects.

use std::time::Duration;

use behavior::EventLayer;
use behavior::{
    Acted, Actions, Activate, Behavior, BehaviorActed, BehaviorBase, Births, Crash, Create,
    CreationKind, Delivery, MailAddr, Never, Recipient, RestartPolicy, StashRoute, Step, Strategy,
    Supervise, SupervisionEvent, TimerElapsed, TimerGeneration, TimerId, User, UserEvent,
    WorkerCreationResolved, WorkerStopped,
};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;
use tokio::runtime::Builder;

#[derive(Default)]
struct Recorder;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Recorder {
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

type Child = Recorder;

fn child(_index: usize) -> Child {
    Recorder
}

/// A parent that echoes every user message on its own send lane (Out = u64)
/// and can birth children (`Birth = Births<Child>`). The echo lane is how the
/// test observes what the parent actually processed.
struct EchoingParent {
    seen: Vec<u64>,
}

type ParentEvent = User<MailAddr, u64>;

impl behavior::Protocol for EchoingParent {
    type Addr = MailAddr;
    type Msg = u64;
}

impl BehaviorBase for EchoingParent {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for EchoingParent {
    type Protocol = Self;
    type Event = ParentEvent;
    type Sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        self.seen.push(event.message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), event.message)],
            creates: if event.message == u64::MAX {
                vec![Create::birth(event.message, child(0))]
            } else {
                Vec::new()
            },
            become_: Step::Continue,
        })
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "the stash API routes through fn(&Msg)"
)]
fn route(message: &u64) -> StashRoute {
    match message % 3 {
        0 => StashRoute::Release,
        1 => StashRoute::Deliver,
        _ => StashRoute::Stash,
    }
}

macro_rules! user {
    ($behavior:expr, $message:expr) => {{
        let actions = $behavior
            .transition(SupervisionEvent::Behavior(UserEvent::user(
                MailAddr(9),
                $message,
            )))
            .unwrap();
        actions
            .sends
            .inner
            .iter()
            .map(|delivery| delivery.message)
            .collect::<Vec<_>>()
    }};
}

/// The user lane is filtered by the stash exactly as in the unfettered
/// stack (Deliver and Release triggers pass in order, Stash stays held), and
/// no user step ever emits a supervision send.
#[tokio::test]
async fn supervised_stash_routes_user_lane_without_cross_lane_effects() {
    let behavior = behavior::Supervise::new(
        behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
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
        behavior::Proxy::new,
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Stash-routed messages produce no echo; Deliver and Release triggers
    // pass through in order (the parent echoes what it processed).
    assert_eq!(user!(&mut behavior, 2), vec![]); // 2 % 3 = 2: Stash
    assert_eq!(user!(&mut behavior, 5), vec![]); // 5 % 3 = 2: Stash
    assert_eq!(user!(&mut behavior, 1), vec![1]); // Deliver
    assert_eq!(user!(&mut behavior, 0), vec![0]); // Release: trigger + drain re-stashes
    // A second release still cannot replay the stashed pair.
    assert_eq!(user!(&mut behavior, 0), vec![0]);
}

/// A child death produces only a replacement send: the parent's echo lane
/// and the stash buffer stay untouched.
#[tokio::test]
async fn child_death_never_leaks_into_the_user_lane() {
    let behavior = behavior::Supervise::new(
        behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
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
        behavior::Proxy::new,
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Buffer one user message first, then kill a child.
    assert!(user!(&mut behavior, 2).is_empty());
    let actions = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(actions.sends.owned.replacement_inputs.len(), 1);
    assert_eq!(actions.sends.owned.replacement_inputs[0].nonce, 0);
    assert!(actions.sends.inner.is_empty());
    assert!(actions.sends.owned.child_observations.is_empty());
}

/// The time lane through a supervised stack: the schedule send survives in
/// its own product lane beside the observe-child sends, and a Reached event
/// fires the inner Deadline (stopping the whole supervised fold) without touching
/// the user or child lanes.
#[tokio::test]
async fn supervision_preserves_inner_at_routing() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = Supervise::new(
        behavior::Deadline::new(
            EchoingParent { seen: Vec::new() },
            behavior::TimerId(0),
            Some(due),
            |_| Step::Stop(behavior::Stopped),
        ),
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
        behavior::Proxy::new,
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.inner.owned[0].at, due);
    assert_eq!(initial.sends.owned.child_observations.len(), 1);
    assert!(initial.sends.inner.inner.is_empty());
    assert_eq!(initial.creates.len(), 1);

    let fired = behavior
        .transition(SupervisionEvent::Behavior(EventLayer::Owned(
            TimerElapsed {
                id: TimerId(0),
                generation: TimerGeneration(0),
            },
        )))
        .unwrap();
    assert_eq!(fired.become_, Step::Stop(behavior::Stopped));
    assert!(fired.sends.owned.replacement_inputs.is_empty());
}

/// The full stack: supervision over at over watch over stash. All four
/// layers' init sends survive in their exact product-lane nesting, and all
/// four event lanes (user, peer, time, child) route to their own layer
/// without cross-lane leakage.
#[tokio::test]
async fn full_stack_all_four_layers_keep_their_own_lanes() {
    use behavior::{PeerStopped, TimerElapsed, stop_on_abnormal_death};

    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let behavior = Supervise::new(
        behavior::Deadline::new(
            behavior::Watch::new(
                behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
                peer,
                stop_on_abnormal_death,
            ),
            behavior::TimerId(0),
            Some(due),
            |_| Step::Continue,
        ),
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
        behavior::Proxy::new,
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.creates.len(), 2);
    assert_eq!(initial.sends.owned.child_observations.len(), 2); // observe-child x2
    assert_eq!(initial.sends.owned.replacement_inputs.len(), 0); // proxy commands
    assert_eq!(initial.sends.inner.owned[0].at, due); // schedule
    assert_eq!(initial.sends.inner.inner.owned[0].peer, peer); // observe-peer
    assert!(initial.sends.inner.inner.inner.is_empty()); // echo lane

    // User lane: Deliver routes through every layer to the parent echo.
    let actions = behavior
        .transition(SupervisionEvent::Behavior(EventLayer::Inner(
            EventLayer::Inner(UserEvent::user(MailAddr(9), 1)),
        )))
        .unwrap();
    assert_eq!(actions.sends.inner.inner.inner[0].message, 1);
    assert!(actions.sends.owned.replacement_inputs.is_empty());

    // Time lane: fires the inner Deadline.
    let fired = behavior
        .transition(SupervisionEvent::Behavior(EventLayer::Owned(
            TimerElapsed {
                id: TimerId(0),
                generation: TimerGeneration(0),
            },
        )))
        .unwrap();
    assert_eq!(fired.become_, Step::Continue);

    // Peer lane: matching peer death stops the fold.
    let died = behavior
        .transition(SupervisionEvent::Behavior(EventLayer::Inner(
            EventLayer::Owned(PeerStopped {
                peer,
                outcome: Err(Crash::Failed),
            }),
        )))
        .unwrap();
    assert!(matches!(died.become_, Step::Stop(behavior::Stopped)));

    // Child lane on a fresh stack: replacement send only.
    let fresh = Supervise::new(
        behavior::Deadline::new(
            behavior::Watch::new(
                behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
                peer,
                stop_on_abnormal_death,
            ),
            behavior::TimerId(0),
            Some(due),
            |_| Step::Continue,
        ),
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
        behavior::Proxy::new,
    )
    .unwrap();
    let initialized = fresh.initialize().unwrap();
    let mut fresh = initialized.behavior;
    let replacement = fresh
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(replacement.sends.owned.replacement_inputs.len(), 1);
    assert_eq!(replacement.sends.owned.replacement_inputs[0].nonce, 0);
    assert!(replacement.sends.inner.inner.inner.is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    /// Randomized full-stack property: random interleavings of user, peer,
    /// time, and child events through supervision ∘ at ∘ watch ∘ stash.
    /// Each lane's effects land in exactly its product-lane, verdicts follow
    /// the per-lane model, and no lane leaks into another.
    #[test]
    fn full_stack_random_lane_routing_never_leaks(
        events in vec((0_u8..4, 0_u8..8, 0_u64..100), 0..80),
    ) {
        use behavior::{TimerGeneration, TimerId, PeerStopped, TimerElapsed, stop_on_abnormal_death};

        let due = Instant::now() + Duration::from_secs(1);
        let peer = MailAddr(44);
        let behavior = Supervise::new(
            behavior::Deadline::new(
                behavior::Watch::new(
                    behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
                    peer,
                    stop_on_abnormal_death,
                ),
                behavior::TimerId(0),
                Some(due),
                |_| Step::Continue,
            ),
            behavior::ChildTopology::new(
                (0..2).map(|index| u64::try_from(index).unwrap()),
                |index| Some(child(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                u32::MAX,
                Duration::MAX, behavior::RestartTiming::Immediate
            ),
                    behavior::Proxy::new,
        ).unwrap();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        let mut model_echo: Vec<u64> = Vec::new();
        let mut impl_echo: Vec<u64> = Vec::new();
        let mut workers = [0_u64, 1];
        let mut next_worker = 2_u64;

        for (tag, arg, at) in events {
            let actions = match tag {
                0 => {
                    // User lane: routed by the stash filter.
                    let actions = runtime
                        .block_on(async { behavior.transition(SupervisionEvent::Behavior(EventLayer::Inner(
                            EventLayer::Inner(UserEvent::user(MailAddr(9), u64::from(arg))),
                        ))) })
                        .unwrap();
                    if u64::from(arg) % 3 != 2 {
                        model_echo.push(u64::from(arg));
                    }
                    actions
                }
                1 => {
                    // Peer lane: watched peer's abnormal death stops the fold.
                    runtime
                        .block_on(async { behavior.transition(SupervisionEvent::Behavior(EventLayer::Inner(
                            EventLayer::Owned(PeerStopped {
                                peer,
                                outcome: Err(Crash::Failed),
                            }),
                        ))) })
                        .unwrap()
                }
                2 => {
                    // Time lane: matching Reached fires (Continue) once, then
                    // duplicates are inert.
                    runtime
                        .block_on(async { behavior.transition(SupervisionEvent::Behavior(EventLayer::Owned(
                            TimerElapsed { id: TimerId(0), generation: TimerGeneration(0) },
                        ))) })
                        .unwrap()
                }
                _ => {
                    // Child lane: replacement send to the dead slot.
                    runtime
                        .block_on(async { behavior.transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                            proxy: u64::from(arg % 2),
                            worker: workers[usize::from(arg % 2)],
            outcome: Err(Crash::Failed),
                            at: base + Duration::from_nanos(at),
                        })) })
                        .unwrap()
                }
            };

            // Echo lane (user deliveries): exactly the filter model's output.
            let echo_step: Vec<u64> = actions
                .sends.inner
                .inner
                .inner
                .iter()
                .map(|d| d.message)
                .collect();
            impl_echo.extend(echo_step.iter().copied());
            if tag == 0 {
                prop_assert_eq!(echo_step.len(), usize::from(u64::from(arg) % 3 != 2));
            } else {
                prop_assert!(echo_step.is_empty(), "non-user event leaked into the echo lane");
            }

            // Cross-lane: user/time/peer steps never emit supervision sends;
            // child steps emit exactly one replacement and no echoes.
            if tag == 3 {
                prop_assert_eq!(actions.sends.owned.replacement_inputs.len(), 1);
                prop_assert_eq!(
                    actions.sends.owned.replacement_inputs[0].nonce,
                    u64::from(arg % 2)
                );
                prop_assert!(echo_step.is_empty());
                let proxy = u64::from(arg % 2);
                let index = usize::from(arg % 2);
                let previous = workers[index];
                let joined = runtime.block_on(async { behavior.transition(
                    SupervisionEvent::WorkerCreationResolved(WorkerCreationResolved::new(
                        proxy,
                        next_worker,
                        CreationKind::ReplacementIncarnation { replaces: previous },
                        Ok(()),
                    )),
                ) }).unwrap();
                prop_assert!(joined.sends.owned.child_observations.is_empty());
                prop_assert!(joined.sends.owned.creation_observations.is_empty());
                prop_assert!(joined.sends.owned.schedules.is_empty());
                prop_assert!(joined.sends.owned.replacement_inputs.is_empty());
                prop_assert!(joined.sends.owned.failure_reports.is_empty());
                prop_assert!(joined.sends.owned.shutdowns.is_empty());
                prop_assert!(joined.sends.inner.owned.is_empty());
                prop_assert!(joined.sends.inner.inner.owned.is_empty());
                prop_assert!(joined.sends.inner.inner.inner.is_empty());
                prop_assert!(joined.creates.is_empty());
                prop_assert!(matches!(joined.become_, Step::Continue));
                workers[index] = next_worker;
                next_worker += 1;
            } else {
                prop_assert!(actions.sends.owned.replacement_inputs.is_empty());
            }

            // Verdict: only a watched-peer death stops the fold.
            if tag == 1 {
                prop_assert!(matches!(
                    actions.become_,
                    Step::Stop(behavior::Stopped)
                ));
            } else {
                prop_assert_eq!(actions.become_, Step::Continue);
            }
        }
        prop_assert_eq!(impl_echo, model_echo);
    }
}
