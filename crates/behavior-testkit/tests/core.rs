use std::time::Duration;

use behavior::EventLayer;
use behavior::{
    Acted, Actions, Behavior, BehaviorActed, BehaviorBase, Births, ChildStopped, Crash, Create,
    CreationKind, CreationResolved, Delivery, EventIngress, Exit, Here, Machine, MailAddr, Move,
    Never, PeerStopped, Proxy, ProxyEvent, Recipient, ReplacementRequested, RestartPolicy,
    StashRoute, Step, SupervisionEvent, SupervisionLifecycle, TimerElapsed, TimerGeneration,
    TimerId, User, UserEvent, WorkerCreationResolved, WorkerStopped, stop_on_abnormal_death,
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
    let behavior = Recorder::default();
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
        behavior::Deadline::new(Recorder::default(), behavior::TimerId(0), Some(due), |_| {
            Step::Continue
        });
    let mut mailbox = Mailbox::new([]);
    let trace = drive(behavior, &mut mailbox).unwrap();

    assert_eq!(trace.transitions, 1);
    assert_eq!(trace.sends.owned.len(), 1);
    assert_eq!(trace.sends.owned[0].at, due);
}

#[tokio::test]
async fn stale_and_duplicate_time_observations_are_inert() {
    let due = Instant::now() + Duration::from_secs(2);
    let behavior =
        behavior::Deadline::new(Recorder::default(), behavior::TimerId(0), Some(due), |_| {
            Step::Stop(behavior::Stopped)
        });
    let mut mailbox = Mailbox::new([
        EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(1),
        }),
        EventLayer::Owned(TimerElapsed {
            id: TimerId(0),
            generation: TimerGeneration(0),
        }),
        EventLayer::Owned(TimerElapsed {
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
    let at_then_watch = behavior::Watch::new(
        behavior::Deadline::new(Recorder::default(), behavior::TimerId(0), Some(due), |_| {
            Step::Continue
        }),
        peer,
        stop_on_abnormal_death,
    );
    let initialized = at_then_watch.initialize().unwrap();
    let first = initialized.actions;
    let _at_then_watch = initialized.behavior;
    assert_eq!(first.sends.inner.owned[0].at, due);
    assert_eq!(first.sends.owned[0].peer, peer);

    let watch_then_at = behavior::Deadline::new(
        behavior::Watch::new(Recorder::default(), peer, stop_on_abnormal_death),
        behavior::TimerId(0),
        Some(due),
        |_| Step::Continue,
    );
    let initialized = watch_then_at.initialize().unwrap();
    let second = initialized.actions;
    let _watch_then_at = initialized.behavior;
    assert_eq!(second.sends.inner.owned[0].peer, peer);
    assert_eq!(second.sends.owned[0].at, due);
}

#[tokio::test]
async fn peer_fact_reaches_only_its_structurally_selected_watcher() {
    let inner_peer = MailAddr(1);
    let outer_peer = MailAddr(2);
    let behavior = behavior::Watch::new(
        behavior::Watch::new(Recorder::default(), inner_peer, stop_on_abnormal_death),
        outer_peer,
        stop_on_abnormal_death,
    );
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let stale_fact = PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    };
    let stale_outer = EventLayer::Owned(stale_fact.clone());
    assert!(matches!(
        behavior.transition(stale_outer),
        Err(behavior::TerminationMonitorError::UnexpectedFact {
            observation: behavior::TerminationObservation::Observing,
            fact,
        }) if fact == stale_fact
    ));

    let selected_inner = EventLayer::Inner(EventLayer::Owned(PeerStopped {
        peer: inner_peer,
        outcome: Err(Crash::Failed),
    }));
    let actions = behavior.transition(selected_inner).unwrap();
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
    let behavior = behavior::Stash::new(Recorder::default(), |message| match message {
        0 => StashRoute::Release,
        1..=9 => StashRoute::Stash,
        _ => StashRoute::Deliver,
    });
    let initialized = behavior.initialize().unwrap();
    let mut behavior = initialized.behavior;
    let mut effect_trace = Vec::new();
    for (from, message) in [
        (MailAddr(1), 3),  // Stash
        (MailAddr(2), 3),  // Stash
        (MailAddr(3), 42), // Deliver — passes straight through
        (MailAddr(9), 0),  // Release — trigger delivered, held pair re-routed
    ] {
        let actions = behavior.transition(UserEvent::user(from, message)).unwrap();
        effect_trace.extend(
            actions
                .sends
                .iter()
                .map(|delivery| (delivery.to.address(), delivery.message)),
        );
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }

    assert_eq!(behavior.base().seen, [(MailAddr(3), 42), (MailAddr(9), 0)]);
    assert_eq!(effect_trace, behavior.base().seen);
    assert_eq!(behavior.held(), 2);

    // A second release neither replays nor drops the held pair.
    let released = behavior
        .transition(UserEvent::user(MailAddr(9), 0))
        .unwrap();
    effect_trace.extend(
        released
            .sends
            .iter()
            .map(|delivery| (delivery.to.address(), delivery.message)),
    );
    assert!(released.creates.is_empty());
    assert!(matches!(released.become_, Step::Continue));
    assert_eq!(behavior.base().seen.len(), 3);
    assert_eq!(effect_trace, behavior.base().seen);
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
    let machine = Machine::new(
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
    let initialized = machine.initialize().unwrap();
    let mut machine = initialized.behavior;
    for message in [Message::Value(1), Message::Value(1), Message::Open] {
        let actions = machine
            .transition(User::user(MailAddr(0), message))
            .unwrap();
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }

    assert_eq!(machine.state(), &[1, 1]);
    assert_eq!(machine.held(), 0);
}

type Child = Recorder;

struct Parent(bool);

enum ParentEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    User(User<MailAddr, u64>),
}

impl UserEvent for ParentEvent {
    type Addr = MailAddr;
    type Message = u64;

    fn user(from: MailAddr, message: u64) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<MailAddr, u64>, Self> {
        match self {
            Self::User(user) => Ok(user),
            lifecycle => Err(lifecycle),
        }
    }
}

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for ParentEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl behavior::Protocol for Parent {
    type Addr = MailAddr;
    type Msg = u64;
}

impl BehaviorBase for Parent {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl Behavior for Parent {
    type Protocol = Self;
    type Event = ParentEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ParentEvent::Lifecycle(_lifecycle) => Ok(Actions::cont()),
            ParentEvent::User(event) => {
                let creates = if self.0 {
                    Vec::new()
                } else {
                    self.0 = true;
                    vec![Create::birth(event.message, child(0))]
                };
                Ok(Actions::new(Vec::new(), creates, Step::Continue))
            }
        }
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
    let installed = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 0,
            kind: CreationKind::Birth,
            result: Ok(MailAddr(999)),
        }))
        .unwrap();
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable_reports.is_empty());
    assert!(installed.sends.child_observations.is_empty());
    assert!(installed.sends.creation_observations.is_empty());
    assert!(installed.sends.stopped_reports.is_empty());
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(installed.sends.shutdowns.is_empty());
    assert!(installed.creates.is_empty());
    assert!(matches!(installed.become_, Step::Continue));

    let before = proxy
        .transition(ProxyEvent::Command(User::user(MailAddr(0), 5)))
        .unwrap();
    assert_eq!(before.sends.deliveries[0].nonce, 0);

    let replacement = proxy
        .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
            child(0),
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
    let installed = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved {
            nonce: 1,
            kind: CreationKind::ReplacementIncarnation { replaces: 0 },
            result: Ok(MailAddr(999)),
        }))
        .unwrap();
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable_reports.is_empty());
    assert!(installed.sends.child_observations.is_empty());
    assert!(installed.sends.creation_observations.is_empty());
    assert!(installed.sends.stopped_reports.is_empty());
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(installed.sends.shutdowns.is_empty());
    assert!(installed.creates.is_empty());
    assert!(matches!(installed.become_, Step::Continue));

    let after = proxy
        .transition(ProxyEvent::Command(User::user(MailAddr(0), 6)))
        .unwrap();
    assert_eq!(after.sends.deliveries[0].nonce, 1);
}

