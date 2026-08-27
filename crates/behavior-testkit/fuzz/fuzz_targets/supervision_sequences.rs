#![no_main]

//! Supervision attack surface: arbitrary child-stopped byte sequences drive
//! a OneForOne/Permanent supervisor with a small restart budget and a small
//! window, while an inline reference model (documented semantics: lazy
//! window pruning with inclusive edge + future-stamp survival, budget counts
//! every replacement, denial kills the dead slot) tracks the expected
//! replacement sends, alive flags, and restart-stamp count. The impl must
//! agree on every byte.

use behavior::{
    Acted, Actions, Activate, Address, Backoff, Behavior, BehaviorActed, Crash, CreationKind,
    CreationRejection, Delivery, DynamicSupervisor, DynamicSupervisorMessage,
    DynamicSupervisorOutcome, EndpointAddress, EstablishedCreation, EstablishedRecipient,
    InterpretEstablished, MailAddr, Never, NoBirths, Protocol, Recipient, RestartDenial,
    RestartPolicy, ShutdownRequested, Step, Strategy, SuperviseError, SupervisionEvent,
    SupervisionFailureReason, Supervisor, TimerElapsed, TimerGeneration, TimerId, User,
    WorkerCreationResolved, WorkerStopped,
};
use core::marker::PhantomData;
use libfuzzer_sys::fuzz_target;
use std::time::Instant;
use tokio::runtime::Builder;

const FLEET: usize = 4;
const BUDGET: u32 = 2;
const WINDOW_NANOS: u64 = 100;

struct Worker;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Worker {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }
}

fn worker(_index: usize) -> Worker {
    Worker
}

