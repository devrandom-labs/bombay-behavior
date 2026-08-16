//! Cross-lane isolation: in a supervised stack over a stash over a sending
//! parent, the user lane, the child-death lane, and the buffer never leak
//! into each other — user messages are routed by the stash (Deliver through,
//! Stash held, Release delivered), child deaths produce replacement sends
//! only, and neither produces the other's effects.

use std::time::Duration;

use behavior::{
    Acted, Actions, Activate, Crash, Delivery, MailAddr, Never, Recipient, RestartPolicy,
    StashRoute, Step, Strategy, SupervisionEvent, TimerElapsed, TimerGeneration, TimerId,
    UserEvent, WorkerStopped,
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

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>, births = behavior::Births<Child>, error = Never)]
impl EchoingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
        behavior::Births<Child>,
        Never,
    > {
        self.seen.push(message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
            creates: Vec::new(),
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

type Stack = behavior::Active<behavior::Supervisor<behavior::Stash<EchoingParent>, Child>>;

async fn user(behavior: &mut Stack, message: u64) -> Vec<u64> {
    let actions = behavior
        .transition(SupervisionEvent::Behavior(UserEvent::user(
            MailAddr(9),
            message,
        )))
        .unwrap();
    actions.sends.behavior.iter().map(|d| d.message).collect()
}

/// The user lane is filtered by the stash exactly as in the unfettered
/// stack (Deliver and Release triggers pass in order, Stash stays held), and
/// no user step ever emits a supervision send.
#[tokio::test]
async fn supervised_stash_routes_user_lane_without_cross_lane_effects() {
    let behavior = behavior::Supervisor::new(
        behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
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
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Stash-routed messages produce no echo; Deliver and Release triggers
    // pass through in order (the parent echoes what it processed).
    assert_eq!(user(&mut behavior, 2).await, vec![]); // 2 % 3 = 2: Stash
    assert_eq!(user(&mut behavior, 5).await, vec![]); // 5 % 3 = 2: Stash
    assert_eq!(user(&mut behavior, 1).await, vec![1]); // Deliver
    assert_eq!(user(&mut behavior, 0).await, vec![0]); // Release: trigger + drain re-stashes
    // A second release still cannot replay the stashed pair.
    assert_eq!(user(&mut behavior, 0).await, vec![0]);
}

/// A child death produces only a replacement send: the parent's echo lane
/// and the stash buffer stay untouched.
#[tokio::test]
async fn child_death_never_leaks_into_the_user_lane() {
    let behavior = behavior::Supervisor::new(
        behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
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
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    // Buffer one user message first, then kill a child.
    user(&mut behavior, 2).await;
    let actions = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(actions.sends.replacement_commands.len(), 1);
    assert_eq!(
        actions.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
    assert!(actions.sends.behavior.is_empty());
    assert!(actions.sends.child_observations.is_empty());
}

/// The time lane through a supervised stack: the schedule send survives in
/// its own product lane beside the observe-child sends, and a Reached event
/// fires the inner Deadline (stopping the whole supervised fold) without touching
/// the user or child lanes.
#[tokio::test]
async fn supervision_preserves_inner_at_routing() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = behavior::Supervisor::new(
        behavior::Deadline::new(
            EchoingParent { seen: Vec::new() },
            behavior::TimerId(0),
            Some(due),
            |_| Ok(Step::Stop(behavior::Stopped)),
        ),
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
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.sends.behavior.schedules[0].at, due);
    assert_eq!(initial.sends.child_observations.len(), 1);
    assert!(initial.sends.behavior.behavior.is_empty());
    assert_eq!(initial.creates.len(), 1);

    let fired = behavior
        .on(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })
        .unwrap();
    assert_eq!(fired.become_, Step::Stop(behavior::Stopped));
    assert!(fired.sends.replacement_commands.is_empty());
}

/// The full stack: supervision over at over watch over stash. All four
/// layers' init sends survive in their exact product-lane nesting, and all
/// four event lanes (user, peer, time, child) route to their own layer
/// without cross-lane leakage.
#[tokio::test]
async fn full_stack_all_four_layers_keep_their_own_lanes() {
    use behavior::{DeadlineEvent, PeerStopped, TimerElapsed, WatchEvent, stop_on_abnormal_death};

    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let behavior = behavior::Supervisor::new(
        behavior::Deadline::new(
            behavior::Watch::new(
                behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
                peer,
                stop_on_abnormal_death,
            ),
            behavior::TimerId(0),
            Some(due),
            |_| Ok(Step::Continue),
        ),
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
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
    let initialized = behavior.initialize().unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(initial.creates.len(), 2);
    assert_eq!(initial.sends.child_observations.len(), 2); // observe-child x2
    assert_eq!(initial.sends.replacement_commands.len(), 0); // proxy commands
    assert_eq!(initial.sends.behavior.schedules[0].at, due); // schedule
    assert_eq!(initial.sends.behavior.behavior.observations[0].peer, peer); // observe-peer
    assert!(initial.sends.behavior.behavior.behavior.is_empty()); // echo lane

    // User lane: Deliver routes through every layer to the parent echo.
    let actions = behavior
        .transition(SupervisionEvent::Behavior(DeadlineEvent::Behavior(
            WatchEvent::Behavior(UserEvent::user(MailAddr(9), 1)),
        )))
        .unwrap();
    assert_eq!(actions.sends.behavior.behavior.behavior[0].message, 1);
    assert!(actions.sends.replacement_commands.is_empty());

    // Time lane: fires the inner Deadline.
    let fired = behavior
        .transition(SupervisionEvent::Behavior(DeadlineEvent::Elapsed(
            TimerElapsed {
                id: TimerId(0),
                generation: TimerGeneration(0),
            },
        )))
        .unwrap();
    assert_eq!(fired.become_, Step::Continue);

    // Peer lane: matching peer death stops the fold.
    let died = behavior
        .transition(SupervisionEvent::Behavior(DeadlineEvent::Behavior(
            WatchEvent::PeerStopped(PeerStopped {
                peer,
                outcome: Err(Crash::Failed),
            }),
        )))
        .unwrap();
    assert!(matches!(died.become_, Step::Stop(behavior::Stopped)));

    // Child lane on a fresh stack: replacement send only.
    let fresh = behavior::Supervisor::new(
        behavior::Deadline::new(
            behavior::Watch::new(
                behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
                peer,
                stop_on_abnormal_death,
            ),
            behavior::TimerId(0),
            Some(due),
            |_| Ok(Step::Continue),
        ),
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
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
    assert_eq!(replacement.sends.replacement_commands.len(), 1);
    assert_eq!(
        replacement.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );
    assert!(replacement.sends.behavior.behavior.behavior.is_empty());
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
        use behavior::{DeadlineEvent, TimerGeneration, TimerId, PeerStopped, TimerElapsed, WatchEvent, stop_on_abnormal_death};

        let due = Instant::now() + Duration::from_secs(1);
        let peer = MailAddr(44);
        let behavior = behavior::Supervisor::new(
            behavior::Deadline::new(
                behavior::Watch::new(
                    behavior::Stash::new(EchoingParent { seen: Vec::new() }, route),
                    peer,
                    stop_on_abnormal_death,
                ),
                behavior::TimerId(0),
                Some(due),
                |_| Ok(Step::Continue),
            ),
            behavior::ChildTopology::new(
                (0..2).map(|index| u64::try_from(index).unwrap()),
                |index| Some(child(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                u32::MAX,
                Duration::MAX,
            ),
        ).unwrap();
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        let mut model_echo: Vec<u64> = Vec::new();
        let mut impl_echo: Vec<u64> = Vec::new();

        for (tag, arg, at) in events {
            let actions = match tag {
                0 => {
                    // User lane: routed by the stash filter.
                    let actions = runtime
                        .block_on(async { behavior.transition(SupervisionEvent::Behavior(DeadlineEvent::Behavior(
                            WatchEvent::Behavior(UserEvent::user(MailAddr(9), u64::from(arg))),
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
                        .block_on(async { behavior.transition(SupervisionEvent::Behavior(DeadlineEvent::Behavior(
                            WatchEvent::PeerStopped(PeerStopped {
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
                        .block_on(async { behavior.transition(SupervisionEvent::Behavior(DeadlineEvent::Elapsed(
                            TimerElapsed { id: TimerId(0), generation: TimerGeneration(0) },
                        ))) })
                        .unwrap()
                }
                _ => {
                    // Child lane: replacement send to the dead slot.
                    runtime
                        .block_on(async { behavior.transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                            proxy: u64::from(arg % 2),
                            worker: u64::from(arg % 2),
            outcome: Err(Crash::Failed),
                            at: base + Duration::from_nanos(at),
                        })) })
                        .unwrap()
                }
            };

            // Echo lane (user deliveries): exactly the filter model's output.
            let echo_step: Vec<u64> = actions
                .sends.behavior
                .behavior
                .behavior
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
                prop_assert_eq!(actions.sends.replacement_commands.len(), 1);
                prop_assert_eq!(
                    actions.sends.replacement_commands[0].to.resolve(MailAddr(17)),
                    behavior::Address::birth(MailAddr(17), u64::from(arg % 2))
                );
                prop_assert!(echo_step.is_empty());
            } else {
                prop_assert!(actions.sends.replacement_commands.is_empty());
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
