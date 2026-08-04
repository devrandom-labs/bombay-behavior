//! Supervising invariant suite — the "adversarial" additions to
//! `tests/oracle.rs`, re-gated for card 1 (address-parameterized grammar,
//! nonce-keyed children, outcome-carrying death legs, and the generalized
//! strategy × policy × windowed-budget supervision space). Pins the restart
//! ALGEBRA beyond the oracle's canonical laws: budget edges, exact windowed
//! accounting via `restarts_in_window()`, candidate-set edges (already-dead
//! slots are never candidates), the policy × outcome matrix, dynamic births,
//! and the zero / count-only window extremes — plus a differential property
//! model and a long-sequence fuzz. The survivor coverage (`next_deadline`
//! forwarding and its orthogonality to supervision events) rides along under
//! the new envelope shapes.
//!
//! NOT here (the oracle owns them): the unknown-nonce / stale-birth /
//! inner-Restart programmer-bug panics. Unknown nonces are therefore
//! EXCLUDED from the generated op space — a `ChildStopped` naming one
//! panics now; the old "out-of-range slot is benign" law is dead, and with
//! it `restarts_left()` (the budget is windowed; `restarts_in_window()` is
//! the observable).

use std::time::Duration;

use behaviorpass::{
    Actions, Base, Behavior, Crash, Create, Deadlined, Envelope, Exit, MailAddr, RestartPolicy,
    Strategy, Supervising, Target,
};
use behaviorpass::{Never, Step};
use proptest::prelude::*;
// `behaviorpass::Strategy` (the enum) shadows the prelude's trait of the
// same name; bring the trait's methods back into scope anonymously.
use proptest::strategy::Strategy as _;
use tokio::time::Instant;

type Kid = Base<MailAddr, u32, u32, Never, &'static str>;
/// The inner's Offspring IS the child menu (the relaxed bound): it creates
/// nothing at runtime, but the type agrees with the fleet.
type SupInner = Base<MailAddr, (), u64, Never, &'static str, Never, Kid>;