fn stop_on_failure(_: &behavior::SupervisionFailure<MailAddr>) -> behavior::Become {
    Step::Stop(behavior::Stopped)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DynamicAddr(u64);

impl Address for DynamicAddr {
    type Nonce = u64;
}

struct DynamicEndpoint<P> {
    id: u64,
    protocol: PhantomData<fn() -> P>,
}

impl<P> DynamicEndpoint<P> {
    const fn new(id: u64) -> Self {
        Self {
            id,
            protocol: PhantomData,
        }
    }
}

impl<P> Clone for DynamicEndpoint<P> {
    fn clone(&self) -> Self {
        Self::new(self.id)
    }
}

impl EndpointAddress for DynamicAddr {
    type Established<P>
        = DynamicEndpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct DynamicEndpointId;

impl<P> InterpretEstablished<P> for DynamicEndpointId
where
    P: Protocol<Addr = DynamicAddr>,
{
    type Output = u64;

    fn interpret_established(&mut self, endpoint: DynamicEndpoint<P>) -> Self::Output {
        endpoint.id
    }
}

struct DynamicWorker;

impl Protocol for DynamicWorker {
    type Addr = DynamicAddr;
    type Msg = u8;
}

impl Behavior for DynamicWorker {
    type Protocol = Self;
    type Event = User<DynamicAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct DynamicReply;

impl Protocol for DynamicReply {
    type Addr = DynamicAddr;
    type Msg = DynamicSupervisorOutcome<DynamicAddr, DynamicWorker>;
}

fn installed_dynamic_proxy() -> EstablishedCreation<DynamicWorker, behavior::ChildHead> {
    EstablishedCreation::installed(
        7,
        CreationKind::Birth,
        EstablishedRecipient::issued(DynamicEndpoint::new(70)),
    )
}

macro_rules! assert_quiet_supervisor_lanes {
    ($actions:expr) => {{
        let actions = &$actions;
        assert!(actions.sends.child_observations.is_empty());
        assert!(actions.sends.creation_observations.is_empty());
        assert!(actions.sends.schedules.is_empty());
        assert!(actions.sends.shutdowns.is_empty());
        assert!(actions.creates.is_empty());
    }};
}

fn fuzz_dynamic_initial_join(bytes: &[u8]) {
    for byte in bytes.iter().copied() {
        let worker_first = byte & 1 != 0;
        let worker_rejected = byte & 2 != 0;
        let shutdown_point = (byte >> 2) & 3;
        let initialized = DynamicSupervisor::<
            DynamicAddr,
            DynamicWorker,
            Recipient<DynamicReply>,
            _,
        >::new(behavior::Proxy::new)
        .initialize()
        .unwrap();
        assert!(initialized.actions.sends.outcomes.is_empty());
        assert!(initialized.actions.sends.child_observations.is_empty());
        assert!(initialized.actions.sends.creation_observations.is_empty());
        assert!(initialized.actions.sends.shutdowns.is_empty());
        assert!(initialized.actions.sends.replacement_inputs.is_empty());
        assert!(initialized.actions.creates.is_empty());
        assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut subject = initialized.behavior;
        let started = subject
            .receive(
                DynamicAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: DynamicWorker,
                    reply_to: Recipient::global(DynamicAddr(99)),
                },
            )
            .unwrap();
        assert!(matches!(
            started.sends.outcomes.as_slice(),
            [Delivery {
                message: DynamicSupervisorOutcome::StartAccepted { nonce: 7 },
                ..
            }]
        ));
        assert_eq!(started.sends.child_observations.len(), 1);
        assert_eq!(started.sends.creation_observations.len(), 1);
        assert!(started.sends.shutdowns.is_empty());
        assert!(started.sends.replacement_inputs.is_empty());
        assert_eq!(started.creates.len(), 1);
        assert!(matches!(started.become_, Step::Continue));

        let worker = WorkerCreationResolved::new(
            7,
            0,
            CreationKind::Birth,
            if worker_rejected {
                Err(CreationRejection::EnvironmentFailed)
            } else {
                Ok(())
            },
        );
        let mut outcomes = 0;
        let mut shutdowns = 0;

        if shutdown_point == 0 {
            let actions = subject.on_path(ShutdownRequested).unwrap();
            outcomes += actions.sends.outcomes.len();
            shutdowns += actions.sends.shutdowns.len();
            assert!(actions.sends.child_observations.is_empty());
            assert!(actions.sends.creation_observations.is_empty());
            assert!(actions.sends.replacement_inputs.is_empty());
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
        }
        let first = if worker_first {
            subject.on_path(worker).unwrap()
        } else {
            subject.on_path(installed_dynamic_proxy()).unwrap()
        };
        outcomes += first.sends.outcomes.len();
        shutdowns += first.sends.shutdowns.len();
        assert!(first.sends.child_observations.is_empty());
        assert!(first.sends.creation_observations.is_empty());
        assert!(first.sends.replacement_inputs.is_empty());
        assert!(first.creates.is_empty());
        assert!(matches!(first.become_, Step::Continue));

        if shutdown_point == 1 {
            let actions = subject.on_path(ShutdownRequested).unwrap();
            outcomes += actions.sends.outcomes.len();
            shutdowns += actions.sends.shutdowns.len();
            assert!(actions.sends.child_observations.is_empty());
            assert!(actions.sends.creation_observations.is_empty());
            assert!(actions.sends.replacement_inputs.is_empty());
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
        }
        let second = if worker_first {
            subject.on_path(installed_dynamic_proxy()).unwrap()
        } else {
            subject.on_path(worker).unwrap()
        };
        outcomes += second.sends.outcomes.len();
        shutdowns += second.sends.shutdowns.len();
        assert!(second.sends.child_observations.is_empty());
        assert!(second.sends.creation_observations.is_empty());
        assert!(second.sends.replacement_inputs.is_empty());
        assert!(second.creates.is_empty());
        assert!(matches!(second.become_, Step::Continue));

        if shutdown_point >= 2 {
            let actions = subject.on_path(ShutdownRequested).unwrap();
            outcomes += actions.sends.outcomes.len();
            shutdowns += actions.sends.shutdowns.len();
            assert!(actions.sends.child_observations.is_empty());
            assert!(actions.sends.creation_observations.is_empty());
            assert!(actions.sends.replacement_inputs.is_empty());
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
        }

        assert_eq!(outcomes, 1);
        assert_eq!(shutdowns, 1);
        assert_eq!(second.sends.outcomes.len(), 1);
        let outcome = second
            .sends
            .outcomes
            .into_iter()
            .next()
            .expect("the completed join emits one outcome")
            .message;
        if worker_rejected {
            assert!(matches!(
                outcome,
                DynamicSupervisorOutcome::StartFailed {
                    nonce: 7,
                    reason: CreationRejection::EnvironmentFailed,
                }
            ));
        } else {
            let DynamicSupervisorOutcome::Started { nonce, child } = outcome else {
                panic!("the completed join emitted an unexpected outcome")
            };
            assert_eq!(nonce, 7);
            assert_eq!(child.interpret(&mut DynamicEndpointId), 70);
        }
    }
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        fuzz_dynamic_initial_join(bytes);
        let behavior = Supervisor::new(
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                FLEET,
                |index| Some(worker(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                BUDGET,
                std::time::Duration::from_nanos(WINDOW_NANOS), behavior::RestartTiming::Immediate
            ),
            behavior::Proxy::new,
        )
        .unwrap()
        .with_failure_reaction(stop_on_failure);
        let initialized = (behavior).initialize().unwrap();
        assert_eq!(initialized.actions.sends.child_observations.len(), FLEET);
        assert_eq!(initialized.actions.sends.creation_observations.len(), FLEET);
        assert!(initialized.actions.sends.schedules.is_empty());
        assert!(initialized.actions.sends.replacement_inputs.is_empty());
        assert!(initialized.actions.sends.failure_reports.is_empty());
        assert!(initialized.actions.sends.shutdowns.is_empty());
        assert_eq!(initialized.actions.creates.len(), FLEET);
        assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut behavior = initialized.behavior;
        let base = Instant::now();

        // Independent budget/window model over the same event stream.
        let mut restarts: Vec<u64> = Vec::new();
        let mut alive = [true; FLEET];
        let mut workers = [0_u64, 1, 2, 3];
        let mut next_worker = u64::try_from(FLEET).unwrap();

        for (index, byte) in bytes.iter().copied().enumerate() {
            let nonce = usize::from(byte) % FLEET;
            // Deliberately non-monotone, equal, and backwards timestamps:
            // (index * 37) % 200 cycles 0..200 with duplicates and drops.
            let at = u64::try_from((index * 37) % 200).unwrap();

            let was_alive = alive[nonce];
            let expected_restart = was_alive && {
                restarts.retain(|stamp| *stamp > at || at - stamp <= WINDOW_NANOS);
                if restarts.len() + 1 <= BUDGET as usize {
                    restarts.push(at);
                    alive[nonce] = true;
                    true
                } else {
                    alive[nonce] = false;
                    false
                }
            };

            let observed = WorkerStopped {
                proxy: u64::try_from(nonce).unwrap(),
                worker: workers[nonce],
                outcome: Err(Crash::Failed),
                at: base + std::time::Duration::from_nanos(at),
            };
            let result = behavior.transition(SupervisionEvent::WorkerStopped(observed.clone()));

            if !was_alive {
                assert!(matches!(
                    result,
                    Err(SuperviseError::UnexpectedWorkerStopped(returned))
                        if returned == observed
                ));
                continue;
            }
            let actions = result.unwrap();

            assert_eq!(
                actions.sends.replacement_inputs.len(),
                usize::from(expected_restart),
                "replacement count mismatch at byte {index}"
            );
            assert!(actions.sends.child_observations.is_empty());
            assert!(actions.sends.creation_observations.is_empty());
            assert!(actions.sends.schedules.is_empty());
            assert!(actions.sends.shutdowns.is_empty());
            assert!(actions.creates.is_empty());
            if expected_restart {
                assert_eq!(actions.become_, Step::Continue);
            } else if was_alive {
                assert_eq!(actions.become_, Step::Stop(behavior::Stopped));
                assert_eq!(actions.sends.failure_reports.len(), 1);
                assert_eq!(
                    actions.sends.failure_reports[0].failure.reason(),
                    SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                        restarts_in_window: restarts.len(),
                        replacements_requested: 1,
                        maximum_restarts: BUDGET,
                    })
                );
            } else {
                assert_eq!(actions.become_, Step::Continue);
                assert!(actions.sends.failure_reports.is_empty());
            }
            if expected_restart {
                let proxy = u64::try_from(nonce).unwrap();
                let previous = workers[nonce];
                let installed = behavior
                    .on_path(WorkerCreationResolved::new(
                        proxy,
                        next_worker,
                        CreationKind::ReplacementIncarnation { replaces: previous },
                        Ok(()),
                    ))
                    .unwrap();
                assert!(installed.sends.replacement_inputs.is_empty());
                assert!(installed.sends.failure_reports.is_empty());
                assert_quiet_supervisor_lanes!(installed);
                assert!(matches!(installed.become_, Step::Continue));
                workers[nonce] = next_worker;
                next_worker = next_worker.checked_add(1).unwrap();
            }
            for slot in 0..FLEET {
                assert_eq!(
                    behavior
                        .is_restartable(u64::try_from(slot).unwrap())
                        .unwrap(),
                    alive[slot],
                    "alive mismatch at byte {index}"
                );
            }
            assert_eq!(
                behavior.restarts_in_window(),
                restarts.len(),
                "restart-stamp count mismatch at byte {index}"
            );
        }

        // Independent generation/pending model for delayed fixed supervision.
        // Each input selects worker failure, exact timer, stale
        // timer, or shutdown; every step checks delayed release, cancellation,
        // and stale-timer rejection.
        let initialized = Supervisor::new(
            behavior::ChildTopology::indexed(
                |index| u64::try_from(index).unwrap(),
                FLEET,
                |index| Some(worker(index)),
            ),
            behavior::RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                u32::MAX,
                std::time::Duration::MAX,
                behavior::RestartTiming::Delayed(Backoff::exponential(
                    std::time::Duration::from_nanos(1),
                    std::time::Duration::from_nanos(8),
                )
                .unwrap()),
            ),
            behavior::Proxy::new,
        )
        .unwrap()
        .initialize()
        .unwrap();
        assert_eq!(initialized.actions.sends.child_observations.len(), FLEET);
        assert_eq!(initialized.actions.sends.creation_observations.len(), FLEET);
        assert!(initialized.actions.sends.schedules.is_empty());
        assert!(initialized.actions.sends.replacement_inputs.is_empty());
        assert!(initialized.actions.sends.failure_reports.is_empty());
        assert!(initialized.actions.sends.shutdowns.is_empty());
        assert_eq!(initialized.actions.creates.len(), FLEET);
        assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut backoff = initialized.behavior;
        let mut pending = [None::<(u64, u64)>; FLEET];
        let mut next_generation = [0_u64; FLEET];
        let mut next_timer = 0_u64;
        let mut backoff_workers = [0_u64, 1, 2, 3];
        let mut next_backoff_worker = u64::try_from(FLEET).unwrap();
        let mut shutting_down = false;
        let mut backoff_stopped = [false; FLEET];

        for (index, byte) in bytes.iter().copied().enumerate() {
            let slot = (usize::from(byte) / 4) % FLEET;
            let nonce = u64::try_from(slot).unwrap();
            match byte % 4 {
                0 => {
                    let observed = WorkerStopped::new(
                        nonce,
                        backoff_workers[slot],
                        Err(Crash::Failed),
                        base + std::time::Duration::from_nanos(u64::try_from(index).unwrap()),
                    );
                    let result = backoff.on_path(observed.clone());
                    let accepts_fact = !backoff_stopped[slot];
                    if !accepts_fact {
                        assert!(matches!(
                            result,
                            Err(SuperviseError::UnexpectedWorkerStopped(returned))
                                if returned == observed
                        ));
                        continue;
                    }
                    let actions = result.unwrap();
                    backoff_stopped[slot] = true;
                    let scheduled = !shutting_down && pending[slot].is_none();
                    assert_eq!(actions.sends.schedules.len(), usize::from(scheduled));
                    assert!(actions.sends.replacement_inputs.is_empty());
                    assert!(actions.sends.child_observations.is_empty());
                    assert!(actions.sends.creation_observations.is_empty());
                    assert!(actions.sends.failure_reports.is_empty());
                    assert!(actions.sends.shutdowns.is_empty());
                    assert!(actions.creates.is_empty());
                    assert!(matches!(actions.become_, Step::Continue));
                    if scheduled {
                        let generation = next_generation[slot];
                        let schedule = actions.sends.schedules.as_slice()[0];
                        assert_eq!(schedule.id, TimerId(next_timer));
                        assert_eq!(schedule.generation, TimerGeneration(generation));
                        pending[slot] = Some((next_timer, generation));
                        next_timer = next_timer.checked_add(1).unwrap();
                        next_generation[slot] = generation.checked_add(1).unwrap();
                    }
                }
                1 => {
                    let (timer, generation) = pending[slot].unwrap_or((u64::MAX, u64::MAX));
                    let actions = backoff
                        .on_path(TimerElapsed::new(
                            TimerId(timer),
                            TimerGeneration(generation),
                        ))
                        .unwrap();
                    let released = pending[slot].take().is_some() && !shutting_down;
                    assert_eq!(
                        actions.sends.replacement_inputs.len(),
                        usize::from(released)
                    );
                    assert!(actions.sends.schedules.is_empty());
                    assert!(actions.sends.child_observations.is_empty());
                    assert!(actions.sends.creation_observations.is_empty());
                    assert!(actions.sends.failure_reports.is_empty());
                    assert!(actions.sends.shutdowns.is_empty());
                    assert!(actions.creates.is_empty());
                    assert!(matches!(actions.become_, Step::Continue));
                    if released {
                        let previous = backoff_workers[slot];
                        let installed = backoff
                            .on_path(WorkerCreationResolved::new(
                                nonce,
                                next_backoff_worker,
                                CreationKind::ReplacementIncarnation { replaces: previous },
                                Ok(()),
                            ))
                            .unwrap();
                        assert!(installed.sends.replacement_inputs.is_empty());
                        assert!(installed.sends.failure_reports.is_empty());
                        assert_quiet_supervisor_lanes!(installed);
                        assert!(matches!(installed.become_, Step::Continue));
                        backoff_workers[slot] = next_backoff_worker;
                        backoff_stopped[slot] = false;
                        next_backoff_worker = next_backoff_worker.checked_add(1).unwrap();
                    }
                }
                2 => {
                    let actions = backoff
                        .on_path(TimerElapsed::new(
                            TimerId(u64::MAX),
                            TimerGeneration(u64::MAX),
                        ))
                        .unwrap();
                    assert!(actions.sends.replacement_inputs.is_empty());
                    assert!(actions.sends.schedules.is_empty());
                    assert!(actions.sends.child_observations.is_empty());
                    assert!(actions.sends.creation_observations.is_empty());
                    assert!(actions.sends.failure_reports.is_empty());
                    assert!(actions.sends.shutdowns.is_empty());
                    assert!(actions.creates.is_empty());
                    assert!(matches!(actions.become_, Step::Continue));
                }
                _ => {
                    let actions = backoff.on_path(ShutdownRequested).unwrap();
                    pending.fill(None);
                    shutting_down = true;
                    assert!(actions.sends.schedules.is_empty());
                    assert!(actions.sends.child_observations.is_empty());
                    assert!(actions.sends.creation_observations.is_empty());
                    assert!(actions.sends.replacement_inputs.is_empty());
                    assert!(actions.sends.failure_reports.is_empty());
                    assert!(actions.sends.shutdowns.len() <= FLEET);
                    assert!(actions.creates.is_empty());
                    assert!(matches!(actions.become_, Step::Continue));
                }
            }
            assert_eq!(
                backoff.pending_restarts(),
                pending
                    .iter()
                    .filter(|generation| generation.is_some())
                    .count()
            );
        }
    });
});
