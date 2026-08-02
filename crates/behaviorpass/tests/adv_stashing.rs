//! Stashing invariant suite — the "adversarial" additions to `tests/oracle.rs`.
//! Targets the two Stashing survivors (stashing.rs:60 `drain_into` -> `Ok(())`;
//! stashing.rs:118 `next_deadline` -> `None`) and pins the buffer algebra:
//! release re-routes the held SNAPSHOT (no livelock), effects accumulate in
//! order, Stop mid-drain abandons + re-holds, deadline forwarding, FIFO ahead
//! of backlog.
//!
//! Under a pure per-message route a held message can never be re-delivered
//! (the route is deterministic: what stashed stays Stash-class), so
//! `drain_into`'s deliver/Stop arms are exercised with interior-mutable
//! messages (`Arc<AtomicU64>`): a held message becomes Deliver-class at drain
//! time — the only way the drain fold is observable. That is exactly the
//! surface the `drain_into -> Ok(())` mutant destroys.
//!
//! Methods: handcrafted edges + sequences + lifecycle + a differential
//! property model + long-sequence fuzz.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use behaviorpass::{Actions, Base, Behavior, Deadlined, Envelope, Exit, MailAddr, StashRoute, Stashing};
use bombay::capability::{Never, Step};

use tokio::time::Instant;

// ---------------------------------------------------------------------------
// Survivor 1: drain_into effect accumulation, via interior-mutable messages
// ---------------------------------------------------------------------------

/// A message whose classification can change between stash and release: the
/// route reads the CURRENT value, so a held message can become Deliver-class.
#[derive(Clone)]
struct M(Arc<AtomicU64>);

fn value(m: &M) -> u64 {
    m.0.load(Ordering::Relaxed)
}

/// 0 = stash, 1 = release trigger, anything else = deliver.
fn mutable_route(m: &M) -> StashRoute {
    match value(m) {
        0 => StashRoute::Stash,
        1 => StashRoute::Release,
        _ => StashRoute::Deliver,
    }
}

fn sender_inner() -> Base<Vec<u64>, M, Never, &'static str, u64, Never> {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, m: M| {
        let id = value(&m);
        seen.push(id);
        let become_ = if id == 3 {
            Step::Stop(Exit::Normal)
        } else {
            Step::Continue
        };
        Ok::<Actions<Never, u64, Never>, &'static str>(Actions {
            sends: vec![(MailAddr(id), id)],
            creates: Vec::new(),
            become_,
        })
    })
}

fn msg(v: u64) -> M {
    M(Arc::new(AtomicU64::new(v)))
}

/// A Release delivers the current message, then replays the held batch,
/// ACCUMULATING each delivered step's sends in order. The `drain_into ->
/// Ok(())` mutant drops the replay: the held message is never folded.
#[tokio::test]
async fn stashing_release_accumulates_drained_sends_in_order() {
    let mut s = Stashing::new(sender_inner(), mutable_route);

    let m1 = msg(0);
    let _ = s.step(Envelope::User(m1.clone())).await; // stash (value 0)
    assert_eq!(s.held(), 1);
    m1.0.store(2, Ordering::Relaxed); // becomes Deliver-class before the release

    let actions = s.step(Envelope::User(msg(1))).await.expect("no error");
    assert_eq!(
        actions.sends,
        vec![(MailAddr(1), 1), (MailAddr(2), 2)],
        "the release folds the current message THEN the drained batch, in order"
    );
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(s.inner().state(), &vec![1, 2], "drain effects accumulate in fold order");
    assert_eq!(s.held(), 0, "a delivered held message leaves the buffer");
}

/// The same accumulation for CREATES.
#[tokio::test]
async fn stashing_release_accumulates_drained_creates_in_order() {
    let inner: Base<Vec<u64>, M, Never, &'static str, Never, u32> =
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, m: M| {
            let id = value(&m);
            seen.push(id);
            Ok::<Actions<Never, Never, u32>, &'static str>(Actions {
                sends: Vec::new(),
                creates: vec![id as u32],
                become_: Step::Continue,
            })
        });
    let mut s = Stashing::new(inner, mutable_route);

    let m1 = msg(0);
    let _ = s.step(Envelope::User(m1.clone())).await; // stash
    m1.0.store(2, Ordering::Relaxed);

    let actions = s.step(Envelope::User(msg(1))).await.expect("no error");
    assert_eq!(actions.creates, vec![1, 2], "creates accumulate in fold order");
    assert_eq!(s.held(), 0);
}

