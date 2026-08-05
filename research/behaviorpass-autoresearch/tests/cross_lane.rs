//! Cross-lane isolation: in a supervised stack over a stash over a sending
//! parent, the user lane, the child-death lane, and the buffer never leak
//! into each other — user messages are routed by the stash (Deliver through,
//! Stash held, Release delivered), child deaths produce replacement sends
//! only, and neither produces the other's effects.

use behaviorpass::{
    Acted, Actions, Base, Behavior, ChildStopped, Crash, Delivery, MailAddr, Never, Recipient,
    Route, Spec, StashRoute, State, Step, SupervisionEvent, UserEvent,
};
use tokio::time::Instant;

#[derive(Default)]
struct Recorder;

impl State<u8, Never, Never> for Recorder {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, Never, Never> {
        Ok(Actions::cont())
    }
}

type Child = Base<Recorder, u8>;

fn child(_index: usize) -> Child {
    Base::new(Recorder)
}

/// A parent that echoes every user message on its own send lane (Out = u64)
/// and can birth children (Offspring = Child). The echo lane is how the
/// test observes what the parent actually processed.
struct EchoingParent {
    seen: Vec<u64>,
}

impl State<u64, Child, Never> for EchoingParent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u64>>, Child, Never> {
        self.seen.push(message);
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(MailAddr(0)), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

#[allow(clippy::trivially_copy_pass_by_ref, reason = "the stash API routes through fn(&Msg)")]
fn route(message: &u64) -> StashRoute {
    match message % 3 {
        0 => StashRoute::Release,
        1 => StashRoute::Deliver,
        _ => StashRoute::Stash,
    }
}

type Stack = behaviorpass::Spec<
    behaviorpass::Supervising<behaviorpass::Stashing<Base<EchoingParent, u64, Child, Never>>, Child>,
>;

async fn user(behavior: &mut Stack, message: u64) -> Vec<u64> {
    let actions = behavior
        .step(SupervisionEvent::Inner(UserEvent::user(MailAddr(9), message)))
        .await
        .unwrap();
    actions.sends.inner.iter().map(|d| d.message).collect()
}

/// The user lane is filtered by the stash exactly as in the unfettered
/// stack (Deliver and Release triggers pass in order, Stash stays held), and
/// no user step ever emits a supervision send.
#[tokio::test]
async fn supervised_stash_routes_user_lane_without_cross_lane_effects() {
    let mut behavior = Spec::new(EchoingParent { seen: Vec::new() })
        .stash(route)
        .children((2, child));
    behavior.init().await.unwrap();

    // Stash-routed messages produce no echo; Deliver and Release triggers
    // pass through in order (the parent echoes what it processed).
    assert_eq!(user(&mut behavior, 2).await, vec![]); // 2 % 3 = 2: Stash
    assert_eq!(user(&mut behavior, 5).await, vec![]); // 5 % 3 = 2: Stash
    assert_eq!(user(&mut behavior, 1).await, vec![1]); // Deliver
    assert_eq!(user(&mut behavior, 0).await, vec![0]); // Release: trigger + drain re-stashes
    // A second release still cannot replay the stashed pair.
    assert_eq!(user(&mut behavior, 0).await, vec![0]);
}

/// A child death produces only a replacement send: the parent's echo lane
/// and the stash buffer stay untouched.
#[tokio::test]
async fn child_death_never_leaks_into_the_user_lane() {
    let mut behavior = Spec::new(EchoingParent { seen: Vec::new() })
        .stash(route)
        .children((2, child));
    behavior.init().await.unwrap();

    // Buffer one user message first, then kill a child.
    user(&mut behavior, 2).await;
    let actions = behavior
        .step(SupervisionEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .await
        .unwrap();
    assert_eq!(actions.sends.own.own.len(), 1);
    assert_eq!(actions.sends.own.own[0].to.route(), Route::Child(0));
    assert!(actions.sends.inner.is_empty());
    assert!(actions.sends.own.inner.is_empty());
}
