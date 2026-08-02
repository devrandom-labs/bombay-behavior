//! Base (the floor) invariant suite — the "adversarial" additions to
//! `tests/oracle.rs`. A plain actor owns no framework source: deadline /
//! link-death / child-stop are no-ops that emit NOTHING — even when the floor
//! is typed with outbound and create menus that COULD emit. User messages fold
//! through the handler and ride its effects out.
//! Methods: handcrafted edges + a property sweep over the framework alphabet.

use behaviorpass::{Actions, Base, Behavior, Envelope, Exit, MailAddr};
use bombay::capability::{Never, Step};
use proptest::prelude::*;

/// A floor typed with BOTH menus: it *could* send and create on user messages,
/// but framework events must emit nothing at all.
fn menu_floor() -> Base<Vec<u64>, u64, Never, &'static str, u64, u32> {
    Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<Never, u64, u32>, &'static str>(Actions {
            sends: vec![(MailAddr(id), id)],
            creates: vec![id as u32],
            become_: Step::Continue,
        })
    })
}

/// Every framework event is a total no-op for the floor: no sends, no creates,
/// become(same), state untouched — even with menus armed.
#[tokio::test]
async fn base_framework_events_emit_nothing_even_with_menus() {
    for ev in [
        Envelope::Deadline,
        Envelope::LinkDied { peer: 42, abnormal: true },
        Envelope::LinkDied { peer: 0, abnormal: false },
        Envelope::ChildStopped { idx: 0, abnormal: true },
        Envelope::ChildStopped { idx: usize::MAX, abnormal: false },
    ] {
        let mut b = menu_floor();
        let actions = b.step(ev).await.expect("no error");
        assert_eq!(actions.become_, Step::Continue, "framework events become(same)");
        assert!(actions.sends.is_empty(), "a framework event never sends");
        assert!(actions.creates.is_empty(), "a framework event never creates");
        assert_eq!(b.state(), &Vec::<u64>::new(), "a framework event never folds the state");
    }
}

/// User messages ride the handler's full effect triple out.
#[tokio::test]
async fn base_user_messages_ride_the_full_triple_out() {
    let mut b = menu_floor();
    let actions = b.step(Envelope::User(5)).await.expect("no error");
    assert_eq!(actions.sends, vec![(MailAddr(5), 5)]);
    assert_eq!(actions.creates, vec![5]);
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(b.state(), &vec![5]);
}

/// Boundary addresses ride through the send trace untouched.
#[tokio::test]
async fn base_boundary_addresses_ride_through() {
    for addr in [0_u64, u64::MAX] {
        let mut b: Base<(), u64, Never, &'static str, u64, Never> = Base::new((), |(): &mut (), m: u64| {
            Ok::<Actions<Never, u64, Never>, &'static str>(Actions {
                sends: vec![(MailAddr(m), m)],
                creates: Vec::new(),
                become_: Step::Stop(Exit::Normal),
            })
        });
        let actions = b.step(Envelope::User(addr)).await.expect("no error");
        assert_eq!(actions.sends, vec![(MailAddr(addr), addr)], "address {addr} survives the trace");
    }
}

/// A handler error surfaces with its exact value.
#[tokio::test]
async fn base_handler_error_propagates_exactly() {
    let mut b: Base<(), u64, Never, &'static str> = Base::new((), |(): &mut (), id: u64| {
        if id == 7 {
            Err("boom")
        } else {
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        }
    });
    assert_eq!(b.step(Envelope::User(1)).await.unwrap().become_, Step::Continue);
    let err = b.step(Envelope::User(7)).await.err().expect("expected an error");
    assert_eq!(err, "boom");
}

// ---------------------------------------------------------------------------
// Property sweep over the framework alphabet
// ---------------------------------------------------------------------------

proptest::proptest! {
    #![proptest_config(proptest::prelude::ProptestConfig { cases: 256, ..proptest::prelude::ProptestConfig::default() })]

    /// For ANY framework event (random peers / indices / abnormality): the
    /// floor emits nothing and never folds.
    #[test]
    fn prop_base_framework_events_are_noops(peer in any::<u64>(), idx in any::<usize>(), abnormal in any::<bool>()) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let mut b = menu_floor();
            for ev in [
                Envelope::Deadline,
                Envelope::LinkDied { peer, abnormal },
                Envelope::ChildStopped { idx, abnormal },
            ] {
                let actions = b.step(ev).await.unwrap();
                assert_eq!(actions.become_, Step::Continue, "peer={peer} idx={idx} abnormal={abnormal}");
                assert!(actions.sends.is_empty());
                assert!(actions.creates.is_empty());
                assert_eq!(b.state(), &Vec::<u64>::new(), "state untouched by framework events");
            }
        });
    }
}
