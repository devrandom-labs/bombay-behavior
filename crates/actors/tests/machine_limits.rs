use std::time::Duration;

use behavior::{
    Actions, BehaviorBase as _, EventLayer, MailAddr, MessageProtocol, Never, NoSends, Recipient,
    SendLayer, Step, Stopped, User, UserEvent, delegate_transition, initialize,
};
use behavior_actors::{
    Activate as _, InputLimit, Machine, Move, OwnedTimerElapsed, ReceiveTimeout, StashRoute,
    StopOnShutdown, UnlimitedStash,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Door {
    Closed,
    Admitting,
    Open,
}

#[derive(Debug)]
enum Command {
    Work(Box<[u64]>),
    Open,
    Poll,
    Halt,
}

#[derive(Default)]
struct Account {
    attempts: usize,
    deposits: Vec<u64>,
}

fn receive(door: Door, account: &mut Account, command: &Command) -> Result<Move<Door>, Never> {
    match (door, command) {
        (_, Command::Open) => Ok(Move::Goto(Door::Open)),
        (Door::Closed | Door::Admitting, Command::Work(_)) => {
            account.attempts += 1;
            Ok(Move::Defer)
        }
        (Door::Open, Command::Work(amounts)) => {
            account.deposits.extend_from_slice(amounts);
            Ok(Move::Stay)
        }
        (Door::Closed, Command::Poll) => Ok(Move::Goto(Door::Admitting)),
        (Door::Admitting | Door::Open, Command::Poll) => Ok(Move::Stay),
        (_, Command::Halt) => Ok(Move::Stop),
    }
}

#[test]
fn zero_slots_returns_the_complete_original_and_keeps_the_callback_mutation() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Command>>>::global(MailAddr(29));
    let mut account = Machine::bounded(
        Account::default(),
        Door::Closed,
        receive,
        InputLimit::new(0, refused),
    );
    let initialized = initialize(&mut account).expect("the empty account starts");
    assert!(initialized.sends.is_empty());
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    let payload = vec![11_u64, 13].into_boxed_slice();
    let original = payload.as_ptr();
    let actions = delegate_transition(
        &mut account,
        User::new(MailAddr(17), Command::Work(payload)),
    )
    .expect("full holding is a normal refusal send");
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
    let mut sends = actions.sends;
    assert_eq!(sends.len(), 1);
    let delivery = sends.pop().expect("the refused original is emitted once");
    assert_eq!(delivery.to, refused);
    assert_eq!(delivery.message.from, MailAddr(17));
    let Command::Work(payload) = delivery.message.message else {
        panic!("the refusal contains the original work");
    };
    assert_eq!(payload.as_ptr(), original);
    assert_eq!(&*payload, &[11, 13]);
    assert_eq!(account.state().attempts, 1);
    assert!(account.state().deposits.is_empty());
    assert_eq!(account.phase(), Door::Closed);
    assert_eq!(account.held(), 0);
    let remaining = account.take();
    assert!(remaining.is_empty());
}

#[test]
fn full_holding_does_not_block_poll_open_or_later_work() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Command>>>::global(MailAddr(29));
    let mut account = Machine::bounded(
        Account::default(),
        Door::Closed,
        receive,
        InputLimit::new(2, refused),
    )
    .initialize()
    .expect("the account starts")
    .behavior;
    for amount in [3_u64, 5] {
        let actions = account
            .receive(MailAddr(amount), Command::Work(Box::from([amount])))
            .expect("the original fits a held slot");
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
    }
    assert_eq!(account.held(), 2);
    let payload = vec![7_u64].into_boxed_slice();
    let original = payload.as_ptr();
    let overflow = account
        .receive(MailAddr(19), Command::Work(payload))
        .expect("the full account continues");
    assert_eq!(overflow.become_, Step::Continue);
    assert!(overflow.creates.is_empty());
    assert_eq!(overflow.sends.len(), 1);
    let delivery = overflow
        .sends
        .into_iter()
        .next()
        .expect("one original is refused");
    assert_eq!(delivery.to, refused);
    assert_eq!(delivery.message.from, MailAddr(19));
    let Command::Work(payload) = delivery.message.message else {
        panic!("the third original is refused");
    };
    assert_eq!(payload.as_ptr(), original);
    assert_eq!(&*payload, &[7]);
    assert_eq!(account.held(), 2);
    assert_eq!(account.state().attempts, 3);
    let poll = account
        .receive(MailAddr(23), Command::Poll)
        .expect("poll needs no slot");
    assert!(poll.sends.is_empty());
    assert!(poll.creates.is_empty());
    assert_eq!(poll.become_, Step::Continue);
    assert_eq!(account.held(), 2);
    assert_eq!(account.phase(), Door::Admitting);
    assert_eq!(account.state().attempts, 5);
    let opened = account
        .receive(MailAddr(31), Command::Open)
        .expect("open replays admitted work");
    assert!(opened.sends.is_empty());
    assert!(opened.creates.is_empty());
    assert_eq!(opened.become_, Step::Continue);
    assert_eq!(account.state().deposits, [3, 5]);
    assert_eq!(account.held(), 0);
    let direct = account
        .receive(MailAddr(37), Command::Work(Box::from([41_u64])))
        .expect("open work does not require a held slot");
    assert!(direct.sends.is_empty());
    assert!(direct.creates.is_empty());
    assert_eq!(direct.become_, Step::Continue);
    assert_eq!(account.state().deposits, [3, 5, 41]);
    assert_eq!(account.held(), 0);
}