fn kid() -> Kid {
    Base::new(0_u32, |count: &mut u32, n: u32| {
        *count += n;
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

/// The restart nonces a step emitted, in emission order — the
/// restart-decision vocabulary: a restart is never a bare birth, and the
/// nonce ties the decision to the surviving mailbox the driver will swap.
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

// ---------------------------------------------------------------------------
// Survivor: Supervising::next_deadline forwarding
// ---------------------------------------------------------------------------

fn deadlined_inner(due: Option<Instant>) -> Deadlined<SupInner> {
    Deadlined::new(sup_inner(), due, |_| Ok(Step::Continue))
}

/// Supervising must forward the inner deadline.
#[tokio::test]
async fn supervising_next_deadline_forwards_inner() {
    let due = Instant::now() + Duration::from_secs(5);
    let sup = Supervising::new(
        deadlined_inner(Some(due)),
        |i| i as u64,
        1,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        3,
        Duration::MAX,
    );
    assert_eq!(sup.next_deadline(), Some(due), "Supervising must forward the inner deadline");
}

#[tokio::test]
async fn supervising_next_deadline_none_when_inner_none() {
    let sup = Supervising::new(
        deadlined_inner(None),
        |i| i as u64,
        1,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        3,
        Duration::MAX,
    );
    assert_eq!(sup.next_deadline(), None);
}

/// The min-fold law surfaces through Supervising: the earliest nested deadline
/// arms the timer, and a `ChildStopped` event does not disturb it.
#[tokio::test]
async fn supervising_next_deadline_min_folds_and_survives_events() {
    let t1 = Instant::now() + Duration::from_secs(1);
    let t2 = Instant::now() + Duration::from_secs(5);
    let inner_d = Deadlined::new(deadlined_inner(Some(t1)), Some(t2), |_| Ok(Step::Continue));
    let mut sup = Supervising::new(
        inner_d,
        |i| i as u64,
        1,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        3,
        Duration::MAX,
    );
    assert_eq!(sup.next_deadline(), Some(t1));

    // A supervision event is orthogonal to the deadline fold: a normal stop
    // emits nothing and leaves the armed deadline alone.
    let actions = sup.step(stopped(0, Ok(Exit::Normal), Instant::now())).await.expect("no error");
    assert!(actions.creates.is_empty(), "a normal stop emits nothing");
    assert!(!sup.is_alive(0), "the stopped slot dies");
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
// Restart algebra: budget edges, candidate sets, policy matrix, forwarding
// ---------------------------------------------------------------------------

/// Budget 0: a stop is final no matter the outcome or policy — zero creates,
/// slot dead, zero units spent.
#[tokio::test]
async fn supervising_zero_budget_never_restarts() {
    let t0 = Instant::now();
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 0, Duration::MAX);
    let actions = sup.step(stopped(0, Err(Crash::Panicked), t0)).await.expect("no error");
    assert!(actions.creates.is_empty(), "zero budget ⇒ zero creates, even on a crash");
    assert!(!sup.is_alive(0), "zero budget ⇒ give up");
    assert_eq!(sup.restarts_in_window(), 0);

    // Even Permanent cannot restart without budget.
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Permanent, 0, Duration::MAX);
    let actions = sup.step(stopped(0, Ok(Exit::Normal), t0)).await.expect("no error");
    assert!(actions.creates.is_empty(), "permanent still needs budget");
    assert!(!sup.is_alive(0));
    assert_eq!(sup.restarts_in_window(), 0);
}

/// Exact budget accounting across a full lifecycle: every admitted restart
/// spends exactly one unit; normal stops and budget-denied stops spend none.
#[tokio::test]
async fn supervising_budget_accounting_is_exact_across_lifecycle() {
    let t0 = Instant::now();
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, u32::MAX, Duration::MAX);
    for i in 0..3 {
        let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
        assert_eq!(restart_nonces(&actions), vec![0]);
        assert!(sup.is_alive(0), "a restart re-marks its own slot live");
        assert_eq!(sup.restarts_in_window(), i + 1, "each admitted restart spends exactly one unit");
    }
    let actions = sup.step(stopped(0, Ok(Exit::Normal), t0)).await.expect("no error");
    assert!(actions.creates.is_empty());
    assert_eq!(sup.restarts_in_window(), 3, "a normal stop spends nothing");

    // Budget 1: the first restart spends the only unit; the second abnormal
    // stop is denied and spends nothing.
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 1, Duration::MAX);
    let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![0]);
    assert_eq!(sup.restarts_in_window(), 1);
    let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert!(actions.creates.is_empty(), "an exhausted budget never goes negative");
    assert!(!sup.is_alive(0));
    assert_eq!(sup.restarts_in_window(), 1, "a denied stop spends nothing");
}

/// Multiple children under `OneForOne`: slots are independent — one child's
/// death does not disturb the others.
#[tokio::test]
async fn supervising_multi_child_slots_are_independent() {
    let t0 = Instant::now();
    let mut sup = supervisor(3, Strategy::OneForOne, RestartPolicy::Transient, 5, Duration::MAX);
    assert_eq!(sup.child_count(), 3, "n_children live slots at birth");
    assert!((0..3).all(|n| sup.is_alive(n)));

    let actions = sup.step(stopped(1, Ok(Exit::Normal), t0)).await.expect("no error");
    assert!(actions.creates.is_empty(), "a normal stop emits nothing");
    assert!(sup.is_alive(0));
    assert!(!sup.is_alive(1), "only the stopped slot dies");
    assert!(sup.is_alive(2));

    let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![0]);
    assert!(sup.is_alive(0), "restart re-marks its own slot live");
    assert!(!sup.is_alive(1), "other slots unaffected");
    assert!(sup.is_alive(2));
    assert_eq!(sup.restarts_in_window(), 1);
}

