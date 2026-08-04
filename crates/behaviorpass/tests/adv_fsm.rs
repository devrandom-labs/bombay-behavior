//! Fsm (state machine) invariant suite — the "adversarial" additions to
//! `tests/oracle.rs`. Targets the two Fsm survivors (fsm.rs:123 delete arm
//! `Step::Stop(exit)`; fsm.rs:124 guard `changed` -> true) and pins the whole
//! algebra: Stop verdict, replay-on-ACTUAL-change only, D3 commit-after-Ok,
//! drain ordering + abandonment + snapshot bound, framework no-ops.
//!
//! Methods: handcrafted edge/sequence/lifecycle tests + a differential
//! property model (an independent fold is the oracle) + long-sequence fuzz.

use std::collections::VecDeque;

use behaviorpass::{Behavior, Crash, Envelope, Exit, Fsm, MailAddr, Move, run};
use behaviorpass::{Never, Step};
use fastpass::{Config, channel};
use tokio::time::Instant;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ph {
    Loading,
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Msg {
    Work(u64),
    Refresh,
    Promote,
    Quit,
}

/// `Move::Stop` must surface as a `Stop` verdict from `step` — the delete-arm
/// mutant (fsm.rs:123) makes it fall through to `cont()`.
#[tokio::test]
async fn fsm_stop_verdict_rides_out_of_step() {
    let mut fsm = Fsm::new((), Ph::Loading, |_phase, (): &mut (), m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match m {
            Msg::Quit => Move::Stop,
            _ => Move::Stay,
        })
    });
    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Quit }).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::Normal),
        "a Move::Stop must ride out as a Stop verdict from step"
    );
}

/// A step-level Stop abandons the held batch without folding it.
#[tokio::test]
async fn fsm_stop_keeps_the_held_buffer() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, _seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            (_, Msg::Quit) => Move::Stop,
            _ => Move::Stay,
        })
    });
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await;
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(2) }).await;
    assert_eq!(fsm.held(), 2);

    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Quit }).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal));
    assert_eq!(fsm.held(), 2, "a step-level Stop never drains the held batch");
}

/// A Goto to the SAME phase is a no-op (bombay: "Goto(current) is deliberately
/// a no-op: no unstash") — the `changed` guard (fsm.rs:124) must NOT replay.
/// The transition fn classifies by call count: the FIRST sight of a Work
/// defers, later sights fold. A drain after the same-phase Goto would make the
/// held Work fold (seen grows to 2 entries) — observable only if a replay ran.
#[tokio::test]
async fn fsm_goto_to_the_same_phase_does_not_replay() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Ready, |phase, seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Ready, Msg::Work(id)) => {
                seen.push(*id);
                if seen.len() < 2 {
                    Move::Defer
                } else {
                    Move::Stay
                }
            }
            (_, Msg::Refresh) => Move::Goto(phase),
            _ => Move::Stay,
        })
    });

    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await; // first sight: defers
    assert_eq!(fsm.state(), &vec![1]);
    assert_eq!(fsm.held(), 1);

    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Refresh }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "a same-phase Goto is a Continue no-op");
    assert_eq!(fsm.phase(), Ph::Ready);
    assert_eq!(fsm.state(), &vec![1], "same-phase Goto must NOT replay the held batch");
    assert_eq!(fsm.held(), 1, "the held batch is untouched by a no-op Goto");
}

/// A plain `Stay` verdict must never drain either — same discriminator.
#[tokio::test]
async fn fsm_stay_never_drains() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Ready, |phase, seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Ready, Msg::Work(id)) => {
                seen.push(*id);
                if seen.len() < 2 {
                    Move::Defer
                } else {
                    Move::Stay
                }
            }
            _ => Move::Stay,
        })
    });

    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await; // defers: held=[1]
    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(2) }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(fsm.state(), &vec![1, 2], "Stay folds Work(2) only");
    assert_eq!(fsm.held(), 1, "Stay never drains — Work(1) stays held");
}

