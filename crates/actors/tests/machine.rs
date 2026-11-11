use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use behavior::{
    Actions, BehaviorBase as _, EventLayer, MailAddr, Never, NoBirths, NoSends, SendLayer, Step,
    Stopped, User, UserEvent, delegate_transition, initialize,
};
use behavior_actors::{
    Activate as _, Machine, Move, OwnedTimerElapsed, ReceiveTimeout, StopOnShutdown,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Closed,
    Open,
}

#[derive(Debug)]
enum Command {
    Deposit(Box<[u64]>),
    Refuse(Box<[u64]>),
    Hold(Box<[u64]>),
    Open,
    Stop,
    Panic,
}

#[derive(Debug, Default)]
struct Account {
    deposits: Vec<u64>,
}

#[derive(Debug, PartialEq, Eq)]
struct Refusal;

fn receive(phase: Phase, account: &mut Account, command: &Command) -> Result<Move<Phase>, Refusal> {
    match (phase, command) {
        (_, Command::Open) => Ok(Move::Goto(Phase::Open)),
        (Phase::Closed, _) => Ok(Move::Defer),
        (Phase::Open, Command::Deposit(amounts)) => {
            account.deposits.extend_from_slice(amounts);
            Ok(Move::Stay)
        }
        (Phase::Open, Command::Refuse(_)) => Err(Refusal),
        (Phase::Open, Command::Hold(_)) => Ok(Move::Defer),
        (Phase::Open, Command::Stop) => Ok(Move::Stop),
        (Phase::Open, Command::Panic) => panic!("the deposit callback unwinds"),
    }
}

