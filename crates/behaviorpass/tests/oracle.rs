//! FROZEN trace oracle (bombay card #298): drives the SUT's public capability
//! layers through representative scripts and asserts the EXACT observable trace
//! (verdicts + folded state) — the reference-derived ground truth. The
//! concision loop may rewrite `crates/behaviorpass/src/**` freely but cannot
//! edit THIS file (`.auto/checks.sh` freezes it): if golf changes a layer's
//! observable behavior at any point, a trace here diverges and the gate reverts.
//!
//! Re-gated for card 1 (address / nonce-keyed creation / general supervision):
//! every scenario uses the new shapes (`from` stamps, nonces, `Target`,
//! `outcome: Result<Exit<A>, Crash>` — NO bools anywhere), and the supervising
//! scenarios assert the new LAWS: outcome classification in the layer, the
//! per-policy gate, window eviction, all-or-nothing budget, rest-for-one in
//! birth-SEQUENCE order, and the programmer-bug panics. After this pass the
//! file is frozen AGAIN against the golf loop.
//!
//! Trace equality is asserted at the composed-`Behavior::step` level (the fold
//! IS the semantics, ADR-0028); the async driver's select wiring is exercised
//! by the in-crate integration tests. Every awaited step is naturally bounded
//! (no real timer), so a hung layer cannot stall the measure loop.

use behaviorpass::{
    Actions, Base, Behavior, Crash, Create, Deadlined, Envelope, Exit, Fsm, MailAddr, Move,
    RestartPolicy, StashRoute, Stashing, Strategy, Supervising, Watching, stop_on_abnormal_death,
};
use behaviorpass::{Never, Step};
use core::time::Duration;
use tokio::time::Instant;

type Rec = Base<MailAddr, Vec<u64>, u64, Never, &'static str>;

fn recorder() -> Rec {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
    })
}

fn user(msg: u64) -> Envelope<MailAddr, u64> {
    Envelope::User { from: MailAddr(1), msg }
}

/// Plain floor: user messages fold FIFO; framework events are no-ops.
#[tokio::test]
async fn plain_folds_fifo_and_ignores_framework_events() {
    let mut b = recorder();
    assert!(matches!(b.step(Envelope::Deadline).await.unwrap().become_, Step::Continue));
    assert!(matches!(b.step(user(1)).await.unwrap().become_, Step::Continue));
    assert!(matches!(
        b.step(Envelope::LinkDied { peer: MailAddr(9), outcome: Err(Crash::Failed) })
            .await
            .unwrap()
            .become_,
        Step::Continue
    ));
    assert!(matches!(b.step(user(2)).await.unwrap().become_, Step::Continue));
    assert_eq!(b.state(), &vec![1, 2]);
    assert!(b.fleet().is_none(), "a plain actor declares no fleet");
}

/// Deadlined: the slot arms the timer (min-fold), the fire routes to the
/// reaction, and it fires exactly once.
#[tokio::test]
async fn deadlined_arms_fires_once_and_forwards() {
    let due = Instant::now() + Duration::from_secs(5);
    let mut d = Deadlined::new(recorder(), Some(due), |_| Ok(Step::Stop(Exit::Normal)));
    assert_eq!(d.next_deadline(), Some(due));
    assert!(matches!(d.step(user(7)).await.unwrap().become_, Step::Continue));
    assert!(matches!(
        d.step(Envelope::Deadline).await.unwrap().become_,
        Step::Stop(Exit::Normal)
    ));
    assert_eq!(d.next_deadline(), None, "fires once");
    assert_eq!(d.inner().state(), &vec![7]);
}