/// `OneForAll` candidate-set edge: an ALREADY-DEAD slot is never a candidate.
/// Kill 0 normally (it dies, no restart under Transient), then kill 1
/// abnormally with budget to spare: the candidate set is the LIVE children
/// {1, 2} — NOT 0 — emitted dead-first then in birth-sequence order.
#[tokio::test]
async fn supervising_one_for_all_skips_already_dead_slots() {
    let t0 = Instant::now();
    let mut sup = supervisor(3, Strategy::OneForAll, RestartPolicy::Transient, 10, Duration::MAX);
    let actions = sup.step(stopped(0, Ok(Exit::Normal), t0)).await.expect("no error");
    assert!(actions.creates.is_empty(), "transient never restarts a normal stop");
    assert!(!sup.is_alive(0));

    let actions = sup.step(stopped(1, Err(Crash::Panicked), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![1, 2], "live candidates only: dead child first, then seq order");
    assert!(!sup.is_alive(0), "the already-dead slot is not re-marked");
    assert!(sup.is_alive(1) && sup.is_alive(2), "the live candidates are re-marked live");
    assert_eq!(sup.restarts_in_window(), 2, "one unit per restarted candidate — the dead slot costs nothing");
}

/// `RestForOne` candidate-set edge: candidates are the LIVE children born at
/// seq >= the dead child's seq — an already-dead YOUNGER sibling is skipped.
#[tokio::test]
async fn supervising_rest_for_one_skips_dead_younger_sibling() {
    let t0 = Instant::now();
    let mut sup = supervisor(4, Strategy::RestForOne, RestartPolicy::Transient, 10, Duration::MAX);
    // Kill the youngest (nonce 3, seq 3) normally: it dies and stays dead.
    let actions = sup.step(stopped(3, Ok(Exit::Normal), t0)).await.expect("no error");
    assert!(actions.creates.is_empty());
    assert!(!sup.is_alive(3));

    // Kill nonce 1 (seq 1) abnormally: live candidates with seq >= 1 are
    // {1, 2} — the dead seq-3 sibling is skipped, the seq-0 elder untouched.
    let actions = sup.step(stopped(1, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![1, 2], "dead first, then seq order; the dead younger sibling is skipped");
    assert!(sup.is_alive(0), "an older sibling is never a candidate");
    assert!(sup.is_alive(1) && sup.is_alive(2));
    assert!(!sup.is_alive(3), "the skipped sibling stays dead");
    assert_eq!(sup.restarts_in_window(), 2);
}

/// The policy × outcome matrix, in one sweep: Permanent restarts EVERY
/// outcome (normal and crash alike); Temporary restarts NONE; Transient
/// restarts exactly the abnormal ones (`Err(Crash)` in either domain, or an
/// `Ok` exit outside `{Normal, Collected}`).
#[tokio::test]
async fn supervising_policy_matrix_gates_every_outcome() {
    let t0 = Instant::now();
    let outcomes: [(Result<Exit<MailAddr>, Crash>, bool); 5] = [
        (Ok(Exit::Normal), false),
        (Ok(Exit::Collected), false),
        (Ok(Exit::LinkDied(MailAddr(9))), true),
        (Err(Crash::Failed), true),
        (Err(Crash::Panicked), true),
    ];
    for policy in [RestartPolicy::Permanent, RestartPolicy::Transient, RestartPolicy::Temporary] {
        for (outcome, abnormal) in outcomes {
            let mut sup = supervisor(1, Strategy::OneForOne, policy, 5, Duration::MAX);
            let actions = sup.step(stopped(0, outcome, t0)).await.expect("no error");
            let expect_restart = match policy {
                RestartPolicy::Permanent => true,
                RestartPolicy::Transient => abnormal,
                RestartPolicy::Temporary => false,
            };
            if expect_restart {
                assert_eq!(restart_nonces(&actions), vec![0], "{policy:?} must restart {outcome:?}");
                assert!(sup.is_alive(0), "{policy:?} re-marks the slot live after {outcome:?}");
                assert_eq!(sup.restarts_in_window(), 1, "{policy:?} spends one unit on {outcome:?}");
            } else {
                assert!(actions.creates.is_empty(), "{policy:?} never restarts {outcome:?}");
                assert!(!sup.is_alive(0), "{policy:?} lets the slot die after {outcome:?}");
                assert_eq!(sup.restarts_in_window(), 0, "{policy:?} spends nothing on {outcome:?}");
            }
        }
    }
}

/// User traffic passes through Supervising unchanged: sends (as
/// `Target::Global`) and become_ ride out, creates stay empty, children
/// untouched.
#[tokio::test]
async fn supervising_forwards_user_actions_unchanged() {
    let sender: Base<MailAddr, (), u64, Never, &'static str, u64, Kid> =
        Base::new((), |(): &mut (), m: u64| {
            Ok::<Actions<MailAddr, Never, u64, Kid>, &'static str>(Actions {
                sends: vec![(Target::Global(MailAddr(7)), m)],
                creates: Vec::new(),
                become_: if m == 9 { Step::Stop(Exit::Normal) } else { Step::Continue },
            })
        });
    let mut sup = Supervising::new(
        sender,
        |i| i as u64,
        2,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        3,
        Duration::MAX,
    );
    let actions = sup.step(Envelope::User { from: MailAddr(1), msg: 4 }).await.expect("no error");
    assert_eq!(actions.sends, vec![(Target::Global(MailAddr(7)), 4)], "sends pass through unchanged");
    assert!(actions.creates.is_empty(), "Supervising creates nothing of its own");
    assert_eq!(actions.become_, Step::Continue);

    let actions = sup.step(Envelope::User { from: MailAddr(1), msg: 9 }).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "an inner Stop rides out unchanged");
    assert_eq!(actions.sends, vec![(Target::Global(MailAddr(7)), 9)]);
    assert!(sup.is_alive(0) && sup.is_alive(1), "user traffic never touches children");
}