#[test]
fn stopping_at_capacity_keeps_fifo_originals_available_once() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Command>>>::global(MailAddr(29));
    let mut account = Machine::bounded(
        Account::default(),
        Door::Closed,
        receive,
        InputLimit::new(2, refused),
    );
    let initialized = initialize(&mut account).expect("the account starts");
    assert!(initialized.sends.is_empty());
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    let first = vec![43_u64, 47].into_boxed_slice();
    let first_pointer = first.as_ptr();
    let second = vec![53_u64, 59].into_boxed_slice();
    let second_pointer = second.as_ptr();
    for (sender, payload) in [(61, first), (67, second)] {
        let actions = delegate_transition(
            &mut account,
            User::new(MailAddr(sender), Command::Work(payload)),
        )
        .expect("the original is held");
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
    }
    let stopped = delegate_transition(&mut account, User::new(MailAddr(71), Command::Halt))
        .expect("stop needs no slot");
    assert!(stopped.sends.is_empty());
    assert!(stopped.creates.is_empty());
    assert!(matches!(stopped.become_, Step::Stop(_)));
    let mut originals = account.take();
    assert_eq!(originals.len(), 2);
    for (sender, pointer, amounts) in [
        (61, first_pointer, [43, 47]),
        (67, second_pointer, [53, 59]),
    ] {
        let original = originals
            .pop_front()
            .expect("the next admitted original remains");
        assert_eq!(original.from, MailAddr(sender));
        let Command::Work(payload) = original.message else {
            panic!("original work remains");
        };
        assert_eq!(payload.as_ptr(), pointer);
        assert_eq!(&*payload, &amounts);
    }
    assert!(originals.is_empty());
    let repeated = account.take();
    assert!(repeated.is_empty());
    assert_eq!(account.held(), 0);
    assert_eq!(account.phase(), Door::Closed);
    assert_eq!(account.state().attempts, 2);
}

#[test]
fn outer_stash_replay_accumulates_inner_capacity_refusals_without_actor_error() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Command>>>::global(MailAddr(29));
    let machine = Machine::bounded(
        Account::default(),
        Door::Closed,
        receive,
        InputLimit::new(1, refused),
    );
    let mut stash = UnlimitedStash::new(machine, |machine, command| match command {
        Command::Poll => StashRoute::Release,
        Command::Open | Command::Halt => StashRoute::Deliver,
        Command::Work(_) => match machine.phase() {
            Door::Closed => StashRoute::Stash,
            Door::Admitting | Door::Open => StashRoute::Deliver,
        },
    })
    .initialize()
    .expect("both input owners start")
    .behavior;
    let mut pointers = Vec::new();
    for amount in [73_u64, 79, 83] {
        let payload = vec![amount].into_boxed_slice();
        pointers.push(payload.as_ptr());
        let actions = stash
            .receive(MailAddr(amount), Command::Work(payload))
            .expect("the outer stash holds the original");
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
    }
    assert_eq!(stash.held(), 3);
    assert_eq!(stash.base().held(), 0);
    let released = stash
        .receive(MailAddr(89), Command::Poll)
        .expect("all replayed inputs settle without an actor error");
    assert!(released.creates.is_empty());
    assert_eq!(released.become_, Step::Continue);
    assert_eq!(released.sends.len(), 2);
    for (delivery, (amount, pointer)) in released
        .sends
        .into_iter()
        .zip([(79, pointers[1]), (83, pointers[2])])
    {
        assert_eq!(delivery.to, refused);
        assert_eq!(delivery.message.from, MailAddr(amount));
        let Command::Work(payload) = delivery.message.message else {
            panic!("refused original work");
        };
        assert_eq!(payload.as_ptr(), pointer);
        assert_eq!(&*payload, &[amount]);
    }
    assert_eq!(stash.held(), 0);
    assert_eq!(stash.base().held(), 1);
    assert_eq!(stash.base().state().attempts, 3);
    let opened = stash
        .receive(MailAddr(97), Command::Open)
        .expect("the admitted original completes");
    assert!(opened.sends.is_empty());
    assert!(opened.creates.is_empty());
    assert_eq!(opened.become_, Step::Continue);
    assert_eq!(stash.base().state().deposits, [73]);
    assert_eq!(stash.base().held(), 0);
}

