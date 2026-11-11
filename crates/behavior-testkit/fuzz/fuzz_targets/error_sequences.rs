#![no_main]

//! Refusal and custody attacks against deferred replay. Input bytes select
//! operations; their positions distinguish occurrences. An independent range
//! oracle partitions the inputs around the first opening and first refusal,
//! rather than reproducing the machine's replay loop.

use behavior_actors::{Machine, Move};
use behavior_core::{MailAddr, Step, User, delegate_transition, initialize};
use libfuzzer_sys::fuzz_target;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Closed,
    Open,
}

fn occurrence(index: usize, byte: u8) -> u64 {
    u64::try_from(index).unwrap().checked_mul(4).unwrap() + u64::from(byte % 4)
}

fuzz_target!(|bytes: &[u8]| {
    let opening = bytes.iter().position(|byte| byte % 4 == 0);
    let replay_refusal =
        opening.and_then(|open| bytes[..open].iter().position(|byte| byte % 4 == 1));
    let mut machine = Machine::new(
        Vec::new(),
        Phase::Closed,
        |phase, seen: &mut Vec<u64>, id: &u64| match (phase, id % 4) {
            (Phase::Closed, 0) => Ok(Move::Goto(Phase::Open)),
            (Phase::Closed, _) => Ok(Move::Defer),
            (Phase::Open, 1) => Err(()),
            (Phase::Open, _) => {
                seen.push(*id);
                Ok(Move::Stay)
            }
        },
    );
    let started = initialize(&mut machine).unwrap();
    assert!(started.sends.is_empty());
    assert!(started.creates.is_empty());
    assert_eq!(started.become_, Step::Continue);

    for (index, byte) in bytes.iter().copied().enumerate() {
        let id = occurrence(index, byte);
        let result = delegate_transition(&mut machine, User::new(MailAddr(0), id));
        let expected_refusal = match opening {
            Some(open) if index == open => replay_refusal,
            Some(open) if index > open && byte % 4 == 1 => Some(index),
            _ => None,
        };
        match (result, expected_refusal) {
            (Ok(actions), None) => {
                assert!(actions.sends.is_empty());
                assert!(actions.creates.is_empty());
                assert_eq!(actions.become_, Step::Continue);
            }
            (Err(error), Some(refused)) => {
                assert_eq!(error.event.from, MailAddr(0));
                assert_eq!(error.event.message, occurrence(refused, bytes[refused]));
                assert_eq!(error.cause, ());
            }
            _ => panic!("the exact refusal and successful Actions must match"),
        }

        let expected_seen: Vec<_> = bytes
            .iter()
            .copied()
            .enumerate()
            .take(index + 1)
            .filter(|(position, value)| match opening {
                Some(open) if index >= open && *position < open => {
                    *position < replay_refusal.unwrap_or(open)
                }
                Some(open) if *position > open => value % 4 != 1,
                _ => false,
            })
            .map(|(position, value)| occurrence(position, value))
            .collect();
        assert_eq!(machine.state(), &expected_seen);
        let expected_phase = match opening {
            Some(open) if index >= open => Phase::Open,
            _ => Phase::Closed,
        };
        assert_eq!(machine.phase(), expected_phase);
        let expected_held = match opening {
            Some(open) if index >= open => replay_refusal.map_or(0, |refused| open - refused - 1),
            _ => index + 1,
        };
        assert_eq!(machine.held(), expected_held);
    }

    let remaining = machine.take();
    assert_eq!(machine.held(), 0);
    let actual: Vec<_> = remaining
        .into_iter()
        .map(|user| (user.from, user.message))
        .collect();
    let retained = match opening {
        Some(open) => replay_refusal.map_or(open..open, |refused| refused + 1..open),
        None => 0..bytes.len(),
    };
    let expected: Vec<_> = retained
        .map(|index| (MailAddr(0), occurrence(index, bytes[index])))
        .collect();
    assert_eq!(actual, expected);
});