/// An inner-emitted Birth is recorded and forwarded AS-IS (the creator-minted
/// nonce is never rewritten), the liveness table grows, and the dynamic child
/// is then restartable through a `ChildStopped` naming its nonce.
#[tokio::test]
async fn supervising_dynamic_birth_is_recorded_and_restartable() {
    let t0 = Instant::now();
    let birthing: SupInner = Base::new((), |(): &mut (), m: u64| {
        let creates = if m == 42 {
            vec![Create::Birth { nonce: 7, child: kid() }]
        } else {
            Vec::new()
        };
        Ok(Actions { sends: Vec::new(), creates, become_: Step::Continue })
    });
    let mut sup = Supervising::new(
        birthing,
        |i| i as u64,
        2,
        |_| kid(),
        Strategy::OneForOne,
        RestartPolicy::Transient,
        5,
        Duration::MAX,
    );
    let actions = sup.step(Envelope::User { from: MailAddr(1), msg: 42 }).await.expect("no error");
    let [Create::Birth { nonce, .. }] = actions.creates.as_slice() else {
        panic!("a fresh dynamic birth is emitted as-is, never rewritten");
    };
    assert_eq!(*nonce, 7, "the birth keeps the creator-minted nonce");
    assert_eq!(sup.child_count(), 3, "the dynamic birth joins the table");
    assert!(sup.is_alive(7), "a dynamic child is born live");

    // The dynamic child participates in the restart algebra by its nonce.
    let actions = sup.step(stopped(7, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![7], "the dynamic child restarts by its nonce");
    assert!(sup.is_alive(7));
    assert_eq!(sup.restarts_in_window(), 1);
}

/// Window edge — `Duration::ZERO`: only same-instant restarts count; ANY
/// later stamp evicts every prior restart.
#[tokio::test]
async fn supervising_zero_window_counts_only_same_instant() {
    let t0 = Instant::now();
    // Same-instant restarts still occupy the budget.
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 1, Duration::ZERO);
    let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![0]);
    let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert!(actions.creates.is_empty(), "the same-instant restart still occupies the budget");
    assert!(!sup.is_alive(0));
    assert_eq!(sup.restarts_in_window(), 1, "the denied stop spent nothing");

    // One millisecond later, the zero window has evicted everything.
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 1, Duration::ZERO);
    let _ = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(sup.restarts_in_window(), 1);
    let actions =
        sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_millis(1))).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![0], "a later stamp always evicts under a zero window");
    assert_eq!(sup.restarts_in_window(), 1, "the evicted entry no longer counts");
}

