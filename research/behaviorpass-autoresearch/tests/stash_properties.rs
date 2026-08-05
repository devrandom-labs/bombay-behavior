//! Stash filter-model properties: with a pure route function the stash
//! composition is a filter — delivered messages are exactly those the route
//! admits (Deliver or Release trigger), in arrival order with origins
//! intact; Stash-routed messages are held, never lost, never duplicated,
//! across any number of release events.

use behaviorpass::{
    Acted, Actions, Behavior, Delivery, MailAddr, Never, Recipient, Spec, StashRoute, State, Step,
    User, UserEvent,
};
use behaviorpass_autoresearch::Mailbox;
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

impl State<u8, Never, Never> for Recorder {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "the stash API routes through fn(&Msg)")]
fn route(message: &u8) -> StashRoute {
    match message {
        7 => StashRoute::Release,
        even if even % 2 == 0 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn stash_is_an_exact_filter_without_loss_or_duplication(
        messages in vec((any::<u64>(), any::<u8>()), 0..256),
        releases in vec(any::<u8>(), 0..32),
    ) {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let mut behavior = Spec::new(Recorder::default()).stash(route);

        for (from, message) in &messages {
            runtime.block_on(behavior.step(UserEvent::user(MailAddr(*from), *message))).unwrap();
        }
        for _ in &releases {
            runtime.block_on(behavior.step(UserEvent::user(MailAddr(99), 7))).unwrap();
        }

        // Independent filter model: delivered == route-admitted messages in
        // arrival order with origins, plus every release trigger (which is
        // itself delivered on arrival); held == the Stash-routed rest.
        let mut expected: Vec<(MailAddr, u8)> = messages
            .iter()
            .filter(|(_, message)| route(message) != StashRoute::Stash)
            .map(|(from, message)| (MailAddr(*from), *message))
            .collect();
        expected.extend(releases.iter().map(|_| (MailAddr(99), 7)));
        prop_assert_eq!(
            &behavior.behavior().inner().state().seen,
            &expected,
            "delivered set differs from the route filter"
        );
        let expected_held = messages
            .iter()
            .filter(|(_, message)| route(message) == StashRoute::Stash)
            .count();
        prop_assert_eq!(
            behavior.behavior().held(),
            expected_held,
            "held count differs: loss or duplication"
        );
    }
}

/// Mailbox-driven variant: the same filter property through the driver,
/// including the unconsumed-tail accounting after a stop.
#[tokio::test]
async fn stash_filter_holds_through_the_driver() {
    let events = [
        User::user(MailAddr(1), 2),  // Stash
        User::user(MailAddr(2), 3),  // Deliver
        User::user(MailAddr(3), 7),  // Release trigger — delivered
        User::user(MailAddr(4), 4),  // Stash
    ];
    let mut mailbox = Mailbox::new(events);
    let mut behavior = Spec::new(Recorder::default()).stash(route);
    let trace = behaviorpass_autoresearch::drive(&mut behavior, &mut mailbox).await.unwrap();

    assert_eq!(
        behavior.behavior().inner().state().seen,
        [(MailAddr(2), 3), (MailAddr(3), 7)]
    );
    assert_eq!(behavior.behavior().held(), 2);
    assert_eq!(trace.pending, 0);
    assert_eq!(trace.transitions, 5);
}
