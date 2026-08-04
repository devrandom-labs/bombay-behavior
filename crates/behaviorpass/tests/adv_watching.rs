//! Watching invariant suite — the "adversarial" additions to `tests/oracle.rs`.
//! Pins the link-death algebra: an abnormal OUTCOME propagates
//! `Stop(LinkDied(peer))` with the EXACT peer address; a normal one is
//! absorbed (Continue); custom reactions and errors ride out; user traffic
//! and deadlines pass through. Classification rides the outcome vocabulary —
//! `Err(Crash)` in either domain and any `Ok` exit outside `{Normal,
//! Collected}` are abnormal. Methods: handcrafted edges + a property sweep
//! over (peer, outcome) + a mixed-event fuzz against a differential model.

use std::time::Duration;

use behaviorpass::{
    Actions, Base, Become, Behavior, Crash, Deadlined, Envelope, Exit, FnState, MailAddr, Target, Watching,
    stop_on_abnormal_death,
};
use behaviorpass::{Never, Step};
use proptest::prelude::*;
use tokio::time::Instant;

type Rec = Base<FnState<Vec<u64>, MailAddr, u64, Never, Never, &'static str>, Never, Never, &'static str>;

fn recorder() -> Rec {
    Base::from_fn(Vec::<u64>::new(), |seen: &mut Vec<u64>, _from: MailAddr, id: u64| {
        seen.push(id);
        Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
    })
}

fn user(msg: u64) -> Envelope<MailAddr, u64> {
    Envelope::User { from: MailAddr(1), msg }
}

/// The outcome-classification law as the model's oracle: NORMAL is exactly
/// `Ok(Exit::Normal | Exit::Collected)`; everything else — either crash
/// domain, or an `Ok` carrying any other exit value — is ABNORMAL.
fn is_abnormal(outcome: &Result<Exit<MailAddr>, Crash>) -> bool {
    !matches!(outcome, Ok(Exit::Normal | Exit::Collected))
}

/// Abnormal crash outcomes propagate with the exact carried peer — both crash
/// domains — including the boundary peers 0 and `u64::MAX`.
#[tokio::test]
async fn watching_abnormal_death_propagates_exact_peer() {
    let cases: [(MailAddr, Result<Exit<MailAddr>, Crash>); 3] = [
        (MailAddr(42), Err(Crash::Failed)),
        (MailAddr(0), Err(Crash::Panicked)),
        (MailAddr(u64::MAX), Err(Crash::Failed)),
    ];
    for (peer, outcome) in cases {
        let mut w = Watching::new(recorder(), stop_on_abnormal_death);
        let actions = w.step(Envelope::LinkDied { peer, outcome }).await.expect("no error");
        assert_eq!(
            actions.become_,
            Step::Stop(Exit::LinkDied(peer)),
            "the abnormal outcome {outcome:?} carries the exact peer {peer:?}"
        );
        assert!(actions.sends.is_empty(), "a link reaction emits nothing");
        assert!(actions.creates.is_empty());
    }
}

/// An abnormal EXIT VALUE — an `Ok` carrying an exit outside `{Normal,
/// Collected}` — propagates under `stop_on_abnormal_death`, stopping with the
/// DYING peer (not the carried address). The bool world could not express
/// this class.
#[tokio::test]
async fn watching_abnormal_exit_value_propagates() {
    for peer in [MailAddr(42), MailAddr(0), MailAddr(u64::MAX)] {
        let mut w = Watching::new(recorder(), stop_on_abnormal_death);
        let carried = MailAddr(7);
        let actions = w
            .step(Envelope::LinkDied { peer, outcome: Ok(Exit::LinkDied(carried)) })
            .await
            .expect("no error");
        assert_eq!(
            actions.become_,
            Step::Stop(Exit::LinkDied(peer)),
            "an Ok exit outside the normal subset is abnormal: stops with the dying peer {peer:?}"
        );
        assert!(actions.sends.is_empty(), "a link reaction emits nothing");
        assert!(actions.creates.is_empty());
        assert_eq!(w.inner().state().state, Vec::<u64>::new(), "a link event never reaches the inner fold");
    }
}

