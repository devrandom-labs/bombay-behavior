//! Driver-level lossless accumulation: `drive` folds every transition's
//! effect into the trace via `SendAlgebra::append`. For composed behaviors
//! the sends are `SendProduct`s; this is where "send/create order and
//! wrapper preservation are lossless under composition" (contract #3) is
//! actually enforced at the trace level. Also: the `SendAlgebra` monoid law
//! itself, and the empty-fleet birth→death restart path.

use std::time::Duration;

use behavior::{
    Acted, Actions, Base, Behavior, Crash, Create, Delivery, Exit, MailAddr, Never, Recipient,
    RestartPolicy, Route, SendAlgebra, SendProduct, State, Step, Strategy, Supervising,
    SupervisionEvent, User, UserEvent, WorkerStopped,
};
use behavior_testkit::{Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::*;
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

fn child(_index: usize) -> Child {
    Base::new(Echo)
}

/// Parent that echoes every user message on its own send lane and can birth
/// children.
struct EchoingParent {
    seen: Vec<u64>,
}

impl State<u64, behavior::Births<Child>, Never> for EchoingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, behavior::Births<Child>, Never> {
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

impl State<Never, behavior::Births<Child>, Never> for BirthingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Never>
    {
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

type Supervisor = Supervising<Base<EchoingParent, u64, behavior::Births<Child>, Never>, Child>;

/// A driven supervised trace: user echoes accumulate in the inner lane,
/// replacement sends in the supervisor's own lane, observe-child sends stay
/// exactly at init — every product lane keeps its own accumulation order.
#[tokio::test]
async fn driver_accumulates_supervising_send_products_losslessly() {
    let at = Instant::now();
    let mut supervisor: Supervisor = Supervising::new(
        Base::new(EchoingParent { seen: Vec::new() }),
        |index| u64::try_from(index).unwrap(),
        2,
        child,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Inner(User::user(MailAddr(9), 3)),
        SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at,
        }),
        SupervisionEvent::Inner(User::user(MailAddr(9), 5)),
        SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 1,
            worker: 1,
            outcome: Err(Crash::Failed),
            at,
        }),
    ]);
    let trace = drive(&mut supervisor, &mut mailbox).await.unwrap();

    assert_eq!(trace.transitions, 5);
    assert_eq!(trace.pending, 0);
    assert_eq!(trace.exit, None);

    // Inner lane: user echoes, in order, exactly the delivered messages.
    let echoes: Vec<u64> = trace.sends.behavior.iter().map(|d| d.message).collect();
    assert_eq!(echoes, [3, 5]);
    // Supervisor's own replacement lane: one per death, in order.
    let replacements: Vec<Route<MailAddr>> = trace
        .sends
        .replacement_commands
        .iter()
        .map(|d| d.to.route())
        .collect();
    assert_eq!(replacements, [Route::Child(0), Route::Child(1)]);
    // Observe-child sends: emitted once at init, never again.
    assert_eq!(trace.sends.child_observations.len(), 2);
    // Creates: exactly the two init proxies; the driver never re-creates.
    assert_eq!(trace.creates.len(), 2);
}

// The `SendAlgebra` monoid law that the driver's accumulation depends on:
// `empty` is a two-sided identity and `append` is associative, at both the
// `Vec` and `SendProduct` levels.
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

        // SendProduct: identity and associativity hold per lane.
        let prod = |l: Vec<u8>, r: Vec<u8>| SendProduct { inner: l, own: r };
        let p = prod(a.clone(), b.clone());
        let mut left_id = SendProduct::empty();
        SendAlgebra::append(&mut left_id, p.clone());
        prop_assert_eq!(&left_id, &p);
        let mut right_id = p.clone();
        SendAlgebra::append(&mut right_id, SendProduct::empty());
        prop_assert_eq!(&right_id, &p);

        let q = prod(c.clone(), a.clone());
        let r = prod(vec![1], vec![2]);
        let mut left_assoc = p.clone();
        SendAlgebra::append(&mut left_assoc, q.clone());
        SendAlgebra::append(&mut left_assoc, r.clone());
        let mut mid = q;
        SendAlgebra::append(&mut mid, r);
        let mut right_assoc = p;
        SendAlgebra::append(&mut right_assoc, mid);
        prop_assert_eq!(&left_assoc, &right_assoc);
    }
}

/// Empty configured fleet: a dynamic birth (sequence 0) followed by its
/// death restarts exactly that child under `OneForOne`.
#[tokio::test]
async fn empty_fleet_dynamic_birth_then_death_restarts() {
    let at = Instant::now();
    let mut supervisor = Supervising::new(
        Base::new(BirthingParent { born: false }),
        |index| u64::try_from(index).unwrap(),
        0,
        child,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    supervisor.init().await.unwrap();
    supervisor
        .step(UserEvent::user(MailAddr(0), 9))
        .await
        .unwrap();
    assert_eq!(supervisor.child_count(), 1);
    assert!(supervisor.is_alive(9));

    let actions = supervisor
        .step(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 9,
            worker: 9,
            outcome: Err(Crash::Failed),
            at,
        }))
        .await
        .unwrap();
    assert_eq!(actions.sends.replacement_commands.len(), 1);
    assert_eq!(
        actions.sends.replacement_commands[0].to.route(),
        Route::Child(9)
    );
    assert!(supervisor.is_alive(9));
}