fn assert_continuing(actions: Actions<MailAddr, Never, Vec<Never>, NoBirths>) {
    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn refused_deposit_returns_its_original_sender_and_payload() {
    let definition = Machine::new(Account::default(), Phase::Closed, receive);
    let initialized = definition.initialize().expect("the account starts");
    assert_continuing(initialized.actions);
    let mut account = initialized.behavior;
    let payload = vec![11_u64, 13].into_boxed_slice();
    let original = payload.as_ptr();
    let held = account
        .receive(MailAddr(17), Command::Refuse(payload))
        .expect("the closed account retains the original refusal input");
    assert_continuing(held);

    let refusal = account
        .receive(MailAddr(19), Command::Open)
        .expect_err("opening encounters the refused deposit");
    assert_eq!(refusal.cause, Refusal);
    assert_eq!(refusal.event.from, MailAddr(17));
    let Command::Refuse(payload) = refusal.event.message else {
        panic!("the refusal owns the failed deposit, not the accepted Open");
    };
    assert_eq!(payload.as_ptr(), original);
    assert_eq!(&*payload, &[11, 13]);
    assert_eq!(account.phase(), Phase::Open);
    assert_eq!(account.held(), 0);
    assert!(account.state().deposits.is_empty());
}

#[test]
fn later_refusal_preserves_the_completed_deposit() {
    let definition = Machine::new(Account::default(), Phase::Closed, receive);
    let initialized = definition.initialize().expect("the account starts");
    assert_continuing(initialized.actions);
    let mut account = initialized.behavior;
    for (origin, command) in [
        (MailAddr(23), Command::Deposit(Box::from([29_u64]))),
        (MailAddr(31), Command::Refuse(Box::from([37_u64]))),
        (MailAddr(41), Command::Deposit(Box::from([43_u64]))),
    ] {
        let held = account
            .receive(origin, command)
            .expect("closed inputs defer");
        assert_continuing(held);
    }

    let refusal = account
        .receive(MailAddr(47), Command::Open)
        .expect_err("the middle input refuses after the first deposit");
    assert_eq!(account.state().deposits, [29]);
    assert_eq!(account.phase(), Phase::Open);
    assert_eq!(account.held(), 1);
    assert_eq!(refusal.event.from, MailAddr(31));
    assert_eq!(refusal.cause, Refusal);
    let Command::Refuse(amounts) = refusal.event.message else {
        panic!("the actual refused input remains owned");
    };
    assert_eq!(&*amounts, &[37]);
}

#[test]
fn taking_inputs_preserves_the_reheld_prefix_and_unread_suffix() {
    let mut account = Machine::new(Account::default(), Phase::Closed, receive);
    let prefix = vec![53_u64].into_boxed_slice();
    let prefix_pointer = prefix.as_ptr();
    let suffix = vec![59_u64].into_boxed_slice();
    let suffix_pointer = suffix.as_ptr();
    let initialized = initialize(&mut account).expect("the account starts");
    assert_continuing(initialized);
    for (origin, command) in [
        (MailAddr(61), Command::Hold(prefix)),
        (MailAddr(67), Command::Refuse(Box::from([71_u64]))),
        (MailAddr(73), Command::Deposit(suffix)),
    ] {
        let held = delegate_transition(&mut account, User::new(origin, command))
            .expect("the closed account defers each input");
        assert_continuing(held);
    }
    let refusal = delegate_transition(&mut account, User::new(MailAddr(79), Command::Open))
        .expect_err("the middle input refuses");
    assert_eq!(refusal.event.from, MailAddr(67));
    assert_eq!(refusal.cause, Refusal);
    let Command::Refuse(amounts) = refusal.event.message else {
        panic!("the refused payload remains in its error");
    };
    assert_eq!(&*amounts, &[71]);
    assert_eq!(account.phase(), Phase::Open);
    assert!(account.state().deposits.is_empty());

    let mut inputs = account.take();
    assert_eq!(account.held(), 0);
    assert_eq!(inputs.len(), 2);
    let prefix = inputs.pop_front().expect("the reheld prefix remains first");
    assert_eq!(prefix.from, MailAddr(61));
    let Command::Hold(amounts) = prefix.message else {
        panic!("the prefix retains its decision and payload");
    };
    assert_eq!(amounts.as_ptr(), prefix_pointer);
    assert_eq!(&*amounts, &[53]);
    let suffix = inputs.pop_front().expect("the unread suffix follows");
    assert_eq!(suffix.from, MailAddr(73));
    let Command::Deposit(amounts) = suffix.message else {
        panic!("the unread deposit remains owned");
    };
    assert_eq!(amounts.as_ptr(), suffix_pointer);
    assert_eq!(&*amounts, &[59]);
    let repeated = account.take();
    assert!(repeated.is_empty());
    assert_eq!(account.phase(), Phase::Open);
    assert!(account.state().deposits.is_empty());
    let deposit = delegate_transition(
        &mut account,
        User::new(MailAddr(157), Command::Deposit(Box::from([163_u64]))),
    )
    .expect("taking inputs preserves the callback and the current phase");
    assert_continuing(deposit);
    assert_eq!(account.state().deposits, [163]);
}

#[test]
fn stop_preserves_deferred_and_unprocessed_inputs() {
    let mut account = Machine::new(Account::default(), Phase::Closed, receive);
    let initialized = initialize(&mut account).expect("the account starts");
    assert_continuing(initialized);
    for (origin, command) in [
        (MailAddr(83), Command::Hold(Box::from([89_u64]))),
        (MailAddr(97), Command::Stop),
        (MailAddr(101), Command::Deposit(Box::from([103_u64]))),
    ] {
        let held = delegate_transition(&mut account, User::new(origin, command))
            .expect("the closed account defers each input");
        assert_continuing(held);
    }
    let stopped = delegate_transition(&mut account, User::new(MailAddr(107), Command::Open))
        .expect("the held Stop is a normal verdict");
    assert!(stopped.sends.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
    assert_eq!(account.phase(), Phase::Open);
    assert!(account.state().deposits.is_empty());
    let inputs = account.take();
    let mut inputs = inputs.into_iter();
    let prefix = inputs.next().expect("the reheld prefix survives Stop");
    assert_eq!(prefix.from, MailAddr(83));
    let Command::Hold(amounts) = prefix.message else {
        panic!("the reheld input remains intact");
    };
    assert_eq!(&*amounts, &[89]);
    let suffix = inputs.next().expect("Stop does not visit the suffix");
    assert_eq!(suffix.from, MailAddr(101));
    let Command::Deposit(amounts) = suffix.message else {
        panic!("the unread deposit remains intact");
    };
    assert_eq!(&*amounts, &[103]);
    let end = inputs.next();
    assert!(end.is_none());
}

#[test]
fn replay_unwind_keeps_the_borrowed_input_and_unread_suffix() {
    let mut account = Machine::new(Account::default(), Phase::Closed, receive);
    let initialized = initialize(&mut account).expect("the account starts");
    assert_continuing(initialized);
    for (origin, command) in [
        (MailAddr(109), Command::Hold(Box::from([113_u64]))),
        (MailAddr(127), Command::Panic),
        (MailAddr(131), Command::Deposit(Box::from([137_u64]))),
    ] {
        let held = delegate_transition(&mut account, User::new(origin, command))
            .expect("the closed account defers each input");
        assert_continuing(held);
    }
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        delegate_transition(&mut account, User::new(MailAddr(139), Command::Open))
    }));
    assert!(unwind.is_err());
    assert_eq!(account.phase(), Phase::Open);
    assert!(account.state().deposits.is_empty());
    let inputs = account.take();
    let mut inputs = inputs.into_iter();
    let prefix = inputs.next().expect("the reheld prefix survives unwind");
    assert_eq!(prefix.from, MailAddr(109));
    let Command::Hold(amounts) = prefix.message else {
        panic!("the reheld input remains intact");
    };
    assert_eq!(&*amounts, &[113]);
    let current = inputs
        .next()
        .expect("the callback only borrowed this input");
    assert_eq!(current.from, MailAddr(127));
    assert!(matches!(current.message, Command::Panic));
    let suffix = inputs
        .next()
        .expect("the callback never visited this suffix");
    assert_eq!(suffix.from, MailAddr(131));
    let Command::Deposit(amounts) = suffix.message else {
        panic!("the unread deposit remains intact");
    };
    assert_eq!(&*amounts, &[137]);
    let end = inputs.next();
    assert!(end.is_none());
}

