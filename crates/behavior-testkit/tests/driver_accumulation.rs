//! Driver-level lossless accumulation: `drive` folds every transition's
//! effect into the trace via `SendAlgebra::append`. For composed behaviors
//! the sends use named products; this is where "send/create order and wrapper
//! preservation are lossless under composition" (contract #3) is enforced at
//! the trace level. Also: the `SendAlgebra` monoid law itself, and the
//! empty-fleet birth→death restart path.

use std::time::Duration;

use behavior::{
    Acted, Actions, Compose, Crash, Create, Delivery, MailAddr, Never, Recipient, RestartPolicy,
    SendAlgebra, Step, Strategy, SupervisionEvent, User, UserEvent, WorkerStopped,
};
use behavior_testkit::{Mailbox, drive};
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

fn child(_index: usize) -> Child {
    Echo
}

/// Parent that echoes every user message on its own send lane and can birth
/// children.
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

type TestSupervisor = behavior::Supervisor<EchoingParent, Child>;

/// A driven supervised trace: user echoes accumulate in the inner lane,
/// replacement sends in the supervisor's own lane, observe-child sends stay
/// exactly at init — every product lane keeps its own accumulation order.
#[tokio::test]
async fn driver_accumulates_supervising_send_products_losslessly() {
    let at = Instant::now();
    let supervisor: TestSupervisor = behavior::Supervisor::new(
        EchoingParent { seen: Vec::new() },
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            2,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
        ),
    )
    .unwrap();
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Behavior(User::user(MailAddr(9), 3)),
        SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at,
        }),
        SupervisionEvent::Behavior(User::user(MailAddr(9), 5)),
        SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            worker: 1,
            outcome: Err(Crash::Failed),
            at,
        }),
    ]);
    let trace = drive(supervisor, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 5);
    assert_eq!(trace.pending, 0);
    assert!(!trace.stopped);

    // Inner lane: user echoes, in order, exactly the delivered messages.
    let echoes: Vec<u64> = trace.sends.behavior.iter().map(|d| d.message).collect();
    assert_eq!(echoes, [3, 5]);
    // Supervisor's own replacement lane: one per death, in order.
    let replacements: Vec<MailAddr> = trace
        .sends
        .replacement_commands
        .iter()
        .map(|d| d.to.resolve(MailAddr(17)))
        .collect();
    assert_eq!(
        replacements,
        [
            behavior::Address::birth(MailAddr(17), 0),
            behavior::Address::birth(MailAddr(17), 1)
        ]
    );
    // Observe-child sends: emitted once at init, never again.
    assert_eq!(trace.sends.child_observations.len(), 2);
    // Creates: exactly the two init proxies; the driver never re-creates.
    assert_eq!(trace.creates.len(), 2);
}

// The `SendAlgebra` monoid law that the driver's accumulation depends on:
// `empty` is a two-sided identity and `append` is associative, at both the
// `Vec` level. Named wrapper products have composition-specific tests below.
proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })]

    #[test]
    fn send_algebra_is_a_monoid(
        a in vec(any::<u8>(), 0..32),
        b in vec(any::<u8>(), 0..32),
        c in vec(any::<u8>(), 0..32),
    ) {
        // Vec: empty is a two-sided identity.
        let mut left_id = Vec::empty();
        SendAlgebra::append(&mut left_id, a.clone());
        prop_assert_eq!(&left_id, &a);
        let mut right_id = a.clone();
        SendAlgebra::append(&mut right_id, Vec::empty());
        prop_assert_eq!(&right_id, &a);

        // Vec: append is associative.
        let mut left_assoc = a.clone();
        SendAlgebra::append(&mut left_assoc, b.clone());
        SendAlgebra::append(&mut left_assoc, c.clone());
        let mut mid = b.clone();
        SendAlgebra::append(&mut mid, c.clone());
        let mut right_assoc = a.clone();
        SendAlgebra::append(&mut right_assoc, mid);
        prop_assert_eq!(&left_assoc, &right_assoc);

    }
}

/// Empty configured fleet: a dynamic birth (sequence 0) followed by its
/// death restarts exactly that child under `OneForOne`.
#[tokio::test]
async fn empty_fleet_dynamic_birth_then_death_restarts() {
    let at = Instant::now();
    let supervisor = behavior::Supervisor::new(
        BirthingParent { born: false },
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            0,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(
            Strategy::OneForOne,
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
    assert_eq!(supervisor.child_count(), 1);
    assert!(supervisor.is_alive(9).unwrap());

    let actions = supervisor
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 9,
            worker: 9,
            outcome: Err(Crash::Failed),
            at,
        }))
        .unwrap();
    assert_eq!(actions.sends.replacement_commands.len(), 1);
    assert_eq!(
        actions.sends.replacement_commands[0]
            .to
            .resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 9)
    );
    assert!(supervisor.is_alive(9).unwrap());
}