/// Watching: an abnormal linked death propagates with the carried reason; a
/// normal one is absorbed. Classification rides the OUTCOME — no bools:
/// `Err(Crash)` (either domain) and an `Ok` exit outside `{Normal, Collected}`
/// are abnormal.
#[tokio::test]
async fn watching_propagates_abnormal_only() {
    let mut w = Watching::new(recorder(), stop_on_abnormal_death);
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: MailAddr(3), outcome: Ok(Exit::Normal) })
            .await
            .unwrap()
            .become_,
        Step::Continue
    ));
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: MailAddr(3), outcome: Ok(Exit::Collected) })
            .await
            .unwrap()
            .become_,
        Step::Continue
    ), "collection classifies normal");
    assert!(matches!(w.step(user(1)).await.unwrap().become_, Step::Continue));
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: MailAddr(3), outcome: Err(Crash::Failed) })
            .await
            .unwrap()
            .become_,
        Step::Stop(Exit::LinkDied(MailAddr(3)))
    ));
    assert_eq!(w.inner().state(), &vec![1]);

    // A panic-domain crash and an abnormal Exit value both propagate.
    let mut w2 = Watching::new(recorder(), stop_on_abnormal_death);
    assert!(matches!(
        w2.step(Envelope::LinkDied { peer: MailAddr(4), outcome: Err(Crash::Panicked) })
            .await
            .unwrap()
            .become_,
        Step::Stop(Exit::LinkDied(MailAddr(4)))
    ), "Panicked is abnormal");
    let mut w3 = Watching::new(recorder(), stop_on_abnormal_death);
    assert!(matches!(
        w3.step(Envelope::LinkDied { peer: MailAddr(5), outcome: Ok(Exit::LinkDied(MailAddr(9))) })
            .await
            .unwrap()
            .become_,
        Step::Stop(Exit::LinkDied(MailAddr(5)))
    ), "an Ok exit outside the normal subset is abnormal");
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
        let _ = s.step(user(id)).await;
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

fn fsm_machine() -> Fsm<MailAddr, Vec<u64>, Msg, Ph, &'static str> {
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
        let _ = p.step(Envelope::User { from: MailAddr(1), msg: m }).await;
    }
    assert_eq!(p.state(), &vec![1, 2, 3]);
    assert_eq!(p.phase(), Ph::Ready);
}

// ---------------------------------------------------------------------------
// Supervising — the generalized strategy space (card 1 laws)
// ---------------------------------------------------------------------------

type Kid = Base<MailAddr, u32, u32, Never, &'static str>;
/// The inner's Offspring IS the child menu (the relaxed bound): it creates
/// nothing at runtime, but the type agrees with the fleet.
type SupInner = Base<MailAddr, (), u64, Never, &'static str, Never, Kid>;

fn kid() -> Kid {
    Base::new(0_u32, |c: &mut u32, n: u32| {
        *c += n;
        Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
    })
}

fn sup_inner() -> SupInner {
    Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<MailAddr, Never, Never, Kid>, &'static str>(Actions::cont())
    })
}

fn supervisor(
    n: usize,
    strategy: Strategy,
    policy: RestartPolicy,
    max_restarts: u32,
    window: Duration,
) -> Supervising<SupInner, Kid> {
    Supervising::new(sup_inner(), |i| i as u64, n, |_| kid(), strategy, policy, max_restarts, window)
}

fn stopped(nonce: u64, outcome: Result<Exit<MailAddr>, Crash>, at: Instant) -> Envelope<MailAddr, u64> {
    Envelope::ChildStopped { nonce, outcome, at }
}

/// The restart nonces a step emitted, in emission order.
fn restart_nonces(actions: &Actions<MailAddr, Never, Never, Kid>) -> Vec<u64> {
    actions
        .creates
        .iter()
        .map(|c| match c {
            Create::Restart { nonce, .. } => *nonce,
            Create::Birth { .. } => panic!("supervision never emits a bare birth"),
        })
        .collect()
}