/// A REAL phase change replays the held batch FIFO, ahead of the backlog.
#[tokio::test]
async fn fsm_real_goto_replays_held_fifo_ahead_of_backlog() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            (Ph::Ready, Msg::Work(id)) => {
                seen.push(*id);
                Move::Stay
            }
            (_, Msg::Promote) => Move::Goto(Ph::Ready),
            _ => Move::Stay,
        })
    });

    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await;
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(2) }).await; // held=[1,2]
    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Promote }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "a completed replay is Continue");
    assert_eq!(fsm.phase(), Ph::Ready);
    assert_eq!(fsm.state(), &vec![1, 2], "the deferred batch replays FIFO");

    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(3) }).await; // backlog after replay
    assert_eq!(fsm.state(), &vec![1, 2, 3], "the backlog folds after the replay");
    assert_eq!(fsm.held(), 0);
}

/// A `Stop` DURING a replay abandons the rest of the batch and re-holds it.
#[tokio::test]
async fn fsm_drain_stop_abandons_the_rest_and_re_holds() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, _seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            (Ph::Ready, Msg::Work(_)) => Move::Stop,
            (_, Msg::Promote) => Move::Goto(Ph::Ready),
            _ => Move::Stay,
        })
    });
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await;
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(2) }).await; // held=[1,2]

    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Promote }).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::Normal),
        "a Stop during the replay rides out of step"
    );
    assert_eq!(fsm.held(), 1, "the abandoned tail re-enters held");
}

/// A replay in a phase that re-defers is snapshot-bounded: the batch re-holds
/// and the step terminates (no livelock).
#[tokio::test]
async fn fsm_drain_re_defer_is_snapshot_bounded() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, _seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (_, Msg::Work(_)) => Move::Defer,
            (Ph::Loading, Msg::Promote) => Move::Goto(Ph::Ready),
            _ => Move::Stay,
        })
    });
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await; // held=[1]
    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Promote }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "a re-holding replay still terminates");
    assert_eq!(fsm.phase(), Ph::Ready);
    assert_eq!(fsm.state(), &Vec::<u64>::new(), "nothing folded — everything re-held");
    assert_eq!(fsm.held(), 1, "the re-deferred message lands back in held, once");
}

/// D3: an Err transition never half-switches the phase, and the failing
/// message is neither folded nor held.
#[tokio::test]
async fn fsm_err_never_commits_and_never_holds() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, _seen: &mut Vec<u64>, m: &Msg| {
        match (phase, m) {
            (_, Msg::Work(_)) => Ok(Move::Defer),
            (_, Msg::Promote) => Err("boom"),
            _ => Ok(Move::Stay),
        }
    });
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await; // held=[1]

    let err = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Promote }).await.err().expect("expected the failing transition");
    assert_eq!(err, "boom", "the failing transition surfaces its exact error");
    assert_eq!(fsm.phase(), Ph::Loading, "an Err never half-switches the phase (D3)");
    assert_eq!(fsm.held(), 1, "the errored message is not consumed or held");
}

/// A mid-replay phase change folds freshly held messages back into the batch.
#[tokio::test]
async fn fsm_mid_replay_transition_folds_fresh_holds_back_in() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph3 {
        Loading,
        Ready,
        Done,
    }
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph3::Loading, |phase, seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph3>, &'static str>(match (phase, m) {
            (Ph3::Loading, Msg::Work(_)) => Move::Defer,
            (Ph3::Ready, Msg::Work(id)) if *id >= 2 => Move::Goto(Ph3::Done),
            (_, Msg::Work(id)) => {
                seen.push(*id);
                Move::Stay
            }
            (_, Msg::Promote) => Move::Goto(Ph3::Ready),
            _ => Move::Stay,
        })
    });
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await; // held=[1]
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(9) }).await; // held=[1,9]
    let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Promote }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(fsm.phase(), Ph3::Done, "the mid-replay transition committed");
    assert_eq!(fsm.state(), &vec![1], "Work(1) folded in Ready; Work(9) moved to Done");
    assert_eq!(fsm.held(), 0, "the phase change folded the fresh holds back in");
}

