#![no_main]

//! Stash attack surface: byte sequences drive a stash filter whose route
//! classifies each message by `id % 3` (0 = Release, 1 = Deliver, 2 =
//! Stash), with messages as their own unique occurrence ids. The black-box
//! no-drop/no-duplication invariant is asserted per byte: every message is
//! either delivered to the inner recorder exactly once or still held, so
//! `|recorded| + held() == stepped` and no id is recorded twice.

use behaviorpass::{Acted, Actions, Base, Behavior, Delivery, MailAddr, Never, Recipient, State, Step, StashRoute, Stashing, User, UserEvent};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

#[derive(Default)]
struct Recorder {
    seen: Vec<u64>,
}

impl State<u64> for Recorder {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Never, Never> {
        self.seen.push(message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
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
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let mut behavior = Stashing::new(Base::new(Recorder::default()), route);
        for (index, _) in bytes.iter().enumerate() {
            let id = u64::try_from(index).unwrap();
            behavior.step(User::user(MailAddr(0), id)).await.unwrap();
            assert_eq!(
                behavior.inner().state().seen.len() + behavior.held(),
                index + 1,
                "drop or duplication at byte {index}"
            );
            let mut sorted = behavior.inner().state().seen.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                behavior.inner().state().seen.len(),
                "duplicate delivery at byte {index}"
            );
        }
    });
});