/// Both normal outcomes are absorbed: Continue, inner untouched.
#[tokio::test]
async fn watching_normal_death_is_absorbed_and_forwards() {
    let normals: [Result<Exit<MailAddr>, Crash>; 2] = [Ok(Exit::Normal), Ok(Exit::Collected)];
    for outcome in normals {
        let mut w = Watching::new(recorder(), stop_on_abnormal_death);
        let actions = w
            .step(Envelope::LinkDied { peer: MailAddr(42), outcome })
            .await
            .expect("no error");
        assert_eq!(actions.become_, Step::Continue, "{outcome:?} classifies normal and is absorbed");
        assert_eq!(w.inner().state().state, Vec::<u64>::new(), "the link event never reaches the inner fold");

        let actions = w.step(user(2)).await.expect("no error");
        assert_eq!(actions.become_, Step::Continue);
        assert_eq!(w.inner().state().state, vec![2], "user traffic still folds after an absorbed death");
    }
}

/// A custom reaction's verdict rides out verbatim.
#[tokio::test]
async fn watching_custom_reaction_verdict_rides_out() {
    let mut w = Watching::new(
        recorder(),
        |_inner: &mut Rec, _peer: MailAddr, _outcome: Result<Exit<MailAddr>, Crash>| {
            Ok(Step::Stop(Exit::Normal))
        },
    );
    let actions = w
        .step(Envelope::LinkDied { peer: MailAddr(1), outcome: Ok(Exit::Normal) })
        .await
        .expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "the custom reaction's Stop rides out");

    let mut w2 = Watching::new(
        recorder(),
        |_inner: &mut Rec, _peer: MailAddr, _outcome: Result<Exit<MailAddr>, Crash>| Ok(Step::Continue),
    );
    let actions = w2
        .step(Envelope::LinkDied { peer: MailAddr(1), outcome: Err(Crash::Panicked) })
        .await
        .expect("no error");
    assert_eq!(actions.become_, Step::Continue, "the custom reaction's Continue rides out");
}

/// A custom reaction's error propagates with its exact value.
#[tokio::test]
async fn watching_custom_reaction_error_propagates() {
    let mut w = Watching::new(
        recorder(),
        |_inner: &mut Rec, _peer: MailAddr, _outcome: Result<Exit<MailAddr>, Crash>| {
            Err::<Become<MailAddr>, &'static str>("boom")
        },
    );
    let err = w
        .step(Envelope::LinkDied { peer: MailAddr(1), outcome: Err(Crash::Failed) })
        .await
        .err()
        .expect("expected an error");
    assert_eq!(err, "boom");
}

/// User actions pass through Watching unchanged.
#[tokio::test]
async fn watching_user_actions_forward_unchanged() {
    type Sender = Base<FnState<(), MailAddr, u64, u64, Never, &'static str>, u64, Never, &'static str>;
    let sender: Sender =
        Base::from_fn((), |(): &mut (), _from: MailAddr, m: u64| {
            Ok::<Actions<MailAddr, Never, u64, Never>, &'static str>(Actions {
                sends: vec![(Target::Global(MailAddr(7)), m)],
                creates: Vec::new(),
                become_: if m == 9 { Step::Stop(Exit::Normal) } else { Step::Continue },
            })
        });
    let mut w = Watching::new(sender, stop_on_abnormal_death);
    let actions = w.step(user(4)).await.expect("no error");
    assert_eq!(actions.sends, vec![(Target::Global(MailAddr(7)), 4)], "sends pass through unchanged");
    assert_eq!(actions.become_, Step::Continue);

    let actions = w.step(user(9)).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "an inner Stop rides out unchanged");
}

/// Watching forwards the inner deadline and fires it inward.
#[tokio::test]
async fn watching_forwards_deadline_and_arms_inner() {
    let due = Instant::now() + Duration::from_secs(5);
    let inner_d = Deadlined::new(recorder(), Some(due), |_inner| Ok(Step::Stop(Exit::Normal)));
    let mut w = Watching::new(inner_d, stop_on_abnormal_death);
    assert_eq!(w.next_deadline(), Some(due), "Watching forwards the inner deadline");

    let actions = w.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "the Deadline event forwards inward and fires");
    assert_eq!(w.next_deadline(), None, "fires once through Watching");
}

