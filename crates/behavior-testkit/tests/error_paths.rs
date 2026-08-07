//! Controlled-error attacks: every composition's error path was previously
//! unprobed. Key finding: `Fsm::drain` propagates a mid-drain error with `?`
//! and DROPS the unprocessed batch from the held queue, while the `Stop`
//! path explicitly preserves it — an asymmetry between controlled error and
//! controlled stop. Also pinned: supervision inner-error propagation, the
//! one-shot At timer consumed by a failing reaction, watch reaction errors,
//! and the `restart_*` helper constants.

use std::time::Duration;

use behavior::{
    Acted, Actions, AtEvent, AtId, Base, Behavior, Crash, Delivery, Fsm, MailAddr, Move, Never,
    PeerStopped, RestartPolicy, Spec, State, Step, Strategy, Supervising, SupervisionEvent,
    TimeReached, User, UserEvent, WatchEvent, restart_all, restart_one, restart_rest,
};
use behavior_testkit::{Mailbox, drive};
use tokio::time::Instant;

/// A controlled failure type: unit-like, `Send`, no display machinery.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Boom;

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

/// A parent whose handle fails on the first message.
struct FailingParent {
    fail: bool,
}

impl State<Never, behavior::Births<Child>, Boom> for FailingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Boom> {
        if self.fail {
            self.fail = false;
            return Err(Boom);
        }
        Ok(Actions::cont())
    }
}

/// A controlled error mid-drain drops the unprocessed held batch, whereas a
/// `Move::Stop` mid-drain preserves it (`fsm_stop_mid_drain_preserves_remaining_batch`).
#[tokio::test]
async fn fsm_error_mid_drain_drops_the_unprocessed_batch() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        P0,
        P1,
    }
    let mut machine = Fsm::new(
        Vec::new(),
        Phase::P0,
        |phase, _seen: &mut Vec<u64>, id: &u64| match (phase, id % 4) {
            (Phase::P0, 0) => Ok(Move::Goto(Phase::P1)),
            (Phase::P0, _) => Ok(Move::Defer),
            (Phase::P1, 1) => Err(Boom),
            (Phase::P1, _) => Ok(Move::Stay),
        },
    );
    // Defer ids 1 (would fail in P1) and 2; id 0 opens P1 and drains.
    machine.step(User::user(MailAddr(0), 1)).await.unwrap();
    machine.step(User::user(MailAddr(0), 2)).await.unwrap();
    assert_eq!(machine.held(), 2);

    let result = machine.step(User::user(MailAddr(0), 0)).await;
    assert!(matches!(result, Err(Boom)));
    // The unprocessed id 2 was drained into the local batch and dropped on
    // the error: held is empty, not [2] as the Stop path would leave it.
    assert_eq!(machine.held(), 0);
}

/// A direct-step error consumes only the errored message: held stays intact
/// and the fold is still usable.
#[tokio::test]
async fn fsm_direct_step_error_keeps_held_intact() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        P0,
        P1,
    }
    let mut machine = Fsm::new(
        Vec::new(),
        Phase::P0,
        |phase, _seen: &mut Vec<u64>, id: &u64| match (phase, id % 4) {
            (Phase::P0, 0) => Ok(Move::Goto(Phase::P1)),
            (Phase::P0, _) => Ok(Move::Defer),
            (Phase::P1, 1) => Err(Boom),
            (Phase::P1, _) => Ok(Move::Stay),
        },
    );
    machine.step(User::user(MailAddr(0), 2)).await.unwrap(); // held
    machine.step(User::user(MailAddr(0), 0)).await.unwrap(); // P1, drain: id 2 stays (P1,2 -> Stay, not held)
    assert_eq!(machine.held(), 0);

    // In P1 now: a direct id-1 message errors, held untouched.
    let result = machine.step(User::user(MailAddr(0), 1)).await;
    assert!(matches!(result, Err(Boom)));
    assert_eq!(machine.held(), 0);
    // The fold is still live for a non-failing message.
    machine.step(User::user(MailAddr(0), 3)).await.unwrap();
}