/// LAW — outcome classification (no bools anywhere): `Err(Crash)` in either
/// domain and an `Ok` exit outside `{Normal, Collected}` are ABNORMAL; the
/// transient gate restarts exactly the abnormal outcomes.
#[tokio::test]
async fn supervising_classifies_outcomes_and_gates_on_transient() {
    let t0 = Instant::now();
    let mut sup = supervisor(3, Strategy::OneForOne, RestartPolicy::Transient, 5, Duration::MAX);

    for (nonce, outcome, why) in [
        (0, Err(Crash::Failed), "a controlled crash is abnormal"),
        (1, Err(Crash::Panicked), "a panic is abnormal"),
        (2, Ok(Exit::LinkDied(MailAddr(9))), "an exit outside the normal subset is abnormal"),
    ] {
        let actions = sup.step(stopped(nonce, outcome, t0)).await.unwrap();
        assert_eq!(restart_nonces(&actions), vec![nonce], "{why}");
        assert!(sup.is_alive(nonce), "{why} — restarted live");
    }
    assert_eq!(sup.restarts_in_window(), 3, "one budget unit per restart");

    let mut sup = supervisor(3, Strategy::OneForOne, RestartPolicy::Transient, 5, Duration::MAX);
    for (nonce, outcome, why) in [
        (0, Ok(Exit::Normal), "Normal is normal"),
        (1, Ok(Exit::Collected), "Collected is normal"),
    ] {
        let actions = sup.step(stopped(nonce, outcome, t0)).await.unwrap();
        assert!(actions.creates.is_empty(), "{why} — transient never restarts it");
        assert!(!sup.is_alive(nonce), "{why} — the slot dies");
    }
    assert_eq!(sup.restarts_in_window(), 0, "no budget spent on normal stops");
    assert!(sup.is_alive(2), "untouched children stay live");
}

/// LAW — the dead child's OWN policy gates evaluation: permanent restarts
/// even a normal stop; temporary never restarts, not even a crash.
#[tokio::test]
async fn supervising_policy_gates_evaluation() {
    let t0 = Instant::now();
    let mut perm = supervisor(1, Strategy::OneForOne, RestartPolicy::Permanent, 5, Duration::MAX);
    let actions = perm.step(stopped(0, Ok(Exit::Normal), t0)).await.unwrap();
    assert_eq!(restart_nonces(&actions), vec![0], "permanent restarts a normal stop");

    let mut temp = supervisor(1, Strategy::OneForOne, RestartPolicy::Temporary, 5, Duration::MAX);
    let actions = temp.step(stopped(0, Err(Crash::Failed), t0)).await.unwrap();
    assert!(actions.creates.is_empty(), "temporary never restarts, not even a crash");
    assert!(!temp.is_alive(0));
    assert_eq!(temp.restarts_in_window(), 0, "temporary spends no budget");
}

/// LAW — windowed budget: a restart older than the window stops counting
/// (eviction), and `Duration::MAX` is the count-only case.
#[tokio::test]
async fn supervising_window_evicts_old_restarts() {
    let t0 = Instant::now();
    let window = Duration::from_secs(10);
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 2, window);

    let a = sup.step(stopped(0, Err(Crash::Failed), t0)).await.unwrap();
    assert_eq!(restart_nonces(&a), vec![0]);
    let a = sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_secs(1))).await.unwrap();
    assert_eq!(restart_nonces(&a), vec![0]);
    // Two restarts inside the window: the third does not fit.
    let a = sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_secs(2))).await.unwrap();
    assert!(a.creates.is_empty(), "budget exhausted inside the window");
    assert!(!sup.is_alive(0));

    // Fresh supervisor: the first restart ages out of the window, freeing budget.
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 2, window);
    let _ = sup.step(stopped(0, Err(Crash::Failed), t0)).await.unwrap();
    let _ = sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_secs(1))).await.unwrap();
    let a = sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_secs(11))).await.unwrap();
    assert_eq!(restart_nonces(&a), vec![0], "the t0 restart evicted at t0+11s (age > window)");
    assert!(sup.is_alive(0));
    assert_eq!(
        sup.restarts_in_window(),
        2,
        "age == window still counts (prune is age > window): t0+1s survives, plus the new restart"
    );
}

