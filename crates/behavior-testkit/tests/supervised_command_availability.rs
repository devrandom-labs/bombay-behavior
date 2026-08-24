//! Customer-facing availability laws for stable supervised capabilities.
//!
//! These tests use only the public proxy protocol and lifecycle facts. Their
//! oracle is that expected unavailability remains an ordinary successful fold
//! with an observable typed response; it must not fail the actor transition.

use behavior::{
    Activate as _, Births, ChildStopped, ChildTopology, Crash, CreationResolved, IncarnationPhase,
    InterruptionPolicy, JobId, PoolAssignment, PoolConfiguration, PoolMessage, PoolResponse, Proxy,
    ProxyCommand, ProxyEvent, ProxySends, Recipient, RestartPolicy, ShutdownRequested,
    WorkerCreationResolved, WorkerPool, WorkerPoolProtocol,
};
use foundation::{
    Actions, Behavior, BehaviorActed, CreationKind, CreationRejection, MailAddr, Never, NoBirths,
    Protocol, Step, User,
};
use std::time::Duration;

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

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn forward(command: Command, customer: MailAddr) -> ProxyCommand<Worker> {
    ProxyCommand::Forward {
        command,
        unavailable_to: Recipient::global(customer),
    }
}

fn assert_unavailable(
    actions: Actions<MailAddr, Never, ProxySends<Worker>, Births<Worker>>,
    customer: MailAddr,
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
    assert_eq!(actions.sends.unavailable.len(), 1);
    let returned = &actions.sends.unavailable[0];
    assert_eq!(returned.to, Recipient::global(customer));
    assert_eq!(returned.message.phase, phase);
    assert_eq!(returned.message.command, command);
}

#[test]
fn admitted_command_during_initial_installation_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    let actions = proxy
        .transition(ProxyEvent::Command(User::new(
            MailAddr(4),
            forward(Command(9), MailAddr(41)),
        )))
        .expect("mailbox-admitted expected unavailability must remain in Actions");
    assert_unavailable(
        actions,
        MailAddr(41),
        IncarnationPhase::Installing {
            attempt: 0,
            kind: CreationKind::Birth,
        },
        Command(9),
    );
}

#[test]
fn admitted_command_after_rejected_birth_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    proxy
        .on_path(CreationResolved::rejected(
            0,
            CreationKind::Birth,
            CreationRejection::InitializationFailed,
        ))
        .unwrap();

    let actions = proxy
        .transition(ProxyEvent::Command(User::new(
            MailAddr(4),
            forward(Command(10), MailAddr(42)),
        )))
        .unwrap();
    assert_unavailable(
        actions,
        MailAddr(42),
        IncarnationPhase::Vacant {
            last_installed: None,
        },
        Command(10),
    );
}

#[test]
fn admitted_command_during_replacement_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    proxy
        .on_path(CreationResolved::birth(0, MailAddr(40)))
        .unwrap();
    proxy
        .transition(ProxyEvent::Command(User::new(
            MailAddr(4),
            ProxyCommand::Replace(Worker),
        )))
        .unwrap();
    proxy
        .on_path(ChildStopped::new(
            0,
            Err(Crash::Failed),
            std::time::Instant::now(),
        ))
        .unwrap();

    let actions = proxy
        .transition(ProxyEvent::Command(User::new(
            MailAddr(4),
            forward(Command(11), MailAddr(43)),
        )))
        .unwrap();
    assert_unavailable(
        actions,
        MailAddr(43),
        IncarnationPhase::Installing {
            attempt: 1,
            kind: CreationKind::replacement_of(0),
        },
        Command(11),
    );
}

#[test]
fn admitted_command_during_shutdown_is_expected_unavailability_not_actor_failure() {
    let mut proxy = Proxy::new(Worker).initialize().unwrap().behavior;
    proxy
        .on_path(CreationResolved::birth(0, MailAddr(40)))
        .unwrap();
    proxy.on_path(ShutdownRequested).unwrap();

    let actions = proxy
        .transition(ProxyEvent::Command(User::new(
            MailAddr(4),
            forward(Command(12), MailAddr(44)),
        )))
        .unwrap();
    assert_unavailable(
        actions,
        MailAddr(44),
        IncarnationPhase::ShuttingDown { incarnation: 0 },
        Command(12),
    );
}

struct PoolReplies;

impl Protocol for PoolReplies {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, (), MailAddr>;
}

type PoolRoute = Recipient<PoolReplies>;
type PoolProtocol = WorkerPoolProtocol<MailAddr, u8, (), PoolRoute>;

struct PoolWorker;

impl Protocol for PoolWorker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<PoolProtocol>;
}

