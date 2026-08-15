use std::time::Duration;

use behavior::{
    Acted, Actions, ChildStopped, Compose, Crash, Create, CreationKind, CreationResolved,
    DeadlineEvent, Delivery, Exit, Machine, MailAddr, Move, Never, PeerStopped, Proxy,
    ProxyCommand, ProxyEvent, Recipient, RestartPolicy, StashRoute, Step, Strategy,
    SupervisionEvent, TimerElapsed, TimerGeneration, TimerId, User, UserEvent, WatchEvent,
    WorkerStopped, stop_on_abnormal_death,
};
use behavior_testkit::{Mailbox, drive};
use std::time::Instant;

#[derive(Default)]
struct Recorder {
    seen: Vec<(MailAddr, u8)>,
}

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Recorder {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        self.seen.push((from, message));
        Ok(Actions {
            sends: vec![Delivery::new(Recipient::global(from), message)],
            creates: Vec::new(),
            become_: if message == u8::MAX {
                Step::Stop(behavior::Stopped)
            } else {
                Step::Continue
            },
        })
    }
}

#[tokio::test]
async fn owned_mailbox_is_fifo_and_stops_without_consuming_tail() {
    let behavior = Compose::new(Recorder::default());
    let mut mailbox = Mailbox::new([
        User::user(MailAddr(1), 3),
        User::user(MailAddr(2), u8::MAX),
        User::user(MailAddr(3), 7),
    ]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 3);
    assert_eq!(trace.pending, 1);
    assert!(trace.stopped);
    assert_eq!(trace.sends.len(), 2);
    assert_eq!(trace.sends[0].message, 3);
    assert_eq!(trace.sends[1].message, u8::MAX);
}

#[tokio::test]
async fn empty_mailbox_still_observes_initialization_exactly_once() {
    let due = Instant::now() + Duration::from_secs(1);
    let behavior =
        Compose::new(Recorder::default())
            .deadline(behavior::TimerId(0), Some(due), |_| Ok(Step::Continue));
    let mut mailbox = Mailbox::new([]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 1);
    assert_eq!(trace.sends.schedules.len(), 1);
    assert_eq!(trace.sends.schedules[0].at, due);
}

#[tokio::test]
async fn stale_and_duplicate_time_observations_are_inert() {
    let due = Instant::now() + Duration::from_secs(2);
    let behavior =
        Compose::new(Recorder::default()).deadline(behavior::TimerId(0), Some(due), |_| {
            Ok(Step::Stop(behavior::Stopped))
        });
    let mut mailbox = Mailbox::new([
        DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }),
        DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }),
        DeadlineEvent::Elapsed(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }),
    ]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert!(trace.stopped);
    assert_eq!(trace.pending, 1);
    assert_eq!(trace.transitions, 3);
}

#[tokio::test]
async fn wrapper_orderings_preserve_both_initial_protocols() {
    let due = Instant::now() + Duration::from_secs(1);
    let peer = MailAddr(44);
    let at_then_watch = Compose::new(Recorder::default())
        .deadline(behavior::TimerId(0), Some(due), |_| Ok(Step::Continue))
        .watch(peer, stop_on_abnormal_death);
    let initialized = at_then_watch.initialize().unwrap();
    let first = initialized.actions;
    let _at_then_watch = initialized.behavior;
    assert_eq!(first.sends.behavior.schedules[0].at, due);
    assert_eq!(first.sends.observations[0].peer, peer);

    let watch_then_at = Compose::new(Recorder::default())
        .watch(peer, stop_on_abnormal_death)
        .deadline(behavior::TimerId(0), Some(due), |_| Ok(Step::Continue));
    let initialized = watch_then_at.initialize().unwrap();
    let second = initialized.actions;
    let _watch_then_at = initialized.behavior;
    assert_eq!(second.sends.behavior.observations[0].peer, peer);
    assert_eq!(second.sends.schedules[0].at, due);
}