/// A Stop DURING the drain sets the verdict and re-holds the abandoned tail:
/// the `drain_into -> Ok(())` mutant loses both the Stop and the re-hold.
#[tokio::test]
async fn stashing_stop_mid_drain_abandons_the_rest_and_re_holds() {
    let mut s = Stashing::new(sender_inner(), mutable_route);

    let m3 = msg(0); // will become the stop trigger
    let m1 = msg(0); // will become deliverable
    let _ = s.step(Envelope::User(m3.clone())).await; // held=[m3]
    let _ = s.step(Envelope::User(m1.clone())).await; // held=[m3,m1]
    m3.0.store(3, Ordering::Relaxed); // inner stops on 3
    m1.0.store(2, Ordering::Relaxed); // inner delivers on 2

    let actions = s.step(Envelope::User(msg(1))).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::Normal),
        "a Stop during the drain rides out of the release"
    );
    assert_eq!(
        actions.sends,
        vec![(MailAddr(1), 1), (MailAddr(3), 3)],
        "the current message and the stop message fold; the tail never does"
    );
    assert_eq!(s.inner().state(), &vec![1, 3], "the tail (value 2) was abandoned");
    assert_eq!(s.held(), 1, "the abandoned tail re-enters held");
}

/// A Release whose CURRENT message stops skips the drain entirely — the held
/// batch is untouched.
#[tokio::test]
async fn stashing_release_stop_on_current_message_skips_drain() {
    let mut s = Stashing::new(sender_inner(), mutable_route);
    let held = msg(0);
    let _ = s.step(Envelope::User(held.clone())).await; // held=[held]

    let actions = s.step(Envelope::User(msg(1))).await.expect("no error");
    // value 1 = Release, but the sender stops only on 3 — so no Stop here.
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(s.held(), 1, "held untouched when the release has nothing to drain");

    // Now make the CURRENT message stop: release trigger value 1 stays, but
    // stop only fires on 3 — use value 3 as the release trigger instead.
    let mut s2 = Stashing::new(sender_inner(), |m: &M| {
        if value(m) == 3 { StashRoute::Release } else { StashRoute::Stash }
    });
    let held2 = msg(7);
    let _ = s2.step(Envelope::User(held2.clone())).await; // held=[7]
    let actions = s2.step(Envelope::User(msg(3))).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::Normal),
        "a current-message Stop returns before any drain"
    );
    assert_eq!(s2.held(), 1, "the held batch is not drained past a current-message Stop");
}

/// Release with an empty held batch just delivers the current message.
#[tokio::test]
async fn stashing_release_with_empty_held_delivers_current() {
    let mut s = Stashing::new(sender_inner(), mutable_route);
    let actions = s.step(Envelope::User(msg(1))).await.expect("no error");
    assert_eq!(actions.sends, vec![(MailAddr(1), 1)]);
    assert_eq!(s.inner().state(), &vec![1]);
    assert_eq!(s.held(), 0);
}

/// Snapshot bound: an all-restash route re-holds the whole batch and the
/// release terminates (no livelock) — each stashed message is re-routed ONCE.
#[tokio::test]
async fn stashing_release_re_stashes_under_snapshot_bound() {
    let recorder: Base<Vec<u64>, u64, Never, &'static str> = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    let mut s = Stashing::new(recorder, |&id| {
        if id == 0 { StashRoute::Release } else { StashRoute::Stash }
    });
    for id in [1_u64, 2, 3] {
        let _ = s.step(Envelope::User(id)).await; // held=[1,2,3]
    }
    let actions = s.step(Envelope::User(0)).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "an all-restash release still terminates");
    assert_eq!(s.inner().state(), &vec![0], "only the current message folded");
    assert_eq!(s.held(), 3, "the whole batch re-enters held, once each");
}

// ---------------------------------------------------------------------------
// Survivor 2: Stashing::next_deadline forwarding
// ---------------------------------------------------------------------------

fn flake_inner(due: Option<Instant>) -> Deadlined<Base<(), u64, Never, &'static str>> {
    let base: Base<(), u64, Never, &'static str> = Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    Deadlined::new(base, due, |_inner| Ok(Step::Stop(Exit::Normal)))
}

/// Stashing must forward the inner deadline — the `next_deadline -> None`
/// mutant (stashing.rs:118) breaks this.
#[tokio::test]
async fn stashing_next_deadline_forwards_inner() {
    let due = Instant::now() + Duration::from_secs(5);
    let mut s = Stashing::new(flake_inner(Some(due)), |&_| StashRoute::Deliver);
    assert_eq!(s.next_deadline(), Some(due), "Stashing must forward the inner deadline");

    // The Deadline event forwards inward through Stashing and fires there.
    let actions = s.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "the inner reaction rides out");
    assert_eq!(s.next_deadline(), None, "fires once — the forwarded slot cleared");
}

#[tokio::test]
async fn stashing_next_deadline_none_when_inner_none() {
    let s = Stashing::new(flake_inner(None), |&_| StashRoute::Deliver);
    assert_eq!(s.next_deadline(), None);
}