#[test]
fn direct_refusal_keeps_actual_mutations_and_the_original_input() {
    let definition = Machine::new(
        Account::default(),
        Phase::Open,
        |phase, account, command| match command {
            Command::Refuse(amounts) => {
                account.deposits.extend_from_slice(amounts);
                Err(Refusal)
            }
            other => receive(phase, account, other),
        },
    );
    let initialized = definition.initialize().expect("the account starts");
    assert_continuing(initialized.actions);
    let mut account = initialized.behavior;
    let payload = vec![149_u64].into_boxed_slice();
    let original = payload.as_ptr();
    let refusal = account
        .receive(MailAddr(151), Command::Refuse(payload))
        .expect_err("the callback refuses after mutating its actual state");
    assert_eq!(refusal.cause, Refusal);
    assert_eq!(refusal.event.from, MailAddr(151));
    let Command::Refuse(amounts) = refusal.event.message else {
        panic!("direct refusal retains the actual input");
    };
    assert_eq!(amounts.as_ptr(), original);
    assert_eq!(&*amounts, &[149]);
    assert_eq!(account.state().deposits, [149]);
    assert_eq!(account.phase(), Phase::Open);
    assert_eq!(account.held(), 0);
}

#[test]
fn timeout_outside_shutdown_keeps_the_refusal_and_its_live_timer() {
    let machine = Machine::new(Account::default(), Phase::Closed, receive);
    let mut account =
        ReceiveTimeout::new(StopOnShutdown::new(machine), Duration::from_secs(5), |_| {
            Actions::stop()
        });
    let started = initialize(&mut account).expect("both policies initialize");
    let SendLayer {
        owned: timers,
        inner: SendLayer {
            owned: NoSends,
            inner: sends,
        },
    } = started.sends;
    assert!(sends.is_empty());
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);
    let held = delegate_transition(
        &mut account,
        UserEvent::user(MailAddr(167), Command::Refuse(Box::from([173_u64]))),
    )
    .expect("the original user input reaches the machine once");
    let SendLayer {
        owned: timers,
        inner: SendLayer {
            owned: NoSends,
            inner: sends,
        },
    } = held.sends;
    assert!(sends.is_empty());
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    let generation = timers[0].generation;
    assert!(held.creates.is_empty());
    assert_eq!(held.become_, Step::Continue);
    let refusal = delegate_transition(&mut account, UserEvent::user(MailAddr(179), Command::Open))
        .expect_err("both wrappers return the actual inner refusal");
    assert_eq!(refusal.cause, Refusal);
    assert_eq!(refusal.event.from, MailAddr(167));
    let Command::Refuse(amounts) = refusal.event.message else {
        panic!("neither wrapper substitutes the replay trigger");
    };
    assert_eq!(&*amounts, &[173]);
    assert_eq!(account.base().phase(), Phase::Open);
    assert_eq!(account.base().held(), 0);
    let stopped = delegate_transition(
        &mut account,
        EventLayer::Owned(OwnedTimerElapsed::new(generation)),
    )
    .expect("refusal did not replace or consume the live inactivity timer");
    let SendLayer {
        owned: timers,
        inner: SendLayer {
            owned: NoSends,
            inner: sends,
        },
    } = stopped.sends;
    assert!(timers.is_empty());
    assert!(sends.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
}

