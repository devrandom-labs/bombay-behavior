//! Two replay buffers in one stack: stash ∘ fsm under an Deadline wrapper. Every
//! user message is routed by the stash (Stash class held in the stash
//! buffer, Deliver/Release reaching the FSM, which may defer into its own
//! buffer or record), so the black-box no-drop/no-duplication
//! reconciliation spans BOTH buffers:
//! `recorded + fsm_held + stash_held + goto_consumed == stepped`.

use std::time::Duration;

use behavior::{
    Activate, DeadlineEvent, Machine, MailAddr, Move, Never, StashRoute, Step, TimerElapsed,
    TimerGeneration, TimerId, UserEvent,
};
use proptest::collection::vec;
use proptest::prelude::*;
use std::time::Instant;
use tokio::runtime::Builder;

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    A,
    B,
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unnecessary_wraps,
    reason = "the Machine `on` signature is fn(P, &mut S, &M) returning Result<_, Never>"
)]
fn on(phase: Phase, seen: &mut Vec<u64>, id: &u64) -> Result<Move<Phase>, Never> {
    Ok(match (phase, id % 4) {
        (Phase::A, 0) => Move::Goto(Phase::B),
        (Phase::B, 2) => Move::Goto(Phase::A),
        (_, 1) => Move::Defer,
        (_, _) => {
            seen.push(*id);
            Move::Stay
        }
    })
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "the stash route signature is fn(&Msg)"
)]
fn route(message: &u64) -> StashRoute {
    match message % 3 {
        0 => StashRoute::Release,
        1 => StashRoute::Deliver,
        _ => StashRoute::Stash,
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    /// Random user messages plus occasional Reached events through
    /// at ∘ stash ∘ fsm: no message is dropped or duplicated across the two
    /// buffer layers, and the time lane never disturbs them.
    #[test]
    fn two_replay_buffers_never_drop_or_duplicate(
        ids in vec(any::<u8>(), 0..256),
        fires in vec(any::<u8>(), 0..32),
    ) {
        let due = Instant::now() + Duration::from_secs(1);
        let behavior = behavior::Deadline::new(
            behavior::Stash::new(Machine::new(Vec::new(), Phase::A, on), route),
            behavior::TimerId(0),
            Some(due),
            |_| Ok(Step::Continue),
        );
        let runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let initialized = behavior.initialize().unwrap();
        let mut behavior = initialized.behavior;

        let mut consumed = 0_usize;
        let mut stepped = 0_usize;
        let mut fire_index = 0_usize;
        for (index, _) in ids.iter().enumerate() {
            let id = u64::try_from(index).unwrap();

            // Interleave a Reached delivery (fires once; duplicates are
            // then inert) between user messages.
            if fire_index < fires.len() && fires[fire_index] % 2 == 0 {
                let actions = runtime
                    .block_on(async { behavior.transition(DeadlineEvent::Elapsed(TimerElapsed {
                        id: TimerId(0),
                        generation: TimerGeneration(0),})) })
                    .unwrap();
                prop_assert_eq!(actions.become_, Step::Continue, "time lane verdict");
                fire_index += 1;
            }

            // User message through the stack. The fresh message is classified
            // in the phase BEFORE the step (a Goto flips the phase).
            let phase_before = behavior.base().phase();
            let _ = runtime
                .block_on(async { behavior.transition(DeadlineEvent::Behavior(UserEvent::user(MailAddr(0), id))) })
                .unwrap();
            stepped += 1;
            // Only Deliver/Release-routed messages reach the FSM, and only
            // those can be goto-consumed (class 0 in A, 2 in B).
            if id % 3 != 2 {
                let goto_class = match phase_before {
                    Phase::A => 0,
                    Phase::B => 2,
                };
                consumed += usize::from(id % 4 == goto_class);
            }

            let recorded = behavior.base().state().len();
            let fsm_held = behavior.base().held();
            let stash_held = behavior.stashed();
            prop_assert_eq!(
                recorded + fsm_held + stash_held + consumed,
                stepped,
                "drop/dup across two buffers at message {}: recorded={} fsm_held={} stash_held={} consumed={} stepped={}",
                id,
                recorded,
                fsm_held,
                stash_held,
                consumed,
                stepped
            );
        }
    }
}
