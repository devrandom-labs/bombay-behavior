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
    Admit, Base, Behavior, Deadlined, Envelope, Exit, Phased, StashRoute, Stashing, Supervising,
    Watching, otp_propagation,
};
use bombay::capability::{Never, Step};
use core::time::Duration;
use tokio::time::Instant;

type Rec = Base<Vec<u64>, u64, Never, &'static str>;

fn recorder() -> Rec {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
    })
}

/// Plain floor: user messages fold FIFO; framework events are no-ops.
#[tokio::test]
async fn plain_folds_fifo_and_ignores_framework_events() {
    let mut b = recorder();
    assert!(matches!(b.step(Envelope::Deadline).await, Ok(Step::Continue)));
    assert!(matches!(b.step(Envelope::User(1)).await, Ok(Step::Continue)));
    assert!(matches!(
        b.step(Envelope::LinkDied { peer: 9, abnormal: true }).await,
        Ok(Step::Continue)
    ));
    assert!(matches!(b.step(Envelope::User(2)).await, Ok(Step::Continue)));
    assert_eq!(b.state(), &vec![1, 2]);
}

/// Deadlined: the slot arms the timer (min-fold), the fire routes to the
/// reaction, and it fires exactly once.
#[tokio::test]
async fn deadlined_arms_fires_once_and_forwards() {
    let due = Instant::now() + Duration::from_secs(5);
    let mut d = Deadlined::new(recorder(), Some(due), |_| Ok(Step::Stop(Exit::Normal)));
    assert_eq!(d.next_deadline(), Some(due));
    assert!(matches!(d.step(Envelope::User(7)).await, Ok(Step::Continue)));
    assert!(matches!(
        d.step(Envelope::Deadline).await,
        Ok(Step::Stop(Exit::Normal))
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
        w.step(Envelope::LinkDied { peer: 3, abnormal: false }).await,
        Ok(Step::Continue)
    ));
    assert!(matches!(w.step(Envelope::User(1)).await, Ok(Step::Continue)));
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: 3, abnormal: true }).await,
        Ok(Step::Stop(Exit::LinkDied(3)))
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

fn phased_machine() -> Phased<Base<Vec<u64>, Msg, Ph, &'static str>> {
    let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, msg: Msg| match msg {
        Msg::Work(id) => {
            seen.push(id);
            Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
        }
        Msg::Promote => Ok(Step::Goto(Ph::Ready)),
        Msg::Quit => Ok(Step::Stop(Exit::Normal)),
    });
    Phased::new(inner, Ph::Loading, |ph, msg| match (ph, msg) {
        (Ph::Loading, Msg::Work(_)) => Admit::Defer,
        _ => Admit::Deliver,
    })
}

/// Phased: work defers in Loading; the promotion releases the deferred batch
/// FIFO within the goto step, ahead of the backlog.
#[tokio::test]
async fn phased_releases_deferred_batch_fifo_on_goto() {
    let mut p = phased_machine();
    for m in [Msg::Work(1), Msg::Work(2), Msg::Promote, Msg::Work(3), Msg::Quit] {
        let _ = p.step(Envelope::User(m)).await;
    }
    assert_eq!(p.inner().state(), &vec![1, 2, 3]);
    assert_eq!(p.phase(), Ph::Ready);
}

/// Supervising: an abnormal child stop restarts within budget; exhaustion and
/// normal stops leave the child dead.
#[tokio::test]
async fn supervising_restarts_within_budget_only() {
    type Kid = Base<u32, u32, Never, &'static str>;
    fn kid() -> Kid {
        Base::new(0_u32, |c: &mut u32, n: u32| {
            *c += n;
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }
    let inner = Base::new((), |(): &mut (), _: u64| {
        Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
    });
    let mut sup = Supervising::new(inner, vec![kid()], |_| kid(), 1);

    let _ = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await;
    assert!(sup.children()[0].alive());
    assert_eq!(sup.restarts_left(), 0);

    let _ = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await;
    assert!(!sup.children()[0].alive(), "budget exhausted ⇒ dead");
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
    assert!(matches!(w.step(Envelope::User(5)).await, Ok(Step::Continue)));
    // The deadline fire reaches the Deadlined layer through the outer forward.
    assert!(matches!(w.step(Envelope::Deadline).await, Ok(Step::Continue)));
    assert_eq!(w.next_deadline(), None, "the inner slot fired once");
    // An abnormal death still propagates at the outer layer.
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: 8, abnormal: true }).await,
        Ok(Step::Stop(Exit::LinkDied(8)))
    ));
    assert_eq!(w.inner().inner().state(), &vec![5]);
}
