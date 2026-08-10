use std::time::Duration;

use behavior::{
    Acted, Actions, AtEvent, Base, Behavior, ChildStopped, Crash, Create, CreationKind,
    CreationResolved, Delivery, Exit, MailAddr, Move, Never, PeerStopped, Proxy, ProxyCommand,
    ProxyEvent, Recipient, RestartPolicy, Route, Spec, StashRoute, State, Step, Strategy,
    SupervisionEvent, TimerElapsed, TimerGeneration, TimerId, User, UserEvent, WatchEvent,
    WorkerStopped, stop_on_abnormal_death,
};
use behavior_testkit::{Mailbox, drive};
use tokio::time::Instant;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

impl State<u8, behavior::NoBirths, Never> for Recorder {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: if message == u8::MAX {
                Step::Stop(Exit::Normal)
            } else {
                Step::Continue
            },
        })
    }
}

#[tokio::test]
async fn owned_mailbox_is_fifo_and_stops_without_consuming_tail() {
    let mut behavior = Base::new(Recorder::default());
    let mut mailbox = Mailbox::new([
        User::user(MailAddr(1), 3),
        User::user(MailAddr(2), u8::MAX),
        User::user(MailAddr(3), 7),
    ]);
    let trace = drive(&mut behavior, &mut mailbox).await.unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 1);
    assert_eq!(trace.exit, Some(Exit::Normal));
    assert_eq!(trace.sends.len(), 2);
    assert_eq!(trace.sends[0].message, 3);
    assert_eq!(trace.sends[1].message, u8::MAX);
}

#[tokio::test]
async fn empty_mailbox_still_observes_initialization_exactly_once() {
    let due = Instant::now() + Duration::from_secs(1);
    let mut behavior = Spec::new(Recorder::default()).at(Some(due), |_| Ok(Step::Continue));
    let mut mailbox = Mailbox::new([]);
    let trace = drive(&mut behavior, &mut mailbox).await.unwrap();

    assert_eq!(trace.transitions, 1);
    assert_eq!(trace.sends.own.len(), 1);
    assert_eq!(trace.sends.own[0].at, due);
}

#[tokio::test]
async fn stale_and_duplicate_time_observations_are_inert() {
    let due = Instant::now() + Duration::from_secs(2);
    let mut behavior =
        Spec::new(Recorder::default()).at(Some(due), |_| Ok(Step::Stop(Exit::Normal)));
    let mut mailbox = Mailbox::new([
        AtEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }),
        AtEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }),
        AtEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }),
    ]);
    let trace = drive(&mut behavior, &mut mailbox).await.unwrap();

    assert_eq!(trace.exit, Some(Exit::Normal));
    assert_eq!(trace.pending, 1);
    assert_eq!(trace.transitions, 3);
}

#[tokio::test]
async fn wrapper_orderings_preserve_both_initial_protocols() {
    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let mut at_then_watch = Spec::new(Recorder::default())
        .at(Some(due), |_| Ok(Step::Continue))
        .watch(peer, stop_on_abnormal_death);
    let first = at_then_watch.init().await.unwrap();
    assert_eq!(first.sends.inner.own[0].at, due);
    assert_eq!(first.sends.own[0].peer, peer);

    let mut watch_then_at = Spec::new(Recorder::default())
        .watch(peer, stop_on_abnormal_death)
        .at(Some(due), |_| Ok(Step::Continue));
    let second = watch_then_at.init().await.unwrap();
    assert_eq!(second.sends.inner.own[0].peer, peer);
    assert_eq!(second.sends.own[0].at, due);
}

#[tokio::test]
async fn unrelated_peer_event_passes_to_the_inner_watcher() {
    let inner_peer = MailAddr(1);
    let outer_peer = MailAddr(2);
    let mut behavior = Spec::new(Recorder::default())
        .watch(inner_peer, stop_on_abnormal_death)
        .watch(outer_peer, stop_on_abnormal_death);
    behavior.init().await.unwrap();
    let event = WatchEvent::PeerStopped(PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    });
    let actions = behavior.step(event).await.unwrap();

    assert_eq!(actions.become_, Step::Stop(Exit::LinkDied(inner_peer)));
}

