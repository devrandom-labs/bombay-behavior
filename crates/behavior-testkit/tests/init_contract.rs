//! Initialization is a consuming typestate transition. Definitions can only
//! initialize; active behaviors can only process events.

use std::time::Duration;

use behavior::{Acted, Actions, Compose, Delivery, MailAddr, Never, Recipient, Step};
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

/// A quiet parent that births nothing at init.
struct Parent;

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl Parent {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
        Ok(Actions::cont())
    }
}

#[tokio::test]
async fn deadline_initialization_emits_exactly_one_schedule() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior =
        Compose::new(Recorder::default())
            .deadline(behavior::TimerId(0), Some(due), |_| Ok(Step::Continue));
    let initialized = behavior.initialize().unwrap();
    assert_eq!(initialized.actions.sends.schedules.len(), 1);
}

#[tokio::test]
async fn initialized_behavior_processes_mailbox_events() {
    let peer = MailAddr(44);
    let initialized = Compose::new(Recorder::default()).initialize().unwrap();
    let mut behavior = initialized.behavior;
    behavior.receive(peer, 7).unwrap();
    assert_eq!(behavior.seen, [(peer, 7)]);
}

#[tokio::test]
async fn supervisor_initialization_emits_the_configured_fleet_once() {
    let behavior = Compose::new(Parent)
        .children(
            |index| u64::try_from(index).unwrap(),
            2,
            |index| Some(child(index)),
        )
        .unwrap();
    let initialized = behavior.initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 2);
    assert_eq!(initialized.actions.sends.child_observations.len(), 2);
}
