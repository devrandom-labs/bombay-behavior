#![no_main]

//! Two-buffer composition attack surface: stash ∘ fsm. Each byte selects a
//! user message (unique occurrence id); the stash routes by `id % 3`
//! (0=Release, 1=Deliver, 2=Stash) and the FSM defers/gotos/records by
//! `id % 4`. The black-box no-drop/no-duplication reconciliation spans both
//! buffers and is asserted per byte:
//! `recorded + fsm_held + stash_held + goto_consumed == stepped`, with the
//! goto class taken from the phase BEFORE the step (a Goto flips the phase).

use behavior::{Behavior, Compose, Machine, MailAddr, Move, Never, StashRoute, User, UserEvent};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    A,
    B,
}

type Stack = behavior::Compose<behavior::Stash<Machine<MailAddr, Vec<u64>, u64, Phase, Never>>>;

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let mut behavior: Stack = Compose::machine(
            Vec::new(),
            Phase::A,
            |phase, seen: &mut Vec<u64>, id: &u64| {
                Ok::<Move<Phase>, Never>(match (phase, id % 4) {
                    (Phase::A, 0) => Move::Goto(Phase::B),
                    (Phase::B, 2) => Move::Goto(Phase::A),
                    (_, 1) => Move::Defer,
                    (_, _) => {
                        seen.push(*id);
                        Move::Stay
                    }
                })
            },
        )
        .stash(|message: &u64| match message % 3 {
            0 => StashRoute::Release,
            1 => StashRoute::Deliver,
            _ => StashRoute::Stash,
        });

        let mut consumed = 0_usize;
        let mut stepped = 0_usize;
        for (index, _) in bytes.iter().enumerate() {
            let id = u64::try_from(index).unwrap();
            let phase_before = behavior.behavior().inner().phase();
            behavior.transition(User::user(MailAddr(0), id)).unwrap();
            stepped += 1;
            if id % 3 != 2 {
                let goto_class = match phase_before {
                    Phase::A => 0,
                    Phase::B => 2,
                };
                consumed += usize::from(id % 4 == goto_class);
            }
            let recorded = behavior.behavior().inner().state().len();
            let fsm_held = behavior.behavior().inner().held();
            let stash_held = behavior.behavior().held();
            assert_eq!(
                recorded + fsm_held + stash_held + consumed,
                stepped,
                "drop/dup across two buffers at byte {index}"
            );
            let mut sorted = behavior.behavior().inner().state().clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), recorded, "duplicate delivery at byte {index}");
        }
    });
});