/// The full four-layer stack driven through the mailbox: user echoes
/// accumulate in the innermost lane, the time lane fires once, a watched
/// peer's death stops the fold with `LinkDied` and leaves the remaining
/// mailbox unconsumed.
#[tokio::test]
async fn driver_full_stack_mixed_lanes_stop_on_peer_death() {
    use behavior::{
        AtEvent, PeerStopped, Spec, StashRoute, TimerElapsed, TimerGeneration, TimerId, WatchEvent,
        stop_on_abnormal_death,
    };

    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let mut behavior = Spec::new(EchoingParent { seen: Vec::new() })
        .stash(|m| {
            if m % 3 == 2 {
                StashRoute::Stash
            } else {
                StashRoute::Deliver
            }
        })
        .watch(peer, stop_on_abnormal_death)
        .at(Some(due), |_| Ok(Step::Continue))
        .children((2, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Permanent)
        .within(u32::MAX, Duration::MAX);
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Inner(AtEvent::Inner(WatchEvent::Inner(User::user(
            MailAddr(9),
            1,
        )))),
        SupervisionEvent::Inner(AtEvent::Inner(WatchEvent::Inner(User::user(
            MailAddr(9),
            5,
        )))),
        SupervisionEvent::Inner(AtEvent::Reached(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        })),
        SupervisionEvent::Inner(AtEvent::Inner(WatchEvent::PeerStopped(PeerStopped {
            peer,
            outcome: Err(Crash::Failed),
        }))),
        SupervisionEvent::Inner(AtEvent::Inner(WatchEvent::Inner(User::user(
            MailAddr(9),
            7,
        )))),
    ]);
    let trace = drive(&mut behavior, &mut mailbox).await.unwrap();

    // Stopped at the peer death: init + 4 processed events, tail left.
    assert_eq!(trace.transitions, 5);
    assert_eq!(trace.pending, 1);
    assert!(matches!(trace.exit, Some(Exit::LinkDied(p)) if p == peer));

    // Echo lane accumulated exactly the Deliver-routed message (1); the
    // stashed message (5 % 3 == 2 -> Stash) and the post-stop message (7)
    // never reached the parent.
    let echoes: Vec<u64> = trace
        .sends
        .behavior
        .inner
        .inner
        .iter()
        .map(|d| d.message)
        .collect();
    assert_eq!(echoes, [1]);
    // Schedule send emitted once at init.
    assert_eq!(trace.sends.behavior.own.len(), 1);
    // Observe-peer emitted once at init.
    assert_eq!(trace.sends.behavior.inner.own.len(), 1);
    // Observe-child sends emitted once at init.
    assert_eq!(trace.sends.child_observations.len(), 2);
}

/// `Base::from_fn` (the functional state adapter) folds exactly like a
/// hand-written `State`: same effect algebra, same driver accumulation.
#[tokio::test]
#[allow(
    clippy::type_complexity,
    reason = "the FnState adapter's full generic surface"
)]
async fn fn_state_adapter_drives_like_a_base() {
    use behavior::Base;

    #[allow(
        clippy::unnecessary_wraps,
        reason = "the Transition signature requires Acted"
    )]
    fn handle(
        seen: &mut Vec<u64>,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, behavior::NoBirths, Never> {
        seen.push(message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
            creates: Vec::new(),
            become_: if message == 9 {
                Step::Stop(Exit::Normal)
            } else {
                Step::Continue
            },
        })
    }
    let mut behavior: Base<
        behavior::FnState<Vec<u64>, MailAddr, u64, u64, behavior::NoBirths, Never>,
        u64,
        behavior::NoBirths,
        Never,
    > = Base::from_fn(Vec::new(), handle);
    let mut mailbox = Mailbox::new([
        User::user(MailAddr(1), 3),
        User::user(MailAddr(2), 9),
        User::user(MailAddr(3), 5),
    ]);
    let trace = drive(&mut behavior, &mut mailbox).await.unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 1);
    assert_eq!(trace.exit, Some(Exit::Normal));
    let echoes: Vec<u64> = trace.sends.iter().map(|d| d.message).collect();
    assert_eq!(echoes, [3, 9]);
    assert_eq!(behavior.state().state, [3, 9]);
}

/// A stash release whose trigger stops the inner fold: the driver stops,
/// the stash buffer keeps the held messages, and nothing is lost.
#[tokio::test]
async fn driver_stash_stop_preserves_held_and_stops() {
    use behavior::{StashRoute, Stashing};

    struct StopOnZero {
        seen: Vec<(MailAddr, u64)>,
    }
    impl State<u64, behavior::NoBirths, Never> for StopOnZero {
        type Addr = MailAddr;
        type Msg = u64;

        fn handle(
            &mut self,
            from: MailAddr,
            message: u64,
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, behavior::NoBirths, Never>
        {
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

    let mut behavior = Stashing::new(Base::new(StopOnZero { seen: Vec::new() }), |m: &u64| {
        if *m == 0 {
            StashRoute::Release
        } else {
            StashRoute::Stash
        }
    });
    let mut mailbox = Mailbox::new([User::user(MailAddr(1), 5), User::user(MailAddr(9), 0)]);
    let trace = drive(&mut behavior, &mut mailbox).await.unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 0);
    assert_eq!(trace.exit, Some(Exit::Normal));
    assert_eq!(behavior.held(), 1); // the stashed message survives the stop
    assert_eq!(behavior.inner().state().seen, [(MailAddr(9), 0)]);
}
