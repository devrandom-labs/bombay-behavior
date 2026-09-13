//! Generated mailbox sequences for the pure testkit Driver.

use behavior::{Acted, Actions, Creations, Delivery, MailAddr, Never, Recipient, Step, User};
use behavior_testkit::{Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::{ProptestConfig, any};
use proptest::{prop_assert_eq, proptest};

struct Echo;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Echo {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        let next = match message {
            u8::MAX => Step::Stop(behavior::Stopped),
            _ => Step::Continue,
        };
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), message)],
            Creations::empty(),
            next,
        ))
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn driver_stops_at_first_requested_stop_and_preserves_prefix(
        messages in vec(any::<u8>(), 0..256)
    ) {
        let events = messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let sender = u64::try_from(index).expect("mailbox index fits in an address");
                User::new(MailAddr(sender), *message)
            });
        let mut mailbox = Mailbox::new(events);
        let trace = drive(Echo, &mut mailbox).unwrap();
        let delivered = messages
            .iter()
            .position(|message| *message == u8::MAX)
            .map_or(messages.len(), |index| index + 1);

        prop_assert_eq!(trace.sends.len(), delivered);
        prop_assert_eq!(trace.pending, messages.len() - delivered);
        for (index, delivery) in trace.sends.iter().enumerate() {
            let sender = u64::try_from(index).expect("delivery index fits in an address");
            prop_assert_eq!(delivery.message, messages[index]);
            prop_assert_eq!(delivery.to.address(), MailAddr(sender));
        }
    }
}
