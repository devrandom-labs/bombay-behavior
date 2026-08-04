//! Watching invariant suite — the "adversarial" additions to `tests/oracle.rs`.
//! Pins the link-death algebra: an abnormal death propagates `Stop(LinkDied(peer))`
//! with the EXACT peer id; a normal death is absorbed (Continue); custom
//! reactions and errors ride out; user traffic and deadlines pass through.
//! Methods: handcrafted edges + a property sweep over (peer, abnormal) + a
//! mixed-event fuzz against a differential model.

use std::time::Duration;

use behaviorpass::{Actions, Base, Behavior, Deadlined, Envelope, Exit, MailAddr, Watching, stop_on_abnormal_death};
use behaviorpass::{Never, Step};
use proptest::prelude::*;
use tokio::time::Instant;

fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    })
}

/// Abnormal death propagates with the exact carried peer — including the
/// boundary peers 0 and u64::MAX.
#[tokio::test]
async fn watching_abnormal_death_propagates_exact_peer() {
    for peer in [42_u64, 0, u64::MAX] {
        let mut w = Watching::new(recorder(), stop_on_abnormal_death);
        let actions = w.step(Envelope::LinkDied { peer, abnormal: true }).await.expect("no error");
        assert_eq!(
            actions.become_,
            Step::Stop(Exit::LinkDied(peer)),
            "the abnormal death carries the exact peer {peer}"
        );
        assert!(actions.sends.is_empty(), "a link reaction emits nothing");
        assert!(actions.creates.is_empty());
    }
}

/// A normal death is absorbed: Continue, inner untouched.
#[tokio::test]
async fn watching_normal_death_is_absorbed_and_forwards() {
    let mut w = Watching::new(recorder(), stop_on_abnormal_death);
    let actions = w.step(Envelope::LinkDied { peer: 42, abnormal: false }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "a normal death is absorbed");
    assert_eq!(w.inner().state(), &Vec::<u64>::new(), "the link event never reaches the inner fold");

    let actions = w.step(Envelope::User(2)).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(w.inner().state(), &vec![2], "user traffic still folds after an absorbed death");
}

/// A custom reaction's verdict rides out verbatim.
#[tokio::test]
async fn watching_custom_reaction_verdict_rides_out() {
    let mut w = Watching::new(recorder(), |_inner: &mut Base<Vec<u64>, u64, Never, &'static str>, _peer: u64, _abnormal: bool| {
        Ok(Step::Stop(Exit::Normal))
    });
    let actions = w.step(Envelope::LinkDied { peer: 1, abnormal: false }).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "the custom reaction's Stop rides out");

    let mut w2 = Watching::new(recorder(), |_inner: &mut Base<Vec<u64>, u64, Never, &'static str>, _peer: u64, _abnormal: bool| {
        Ok(Step::Continue)
    });
    let actions = w2.step(Envelope::LinkDied { peer: 1, abnormal: true }).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue, "the custom reaction's Continue rides out");
}

/// A custom reaction's error propagates with its exact value.
#[tokio::test]
async fn watching_custom_reaction_error_propagates() {
    let mut w = Watching::new(recorder(), |_inner: &mut Base<Vec<u64>, u64, Never, &'static str>, _peer: u64, _abnormal: bool| {
        Err::<Step<Never, Exit>, &'static str>("boom")
    });
    let err = w.step(Envelope::LinkDied { peer: 1, abnormal: true }).await.err().expect("expected an error");
    assert_eq!(err, "boom");
}