/// Supervision propagates the inner controlled error without touching the
/// slot table: no slot is born, killed, or replaced by a failed step.
#[tokio::test]
async fn supervision_propagates_inner_errors_without_touching_slots() {
    let mut supervisor = Supervising::new(
        Base::new(FailingParent { fail: true }),
        |index| u64::try_from(index).unwrap(),
        2,
        child,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    supervisor.init().await.unwrap();
    let before = supervisor.child_count();

    let result = supervisor
        .step(SupervisionEvent::Inner(UserEvent::user(MailAddr(0), 7)))
        .await;
    assert!(matches!(result, Err(Boom)));
    assert_eq!(supervisor.child_count(), before);
    for nonce in 0..2 {
        assert!(supervisor.is_alive(u64::try_from(nonce).unwrap()));
    }
    // The fold recovers on the next message.
    supervisor
        .step(SupervisionEvent::Inner(UserEvent::user(MailAddr(0), 7)))
        .await
        .unwrap();
}

/// A failing `At` reaction consumes the one-shot timer: the error propagates
/// and the same Reached event can never fire again.
#[tokio::test]
async fn at_reaction_error_consumes_the_timer() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior = Spec::new(FailingParent { fail: true }).at(Some(due), |_| Err(Boom));
    behavior.init().await.unwrap();

    let first = behavior
        .step(AtEvent::Reached(TimeReached {
            id: AtId(0),
            at: due,
        }))
        .await;
    assert!(matches!(first, Err(Boom)));

    // The duplicate delivery cannot re-fire the consumed timer.
    let second = behavior
        .step(AtEvent::Reached(TimeReached {
            id: AtId(0),
            at: due,
        }))
        .await
        .unwrap();
    assert_eq!(second.become_, Step::Continue);
}

/// A failing watch reaction propagates the error; the fold keeps working.
#[tokio::test]
async fn watch_reaction_error_propagates() {
    let peer = MailAddr(44);
    let mut behavior = Spec::new(FailingParent { fail: true }).watch(peer, |_b, _p, _o| Err(Boom));
    behavior.init().await.unwrap();

    let death = WatchEvent::PeerStopped(PeerStopped {
        peer,
        outcome: Err(Crash::Failed),
    });
    let result = behavior.step(death).await;
    assert!(matches!(result, Err(Boom)));
}

/// The `restart_*` helpers expose exactly the documented strategies.
#[test]
fn restart_helpers_expose_the_documented_strategies() {
    assert_eq!(restart_one(), Strategy::OneForOne);
    assert_eq!(restart_all(), Strategy::OneForAll);
    assert_eq!(restart_rest(), Strategy::RestForOne);
}

/// A `Stashing` Deliver-arm error propagates and leaves the held buffer
/// untouched (the errored message was the fresh one, not a replayed one).
#[tokio::test]
async fn stash_deliver_arm_error_keeps_held_intact() {
    use behavior::{Spec, StashRoute};

    let mut behavior = Spec::new(FailingParent { fail: true }).stash(|message| match *message {
        0 => StashRoute::Release,
        1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    behavior
        .step(UserEvent::user(MailAddr(1), 1))
        .await
        .unwrap();
    assert_eq!(behavior.behavior().held(), 1);

    // Message 2 routes Deliver; the parent fails on the first handled
    // message. The held message survives.
    let result = behavior.step(UserEvent::user(MailAddr(2), 2)).await;
    assert!(matches!(result, Err(Boom)));
    assert_eq!(behavior.behavior().held(), 1);
}

/// The driver propagates the first controlled failure and leaves the
/// unconsumed mailbox tail intact.
#[tokio::test]
async fn driver_propagates_errors_and_preserves_the_tail() {
    let mut supervisor = Supervising::new(
        Base::new(FailingParent { fail: true }),
        |index| u64::try_from(index).unwrap(),
        1,
        child,
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    );
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Inner(UserEvent::user(MailAddr(9), 3)), // fails (first)
        SupervisionEvent::Inner(UserEvent::user(MailAddr(9), 5)), // never reached
    ]);
    let result = drive(&mut supervisor, &mut mailbox).await;
    assert!(matches!(result, Err(Boom)));
    assert_eq!(mailbox.pending(), 1);
}

/// Messages processed before the mid-drain error keep their effects: the
/// fold's state retains partial drain progress, the erroring message and
/// the unprocessed batch are gone from the buffer.
#[tokio::test]
async fn fsm_error_mid_drain_keeps_prior_drain_effects() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        P0,
        P1,
    }
    let mut machine = Fsm::new(
        Vec::new(),
        Phase::P0,
        |phase, seen: &mut Vec<u64>, id: &u64| match (phase, id % 4) {
            (Phase::P0, 0) => Ok(Move::Goto(Phase::P1)),
            (Phase::P0, _) => Ok(Move::Defer),
            (Phase::P1, 1) => Err(Boom),
            (Phase::P1, _) => {
                seen.push(*id);
                Ok(Move::Stay)
            }
        },
    );
    // Held order [2, 3, 1]: the drain records 2 and 3, then id 1 errors.
    machine.step(User::user(MailAddr(0), 2)).await.unwrap();
    machine.step(User::user(MailAddr(0), 3)).await.unwrap();
    machine.step(User::user(MailAddr(0), 1)).await.unwrap();
    let result = machine.step(User::user(MailAddr(0), 0)).await;
    assert!(matches!(result, Err(Boom)));
    assert_eq!(machine.state().as_slice(), &[2, 3]);
    assert_eq!(machine.held(), 0);
}
