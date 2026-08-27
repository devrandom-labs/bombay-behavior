//! Customer-facing availability laws for stable supervised capabilities.
//!
//! These tests use the worker's public protocol and the proxy owner's typed
//! parent ingress. Their oracle is that expected unavailability remains an
//! ordinary successful fold with the complete command returned; it must not
//! fail the actor transition.

use behavior::{
    Activate as _, Births, ChildStopped, Crash, CreationResolved, IncarnationPhase, Proxy,
    ProxyEvent, ProxySends, ReplacementRequested, ShutdownRequested,
};
use foundation::{
    Actions, Behavior, BehaviorActed, CreationKind, CreationRejection, MailAddr, Never, NoBirths,
    Protocol, Step, User,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Command {
    value: u8,
}

fn command(value: u8) -> Command {
    Command { value }
}

struct Worker;

impl Protocol for Worker {
    type Addr = MailAddr;
    type Msg = Command;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, Command>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn assert_unavailable(
    actions: Actions<MailAddr, Never, ProxySends<Worker>, Births<Worker>>,
    phase: IncarnationPhase<u64>,
    command: Command,
) {
    assert!(actions.sends.deliveries.is_empty());
    assert!(actions.sends.child_observations.is_empty());
    assert!(actions.sends.creation_observations.is_empty());
    assert!(actions.sends.stopped_reports.is_empty());
    assert!(actions.sends.creation_reports.is_empty());
    assert!(actions.sends.shutdowns.is_empty());
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
    assert_eq!(actions.sends.unavailable_reports.len(), 1);
    let returned = &actions.sends.unavailable_reports[0].report;
    assert_eq!(returned.from, MailAddr(4));
    assert_eq!(returned.phase, phase);
    assert_eq!(returned.command, command);
}

fn assert_proxy_effect_counts(
    actions: &Actions<MailAddr, Never, ProxySends<Worker>, Births<Worker>>,
    child_observations: usize,
    creation_observations: usize,
    stopped_reports: usize,
    creation_reports: usize,
    shutdowns: usize,
    creates: usize,
) {
    assert!(actions.sends.deliveries.is_empty());
    assert!(actions.sends.unavailable_reports.is_empty());
    assert_eq!(actions.sends.child_observations.len(), child_observations);
    assert_eq!(
        actions.sends.creation_observations.len(),
        creation_observations
    );
    assert_eq!(actions.sends.stopped_reports.len(), stopped_reports);
    assert_eq!(actions.sends.creation_reports.len(), creation_reports);
    assert_eq!(actions.sends.shutdowns.len(), shutdowns);
    assert_eq!(actions.creates.len(), creates);
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn admitted_command_during_initial_installation_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    let actions = proxy
        .transition(ProxyEvent::Command(User::new(MailAddr(4), command(9))))
        .expect("mailbox-admitted expected unavailability must remain in Actions");
    assert_unavailable(
        actions,
        IncarnationPhase::Installing {
            attempt: 0,
            kind: CreationKind::Birth,
        },
        command(9),
    );
}

#[test]
fn admitted_command_after_rejected_birth_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    let rejected = proxy
        .on_path(CreationResolved::rejected(
            0,
            CreationKind::Birth,
            CreationRejection::InitializationFailed,
        ))
        .unwrap();
    assert_proxy_effect_counts(&rejected, 0, 0, 0, 1, 0, 0);

    let actions = proxy
        .transition(ProxyEvent::Command(User::new(MailAddr(4), command(10))))
        .unwrap();
    assert_unavailable(
        actions,
        IncarnationPhase::Vacant {
            last_installed: None,
        },
        command(10),
    );
}

#[test]
fn admitted_command_during_replacement_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    let installed = proxy
        .on_path(CreationResolved::birth(0, MailAddr(40)))
        .unwrap();
    assert_proxy_effect_counts(&installed, 0, 0, 0, 1, 0, 0);
    let replacing = proxy
        .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
            Worker,
        )))
        .unwrap();
    assert_proxy_effect_counts(&replacing, 0, 0, 0, 0, 1, 0);
    let stopped = proxy
        .on_path(ChildStopped::new(
            0,
            Err(Crash::Failed),
            std::time::Instant::now(),
        ))
        .unwrap();
    assert_proxy_effect_counts(&stopped, 1, 1, 1, 0, 0, 1);

    let actions = proxy
        .transition(ProxyEvent::Command(User::new(MailAddr(4), command(11))))
        .unwrap();
    assert_unavailable(
        actions,
        IncarnationPhase::Installing {
            attempt: 1,
            kind: CreationKind::replacement_of(0),
        },
        command(11),
    );
}

#[test]
fn admitted_command_during_shutdown_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    let installed = proxy
        .on_path(CreationResolved::birth(0, MailAddr(40)))
        .unwrap();
    assert_proxy_effect_counts(&installed, 0, 0, 0, 1, 0, 0);
    let requested = proxy.on_path(ShutdownRequested).unwrap();
    assert_proxy_effect_counts(&requested, 0, 0, 0, 0, 1, 0);

    let actions = proxy
        .transition(ProxyEvent::Command(User::new(MailAddr(4), command(12))))
        .unwrap();
    assert_unavailable(
        actions,
        IncarnationPhase::ShuttingDown { incarnation: 0 },
        command(12),
    );
}