impl Behavior for PoolWorker {
    type Protocol = Self;
    type Event = User<MailAddr, PoolAssignment<PoolProtocol>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn pool_worker(_: usize) -> Option<PoolWorker> {
    Some(PoolWorker)
}

#[test]
fn pool_joins_assignment_return_and_worker_stop_in_both_orders_without_losing_the_job() {
    for (maximum_restarts, replacement_expected) in [(2, true), (0, false)] {
        for command_returned_first in [true, false] {
            let initialized = WorkerPool::new(
                ChildTopology::new([7], pool_worker),
                PoolConfiguration::new(
                    0,
                    InterruptionPolicy::Fail,
                    RestartPolicy::Permanent,
                    maximum_restarts,
                    Duration::from_secs(30),
                ),
                Recipient::global(MailAddr(9)),
            )
            .unwrap()
            .initialize()
            .unwrap();
            let creation = initialized.actions.creates.into_iter().next().unwrap();
            let mut pool = initialized.behavior;
            let mut proxy = creation.child.initialize().unwrap().behavior;

            proxy
                .on_path(CreationResolved::birth(0, MailAddr(100)))
                .unwrap();
            pool.on_path(WorkerCreationResolved::new(
                7,
                0,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
            let dispatched = pool
                .receive(
                    MailAddr(90),
                    PoolMessage::Submit {
                        job: JobId(1),
                        payload: 42,
                        reply_to: Recipient::global(MailAddr(91)),
                    },
                )
                .unwrap();
            assert_eq!(dispatched.sends.inner.responses.len(), 1);
            assert!(matches!(
                dispatched.sends.inner.responses[0].message,
                PoolResponse::Accepted { job: JobId(1) }
            ));
            let assignment = dispatched
                .sends
                .inner
                .assignments
                .into_iter()
                .next()
                .unwrap();

            let stopped_actions = proxy
                .on_path(ChildStopped::new(
                    0,
                    Err(Crash::Failed),
                    std::time::Instant::now(),
                ))
                .unwrap();
            let stopped: behavior::WorkerStopped<MailAddr> = (
                7_u64,
                stopped_actions
                    .sends
                    .stopped_reports
                    .into_iter()
                    .next()
                    .unwrap(),
            )
                .into();
            let returned_actions = proxy
                .transition(ProxyEvent::Command(User::new(
                    MailAddr(9),
                    assignment.message,
                )))
                .expect("the proxy returns rather than fails an unavailable assignment");
            assert_eq!(returned_actions.sends.unavailable.len(), 1);
            let returned = returned_actions
                .sends
                .unavailable
                .into_iter()
                .next()
                .unwrap()
                .message;

            let stop_fold;
            let return_fold;
            if command_returned_first {
                return_fold = pool.on_path(User::new(MailAddr(7), returned)).unwrap();
                assert!(return_fold.sends.inner.responses.is_empty());
                assert!(return_fold.sends.inner.assignments.is_empty());
                stop_fold = pool.on_path(stopped).unwrap();
            } else {
                stop_fold = pool.on_path(stopped).unwrap();
                if replacement_expected {
                    let ready = pool
                        .on_path(WorkerCreationResolved::new(
                            7,
                            1,
                            CreationKind::replacement_of(0),
                            Ok(()),
                        ))
                        .unwrap();
                    assert!(ready.sends.inner.assignments.is_empty());
                    assert_eq!(pool.worker_phase(7), Some(behavior::WorkerPhase::Idle));
                }
                return_fold = pool.on_path(User::new(MailAddr(7), returned)).unwrap();
            }

            assert_eq!(
                stop_fold.sends.owned.replacement_commands.len(),
                usize::from(replacement_expected)
            );
            assert_eq!(stop_fold.sends.inner.responses.len(), 1);
            assert!(matches!(
                stop_fold.sends.inner.responses[0].message,
                PoolResponse::Interrupted {
                    job: JobId(1),
                    payload: 42,
                    reason: behavior::PoolInterruption::WorkerStopped {
                        worker: 7,
                        outcome: Err(Crash::Failed),
                    },
                }
            ));
            assert!(return_fold.sends.inner.responses.is_empty());
            assert!(return_fold.sends.inner.assignments.is_empty());

            if command_returned_first && replacement_expected {
                let ready = pool
                    .on_path(WorkerCreationResolved::new(
                        7,
                        1,
                        CreationKind::replacement_of(0),
                        Ok(()),
                    ))
                    .unwrap();
                assert!(ready.sends.inner.assignments.is_empty());
            }
            assert_eq!(
                pool.worker_phase(7),
                Some(if replacement_expected {
                    behavior::WorkerPhase::Idle
                } else {
                    behavior::WorkerPhase::Retired {
                        reason: behavior::WorkerRetirement::ReplacementUnavailable,
                    }
                })
            );
        }
    }
}
