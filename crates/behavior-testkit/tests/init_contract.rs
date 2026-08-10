//! The init contract: double-initialization and step-before-init were never
//! probed. Findings: `Proxy::init` panics on double-init ("a proxy
//! initializes once"), but `Deadline`/`Watch`/`Supervisor` have NO guard —
//! a second `init` silently duplicates init effects (schedule sends,
//! observe sends, and the entire configured fleet, slots included).
//! Step-before-init: `Proxy::step(Forward)` routes to a generation whose
//! birth has not happened yet; `Supervisor::step` panics on the empty slot
//! table. Both are unreachable through the driver (which inits first), but
//! they are direct-misuse boundaries with an asymmetric guard story.

use std::time::Duration;

use behavior::{
    Acted, Actions, Behavior, Compose, Crash, Delivery, Handler, MailAddr, Never, Proxy,
    ProxyCommand, ProxyEvent, Pure, Recipient, Route, StashRoute, Step, SupervisionEvent, User,
    UserEvent, WorkerStopped, stop_on_abnormal_death,
};
use tokio::time::Instant;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

impl Handler<u8, behavior::NoBirths, Never> for Recorder {
    type Addr = MailAddr;
    type Msg = u8;

    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: Step::Continue,
        })
    }
}

type Child = Pure<Recorder, u8>;

fn child(_index: usize) -> Child {
    Pure::new(Recorder::default())
}

/// A quiet parent that births nothing at init.
struct Parent;

impl Handler<Never, behavior::Births<Child>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Never>
    {
        Ok(Actions::cont())
    }
}

/// A second `Deadline::init` emits a second, identical schedule send.
#[tokio::test]
async fn at_double_init_duplicates_the_schedule_send() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior =
        Compose::new(Recorder::default()).deadline(Some(due), |_| Ok(Step::Continue));
    let first = behavior.init().unwrap();
    let second = behavior.init().unwrap();

    assert_eq!(first.sends.schedules.len(), 1);
    assert_eq!(second.sends.schedules.len(), 1);
    assert_eq!(first.sends.schedules[0], second.sends.schedules[0]);
}

/// A second `Watch::init` emits a second, identical observe-peer send.
#[tokio::test]
async fn watch_double_init_duplicates_the_observe_send() {
    let peer = MailAddr(44);
    let mut behavior = Compose::new(Recorder::default()).watch(peer, stop_on_abnormal_death);
    let first = behavior.init().unwrap();
    let second = behavior.init().unwrap();

    assert_eq!(first.sends.observations.len(), 1);
    assert_eq!(second.sends.observations.len(), 1);
    assert_eq!(first.sends.observations[0], second.sends.observations[0]);
}

/// A second `Supervisor::init` re-emits the whole configured fleet: two
/// more creates at the SAME nonces plus two more observe sends, while the
/// slot table is untouched (the constructor pre-fills it). The interpreter
/// receives duplicate births at the same routes — unlike the proxy's panic
/// guard, there is no idempotence check.
#[tokio::test]
async fn supervising_double_init_duplicates_the_configured_fleet() {
    let mut behavior = Compose::new(Parent).children((2, child));
    let first = behavior.init().unwrap();
    let second = behavior.init().unwrap();

    assert_eq!(first.creates.len(), 2);
    assert_eq!(second.creates.len(), 2);
    assert_eq!(first.creates[0].nonce, second.creates[0].nonce); // same birth routes
    assert_eq!(first.sends.child_observations.len(), 2);
    assert_eq!(second.sends.child_observations.len(), 2);
    // The slot table is constructor-prefilled, not extended by init.
    assert_eq!(behavior.behavior().child_count(), 2);

    // A death still addresses the tracked slot.
    let actions = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(actions.sends.replacement_commands.len(), 1);
}

/// Full-stack double init duplicates every init send and the fleet.
#[tokio::test]
async fn full_stack_double_init_duplicates_every_init_effect() {
    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let mut behavior = Compose::new(Parent)
        .stash(|_| StashRoute::Deliver)
        .watch(peer, stop_on_abnormal_death)
        .deadline(Some(due), |_| Ok(Step::Continue))
        .children((2, child));
    let first = behavior.init().unwrap();
    let second = behavior.init().unwrap();

    assert_eq!(first.creates.len(), 2);
    assert_eq!(second.creates.len(), 2);
    assert_eq!(first.sends.child_observations.len(), 2);
    assert_eq!(second.sends.child_observations.len(), 2);
    assert_eq!(first.sends.behavior.schedules.len(), 1); // schedule
    assert_eq!(second.sends.behavior.schedules.len(), 1);
    assert_eq!(first.sends.behavior.behavior.observations.len(), 1); // observe-peer
    assert_eq!(second.sends.behavior.behavior.observations.len(), 1);
    assert_eq!(behavior.behavior().child_count(), 2);
}

/// `Proxy::step(Forward)` before `init` is inert because no worker birth has
/// been emitted yet.
#[tokio::test]
async fn proxy_step_before_init_is_inert_until_worker_birth() {
    let mut proxy = Proxy::new(child(0));
    let actions = proxy
        .transition(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(5),
        )))
        .unwrap();
    assert!(actions.sends.deliveries.is_empty());
    assert!(actions.creates.is_empty()); // no birth has been emitted

    let initial = proxy.init().unwrap();
    assert_eq!(initial.creates[0].nonce, 0);
}

/// `Supervisor::step` before `init` does NOT panic — the slot table is
/// constructor-prefilled — but emits a replacement targeting a proxy whose
/// birth (init) never happened: the same unborn-generation gap as the
/// proxy, at fleet scale.
#[tokio::test]
async fn supervisor_step_before_init_routes_to_unborn_proxies() {
    let mut behavior = Compose::new(Parent).children((2, child));
    let actions = behavior
        .transition(SupervisionEvent::WorkerStopped(WorkerStopped {
            proxy: 0,
            worker: 0,
            outcome: Err(Crash::Failed),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(actions.sends.replacement_commands.len(), 1);
    assert_eq!(
        actions.sends.replacement_commands[0].to.route(),
        Route::Child(0)
    );
    assert!(actions.creates.is_empty()); // no proxy has ever been born
    assert!(behavior.behavior().is_alive(0)); // slot bookkeeping pre-exists
}
