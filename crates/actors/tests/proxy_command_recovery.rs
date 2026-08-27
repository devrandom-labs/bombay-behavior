//! Owner-level regression for stable-proxy command ownership.

use behavior_actors::{
    Actions, Activate as _, Behavior, BehaviorActed, Births, CreationKind, CreationRejection,
    CreationResolved, IncarnationPhase, MailAddr, Never, NoBirths, Protocol, Proxy, ProxyEvent,
    ProxySends, ReplacementRequested, ShutdownRequested, Step, User,
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Command(u8);

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

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn assert_returned(
    actions: &Actions<MailAddr, Never, ProxySends<Worker>, Births<Worker>>,
    expected_phase: IncarnationPhase<u64>,
    expected: &Command,
) {
    assert!(actions.sends.deliveries.is_empty());
    assert!(actions.sends.child_observations.is_empty());
    assert!(actions.sends.creation_observations.is_empty());
    assert!(actions.sends.stopped_reports.is_empty());
    assert!(actions.sends.creation_reports.is_empty());
    assert!(actions.sends.shutdowns.is_empty());
    assert_eq!(actions.sends.unavailable_reports.len(), 1);
    let report = &actions.sends.unavailable_reports[0];
    assert_eq!(report.report.from, MailAddr(10));
    assert_eq!(report.report.phase, expected_phase);
    assert_eq!(&report.report.command, expected);
    assert!(actions.creates.is_empty());
    assert_eq!(actions.become_, Step::Continue);
}

#[test]
fn every_unavailable_phase_returns_the_complete_command_once() {
    let initialized = Worker.layer(Proxy::new).initialize().unwrap();
    assert!(initialized.actions.sends.deliveries.is_empty());
    assert!(initialized.actions.sends.unavailable_reports.is_empty());
    assert_eq!(initialized.actions.sends.child_observations.len(), 1);
    assert_eq!(initialized.actions.sends.creation_observations.len(), 1);
    assert!(initialized.actions.sends.stopped_reports.is_empty());
    assert!(initialized.actions.sends.creation_reports.is_empty());
    assert!(initialized.actions.sends.shutdowns.is_empty());
    assert!(matches!(
        initialized.actions.creates.as_slice(),
        [creation]
            if creation.nonce == 0 && creation.kind == CreationKind::Birth
    ));
    assert_eq!(initialized.actions.become_, Step::Continue);
    let mut proxy = initialized.behavior;

    let during_initial_install = Command(1);
    let returned = proxy
        .receive(MailAddr(10), during_initial_install.clone())
        .unwrap();
    assert_returned(
        &returned,
        IncarnationPhase::Installing {
            attempt: 0,
            kind: CreationKind::Birth,
        },
        &during_initial_install,
    );
    assert!(returned.creates.is_empty());

    let initialized = Worker.layer(Proxy::new).initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 1);
    let mut shutting_down_install = initialized.behavior;
    let shutdown = shutting_down_install
        .transition(ProxyEvent::ShutdownRequested(ShutdownRequested))
        .unwrap();
    assert!(shutdown.sends.deliveries.is_empty());
    assert!(shutdown.sends.unavailable_reports.is_empty());
    assert!(shutdown.sends.child_observations.is_empty());
    assert!(shutdown.sends.creation_observations.is_empty());
    assert!(shutdown.sends.stopped_reports.is_empty());
    assert!(shutdown.sends.creation_reports.is_empty());
    assert!(shutdown.sends.shutdowns.is_empty());
    assert!(shutdown.creates.is_empty());
    assert_eq!(shutdown.become_, Step::Continue);
    let during_install_shutdown = Command(7);
    let returned = shutting_down_install
        .receive(MailAddr(10), during_install_shutdown.clone())
        .unwrap();
    assert_returned(
        &returned,
        IncarnationPhase::InstallingDuringShutdown {
            attempt: 0,
            kind: CreationKind::Birth,
        },
        &during_install_shutdown,
    );

    let installed = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved::birth(
            0,
            MailAddr(70),
        )))
        .unwrap();
    assert!(installed.sends.deliveries.is_empty());
    assert!(installed.sends.unavailable_reports.is_empty());
    assert!(installed.sends.child_observations.is_empty());
    assert!(installed.sends.creation_observations.is_empty());
    assert!(installed.sends.stopped_reports.is_empty());
    assert!(installed.sends.shutdowns.is_empty());
    assert!(matches!(
        installed.sends.creation_reports.as_slice(),
        [report]
            if report.report.worker == 0
                && report.report.kind == CreationKind::Birth
                && report.report.result == Ok(())
    ));
    assert!(installed.creates.is_empty());
    assert_eq!(installed.become_, Step::Continue);
    let while_running = Command(2);
    let forwarded = proxy.receive(MailAddr(10), while_running.clone()).unwrap();
    assert_eq!(forwarded.sends.deliveries.len(), 1);
    assert_eq!(forwarded.sends.deliveries[0].nonce, 0);
    assert_eq!(forwarded.sends.deliveries[0].message, while_running);
    assert!(forwarded.sends.unavailable_reports.is_empty());

    let replacement_requested = proxy
        .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
            Worker,
        )))
        .unwrap();
    assert_eq!(replacement_requested.sends.shutdowns.len(), 1);
    assert_eq!(replacement_requested.sends.shutdowns[0].nonce, 0);
    assert!(replacement_requested.creates.is_empty());
    assert_eq!(replacement_requested.become_, Step::Continue);
    let while_stopping_for_replacement = Command(3);
    let returned = proxy
        .receive(MailAddr(10), while_stopping_for_replacement.clone())
        .unwrap();
    assert_returned(
        &returned,
        IncarnationPhase::AwaitingStop { incarnation: 0 },
        &while_stopping_for_replacement,
    );

    let stopped = proxy
        .transition(ProxyEvent::ChildStopped(
            behavior_actors::ChildStopped::new(
                0,
                Ok(behavior_actors::Exit::Normal),
                std::time::Instant::now(),
            ),
        ))
        .unwrap();
    assert_eq!(stopped.sends.child_observations.len(), 1);
    assert_eq!(stopped.sends.creation_observations.len(), 1);
    assert_eq!(stopped.sends.stopped_reports.len(), 1);
    assert_eq!(stopped.sends.stopped_reports[0].report.worker, 0);
    assert!(matches!(
        stopped.creates.as_slice(),
        [creation]
            if creation.nonce == 1
                && creation.kind == CreationKind::replacement_of(0)
    ));
    assert_eq!(stopped.become_, Step::Continue);
    let during_replacement_install = Command(4);
    let returned = proxy
        .receive(MailAddr(10), during_replacement_install.clone())
        .unwrap();
    assert_returned(
        &returned,
        IncarnationPhase::Installing {
            attempt: 1,
            kind: CreationKind::replacement_of(0),
        },
        &during_replacement_install,
    );

    let rejected = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved::new(
            1,
            CreationKind::replacement_of(0),
            Err(CreationRejection::NonceAlreadyBound),
        )))
        .unwrap();
    assert!(matches!(
        rejected.sends.creation_reports.as_slice(),
        [report]
            if report.report.worker == 1
                && report.report.kind == CreationKind::replacement_of(0)
                && report.report.result == Err(CreationRejection::NonceAlreadyBound)
    ));
    assert!(rejected.creates.is_empty());
    assert_eq!(rejected.become_, Step::Continue);
    let while_vacant = Command(5);
    let returned = proxy.receive(MailAddr(10), while_vacant.clone()).unwrap();
    assert_returned(
        &returned,
        IncarnationPhase::Vacant {
            last_installed: Some(0),
        },
        &while_vacant,
    );

    let requested = proxy
        .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
            Worker,
        )))
        .unwrap();
    assert_eq!(requested.sends.child_observations.len(), 1);
    assert_eq!(requested.sends.creation_observations.len(), 1);
    assert!(matches!(
        requested.creates.as_slice(),
        [creation]
            if creation.nonce == 2
                && creation.kind == CreationKind::replacement_of(0)
    ));
    assert_eq!(requested.become_, Step::Continue);
    let installed = proxy
        .transition(ProxyEvent::CreationResolved(CreationResolved::new(
            2,
            CreationKind::replacement_of(0),
            Ok(MailAddr(72)),
        )))
        .unwrap();
    assert_eq!(installed.sends.creation_reports.len(), 1);
    assert!(installed.creates.is_empty());
    assert_eq!(installed.become_, Step::Continue);
    let shutdown = proxy
        .transition(ProxyEvent::ShutdownRequested(ShutdownRequested))
        .unwrap();
    assert_eq!(shutdown.sends.shutdowns.len(), 1);
    assert_eq!(shutdown.sends.shutdowns[0].nonce, 2);
    assert!(shutdown.creates.is_empty());
    assert_eq!(shutdown.become_, Step::Continue);
    let during_shutdown = Command(6);
    let returned = proxy
        .receive(MailAddr(10), during_shutdown.clone())
        .unwrap();
    assert_returned(
        &returned,
        IncarnationPhase::ShuttingDown { incarnation: 2 },
        &during_shutdown,
    );
}
