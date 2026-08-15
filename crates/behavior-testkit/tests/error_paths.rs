//! Controlled-error attacks: every composition's error path was previously
//! unprobed. `Machine::drain` preserves the unprocessed deferred suffix on a
//! controlled error, just as it does on a controlled stop. Also pinned:
//! supervision behavior-error propagation, the
//! one-shot Deadline timer consumed by a failing reaction, watch reaction errors,
//! and the `restart_*` helper constants.

use std::time::Duration;

use behavior::{
    Acted, Actions, Compose, Crash, DeadlineEvent, Delivery, Machine, MailAddr, Move, Never,
    PeerStopped, RestartPolicy, Step, Strategy, SupervisionEvent, Supervisor, TimerElapsed,
    TimerGeneration, TimerId, User, UserEvent, WatchEvent, restart_all, restart_one, restart_rest,
};
use behavior_testkit::{Mailbox, drive};
use std::time::Instant;

/// A controlled failure type: unit-like, `Send`, no display machinery.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Boom;

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

/// A parent whose handle fails on the first message.
struct FailingParent {
    fail: bool,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Boom)]
impl FailingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Boom> {
        if self.fail {
            self.fail = false;
            return Err(Boom);
        }
        Ok(Actions::cont())
    }
}

/// A controlled error consumes the rejected message but preserves the
/// unprocessed held suffix, matching the controlled-stop prefix law.
#[tokio::test]
async fn fsm_error_mid_drain_preserves_the_unprocessed_batch() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        P0,
        P1,
    }
    let machine = Machine::new(
        Vec::new(),
        Phase::P0,
        |phase, _seen: &mut Vec<u64>, id: &u64| match (phase, id % 4) {
            (Phase::P0, 0) => Ok(Move::Goto(Phase::P1)),
            (Phase::P0, _) => Ok(Move::Defer),
            (Phase::P1, 1) => Err(Boom),
            (Phase::P1, _) => Ok(Move::Stay),
        },
    );
    let mut machine = machine.initialize().unwrap().behavior;
    // Defer ids 1 (would fail in P1) and 2; id 0 opens P1 and drains.
    machine.transition(User::user(MailAddr(0), 1)).unwrap();
    machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_eq!(machine.held(), 2);

    let result = machine.transition(User::user(MailAddr(0), 0));
    assert!(matches!(result, Err(Boom)));
    assert_eq!(machine.held(), 1);

    // The fold remains usable without disturbing that suffix.
    machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_eq!(machine.held(), 1);
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
    let machine = Machine::new(
        Vec::new(),
        Phase::P0,
        |phase, _seen: &mut Vec<u64>, id: &u64| match (phase, id % 4) {
            (Phase::P0, 0) => Ok(Move::Goto(Phase::P1)),
            (Phase::P0, _) => Ok(Move::Defer),
            (Phase::P1, 1) => Err(Boom),
            (Phase::P1, _) => Ok(Move::Stay),
        },
    );
    let mut machine = machine.initialize().unwrap().behavior;
    machine.transition(User::user(MailAddr(0), 2)).unwrap(); // held
    machine.transition(User::user(MailAddr(0), 0)).unwrap(); // P1, drain: id 2 stays (P1,2 -> Stay, not held)
    assert_eq!(machine.held(), 0);

    // In P1 now: a direct id-1 message errors, held untouched.
    let result = machine.transition(User::user(MailAddr(0), 1));
    assert!(matches!(result, Err(Boom)));
    assert_eq!(machine.held(), 0);
    // The fold is still live for a non-failing message.
    machine.transition(User::user(MailAddr(0), 3)).unwrap();
}

