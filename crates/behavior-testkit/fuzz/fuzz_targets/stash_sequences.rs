#![no_main]

//! Byte-selected finite admission, release and recovery. Between takes, only
//! the first N hold-selected originals may remain; later originals move to
//! refusals. All direct inputs produce their exact existing actor effects.
//! This covers entry counts and sender/order custody, not a byte-memory bound
//! or dynamically changing replay eligibility.

use behavior_actors::{StashRoute, StashSends};
use behavior_core::{
    Acted, Actions, BehaviorBase as _, Delivery, MailAddr, Never, Recipient, Step, User, UserEvent,
    delegate_transition, initialize,
};
use libfuzzer_sys::fuzz_target;

#[derive(Default)]
struct Recorder {
    seen: Vec<u64>,
}

#[behavior_core::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u64>>>, births = behavior_core::NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u64>>>,
        behavior_core::NoBirths,
        Never,
    > {
        self.seen.push(message);
        Ok(Actions::cont().with_send(Delivery::new(Recipient::global(MailAddr(0)), message)))
    }
}

fn route(message: &u64) -> StashRoute {
    match message % 3 {
        0 => StashRoute::Release,
        1 => StashRoute::Deliver,
        _ => StashRoute::Stash,
    }
}

fuzz_target!(|bytes: &[u8]| {
    let Some((&capacity, operations)) = bytes.split_first() else {
        return;
    };
    let limit = usize::from(capacity % 17);
    let refused =
        Recipient::<bombay_behavior_fuzz::TestRecipient<User<MailAddr, u64>>>::global(MailAddr(31));
    let mut behavior =
        behavior_actors::Stash::new(Recorder::default(), limit, refused, |_, message| {
            route(message)
        });
    let initialized = initialize(&mut behavior).unwrap();
    assert!(initialized.sends.actor.is_empty());
    assert!(initialized.sends.refusals.is_empty());
    assert!(initialized.creates.is_empty());
    assert!(matches!(initialized.become_, Step::Continue));
    let mut selected = Vec::new();
    let mut delivered = Vec::new();
    for (index, &operation) in operations.iter().chain(core::iter::once(&3)).enumerate() {
        let choice = operation % 4;
        if choice == 3 {
            let originals = behavior.take();
            let expected: Vec<_> = selected.iter().take(limit).collect();
            assert_eq!(originals.len(), expected.len());
            for (original, &&(from, message)) in originals.into_iter().zip(&expected) {
                assert_eq!(original.from, from);
                assert_eq!(original.message, message);
            }
            selected.clear();
            assert_eq!(behavior.held(), 0);
        } else {
            let message = u64::try_from(index).unwrap() * 3 + u64::from(choice);
            let from = MailAddr(u64::from(operation));
            let actions = delegate_transition(&mut behavior, User::user(from, message)).unwrap();
            let StashSends { actor, refusals } = actions.sends;
            let expected = if choice == 2 {
                Vec::new()
            } else {
                delivered.push(message);
                vec![Delivery::new(Recipient::global(MailAddr(0)), message)]
            };
            assert!(actor == expected);
            assert_eq!(behavior.base().seen, delivered);
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
            if choice == 2 {
                selected.push((from, message));
            }
            let overflow = selected
                .iter()
                .skip(limit)
                .filter(|(_, candidate)| *candidate == message)
                .count();
            assert_eq!(refusals.len(), overflow);
            for delivery in refusals {
                assert_eq!(delivery.to, refused);
                assert_eq!(delivery.message.from, from);
                assert_eq!(delivery.message.message, message);
            }
            let expected_held = selected.iter().take(limit).count();
            assert_eq!(behavior.held(), expected_held);
        }
    }
});