/// The full four-layer stack driven through the mailbox: user echoes
/// accumulate in the innermost lane, the time lane fires once, a watched
/// peer's death stops the fold with `LinkDied` and leaves the remaining
/// mailbox unconsumed.
#[tokio::test]
async fn driver_full_stack_mixed_lanes_stop_on_peer_death() {
    use behavior::{
        Compose, DeadlineEvent, PeerStopped, StashRoute, TimerElapsed, TimerGeneration, TimerId,
        WatchEvent, stop_on_abnormal_death,
    };

    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let behavior = (EchoingParent { seen: Vec::new() })
        .stash(|m| {
            if m % 3 == 2 {
                StashRoute::Stash
            } else {
                StashRoute::Deliver
            }
        })
        .watch(peer, stop_on_abnormal_death)
        .deadline(behavior::TimerId(0), Some(due), |_| Ok(Step::Continue))
        .children(
            |index| u64::try_from(index).unwrap(),
            2,
            |index| Some(child(index)),
        )
        .unwrap()
        .with_strategy(Strategy::OneForOne)
        .with_policy(RestartPolicy::Permanent)
        .with_budget(u32::MAX, Duration::MAX);
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Behavior(DeadlineEvent::Behavior(WatchEvent::Behavior(User::user(
            MailAddr(9),
            1,
        )))),
        SupervisionEvent::Behavior(DeadlineEvent::Behavior(WatchEvent::Behavior(User::user(
            MailAddr(9),
            5,
        )))),
        SupervisionEvent::Behavior(DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })),
        SupervisionEvent::Behavior(DeadlineEvent::Behavior(WatchEvent::PeerStopped(
            PeerStopped {
                peer,
                outcome: Err(Crash::Failed),
            },
        ))),
        SupervisionEvent::Behavior(DeadlineEvent::Behavior(WatchEvent::Behavior(User::user(
            MailAddr(9),
            7,
        )))),
    ]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    // Stopped at the peer death: init + 4 processed events, tail left.
    assert_eq!(trace.transitions, 5);
    assert_eq!(trace.pending, 1);
    assert!(trace.stopped);

    // Echo lane accumulated exactly the Deliver-routed message (1); the
    // stashed message (5 % 3 == 2 -> Stash) and the post-stop message (7)
    // never reached the parent.
    let echoes: Vec<u64> = trace
        .sends
        .behavior
        .behavior
        .behavior
        .iter()
        .map(|d| d.message)
        .collect();
    assert_eq!(echoes, [1]);
    // Schedule send emitted once at init.
    assert_eq!(trace.sends.behavior.schedules.len(), 1);
    // Observe-peer emitted once at init.
    assert_eq!(trace.sends.behavior.behavior.observations.len(), 1);
    // Observe-child sends emitted once at init.
    assert_eq!(trace.sends.child_observations.len(), 2);
}

/// A macro-defined behavior folds through the same driver boundary as a
/// hand-written `Behavior`: same effect algebra and accumulation.
#[tokio::test]
#[allow(
    clippy::type_complexity,
    reason = "the fixture exposes the complete typed effect surface"
)]
async fn macro_defined_behavior_drives_like_a_base() {
    struct FnRecorder {
        seen: Vec<u64>,
    }

    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>, births = behavior::NoBirths, error = Never)]
    impl FnRecorder {
        fn receive(
            &mut self,
            _from: MailAddr,
            message: u64,
        ) -> Acted<
            MailAddr,
            Never,
            Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
            behavior::NoBirths,
            Never,
        > {
            self.seen.push(message);
            Ok(Actions {
                sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
                creates: Vec::new(),
                become_: if message == 9 {
                    Step::Stop(behavior::Stopped)
                } else {
                    Step::Continue
                },
            })
        }
    }

    let behavior = FnRecorder { seen: Vec::new() };
    let mut mailbox = Mailbox::new([
        User::user(MailAddr(1), 3),
        User::user(MailAddr(2), 9),
        User::user(MailAddr(3), 5),
    ]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 1);
    assert!(trace.stopped);
    let echoes: Vec<u64> = trace.sends.iter().map(|d| d.message).collect();
    assert_eq!(echoes, [3, 9]);
    assert_eq!(trace.behavior.seen, [3, 9]);
}

/// A stash release whose trigger stops the inner fold: the driver stops,
/// the stash buffer keeps the held messages, and nothing is lost.
#[tokio::test]
async fn driver_stash_stop_preserves_held_and_stops() {
    use behavior::StashRoute;

    struct StopOnZero {
        seen: Vec<(MailAddr, u64)>,
    }
    #[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>, births = behavior::NoBirths, error = Never)]
    impl StopOnZero {
        fn receive(
            &mut self,
            from: MailAddr,
            message: u64,
        ) -> Acted<
            MailAddr,
            Never,
            Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
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

    let behavior = (StopOnZero { seen: Vec::new() }).stash(|m: &u64| {
        if *m == 0 {
            StashRoute::Release
        } else {
            StashRoute::Stash
        }
    });
    let mut mailbox = Mailbox::new([User::user(MailAddr(1), 5), User::user(MailAddr(9), 0)]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 0);
    assert!(trace.stopped);
    assert_eq!(trace.behavior.held(), 1); // the stashed message survives the stop
    assert_eq!(trace.behavior.base().seen, [(MailAddr(9), 0)]);
}
use behavior_testkit::InitializeTest;