/// Supervision propagates the inner controlled error without touching the
/// slot table: no slot is born, killed, or replaced by a failed step.
#[tokio::test]
async fn supervision_propagates_inner_errors_without_touching_slots() {
    let supervisor = Supervisor::new(
        FailingParent { fail: true },
        |index| u64::try_from(index).unwrap(),
        2,
        |index| Some(child(index)),
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let before = supervisor.child_count();

    let result = supervisor.transition(SupervisionEvent::Behavior(UserEvent::user(MailAddr(0), 7)));
    assert!(matches!(
        result,
        Err(behavior::SupervisorError::Behavior(Boom))
    ));
    assert_eq!(supervisor.child_count(), before);
    for nonce in 0..2 {
        assert!(supervisor.is_alive(u64::try_from(nonce).unwrap()).unwrap());
    }
    // The fold recovers on the next message.
    supervisor
        .transition(SupervisionEvent::Behavior(UserEvent::user(MailAddr(0), 7)))
        .unwrap();
}

/// A failing `Deadline` reaction consumes the one-shot timer: the error propagates
/// and the same Reached event can never fire again.
#[tokio::test]
async fn at_reaction_error_consumes_the_timer() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior = Compose::new(FailingParent { fail: true }).deadline(
        behavior::TimerId(0),
        Some(due),
        |_| Err(Boom),
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let first = behavior.transition(DeadlineEvent::Elapsed(TimerElapsed {
        id: TimerId(0),
        generation: TimerGeneration(0),
    }));
    assert!(matches!(first, Err(Boom)));

    // The duplicate delivery cannot re-fire the consumed timer.
    let second = behavior
        .transition(DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }))
        .unwrap();
    assert_eq!(second.become_, Step::Continue);
}

/// A failing watch reaction propagates the error; the fold keeps working.
#[tokio::test]
async fn watch_reaction_error_propagates() {
    let peer = MailAddr(44);
    let behavior = Compose::new(FailingParent { fail: true }).watch(peer, |_b, _p, _o| Err(Boom));
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    let death = WatchEvent::PeerStopped(PeerStopped {
        peer,
        outcome: Err(Crash::Failed),
    });
    let result = behavior.transition(death);
    assert!(matches!(result, Err(Boom)));
}

/// The `restart_*` helpers expose exactly the documented strategies.
#[test]
fn restart_helpers_expose_the_documented_strategies() {
    assert_eq!(restart_one(), Strategy::OneForOne);
    assert_eq!(restart_all(), Strategy::OneForAll);
    assert_eq!(restart_rest(), Strategy::RestForOne);
}

/// A `Stash` Deliver-arm error propagates and leaves the held buffer
/// untouched (the errored message was the fresh one, not a replayed one).
#[tokio::test]
async fn stash_deliver_arm_error_keeps_held_intact() {
    use behavior::{Compose, StashRoute};

    let behavior = Compose::new(FailingParent { fail: true }).stash(|message| match *message {
        0 => StashRoute::Release,
        1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    behavior
        .transition(UserEvent::user(MailAddr(1), 1))
        .unwrap();
    assert_eq!(behavior.held(), 1);

    // Message 2 routes Deliver; the parent fails on the first handled
    // message. The held message survives.
    let result = behavior.transition(UserEvent::user(MailAddr(2), 2));
    assert!(matches!(result, Err(Boom)));
    assert_eq!(behavior.held(), 1);
}

/// The driver propagates the first controlled failure and leaves the
/// unconsumed mailbox tail intact.
#[tokio::test]
async fn driver_propagates_errors_and_preserves_the_tail() {
    let supervisor = Supervisor::new(
        FailingParent { fail: true },
        |index| u64::try_from(index).unwrap(),
        1,
        |index| Some(child(index)),
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        u32::MAX,
        Duration::MAX,
    )
    .unwrap();
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Behavior(UserEvent::user(MailAddr(9), 3)), // fails (first)
        SupervisionEvent::Behavior(UserEvent::user(MailAddr(9), 5)), // never reached
    ]);
    let result = drive(Compose::new(supervisor), &mut mailbox);
    assert!(matches!(
        result,
        Err(behavior::SupervisorError::Behavior(Boom))
    ));
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
    let machine = Machine::new(
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
    let mut machine = machine.initialize().unwrap().behavior;
    // Held order [2, 3, 1]: the drain records 2 and 3, then id 1 errors.
    machine.transition(User::user(MailAddr(0), 2)).unwrap();
    machine.transition(User::user(MailAddr(0), 3)).unwrap();
    machine.transition(User::user(MailAddr(0), 1)).unwrap();
    let result = machine.transition(User::user(MailAddr(0), 0));
    assert!(matches!(result, Err(Boom)));
    assert_eq!(machine.state().as_slice(), &[2, 3]);
    assert_eq!(machine.held(), 0);
}
use behavior_testkit::InitializeTest;
