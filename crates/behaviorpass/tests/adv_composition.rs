//! Composition invariant suite — the "adversarial" additions to
//! `tests/oracle.rs`. Pins the composition laws: sends/creates/become pass
//! through EVERY layer unchanged; the deadline min-fold surfaces through outer
//! layers; framework events route to the layer that owns them even when
//! wrapped (Stashing forwards Deadline and LinkDied; Supervising forwards
//! everything but ChildStopped).

use std::time::Duration;

use behaviorpass::{
    Actions, Base, Behavior, Deadlined, Envelope, Exit, MailAddr, StashRoute, Stashing,
    Supervising, Watching, stop_on_abnormal_death,
};
use bombay::capability::{Never, Step};
use tokio::time::Instant;

type Kid = Base<u32, u32, Never, &'static str>;

fn kid() -> Kid {
    Base::new(0_u32, |count: &mut u32, n: u32| {
        *count += n;
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    })
}

/// The full send/stop stack: Watching<Supervising<Stashing<Deadlined<Base>>>>
/// — a user message's sends and a Stop verdict must ride out unchanged through
/// every layer.
type FullStack = Watching<Supervising<Stashing<Deadlined<Base<(), u64, Never, &'static str, u64, Never>>>, Kid>>;

fn full_stack() -> FullStack {
    let base: Base<(), u64, Never, &'static str, u64, Never> = Base::new((), |(): &mut (), m: u64| {
        Ok::<Actions<Never, u64, Never>, &'static str>(Actions {
            sends: vec![(MailAddr(9), m)],
            creates: Vec::new(),
            become_: if m == 0 { Step::Stop(Exit::Normal) } else { Step::Continue },
        })
    });
    let deadlined = Deadlined::new(base, None, |_| Ok(Step::Continue));
    let stashing = Stashing::new(deadlined, |&_| StashRoute::Deliver);
    let supervising = Supervising::new(stashing, 2, |_| kid(), 3);
    Watching::new(supervising, stop_on_abnormal_death)
}

/// Sends pass through EVERY layer unchanged; creates stay empty (the floor's
/// menu is send-only); the user message folds exactly once.
#[tokio::test]
async fn composition_sends_pass_through_every_layer() {
    let mut stack = full_stack();
    let actions = stack.step(Envelope::User(4)).await.expect("no error");
    assert_eq!(actions.sends, vec![(MailAddr(9), 4)], "the send survives all five layers");
    assert!(actions.creates.is_empty(), "no layer invents creates");
    assert_eq!(actions.become_, Step::Continue);
}

/// A Stop verdict rides out through every layer unchanged.
#[tokio::test]
async fn composition_stop_verdict_rides_out_through_every_layer() {
    let mut stack = full_stack();
    let actions = stack.step(Envelope::User(0)).await.expect("no error");
    assert_eq!(actions.become_, Step::Stop(Exit::Normal), "the Stop survives all five layers");
    assert_eq!(actions.sends, vec![(MailAddr(9), 0)]);
}

/// The deadline min-fold surfaces through the outer layers: Stashing and
/// Watching both forward `next_deadline`, and a Deadline event routes inward
/// to the Deadlined layer through both wrappers.
#[tokio::test]
async fn composition_deadline_min_surfaces_through_outer_layers() {
    let t1 = Instant::now() + Duration::from_secs(1);
    let t2 = Instant::now() + Duration::from_secs(5);
    let base: Base<(), u64, Never, &'static str> = Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    let inner_d = Deadlined::new(base, Some(t1), |_| Ok(Step::Continue));
    let outer_d = Deadlined::new(inner_d, Some(t2), |_| Ok(Step::Continue));
    let stashing = Stashing::new(outer_d, |&_| StashRoute::Deliver);
    let mut watching = Watching::new(stashing, stop_on_abnormal_death);

    assert_eq!(watching.next_deadline(), Some(t1), "the min of both slots surfaces through Stashing AND Watching");

    let actions = watching.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(watching.next_deadline(), Some(t1), "the outer slot cleared; the inner slot is untouched");

    // A firing always lands on the OUTERMOST Deadlined layer (the one holding
    // t2): a second Deadline is absorbed there too, so the inner slot stays
    // armed and keeps surfacing through both wrapper layers.
    let actions = watching.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(watching.next_deadline(), Some(t1), "a disarmed outer layer still absorbs the Deadline event");
}

