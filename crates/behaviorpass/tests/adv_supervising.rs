//! Supervising invariant suite — the "adversarial" additions to
//! `tests/oracle.rs`. Targets the Supervising survivor (supervising.rs:108
//! `next_deadline` -> `None`) and pins the restart algebra: abnormal-within-
//! budget emits exactly ONE create and re-marks the slot live; normal /
//! exhausted / out-of-range produce zero creates and mark dead; budget
//! accounting is exact; user traffic passes through unchanged.
//!
//! Methods: handcrafted edges (budget 0 / 1 / MAX, out-of-range slots,
//! multi-child) + sequences + lifecycle + a differential property model +
//! long-sequence fuzz.

use std::time::Duration;

use behaviorpass::{Actions, Base, Behavior, Deadlined, Envelope, Exit, MailAddr, Supervising};
use bombay::capability::{Never, Step};
use proptest::prelude::*;
use tokio::time::Instant;

type Kid = Base<u32, u32, Never, &'static str>;

fn kid() -> Kid {
    Base::new(0_u32, |count: &mut u32, n: u32| {
        *count += n;
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    })
}

fn inner() -> Base<(), u64, Never, &'static str> {
    Base::new((), |(): &mut (), _: u64| Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont()))
}

fn supervisor(budget: u32) -> Supervising<Base<(), u64, Never, &'static str>, Kid> {
    Supervising::new(inner(), 1, |_| kid(), budget)
}

// ---------------------------------------------------------------------------
// Survivor 3: Supervising::next_deadline forwarding
// ---------------------------------------------------------------------------

fn deadlined_inner(due: Option<Instant>) -> Deadlined<Base<(), u64, Never, &'static str>> {
    Deadlined::new(inner(), due, |_| Ok(Step::Continue))
}

/// Supervising must forward the inner deadline — the `next_deadline -> None`
/// mutant (supervising.rs:108) breaks this.
#[tokio::test]
async fn supervising_next_deadline_forwards_inner() {
    let due = Instant::now() + Duration::from_secs(5);
    let sup = Supervising::new(deadlined_inner(Some(due)), 1, |_| kid(), 3);
    assert_eq!(sup.next_deadline(), Some(due), "Supervising must forward the inner deadline");
}

#[tokio::test]
async fn supervising_next_deadline_none_when_inner_none() {
    let sup = Supervising::new(deadlined_inner(None), 1, |_| kid(), 3);
    assert_eq!(sup.next_deadline(), None);
}

/// The min-fold law surfaces through Supervising: the earliest nested deadline
/// arms the timer, and a ChildStopped event does not disturb it.
#[tokio::test]
async fn supervising_next_deadline_min_folds_and_survives_events() {
    let t1 = Instant::now() + Duration::from_secs(1);
    let t2 = Instant::now() + Duration::from_secs(5);
    let inner_d = Deadlined::new(deadlined_inner(Some(t1)), Some(t2), |_| Ok(Step::Continue));
    let mut sup = Supervising::new(inner_d, 1, |_| kid(), 3);
    assert_eq!(sup.next_deadline(), Some(t1));

    // A supervision event is orthogonal to the deadline fold.
    let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: false }).await.expect("no error");
    assert!(actions.creates.is_empty());
    assert_eq!(sup.next_deadline(), Some(t1), "a child event does not disturb the deadline");

    // The Deadline event forwards inward and fires the outer slot.
    let actions = sup.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "the inner reaction's Continue rides out");
    assert_eq!(sup.next_deadline(), Some(t1), "the inner slot still arms after the outer fired");

    // A firing always lands on the outermost Deadlined layer (the one holding
    // t2): a second Deadline is absorbed there too, so the inner slot stays
    // armed — the min-fold keeps surfacing it.
    let _ = sup.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(sup.next_deadline(), Some(t1), "a disarmed outer layer still absorbs the Deadline event");
}

// ---------------------------------------------------------------------------
// Restart algebra: budget edges, slots, forwarding
// ---------------------------------------------------------------------------