/// The min-fold law surfaces through Stashing: with two nested deadlines, the
/// EARLIEST one arms the timer, and each layer clears on its own firing.
#[tokio::test]
async fn stashing_next_deadline_min_folds_through_layers() {
    let t1 = Instant::now() + Duration::from_secs(1);
    let t2 = Instant::now() + Duration::from_secs(5);
    let inner_d = Deadlined::new(flake_inner(Some(t1)), Some(t2), |_| Ok(Step::Continue));
    let mut s = Stashing::new(inner_d, |&_| StashRoute::Deliver);
    assert_eq!(s.next_deadline(), Some(t1), "the min of the nested slots rides out");

    let _ = s.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(s.next_deadline(), Some(t1), "the inner slot still arms after the outer fired");

    // A firing always lands on the outermost Deadlined layer (here: the one
    // holding t2): a second Deadline is absorbed there too, so the inner slot
    // stays armed — the min-fold keeps surfacing it.
    let _ = s.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(s.next_deadline(), Some(t1), "a disarmed outer layer still absorbs the Deadline event");
}

// ---------------------------------------------------------------------------
// Differential property model + fuzz
// ---------------------------------------------------------------------------

fn pure_route(id: &u64) -> StashRoute {
    match id {
        0 => StashRoute::Release,
        n if n % 2 == 1 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    }
}

fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    })
}

/// The independent model: an obviously-correct fold of the same script.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StashModel {
    seen: Vec<u64>,
    held: VecDeque<u64>,
}

impl StashModel {
    fn new() -> Self {
        Self { seen: Vec::new(), held: VecDeque::new() }
    }

    fn fold(&mut self, id: u64) {
        match pure_route(&id) {
            StashRoute::Stash => self.held.push_back(id),
            StashRoute::Deliver => self.seen.push(id),
            StashRoute::Release => {
                self.seen.push(id); // the current message is delivered first
                let batch: VecDeque<u64> = self.held.drain(..).collect();
                for m in batch {
                    match pure_route(&m) {
                        StashRoute::Stash => self.held.push_back(m),
                        StashRoute::Deliver | StashRoute::Release => self.seen.push(m),
                    }
                }
            }
        }
    }
}

fn id_strategy() -> impl proptest::strategy::Strategy<Value = u64> {
    use proptest::prelude::*;
    prop_oneof![
        Just(0),
        Just(1),
        Just(u64::MAX),
        any::<u64>(),
    ]
}

fn fold_stash_script_and_check(rt: &tokio::runtime::Runtime, ids: &[u64]) {
    let mut model = StashModel::new();
    let mut s = Stashing::new(recorder(), pure_route);
    rt.block_on(async {
        for (i, id) in ids.iter().copied().enumerate() {
            let actions = s.step(Envelope::User(id)).await.expect("no error");
            assert_eq!(actions.become_, Step::Continue, "op #{i}");
            assert!(actions.sends.is_empty(), "op #{i}: the recorder sends nothing");
            model.fold(id);
            assert_eq!(s.inner().state(), &model.seen, "op #{i}: fold order equality");
            assert_eq!(s.held(), model.held.len(), "op #{i}: held count equality");
        }
    });
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Any stash/deliver/release script (ids incl. boundaries 0 / 1 / MAX,
    /// scripts empty and max-length) folds identically through the SUT and the
    /// independent model: release = current first, then the held snapshot,
    /// re-stashed under the bound.
    #[test]
    fn prop_stashing_fold_matches_differential_model(ids in proptest::collection::vec(id_strategy(), 0..=16)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_stash_script_and_check(&rt, &ids);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 64, ..proptest::prelude::ProptestConfig::default() })]

    /// Long interleavings with a stash-heavy route (livelock / bound surface):
    /// the fold stays model-equal and terminates; held never exceeds the
    /// number of stashed messages.
    #[test]
    fn prop_stashing_long_sequences_terminate_model_equal(ids in proptest::collection::vec(id_strategy(), 0..=64)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_stash_script_and_check(&rt, &ids);
    }

    /// All-restash route with occasional releases: every release re-holds the
    /// entire snapshot exactly once (count equality), and the step terminates.
    #[test]
    fn prop_stashing_all_restash_snapshot_bound(ids in proptest::collection::vec(id_strategy(), 0..=64)) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let route = |&id: &u64| if id == 0 { StashRoute::Release } else { StashRoute::Stash };
        let mut s = Stashing::new(recorder(), route);
        let mut held = 0usize;
        rt.block_on(async {
            for (i, id) in ids.iter().copied().enumerate() {
                let actions = s.step(Envelope::User(id)).await.expect("no error");
                assert_eq!(actions.become_, Step::Continue, "op #{i}: terminates");
                if id != 0 {
                    held += 1; // stash
                } // release: current folds, the held snapshot re-holds whole
                assert_eq!(s.held(), held, "op #{i}: snapshot bound — nothing lost, nothing duplicated");
            }
        });
    }
}