/// The full driver path for a state machine: defer → real phase change →
/// replay → stop, folded through the real mailbox; the Stop exit rides out.
#[tokio::test]
async fn fsm_driven_through_the_mailbox_defer_replay_stop() {
    let fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            (Ph::Ready, Msg::Work(id)) => {
                seen.push(*id);
                Move::Stay
            }
            (_, Msg::Promote) => Move::Goto(Ph::Ready),
            (_, Msg::Quit) => Move::Stop,
            _ => Move::Stay,
        })
    });
    let (ctl, usr, rx) = channel::<Never, Msg>(Config::new(8));
    let handle = tokio::spawn(run(fsm, rx, MailAddr(0)));

    usr.send(Msg::Work(1)).await.expect("mailbox open");
    usr.send(Msg::Work(2)).await.expect("mailbox open");
    usr.send(Msg::Promote).await.expect("mailbox open"); // real phase change → replay
    usr.send(Msg::Work(3)).await.expect("mailbox open");
    usr.send(Msg::Quit).await.expect("mailbox open"); // Stop
    // Closing the lanes guarantees the driver terminates even on a mutant that
    // breaks the Stop path — a broken Stop then fails the exit assertion
    // (Collected vs Normal) instead of hanging the mutant run.
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.exit, Exit::Normal, "the Fsm's Stop verdict rides out of the driver");
    assert!(transcript.sends.is_empty() && transcript.creates.is_empty(), "an Fsm sends and creates nothing");
}

/// A drained mailbox collects an Fsm that never stops.
#[tokio::test]
async fn fsm_driven_through_the_mailbox_collects_on_close() {
    let fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            (Ph::Ready, Msg::Work(id)) => {
                seen.push(*id);
                Move::Stay
            }
            (_, Msg::Promote) => Move::Goto(Ph::Ready),
            _ => Move::Stay,
        })
    });
    let (ctl, usr, rx) = channel::<Never, Msg>(Config::new(8));
    let handle = tokio::spawn(run(fsm, rx, MailAddr(0)));

    usr.send(Msg::Work(1)).await.expect("mailbox open");
    usr.send(Msg::Work(2)).await.expect("mailbox open");
    usr.send(Msg::Promote).await.expect("mailbox open");
    drop(usr);
    drop(ctl);

    let transcript = handle.await.expect("driver joins").expect("no crash");
    assert_eq!(transcript.exit, Exit::Collected, "a fully-closed mailbox collects the Fsm");
}

/// Framework events are no-ops for a plain state machine.
#[tokio::test]
async fn fsm_framework_events_are_noops() {
    let mut fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, _seen: &mut Vec<u64>, m: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            _ => Move::Stay,
        })
    });
    let _ = fsm.step(Envelope::User { from: MailAddr(1), msg: Msg::Work(1) }).await; // held=[1]
    for ev in [
        Envelope::Deadline,
        Envelope::LinkDied { peer: MailAddr(42), outcome: Err(Crash::Failed) },
        Envelope::ChildStopped { nonce: 0, outcome: Ok(Exit::Normal), at: Instant::now() },
    ] {
        let actions = fsm.step(ev).await.expect("no error");
        assert_eq!(actions.become_, Step::Continue, "a framework event is a no-op");
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
    }
    assert_eq!(fsm.phase(), Ph::Loading);
    assert_eq!(fsm.held(), 1, "framework events leave the machine untouched");
    assert_eq!(fsm.state(), &Vec::<u64>::new());
}

// ---------------------------------------------------------------------------
// Differential property model + fuzz
// ---------------------------------------------------------------------------