/// Budget 0: an abnormal stop is final — zero creates, slot dead, budget 0.
#[tokio::test]
async fn supervising_zero_budget_never_restarts() {
    let mut sup = supervisor(0);
    let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert!(actions.creates.is_empty(), "zero budget ⇒ zero creates");
    assert!(!sup.children()[0].alive(), "zero budget ⇒ give up");
    assert_eq!(sup.restarts_left(), 0);
}

/// u32::MAX budget: restarts spend exactly one unit each, up to exhaustion.
#[tokio::test]
async fn supervising_budget_spends_exactly_one_per_restart() {
    let mut sup = supervisor(u32::MAX);
    for _ in 0..3 {
        let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
        assert_eq!(actions.creates.len(), 1, "each abnormal stop within budget emits one create");
        assert!(sup.children()[0].alive());
    }
    assert_eq!(sup.restarts_left(), u32::MAX - 3, "exactly one unit spent per restart");
}

/// Out-of-range slots are benign: no create, no panic, no budget spend, and
/// the rest of the table is untouched.
#[tokio::test]
async fn supervising_out_of_range_slot_is_benign() {
    let mut sup = supervisor(5);
    for idx in [1usize, 999, usize::MAX] {
        let actions = sup.step(Envelope::ChildStopped { idx, abnormal: true }).await.expect("no error");
        assert!(actions.creates.is_empty(), "out-of-range slot {idx} emits nothing");
    }
    assert_eq!(sup.restarts_left(), 5, "out-of-range stops spend no budget");
    assert!(sup.children()[0].alive(), "the in-range slot is untouched");
}

/// Multiple children: slots are independent — one child's death does not
/// disturb the others.
#[tokio::test]
async fn supervising_multi_child_slots_are_independent() {
    let mut sup = Supervising::new(inner(), 3, |_| kid(), 5);
    assert_eq!(sup.children().len(), 3, "n_children live slots at birth");
    assert!(sup.children().iter().all(|c| c.alive()));

    let actions = sup.step(Envelope::ChildStopped { idx: 1, abnormal: false }).await.expect("no error");
    assert!(actions.creates.is_empty(), "a normal stop emits nothing");
    assert!(sup.children()[0].alive());
    assert!(!sup.children()[1].alive(), "only the stopped slot dies");
    assert!(sup.children()[2].alive());

    let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert_eq!(actions.creates.len(), 1, "the abnormal slot restarts independently");
    assert!(sup.children()[0].alive(), "restart re-marks its own slot live");
    assert!(!sup.children()[1].alive(), "other slots unaffected");
    assert_eq!(sup.restarts_left(), 4);
}

/// An abnormal stop with budget re-marks the slot ALIVE and emits exactly one
/// create — the restart DECISION, one-for-one (already unit-covered; here with
/// exact slot/budget state across a full lifecycle: start → restart → exhaust).
#[tokio::test]
async fn supervising_lifecycle_start_restart_exhaust() {
    let mut sup = supervisor(2);
    // start: slot alive, budget 2
    assert!(sup.children()[0].alive());
    assert_eq!(sup.restarts_left(), 2);
    // abnormal → restart #1
    let a = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert_eq!(a.creates.len(), 1);
    assert_eq!(sup.restarts_left(), 1);
    // abnormal → restart #2 (last unit)
    let a = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert_eq!(a.creates.len(), 1);
    assert_eq!(sup.restarts_left(), 0);
    // abnormal → exhausted: dead, no create
    let a = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert!(a.creates.is_empty());
    assert!(!sup.children()[0].alive());
    assert_eq!(sup.restarts_left(), 0, "an exhausted budget never goes negative");
}

/// User traffic passes through Supervising unchanged: sends and become_ ride
/// out, creates stay empty, children untouched.
#[tokio::test]
async fn supervising_forwards_user_actions_unchanged() {
    let sender: Base<(), u64, Never, &'static str, u64, Never> = Base::new((), |(): &mut (), m: u64| {
        Ok::<Actions<Never, u64, Never>, &'static str>(Actions {
            sends: vec![(MailAddr(7), m)],
            creates: Vec::new(),
            become_: if m == 9 { Step::Stop(Exit::Normal) } else { Step::Continue },
        })
    });
    let mut sup = Supervising::new(sender, 2, |_| kid(), 3);
    let actions = sup.step(Envelope::User(4)).await.expect("no error");
    assert_eq!(actions.sends, vec![(MailAddr(7), 4)], "sends pass through unchanged");
    assert!(actions.creates.is_empty(), "Supervising creates nothing of its own");
    assert_eq!(actions.become_, Step::Continue);

    let actions = sup.step(Envelope::User(9)).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "an inner Stop rides out unchanged");
    assert_eq!(actions.sends, vec![(MailAddr(7), 9)]);
    assert!(sup.children()[0].alive() && sup.children()[1].alive(), "user traffic never touches children");
}