/// LAW — all-or-nothing per event: if the whole candidate set does not fit
/// the budget, NOTHING restarts — the dead child dies, the live candidates
/// are untouched, and no budget is spent.
#[tokio::test]
async fn supervising_budget_is_all_or_nothing_per_event() {
    let t0 = Instant::now();
    let mut sup = supervisor(3, Strategy::OneForAll, RestartPolicy::Transient, 2, Duration::MAX);
    let a = sup.step(stopped(0, Err(Crash::Failed), t0)).await.unwrap();
    assert!(a.creates.is_empty(), "3 candidates > budget 2 ⇒ nothing restarts");
    assert!(!sup.is_alive(0), "the dead child dies");
    assert!(sup.is_alive(1) && sup.is_alive(2), "the live candidates are untouched");
    assert_eq!(sup.restarts_in_window(), 0, "a denied event spends no budget");

    // The same event with room for the whole set restarts ALL THREE, the dead
    // child FIRST, then the rest in birth-sequence order.
    let mut sup = supervisor(3, Strategy::OneForAll, RestartPolicy::Transient, 3, Duration::MAX);
    let a = sup.step(stopped(1, Err(Crash::Failed), t0)).await.unwrap();
    assert_eq!(restart_nonces(&a), vec![1, 0, 2], "dead child first, then seq order");
    assert!((0..3).all(|n| sup.is_alive(n)), "the whole fleet is live again");
    assert_eq!(sup.restarts_in_window(), 3, "one unit per restarted candidate");
}

/// LAW — rest-for-one restarts the dead child and every child born AFTER it,
/// in birth-SEQUENCE order — NOT nonce order. The static fleet mints nonces
/// 10 and 20 (seq 0 and 1); the dynamic birth at nonce 15 takes seq 2, so
/// seq order [10, 20, 15] differs from nonce order [10, 15, 20].
#[tokio::test]
async fn supervising_rest_for_one_follows_birth_sequence_not_nonce_order() {
    let t0 = Instant::now();
    // A birthing inner: on user message 15 it emits a dynamic birth at nonce 15.
    let birthing: SupInner = Base::new((), |(): &mut (), m: u64| {
        let creates = if m == 15 {
            vec![Create::Birth { nonce: 15, child: kid() }]
        } else {
            Vec::new()
        };
        Ok(Actions { sends: Vec::new(), creates, become_: Step::Continue })
    });
    let mut sup = Supervising::new(
        birthing,
        |i| ((i + 1) as u64) * 10,
        2,
        |_| kid(),
        Strategy::RestForOne,
        RestartPolicy::Transient,
        10,
        Duration::MAX,
    );
    let a = sup.step(user(15)).await.unwrap();
    let [Create::Birth { nonce, .. }] = a.creates.as_slice() else {
        panic!("a fresh dynamic birth is emitted as-is, never rewritten");
    };
    assert_eq!(*nonce, 15, "the birth keeps the creator-minted nonce");
    assert_eq!(sup.child_count(), 3, "the dynamic birth joins the table");
    assert!(sup.is_alive(15));

    // Killing the YOUNGEST child (nonce 15, seq 2) restarts it alone — even
    // though nonce 15 < 20, no older sibling rides along.
    let a = sup.step(stopped(15, Err(Crash::Failed), t0)).await.unwrap();
    assert_eq!(restart_nonces(&a), vec![15], "seq >= 2: only the youngest");

    // Killing the OLDEST (nonce 10, seq 0) restarts all three — dead first,
    // then BIRTH-SEQUENCE order [20, 15], not nonce order [15, 20].
    let a = sup.step(stopped(10, Err(Crash::Failed), t0)).await.unwrap();
    assert_eq!(restart_nonces(&a), vec![10, 20, 15], "birth-sequence order, NOT nonce order");
    assert!(sup.is_alive(10) && sup.is_alive(20) && sup.is_alive(15));
}

