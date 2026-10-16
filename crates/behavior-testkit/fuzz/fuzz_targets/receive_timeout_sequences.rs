#![no_main]

//! Compare complete receive-timeout turns with the independent idle-period
//! model. Inner deadline traffic is observable but never counts as user activity.

use std::time::{Duration, Instant};

use behavior_actors::{
    Activate, Deadline, OwnedScheduleAfter, OwnedScheduleAt, OwnedTimerElapsed, ReceiveTimeout,
    TimerGeneration,
};
use behavior_core::{
    Acted, Actions, Behavior, Delivery, EventLayer, MailAddr, Never, NoBirths, Recipient, Step,
    User, UserEvent,
};
use bombay_behavior_fuzz::TestRecipient;
use libfuzzer_sys::fuzz_target;
use model::InactivityModel;

#[path = "../../src/model.rs"]
mod model;

#[derive(Default)]
struct Recorder {
    messages: Vec<u8>,
    deadlines: usize,
}

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<TestRecipient<u8>>>, births = NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<TestRecipient<u8>>>, NoBirths, Never> {
        self.messages.push(message);
        Ok(Actions::cont().with_send(Delivery::new(Recipient::global(MailAddr(90)), message)))
    }
}

fn deadline(inner: &mut Recorder) -> behavior_core::Become {
    inner.deadlines += 1;
    Step::Continue
}

fn elapsed(
    _inner: &mut Deadline<Recorder>,
) -> Actions<MailAddr, Never, <Deadline<Recorder> as Behavior>::Sends, NoBirths> {
    Actions::cont().with_send(Delivery::new(Recipient::global(MailAddr(91)), 99))
}

fuzz_target!(|bytes: &[u8]| {
    let due = Instant::now();
    let after = Duration::from_nanos(1);
    let mut model = InactivityModel::new();
    let first = model.initialize();
    let initialized = ReceiveTimeout::new(
        Deadline::new(Recorder::default(), Some(due), deadline),
        after,
        elapsed,
    )
    .initialize()
    .unwrap();
    let initial = initialized.actions;
    let mut behavior = initialized.behavior;
    assert_eq!(
        initial.sends.owned.as_slice(),
        [OwnedScheduleAfter::new(TimerGeneration(first), after)]
    );
    assert_eq!(
        initial.sends.inner.owned.as_slice(),
        [OwnedScheduleAt::new(TimerGeneration(0), due)]
    );
    assert!(initial.sends.inner.inner.is_empty());
    assert!(initial.creates.is_empty());
    assert_eq!(initial.become_, Step::Continue);
    assert!(behavior.base().messages.is_empty());
    assert_eq!(behavior.base().deadlines, 0);

    let mut messages = Vec::new();
    let mut pending_deadline = Some(0);
    let mut deadlines = 0;
    for byte in bytes.iter().copied() {
        let (actions, schedule, notice) = match byte % 4 {
            0 => {
                let Some(next) = model.activity() else {
                    break;
                };
                messages.push(byte);
                let actions = behavior
                    .transition(EventLayer::Inner(EventLayer::Inner(User::user(
                        MailAddr(1),
                        byte,
                    ))))
                    .unwrap();
                (
                    actions,
                    Some(OwnedScheduleAfter::new(TimerGeneration(next), after)),
                    Some((MailAddr(90), byte)),
                )
            }
            1 | 2 => {
                let token = match byte % 4 {
                    1 => u64::from(byte / 4),
                    _ => model.no_activity().unwrap_or(0),
                };
                let notice = model.notification(token).map(|_| (MailAddr(91), 99));
                let actions = behavior
                    .transition(EventLayer::Owned(OwnedTimerElapsed::new(TimerGeneration(
                        token,
                    ))))
                    .unwrap();
                (actions, None, notice)
            }
            _ => {
                let token = u64::from(byte / 4);
                if pending_deadline == Some(token) {
                    pending_deadline = None;
                    deadlines += 1;
                }
                let actions = behavior
                    .transition(EventLayer::Inner(EventLayer::Owned(
                        OwnedTimerElapsed::new(TimerGeneration(token)),
                    )))
                    .unwrap();
                (actions, None, None)
            }
        };
        let schedules: Vec<_> = schedule.into_iter().collect();
        let notices: Vec<_> = notice.into_iter().collect();
        let actual: Vec<_> = actions
            .sends
            .inner
            .inner
            .iter()
            .map(|delivery| (delivery.to.address(), delivery.message))
            .collect();
        assert_eq!(actions.sends.owned.as_slice(), schedules);
        assert!(actions.sends.inner.owned.is_empty());
        assert_eq!(actual, notices);
        assert!(actions.creates.is_empty());
        assert_eq!(actions.become_, Step::Continue);
        assert_eq!(behavior.base().messages, messages);
        assert_eq!(behavior.base().deadlines, deadlines);
    }
});
