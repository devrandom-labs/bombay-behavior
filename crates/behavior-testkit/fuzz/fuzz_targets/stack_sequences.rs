#![no_main]

//! Capstone: the full four-layer stack (supervision ∘ at ∘ watch ∘ stash)
//! under coverage-guided byte sequences. Each byte selects one of four
//! lanes (user / peer / time / child); per-lane reference models — the
//! stash filter for the echo lane, the watch verdict for peer deaths, the
//! one-shot Deadline for time events, OneForOne/Permanent/unbounded supervision
//! for child deaths — are asserted per byte: effects land in exactly their
//! product lane and never leak across.

use behavior::{
    Acted, Actions, Activate, Crash, Create, Delivery, MailAddr, Never, PeerStopped, Recipient,
    RestartPolicy, StashRoute, Step, Strategy, SupervisionEvent, TimerElapsed, TimerGeneration,
    TimerId, UserEvent, WorkerStopped, stop_on_abnormal_death,
};
use behavior::EventLayer;
use libfuzzer_sys::fuzz_target;
use std::time::Instant;
use tokio::runtime::Builder;

#[derive(Default)]
struct EchoingParent {
    seen: Vec<u64>,
}

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u64>>>, births = behavior::Births<Echo>, error = Never)]
impl EchoingParent {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u64>>>,
        behavior::Births<Echo>,
        Never,
    > {
        self.seen.push(message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
            creates: if message == u64::MAX {
                vec![Create::birth(message, child(0))]
            } else {
                Vec::new()
            },
            become_: Step::Continue,
        })
    }
}

#[derive(Default)]
struct Echo;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Echo {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Echo {
    Echo
}

fn route(message: &u64) -> StashRoute {
    match message % 3 {
        0 => StashRoute::Release,
        1 => StashRoute::Deliver,
        _ => StashRoute::Stash,
    }
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let due = Instant::now() + std::time::Duration::from_secs(1);
        let peer = MailAddr(44);
        let behavior = behavior::Supervise::new(
            behavior::Deadline::new(
                behavior::Watch::new(
                    behavior::Stash::new(EchoingParent::default(), route),
                    peer,
                    stop_on_abnormal_death,
                ),
                behavior::TimerId(0),
                Some(due),
                |_| Ok(Step::Continue),
            ),
            behavior::ChildTopology::new(
                (0..2).map(|index| u64::try_from(index).unwrap()),
                |index| Some(child(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                u32::MAX,
                std::time::Duration::MAX,
            ),
        ).unwrap();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        for (index, byte) in bytes.iter().copied().enumerate() {
            let actions = match byte % 4 {
                0 => {
                    // User lane: stash filter — echo iff not Stash-routed.
                    let arg = u64::try_from(index).unwrap();
                    let actions = behavior
                        .transition(SupervisionEvent::Behavior(EventLayer::Inner(
                            EventLayer::Inner(UserEvent::user(MailAddr(9), arg)),
                        )))
                        .unwrap();
                    let echo_step: Vec<u64> = actions
                        .sends
                        .inner
                        .inner
                        .inner
                        .iter()
                        .map(|d| d.message)
                        .collect();
                    let expected = if arg % 3 != 2 { vec![arg] } else { vec![] };
                    assert_eq!(echo_step, expected, "echo lane mismatch at byte {index}");
                    assert!(
                        actions.sends.owned.replacement_commands.is_empty(),
                        "user leaked to child lane"
                    );
                    assert_eq!(actions.become_, Step::Continue);
                    actions
                }
                1 => {
                    // Peer lane: watched abnormal death stops the fold.
                    let actions = behavior
                        .transition(SupervisionEvent::Behavior(EventLayer::Inner(
                            EventLayer::Owned(PeerStopped {
                                peer,
                                outcome: Err(Crash::Failed),
                            }),
                        )))
                        .unwrap();
                    assert!(
                        matches!(actions.become_, Step::Stop(behavior::Stopped)),
                        "peer death verdict at byte {index}"
                    );
                    assert!(actions.sends.owned.replacement_commands.is_empty());
                    actions
                }
                2 => {
                    // Time lane: matching Reached fires once, then inert.
                    let actions = behavior
                        .transition(SupervisionEvent::Behavior(EventLayer::Owned(
                            TimerElapsed {
                                id: TimerId(0),
                                generation: TimerGeneration(0),
                            },
                        )))
                        .unwrap();
                    assert_eq!(
                        actions.become_,
                        Step::Continue,
                        "time verdict at byte {index}"
                    );
                    actions
                }
                _ => {
                    // Child lane: exactly one replacement to the dead slot.
                    let nonce = u64::from(byte % 2);
                    let actions = behavior
                        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
                            proxy: nonce,
                            worker: nonce,
                            outcome: Err(Crash::Failed),
                            at: base
                                + std::time::Duration::from_nanos(u64::try_from(index).unwrap()),
                        }))
                        .unwrap();
                    assert_eq!(
                        actions.sends.owned.replacement_commands.len(),
                        1,
                        "replacement at byte {index}"
                    );
                    assert_eq!(
                        actions.sends.owned.replacement_commands[0].nonce,
                        nonce,
                        "replacement route at byte {index}"
                    );
                    assert!(
                        actions.sends.inner.inner.inner.is_empty(),
                        "child event leaked into the echo lane at byte {index}"
                    );
                    assert_eq!(actions.become_, Step::Continue);
                    actions
                }
            };
            let _ = actions;
        }
    });
});
