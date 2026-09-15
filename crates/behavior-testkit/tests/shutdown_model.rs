use behavior_actors::{ShutdownEvent, ShutdownRequested};
use behavior_core::{Acted, Actions, Delivery, MailAddr, Never, NoBirths, Recipient, User};
use behavior_testkit::{DriveDisposition, Mailbox, drive};
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;

struct Echo;

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = NoBirths, error = Never)]
impl Echo {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, NoBirths, Never>
    {
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            message,
        )]))
    }
}

#[derive(Clone, Copy, Debug)]
enum Input {
    Shutdown,
    Message(u8),
}

fn input() -> impl Strategy<Value = Input> {
    prop_oneof![Just(Input::Shutdown), any::<u8>().prop_map(Input::Message)]
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn shutdown_matches_a_first_request_prefix_model(
        inputs in vec(input(), 0..256)
    ) {
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let events = inputs.iter().enumerate().map(|(index, input)| match input {
            Input::Shutdown => ShutdownEvent::Owned(ShutdownRequested),
            Input::Message(message) => ShutdownEvent::Inner(User {
                    from: MailAddr(u64::try_from(index).unwrap()),
                    message: *message,
                }),
        });
        let mut mailbox = Mailbox::new(events);
        let behavior = behavior_actors::StopOnShutdown::new(Echo);
        let trace = drive(behavior, &mut mailbox).unwrap();
        let stop = inputs
            .iter()
            .position(|input| matches!(input, Input::Shutdown));
        let consumed = stop.map_or(inputs.len(), |index| index + 1);
        let expected_messages: Vec<_> = inputs
            .iter()
            .take(stop.unwrap_or(inputs.len()))
            .filter_map(|input| match input {
                Input::Shutdown => None,
                Input::Message(message) => Some(*message),
            })
            .collect();

        prop_assert_eq!(trace.pending, inputs.len() - consumed);
        prop_assert_eq!(trace.transitions, consumed + 1);
        prop_assert_eq!(trace.sends.inner.len(), expected_messages.len());
        prop_assert_eq!(
            trace
                .sends
                .inner
                .iter()
                .map(|delivery| delivery.message)
                .collect::<Vec<_>>(),
            expected_messages
        );
        let expected_disposition = match stop {
            Some(_) => DriveDisposition::BehaviorStopped(behavior_core::Stopped),
            None => DriveDisposition::MailboxDrained,
        };
        prop_assert_eq!(trace.disposition, expected_disposition);
    }
}