#[tokio::test]
async fn restart_window_boundary_is_inclusive() {
    let start = Instant::now();
    let supervisor = behavior::Supervise::new(
        Parent(true),
        behavior::ChildTopology::new((0..1).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(5),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
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
            .owned
            .replacement_inputs
            .len(),
        1
    );
    let joined = supervisor
        .transition(SupervisionEvent::WorkerCreationResolved(
            WorkerCreationResolved::new(
                0,
                1,
                CreationKind::ReplacementIncarnation { replaces: 0 },
                Ok(()),
            ),
        ))
        .unwrap();
    assert!(joined.sends.owned.child_observations.is_empty());
    assert!(joined.sends.owned.creation_observations.is_empty());
    assert!(joined.sends.owned.schedules.is_empty());
    assert!(joined.sends.owned.replacement_inputs.is_empty());
    assert!(joined.sends.owned.failure_reports.is_empty());
    assert!(joined.sends.owned.shutdowns.is_empty());
    assert!(joined.sends.inner.is_empty());
    assert!(joined.creates.is_empty());
    assert!(matches!(joined.become_, Step::Continue));

    let edge = SupervisionEvent::WorkerStopped(WorkerStopped {
        proxy: 0,
        worker: 1,
        outcome: Err(Crash::Failed),
        at: start + Duration::from_secs(5),
    });
    assert!(
        supervisor
            .transition(edge)
            .unwrap()
            .sends
            .owned
            .replacement_inputs
            .is_empty()
    );
}

#[tokio::test]
async fn application_births_are_not_adopted_by_fixed_supervision() {
    let supervisor = behavior::Supervise::new(
        Parent(false),
        behavior::ChildTopology::new((0..1).map(|index| u64::try_from(index).unwrap()), |index| {
            Some(child(index))
        }),
        behavior::RestartConfiguration::new(
            behavior::Strategy::OneForOne,
            behavior::RestartPolicy::Transient,
            1,
            std::time::Duration::from_secs(5),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let mut supervisor = initialized.behavior;
    let acted = supervisor
        .transition(UserEvent::user(MailAddr(0), 0))
        .unwrap();
    assert_eq!(acted.creates.len(), 1);
    assert!(acted.sends.owned.child_observations.is_empty());
    assert!(acted.sends.owned.creation_observations.is_empty());
    assert!(acted.sends.owned.failure_reports.is_empty());
    assert_eq!(supervisor.child_count(), 1);
}
use behavior_testkit::InitializeTest;
