//! Controlled-error attacks: every composition's error path was previously
//! unprobed. `Machine::drain` preserves the unprocessed deferred suffix on a
//! controlled error, just as it does on a controlled stop. Also pinned:
//! supervision behavior-error propagation and the `restart_*` helper constants.

use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, BehaviorActed, BehaviorBase, Births, Delivery, Machine, MailAddr,
    Move, Never, NoBirths, RestartPolicy, Step, Strategy, Supervise, SupervisionEvent, User,
    UserEvent, restart_all, restart_one, restart_rest,
};
use behavior_testkit::{Mailbox, drive};

/// A controlled failure type: unit-like, `Send`, no display machinery.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Boom;

fn assert_machine_continue(actions: &Actions<MailAddr, Never, Vec<Never>, NoBirths>) {
    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
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

/// A parent whose handle fails on the first message.
struct FailingParent {
    fail: bool,
}

type ParentEvent = User<MailAddr, u64>;

impl behavior::Protocol for FailingParent {
    type Addr = MailAddr;
    type Msg = u64;
}

impl BehaviorBase for FailingParent {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for FailingParent {
    type Protocol = Self;
    type Event = ParentEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Boom;
    type Birth = Births<Child>;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        if self.fail {
            self.fail = false;
            return Err(Boom);
        }
        Ok(Actions::create(vec![behavior::Create::birth(
            event.message,
            child(0),
        )]))
    }
}

/// A controlled error during a deferred drain rejects the complete mailbox
/// turn. State, phase, and the accepted held queue remain exactly unchanged.
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
    let deferred = machine.transition(User::user(MailAddr(0), 1)).unwrap();
    assert_machine_continue(&deferred);
    let deferred = machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_machine_continue(&deferred);
    assert_eq!(machine.held(), 2);

    let result = machine.transition(User::user(MailAddr(0), 0));
    assert!(matches!(
        result,
        Err(behavior::MachineError {
            event: User {
                from: MailAddr(0),
                message: 0
            },
            cause: Boom,
        })
    ));
    assert_eq!(machine.held(), 2);
    assert!(machine.phase() == Phase::P0);
    assert!(machine.state().is_empty());

    // The fold remains usable and the next P0 communication joins the same queue.
    let deferred = machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_machine_continue(&deferred);
    assert_eq!(machine.held(), 3);
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
            (Phase::P1, 1) => {
                _seen.push(99);
                Err(Boom)
            }
            (Phase::P1, _) => Ok(Move::Stay),
        },
    );
    let mut machine = machine.initialize().unwrap().behavior;
    let deferred = machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_machine_continue(&deferred);
    let opened = machine.transition(User::user(MailAddr(0), 0)).unwrap();
    assert_machine_continue(&opened);
    assert_eq!(machine.held(), 0);

    // In P1 now: a direct id-1 message errors, held untouched.
    let result = machine.transition(User::user(MailAddr(0), 1));
    assert!(matches!(
        result,
        Err(behavior::MachineError {
            event: User {
                from: MailAddr(0),
                message: 1
            },
            cause: Boom,
        })
    ));
    assert_eq!(machine.held(), 0);
    assert!(machine.state().is_empty());
    // The fold is still live for a non-failing message.
    let continued = machine.transition(User::user(MailAddr(0), 3)).unwrap();
    assert_machine_continue(&continued);
}

/// Supervision propagates the inner controlled error without touching the
/// slot table: no slot is born, killed, or replaced by a failed step.
#[tokio::test]
async fn supervision_propagates_inner_errors_without_touching_slots() {
    let supervisor = Supervise::new(
        FailingParent { fail: true },
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
            behavior::RestartTiming::Immediate,
        ),
        behavior::Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let before = supervisor.child_count();

    let result = supervisor.transition(SupervisionEvent::Behavior(UserEvent::user(MailAddr(0), 7)));
    assert!(matches!(
        result,
        Err(behavior::SuperviseError::Behavior(Boom))
    ));
    assert_eq!(supervisor.child_count(), before);
    for nonce in 0..2 {
        assert!(
            supervisor
                .is_restartable(u64::try_from(nonce).unwrap())
                .unwrap()
        );
    }
    // The fold recovers on the next message.
    let recovered = supervisor
        .transition(SupervisionEvent::Behavior(UserEvent::user(MailAddr(0), 7)))
        .unwrap();
    assert!(recovered.sends.owned.child_observations.is_empty());
    assert!(recovered.sends.owned.creation_observations.is_empty());
    assert!(recovered.sends.owned.schedules.is_empty());
    assert!(recovered.sends.owned.replacement_inputs.is_empty());
    assert!(recovered.sends.owned.failure_reports.is_empty());
    assert!(recovered.sends.owned.shutdowns.is_empty());
    assert!(recovered.sends.inner.is_empty());
    assert_eq!(recovered.creates.len(), 1);
    assert_eq!(recovered.creates[0].nonce, 7);
    assert!(matches!(recovered.become_, Step::Continue));
}

/// The `restart_*` helpers expose exactly the documented strategies.
#[test]
fn restart_helpers_expose_the_documented_strategies() {
    assert_eq!(restart_one(), Strategy::OneForOne);
    assert_eq!(restart_all(), Strategy::OneForAll);
    assert_eq!(restart_rest(), Strategy::RestForOne);
}

/// The driver propagates the first controlled failure and leaves the
/// unconsumed mailbox tail intact.
#[tokio::test]
async fn driver_propagates_errors_and_preserves_the_tail() {
    let supervisor = Supervise::new(
        FailingParent { fail: true },
        behavior::ChildTopology::indexed(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        ),
        behavior::RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            u32::MAX,
            Duration::MAX,
            behavior::RestartTiming::Immediate,
        ),
        behavior::Proxy::new,
    )
    .unwrap();
    let mut mailbox = Mailbox::new([
        SupervisionEvent::Behavior(UserEvent::user(MailAddr(9), 3)), // fails (first)
        SupervisionEvent::Behavior(UserEvent::user(MailAddr(9), 5)), // never reached
    ]);
    let result = drive(supervisor, &mut mailbox);
    assert!(matches!(
        result,
        Err(behavior::SuperviseError::Behavior(Boom))
    ));
    assert_eq!(mailbox.pending(), 1);
}

/// A later deferred-message error cannot commit the successful prefix of the
/// staged drain or consume any accepted held communication.
#[tokio::test]
async fn fsm_error_mid_drain_rolls_back_the_complete_staged_drain() {
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
    let deferred = machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_machine_continue(&deferred);
    let deferred = machine.transition(User::user(MailAddr(0), 3)).unwrap();
    assert_machine_continue(&deferred);
    let deferred = machine.transition(User::user(MailAddr(0), 1)).unwrap();
    assert_machine_continue(&deferred);
    let result = machine.transition(User::user(MailAddr(0), 0));
    assert!(matches!(
        result,
        Err(behavior::MachineError {
            event: User {
                from: MailAddr(0),
                message: 0
            },
            cause: Boom,
        })
    ));
    assert!(machine.state().is_empty());
    assert!(machine.phase() == Phase::P0);
    assert_eq!(machine.held(), 3);
}
use behavior_testkit::InitializeTest;