/// LAW — a dynamic birth whose nonce collides with ANY known slot (static or
/// dynamic, alive or dead) is a programmer bug and panics.
#[tokio::test]
#[should_panic(expected = "creator-minted nonces must be fresh")]
async fn supervising_panics_on_a_stale_birth_nonce() {
    let colliding: SupInner = Base::new((), |(): &mut (), _: u64| {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::Birth { nonce: 0, child: kid() }],
            become_: Step::Continue,
        })
    });
    let mut sup = Supervising::new(
        colliding,
        |i| i as u64,
        1,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        5,
        Duration::MAX,
    );
    let _ = sup.step(user(1)).await;
}

/// LAW — an inner-emitted `Create::Restart` is a programmer bug (restart
/// decisions belong to the layer) and panics.
#[tokio::test]
#[should_panic(expected = "restart decisions belong to the Supervising layer")]
async fn supervising_panics_on_an_inner_restart() {
    let restarting: SupInner = Base::new((), |(): &mut (), _: u64| {
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create::Restart { nonce: 0, child: kid() }],
            become_: Step::Continue,
        })
    });
    let mut sup = Supervising::new(
        restarting,
        |i| i as u64,
        1,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        5,
        Duration::MAX,
    );
    let _ = sup.step(user(1)).await;
}

/// LAW — a `ChildStopped` naming an unknown nonce is a driver/behavior
/// desync — a programmer bug — and panics.
#[tokio::test]
#[should_panic(expected = "unknown nonce")]
async fn supervising_panics_on_an_unknown_nonce() {
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 5, Duration::MAX);
    let _ = sup.step(stopped(999, Err(Crash::Failed), Instant::now())).await;
}

/// The `fleet()` query: `Supervising` declares its static fleet; wrappers
/// forward it one line each; a plain actor declares none.
#[tokio::test]
async fn fleet_query_surfaces_through_wrappers() {
    let sup = supervisor(3, Strategy::OneForOne, RestartPolicy::Transient, 5, Duration::MAX);
    let Some((n, build)) = Behavior::fleet(&sup) else {
        panic!("Supervising declares its static fleet");
    };
    assert_eq!(n, 3);
    let _ = build(0);

    let due = Instant::now() + Duration::from_secs(5);
    let wrapped = Watching::new(
        Stashing::new(
            Deadlined::new(
                supervisor(2, Strategy::OneForOne, RestartPolicy::Transient, 5, Duration::MAX),
                Some(due),
                |_| Ok(Step::Continue),
            ),
            |_| StashRoute::Deliver,
        ),
        stop_on_abnormal_death,
    );
    let Some((n, _)) = Behavior::fleet(&wrapped) else {
        panic!("wrappers forward the fleet query");
    };
    assert_eq!(n, 2, "the static fleet surfaces through every wrapper");
}

/// A LEGAL composition — `Watching<Deadlined<Base>>`: each layer routes its own
/// framework event and forwards the rest, and the deadline min-fold surfaces
/// through the outer watch layer.
#[tokio::test]
async fn composed_watching_over_deadlined_routes_each_source() {
    let due = Instant::now() + Duration::from_secs(3);
    let deadlined = Deadlined::new(recorder(), Some(due), |_| Ok(Step::Continue));
    let mut w = Watching::new(deadlined, stop_on_abnormal_death);

    assert_eq!(w.next_deadline(), Some(due), "the inner deadline surfaces through watch");
    assert!(matches!(w.step(user(5)).await.unwrap().become_, Step::Continue));
    // The deadline fire reaches the Deadlined layer through the outer forward.
    assert!(matches!(w.step(Envelope::Deadline).await.unwrap().become_, Step::Continue));
    assert_eq!(w.next_deadline(), None, "the inner slot fired once");
    // An abnormal death still propagates at the outer layer.
    assert!(matches!(
        w.step(Envelope::LinkDied { peer: MailAddr(8), outcome: Err(Crash::Panicked) })
            .await
            .unwrap()
            .become_,
        Step::Stop(Exit::LinkDied(MailAddr(8)))
    ));
    assert_eq!(w.inner().inner().state(), &vec![5]);
}
