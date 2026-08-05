#![no_main]

//! Error-path attack surface: an oscillating two-phase machine whose `on`
//! returns a controlled error for one residue class in the second phase.
//! Black-box buffer invariants asserted per byte:
//! - an errored step never grows the held buffer (`held_after <= held_before`);
//! - an ok step grows it by at most one (the fresh deferred message);
//! - no occurrence is ever recorded twice (no duplicate delivery);
//! - the fold never panics.

use behaviorpass::{Behavior, Fsm, MailAddr, Move, User, UserEvent};
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
        let mut machine = Fsm::new(Vec::new(), Phase::A, |phase, seen: &mut Vec<u64>, id: &u64| {
            match (phase, id % 4) {
                (Phase::A, 0) => Ok(Move::Goto(Phase::B)),
                (Phase::A, _) => Ok(Move::Defer),
                (Phase::B, 1) => Err(()),
                (Phase::B, _) => {
                    seen.push(*id);
                    Ok(Move::Stay)
                }
            }
        });
        let mut held_before = 0_usize;
        for (index, _) in bytes.iter().enumerate() {
            let id = u64::try_from(index).unwrap();
            let result = machine.step(User::user(MailAddr(0), id)).await;
            let held_after = machine.held();
            if result.is_err() {
                assert!(
                    held_after <= held_before,
                    "error grew the buffer at byte {index}: {held_before} -> {held_after}"
                );
            } else {
                assert!(
                    held_after <= held_before + 1,
                    "ok grew the buffer too fast at byte {index}: {held_before} -> {held_after}"
                );
            }
            let mut sorted = machine.state().clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                machine.state().len(),
                "duplicate delivery at byte {index}"
            );
            held_before = held_after;
        }
    });
});