// ---------------------------------------------------------------------------
// Differential property model + fuzz
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum StopOp {
    Stop { idx: usize, abnormal: bool },
}

/// The model: exact budget + liveness accounting, the oracle for the SUT.
struct SupModel {
    alive: Vec<bool>,
    restarts_left: u32,
}

impl SupModel {
    fn new(n_children: usize, budget: u32) -> Self {
        Self { alive: vec![true; n_children], restarts_left: budget }
    }

    /// Fold one child stop; returns the creates the supervisor must emit.
    fn fold(&mut self, idx: usize, abnormal: bool) -> usize {
        let Some(slot) = self.alive.get_mut(idx) else {
            return 0;
        };
        if abnormal && self.restarts_left > 0 {
            self.restarts_left -= 1;
            *slot = true;
            1
        } else {
            *slot = false;
            0
        }
    }
}

fn child_stop_strategy(n_children: usize) -> impl proptest::strategy::Strategy<Value = StopOp> {
    use proptest::prelude::*;
    // in-range 0..n, exactly n (first out-of-range), and usize::MAX.
    prop_oneof![
        0..n_children,
        Just(n_children),
        Just(n_children.saturating_add(999)),
        Just(usize::MAX),
    ]
    .prop_flat_map(|idx| (Just(idx), any::<bool>()))
    .prop_map(|(idx, abnormal)| StopOp::Stop { idx, abnormal })
}

fn fold_supervising_and_check(rt: &tokio::runtime::Runtime, n_children: usize, budget: u32, ops: &[StopOp]) {
    let mut model = SupModel::new(n_children, budget);
    let mut sup = Supervising::new(inner(), n_children, |_| kid(), budget);
    rt.block_on(async {
        for (i, op) in ops.iter().copied().enumerate() {
            let StopOp::Stop { idx, abnormal } = op;
            let actions = sup.step(Envelope::ChildStopped { idx, abnormal }).await.expect("no error");
            let expected = model.fold(idx, abnormal);
            assert_eq!(actions.creates.len(), expected, "op #{i}: create count");
            assert!(actions.sends.is_empty(), "op #{i}: supervision emits no sends");
            assert_eq!(actions.become_, Step::Continue, "op #{i}: supervision never becomes");
            assert_eq!(sup.restarts_left(), model.restarts_left, "op #{i}: budget accounting");
            for (j, (slot, alive)) in sup.children().iter().zip(&model.alive).enumerate() {
                assert_eq!(slot.alive(), *alive, "op #{i}: slot {j} liveness");
            }
        }
    });
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Random child-stop scripts against budgets and table sizes incl. the
    /// boundary budgets 0 / 1 / MAX and out-of-range slots: create count,
    /// budget spend, and every slot's liveness match the independent model
    /// after every event.
    #[test]
    fn prop_supervising_matches_differential_model(
        n_children in 0usize..=4,
        budget in prop_oneof![Just(0u32), Just(1u32), Just(u32::MAX), any::<u32>()],
        ops in proptest::collection::vec(child_stop_strategy(4), 0..=20),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_supervising_and_check(&rt, n_children, budget, &ops);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 64, ..proptest::prelude::ProptestConfig::default() })]

    /// Long interleavings: no panic, no negative budget, liveness stays
    /// model-equal throughout.
    #[test]
    fn prop_supervising_long_sequences_model_equal(
        n_children in 1usize..=8,
        budget in prop_oneof![Just(0u32), Just(1u32), Just(u32::MAX), any::<u32>()],
        ops in proptest::collection::vec(child_stop_strategy(8), 0..=64),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_supervising_and_check(&rt, n_children, budget, &ops);
    }
}
