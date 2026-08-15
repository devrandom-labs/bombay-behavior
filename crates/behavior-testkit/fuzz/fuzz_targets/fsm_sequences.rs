#![no_main]

//! FSM attack surface: arbitrary byte sequences drive an oscillating
//! two-phase machine whose messages are their own unique occurrence ids.
//! The black-box no-drop/no-duplication invariant is asserted per byte:
//! every stepped message is either delivered exactly once or still held
//! (recorded + held + fresh-goto-consumed == stepped), and no occurrence is
//! ever recorded twice.

use behavior::{Activate, Machine, MailAddr, Move, Never, User, UserEvent};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    A,
    B,
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let machine = Machine::new(
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
        );
        let mut machine = (machine)
            .initialize()
            .unwrap()
            .behavior;
        let mut consumed = 0_usize;
        for (index, _) in bytes.iter().enumerate() {
            let id = u64::try_from(index).unwrap();
            // The fresh message is a goto only if its residue is the
            // current phase's goto class (0 in A, 2 in B).
            let goto_class = match machine.phase() {
                Phase::A => 0,
                Phase::B => 2,
            };
            consumed += usize::from(id % 4 == goto_class);
            machine.transition(User::user(MailAddr(0), id)).unwrap();
            assert_eq!(
                machine.state().len() + machine.held() + consumed,
                index + 1,
                "drop or duplication at byte {index}"
            );
            let mut sorted = machine.state().clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                machine.state().len(),
                "duplicate delivery at byte {index}"
            );
        }
    });
});
