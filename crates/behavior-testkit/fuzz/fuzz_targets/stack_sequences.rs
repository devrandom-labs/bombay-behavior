#![no_main]

//! Deadline over Watch over Stash under coverage-guided byte sequences. Each
//! byte selects the user, peer, or timer lane. Per-lane reference models assert
//! the stash filter, peer-death verdict, and one-shot deadline without an atomic
//! actor wrapper.

use behavior::EventLayer;
use behavior::{
    Actions, Activate, Behavior, BehaviorActed, BehaviorBase, Crash, Delivery, MailAddr, Never,
    NoBirths, PeerStopped, Recipient, StashRoute, Step, TimerElapsed, TimerGeneration, TimerId,
    User, UserEvent, stop_on_abnormal_death,
};
use libfuzzer_sys::fuzz_target;
use std::time::Instant;

#[derive(Default)]
struct EchoingParent {
    seen: Vec<u64>,
}

impl behavior::Protocol for EchoingParent {
    type Addr = MailAddr;
    type Msg = u64;
}

impl BehaviorBase for EchoingParent {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for EchoingParent {
    type Protocol = Self;
    type Event = User<MailAddr, u64>;
    type Sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u64>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        self.seen.push(event.message);
        Ok(Actions::cont().with_send(Delivery::new(Recipient::global(MailAddr(0)), event.message)))
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
    let due = Instant::now() + std::time::Duration::from_secs(1);
    let peer = MailAddr(44);
    let behavior = behavior::Deadline::new(
        behavior::Watch::new(
            behavior::Stash::new(EchoingParent::default(), route),
            peer,
            stop_on_abnormal_death,
        ),
        behavior::TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;

    for (index, byte) in bytes.iter().copied().enumerate() {
        match byte % 3 {
            0 => {
                let arg = u64::try_from(index).unwrap();
                let actions = behavior
                    .transition(EventLayer::Inner(EventLayer::Inner(UserEvent::user(
                        MailAddr(9),
                        arg,
                    ))))
                    .unwrap();
                let echo_step: Vec<u64> = actions
                    .sends
                    .inner
                    .inner
                    .iter()
                    .map(|delivery| delivery.message)
                    .collect();
                let expected = match arg % 3 {
                    2 => Vec::new(),
                    _ => vec![arg],
                };
                assert_eq!(echo_step, expected, "echo lane mismatch at byte {index}");
                assert_eq!(actions.become_, Step::Continue);
            }
            1 => {
                let actions = behavior
                    .transition(EventLayer::Inner(EventLayer::Owned(PeerStopped {
                        peer,
                        outcome: Err(Crash::Failed),
                    })))
                    .unwrap();
                assert!(
                    matches!(actions.become_, Step::Stop(behavior::Stopped)),
                    "peer death verdict at byte {index}"
                );
            }
            _ => {
                let actions = behavior
                    .transition(EventLayer::Owned(TimerElapsed {
                        id: TimerId(0),
                        generation: TimerGeneration(0),
                    }))
                    .unwrap();
                assert_eq!(
                    actions.become_,
                    Step::Continue,
                    "time verdict at byte {index}"
                );
            }
        }
    }
});