#[test]
fn timeout_outside_shutdown_preserves_the_capacity_refusal_and_its_live_timer() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Command>>>::global(MailAddr(29));
    let machine = Machine::bounded(
        Account::default(),
        Door::Closed,
        receive,
        InputLimit::new(0, refused),
    );
    let mut account =
        ReceiveTimeout::new(StopOnShutdown::new(machine), Duration::from_secs(5), |_| {
            Actions::stop()
        });
    let initial = initialize(&mut account).expect("both policies start");
    let SendLayer {
        owned: timers,
        inner: SendLayer {
            owned: NoSends,
            inner: sends,
        },
    } = initial.sends;
    assert!(sends.is_empty());
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    let payload = vec![101_u64, 103].into_boxed_slice();
    let original = payload.as_ptr();
    let actions = delegate_transition(
        &mut account,
        UserEvent::user(MailAddr(107), Command::Work(payload)),
    )
    .expect("capacity refusal is successful composition");
    let SendLayer {
        owned: timers,
        inner: SendLayer {
            owned: NoSends,
            inner: sends,
        },
    } = actions.sends;
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    let generation = timers[0].generation;
    assert_eq!(sends.len(), 1);
    let delivery = sends.into_iter().next().expect("one complete refusal");
    assert_eq!(delivery.to, refused);
    assert_eq!(delivery.message.from, MailAddr(107));
    let Command::Work(payload) = delivery.message.message else {
        panic!("original refused work");
    };
    assert_eq!(payload.as_ptr(), original);
    assert_eq!(&*payload, &[101, 103]);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(account.base().held(), 0);
    assert_eq!(account.base().state().attempts, 1);
    let stopped = delegate_transition(
        &mut account,
        EventLayer::Owned(OwnedTimerElapsed::new(generation)),
    )
    .expect("the real timer remains usable after refusal");
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
fn shutdown_outside_timeout_preserves_the_capacity_refusal_and_its_live_timer() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Command>>>::global(MailAddr(29));
    let machine = Machine::bounded(
        Account::default(),
        Door::Closed,
        receive,
        InputLimit::new(0, refused),
    );
    let mut account =
        StopOnShutdown::new(ReceiveTimeout::new(machine, Duration::from_secs(5), |_| {
            Actions::stop()
        }));
    let initial = initialize(&mut account).expect("both policies start");
    let SendLayer {
        owned: NoSends,
        inner: SendLayer {
            owned: timers,
            inner: sends,
        },
    } = initial.sends;
    assert!(sends.is_empty());
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    let payload = vec![109_u64, 113].into_boxed_slice();
    let original = payload.as_ptr();
    let actions = delegate_transition(
        &mut account,
        UserEvent::user(MailAddr(127), Command::Work(payload)),
    )
    .expect("capacity refusal is successful composition");
    let SendLayer {
        owned: NoSends,
        inner: SendLayer {
            owned: timers,
            inner: sends,
        },
    } = actions.sends;
    assert_eq!(timers.len(), 1);
    assert_eq!(timers[0].after, Duration::from_secs(5));
    let generation = timers[0].generation;
    assert_eq!(sends.len(), 1);
    let delivery = sends.into_iter().next().expect("one complete refusal");
    assert_eq!(delivery.to, refused);
    assert_eq!(delivery.message.from, MailAddr(127));
    let Command::Work(payload) = delivery.message.message else {
        panic!("original refused work");
    };
    assert_eq!(payload.as_ptr(), original);
    assert_eq!(&*payload, &[109, 113]);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(account.base().held(), 0);
    assert_eq!(account.base().state().attempts, 1);
    let stopped = delegate_transition(
        &mut account,
        EventLayer::Inner(EventLayer::Owned(OwnedTimerElapsed::new(generation))),
    )
    .expect("the real timer remains usable after refusal");
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