// ---------------------------------------------------------------------------
// Property sweep + fuzz
// ---------------------------------------------------------------------------

fn addr_strategy() -> impl Strategy<Value = MailAddr> {
    prop_oneof![
        Just(MailAddr(0)),
        Just(MailAddr(1)),
        Just(MailAddr(u64::MAX)),
        any::<u64>().prop_map(MailAddr),
    ]
}

/// Every outcome class: both crash domains, both normal exits, and an `Ok`
/// carrying an abnormal exit value — the vocabulary the bool sweep could not
/// reach.
fn outcome_strategy() -> impl Strategy<Value = Result<Exit<MailAddr>, Crash>> {
    prop_oneof![
        Just(Err(Crash::Failed)),
        Just(Err(Crash::Panicked)),
        Just(Ok(Exit::Normal)),
        Just(Ok(Exit::Collected)),
        addr_strategy().prop_map(|carried| Ok(Exit::LinkDied(carried))),
    ]
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 256, ..proptest::prelude::ProptestConfig::default() })]

    /// The default policy is a pure function of (peer, outcome): abnormal ⇒
    /// Stop(LinkDied(peer)) with the exact peer; normal ⇒ Continue; and the
    /// inner fold is never touched by a link event.
    #[test]
    fn prop_watching_stop_on_abnormal_death_policy(
        peer in addr_strategy(),
        outcome in outcome_strategy(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut w = Watching::new(recorder(), stop_on_abnormal_death);
            let actions = w.step(Envelope::LinkDied { peer, outcome }).await.unwrap();
            let expected = if is_abnormal(&outcome) {
                Step::Stop(Exit::LinkDied(peer))
            } else {
                Step::Continue
            };
            assert_eq!(actions.become_, expected, "peer={peer:?} outcome={outcome:?}");
            assert!(actions.sends.is_empty() && actions.creates.is_empty());
            assert_eq!(w.inner().state().state, Vec::<u64>::new(), "a link event never folds the inner behavior");
        });
    }
}

/// A mixed-event script against a differential model: user ids fold in order,
/// abnormal outcomes stop with the exact peer, normal outcomes are absorbed.
#[derive(Debug, Clone, Copy)]
enum Ev {
    User(u64),
    Die { peer: MailAddr, outcome: Result<Exit<MailAddr>, Crash> },
}

struct WatchModel {
    seen: Vec<u64>,
}

impl WatchModel {
    fn new() -> Self {
        Self { seen: Vec::new() }
    }

    /// Returns the stop reason, if the script stops at this event.
    fn fold(&mut self, ev: Ev) -> Option<Exit<MailAddr>> {
        match ev {
            Ev::User(id) => {
                self.seen.push(id);
                None
            }
            Ev::Die { peer, outcome } if is_abnormal(&outcome) => Some(Exit::LinkDied(peer)),
            Ev::Die { .. } => None,
        }
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Long mixed interleavings stay model-equal: user folds accumulate in
    /// order, the FIRST abnormal outcome stops with the exact peer, everything
    /// after it is never folded.
    #[test]
    fn prop_watching_mixed_events_match_model(evs in proptest::collection::vec(
        prop_oneof![
            addr_strategy().prop_map(|MailAddr(id)| Ev::User(id)),
            (addr_strategy(), outcome_strategy()).prop_map(|(peer, outcome)| Ev::Die { peer, outcome }),
        ],
        0..=64,
    )) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut model = WatchModel::new();
            let mut w = Watching::new(recorder(), stop_on_abnormal_death);
            for (i, ev) in evs.into_iter().enumerate() {
                let actions = w.step(match ev {
                    Ev::User(id) => user(id),
                    Ev::Die { peer, outcome } => Envelope::LinkDied { peer, outcome },
                }).await.unwrap();
                if let Some(exit) = model.fold(ev) {
                    assert_eq!(actions.become_, Step::Stop(exit), "event #{i}: the abnormal outcome stops with its exact peer");
                    assert_eq!(w.inner().state().state, model.seen, "event #{i}: nothing after the death folds");
                    return;
                }
                assert_eq!(actions.become_, Step::Continue, "event #{i}");
                assert_eq!(w.inner().state().state, model.seen, "event #{i}: fold order");
            }
        });
    }
}
