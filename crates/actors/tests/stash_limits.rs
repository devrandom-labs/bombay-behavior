use std::time::Duration;

use behavior::{
    Actions, BehaviorBase as _, Delivery, EventLayer, MailAddr, MessageProtocol, Never, NoSends, Recipient, SendLayer,
    Step, Stopped, User, UserEvent, delegate_transition, initialize,
};
use behavior_actors::{
    Machine, MessageAdapter, Move, OwnedTimerElapsed, ReceiveTimeout, ShutdownRequested, Stash, StashRoute, StashSends, StashStatus as _, StopOnShutdown,
};

struct Invoice {
    number: u8,
    receipt: Box<[u8; 2]>,
}

fn number(invoice: Invoice) -> u8 {
    invoice.number
}

#[test]
fn finite_stashes_return_original_inputs_and_keep_their_limit_after_take() {
    for limit in 0..=2 {
        let paid = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(29));
        let refused =
            Recipient::<MessageProtocol<MailAddr, User<MailAddr, Invoice>>>::global(MailAddr(31));
        let mut stash = Stash::new(MessageAdapter::new(paid, number), limit, refused, |_, _| {
            StashRoute::Stash
        });
        let started = initialize(&mut stash).expect("the adapter starts without effects");
        let StashSends { actor, refusals } = started.sends;
        assert!(actor.is_empty());
        assert!(refusals.is_empty());
        assert!(started.creates.is_empty());
        assert_eq!(started.become_, Step::Continue);
        for _ in 0..2 {
            let mut expected = Vec::new();
            for index in 0..5_u8 {
                let receipt = Box::new([index, index + 1]);
                let allocation = receipt.as_ptr();
                let from = MailAddr(if index % 2 == 0 { 7 } else { 13 });
                let invoice = Invoice {
                    number: index,
                    receipt,
                };
                let actions = delegate_transition(&mut stash, UserEvent::user(from, invoice))
                    .expect("a full stash refuses through Actions and continues");
                let StashSends {
                    actor,
                    mut refusals,
                } = actions.sends;
                assert!(actor.is_empty());
                assert!(actions.creates.is_empty());
                assert_eq!(actions.become_, Step::Continue);
                if usize::from(index) < limit {
                    assert!(refusals.is_empty());
                    expected.push((from, index, allocation));
                } else {
                    assert_eq!(refusals.len(), 1);
                    let delivery = refusals.pop().expect("one complete refused input");
                    assert_eq!(delivery.to, refused);
                    assert_eq!(delivery.message.from, from);
                    assert_eq!(delivery.message.message.number, index);
                    assert_eq!(delivery.message.message.receipt.as_ptr(), allocation);
                    assert_eq!(*delivery.message.message.receipt, [index, index + 1]);
                }
                assert_eq!(stash.held(), (usize::from(index) + 1).min(limit));
                assert_eq!(stash.base().destination(), &paid);
            }
            let held = stash.take();
            assert_eq!(held.len(), limit);
            for (input, (from, index, allocation)) in held.into_iter().zip(expected) {
                assert_eq!(input.from, from);
                assert_eq!(input.message.number, index);
                assert_eq!(input.message.receipt.as_ptr(), allocation);
                assert_eq!(*input.message.receipt, [index, index + 1]);
            }
            assert_eq!(stash.held(), 0);
            let repeated = stash.take();
            assert!(repeated.is_empty());
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Door {
    Closed,
    Open,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Parcel {
    Arrived(u64),
    Released,
}

fn receive(door: Door, stored: &mut Vec<u64>, parcel: &Parcel) -> Result<Move<Door>, Never> {
    Ok(match (door, parcel) {
        (Door::Closed, Parcel::Arrived(_)) => Move::Defer,
        (Door::Open, Parcel::Arrived(number)) => {
            stored.push(*number);
            Move::Stay
        }
        (Door::Closed, Parcel::Released) => Move::Goto(Door::Open),
        (Door::Open, Parcel::Released) => Move::Stay,
    })
}

#[test]
fn a_full_stash_still_delivers_and_releases_an_independent_machine_queue() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Parcel>>>::global(MailAddr(31));
    let machine = Machine::<MailAddr, _, _, _, _>::new(Vec::new(), Door::Closed, receive);
    let mut stash = Stash::new(machine, 1, refused, |machine, parcel| match parcel {
        Parcel::Arrived(number) if number % 2 == 1 && machine.phase() == Door::Closed => {
            StashRoute::Stash
        }
        Parcel::Arrived(_) => StashRoute::Deliver,
        Parcel::Released => StashRoute::Release,
    });
    let initialized = initialize(&mut stash).expect("the independent queues start empty");
    let StashSends { actor, refusals } = initialized.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    for parcel in [Parcel::Arrived(3), Parcel::Arrived(2)] {
        let actions = delegate_transition(&mut stash, UserEvent::user(MailAddr(7), parcel))
            .expect("the queues independently own their inputs");
        let StashSends { actor, refusals } = actions.sends;
        assert!(actor.is_empty());
        assert!(refusals.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
    }
    assert_eq!(stash.held(), 1);
    assert_eq!(stash.base().held(), 1);
    let refused_input = delegate_transition(
        &mut stash,
        UserEvent::user(MailAddr(13), Parcel::Arrived(5)),
    )
    .expect("overflow does not enter the inner Machine");
    let StashSends {
        actor,
        mut refusals,
    } = refused_input.sends;
    assert!(actor.is_empty());
    assert!(refused_input.creates.is_empty());
    assert_eq!(refused_input.become_, Step::Continue);
    assert_eq!(refusals.len(), 1);
    let delivery = refusals
        .pop()
        .expect("the selected recipient owns the original input");
    assert_eq!(delivery.to, refused);
    assert_eq!(delivery.message.from, MailAddr(13));
    assert_eq!(delivery.message.message, Parcel::Arrived(5));
    assert_eq!(stash.held(), 1);
    assert_eq!(stash.base().held(), 1);
    assert_eq!(stash.base().phase(), Door::Closed);
    assert!(stash.base().state().is_empty());
    let released = delegate_transition(&mut stash, UserEvent::user(MailAddr(19), Parcel::Released))
        .expect("release reaches a full stash and replays both independent queues");
    let StashSends { actor, refusals } = released.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(released.creates.is_empty());
    assert_eq!(released.become_, Step::Continue);
    assert_eq!(stash.held(), 0);
    assert_eq!(stash.base().held(), 0);
    assert_eq!(stash.base().phase(), Door::Open);
    assert_eq!(stash.base().state(), &[2, 3]);
}

#[test]
fn a_zero_limit_still_delivers_and_releases_the_inner_machine() {
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Parcel>>>::global(MailAddr(31));
    let machine = Machine::<MailAddr, _, _, _, _>::new(Vec::new(), Door::Closed, receive);
    let mut stash = Stash::new(machine, 0, refused, |_, parcel| match parcel {
        Parcel::Arrived(number) if number % 2 == 1 => StashRoute::Stash,
        Parcel::Arrived(_) => StashRoute::Deliver,
        Parcel::Released => StashRoute::Release,
    });
    let initialized = initialize(&mut stash).expect("the zero-limit composition starts normally");
    let StashSends { actor, refusals } = initialized.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    let overflow = delegate_transition(
        &mut stash,
        UserEvent::user(MailAddr(13), Parcel::Arrived(3)),
    )
    .expect("the zero-limit stash actually uses its refusal recipient");
    let StashSends {
        actor,
        mut refusals,
    } = overflow.sends;
    assert!(actor.is_empty());
    assert!(overflow.creates.is_empty());
    assert_eq!(overflow.become_, Step::Continue);
    assert_eq!(refusals.len(), 1);
    let refused_input = refusals.pop().expect("the original parcel is returned");
    assert_eq!(refused_input.to, refused);
    assert_eq!(refused_input.message.from, MailAddr(13));
    assert_eq!(refused_input.message.message, Parcel::Arrived(3));
    assert_eq!(stash.held(), 0);
    assert_eq!(stash.base().held(), 0);
    assert_eq!(stash.base().phase(), Door::Closed);
    assert!(stash.base().state().is_empty());
    for (parcel, phase, held, stored) in [
        (Parcel::Arrived(2), Door::Closed, 1, Vec::new()),
        (Parcel::Released, Door::Open, 0, vec![2]),
        (Parcel::Arrived(4), Door::Open, 0, vec![2, 4]),
    ] {
        let actions = delegate_transition(&mut stash, UserEvent::user(MailAddr(7), parcel))
            .expect("zero limits holding, not meaningful delivery or release");
        let StashSends { actor, refusals } = actions.sends;
        assert!(actor.is_empty());
        assert!(refusals.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
        assert_eq!(stash.held(), 0);
        assert_eq!(stash.base().held(), held);
        assert_eq!(stash.base().phase(), phase);
        assert_eq!(stash.base().state(), &stored);
    }
}

#[test]
fn shutdown_outside_a_full_stash_preserves_its_held_input() {
    let paid = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(29));
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Invoice>>>::global(MailAddr(31));
    let mut root = StopOnShutdown::new(Stash::new(
        MessageAdapter::new(paid, number),
        1,
        refused,
        |_, _| StashRoute::Stash,
    ));
    let receipt = Box::new([11, 13]);
    let allocation = receipt.as_ptr();
    let held = delegate_transition(
        &mut root,
        UserEvent::user(MailAddr(7), Invoice { number: 3, receipt }),
    )
    .expect("the finite queue owns one input");
    let SendLayer {
        owned: NoSends,
        inner: StashSends { actor, refusals },
    } = held.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(held.creates.is_empty());
    assert_eq!(held.become_, Step::Continue);
    let stopped = delegate_transition(&mut root, EventLayer::Owned(ShutdownRequested))
        .expect("shutdown retains a full queue");
    let SendLayer {
        owned: NoSends,
        inner: StashSends { actor, refusals },
    } = stopped.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
    let mut stash = root.into_inner();
    let mut held = stash.take();
    assert_eq!(held.len(), 1);
    let input = held
        .pop_front()
        .expect("the original input survives shutdown");
    assert_eq!(input.from, MailAddr(7));
    assert_eq!(input.message.number, 3);
    assert_eq!(input.message.receipt.as_ptr(), allocation);
    assert_eq!(*input.message.receipt, [11, 13]);
    assert_eq!(stash.held(), 0);
}

#[test]
fn shutdown_inside_a_full_stash_bypasses_its_user_limit() {
    let paid = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(29));
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Invoice>>>::global(MailAddr(31));
    let mut stash = Stash::new(
        StopOnShutdown::new(MessageAdapter::new(paid, number)),
        1,
        refused,
        |_, _| StashRoute::Stash,
    );
    let receipt = Box::new([17, 19]);
    let allocation = receipt.as_ptr();
    let held = delegate_transition(
        &mut stash,
        UserEvent::user(MailAddr(13), Invoice { number: 5, receipt }),
    )
    .expect("the finite queue owns one input");
    let StashSends {
        actor: SendLayer {
            owned: NoSends,
            inner: actor,
        },
        refusals,
    } = held.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(held.creates.is_empty());
    assert_eq!(held.become_, Step::Continue);
    let stopped = delegate_transition(&mut stash, EventLayer::Owned(ShutdownRequested))
        .expect("non-user shutdown reaches the inner policy at capacity");
    let StashSends {
        actor: SendLayer {
            owned: NoSends,
            inner: actor,
        },
        refusals,
    } = stopped.sends;
    assert!(actor.is_empty());
    assert!(refusals.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
    let mut held = stash.take();
    assert_eq!(held.len(), 1);
    let input = held
        .pop_front()
        .expect("the original sender and allocation survive");
    assert_eq!(input.from, MailAddr(13));
    assert_eq!(input.message.number, 5);
    assert_eq!(input.message.receipt.as_ptr(), allocation);
    assert_eq!(*input.message.receipt, [17, 19]);
    assert_eq!(stash.held(), 0);
}

#[test]
fn a_timeout_outside_a_full_stash_can_send_to_the_existing_actor_lane() {
    let paid = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(29));
    let refused =
        Recipient::<MessageProtocol<MailAddr, User<MailAddr, Invoice>>>::global(MailAddr(31));
    let stash = Stash::new(MessageAdapter::new(paid, number), 1, refused, |_, invoice| {
        if invoice.number == 0 {
            StashRoute::Release
        } else {
            StashRoute::Stash
        }
    });
    let delay = Duration::from_secs(3);
    let mut timeout = ReceiveTimeout::new(stash, delay, |stash| {
        Actions::cont().with_send(Delivery::new(*stash.base().destination(), 99))
    });
    let started = initialize(&mut timeout).expect("the timeout initializes around the stash");
    assert_eq!(started.sends.owned.len(), 1);
    assert_eq!(started.sends.owned[0].after, delay);
    assert!(started.sends.inner.actor.is_empty());
    assert!(started.sends.inner.refusals.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);
    let mut generation = started.sends.owned[0].generation;
    for (from, value) in [(MailAddr(7), 7), (MailAddr(13), 9)] {
        let receipt = Box::new([value, value + 1]);
        let allocation = receipt.as_ptr();
        let actions = delegate_transition(
            &mut timeout,
            EventLayer::Inner(User::user(from, Invoice { number: value, receipt })),
        ).expect("holding and refusal both preserve the outer timeout");
        assert_eq!(actions.sends.owned.len(), 1);
        assert_eq!(actions.sends.owned[0].after, delay);
        assert_ne!(actions.sends.owned[0].generation, generation);
        generation = actions.sends.owned[0].generation;
        assert!(actions.sends.inner.actor.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
        if value == 7 {
            assert!(actions.sends.inner.refusals.is_empty());
        } else {
            let mut refusals = actions.sends.inner.refusals;
            assert_eq!(refusals.len(), 1);
            let refusal = refusals.pop().expect("one complete original refusal");
            assert_eq!(refusal.to, refused);
            assert_eq!(refusal.message.from, from);
            assert_eq!(refusal.message.message.number, value);
            assert_eq!(refusal.message.message.receipt.as_ptr(), allocation);
            assert_eq!(*refusal.message.message.receipt, [value, value + 1]);
        }
        assert_eq!(timeout.stashed_messages(), 1);
    }
    for expected in [vec![99], vec![]] {
        let actions = delegate_transition(
            &mut timeout,
            EventLayer::Owned(OwnedTimerElapsed::new(generation)),
        ).expect("only the first matching elapsed may emit a reminder");
        assert!(actions.sends.owned.is_empty());
        assert_eq!(actions.sends.inner.actor.len(), expected.len());
        for (delivery, value) in actions.sends.inner.actor.into_iter().zip(expected) {
            assert_eq!(delivery.to, paid);
            assert_eq!(delivery.message, value);
        }
        assert!(actions.sends.inner.refusals.is_empty());
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
        assert_eq!(timeout.stashed_messages(), 1);
        assert_eq!(timeout.base().destination(), &paid);
    }
    let released = delegate_transition(
        &mut timeout,
        EventLayer::Inner(User::user(MailAddr(19), Invoice {
            number: 0,
            receipt: Box::new([0, 1]),
        })),
    ).expect("the reminder leaves the held input available for replay");
    assert_eq!(released.sends.owned.len(), 1);
    assert_eq!(released.sends.owned[0].after, delay);
    assert_eq!(released.sends.inner.actor.len(), 2);
    for (delivery, value) in released.sends.inner.actor.into_iter().zip([0, 7]) {
        assert_eq!(delivery.to, paid);
        assert_eq!(delivery.message, value);
    }
    assert!(released.sends.inner.refusals.is_empty());
    assert!(released.creates.is_empty());
    assert_eq!(released.become_, Step::Continue);
    assert_eq!(timeout.stashed_messages(), 0);
}