/// User actions pass through Watching unchanged.
#[tokio::test]
async fn watching_user_actions_forward_unchanged() {
    let sender: Base<(), u64, Never, &'static str, u64, Never> = Base::new((), |(): &mut (), m: u64| {
        Ok::<Actions<Never, u64, Never>, &'static str>(Actions {
            sends: vec![(MailAddr(7), m)],
            creates: Vec::new(),
            become_: if m == 9 { Step::Stop(Exit::Normal) } else { Step::Continue },
        })
    });
    let mut w = Watching::new(sender, stop_on_abnormal_death);
    let actions = w.step(Envelope::User(4)).await.expect("no error");
    assert_eq!(actions.sends, vec![(MailAddr(7), 4)], "sends pass through unchanged");
    assert_eq!(actions.become_, Step::Continue);

    let actions = w.step(Envelope::User(9)).await.expect("no error");
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

fn peer_strategy() -> impl proptest::strategy::Strategy<Value = u64> {
    use proptest::prelude::*;
    prop_oneof![Just(0), Just(1), Just(u64::MAX), any::<u64>()]
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 256, ..proptest::prelude::ProptestConfig::default() })]

    /// The default policy is a pure function of (peer, abnormal): abnormal ⇒
    /// Stop(LinkDied(peer)) with the exact peer; normal ⇒ Continue; and the
    /// inner fold is never touched by a link event.
    #[test]
    fn prop_watching_stop_on_abnormal_death_policy(peer in peer_strategy(), abnormal in any::<bool>()) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut w = Watching::new(recorder(), stop_on_abnormal_death);
            let actions = w.step(Envelope::LinkDied { peer, abnormal }).await.unwrap();
            let expected = if abnormal {
                Step::Stop(Exit::LinkDied(peer))
            } else {
                Step::Continue
            };
            assert_eq!(actions.become_, expected, "peer={peer} abnormal={abnormal}");
            assert!(actions.sends.is_empty() && actions.creates.is_empty());
            assert_eq!(w.inner().state(), &Vec::<u64>::new(), "a link event never folds the inner behavior");
        });
    }
}

/// A mixed-event script against a differential model: user ids fold in order,
/// abnormal deaths stop with the exact peer, normal deaths are absorbed.
#[derive(Debug, Clone, Copy)]
enum Ev {
    User(u64),
    Die { peer: u64, abnormal: bool },
}

struct WatchModel {
    seen: Vec<u64>,
}

impl WatchModel {
    fn new() -> Self {
        Self { seen: Vec::new() }
    }

    /// Returns the stop reason, if the script stops at this event.
    fn fold(&mut self, ev: Ev) -> Option<Exit> {
        match ev {
            Ev::User(id) => {
                self.seen.push(id);
                None
            }
            Ev::Die { peer, abnormal: true } => Some(Exit::LinkDied(peer)),
            Ev::Die { abnormal: false, .. } => None,
        }
    }
}

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 128, ..proptest::prelude::ProptestConfig::default() })]

    /// Long mixed interleavings stay model-equal: user folds accumulate in
    /// order, the FIRST abnormal death stops with the exact peer, everything
    /// after it is never folded.
    #[test]
    fn prop_watching_mixed_events_match_model(evs in proptest::collection::vec(
        prop_oneof![
            peer_strategy().prop_map(Ev::User),
            (peer_strategy(), any::<bool>()).prop_map(|(peer, abnormal)| Ev::Die { peer, abnormal }),
        ],
        0..=64,
    )) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut model = WatchModel::new();
            let mut w = Watching::new(recorder(), stop_on_abnormal_death);
            for (i, ev) in evs.into_iter().enumerate() {
                let actions = w.step(match ev {
                    Ev::User(id) => Envelope::User(id),
                    Ev::Die { peer, abnormal } => Envelope::LinkDied { peer, abnormal },
                }).await.unwrap();
                if let Some(exit) = model.fold(ev) {
                    assert_eq!(actions.become_, Step::Stop(exit), "event #{i}: the abnormal death stops with its exact peer");
                    assert_eq!(w.inner().state(), &model.seen, "event #{i}: nothing after the death folds");
                    return;
                }
                assert_eq!(actions.become_, Step::Continue, "event #{i}");
                assert_eq!(w.inner().state(), &model.seen, "event #{i}: fold order");
            }
        });
    }
}
