//! Actions (Agha action triple) constructor algebra — the vocabulary every
//! layer folds. Pins constructor exactness: each shorthand carries EXACTLY its
//! become with no hidden sends/creates, and the exit/phase payload survives
//! verbatim. A refactor of the action algebra must keep these.

use behaviorpass::{Actions, Exit, MailAddr, Target};
use behaviorpass::{Never, Step};

#[test]
fn actions_just_carries_the_exact_become_with_no_effects() {
    let a = Actions::<MailAddr, Never, u64, u32>::just(Step::Continue);
    assert_eq!(a.become_, Step::Continue);
    assert!(a.sends.is_empty());
    assert!(a.creates.is_empty());

    let b = Actions::<MailAddr, i32, u64, u32>::just(Step::Goto(7));
    assert_eq!(b.become_, Step::Goto(7));
    assert!(b.sends.is_empty() && b.creates.is_empty());
}

#[test]
fn actions_cont_is_pure_continue() {
    let a = Actions::<MailAddr, Never, u64, u32>::cont();
    assert_eq!(a.become_, Step::Continue);
    assert!(a.sends.is_empty() && a.creates.is_empty());
}

#[test]
fn actions_stop_carries_the_exit_verbatim() {
    for exit in [
        Exit::Normal,
        Exit::Collected,
        Exit::LinkDied(MailAddr(0)),
        Exit::LinkDied(MailAddr(u64::MAX)),
    ] {
        let a = Actions::<MailAddr, Never, u64, u32>::stop(exit);
        assert_eq!(a.become_, Step::Stop(exit), "the stop carries {exit:?} verbatim");
        assert!(a.sends.is_empty() && a.creates.is_empty());
    }
}

#[test]
fn actions_goto_carries_the_phase_verbatim() {
    let a = Actions::<MailAddr, i32, u64, u32>::goto(-3);
    assert_eq!(a.become_, Step::Goto(-3));
    assert!(a.sends.is_empty() && a.creates.is_empty());
}

/// The address token is a plain opaque id: Copy + Eq, and the boundary ids
/// ride through a send trace untouched.
#[test]
fn mailaddr_is_an_opaque_copyable_token() {
    for addr in [MailAddr(0), MailAddr(1), MailAddr(u64::MAX)] {
        let copy = addr;
        assert_eq!(copy, addr, "MailAddr is Copy + Eq");
        let actions = Actions::<MailAddr, Never, u64, u32> {
            sends: vec![(Target::Global(addr), 7)],
            creates: Vec::new(),
            become_: Step::Continue,
        };
        assert_eq!(actions.sends, vec![(Target::Global(addr), 7)], "the boundary address survives the trace");
    }
}
