//! FROZEN trace oracle (bombay card #298): drives the SUT's public capability
//! layers through representative scripts and asserts the EXACT observable trace
//! (verdicts + folded state) — the reference-derived ground truth. The
//! concision loop may rewrite `crates/behaviorpass/src/**` freely but cannot
//! edit THIS file (`.auto/checks.sh` freezes it): if golf changes a layer's
//! observable behavior at any point, a trace here diverges and the gate reverts.
//!
//! Trace equality is asserted at the composed-`Behavior::step` level (the fold
//! IS the semantics, ADR-0028); the async driver's select wiring is exercised
//! by the in-crate integration tests. Every awaited step is naturally bounded
//! (no real timer), so a hung layer cannot stall the measure loop.

use behaviorpass::{
    Actions, Base, Behavior, Deadlined, Envelope, Exit, Fsm, Move, StashRoute, Stashing,
    Supervising, Watching, otp_propagation,
};
use bombay::capability::{Never, Step};
use core::time::Duration;
use tokio::time::Instant;

type Rec = Base<Vec<u64>, u64, Never, &'static str>;

fn recorder() -> Rec {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    })
}

/// Plain floor: user messages fold FIFO; framework events are no-ops.
#[tokio::test]
async fn plain_folds_fifo_and_ignores_framework_events() {
    let mut b = recorder();
    assert!(matches!(b.step(Envelope::Deadline).await.unwrap().become_, Step::Continue));
    assert!(matches!(b.step(Envelope::User(1)).await.unwrap().become_, Step::Continue));
    assert!(matches!(
        b.step(Envelope::LinkDied { peer: 9, abnormal: true }).await.unwrap().become_,
        Step::Continue
    ));
    assert!(matches!(b.step(Envelope::User(2)).await.unwrap().become_, Step::Continue));
    assert_eq!(b.state(), &vec![1, 2]);
}

/// Deadlined: the slot arms the timer (min-fold), the fire routes to the
/// reaction, and it fires exactly once.
#[tokio::test]
async fn deadlined_arms_fires_once_and_forwards() {
    let due = Instant::now() + Duration::from_secs(5);
    let mut d = Deadlined::new(recorder(), Some(due), |_| Ok(Step::Stop(Exit::Normal)));
    assert_eq!(d.next_deadline(), Some(due));
    assert!(matches!(d.step(Envelope::User(7)).await.unwrap().become_, Step::Continue));
    assert!(matches!(
        d.step(Envelope::Deadline).await.unwrap().become_,
        Step::Stop(Exit::Normal)
    ));
    assert_eq!(d.next_deadline(), None, "fires once");
    assert_eq!(d.inner().state(), &vec![7]);
}

/// Watching: an abnormal linked death propagates with the carried reason; a
/// normal one is absorbed.
#[tokio::test]
async fn watching_propagates_abnormal_only() {
    let mut w = Watching::new(recorder(), otp_propagation);
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: 3, abnormal: false }).await.unwrap().become_,
        Step::Continue
    ));
    assert!(matches!(w.step(Envelope::User(1)).await.unwrap().become_, Step::Continue));
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: 3, abnormal: true }).await.unwrap().become_,
        Step::Stop(Exit::LinkDied(3))
    ));
    assert_eq!(w.inner().state(), &vec![1]);
}

/// Stashing: a release delivers its trigger then drains the held batch in one
/// step; re-stashed messages return to held (the snapshot bound — no livelock).
#[tokio::test]
async fn stashing_release_drains_atomically_under_the_snapshot_bound() {
    let mut s = Stashing::new(recorder(), |&id| match id {
        0 => StashRoute::Release,
        n if n % 2 == 1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    for id in [1_u64, 2, 3, 0, 4] {
        let _ = s.step(Envelope::User(id)).await;
    }
    assert_eq!(s.inner().state(), &vec![2, 0, 4]);
    assert_eq!(s.held(), 2);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ph {
    Loading,
    Ready,
}

enum Msg {
    Work(u64),
    Promote,
    Quit,
}

fn fsm_machine() -> Fsm<Vec<u64>, Msg, Ph, &'static str> {
    Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, seen: &mut Vec<u64>, msg: &Msg| {
        Ok::<Move<Ph>, &'static str>(match (phase, msg) {
            (Ph::Loading, Msg::Work(_)) => Move::Defer,
            (_, Msg::Work(id)) => {
                seen.push(*id);
                Move::Stay
            }
            (_, Msg::Promote) => Move::Goto(Ph::Ready),
            (_, Msg::Quit) => Move::Stop,
        })
    })
}