// A `Release` first delivers its own trigger message, then drains the held
// queue by RE-ROUTING each held message through the route fn. The route fn is
// a pure function of the message, so a message stashed on arrival re-routes to
// `Stash` again during the drain and stays held: release replays nothing under
// a pure route (the frozen crate test `stashing_is_local_state_and_replay`
// pins this). The invariants that DO hold: held messages are never lost or
// duplicated, their FIFO order and origins survive any number of releases, and
// `Deliver`-routed messages pass straight through with their origin intact.
#[tokio::test]
async fn stash_holds_in_fifo_order_without_loss_or_duplication_across_releases() {
    let mut behavior = Spec::new(Recorder::default()).stash(|message| match message {
        0 => StashRoute::Release,
        1..=9 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    for (from, message) in [
        (MailAddr(1), 3),  // Stash
        (MailAddr(2), 3),  // Stash
        (MailAddr(3), 42), // Deliver — passes straight through
        (MailAddr(9), 0),  // Release — trigger delivered, held pair re-routed
    ] {
        behavior.step(UserEvent::user(from, message)).await.unwrap();
    }

    assert_eq!(
        behavior.behavior().inner().state().seen,
        [(MailAddr(3), 42), (MailAddr(9), 0)]
    );
    assert_eq!(behavior.behavior().held(), 2);

    // A second release neither replays nor drops the held pair.
    behavior
        .step(UserEvent::user(MailAddr(9), 0))
        .await
        .unwrap();
    assert_eq!(behavior.behavior().inner().state().seen.len(), 3);
    assert_eq!(behavior.behavior().held(), 2);
}

#[tokio::test]
async fn fsm_replays_deferred_messages_once_after_phase_change() {
    #[derive(Clone, Copy, PartialEq)]
    enum Phase {
        Closed,
        Open,
    }
    #[derive(Clone, Copy)]
    enum Message {
        Value(u8),
        Open,
    }
    let mut machine = Spec::machine(
        Vec::new(),
        Phase::Closed,
        |phase, seen: &mut Vec<u8>, message| {
            Ok::<Move<Phase>, Never>(match (phase, message) {
                (Phase::Closed, Message::Value(_)) => Move::Defer,
                (_, Message::Value(value)) => {
                    seen.push(*value);
                    Move::Stay
                }
                (_, Message::Open) => Move::Goto(Phase::Open),
            })
        },
    );
    for message in [Message::Value(1), Message::Value(1), Message::Open] {
        machine
            .step(User::user(MailAddr(0), message))
            .await
            .unwrap();
    }

    assert_eq!(machine.behavior().state(), &[1, 1]);
    assert_eq!(machine.behavior().held(), 0);
}

type Child = Base<Recorder, u8>;

struct Parent(bool);

impl State<Never, behavior::Births<Child>, Never> for Parent {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, behavior::Births<Child>, Never>
    {
        let creates = if self.0 {
            Vec::new()
        } else {
            self.0 = true;
            vec![Create::birth(nonce, child(0))]
        };
        Ok(Actions {
            sends: Vec::new(),
            creates,
            become_: Step::Continue,
        })
    }
}

fn child(_index: usize) -> Child {
    Base::new(Recorder::default())
}

#[tokio::test]
async fn proxy_forwards_only_to_the_current_fresh_generation() {
    let mut proxy = Proxy::new(child(0));
    assert_eq!(proxy.init().await.unwrap().creates[0].nonce, 0);
    proxy
        .step(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(()),
        }))
        .await
        .unwrap();

    let before = proxy
        .step(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(5),
        )))
        .await
        .unwrap();
    assert_eq!(before.sends.deliveries[0].to.route(), Route::Child(0));

    let replacement = proxy
        .step(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .await
        .unwrap();
    assert!(replacement.creates.is_empty());

    let replacement = proxy
        .step(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Ok(Exit::Normal),
            at: Instant::now(),
        }))
        .await
        .unwrap();
    assert_eq!(replacement.creates[0].nonce, 1);
    proxy
        .step(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 1,
            kind: CreationKind::ReplacementIncarnation { replaces: 0 },
            result: Ok(()),
        }))
        .await
        .unwrap();

    let after = proxy
        .step(ProxyEvent::Inner(User::user(
            MailAddr(0),
            ProxyCommand::Forward(6),
        )))
        .await
        .unwrap();
    assert_eq!(after.sends.deliveries[0].to.route(), Route::Child(1));
}

#[tokio::test]
async fn restart_window_boundary_is_inclusive() {
    let start = Instant::now();
    let mut supervisor = Spec::new(Parent(true))
        .children((1, child))
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Permanent)
        .within(1, Duration::from_secs(5));
    supervisor.init().await.unwrap();

    let first = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 0,
        worker: 0,
        outcome: Ok(Exit::Normal),
        at: start,
    });
    assert_eq!(
        supervisor
            .step(first)
            .await
            .unwrap()
            .sends
            .replacement_commands
            .len(),
        1
    );

    let edge = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 0,
        worker: 0,
        outcome: Err(Crash::Failed),
        at: start + Duration::from_secs(5),
    });
    assert!(
        supervisor
            .step(edge)
            .await
            .unwrap()
            .sends
            .replacement_commands
            .is_empty()
    );
}

#[tokio::test]
#[should_panic(expected = "a child birth nonce must be fresh")]
async fn duplicate_dynamic_birth_is_rejected() {
    let mut supervisor = Spec::new(Parent(false)).children((1, child));
    supervisor.init().await.unwrap();
    supervisor
        .step(UserEvent::user(MailAddr(0), 0))
        .await
        .unwrap();
}
