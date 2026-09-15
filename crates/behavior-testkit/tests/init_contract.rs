//! Initialization precedes ordinary actor input.

use std::time::{Duration, Instant};

use behavior_actors::Activate;

use behavior_core::{Acted, Actions, Delivery, MailAddr, Never, Recipient, Step};
use proptest::collection::vec;
use proptest::prelude::ProptestConfig;
use proptest::{prop_assert_eq, proptest};

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior_core::NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior_core::NoBirths,
        Never,
    > {
        self.seen.push((from, message));
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            message,
        )]))
    }
}

#[test]
fn deadline_initialization_emits_exactly_one_schedule() {
    let due = Instant::now() + Duration::from_secs(1);
    let deadline = behavior_actors::Deadline::new(
        Recorder::default(),
        behavior_actors::TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let initialized = deadline.initialize().expect("deadline initializes");
    assert_eq!(initialized.actions.sends.owned.len(), 1);
}

#[test]
fn initialized_behavior_processes_mailbox_events() {
    let peer = MailAddr(44);
    let initialized = Recorder::default()
        .initialize()
        .expect("recorder initializes");
    let mut recorder = initialized.behavior;
    let actions = recorder.receive(peer, 7).expect("recorder accepts input");
    assert_eq!(actions.sends.len(), 1);
    assert_eq!(actions.sends[0].to.address(), peer);
    assert_eq!(actions.sends[0].message, 7);
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
    assert_eq!(recorder.seen, [(peer, 7)]);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn deadline_initialization_preserves_each_due_time(
        offsets in vec(0_u64..1_000_000, 1..32)
    ) {
        let origin = Instant::now();

        for offset in offsets {
            let due = origin + Duration::from_nanos(offset);
            let initialized = behavior_actors::Deadline::new(
                Recorder::default(),
                behavior_actors::TimerId(0),
                Some(due),
                |_| Step::Continue,
            )
            .initialize()
            .unwrap();

            prop_assert_eq!(initialized.actions.sends.owned.len(), 1);
            prop_assert_eq!(initialized.actions.sends.owned[0].at, due);
        }
    }
}
