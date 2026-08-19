//! Initialization is a consuming typestate transition. Definitions can only
//! initialize; active behaviors can only process events.

use std::time::Duration;

use behavior::{Acted, Actions, Activate, Delivery, MailAddr, Never, Recipient, Step};
use std::time::Instant;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Recorder {
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
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

type Child = Recorder;

fn child(_index: usize) -> Child {
    Recorder::default()
}

#[tokio::test]
async fn deadline_initialization_emits_exactly_one_schedule() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior =
        behavior::Deadline::new(Recorder::default(), behavior::TimerId(0), Some(due), |_| {
            Ok(Step::Continue)
        });
    let initialized = behavior.initialize().unwrap();
    assert_eq!(initialized.actions.sends.owned.len(), 1);
}

#[tokio::test]
async fn initialized_behavior_processes_mailbox_events() {
    let peer = MailAddr(44);
    let initialized = (Recorder::default()).initialize().unwrap();
    let mut behavior = initialized.behavior;
    behavior.receive(peer, 7).unwrap();
    assert_eq!(behavior.seen, [(peer, 7)]);
}

#[tokio::test]
async fn supervisor_initialization_emits_the_configured_fleet_once() {
    let behavior = behavior::Supervisor::new(
        behavior::ChildTopology::new((0..2).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
        ),
    )
    .unwrap();
    let initialized = behavior.initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 2);
    assert_eq!(initialized.actions.sends.child_observations.len(), 2);
}