/// A state machine (built from core): work defers in Loading; the promotion
/// transitions to Ready and replays the deferred batch FIFO, ahead of the
/// backlog. (This is the old "phased" scenario, now the `Fsm` example.)
#[tokio::test]
async fn fsm_defers_then_replays_on_transition() {
    let mut p = fsm_machine();
    for m in [Msg::Work(1), Msg::Work(2), Msg::Promote, Msg::Work(3), Msg::Quit] {
        let _ = p.step(Envelope::User(m)).await;
    }
    assert_eq!(p.state(), &vec![1, 2, 3]);
    assert_eq!(p.phase(), Ph::Ready);
}

/// Supervising: an abnormal child stop within budget EMITS one create-spec (the
/// driver spawns it) and re-marks the slot live; exhaustion and normal stops
/// emit no create and leave the slot dead.
#[tokio::test]
async fn supervising_restarts_within_budget_only() {
    type Kid = Base<u32, u32, Never, &'static str>;
    fn kid() -> Kid {
        Base::new(0_u32, |c: &mut u32, n: u32| {
            *c += n;
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        })
    }
    let inner = Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    let mut sup = Supervising::new(inner, 1, |_| kid(), 1);

    let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.unwrap();
    assert_eq!(actions.creates.len(), 1, "the restart emits a create-spec");
    assert!(sup.children()[0].alive());
    assert_eq!(sup.restarts_left(), 0);

    let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.unwrap();
    assert_eq!(actions.creates.len(), 0, "budget exhausted ⇒ no create");
    assert!(!sup.children()[0].alive(), "budget exhausted ⇒ dead");

    // A normal stop, on a fresh supervisor, emits no create and spends nothing.
    let mut sup2 = Supervising::new(
        Base::new((), |(): &mut (), _: u64| {
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        }),
        1,
        |_| kid(),
        5,
    );
    let actions = sup2.step(Envelope::ChildStopped { idx: 0, abnormal: false }).await.unwrap();
    assert_eq!(actions.creates.len(), 0, "a normal stop emits no create");
    assert!(!sup2.children()[0].alive(), "a normal stop is final");
    assert_eq!(sup2.restarts_left(), 5, "no budget spent on a normal stop");
}

/// A LEGAL composition — `Watching<Deadlined<Base>>`: each layer routes its own
/// framework event and forwards the rest, and the deadline min-fold surfaces
/// through the outer watch layer.
#[tokio::test]
async fn composed_watching_over_deadlined_routes_each_source() {
    let due = Instant::now() + Duration::from_secs(3);
    let deadlined = Deadlined::new(recorder(), Some(due), |_| Ok(Step::Continue));
    let mut w = Watching::new(deadlined, otp_propagation);

    assert_eq!(w.next_deadline(), Some(due), "the inner deadline surfaces through watch");
    assert!(matches!(w.step(Envelope::User(5)).await.unwrap().become_, Step::Continue));
    // The deadline fire reaches the Deadlined layer through the outer forward.
    assert!(matches!(w.step(Envelope::Deadline).await.unwrap().become_, Step::Continue));
    assert_eq!(w.next_deadline(), None, "the inner slot fired once");
    // An abnormal death still propagates at the outer layer.
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: 8, abnormal: true }).await.unwrap().become_,
        Step::Stop(Exit::LinkDied(8))
    ));
    assert_eq!(w.inner().inner().state(), &vec![5]);
}
