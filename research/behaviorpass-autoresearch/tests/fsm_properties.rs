//! FSM deferral/replay attacks. The no-drop/no-duplication invariant is
//! black-box observable without a reference model of the drain: every
//! stepped message is either delivered (recorded) exactly once or still
//! held, so `|recorded| + held() == stepped` at every point, and no
//! occurrence is ever recorded twice. The phase machine oscillates so the
//! mid-drain merge (which reorders, but never drops or duplicates) is
//! exercised repeatedly.

use std::collections::HashSet;

use behaviorpass::{Behavior, Fsm, MailAddr, Move, Never, Step, User, UserEvent};
use proptest::collection::vec;
use proptest::prelude::*;
use tokio::runtime::Builder;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    A,
    B,
}

/// Messages are their own unique occurrence ids (`index`), so the machine's
/// recorded trace identifies every delivery exactly.
///
/// - A: `id % 4 == 0` -> Goto(B); `id % 4 == 1` -> Defer; else record.
/// - B: `id % 4 == 2` -> Goto(A); `id % 4 == 1` -> Defer; else record.
fn machine() -> Fsm<MailAddr, Vec<u64>, u64, Phase, Never> {
    Fsm::new(Vec::new(), Phase::A, |phase, seen: &mut Vec<u64>, id: &u64| {
        Ok::<Move<Phase>, Never>(match (phase, id % 4) {
            (Phase::A, 0) => Move::Goto(Phase::B),
            (Phase::B, 2) => Move::Goto(Phase::A),
            (_, 1) => Move::Defer,
            (_, _) => {
                seen.push(*id);
                Move::Stay
            }
        })
    })
}

fn assert_no_drop_no_dup(seen: &[u64], held: usize, consumed: usize, stepped: usize) {
    assert_eq!(
        seen.len() + held + consumed,
        stepped,
        "drop or duplication: delivered={} held={} goto-consumed={} stepped={}",
        seen.len(),
        held,
        consumed,
        stepped
    );
    let unique = seen.iter().copied().collect::<HashSet<u64>>();
    assert_eq!(
        unique.len(),
        seen.len(),
        "duplicate delivery: {seen:?}"
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn fsm_never_drops_or_duplicates(ids in vec(any::<u8>(), 0..256)) {
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let mut machine = machine();
        let mut consumed = 0_usize;
        for (index, _) in ids.iter().enumerate() {
            let id = u64::try_from(index).unwrap();
            // The fresh message is a goto only if its residue is the
            // phase's goto class (0 in A, 2 in B) — observable via phase().
            let goto_class = match machine.phase() {
                Phase::A => 0,
                Phase::B => 2,
            };
            consumed += usize::from(id % 4 == goto_class);
            runtime.block_on(machine.step(User::user(MailAddr(0), id))).unwrap();
            assert_no_drop_no_dup(machine.state(), machine.held(), consumed, index + 1);
        }
    }

    /// A three-phase machine with a third goto class exercises the
    /// mid-drain merge under longer phase chains.
    #[test]
    fn fsm_three_phase_never_drops_or_duplicates(ids in vec(any::<u8>(), 0..256)) {
        #[derive(Clone, Copy, PartialEq)]
        enum Phase3 {
            A,
            B,
            C,
        }
        let mut machine = Fsm::new(Vec::new(), Phase3::A, |phase, seen: &mut Vec<u64>, id: &u64| {
            Ok::<Move<Phase3>, Never>(match (phase, id % 5) {
                (Phase3::A, 0) => Move::Goto(Phase3::B),
                (Phase3::B, 2) => Move::Goto(Phase3::C),
                (Phase3::C, 3) => Move::Goto(Phase3::A),
                (_, 1) => Move::Defer,
                (_, _) => {
                    seen.push(*id);
                    Move::Stay
                }
            })
        });
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let mut consumed = 0_usize;
        for (index, _) in ids.iter().enumerate() {
            let id = u64::try_from(index).unwrap();
            let goto_class = match machine.phase() {
                Phase3::A => 0,
                Phase3::B => 2,
                Phase3::C => 3,
            };
            consumed += usize::from(id % 5 == goto_class);
            runtime.block_on(machine.step(User::user(MailAddr(0), id))).unwrap();
            assert_no_drop_no_dup(machine.state(), machine.held(), consumed, index + 1);
        }
    }
}

/// Exhaustive small enumeration: every sequence of up to four messages over
/// the four residue classes, with occurrence ids kept unique by construction
/// (`id = index * 4 + residue`).
#[test]
#[allow(clippy::items_after_statements, reason = "standalone tests follow the proptest! block")]
fn fsm_exhaustive_sequences_never_drop_or_duplicate() {
    let runtime = Builder::new_current_thread().enable_all().build().unwrap();
    const ALPHABET: usize = 4;
    const MAX_LENGTH: usize = 4;

    let mut checked = 0_usize;
    let mut length = 0_usize;
    while length <= MAX_LENGTH {
        let total = ALPHABET.pow(u32::try_from(length).unwrap());
        for code in 0..total {
            let mut machine = machine();
            let mut consumed = 0_usize;
            let mut residues = Vec::with_capacity(length);
            let mut rest = code;
            for _ in 0..length {
                residues.push(rest % ALPHABET);
                rest /= ALPHABET;
            }
            for (index, residue) in residues.into_iter().enumerate() {
                let id = u64::try_from(index * ALPHABET + residue).unwrap();
                let goto_class = match machine.phase() {
                    Phase::A => 0,
                    Phase::B => 2,
                };
                consumed += usize::from(id % 4 == goto_class);
                runtime
                    .block_on(machine.step(User::user(MailAddr(0), id)))
                    .unwrap();
                assert_no_drop_no_dup(machine.state(), machine.held(), consumed, index + 1);
            }
            checked += 1;
        }
        length += 1;
    }
    assert_eq!(checked, (1 + ALPHABET + 16 + 64 + 256));
}

/// A `Move::Stop` produced mid-drain propagates the verdict and preserves
/// the remaining batch in held order; the fold keeps working afterwards.
#[tokio::test]
async fn fsm_stop_mid_drain_preserves_remaining_batch() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        P0,
        P1,
    }
    let mut machine = Fsm::new(Vec::new(), Phase::P0, |phase, seen: &mut Vec<u64>, id: &u64| {
        Ok::<Move<Phase>, Never>(match (phase, id % 4) {
            (Phase::P0, 0) => Move::Goto(Phase::P1),
            (Phase::P0, _) | (Phase::P1, 1) => Move::Defer,
            (Phase::P1, 2) => Move::Stop,
            (Phase::P1, _) => {
                seen.push(*id);
                Move::Stay
            }
        })
    });
    // Defer ids 1 and 2 in P0; id 0 opens P1 and drains: id 1 re-defers in
    // P1, id 2 stops the drain — id 1 must remain held.
    machine.step(User::user(MailAddr(0), 1)).await.unwrap();
    machine.step(User::user(MailAddr(0), 2)).await.unwrap();
    let opened = machine.step(User::user(MailAddr(0), 0)).await.unwrap();
    assert!(matches!(opened.become_, Step::Stop(behaviorpass::Exit::Normal)));
    assert!(machine.state().is_empty());
    assert_eq!(machine.held(), 1);

    // The fold is still live: id 3 records in P1.
    machine.step(User::user(MailAddr(0), 3)).await.unwrap();
    assert_eq!(machine.state().as_slice(), &[3]);
    assert_eq!(machine.held(), 1);
}
