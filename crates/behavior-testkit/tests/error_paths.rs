//! Controlled-error attacks for `Machine` and the testkit driver.

use behavior_actors::{Machine, Move};
use std::cell::Cell;
use std::rc::Rc;

use behavior_core::{
    Actions, Behavior, BehaviorActed, Delivery, MailAddr, Never, NoBirths, Recipient, Step, User,
    UserEvent,
};
use behavior_testkit::{Mailbox, TestRecipient, drive};

/// A controlled failure type: unit-like, `Send`, no display machinery.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Boom;

fn assert_machine_continue(actions: &Actions<MailAddr, Never, Vec<Never>, NoBirths>) {
    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
}

struct RejectInput;

impl behavior_core::Protocol for RejectInput {
    type Addr = MailAddr;
    type Msg = u64;
}

impl Behavior for RejectInput {
    type Protocol = Self;
    type Event = User<MailAddr, u64>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Boom;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Err(Boom)
    }
}

/// A controlled error during a deferred drain rejects the complete mailbox
/// turn. State, phase, and the accepted held queue remain exactly unchanged.
#[test]
fn fsm_error_mid_drain_preserves_the_unprocessed_batch() {
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
        Err(behavior_actors::MachineError {
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

    // The machine remains usable and the next P0 communication joins the same queue.
    let deferred = machine.transition(User::user(MailAddr(0), 2)).unwrap();
    assert_machine_continue(&deferred);
    assert_eq!(machine.held(), 3);
}

/// A direct-step error consumes only the errored message: held stays intact
/// and the fold is still usable.
#[test]
fn fsm_direct_step_error_keeps_held_intact() {
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
        Err(behavior_actors::MachineError {
            event: User {
                from: MailAddr(0),
                message: 1
            },
            cause: Boom,
        })
    ));
    assert_eq!(machine.held(), 0);
    assert!(machine.state().is_empty());
    // The machine is still live for a non-failing message.
    let continued = machine.transition(User::user(MailAddr(0), 3)).unwrap();
    assert_machine_continue(&continued);
}

/// The driver propagates the first controlled failure and leaves the
/// unconsumed mailbox tail intact.
#[test]
fn driver_propagates_errors_and_preserves_the_tail() {
    let mut mailbox = Mailbox::new([User::user(MailAddr(9), 3), User::user(MailAddr(9), 5)]);
    let result = drive(RejectInput, &mut mailbox);
    assert!(matches!(result, Err(Boom)));
    assert_eq!(mailbox.pending(), 1);
}

struct PrefixEmits {
    seen: Rc<Cell<usize>>,
}

#[behavior_core::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<TestRecipient<u64>>>, births = NoBirths, error = Boom)]
impl PrefixEmits {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u64,
    ) -> behavior_core::Acted<MailAddr, Never, Vec<Delivery<TestRecipient<u64>>>, NoBirths, Boom>
    {
        if message == 3 {
            return Err(Boom);
        }
        self.seen.set(self.seen.get() + 1);
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            message,
        )]))
    }
}

#[test]
fn driver_discards_successful_prefix_after_later_error() {
    let seen = Rc::new(Cell::new(0));
    let mut mailbox = Mailbox::new([1, 2, 3, 4].map(|message| User::new(MailAddr(9), message)));
    let result = drive(PrefixEmits { seen: seen.clone() }, &mut mailbox);
    assert!(matches!(result, Err(Boom)));
    assert_eq!(seen.get(), 2);
    assert_eq!(mailbox.pending(), 1);
    let next_message = mailbox.receive().unwrap();
    assert_eq!(next_message.message, 4);
}

/// A later deferred-message error cannot commit the successful prefix of the
/// staged drain or consume any accepted held communication.
#[test]
fn fsm_error_mid_drain_rolls_back_the_complete_staged_drain() {
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
        Err(behavior_actors::MachineError {
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
use behavior_actors::Activate;
