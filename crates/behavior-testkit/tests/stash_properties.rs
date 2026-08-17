//! Stash filter-model properties: with a pure route function the stash
//! composition is a filter — delivered messages are exactly those the route
//! admits (Deliver or Release trigger), in arrival order with origins
//! intact; Stash-routed messages are held, never lost, never duplicated,
//! across any number of release events.

use behavior::{
    Acted, Actions, Activate, Behavior, Delivery, MailAddr, Never, Recipient, StashRoute, Step,
    User, UserEvent,
};
use behavior_testkit::Mailbox;
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;

struct Sink;

impl behavior::Protocol for Sink {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Sink {
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = behavior::NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<Sink>>, births = behavior::NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Sink>>, behavior::NoBirths, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "the stash API routes through fn(&Msg)"
)]
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
        let _runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let behavior = behavior::Stash::new(Recorder::default(), route);
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;

        for (from, message) in &messages {
            behavior.transition(UserEvent::user(MailAddr(*from), *message)).unwrap();
        }
        for _ in &releases {
            behavior.transition(UserEvent::user(MailAddr(99), 7)).unwrap();
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
            &behavior.base().seen,
            &expected,
            "delivered set differs from the route filter"
        );
        let expected_held = messages
            .iter()
            .filter(|(_, message)| route(message) == StashRoute::Stash)
            .count();
        prop_assert_eq!(
            behavior.held(),
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
        User::user(MailAddr(1), 2), // Stash
        User::user(MailAddr(2), 3), // Deliver
        User::user(MailAddr(3), 7), // Release trigger — delivered
        User::user(MailAddr(4), 4), // Stash
    ];
    let mut mailbox = Mailbox::new(events);
    let behavior = behavior::Stash::new(Recorder::default(), route);
    let trace = behavior_testkit::drive(behavior, &mut mailbox).unwrap();

    assert_eq!(
        trace.behavior.base().seen,
        [(MailAddr(2), 3), (MailAddr(3), 7)]
    );
    assert_eq!(trace.behavior.held(), 2);
    assert_eq!(trace.pending, 0);
    assert_eq!(trace.transitions, 5);
}

/// Exhaustive small enumeration: every sequence of up to four messages over
/// the three route classes (0=Release, 1=Deliver, 2=Stash), with occurrence
/// ids unique by construction (`id = index * 3 + residue`), checked against
/// the filter model.
#[test]
#[allow(
    clippy::items_after_statements,
    clippy::trivially_copy_pass_by_ref,
    reason = "standalone test after proptest! block; stash routes through fn(&Msg)"
)]
fn stash_exhaustive_sequences_match_the_filter_model() {
    fn residue_route(message: &u8) -> StashRoute {
        match message % 3 {
            0 => StashRoute::Release,
            1 => StashRoute::Deliver,
            _ => StashRoute::Stash,
        }
    }

    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    const ALPHABET: usize = 3;
    const MAX_LENGTH: usize = 4;

    let mut checked = 0_usize;
    let mut length = 0_usize;
    while length <= MAX_LENGTH {
        let total = ALPHABET.pow(u32::try_from(length).unwrap());
        for code in 0..total {
            let behavior = behavior::Stash::new(Recorder::default(), residue_route);
            let initialized = behavior.initialize().unwrap();
            let mut behavior = initialized.behavior;
            let mut residues = Vec::with_capacity(length);
            let mut rest = code;
            for _ in 0..length {
                residues.push(rest % ALPHABET);
                rest /= ALPHABET;
            }
            for (index, residue) in residues.iter().enumerate() {
                let message = u8::try_from(index * ALPHABET + *residue).unwrap();
                runtime
                    .block_on(async { behavior.transition(UserEvent::user(MailAddr(1), message)) })
                    .unwrap();
            }
            // Expected: route-admitted messages (Release/Deliver) in arrival
            // order with origins; held == Stash-routed count.
            let mut expected = Vec::new();
            for (index, residue) in residues.iter().enumerate() {
                if *residue != 2 {
                    let message = u8::try_from(index * ALPHABET + residue).unwrap();
                    expected.push((MailAddr(1), message));
                }
            }
            assert_eq!(behavior.base().seen, expected, "sequence {residues:?}");
            assert_eq!(
                behavior.held(),
                residues.iter().filter(|r| **r == 2).count(),
                "held mismatch for sequence {residues:?}"
            );
            checked += 1;
        }
        length += 1;
    }
    assert_eq!(checked, 1 + 3 + 9 + 27 + 81);
}

/// A stopping inner state: records, emits, and stops (Normal) on 9.
#[derive(Default)]
struct StopRecorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<Sink>>, births = behavior::NoBirths, error = Never)]
impl StopRecorder {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<Sink>>, behavior::NoBirths, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: if message == 9 {
                Step::Stop(behavior::Stopped)
            } else {
                Step::Continue
            },
        })
    }
}

/// The stash filter interacting with a stopping inner: delivered == the
/// route-admitted prefix up to and including the first stopping message,
/// held == the stash-routed count within that prefix, and the fold stops.
#[test]
fn stash_filter_with_a_stopping_inner_matches_the_prefix_model() {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    let behavior = behavior::Stash::new(StopRecorder::default(), route);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let mut stopped = false;
    for (index, message) in (0_u8..40).enumerate() {
        if stopped {
            break;
        }
        let from = MailAddr(u64::try_from(index).unwrap());
        let actions = runtime
            .block_on(async { behavior.transition(UserEvent::user(from, message)) })
            .unwrap();
        if matches!(actions.become_, Step::Stop(behavior::Stopped)) {
            stopped = true;
        }
    }
    assert!(stopped);

    // Prefix model: delivered == route-admitted messages in order up to and
    // including 9; held == stash-routed count in the prefix.
    let mut expected = Vec::new();
    let mut expected_held = 0_usize;
    for message in 0_u8..=9 {
        if route(&message) == StashRoute::Stash {
            expected_held += 1;
        } else {
            expected.push((MailAddr(u64::from(message)), message));
        }
    }
    assert_eq!(behavior.base().seen, expected);
    assert_eq!(behavior.held(), expected_held);
}