#[tokio::test]
async fn unrelated_peer_event_passes_to_the_inner_watcher() {
    let inner_peer = MailAddr(1);
    let outer_peer = MailAddr(2);
    let behavior = Compose::new(Recorder::default())
        .watch(inner_peer, stop_on_abnormal_death)
        .watch(outer_peer, stop_on_abnormal_death);
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let event = WatchEvent::PeerStopped(PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    });
    let actions = behavior.transition(event).unwrap();

    assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
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
    let behavior = Compose::new(Recorder::default()).stash(|message| match message {
        0 => StashRoute::Release,
        1..=9 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    for (from, message) in [
        (MailAddr(1), 3),  // Stash
        (MailAddr(2), 3),  // Stash
        (MailAddr(3), 42), // Deliver — passes straight through
        (MailAddr(9), 0),  // Release — trigger delivered, held pair re-routed
    ] {
        behavior.transition(UserEvent::user(from, message)).unwrap();
    }

    assert_eq!(behavior.base().seen, [(MailAddr(3), 42), (MailAddr(9), 0)]);
    assert_eq!(behavior.held(), 2);

    // A second release neither replays nor drops the held pair.
    behavior
        .transition(UserEvent::user(MailAddr(9), 0))
        .unwrap();
    assert_eq!(behavior.base().seen.len(), 3);
    assert_eq!(behavior.held(), 2);
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
    let machine = Compose::new(Machine::new(
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
    ));
    let initialized = machine.initialize().unwrap();
    let mut machine = initialized.behavior;
    for message in [Message::Value(1), Message::Value(1), Message::Open] {
        machine
            .transition(User::user(MailAddr(0), message))
            .unwrap();
    }

    assert_eq!(machine.state(), &[1, 1]);
    assert_eq!(machine.held(), 0);
}

type Child = Recorder;

struct Parent(bool);

#[behavior::behavior(addr = MailAddr, message = u64, sends = Vec<Never>, births = behavior::Births<Child>, error = Never)]
impl Parent {
    fn receive(
        &mut self,
        _from: MailAddr,
        nonce: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::Births<Child>, Never> {
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
    Recorder::default()
}

#[tokio::test]
async fn proxy_forwards_only_to_the_current_fresh_generation() {
    let initialized = Proxy::new(child(0)).initialize().unwrap();
    assert_eq!(initialized.actions.creates[0].nonce, 0);
    let mut proxy = initialized.behavior;
    proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(()),
        }))
        .unwrap();

    let before = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Forward(5),
        )))
        .unwrap();
    assert_eq!(
        before.sends.deliveries[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 0)
    );

    let replacement = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Replace(child(0)),
        )))
        .unwrap();
    assert!(replacement.creates.is_empty());

    let replacement = proxy
        .transition(ProxyEvent::ChildStopped(ChildStopped {
            nonce: 0,
            outcome: Ok(Exit::Normal),
            at: Instant::now(),
        }))
        .unwrap();
    assert_eq!(replacement.creates[0].nonce, 1);
    proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 1,
            kind: CreationKind::ReplacementIncarnation { replaces: 0 },
            result: Ok(()),
        }))
        .unwrap();

    let after = proxy
        .transition(ProxyEvent::Command(User::user(
            MailAddr(0),
            ProxyCommand::Forward(6),
        )))
        .unwrap();
    assert_eq!(
        after.sends.deliveries[0].to.resolve(MailAddr(17)),
        behavior::Address::birth(MailAddr(17), 1)
    );
}

#[tokio::test]
async fn restart_window_boundary_is_inclusive() {
    let start = Instant::now();
    let supervisor = Compose::new(Parent(true))
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap()
        .restart(Strategy::OneForOne)
        .when(RestartPolicy::Permanent)
        .within(1, Duration::from_secs(5));
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;

    let first = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 0,
        worker: 0,
        outcome: Ok(Exit::Normal),
        at: start,
    });
    assert_eq!(
        supervisor
            .transition(first)
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
            .transition(edge)
            .unwrap()
            .sends
            .replacement_commands
            .is_empty()
    );
}

#[tokio::test]
async fn duplicate_dynamic_birth_is_rejected() {
    let supervisor = Compose::new(Parent(false))
        .children(
            |index| u64::try_from(index).unwrap(),
            1,
            |index| Some(child(index)),
        )
        .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    assert!(matches!(
        supervisor.transition(UserEvent::user(MailAddr(0), 0)),
        Err(behavior::SupervisorError::Fleet(
            behavior::FleetError::DuplicateChild(0)
        ))
    ));
}
use behavior_testkit::InitializeTest;
