use behavior::{
    Acted, Actions, Compose, Delivery, MailAddr, Never, NoBirths, Recipient, ShutdownProtocol,
    ShutdownRequested, User,
};
use behavior_testkit::{Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;

struct Echo;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = NoBirths, error = Never)]
impl Echo {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, NoBirths, Never>
    {
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: behavior::Step::Continue,
        })
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn shutdown_matches_a_first_request_prefix_model(
        inputs in vec((any::<bool>(), any::<u8>()), 0..256)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let events = inputs.iter().enumerate().map(|(index, (shutdown, message))| {
            if *shutdown {
                ShutdownProtocol::ShutdownRequested(ShutdownRequested)
            } else {
                ShutdownProtocol::Behavior(User {
                    from: MailAddr(u64::try_from(index).unwrap()),
                    message: *message,
                })
            }
        });
        let mut mailbox = Mailbox::new(events);
        let behavior = Compose::new(Echo).stop_on_shutdown();
        let trace = drive(behavior, &mut mailbox).unwrap();
        let stop = inputs.iter().position(|(shutdown, _)| *shutdown);
        let consumed = stop.map_or(inputs.len(), |index| index + 1);
        let expected_messages: Vec<_> = inputs
            .iter()
            .take(stop.unwrap_or(inputs.len()))
            .filter_map(|(shutdown, message)| (!shutdown).then_some(*message))
            .collect();

        prop_assert_eq!(trace.pending, inputs.len() - consumed);
        prop_assert_eq!(trace.transitions, consumed + 1);
        prop_assert_eq!(trace.sends.len(), expected_messages.len());
        prop_assert_eq!(
            trace.sends.iter().map(|delivery| delivery.message).collect::<Vec<_>>(),
            expected_messages
        );
        prop_assert_eq!(trace.stopped, stop.is_some());
    }
}
