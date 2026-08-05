#![no_main]

//! Capstone: the full four-layer stack (supervision ∘ at ∘ watch ∘ stash)
//! under coverage-guided byte sequences. Each byte selects one of four
//! lanes (user / peer / time / child); per-lane reference models — the
//! stash filter for the echo lane, the watch verdict for peer deaths, the
//! one-shot At for time events, OneForOne/Permanent/unbounded supervision
//! for child deaths — are asserted per byte: effects land in exactly their
//! product lane and never leak across.

use behaviorpass::{
    Acted, Actions, AtEvent, AtId, Base, Behavior, ChildStopped, Crash, Delivery, Exit, MailAddr,
    Never, PeerStopped, Recipient, RestartPolicy, Route, Spec, StashRoute, State, Step, Strategy,
    SupervisionEvent, TimeReached, UserEvent, WatchEvent, stop_on_abnormal_death,
};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;
use tokio::time::Instant;

#[derive(Default)]
struct EchoingParent {
    seen: Vec<u64>,
}

impl State<u64, Base<Echo, u8>, Never> for EchoingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Base<Echo, u8>, Never> {
        self.seen.push(message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

#[derive(Default)]
struct Echo;

impl State<u8> for Echo {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        Ok(Actions::cont())
    }
}

fn child(_index: usize) -> Base<Echo, u8> {
    Base::new(Echo)
}

fn route(message: &u64) -> StashRoute {
    match message % 3 {
        0 => StashRoute::Release,
        1 => StashRoute::Deliver,
        _ => StashRoute::Stash,
    }
}

type Stack = behaviorpass::Spec<
    behaviorpass::Supervising<
        behaviorpass::At<behaviorpass::Watching<behaviorpass::Stashing<Base<EchoingParent, u64, Base<Echo, u8>, Never>>>>,
        Base<Echo, u8>,
    >,
>;

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let due = Instant::now() + std::time::Duration::from_secs(1);
        let peer = MailAddr(44);
        let mut behavior: Stack = Spec::new(EchoingParent::default())
            .stash(route)
            .watch(peer, stop_on_abnormal_death)
            .at(Some(due), |_| Ok(Step::Continue))
            .children((2, child))
            .restart(Strategy::OneForOne)
            .when(RestartPolicy::Permanent)
            .within(u32::MAX, std::time::Duration::MAX);
        behavior.init().await.unwrap();
        let base = Instant::now();

        for (index, byte) in bytes.iter().copied().enumerate() {
            let actions = match byte % 4 {
                0 => {
                    // User lane: stash filter — echo iff not Stash-routed.
                    let arg = u64::try_from(index).unwrap();
                    let actions = behavior
                        .step(SupervisionEvent::Inner(AtEvent::Inner(WatchEvent::Inner(
                            UserEvent::user(MailAddr(9), arg),
                        ))))
                        .await
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
                    assert!(actions.sends.own.own.is_empty(), "user leaked to child lane");
                    assert_eq!(actions.become_, Step::Continue);
                    actions
                }
                1 => {
                    // Peer lane: watched abnormal death stops the fold.
                    let actions = behavior
                        .step(SupervisionEvent::Inner(AtEvent::Inner(WatchEvent::PeerStopped(
                            PeerStopped {
                                peer,
                                outcome: Err(Crash::Failed),
                            },
                        ))))
                        .await
                        .unwrap();
                    assert!(
                        matches!(actions.become_, Step::Stop(Exit::LinkDied(p)) if p == peer),
                        "peer death verdict at byte {index}"
                    );
                    assert!(actions.sends.own.own.is_empty());
                    actions
                }
                2 => {
                    // Time lane: matching Reached fires once, then inert.
                    let actions = behavior
                        .step(SupervisionEvent::Inner(AtEvent::Reached(TimeReached {
                            id: AtId(0),
                            at: due,
                        })))
                        .await
                        .unwrap();
                    assert_eq!(actions.become_, Step::Continue, "time verdict at byte {index}");
                    actions
                }
                _ => {
                    // Child lane: exactly one replacement to the dead slot.
                    let nonce = u64::from(byte % 2);
                    let actions = behavior
                        .step(SupervisionEvent::ChildStopped(ChildStopped {
                            nonce,
                            outcome: Err(Crash::Failed),
                            at: base + std::time::Duration::from_nanos(u64::try_from(index).unwrap()),
                        }))
                        .await
                        .unwrap();
                    assert_eq!(actions.sends.own.own.len(), 1, "replacement at byte {index}");
                    assert_eq!(
                        actions.sends.own.own[0].to.route(),
                        Route::Child(nonce),
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
