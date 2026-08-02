//! Deadlined invariant suite — the "adversarial" additions to `tests/oracle.rs`.
//! Pins the min-fold law (the earliest of own and inner slots arms the one
//! timer), fires-once-per-layer clearing, error propagation, and passthrough.
//! Methods: handcrafted edges + a property sweep over the (own, inner) slot
//! lattice.

use std::time::Duration;

use behaviorpass::{Actions, Base, Behavior, Deadlined, Envelope, Exit};
use bombay::capability::{Never, Step};
use tokio::time::Instant;

fn floor() -> Base<(), u64, Never, &'static str> {
    Base::new((), |(): &mut (), _: u64| Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont()))
}

fn at(secs: u64) -> Instant {
    Instant::now() + Duration::from_secs(secs)
}

/// The min-fold law: `next_deadline` is the EARLIEST of the own slot and any
/// inner slot — a later own slot must not mask an earlier inner one.
#[tokio::test]
async fn deadlined_next_deadline_min_folds_own_and_inner() {
    let t_inner = at(1);
    let t_own = at(5);
    let inner_d = Deadlined::new(floor(), Some(t_inner), |_| Ok(Step::Continue));
    let d = Deadlined::new(inner_d, Some(t_own), |_| Ok(Step::Continue));
    assert_eq!(d.next_deadline(), Some(t_inner), "the EARLIER inner slot wins");

    let t_own2 = at(1);
    let t_inner2 = at(5);
    let inner_d2 = Deadlined::new(floor(), Some(t_inner2), |_| Ok(Step::Continue));
    let d2 = Deadlined::new(inner_d2, Some(t_own2), |_| Ok(Step::Continue));
    assert_eq!(d2.next_deadline(), Some(t_own2), "the EARLIER own slot wins");

    let t_eq = at(3);
    let inner_d3 = Deadlined::new(floor(), Some(t_eq), |_| Ok(Step::Continue));
    let d3 = Deadlined::new(inner_d3, Some(t_eq), |_| Ok(Step::Continue));
    assert_eq!(d3.next_deadline(), Some(t_eq), "equal slots arm once");
}

/// Each layer's slot clears on ITS OWN firing, leaving the other armed.
#[tokio::test]
async fn deadlined_fires_once_per_layer() {
    let t_inner = at(1);
    let t_own = at(5);
    let inner_d = Deadlined::new(floor(), Some(t_inner), |_| Ok(Step::Continue));
    let mut d = Deadlined::new(inner_d, Some(t_own), |_| Ok(Step::Continue));

    let actions = d.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "a Continue reaction rides out");
    assert_eq!(d.next_deadline(), Some(t_inner), "the OUTER slot cleared; the inner slot is untouched");

    // A firing always lands on the OUTERMOST layer: a second Deadline is
    // absorbed there too, so the inner slot stays armed.
    let actions = d.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(d.next_deadline(), Some(t_inner), "a disarmed outer layer still absorbs the Deadline event");
}

/// The (own, inner) presence lattice: None + None, None + Some, Some + None.
#[tokio::test]
async fn deadlined_presence_lattice() {
    let t = at(5);
    let d = Deadlined::new(floor(), None, |_| Ok(Step::Continue));
    assert_eq!(d.next_deadline(), None, "no slots ⇒ no deadline");

    let inner_d = Deadlined::new(floor(), Some(t), |_| Ok(Step::Continue));
    let d = Deadlined::new(inner_d, None, |_| Ok(Step::Continue));
    assert_eq!(d.next_deadline(), Some(t), "inner slot alone arms");

    let inner_d = Deadlined::new(floor(), None, |_| Ok(Step::Continue));
    let d = Deadlined::new(inner_d, Some(t), |_| Ok(Step::Continue));
    assert_eq!(d.next_deadline(), Some(t), "own slot alone arms");
}

/// A firing reaction's error propagates and the fired slot still clears.
#[tokio::test]
async fn deadlined_reaction_error_propagates_and_clears() {
    let t = at(5);
    let mut d = Deadlined::new(floor(), Some(t), |_| Err::<Step<Never, Exit>, &'static str>("boom"));
    let err = d.step(Envelope::Deadline).await.err().expect("expected an error");
    assert_eq!(err, "boom", "the reaction's exact error surfaces");
    assert_eq!(d.next_deadline(), None, "the slot cleared even though the reaction failed");
}

/// A Stop reaction rides out of the firing.
#[tokio::test]
async fn deadlined_reaction_stop_rides_out() {
    let mut d = Deadlined::new(floor(), Some(at(5)), |_| Ok(Step::Stop(Exit::Normal)));
    let actions = d.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal));
}

/// User events forward inward, fold the inner behavior, and do NOT clear the
/// deadline slot.
#[tokio::test]
async fn deadlined_user_events_forward_and_keep_the_slot() {
    let t = at(5);
    let recorder: Base<Vec<u64>, u64, Never, &'static str> =
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        });
    let mut d = Deadlined::new(recorder, Some(t), |_| Ok(Step::Continue));

    let actions = d.step(Envelope::User(7)).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(d.inner().state(), &vec![7], "user events forward inward");
    assert_eq!(d.next_deadline(), Some(t), "a user event never clears the slot");
}

/// Non-deadline framework events forward inward without disturbing the slot.
#[tokio::test]
async fn deadlined_framework_events_forward() {
    let t = at(5);
    let mut d = Deadlined::new(floor(), Some(t), |_| Ok(Step::Continue));
    for ev in [
        Envelope::LinkDied { peer: 42, abnormal: true },
        Envelope::ChildStopped { idx: 0, abnormal: false },
    ] {
        let actions = d.step(ev).await.expect("no error");
        assert_eq!(actions.become_, Step::Continue);
    }
    assert_eq!(d.next_deadline(), Some(t));
}

// ---------------------------------------------------------------------------
// Property sweep over the (own, inner) slot lattice
// ---------------------------------------------------------------------------

fn nanos_strategy() -> impl proptest::strategy::Strategy<Value = Option<u64>> {
    use proptest::prelude::*;
    prop_oneof![
        Just(None),
        Just(Some(0)), // Duration::ZERO boundary
        Just(Some(1_000_000_000)), // 1s boundary
        any::<u64>().prop_map(|n| Some(n % 1_000_000_001)), // 0..=1s
    ]
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 256, ..proptest::prelude::ProptestConfig::default() })]

    /// For ANY (own, inner) slot pair the fold is exactly the min: the
    /// earliest Some arms, None never hides a Some. Fires once per layer.
    #[test]
    fn prop_deadlined_next_deadline_is_the_min_fold(own_nanos in nanos_strategy(), inner_nanos in nanos_strategy()) {
        let t_own = own_nanos.map(|n| Instant::now() + Duration::from_nanos(n));
        let t_inner = inner_nanos.map(|n| Instant::now() + Duration::from_nanos(n));
        let expected = match (t_own, t_inner) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };

        let inner_d = Deadlined::new(floor(), t_inner, |_| Ok(Step::Continue));
        let mut d = Deadlined::new(inner_d, t_own, |_| Ok(Step::Continue));
        assert_eq!(d.next_deadline(), expected, "min-fold law");

        // A firing always lands on the OUTERMOST layer: its own slot clears
        // (or stays absent), and the inner slot is never touched.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let _ = d.step(Envelope::Deadline).await.unwrap();
        });
        assert_eq!(d.next_deadline(), t_inner, "the outer slot cleared; the inner slot is untouched");
    }
}