/// Window edge — `Duration::MAX`: the count-only case. No eviction EVER, no
/// matter how far apart the stamps; the budget exhausts permanently.
#[tokio::test]
async fn supervising_max_window_never_evicts() {
    let t0 = Instant::now();
    let mut sup = supervisor(1, Strategy::OneForOne, RestartPolicy::Transient, 2, Duration::MAX);
    let actions = sup.step(stopped(0, Err(Crash::Failed), t0)).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![0]);
    let actions =
        sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_hours(1))).await.expect("no error");
    assert_eq!(restart_nonces(&actions), vec![0], "an hour later, nothing has evicted");
    assert_eq!(sup.restarts_in_window(), 2);
    let actions =
        sup.step(stopped(0, Err(Crash::Failed), t0 + Duration::from_hours(2))).await.expect("no error");
    assert!(actions.creates.is_empty(), "a count-only budget exhausts permanently");
    assert!(!sup.is_alive(0));
    assert_eq!(sup.restarts_in_window(), 2);
}

// ---------------------------------------------------------------------------
// Differential property model + fuzz (OneForOne × Transient × windowed budget)
// ---------------------------------------------------------------------------

/// A generated stop outcome, spanning the classification space: Normal and
/// Collected classify NORMAL; `LinkDied` and both Crash domains classify
/// ABNORMAL.
#[derive(Debug, Clone, Copy)]
enum Outcome {
    Normal,
    Collected,
    LinkDied,
    Failed,
    Panicked,
}

impl Outcome {
    fn into_envelope(self) -> Result<Exit<MailAddr>, Crash> {
        match self {
            Self::Normal => Ok(Exit::Normal),
            Self::Collected => Ok(Exit::Collected),
            Self::LinkDied => Ok(Exit::LinkDied(MailAddr(9))),
            Self::Failed => Err(Crash::Failed),
            Self::Panicked => Err(Crash::Panicked),
        }
    }

    fn is_abnormal(self) -> bool {
        matches!(self, Self::LinkDied | Self::Failed | Self::Panicked)
    }
}

/// One supervision event: a stop at a KNOWN nonce (unknown nonces are a
/// programmer-bug panic — the oracle owns that law — so the op space never
/// leaves the fleet), a weighted outcome, and a monotonically accumulating
/// stamp delta in logical millis (bounded so `t0 + elapsed` never overflows).
#[derive(Debug, Clone, Copy)]
struct StopOp {
    nonce: u64,
    outcome: Outcome,
    stamp_delta_ms: u64,
}

/// The model: exact liveness + windowed-budget accounting, the oracle for
/// the SUT. Timestamps are logical millis since t0; `u64::MAX` is the
/// count-only window (no eviction — the prune is skipped wholesale, exactly
/// as the SUT skips it for `Duration::MAX`).
struct SupModel {
    alive: Vec<bool>,
    restarts: Vec<u64>,
    max_restarts: u32,
    window_ms: u64,
}

impl SupModel {
    fn new(n_children: usize, max_restarts: u32, window_ms: u64) -> Self {
        Self { alive: vec![true; n_children], restarts: Vec::new(), max_restarts, window_ms }
    }

    /// Fold one child stop; returns the creates the supervisor must emit.
    /// Mirrors the SUT's order: the Transient policy gate FIRST (a normal
    /// outcome marks dead without touching the window), then eviction
    /// (`age <= window` keeps counting), then the all-or-nothing check
    /// against the one-candidate set.
    fn fold(&mut self, nonce: u64, abnormal: bool, now_ms: u64) -> usize {
        let idx = usize::try_from(nonce).expect("fleet nonces fit usize");
        if !abnormal {
            self.alive[idx] = false;
            return 0;
        }
        if self.window_ms != u64::MAX {
            self.restarts.retain(|&ts| now_ms - ts <= self.window_ms);
        }
        if self.restarts.len() < self.max_restarts as usize {
            self.restarts.push(now_ms);
            self.alive[idx] = true;
            1
        } else {
            self.alive[idx] = false;
            0
        }
    }
}

fn outcome_strategy() -> impl proptest::strategy::Strategy<Value = Outcome> {
    prop_oneof![
        2 => Just(Outcome::Normal),
        2 => Just(Outcome::Collected),
        2 => Just(Outcome::LinkDied),
        3 => Just(Outcome::Failed),
        3 => Just(Outcome::Panicked),
    ]
}

