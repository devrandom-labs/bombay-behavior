#![no_main]

use std::time::Duration;

use behavior::{
    Acted, Actions, Activate, MailAddr, Never, NoBirths, Step, TimerElapsed, TimerGeneration,
    TimerId, User, UserEvent,
};
use behavior::EventLayer;
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

struct Sink;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Never>, births = NoBirths, error = Never)]
impl Sink {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

type SinkBehavior = Sink;

fn elapsed(_inner: &mut SinkBehavior) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
    Ok(Actions::cont())
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().build().unwrap();
    runtime.block_on(async {
        let behavior = behavior::ReceiveTimeout::new(
            Sink,
            TimerId(0),
            Duration::from_nanos(1),
            elapsed,
        );
        let initialized = behavior.initialize().unwrap();
        let initial = initialized.actions;
        let mut behavior = initialized.behavior;
        assert_eq!(initial.sends.schedules[0].generation, TimerGeneration(0));
        let mut issued = 0_u64;
        let mut live = Some(0_u64);

        for byte in bytes.iter().copied() {
            match byte % 3 {
                0 => {
                    let Some(next) = issued.checked_add(1) else {
                        break;
                    };
                    let actions = behavior
                        .transition(EventLayer::Inner(User::user(MailAddr(1), byte)))
                        .unwrap();
                    issued = next;
                    live = Some(next);
                    assert_eq!(actions.sends.schedules[0].generation, TimerGeneration(next));
                }
                1 => {
                    let delivered = u64::from(byte / 3);
                    let matched = live == Some(delivered);
                    let actions = behavior
                        .transition(EventLayer::Owned(TimerElapsed {
                            id: TimerId(0),
                            generation: TimerGeneration(delivered),
                        }))
                        .unwrap();
                    if matched {
                        live = None;
                    }
                    assert!(actions.sends.behavior.is_empty());
                    assert!(matches!(actions.become_, Step::Continue));
                }
                _ => {
                    let before = live;
                    let actions = behavior
                        .transition(EventLayer::Owned(TimerElapsed {
                            id: TimerId(1),
                            generation: TimerGeneration(u64::from(byte)),
                        }))
                        .unwrap();
                    assert_eq!(live, before);
                    assert!(actions.sends.behavior.is_empty());
                }
            }
        }
    });
});