/// Watching INSIDE Supervising: each layer owns its source — link-death is
/// handled by the inner Watching (verdict rides out through Supervising::
/// forward), child-stop by the outer Supervising, user sends pass through both.
#[tokio::test]
async fn composition_watching_inside_supervising_handles_both_sources() {
    let sender: Base<(), u64, Never, &'static str, u64, Never> = Base::new((), |(): &mut (), m: u64| {
        Ok::<Actions<Never, u64, Never>, &'static str>(Actions {
            sends: vec![(MailAddr(9), m)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    });
    let watching = Watching::new(sender, stop_on_abnormal_death);
    let mut sup = Supervising::new(watching, 1, |_| kid(), 2);

    let actions = sup.step(Envelope::User(4)).await.expect("no error");
    assert_eq!(actions.sends, vec![(MailAddr(9), 4)], "user sends pass through both layers");

    let actions = sup.step(Envelope::LinkDied { peer: 42, abnormal: true }).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::LinkDied(42)),
        "the inner Watching's propagation verdict rides out through Supervising"
    );
    assert!(actions.sends.is_empty(), "a link reaction emits no sends");
    assert!(actions.creates.is_empty(), "a link death is not a restart decision");

    let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert_eq!(actions.creates.len(), 1, "the outer Supervising restarts on child-stop");
    assert_eq!(sup.restarts_left(), 1);
}

/// Deadlined ABOVE Watching: the deadline owns its event outside, the link
/// reaction fires inside — the two sources coexist without interference.
#[tokio::test]
async fn composition_deadline_above_watching_both_sources() {
    let due = Instant::now() + Duration::from_secs(5);
    let base: Base<(), u64, Never, &'static str> = Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    let watching = Watching::new(base, stop_on_abnormal_death);
    let mut d = Deadlined::new(watching, Some(due), |_| Ok(Step::Continue));

    assert_eq!(d.next_deadline(), Some(due), "the deadline surfaces above Watching");

    let actions = d.step(Envelope::LinkDied { peer: 7, abnormal: true }).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::LinkDied(7)),
        "the link reaction fires through the deadline layer"
    );
    assert_eq!(d.next_deadline(), Some(due), "a link event does not disturb the deadline");

    let actions = d.step(Envelope::Deadline).await.expect("no error");
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(d.next_deadline(), None, "the deadline fired once");
}

/// An abnormal link-death routes through Stashing (which forwards it) to the
/// Watching layer — the held buffer is untouched.
#[tokio::test]
async fn composition_link_died_propagates_through_stashing() {
    let base: Base<Vec<u64>, u64, Never, &'static str> = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
        seen.push(id);
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    let watching = Watching::new(base, stop_on_abnormal_death);
    let mut stashing = Stashing::new(watching, |&id| {
        if id % 2 == 1 { StashRoute::Stash } else { StashRoute::Deliver }
    });
    let _ = stashing.step(Envelope::User(1)).await.expect("no error"); // held=[1]

    let actions = stashing.step(Envelope::LinkDied { peer: 42, abnormal: true }).await.expect("no error");
    assert_eq!(
        actions.become_,
        Step::Stop(Exit::LinkDied(42)),
        "the link-death propagates through Stashing to the Watching layer"
    );
    assert_eq!(stashing.held(), 1, "the held buffer is untouched by a framework event");
}

/// A child-stop routes through Stashing to the Supervising layer, which emits
/// its restart create.
#[tokio::test]
async fn composition_child_stopped_decides_through_stashing() {
    let base: Base<(), u64, Never, &'static str> = Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
    });
    let supervising = Supervising::new(base, 1, |_| kid(), 3);
    let mut stashing = Stashing::new(supervising, |&_| StashRoute::Deliver);

    let actions = stashing.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.expect("no error");
    assert_eq!(actions.creates.len(), 1, "the restart create rides out through Stashing");
    assert!(actions.sends.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}