#[test]
fn shutdown_outside_timeout_keeps_the_refusal_and_its_live_timer() {
    let machine = Machine::new(Account::default(), Phase::Closed, receive);
    let mut account =
        StopOnShutdown::new(ReceiveTimeout::new(machine, Duration::from_secs(5), |_| {
            Actions::stop()
        }));
    let started = initialize(&mut account).expect("both policies initialize");
    let SendLayer {
        owned: NoSends,
        inner: SendLayer {
            owned: timers,
            inner: sends,
        },
    } = started.sends;
    assert!(sends.is_empty());
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);
    let held = delegate_transition(
        &mut account,
        UserEvent::user(MailAddr(181), Command::Refuse(Box::from([191_u64]))),
    )
    .expect("the original user input reaches the machine once");
    let SendLayer {
        owned: NoSends,
        inner: SendLayer {
            owned: timers,
            inner: sends,
        },
    } = held.sends;
    assert!(sends.is_empty());
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    let generation = timers[0].generation;
    assert!(held.creates.is_empty());
    assert_eq!(held.become_, Step::Continue);
    let refusal = delegate_transition(&mut account, UserEvent::user(MailAddr(193), Command::Open))
        .expect_err("both wrappers return the actual inner refusal");
    assert_eq!(refusal.cause, Refusal);
    assert_eq!(refusal.event.from, MailAddr(181));
    let Command::Refuse(amounts) = refusal.event.message else {
        panic!("neither wrapper substitutes the replay trigger");
    };
    assert_eq!(&*amounts, &[191]);
    assert_eq!(account.base().phase(), Phase::Open);
    assert_eq!(account.base().held(), 0);
    let stopped = delegate_transition(
        &mut account,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(generation))),
    )
    .expect("refusal did not replace or consume the live inactivity timer");
    let SendLayer {
        owned: NoSends,
        inner: SendLayer {
            owned: timers,
            inner: sends,
        },
    } = stopped.sends;
    assert!(timers.is_empty());
    assert!(sends.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
}
#[derive(Clone, Copy, Debug)]
enum Door {
    Closed,
    Open(u8),
}

impl PartialEq for Door {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Closed, Self::Closed) | (Self::Open(_), Self::Open(_))
        )
    }
}

fn receive_door(door: Door, _: &mut (), message: &u8) -> Result<Move<Door>, Never> {
    match (door, message) {
        (Door::Closed, 0) => Ok(Move::Defer),
        (Door::Closed, _) => Ok(Move::Goto(Door::Open(1))),
        (Door::Open(_), _) => Ok(Move::Goto(Door::Open(2))),
    }
}

#[test]
fn direct_designation_keeps_new_data_when_the_phase_compares_equal() {
    let mut door = Machine::new((), Door::Open(1), receive_door);
    let started = initialize(&mut door).unwrap();
    assert!(started.sends.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);
    let changed = delegate_transition(&mut door, User::new(MailAddr(7), 3)).unwrap();
    assert!(changed.sends.is_empty());
    assert!(changed.creates.is_empty());
    assert_eq!(changed.become_, Step::Continue);
    let Door::Open(revision) = door.phase() else {
        panic!("the open phase remains designated")
    };
    assert_eq!(revision, 2);
    assert_eq!(door.held(), 0);
}

#[test]
fn replay_designation_keeps_new_data_when_the_phase_compares_equal() {
    let mut door = Machine::new((), Door::Closed, receive_door);
    let started = initialize(&mut door).unwrap();
    assert!(started.sends.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);
    let held = delegate_transition(&mut door, User::new(MailAddr(7), 0)).unwrap();
    assert!(held.sends.is_empty());
    assert!(held.creates.is_empty());
    assert_eq!(held.become_, Step::Continue);
    let changed = delegate_transition(&mut door, User::new(MailAddr(11), 3)).unwrap();
    assert!(changed.sends.is_empty());
    assert!(changed.creates.is_empty());
    assert_eq!(changed.become_, Step::Continue);
    let Door::Open(revision) = door.phase() else {
        panic!("the open phase remains designated")
    };
    assert_eq!(revision, 2);
    assert_eq!(door.held(), 0);
}