/// The model: an independent, obviously-correct fold of the same script — the
/// oracle the SUT must match after every op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Work(u64),
    Refresh,
    Promote,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Model {
    phase: Ph,
    seen: Vec<u64>,
    held: VecDeque<u64>,
}

impl Model {
    fn new() -> Self {
        Self { phase: Ph::Loading, seen: Vec::new(), held: VecDeque::new() }
    }

    /// Fold one op. Returns true once the machine has stopped (Quit).
    fn fold(&mut self, op: Op) -> bool {
        match op {
            Op::Work(id) => match self.phase {
                Ph::Loading => self.held.push_back(id),
                Ph::Ready => self.seen.push(id),
            },
            Op::Refresh => {} // Goto(current) is a no-op: no phase change, no replay
            Op::Promote => {
                if self.phase == Ph::Loading {
                    self.phase = Ph::Ready;
                    // Replay the held batch FIFO in the new phase; Work folds.
                    let batch: VecDeque<u64> = self.held.drain(..).collect();
                    self.seen.extend(batch);
                }
            }
            Op::Quit => return true,
        }
        false
    }
}

fn sut(phase: Ph) -> Fsm<MailAddr, Vec<u64>, Op, Ph, &'static str> {
    Fsm::new(Vec::new(), phase, |phase, seen: &mut Vec<u64>, m: &Op| {
        Ok::<Move<Ph>, &'static str>(match (phase, m) {
            (Ph::Loading, Op::Work(_)) => Move::Defer,
            (Ph::Ready, Op::Work(id)) => {
                seen.push(*id);
                Move::Stay
            }
            (_, Op::Refresh) => Move::Goto(phase),
            (_, Op::Promote) => Move::Goto(Ph::Ready),
            (_, Op::Quit) => Move::Stop,
        })
    })
}

fn op_strategy() -> impl proptest::strategy::Strategy<Value = Op> {
    use proptest::prelude::*;
    prop_oneof![
        proptest::prelude::Just(Op::Work(0)),
        proptest::prelude::Just(Op::Work(1)),
        proptest::prelude::Just(Op::Work(u64::MAX)),
        any::<u64>().prop_map(Op::Work),
        proptest::prelude::Just(Op::Refresh),
        proptest::prelude::Just(Op::Promote),
        proptest::prelude::Just(Op::Quit),
    ]
}

fn fold_script_and_check(rt: &tokio::runtime::Runtime, ops: &[Op]) {
    let mut model = Model::new();
    let mut fsm = sut(Ph::Loading);
    rt.block_on(async {
        for (i, op) in ops.iter().copied().enumerate() {
            let actions = fsm.step(Envelope::User { from: MailAddr(1), msg: op }).await.expect("no error");
            if model.fold(op) {
                assert_eq!(
                    actions.become_,
                    Step::Stop(Exit::Normal),
                    "op #{i}: the Quit that stopped the model must stop the SUT"
                );
                break; // ops after the stop are never folded
            }
            assert_eq!(
                actions.become_,
                Step::Continue,
                "op #{i}: a non-stopping op must not stop"
            );
            assert_eq!(fsm.phase(), model.phase, "op #{i}: phase equality");
            assert_eq!(fsm.state(), &model.seen, "op #{i}: fold order equality");
            assert_eq!(fsm.held(), model.held.len(), "op #{i}: held count equality");
        }
    });
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Any op script (boundaries 0 / 1 / MAX / mid, empty / max length) folds
    /// identically through the SUT and the independent model.
    #[test]
    fn prop_fsm_fold_matches_differential_model(ops in proptest::collection::vec(op_strategy(), 0..=16)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_script_and_check(&rt, &ops);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 64, ..proptest::prelude::ProptestConfig::default() })]

    /// Long interleavings (livelock / ordering surface): the fold stays
    /// model-equal and terminates; held never exceeds the stashed count.
    #[test]
    fn prop_fsm_long_sequences_terminate_model_equal(ops in proptest::collection::vec(op_strategy(), 0..=64)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_script_and_check(&rt, &ops);
    }
}
