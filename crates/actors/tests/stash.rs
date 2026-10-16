use std::panic::{AssertUnwindSafe, catch_unwind};

use behavior::{
    BehaviorBase as _, EventLayer, MailAddr, MessageProtocol, Never, NoSends, Recipient, SendLayer,
    Step, Stopped, UserEvent, delegate_transition, initialize,
};
use behavior_actors::{
    Machine, MessageAdapter, Move, ShutdownRequested, StashRoute, StopOnShutdown, UnlimitedStash,
};

fn cents(dollars: u64) -> u64 {
    dollars * 100
}

#[test]
fn shutdown_outside_stash_returns_the_original_fifo_once() {
    let destination = Recipient::<MessageProtocol<MailAddr, u64>>::global(MailAddr(29));
    let mut stash = UnlimitedStash::new(MessageAdapter::new(destination, cents), |_, _| {
        StashRoute::Stash
    });
    let empty = stash.take();
    assert!(empty.is_empty());
    let mut root = StopOnShutdown::new(stash);
    let initialized = initialize(&mut root).expect("the adapter starts infallibly");
    let SendLayer {
        owned: NoSends,
        inner: sends,
    } = initialized.sends;
    assert!(sends.is_empty());
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    for (origin, amount) in [(MailAddr(7), 11), (MailAddr(13), 17)] {
        let held = delegate_transition(&mut root, UserEvent::user(origin, amount))
            .expect("holding does not process the adapter input");
        let SendLayer {
            owned: NoSends,
            inner: sends,
        } = held.sends;
        assert!(sends.is_empty());
        assert!(held.creates.is_empty());
        assert_eq!(held.become_, Step::Continue);
    }
    let stopped = delegate_transition(&mut root, EventLayer::Owned(ShutdownRequested))
        .expect("the outer shutdown policy stops normally");
    let SendLayer {
        owned: NoSends,
        inner: sends,
    } = stopped.sends;
    assert!(sends.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
    let mut stash = root.into_inner();
    let recovered = stash.take();
    assert_eq!(stash.held(), 0);
    let actual: Vec<_> = recovered
        .into_iter()
        .map(|input| (input.from, input.message))
        .collect();
    assert_eq!(actual, [(MailAddr(7), 11), (MailAddr(13), 17)]);
    assert_eq!(stash.base().destination(), &destination);
    let repeated = stash.take();
    assert!(repeated.is_empty());
}

#[test]
fn stash_outside_shutdown_returns_the_original_fifo_once() {
    let destination = Recipient::<MessageProtocol<MailAddr, u64>>::global(MailAddr(31));
    let mut root = UnlimitedStash::new(
        StopOnShutdown::new(MessageAdapter::new(destination, cents)),
        |_, _| StashRoute::Stash,
    );
    let initialized = initialize(&mut root).expect("the adapter starts infallibly");
    let SendLayer {
        owned: NoSends,
        inner: sends,
    } = initialized.sends;
    assert!(sends.is_empty());
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    for (origin, amount) in [(MailAddr(19), 23), (MailAddr(29), 37)] {
        let held = delegate_transition(&mut root, UserEvent::user(origin, amount))
            .expect("holding does not process the adapter input");
        let SendLayer {
            owned: NoSends,
            inner: sends,
        } = held.sends;
        assert!(sends.is_empty());
        assert!(held.creates.is_empty());
        assert_eq!(held.become_, Step::Continue);
    }
    let stopped = delegate_transition(&mut root, EventLayer::Owned(ShutdownRequested))
        .expect("shutdown bypasses the outer stash's user lane");
    let SendLayer {
        owned: NoSends,
        inner: sends,
    } = stopped.sends;
    assert!(sends.is_empty());
    assert!(stopped.creates.is_empty());
    assert_eq!(stopped.become_, Step::Stop(Stopped));
    let recovered = root.take();
    assert_eq!(root.held(), 0);
    let actual: Vec<_> = recovered
        .into_iter()
        .map(|input| (input.from, input.message))
        .collect();
    assert_eq!(actual, [(MailAddr(19), 23), (MailAddr(29), 37)]);
    assert_eq!(root.base().destination(), &destination);
    let repeated = root.take();
    assert!(repeated.is_empty());
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

fn receive_parcel(door: Door, stored: &mut Vec<u64>, parcel: &Parcel) -> Result<Move<Door>, Never> {
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
fn taking_messages_preserves_machine_deferrals_and_the_stash_route() {
    let machine = Machine::<MailAddr, _, _, _, _>::new(Vec::new(), Door::Closed, receive_parcel);
    let mut stash = UnlimitedStash::new(machine, |machine, parcel| match parcel {
        Parcel::Arrived(number) if number % 2 == 1 && machine.phase() == Door::Closed => {
            StashRoute::Stash
        }
        Parcel::Arrived(_) => StashRoute::Deliver,
        Parcel::Released => StashRoute::Release,
    });
    let initialized = initialize(&mut stash).expect("the machine starts infallibly");
    assert_eq!(initialized.sends.as_slice(), []);
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    for (origin, parcel) in [
        (MailAddr(41), Parcel::Arrived(3)),
        (MailAddr(43), Parcel::Arrived(2)),
    ] {
        let actions = delegate_transition(&mut stash, UserEvent::user(origin, parcel))
            .expect("the separate queues accept their respective parcels");
        assert_eq!(actions.sends.as_slice(), []);
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
    }
    assert_eq!(stash.held(), 1);
    assert_eq!(stash.base().held(), 1);
    assert_eq!(stash.base().phase(), Door::Closed);
    assert!(stash.base().state().is_empty());
    let taken = stash.take();
    let actual: Vec<_> = taken
        .into_iter()
        .map(|input| (input.from, input.message))
        .collect();
    assert_eq!(actual, [(MailAddr(41), Parcel::Arrived(3))]);
    assert_eq!(stash.held(), 0);
    assert_eq!(stash.base().held(), 1);
    assert_eq!(stash.base().phase(), Door::Closed);
    assert!(stash.base().state().is_empty());
    let held = delegate_transition(
        &mut stash,
        UserEvent::user(MailAddr(53), Parcel::Arrived(5)),
    )
    .expect("the original route still holds the next selected parcel");
    assert_eq!(held.sends.as_slice(), []);
    assert!(held.creates.is_empty());
    assert_eq!(held.become_, Step::Continue);
    assert_eq!(stash.held(), 1);
    assert_eq!(stash.base().held(), 1);
    assert_eq!(stash.base().phase(), Door::Closed);
    assert!(stash.base().state().is_empty());
    let released = delegate_transition(&mut stash, UserEvent::user(MailAddr(47), Parcel::Released))
        .expect("the original route still releases the inner machine");
    assert_eq!(released.sends.as_slice(), []);
    assert!(released.creates.is_empty());
    assert_eq!(released.become_, Step::Continue);
    assert_eq!(stash.held(), 0);
    assert_eq!(stash.base().held(), 0);
    assert_eq!(stash.base().phase(), Door::Open);
    assert_eq!(stash.base().state(), &[2, 5]);
    let delivered = delegate_transition(
        &mut stash,
        UserEvent::user(MailAddr(59), Parcel::Arrived(7)),
    )
    .expect("the original route now delivers parcels through the open door");
    assert_eq!(delivered.sends.as_slice(), []);
    assert!(delivered.creates.is_empty());
    assert_eq!(delivered.become_, Step::Continue);
    assert_eq!(stash.held(), 0);
    assert_eq!(stash.base().held(), 0);
    assert_eq!(stash.base().state(), &[2, 5, 7]);
    let empty = stash.take();
    assert!(empty.is_empty());
    let repeated = stash.take();
    assert!(repeated.is_empty());
}

#[test]
fn a_replay_selector_panic_preserves_the_reheld_prefix_and_undelegated_suffix() {
    let machine = Machine::<MailAddr, _, _, _, _>::new(Vec::new(), Door::Closed, receive_parcel);
    let mut stash = UnlimitedStash::new(machine, |machine, parcel| match parcel {
        Parcel::Released => StashRoute::Release,
        Parcel::Arrived(1) => StashRoute::Stash,
        Parcel::Arrived(_) if machine.phase() == Door::Closed => StashRoute::Stash,
        Parcel::Arrived(3) => panic!("the borrowed parcel selector failed"),
        Parcel::Arrived(_) => StashRoute::Deliver,
    });
    let initialized = initialize(&mut stash).expect("the machine starts infallibly");
    assert_eq!(initialized.sends.as_slice(), []);
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    for (from, number) in [(7, 1), (11, 2), (13, 3), (17, 4)] {
        let held = delegate_transition(
            &mut stash,
            UserEvent::user(MailAddr(from), Parcel::Arrived(number)),
        )
        .expect("all parcels arrive before the door opens");
        assert_eq!(held.sends.as_slice(), []);
        assert!(held.creates.is_empty());
        assert_eq!(held.become_, Step::Continue);
    }
    assert_eq!(stash.held(), 4);
    let released = catch_unwind(AssertUnwindSafe(|| {
        delegate_transition(&mut stash, UserEvent::user(MailAddr(19), Parcel::Released))
    }));
    let Err(payload) = released else {
        panic!("replay must reach the selected panic");
    };
    let panic_message = payload.downcast_ref::<&str>();
    assert_eq!(panic_message, Some(&"the borrowed parcel selector failed"));
    assert_eq!(stash.base().phase(), Door::Open);
    // The second parcel was delegated before the panic. The first remains
    // held; only the still-undelegated originals belong to this FIFO.
    assert_eq!(stash.base().state(), &[2]);
    assert_eq!(stash.base().held(), 0);
    let held = stash.take();
    let actual: Vec<_> = held
        .into_iter()
        .map(|input| (input.from, input.message))
        .collect();
    assert_eq!(
        actual,
        [
            (MailAddr(7), Parcel::Arrived(1)),
            (MailAddr(13), Parcel::Arrived(3)),
            (MailAddr(17), Parcel::Arrived(4))
        ]
    );
    let repeated = stash.take();
    assert!(repeated.is_empty());
}

#[test]
fn a_replay_actor_panic_preserves_the_reheld_prefix_and_undelegated_suffix() {
    let machine = Machine::<MailAddr, _, _, _, _>::new(
        Vec::new(),
        Door::Closed,
        |door, stored, parcel| match (door, parcel) {
            (Door::Open, Parcel::Arrived(3)) => panic!("the parcel actor failed after delegation"),
            _ => receive_parcel(door, stored, parcel),
        },
    );
    let mut stash = UnlimitedStash::new(machine, |machine, parcel| match parcel {
        Parcel::Released => StashRoute::Release,
        Parcel::Arrived(1) => StashRoute::Stash,
        Parcel::Arrived(_) if machine.phase() == Door::Closed => StashRoute::Stash,
        Parcel::Arrived(_) => StashRoute::Deliver,
    });
    let initialized = initialize(&mut stash).expect("the machine starts infallibly");
    assert_eq!(initialized.sends.as_slice(), []);
    assert!(initialized.creates.is_empty());
    assert_eq!(initialized.become_, Step::Continue);
    for (from, number) in [(7, 1), (11, 2), (13, 3), (17, 4)] {
        let held = delegate_transition(
            &mut stash,
            UserEvent::user(MailAddr(from), Parcel::Arrived(number)),
        )
        .expect("all parcels arrive before the door opens");
        assert_eq!(held.sends.as_slice(), []);
        assert!(held.creates.is_empty());
        assert_eq!(held.become_, Step::Continue);
    }
    assert_eq!(stash.held(), 4);
    let released = catch_unwind(AssertUnwindSafe(|| {
        delegate_transition(&mut stash, UserEvent::user(MailAddr(19), Parcel::Released))
    }));
    let Err(payload) = released else {
        panic!("replay must reach the delegated actor panic");
    };
    let panic_message = payload.downcast_ref::<&str>();
    assert_eq!(
        panic_message,
        Some(&"the parcel actor failed after delegation")
    );
    assert_eq!(stash.base().phase(), Door::Open);
    // The second parcel was delegated before the panic. The first remains
    // held; only the still-undelegated originals belong to this FIFO.
    assert_eq!(stash.base().state(), &[2]);
    assert_eq!(stash.base().held(), 0);
    let held = stash.take();
    let actual: Vec<_> = held
        .into_iter()
        .map(|input| (input.from, input.message))
        .collect();
    assert_eq!(
        actual,
        [
            (MailAddr(7), Parcel::Arrived(1)),
            (MailAddr(17), Parcel::Arrived(4))
        ]
    );
    let repeated = stash.take();
    assert!(repeated.is_empty());
}