/// Budget edges first: 0 (never restarts), 1 (single unit), MAX (effectively
/// unbounded), then anything.
fn budget_strategy() -> impl proptest::strategy::Strategy<Value = u32> {
    prop_oneof![Just(0_u32), Just(1_u32), Just(u32::MAX), any::<u32>()]
}

/// Window edges first, in logical millis: 0 (only the same instant counts),
/// 10 (a small window — eviction exercised against 0..=50ms deltas), MAX
/// (count-only), then any small window.
fn window_strategy() -> impl proptest::strategy::Strategy<Value = u64> {
    prop_oneof![Just(0_u64), Just(10_u64), Just(u64::MAX), 0..=100_u64]
}

/// Stop ops over the KNOWN nonces `0..fleet` only — a generated unknown
/// nonce would panic by law, not by bug.
fn child_stop_strategy(fleet: u64) -> impl proptest::strategy::Strategy<Value = StopOp> {
    (0..fleet, outcome_strategy(), 0..=50_u64)
        .prop_map(|(nonce, outcome, stamp_delta_ms)| StopOp { nonce, outcome, stamp_delta_ms })
}

fn fold_supervising_and_check(
    rt: &tokio::runtime::Runtime,
    n_children: usize,
    budget: u32,
    window_ms: u64,
    ops: &[StopOp],
) {
    let window = if window_ms == u64::MAX { Duration::MAX } else { Duration::from_millis(window_ms) };
    let t0 = Instant::now();
    let mut model = SupModel::new(n_children, budget, window_ms);
    let mut sup = supervisor(n_children, Strategy::OneForOne, RestartPolicy::Transient, budget, window);
    rt.block_on(async {
        let mut now_ms = 0_u64;
        for (i, op) in ops.iter().copied().enumerate() {
            now_ms += op.stamp_delta_ms;
            let at = t0 + Duration::from_millis(now_ms);
            let actions = sup.step(stopped(op.nonce, op.outcome.into_envelope(), at)).await.expect("no error");
            let expected = model.fold(op.nonce, op.outcome.is_abnormal(), now_ms);
            assert_eq!(actions.creates.len(), expected, "op #{i}: create count");
            assert!(
                actions.creates
                    .iter()
                    .all(|c| matches!(c, Create::Restart { nonce, .. } if *nonce == op.nonce)),
                "op #{i}: every emission is a Restart naming the stopped nonce"
            );
            assert!(actions.sends.is_empty(), "op #{i}: supervision emits no sends");
            assert_eq!(actions.become_, Step::Continue, "op #{i}: supervision never becomes");
            assert_eq!(sup.restarts_in_window(), model.restarts.len(), "op #{i}: windowed budget accounting");
            for (j, alive) in model.alive.iter().copied().enumerate() {
                assert_eq!(sup.is_alive(j as u64), alive, "op #{i}: slot {j} liveness");
            }
        }
    });
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Random child-stop scripts over a fleet of 4 against the boundary
    /// budgets 0 / 1 / MAX / any and the boundary windows ZERO / 10ms / MAX /
    /// any: create count, emission shape, windowed accounting, and every
    /// slot's liveness match the independent model after EVERY event.
    #[test]
    fn prop_supervising_matches_differential_model(
        budget in budget_strategy(),
        window_ms in window_strategy(),
        ops in proptest::collection::vec(child_stop_strategy(4), 0..=20),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_supervising_and_check(&rt, 4, budget, window_ms, &ops);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 64, ..proptest::prelude::ProptestConfig::default() })]

    /// Long interleavings over a fleet of 8: no panic, budget never negative,
    /// liveness and windowed accounting stay model-equal throughout.
    #[test]
    fn prop_supervising_long_sequences_model_equal(
        budget in budget_strategy(),
        window_ms in window_strategy(),
        ops in proptest::collection::vec(child_stop_strategy(8), 0..=64),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        fold_supervising_and_check(&rt, 8, budget, window_ms, &ops);
    }
}
