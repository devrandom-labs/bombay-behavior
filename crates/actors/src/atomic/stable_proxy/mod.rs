//! One stable service identity backed by at most one exact worker.

use core::mem;
use core::ops::ControlFlow;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, BirthMode,
    Births, ChildCreationProduct, ChildHead, CreateChild, CreationSequence, Creations,
    CreationsSettled, EndpointAddress, EstablishedDelivery, EstablishedRecipient, Here,
    InjectEvent, InterpreterRequests, Never, Protocol, ReportToParent, SendEffects,
    SendSettlements, User,
};

use crate::{
    ChildStopped, EstablishedShutdownResolved, ObserveChild, ShutdownEstablished,
    ShutdownRequested, StopOnShutdown,
};

mod effects;
mod operation;
mod protocol;
mod state;
mod worker;

use super::worker::ActivationAttempt;
pub use super::worker::WorkerAttempt;
pub use super::worker::{
    ActivationPermit, InitializationAttempt, InitializeWorker, WorkerInitializationFailure,
    WorkerInitializationReport,
};
pub use super::worker::{
    ActivationPlan, ActivationStartRejection, BeginActivation, ImmediateActivation,
    WorkerActivation,
};
pub use effects::ProxyEffects;
pub(crate) use operation::ProxyOperationWitness;
pub use operation::{ProxyInputReceipt, ProxyInputResult, ProxyOperation, ProxyOperationId};
pub use protocol::{
    InitialWorkerOutcome, ProxyControl, ProxyDiagnostic, ProxyDrain, ProxyOutcome, ProxyPhase,
    ReplacementOutcome,
};
use protocol::{ProxyCommand, ProxyEvent};
use state::{
    ActivationDuringDeparture, ActivationProgress, PreReadyFailure, PredecessorReturn,
    PredecessorShutdown, ProxyReplacement, ProxyRetirement, ProxyShutdown, ProxyState,
    ReplacementCompletion, WorkerActivationRetirement, WorkerActivationShutdown,
    WorkerInitializationRetirement, WorkerInitializationShutdown, WorkerStart, WorkerStartKind,
    WorkerStartPhase, WorkerStartRetirement,
};
pub use worker::WorkerStartResult;
use worker::{CurrentWorker, PendingWorker, StoppedWorker, WorkerCreation, WorkerStopping};

/// One stable service identity backed by zero or one exact current worker.
pub struct StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    state: ProxyState<W, P>,
    creations: CreationSequence,
}

impl<W> StableProxy<W, ImmediateActivation>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    /// Construct an empty proxy for workers needing no activation work.
    #[must_use]
    pub const fn immediate() -> Self {
        Self::dormant()
    }
}

impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    const fn dormant() -> Self {
        Self {
            state: ProxyState::Dormant,
            creations: CreationSequence::new(),
        }
    }

    /// Construct an empty proxy whose worker start supplies a concrete plan.
    #[must_use]
    pub const fn activated() -> Self {
        Self::dormant()
    }

    /// Project the current lifecycle phase without exposing worker authority.
    #[must_use]
    pub const fn phase(&self) -> ProxyPhase {
        self.state.phase()
    }
}

#[cfg(test)]
mod shutdown_ownership_tests {
    use std::time::{Duration, Instant};

    use core::ops::ControlFlow;

    use behavior::{
        Actions, ActiveTurn, Address, Behavior, BehaviorActed, CreationSequence, EndpointAddress,
        EstablishedActor, Never, NoBirths, Protocol, User,
    };

    use crate::{ChildStopped, Exit, StopOnShutdown};

    use super::{
        CurrentWorker, ImmediateActivation, InitializationAttempt, StableProxy, WorkerAttempt,
        WorkerInitializationRetirement, WorkerInitializationShutdown, WorkerStopping,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct SearchAddress;

    impl Address for SearchAddress {
        type Nonce = u64;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct SearchEndpoint;

    impl EndpointAddress for SearchAddress {
        type Established<P>
            = SearchEndpoint
        where
            P: Protocol<Addr = Self>;
    }

    struct SearchWorker;

    impl Protocol for SearchWorker {
        type Addr = SearchAddress;
        type Msg = ();
    }

    impl Behavior for SearchWorker {
        type Protocol = Self;
        type Event = User<SearchAddress, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[test]
    fn initialization_and_proxy_stops_remain_separately_owned_during_shutdown() {
        let mut creations = CreationSequence::new();
        let creation = creations
            .issue()
            .unwrap_or_else(|| panic!("the first worker creation is available"));
        let worker = WorkerAttempt::issued(creation);
        let initialization = InitializationAttempt::issued(&worker);
        let current = CurrentWorker {
            attempt: worker.clone(),
            initialization: initialization.clone(),
            actor: EstablishedActor::<StopOnShutdown<SearchWorker>>::issued(SearchEndpoint),
        };
        let (departing, _shutdown) = WorkerStopping::begin(current);
        let proxy_stop = ChildStopped::new(creation, Ok(Exit::Normal), Instant::now());
        let initialization_stop = ChildStopped::new(
            creation,
            Ok(Exit::Normal),
            proxy_stop.at + Duration::from_nanos(1),
        );
        let departing = match departing.worker_stopped(proxy_stop) {
            Ok(ControlFlow::Continue(departing)) => departing,
            Ok(ControlFlow::Break(_)) => panic!("shutdown has not settled"),
            Err(_) => panic!("the proxy stop belongs to the current worker"),
        };

        let retained = StableProxy::<SearchWorker, ImmediateActivation>::retain_initialization_stop(
            departing,
            worker,
            initialization,
            ImmediateActivation,
            initialization_stop,
        );
        let shutdown = match retained {
            Ok(ControlFlow::Continue(shutdown)) => shutdown,
            Ok(ControlFlow::Break(_)) => panic!("shutdown still awaits its settlement"),
            Err(_) => panic!("the initialization stop belongs to the current worker"),
        };
        let WorkerInitializationShutdown::Departing {
            initialization:
                Some(WorkerInitializationRetirement::Stopped {
                    initialization_stop: Some(retained_stop),
                    ..
                }),
            departure,
        } = shutdown
        else {
            panic!("both stop values remain in the current shutdown state");
        };

        assert_eq!(departure.stopped(), Some(&proxy_stop));
        assert_eq!(retained_stop, initialization_stop);
    }

    #[test]
    fn initialization_stop_occupies_the_worker_stop_slot_once() {
        let mut creations = CreationSequence::new();
        let creation = creations
            .issue()
            .unwrap_or_else(|| panic!("the first worker creation is available"));
        let worker = WorkerAttempt::issued(creation);
        let initialization = InitializationAttempt::issued(&worker);
        let current = CurrentWorker {
            attempt: worker.clone(),
            initialization: initialization.clone(),
            actor: EstablishedActor::<StopOnShutdown<SearchWorker>>::issued(SearchEndpoint),
        };
        let (departing, _shutdown) = WorkerStopping::begin(current);
        let initialization_stop = ChildStopped::new(creation, Ok(Exit::Normal), Instant::now());

        let retained = StableProxy::<SearchWorker, ImmediateActivation>::retain_initialization_stop(
            departing,
            worker,
            initialization,
            ImmediateActivation,
            initialization_stop,
        );
        let shutdown = match retained {
            Ok(ControlFlow::Continue(shutdown)) => shutdown,
            Ok(ControlFlow::Break(_)) => panic!("shutdown still awaits its settlement"),
            Err(_) => panic!("the initialization stop belongs to the current worker"),
        };
        let WorkerInitializationShutdown::Departing {
            initialization:
                Some(WorkerInitializationRetirement::Stopped {
                    initialization_stop: None,
                    ..
                }),
            departure,
        } = shutdown
        else {
            panic!("the initialization stop occupies one ownership slot");
        };

        assert_eq!(departure.stopped(), Some(&initialization_stop));
    }
}

impl<W, P> BehaviorBase for StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    fn accept(
        current: StableProxy<W, P>,
        event: ProxyEvent<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match event {
            ProxyEvent::Service(service) => Self::service(current, service),
            ProxyEvent::Owner(control) => Self::owner(current, control),
            ProxyEvent::WorkerCreationsSettled(workers) => Self::workers_created(current, workers),
            ProxyEvent::WorkerInitialization(initialization) => {
                Self::worker_initialized(current, initialization)
            }
            ProxyEvent::WorkerActivationReported(activation) => {
                Self::worker_activated(current, activation)
            }
            ProxyEvent::WorkerShutdownResolved(shutdown) => {
                Self::worker_shutdown_resolved(current, shutdown)
            }
            ProxyEvent::WorkerStopped(stopped) => Self::worker_stopped(current, stopped),
        }
    }

    fn admit_activation(
        progress: &ActivationProgress,
        worker: &CurrentWorker<W>,
        input: WorkerActivation<W, P>,
    ) -> Result<WorkerActivation<W, P>, WorkerActivation<W, P>> {
        let expected = match progress {
            ActivationProgress::WaitingForStart(attempt) | ActivationProgress::Running(attempt) => {
                attempt
            }
        };
        match (input.worker(), input.attempt()) {
            (received_worker, received_activation)
                if received_worker == worker.attempt && received_activation == expected =>
            {
                Ok(input)
            }
            _ => Err(input),
        }
    }

    fn service(
        current: StableProxy<W, P>,
        service: User<BehaviorAddr<W>, <W::Protocol as Protocol>::Msg>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Ready { worker } => {
                let delivery = EstablishedDelivery::new(worker.actor.recipient(), service.message);
                (
                    StableProxy {
                        state: ProxyState::Ready { worker },
                        creations: current.creations,
                    },
                    Actions::send(ProxyEffects {
                        worker_observations: InterpreterRequests::empty(),
                        worker_initializations: InterpreterRequests::empty(),
                        worker_activations: InterpreterRequests::empty(),
                        worker_shutdowns: InterpreterRequests::empty(),
                        worker_deliveries: vec![delivery],
                        owner_outcomes: InterpreterRequests::empty(),
                        diagnostics: InterpreterRequests::empty(),
                    }),
                )
            }
            state => {
                let outcome = ProxyOutcome::Unavailable {
                    sender: service.from,
                    phase: state.phase(),
                    command: service.message,
                };
                (
                    StableProxy {
                        state,
                        creations: current.creations,
                    },
                    Self::report(outcome),
                )
            }
        }
    }

    fn owner(
        current: StableProxy<W, P>,
        control: ProxyControl<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match control.command {
            ProxyCommand::Start(submission) => {
                Self::start_initial(current, submission.worker, submission.activation)
            }
            ProxyCommand::Replace(submission) => {
                Self::replace_worker(current, submission.worker, submission.activation)
            }
            ProxyCommand::Shutdown => Self::owner_shutdown(current),
        }
    }

    fn start_initial(
        current: StableProxy<W, P>,
        worker: W,
        activation: P,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Dormant => Self::begin_initial(current.creations, worker, activation),
            state => {
                let phase = state.phase();
                let outcome = ProxyOutcome::Initial {
                    outcome: InitialWorkerOutcome::Overlap {
                        worker,
                        activation,
                        phase,
                    },
                };
                (
                    StableProxy {
                        state,
                        creations: current.creations,
                    },
                    Self::report(outcome),
                )
            }
        }
    }

    fn begin_initial(
        mut creations: CreationSequence,
        worker: W,
        activation: P,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match PendingWorker::birth(&mut creations, worker, activation) {
            Ok((pending_worker, creation)) => {
                let observation = ObserveChild::new(pending_worker.creation());
                (
                    StableProxy {
                        state: ProxyState::Starting(WorkerStart {
                            kind: WorkerStartKind::Initial,
                            phase: WorkerStartPhase::Creating {
                                worker: pending_worker,
                                stopped: None,
                            },
                        }),
                        creations,
                    },
                    Actions::new(
                        ProxyEffects {
                            worker_observations: InterpreterRequests::one(observation),
                            worker_initializations: InterpreterRequests::empty(),
                            worker_activations: InterpreterRequests::empty(),
                            worker_shutdowns: InterpreterRequests::empty(),
                            worker_deliveries: Vec::new(),
                            owner_outcomes: InterpreterRequests::empty(),
                            diagnostics: InterpreterRequests::empty(),
                        },
                        Creations::one(creation),
                        behavior::Step::Continue,
                    ),
                )
            }
            Err((worker, activation)) => (
                StableProxy {
                    state: ProxyState::Dormant,
                    creations,
                },
                Self::report(ProxyOutcome::Initial {
                    outcome: InitialWorkerOutcome::WorkerAttemptsExhausted { worker, activation },
                }),
            ),
        }
    }

    fn workers_created(
        current: StableProxy<W, P>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Starting(WorkerStart {
                kind,
                phase: WorkerStartPhase::Creating { worker, stopped },
            }) => match worker.created(workers, stopped) {
                WorkerCreation::Initializing {
                    worker,
                    activation,
                    stopped,
                } => {
                    Self::await_initialization(current.creations, kind, worker, activation, stopped)
                }
                WorkerCreation::Rejected {
                    rejection,
                    activation,
                    stopped,
                } => Self::worker_creation_rejected(
                    current.creations,
                    kind,
                    WorkerStartResult::CreationRejected {
                        rejection,
                        activation,
                        stopped,
                    },
                ),
                WorkerCreation::Unexpected {
                    worker,
                    stopped,
                    workers,
                } => {
                    let phase = ProxyPhase::Creating;
                    (
                        StableProxy {
                            state: ProxyState::Starting(WorkerStart {
                                kind,
                                phase: WorkerStartPhase::Creating { worker, stopped },
                            }),
                            creations: current.creations,
                        },
                        Self::diagnose(ProxyDiagnostic::UnexpectedWorkerStart { phase, workers }),
                    )
                }
            },
            ProxyState::ShuttingDown(shutdown) => {
                Self::shutdown_workers_created(current.creations, shutdown, workers)
            }
            state => {
                let phase = state.phase();
                let diagnostic = ProxyDiagnostic::UnexpectedWorkerStart { phase, workers };
                (
                    StableProxy {
                        state,
                        creations: current.creations,
                    },
                    Self::diagnose(diagnostic),
                )
            }
        }
    }

    fn worker_stopped(
        current: StableProxy<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Starting(start) => {
                Self::worker_start_stopped(current.creations, start, stopped)
            }
            ProxyState::Replacing(replacement) => {
                Self::replacement_worker_stopped(current.creations, replacement, stopped)
            }
            ProxyState::ShuttingDown(shutdown) => {
                Self::shutdown_worker_stopped(current.creations, shutdown, stopped)
            }
            ProxyState::Ready { worker } => match worker.admit_stop(stopped) {
                Ok(stopped) => (
                    StableProxy {
                        state: ProxyState::EmptyAfter {
                            previous: worker.attempt.clone(),
                        },
                        creations: current.creations,
                    },
                    Self::report(ProxyOutcome::WorkerStopped {
                        worker: worker.attempt,
                        stopped,
                    }),
                ),
                Err(stopped) => {
                    Self::unexpected_stop(current.creations, ProxyState::Ready { worker }, stopped)
                }
            },
            state => {
                let phase = state.phase();
                (
                    StableProxy {
                        state,
                        creations: current.creations,
                    },
                    Self::diagnose(ProxyDiagnostic::UnexpectedWorkerStop { phase, stopped }),
                )
            }
        }
    }

    fn worker_start_stopped(
        creations: CreationSequence,
        start: WorkerStart<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let WorkerStart { kind, phase } = start;
        match phase {
            WorkerStartPhase::ReturningWorker { departure, failure } => {
                match departure.worker_stopped(stopped) {
                    Ok(ControlFlow::Continue(departure)) => (
                        StableProxy {
                            state: ProxyState::Starting(WorkerStart {
                                kind,
                                phase: WorkerStartPhase::ReturningWorker { departure, failure },
                            }),
                            creations,
                        },
                        Actions::cont(),
                    ),
                    Ok(ControlFlow::Break(StoppedWorker {
                        worker,
                        shutdown,
                        stopped,
                    })) => {
                        Self::worker_returned(creations, kind, worker, failure, shutdown, stopped)
                    }
                    Err((departure, input)) => Self::unexpected_stop(
                        creations,
                        ProxyState::Starting(WorkerStart {
                            kind,
                            phase: WorkerStartPhase::ReturningWorker { departure, failure },
                        }),
                        input,
                    ),
                }
            }
            phase => match Self::admit_pre_ready_stop(phase, stopped) {
                Ok(phase) => (
                    StableProxy {
                        state: ProxyState::Starting(WorkerStart { kind, phase }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Err((phase, stopped)) => Self::unexpected_stop(
                    creations,
                    ProxyState::Starting(WorkerStart { kind, phase }),
                    stopped,
                ),
            },
        }
    }

    fn admit_pre_ready_stop(
        phase: WorkerStartPhase<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<WorkerStartPhase<W, P>, (WorkerStartPhase<W, P>, ChildStopped<BehaviorAddr<W>>)>
    {
        match phase {
            WorkerStartPhase::Creating {
                worker,
                stopped: None,
            } => match worker.admit_stop(stopped) {
                Ok(stopped) => Ok(WorkerStartPhase::Creating {
                    worker,
                    stopped: Some(stopped),
                }),
                Err(stopped) => Err((
                    WorkerStartPhase::Creating {
                        worker,
                        stopped: None,
                    },
                    stopped,
                )),
            },
            WorkerStartPhase::Initializing {
                worker,
                stopped: None,
            } => match worker.admit_stop(stopped) {
                Ok(stopped) => Ok(WorkerStartPhase::Initializing {
                    worker,
                    stopped: Some(stopped),
                }),
                Err(stopped) => Err((
                    WorkerStartPhase::Initializing {
                        worker,
                        stopped: None,
                    },
                    stopped,
                )),
            },
            WorkerStartPhase::Activating {
                worker,
                progress,
                stopped: None,
            } => match worker.admit_stop(stopped) {
                Ok(stopped) => Ok(WorkerStartPhase::Activating {
                    worker,
                    progress,
                    stopped: Some(stopped),
                }),
                Err(stopped) => Err((
                    WorkerStartPhase::Activating {
                        worker,
                        progress,
                        stopped: None,
                    },
                    stopped,
                )),
            },
            phase => Err((phase, stopped)),
        }
    }

    fn unexpected_stop(
        creations: CreationSequence,
        state: ProxyState<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let phase = state.phase();
        (
            StableProxy { state, creations },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerStop { phase, stopped }),
        )
    }

    fn await_initialization(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let request = InitializeWorker::<W, P>::new(
            worker.attempt.clone(),
            worker.initialization.clone(),
            worker.actor.recipient(),
            activation,
        );
        (
            StableProxy {
                state: ProxyState::Starting(WorkerStart {
                    kind,
                    phase: WorkerStartPhase::Initializing { worker, stopped },
                }),
                creations,
            },
            Actions::send(ProxyEffects {
                worker_observations: InterpreterRequests::empty(),
                worker_initializations: InterpreterRequests::one(request),
                worker_activations: InterpreterRequests::empty(),
                worker_shutdowns: InterpreterRequests::empty(),
                worker_deliveries: Vec::new(),
                owner_outcomes: InterpreterRequests::empty(),
                diagnostics: InterpreterRequests::empty(),
            }),
        )
    }

    fn worker_creation_rejected(
        creations: CreationSequence,
        kind: WorkerStartKind,
        result: WorkerStartResult<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match kind {
            WorkerStartKind::Initial => (
                StableProxy {
                    state: ProxyState::EmptyInitial,
                    creations,
                },
                Self::report(ProxyOutcome::Initial {
                    outcome: InitialWorkerOutcome::Resolved { result },
                }),
            ),
            WorkerStartKind::Replacement {
                replaces,
                predecessor_shutdown,
            } => Self::finish_replacement(
                creations,
                replaces.clone(),
                predecessor_shutdown,
                ReplacementCompletion::Empty {
                    worker: replaces,
                    result,
                },
            ),
        }
    }

    fn worker_start_ready(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        result: WorkerStartResult<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match kind {
            WorkerStartKind::Initial => (
                StableProxy {
                    state: ProxyState::Ready { worker },
                    creations,
                },
                Self::report(ProxyOutcome::Initial {
                    outcome: InitialWorkerOutcome::Resolved { result },
                }),
            ),
            WorkerStartKind::Replacement {
                replaces,
                predecessor_shutdown,
            } => Self::finish_replacement(
                creations,
                replaces,
                predecessor_shutdown,
                ReplacementCompletion::Ready { worker, result },
            ),
        }
    }

    fn worker_start_empty(
        creations: CreationSequence,
        kind: WorkerStartKind,
        previous: WorkerAttempt,
        result: WorkerStartResult<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match kind {
            WorkerStartKind::Initial => (
                StableProxy {
                    state: ProxyState::EmptyAfter { previous },
                    creations,
                },
                Self::report(ProxyOutcome::Initial {
                    outcome: InitialWorkerOutcome::Resolved { result },
                }),
            ),
            WorkerStartKind::Replacement {
                replaces,
                predecessor_shutdown,
            } => Self::finish_replacement(
                creations,
                replaces,
                predecessor_shutdown,
                ReplacementCompletion::Empty {
                    worker: previous,
                    result,
                },
            ),
        }
    }

    fn create_worker(
        observation: ObserveChild<W::Protocol, ChildHead>,
        creation: CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
    ) -> ProxyActions<W, P> {
        Actions::new(
            ProxyEffects {
                worker_observations: InterpreterRequests::one(observation),
                worker_initializations: InterpreterRequests::empty(),
                worker_activations: InterpreterRequests::empty(),
                worker_shutdowns: InterpreterRequests::empty(),
                worker_deliveries: Vec::new(),
                owner_outcomes: InterpreterRequests::empty(),
                diagnostics: InterpreterRequests::empty(),
            },
            Creations::one(creation),
            behavior::Step::Continue,
        )
    }

    fn report(outcome: ProxyOutcome<W, P>) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::empty(),
            worker_activations: InterpreterRequests::empty(),
            worker_shutdowns: InterpreterRequests::empty(),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::one(ReportToParent::new(outcome)),
            diagnostics: InterpreterRequests::empty(),
        })
    }

    fn diagnose(diagnostic: ProxyDiagnostic<W, P>) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::empty(),
            worker_activations: InterpreterRequests::empty(),
            worker_shutdowns: InterpreterRequests::empty(),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::empty(),
            diagnostics: InterpreterRequests::one(ReportToParent::new(diagnostic)),
        })
    }
}

impl<W, P> Behavior for StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq + Send,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    type Protocol = W::Protocol;
    type Event = ProxyEvent<W, P>;
    type Sends = ProxyEffects<
        InterpreterRequests<ObserveChild<W::Protocol, ChildHead>>,
        InterpreterRequests<InitializeWorker<W, P>>,
        InterpreterRequests<BeginActivation<W, P>>,
        InterpreterRequests<ShutdownEstablished<StopOnShutdown<W>, Here>>,
        Vec<EstablishedDelivery<W::Protocol>>,
        InterpreterRequests<ReportToParent<ProxyOutcome<W, P>>>,
        InterpreterRequests<ReportToParent<ProxyDiagnostic<W, P>>>,
    >;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<StopOnShutdown<W>>;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let current = mem::replace(self, Self::dormant());
        let (next, actions) = Self::accept(current, event);
        *self = next;
        Ok(actions)
    }
}

type ProxyActions<W, P> = Actions<
    BehaviorAddr<W>,
    Never,
    ProxyEffects<
        InterpreterRequests<ObserveChild<<W as Behavior>::Protocol, ChildHead>>,
        InterpreterRequests<InitializeWorker<W, P>>,
        InterpreterRequests<BeginActivation<W, P>>,
        InterpreterRequests<ShutdownEstablished<StopOnShutdown<W>, Here>>,
        Vec<EstablishedDelivery<<W as Behavior>::Protocol>>,
        InterpreterRequests<ReportToParent<ProxyOutcome<W, P>>>,
        InterpreterRequests<ReportToParent<ProxyDiagnostic<W, P>>>,
    >,
    Births<StopOnShutdown<W>>,
>;

// Worker replacement transitions.
impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    fn replace_worker(
        current: StableProxy<W, P>,
        worker: W,
        activation: P,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Ready {
                worker: current_worker,
            } => {
                let replaces = current_worker.attempt.clone();
                let mut creations = current.creations;
                match PendingWorker::replacement(
                    &mut creations,
                    replaces.creation(),
                    worker,
                    activation,
                ) {
                    Ok((successor, creation)) => {
                        let (departure, shutdown) = WorkerStopping::begin(current_worker);
                        (
                            StableProxy {
                                state: ProxyState::Replacing(
                                    ProxyReplacement::ReturningPredecessor {
                                        departure,
                                        successor,
                                        creation,
                                    },
                                ),
                                creations,
                            },
                            Self::request_worker_shutdown(shutdown),
                        )
                    }
                    Err((worker, activation)) => (
                        StableProxy {
                            state: ProxyState::Ready {
                                worker: current_worker,
                            },
                            creations,
                        },
                        Self::report(ProxyOutcome::Replacement {
                            outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                                replaces,
                                worker,
                                activation,
                            },
                        }),
                    ),
                }
            }
            ProxyState::EmptyAfter { previous } => {
                let mut creations = current.creations;
                match PendingWorker::replacement(
                    &mut creations,
                    previous.creation(),
                    worker,
                    activation,
                ) {
                    Ok((pending_worker, creation)) => {
                        let observation = ObserveChild::new(pending_worker.creation());
                        (
                            StableProxy {
                                state: ProxyState::Starting(WorkerStart {
                                    kind: WorkerStartKind::Replacement {
                                        replaces: previous,
                                        predecessor_shutdown: PredecessorShutdown::Settled,
                                    },
                                    phase: WorkerStartPhase::Creating {
                                        worker: pending_worker,
                                        stopped: None,
                                    },
                                }),
                                creations,
                            },
                            Self::create_worker(observation, creation),
                        )
                    }
                    Err((worker, activation)) => (
                        StableProxy {
                            state: ProxyState::EmptyAfter {
                                previous: previous.clone(),
                            },
                            creations,
                        },
                        Self::report(ProxyOutcome::Replacement {
                            outcome: ReplacementOutcome::WorkerAttemptsExhausted {
                                replaces: previous,
                                worker,
                                activation,
                            },
                        }),
                    ),
                }
            }
            state => {
                let phase = state.phase();
                (
                    StableProxy {
                        state,
                        creations: current.creations,
                    },
                    Self::report(ProxyOutcome::Replacement {
                        outcome: ReplacementOutcome::NotReplaceable {
                            worker,
                            activation,
                            phase,
                        },
                    }),
                )
            }
        }
    }

    fn replacement_worker_stopped(
        creations: CreationSequence,
        replacement: ProxyReplacement<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match replacement {
            ProxyReplacement::ReturningPredecessor {
                departure,
                successor,
                creation,
            } => match departure.worker_stopped(stopped) {
                Ok(ControlFlow::Continue(departure)) => {
                    match departure.into_stopped_before_shutdown() {
                        Ok((worker, shutdown, stopped)) => Self::start_successor_after_stop(
                            creations,
                            worker.attempt,
                            PredecessorShutdown::Awaiting { id: shutdown },
                            successor,
                            creation,
                            stopped,
                        ),
                        Err(departure) => (
                            StableProxy {
                                state: ProxyState::Replacing(
                                    ProxyReplacement::ReturningPredecessor {
                                        departure,
                                        successor,
                                        creation,
                                    },
                                ),
                                creations,
                            },
                            Actions::cont(),
                        ),
                    }
                }
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown: _,
                    stopped,
                })) => Self::start_successor_after_stop(
                    creations,
                    worker.attempt,
                    PredecessorShutdown::Settled,
                    successor,
                    creation,
                    stopped,
                ),
                Err((departure, input)) => Self::unexpected_stop(
                    creations,
                    ProxyState::Replacing(ProxyReplacement::ReturningPredecessor {
                        departure,
                        successor,
                        creation,
                    }),
                    input,
                ),
            },
            ProxyReplacement::SuccessorResultAwaitingShutdown {
                replaces,
                shutdown,
                completion: ReplacementCompletion::Ready { worker, result },
            } => match worker.admit_stop(stopped) {
                Ok(stopped) => (
                    StableProxy {
                        state: ProxyState::Replacing(
                            ProxyReplacement::SuccessorResultAwaitingShutdown {
                                replaces,
                                shutdown,
                                completion: ReplacementCompletion::ReadyAfterStop {
                                    worker: worker.attempt,
                                    result,
                                    stopped,
                                },
                            },
                        ),
                        creations,
                    },
                    Actions::cont(),
                ),
                Err(stopped) => Self::unexpected_stop(
                    creations,
                    ProxyState::Replacing(ProxyReplacement::SuccessorResultAwaitingShutdown {
                        replaces,
                        shutdown,
                        completion: ReplacementCompletion::Ready { worker, result },
                    }),
                    stopped,
                ),
            },
            replacement => {
                Self::unexpected_stop(creations, ProxyState::Replacing(replacement), stopped)
            }
        }
    }

    fn replacement_shutdown_resolved(
        creations: CreationSequence,
        replacement: ProxyReplacement<W, P>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match replacement {
            ProxyReplacement::ReturningPredecessor {
                departure,
                successor,
                creation,
            } => match departure.shutdown_resolved(shutdown) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::Replacing(ProxyReplacement::ReturningPredecessor {
                            departure,
                            successor,
                            creation,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown: _,
                    stopped,
                })) => Self::start_successor_after_stop(
                    creations,
                    worker.attempt,
                    PredecessorShutdown::Settled,
                    successor,
                    creation,
                    stopped,
                ),
                Err((departure, input)) => Self::unexpected_shutdown(
                    creations,
                    ProxyState::Replacing(ProxyReplacement::ReturningPredecessor {
                        departure,
                        successor,
                        creation,
                    }),
                    input,
                ),
            },
            ProxyReplacement::SuccessorResultAwaitingShutdown {
                replaces,
                shutdown: expected,
                completion,
            } => match shutdown.id() {
                id if id == expected => Self::publish_replacement(creations, replaces, completion),
                _ => Self::unexpected_shutdown(
                    creations,
                    ProxyState::Replacing(ProxyReplacement::SuccessorResultAwaitingShutdown {
                        replaces,
                        shutdown: expected,
                        completion,
                    }),
                    shutdown,
                ),
            },
        }
    }

    fn finish_replacement(
        creations: CreationSequence,
        replaces: WorkerAttempt,
        predecessor_shutdown: PredecessorShutdown,
        completion: ReplacementCompletion<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match predecessor_shutdown {
            PredecessorShutdown::Awaiting { id } => (
                StableProxy {
                    state: ProxyState::Replacing(
                        ProxyReplacement::SuccessorResultAwaitingShutdown {
                            replaces,
                            shutdown: id,
                            completion,
                        },
                    ),
                    creations,
                },
                Actions::cont(),
            ),
            PredecessorShutdown::Settled => {
                Self::publish_replacement(creations, replaces, completion)
            }
        }
    }

    fn publish_replacement(
        creations: CreationSequence,
        replaces: WorkerAttempt,
        completion: ReplacementCompletion<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match completion {
            ReplacementCompletion::Ready { worker, result } => (
                StableProxy {
                    state: ProxyState::Ready { worker },
                    creations,
                },
                Self::report(ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::Resolved { replaces, result },
                }),
            ),
            ReplacementCompletion::Empty { worker, result } => (
                StableProxy {
                    state: ProxyState::EmptyAfter {
                        previous: worker.clone(),
                    },
                    creations,
                },
                Self::report(ProxyOutcome::Replacement {
                    outcome: ReplacementOutcome::Resolved { replaces, result },
                }),
            ),
            ReplacementCompletion::ReadyAfterStop {
                worker,
                result,
                stopped,
            } => (
                StableProxy {
                    state: ProxyState::EmptyAfter {
                        previous: worker.clone(),
                    },
                    creations,
                },
                Self::report_pair(
                    ProxyOutcome::Replacement {
                        outcome: ReplacementOutcome::Resolved { replaces, result },
                    },
                    ProxyOutcome::WorkerStopped { worker, stopped },
                ),
            ),
        }
    }

    fn start_successor_after_stop(
        creations: CreationSequence,
        replaces: WorkerAttempt,
        predecessor_shutdown: PredecessorShutdown,
        successor: PendingWorker<P>,
        creation: CreateChild<BehaviorAddr<StopOnShutdown<W>>, StopOnShutdown<W>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let observation = ObserveChild::new(successor.creation());
        (
            StableProxy {
                state: ProxyState::Starting(WorkerStart {
                    kind: WorkerStartKind::Replacement {
                        replaces: replaces.clone(),
                        predecessor_shutdown,
                    },
                    phase: WorkerStartPhase::Creating {
                        worker: successor,
                        stopped: None,
                    },
                }),
                creations,
            },
            Actions::new(
                ProxyEffects {
                    worker_observations: InterpreterRequests::one(observation),
                    worker_initializations: InterpreterRequests::empty(),
                    worker_activations: InterpreterRequests::empty(),
                    worker_shutdowns: InterpreterRequests::empty(),
                    worker_deliveries: Vec::new(),
                    owner_outcomes: InterpreterRequests::one(ReportToParent::new(
                        ProxyOutcome::WorkerStopped {
                            worker: replaces,
                            stopped,
                        },
                    )),
                    diagnostics: InterpreterRequests::empty(),
                },
                Creations::one(creation),
                behavior::Step::Continue,
            ),
        )
    }

    fn request_worker_shutdown(
        shutdown: ShutdownEstablished<StopOnShutdown<W>, Here>,
    ) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::empty(),
            worker_activations: InterpreterRequests::empty(),
            worker_shutdowns: InterpreterRequests::one(shutdown),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::empty(),
            diagnostics: InterpreterRequests::empty(),
        })
    }

    fn report_pair(earlier: ProxyOutcome<W, P>, later: ProxyOutcome<W, P>) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::empty(),
            worker_activations: InterpreterRequests::empty(),
            worker_shutdowns: InterpreterRequests::empty(),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::new(vec![
                ReportToParent::new(earlier),
                ReportToParent::new(later),
            ]),
            diagnostics: InterpreterRequests::empty(),
        })
    }
}

// Owner shutdown and terminal custody transitions.
impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, behavior::ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    fn owner_shutdown(current: StableProxy<W, P>) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Dormant | ProxyState::EmptyInitial => {
                Self::stop_with(current.creations, ProxyRetirement::Empty { previous: None })
            }
            ProxyState::EmptyAfter { previous } => Self::stop_with(
                current.creations,
                ProxyRetirement::Empty {
                    previous: Some(previous),
                },
            ),
            ProxyState::Ready { worker } => {
                let (departure, shutdown) = WorkerStopping::begin(worker);
                (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningWorker(departure)),
                        creations: current.creations,
                    },
                    Self::request_worker_shutdown(shutdown),
                )
            }
            ProxyState::Starting(start) => Self::shutdown_start(current.creations, start),
            ProxyState::Replacing(replacement) => {
                Self::shutdown_replacement(current.creations, replacement)
            }
            ProxyState::ShuttingDown(shutdown) => (
                StableProxy {
                    state: ProxyState::ShuttingDown(shutdown),
                    creations: current.creations,
                },
                Actions::cont(),
            ),
            ProxyState::Stopped(retirement) => Self::stop_with(current.creations, retirement),
        }
    }

    fn shutdown_start(
        creations: CreationSequence,
        start: WorkerStart<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match start {
            WorkerStart {
                kind,
                phase: WorkerStartPhase::Initializing { worker, stopped },
            } => match stopped {
                Some(stopped) => {
                    let initialization = WorkerInitializationShutdown::WaitingForInitialization {
                        worker,
                        shutdown: None,
                        stopped,
                    };
                    (
                        StableProxy {
                            state: ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                                kind,
                                initialization,
                            }),
                            creations,
                        },
                        Actions::cont(),
                    )
                }
                None => {
                    let (departure, shutdown) = WorkerStopping::begin(worker);
                    let initialization = WorkerInitializationShutdown::Departing {
                        initialization: None,
                        departure,
                    };
                    (
                        StableProxy {
                            state: ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                                kind,
                                initialization,
                            }),
                            creations,
                        },
                        Self::request_worker_shutdown(shutdown),
                    )
                }
            },
            WorkerStart {
                kind,
                phase:
                    WorkerStartPhase::Activating {
                        worker,
                        progress,
                        stopped,
                    },
            } => {
                let (activation, shutdown) =
                    Self::begin_activation_shutdown(worker, progress, stopped);
                let actions = match shutdown {
                    Some(shutdown) => Self::request_worker_shutdown(shutdown),
                    None => Actions::cont(),
                };
                (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::Activating {
                            kind,
                            activation,
                        }),
                        creations,
                    },
                    actions,
                )
            }
            WorkerStart {
                kind,
                phase: WorkerStartPhase::ReturningWorker { departure, failure },
            } => (
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::ReturningWorkerStart {
                        kind,
                        departure,
                        failure,
                    }),
                    creations,
                },
                Actions::cont(),
            ),
            start => (
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::Starting(start)),
                    creations,
                },
                Actions::cont(),
            ),
        }
    }

    fn shutdown_replacement(
        creations: CreationSequence,
        replacement: ProxyReplacement<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match replacement {
            ProxyReplacement::ReturningPredecessor {
                departure,
                successor,
                creation,
            } => {
                let replaces = departure.worker().attempt.clone();
                let (successor_attempt, worker, activation) = successor.cancel(creation);
                (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningPredecessor {
                            departure,
                            replaces: replaces.clone(),
                            successor: successor_attempt,
                        }),
                        creations,
                    },
                    Self::report(ProxyOutcome::Replacement {
                        outcome: ReplacementOutcome::CancelledBeforeBirth {
                            replaces,
                            worker,
                            activation,
                        },
                    }),
                )
            }
            ProxyReplacement::SuccessorResultAwaitingShutdown {
                replaces,
                shutdown,
                completion: ReplacementCompletion::Ready { worker, result },
            } => {
                let (departure, successor_shutdown) = WorkerStopping::begin(worker);
                (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningSuccessor {
                            replaces,
                            predecessor: PredecessorReturn::Awaiting { shutdown },
                            departure,
                            result,
                        }),
                        creations,
                    },
                    Self::request_worker_shutdown(successor_shutdown),
                )
            }
            ProxyReplacement::SuccessorResultAwaitingShutdown {
                replaces,
                shutdown,
                completion,
            } => (
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::WaitingForPredecessor {
                        replaces,
                        shutdown,
                        retirement: WorkerStartRetirement::ReplacementCompletion(completion),
                    }),
                    creations,
                },
                Actions::cont(),
            ),
        }
    }

    fn shutdown_workers_created(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match shutdown {
            ProxyShutdown::Starting(WorkerStart {
                kind,
                phase: WorkerStartPhase::Creating { worker, stopped },
            }) => match worker.created(workers, stopped) {
                WorkerCreation::Initializing {
                    worker,
                    activation,
                    stopped,
                } => Self::begin_shutdown_initialization(
                    creations, kind, worker, activation, stopped,
                ),
                WorkerCreation::Rejected {
                    rejection,
                    activation,
                    stopped,
                } => Self::finish_start_retirement(
                    creations,
                    kind,
                    WorkerStartRetirement::Result(WorkerStartResult::CreationRejected {
                        rejection,
                        activation,
                        stopped,
                    }),
                ),
                WorkerCreation::Unexpected {
                    worker,
                    stopped,
                    workers,
                } => Self::unexpected_worker_start(
                    creations,
                    ProxyShutdown::Starting(WorkerStart {
                        kind,
                        phase: WorkerStartPhase::Creating { worker, stopped },
                    }),
                    workers,
                ),
            },
            shutdown => Self::unexpected_worker_start(creations, shutdown, workers),
        }
    }

    fn begin_shutdown_initialization(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        activation: P,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let request = InitializeWorker::new(
            worker.attempt.clone(),
            worker.initialization.clone(),
            worker.actor.recipient(),
            activation,
        );
        match stopped {
            Some(stopped) => {
                let initialization = WorkerInitializationShutdown::WaitingForInitialization {
                    worker,
                    shutdown: None,
                    stopped,
                };
                (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                            kind,
                            initialization,
                        }),
                        creations,
                    },
                    Self::request_initialization(request),
                )
            }
            None => {
                let (departure, shutdown) = WorkerStopping::begin(worker);
                let initialization = WorkerInitializationShutdown::Departing {
                    initialization: None,
                    departure,
                };
                (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                            kind,
                            initialization,
                        }),
                        creations,
                    },
                    Self::initialize_and_shutdown(request, shutdown),
                )
            }
        }
    }

    fn shutdown_worker_initialized(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        initialization: WorkerInitializationReport<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match shutdown {
            ProxyShutdown::Initializing {
                kind,
                initialization: current,
            } => match Self::continue_initialization_shutdown(
                creations,
                kind,
                Self::admit_worker_initialized_during_shutdown(current, initialization),
            ) {
                Ok(changed) => changed,
                Err((creations, shutdown, input)) => {
                    Self::unexpected_initialization(creations, shutdown, input)
                }
            },
            shutdown => Self::unexpected_initialization(creations, shutdown, initialization),
        }
    }

    fn unexpected_initialization(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        initialization: WorkerInitializationReport<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        (
            StableProxy {
                state: ProxyState::ShuttingDown(shutdown),
                creations,
            },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerInitialization {
                phase: ProxyPhase::ShuttingDown,
                initialization,
            }),
        )
    }

    fn continue_initialization_shutdown<I>(
        creations: CreationSequence,
        kind: WorkerStartKind,
        changed: Result<
            ControlFlow<WorkerStartRetirement<W, P>, WorkerInitializationShutdown<W, P>>,
            (WorkerInitializationShutdown<W, P>, I),
        >,
    ) -> Result<(StableProxy<W, P>, ProxyActions<W, P>), (CreationSequence, ProxyShutdown<W, P>, I)>
    {
        match changed {
            Ok(ControlFlow::Continue(initialization)) => Ok((
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                        kind,
                        initialization,
                    }),
                    creations,
                },
                Actions::cont(),
            )),
            Ok(ControlFlow::Break(retirement)) => {
                Ok(Self::finish_start_retirement(creations, kind, retirement))
            }
            Err((initialization, input)) => Err((
                creations,
                ProxyShutdown::Initializing {
                    kind,
                    initialization,
                },
                input,
            )),
        }
    }

    fn admit_worker_initialized_during_shutdown(
        current: WorkerInitializationShutdown<W, P>,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerInitializationShutdown<W, P>>,
        (
            WorkerInitializationShutdown<W, P>,
            WorkerInitializationReport<W, P>,
        ),
    > {
        match current {
            WorkerInitializationShutdown::Departing {
                initialization: None,
                departure,
            } => match departure.worker().admit_initialization(input) {
                Ok(input) => Self::retain_initialization_while_departing(departure, input),
                Err(input) => Err((
                    WorkerInitializationShutdown::Departing {
                        initialization: None,
                        departure,
                    },
                    input,
                )),
            },
            WorkerInitializationShutdown::Departing {
                initialization,
                departure,
            } => Err((
                WorkerInitializationShutdown::Departing {
                    initialization,
                    departure,
                },
                input,
            )),
            WorkerInitializationShutdown::WaitingForInitialization {
                worker,
                shutdown,
                stopped,
            } => match worker.admit_initialization(input) {
                Ok(input) => match Self::classify_initialization_after_stop(&worker, input) {
                    Ok(initialization) => {
                        Ok(ControlFlow::Break(WorkerStartRetirement::Initialization {
                            initialization,
                            worker,
                            shutdown,
                            stopped,
                        }))
                    }
                    Err(input) => Err((
                        WorkerInitializationShutdown::WaitingForInitialization {
                            worker,
                            shutdown,
                            stopped,
                        },
                        input,
                    )),
                },
                Err(input) => Err((
                    WorkerInitializationShutdown::WaitingForInitialization {
                        worker,
                        shutdown,
                        stopped,
                    },
                    input,
                )),
            },
        }
    }

    fn rejoin_initialization_departure<I>(
        initialization: Option<WorkerInitializationRetirement<W, P>>,
        admitted: Result<ControlFlow<StoppedWorker<W>, WorkerStopping<W>>, (WorkerStopping<W>, I)>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerInitializationShutdown<W, P>>,
        (WorkerInitializationShutdown<W, P>, I),
    > {
        match admitted {
            Ok(ControlFlow::Continue(departure)) => Ok(ControlFlow::Continue(
                WorkerInitializationShutdown::Departing {
                    initialization,
                    departure,
                },
            )),
            Ok(ControlFlow::Break(departure)) => match initialization {
                None => {
                    let StoppedWorker {
                        worker,
                        shutdown,
                        stopped,
                    } = departure;
                    Ok(ControlFlow::Continue(
                        WorkerInitializationShutdown::WaitingForInitialization {
                            worker,
                            shutdown: Some(shutdown),
                            stopped,
                        },
                    ))
                }
                Some(initialization) => {
                    let StoppedWorker {
                        worker,
                        shutdown,
                        stopped,
                    } = departure;
                    Ok(ControlFlow::Break(WorkerStartRetirement::Initialization {
                        initialization,
                        worker,
                        shutdown: Some(shutdown),
                        stopped,
                    }))
                }
            },
            Err((departure, input)) => Err((
                WorkerInitializationShutdown::Departing {
                    initialization,
                    departure,
                },
                input,
            )),
        }
    }

    fn retain_initialization_while_departing(
        departure: WorkerStopping<W>,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerInitializationShutdown<W, P>>,
        (
            WorkerInitializationShutdown<W, P>,
            WorkerInitializationReport<W, P>,
        ),
    > {
        match input {
            WorkerInitializationReport::ReadyForActivation {
                permit, activation, ..
            } => Ok(ControlFlow::Continue(
                Self::wait_for_initialization_departure(
                    departure,
                    WorkerInitializationRetirement::Initialized { permit, activation },
                ),
            )),
            WorkerInitializationReport::EffectsRejected {
                failure,
                activation,
                ..
            } => Ok(ControlFlow::Continue(
                Self::wait_for_initialization_departure(
                    departure,
                    WorkerInitializationRetirement::EffectsRejected {
                        failure,
                        activation,
                    },
                ),
            )),
            WorkerInitializationReport::Stopped {
                worker,
                initialization,
                activation,
                stopped,
            } => Self::retain_initialization_stop(
                departure,
                worker,
                initialization,
                activation,
                stopped,
            ),
        }
    }

    fn retain_initialization_stop(
        departure: WorkerStopping<W>,
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        activation: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerInitializationShutdown<W, P>>,
        (
            WorkerInitializationShutdown<W, P>,
            WorkerInitializationReport<W, P>,
        ),
    > {
        match departure.stopped() {
            Some(_) => match departure.worker().admit_stop(stopped) {
                Ok(stopped) => Ok(ControlFlow::Continue(
                    Self::wait_for_initialization_departure(
                        departure,
                        WorkerInitializationRetirement::Stopped {
                            activation,
                            initialization_stop: Some(stopped),
                        },
                    ),
                )),
                Err(stopped) => Err(Self::return_unrelated_initialization_stop(
                    departure,
                    worker,
                    initialization,
                    activation,
                    stopped,
                )),
            },
            None => match departure.worker_stopped(stopped) {
                Ok(ControlFlow::Continue(departure)) => Ok(ControlFlow::Continue(
                    Self::wait_for_initialization_departure(
                        departure,
                        WorkerInitializationRetirement::Stopped {
                            activation,
                            initialization_stop: None,
                        },
                    ),
                )),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown,
                    stopped,
                })) => Ok(ControlFlow::Break(WorkerStartRetirement::Initialization {
                    initialization: WorkerInitializationRetirement::Stopped {
                        activation,
                        initialization_stop: None,
                    },
                    worker,
                    shutdown: Some(shutdown),
                    stopped,
                })),
                Err((departure, input)) => Err(Self::return_unrelated_initialization_stop(
                    departure,
                    worker,
                    initialization,
                    activation,
                    input,
                )),
            },
        }
    }

    fn classify_initialization_after_stop(
        worker: &CurrentWorker<W>,
        input: WorkerInitializationReport<W, P>,
    ) -> Result<WorkerInitializationRetirement<W, P>, WorkerInitializationReport<W, P>> {
        match input {
            WorkerInitializationReport::ReadyForActivation {
                permit, activation, ..
            } => Ok(WorkerInitializationRetirement::Initialized { permit, activation }),
            WorkerInitializationReport::EffectsRejected {
                failure,
                activation,
                ..
            } => Ok(WorkerInitializationRetirement::EffectsRejected {
                failure,
                activation,
            }),
            WorkerInitializationReport::Stopped {
                worker: attempt,
                initialization,
                activation,
                stopped,
            } => match worker.admit_stop(stopped) {
                Ok(stopped) => Ok(WorkerInitializationRetirement::Stopped {
                    activation,
                    initialization_stop: Some(stopped),
                }),
                Err(stopped) => Err(WorkerInitializationReport::Stopped {
                    worker: attempt,
                    initialization,
                    activation,
                    stopped,
                }),
            },
        }
    }

    fn wait_for_initialization_departure(
        departure: WorkerStopping<W>,
        initialization: WorkerInitializationRetirement<W, P>,
    ) -> WorkerInitializationShutdown<W, P> {
        WorkerInitializationShutdown::Departing {
            initialization: Some(initialization),
            departure,
        }
    }

    fn return_unrelated_initialization_stop(
        departure: WorkerStopping<W>,
        worker: WorkerAttempt,
        initialization: InitializationAttempt,
        activation: P,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (
        WorkerInitializationShutdown<W, P>,
        WorkerInitializationReport<W, P>,
    ) {
        (
            WorkerInitializationShutdown::Departing {
                initialization: None,
                departure,
            },
            WorkerInitializationReport::Stopped {
                worker,
                initialization,
                activation,
                stopped,
            },
        )
    }

    fn begin_activation_shutdown(
        worker: CurrentWorker<W>,
        activation: ActivationProgress,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
    ) -> (
        WorkerActivationShutdown<W, P>,
        Option<crate::ShutdownEstablished<StopOnShutdown<W>, behavior::Here>>,
    ) {
        match stopped {
            Some(stopped) => (
                WorkerActivationShutdown::WaitingForActivation {
                    activation,
                    worker,
                    shutdown: None,
                    stopped,
                },
                None,
            ),
            None => {
                let (departure, shutdown) = WorkerStopping::begin(worker);
                (
                    WorkerActivationShutdown::Departing {
                        activation: ActivationDuringDeparture::Pending(activation),
                        departure,
                    },
                    Some(shutdown),
                )
            }
        }
    }

    fn continue_activation_shutdown<I>(
        creations: CreationSequence,
        kind: WorkerStartKind,
        changed: Result<
            ControlFlow<WorkerStartRetirement<W, P>, WorkerActivationShutdown<W, P>>,
            (WorkerActivationShutdown<W, P>, I),
        >,
    ) -> Result<(StableProxy<W, P>, ProxyActions<W, P>), (CreationSequence, ProxyShutdown<W, P>, I)>
    {
        match changed {
            Ok(ControlFlow::Continue(activation)) => Ok((
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::Activating { kind, activation }),
                    creations,
                },
                Actions::cont(),
            )),
            Ok(ControlFlow::Break(retirement)) => {
                Ok(Self::finish_start_retirement(creations, kind, retirement))
            }
            Err((activation, input)) => Err((
                creations,
                ProxyShutdown::Activating { kind, activation },
                input,
            )),
        }
    }

    fn admit_worker_activation_during_shutdown(
        current: WorkerActivationShutdown<W, P>,
        input: WorkerActivation<W, P>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerActivationShutdown<W, P>>,
        (WorkerActivationShutdown<W, P>, WorkerActivation<W, P>),
    > {
        match current {
            WorkerActivationShutdown::Departing {
                activation: ActivationDuringDeparture::Pending(progress),
                departure,
            } => match Self::admit_activation(&progress, departure.worker(), input) {
                Ok(input) => Self::retain_activation_while_departing(progress, departure, input),
                Err(input) => Err((
                    WorkerActivationShutdown::Departing {
                        activation: ActivationDuringDeparture::Pending(progress),
                        departure,
                    },
                    input,
                )),
            },
            WorkerActivationShutdown::Departing {
                activation,
                departure,
            } => Err((
                WorkerActivationShutdown::Departing {
                    activation,
                    departure,
                },
                input,
            )),
            WorkerActivationShutdown::WaitingForActivation {
                activation,
                worker,
                shutdown,
                stopped,
            } => match Self::admit_activation(&activation, &worker, input) {
                Ok(input) => Self::retain_activation_after_worker(
                    activation, worker, shutdown, stopped, input,
                ),
                Err(input) => Err((
                    WorkerActivationShutdown::WaitingForActivation {
                        activation,
                        worker,
                        shutdown,
                        stopped,
                    },
                    input,
                )),
            },
        }
    }

    fn advance_activation_during_shutdown(
        progress: ActivationProgress,
        input: WorkerActivation<W, P>,
    ) -> Result<ActivationDuringDeparture<W, P>, (ActivationProgress, WorkerActivation<W, P>)> {
        match progress {
            ActivationProgress::WaitingForStart(attempt) => match input.into_started() {
                Ok(()) => Ok(ActivationDuringDeparture::Pending(
                    ActivationProgress::Running(attempt),
                )),
                Err(input) => match input.into_start_rejection() {
                    Ok((request, reason)) => Ok(ActivationDuringDeparture::Returned(
                        WorkerActivationRetirement::StartRejected { request, reason },
                    )),
                    Err(input) => Err((ActivationProgress::WaitingForStart(attempt), input)),
                },
            },
            ActivationProgress::Running(attempt) => match input.into_ready() {
                Ok(readiness) => Ok(ActivationDuringDeparture::Returned(
                    WorkerActivationRetirement::Ready { readiness },
                )),
                Err(input) => match input.into_rejection() {
                    Ok(rejection) => Ok(ActivationDuringDeparture::Returned(
                        WorkerActivationRetirement::Rejected { rejection },
                    )),
                    Err(input) => Err((ActivationProgress::Running(attempt), input)),
                },
            },
        }
    }

    fn retain_activation_while_departing(
        progress: ActivationProgress,
        departure: WorkerStopping<W>,
        input: WorkerActivation<W, P>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerActivationShutdown<W, P>>,
        (WorkerActivationShutdown<W, P>, WorkerActivation<W, P>),
    > {
        match Self::advance_activation_during_shutdown(progress, input) {
            Ok(activation) => Ok(ControlFlow::Continue(WorkerActivationShutdown::Departing {
                activation,
                departure,
            })),
            Err((progress, input)) => Err((
                WorkerActivationShutdown::Departing {
                    activation: ActivationDuringDeparture::Pending(progress),
                    departure,
                },
                input,
            )),
        }
    }

    fn retain_activation_after_worker(
        progress: ActivationProgress,
        worker: CurrentWorker<W>,
        shutdown: Option<EstablishedShutdownResolved<W::Protocol>>,
        stopped: ChildStopped<BehaviorAddr<W>>,
        input: WorkerActivation<W, P>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerActivationShutdown<W, P>>,
        (WorkerActivationShutdown<W, P>, WorkerActivation<W, P>),
    > {
        match Self::advance_activation_during_shutdown(progress, input) {
            Ok(ActivationDuringDeparture::Pending(activation)) => Ok(ControlFlow::Continue(
                WorkerActivationShutdown::WaitingForActivation {
                    activation,
                    worker,
                    shutdown,
                    stopped,
                },
            )),
            Ok(ActivationDuringDeparture::Returned(activation)) => {
                Ok(ControlFlow::Break(WorkerStartRetirement::Activation {
                    activation,
                    worker,
                    shutdown,
                    stopped,
                }))
            }
            Err((progress, input)) => Err((
                WorkerActivationShutdown::WaitingForActivation {
                    activation: progress,
                    worker,
                    shutdown,
                    stopped,
                },
                input,
            )),
        }
    }

    fn rejoin_activation_departure<I>(
        activation: ActivationDuringDeparture<W, P>,
        admitted: Result<ControlFlow<StoppedWorker<W>, WorkerStopping<W>>, (WorkerStopping<W>, I)>,
    ) -> Result<
        ControlFlow<WorkerStartRetirement<W, P>, WorkerActivationShutdown<W, P>>,
        (WorkerActivationShutdown<W, P>, I),
    > {
        match admitted {
            Ok(ControlFlow::Continue(departure)) => {
                Ok(ControlFlow::Continue(WorkerActivationShutdown::Departing {
                    activation,
                    departure,
                }))
            }
            Ok(ControlFlow::Break(StoppedWorker {
                worker,
                shutdown,
                stopped,
            })) => match activation {
                ActivationDuringDeparture::Pending(activation) => Ok(ControlFlow::Continue(
                    WorkerActivationShutdown::WaitingForActivation {
                        activation,
                        worker,
                        shutdown: Some(shutdown),
                        stopped,
                    },
                )),
                ActivationDuringDeparture::Returned(activation) => {
                    Ok(ControlFlow::Break(WorkerStartRetirement::Activation {
                        activation,
                        worker,
                        shutdown: Some(shutdown),
                        stopped,
                    }))
                }
            },
            Err((departure, input)) => Err((
                WorkerActivationShutdown::Departing {
                    activation,
                    departure,
                },
                input,
            )),
        }
    }

    fn shutdown_worker_activated(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        input: WorkerActivation<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match shutdown {
            ProxyShutdown::Activating { kind, activation } => {
                match Self::continue_activation_shutdown(
                    creations,
                    kind,
                    Self::admit_worker_activation_during_shutdown(activation, input),
                ) {
                    Ok(changed) => changed,
                    Err((creations, shutdown, input)) => {
                        Self::unexpected_shutdown_activation(creations, shutdown, input)
                    }
                }
            }
            shutdown => Self::unexpected_shutdown_activation(creations, shutdown, input),
        }
    }

    fn unexpected_shutdown_activation(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        activation: WorkerActivation<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        (
            StableProxy {
                state: ProxyState::ShuttingDown(shutdown),
                creations,
            },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerActivation {
                phase: ProxyPhase::ShuttingDown,
                activation,
            }),
        )
    }

    fn shutdown_resolved(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        resolution: EstablishedShutdownResolved<W::Protocol>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match shutdown {
            ProxyShutdown::ReturningWorker(departure) => {
                match departure.shutdown_resolved(resolution) {
                    Ok(ControlFlow::Continue(departure)) => (
                        StableProxy {
                            state: ProxyState::ShuttingDown(ProxyShutdown::ReturningWorker(
                                departure,
                            )),
                            creations,
                        },
                        Actions::cont(),
                    ),
                    Ok(ControlFlow::Break(StoppedWorker {
                        worker,
                        shutdown,
                        stopped,
                    })) => Self::finish_shutdown(creations, worker, shutdown, stopped),
                    Err((departure, input)) => Self::unexpected_shutdown(
                        creations,
                        ProxyState::ShuttingDown(ProxyShutdown::ReturningWorker(departure)),
                        input,
                    ),
                }
            }
            ProxyShutdown::ReturningWorkerStart {
                kind,
                departure,
                failure,
            } => match departure.shutdown_resolved(resolution) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningWorkerStart {
                            kind,
                            departure,
                            failure,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown,
                    stopped,
                })) => {
                    let result = Self::worker_return_result(worker, failure, shutdown, stopped);
                    Self::finish_start_retirement(
                        creations,
                        kind,
                        WorkerStartRetirement::Result(result),
                    )
                }
                Err((departure, input)) => Self::unexpected_shutdown(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::ReturningWorkerStart {
                        kind,
                        departure,
                        failure,
                    }),
                    input,
                ),
            },
            ProxyShutdown::ReturningPredecessor {
                departure,
                replaces,
                successor,
            } => match departure.shutdown_resolved(resolution) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningPredecessor {
                            departure,
                            replaces,
                            successor,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown,
                    stopped,
                })) => Self::finish_cancelled_replacement(
                    creations, replaces, successor, worker, shutdown, stopped,
                ),
                Err((departure, input)) => Self::unexpected_shutdown(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::ReturningPredecessor {
                        departure,
                        replaces,
                        successor,
                    }),
                    input,
                ),
            },
            ProxyShutdown::ReturningSuccessor {
                replaces,
                predecessor: PredecessorReturn::Awaiting { shutdown: expected },
                departure,
                result,
            } => match resolution.id() {
                id if id == expected => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningSuccessor {
                            replaces,
                            predecessor: PredecessorReturn::Returned { resolution },
                            departure,
                            result,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                _ => Self::successor_shutdown_resolved(
                    creations,
                    replaces,
                    PredecessorReturn::Awaiting { shutdown: expected },
                    departure,
                    result,
                    resolution,
                ),
            },
            ProxyShutdown::ReturningSuccessor {
                replaces,
                predecessor,
                departure,
                result,
            } => Self::successor_shutdown_resolved(
                creations,
                replaces,
                predecessor,
                departure,
                result,
                resolution,
            ),
            ProxyShutdown::Initializing {
                kind,
                initialization,
            } => match initialization {
                WorkerInitializationShutdown::Departing {
                    initialization,
                    departure,
                } => match Self::continue_initialization_shutdown(
                    creations,
                    kind,
                    Self::rejoin_initialization_departure(
                        initialization,
                        departure.shutdown_resolved(resolution),
                    ),
                ) {
                    Ok(changed) => changed,
                    Err((creations, shutdown, input)) => Self::unexpected_shutdown(
                        creations,
                        ProxyState::ShuttingDown(shutdown),
                        input,
                    ),
                },
                initialization => Self::unexpected_shutdown(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                        kind,
                        initialization,
                    }),
                    resolution,
                ),
            },
            ProxyShutdown::Activating { kind, activation } => {
                let changed = match activation {
                    WorkerActivationShutdown::Departing {
                        activation,
                        departure,
                    } => Self::rejoin_activation_departure(
                        activation,
                        departure.shutdown_resolved(resolution),
                    ),
                    activation => Err((activation, resolution)),
                };
                match Self::continue_activation_shutdown(creations, kind, changed) {
                    Ok(changed) => changed,
                    Err((creations, shutdown, input)) => Self::unexpected_shutdown(
                        creations,
                        ProxyState::ShuttingDown(shutdown),
                        input,
                    ),
                }
            }
            ProxyShutdown::WaitingForPredecessor {
                replaces,
                shutdown: expected,
                retirement,
            } => match resolution.id() {
                id if id == expected => Self::stop_with(
                    creations,
                    ProxyRetirement::ReplacementAfterPredecessor {
                        replaces,
                        predecessor: resolution,
                        retirement,
                    },
                ),
                _ => Self::unexpected_shutdown(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::WaitingForPredecessor {
                        replaces,
                        shutdown: expected,
                        retirement,
                    }),
                    resolution,
                ),
            },
            shutdown => {
                Self::unexpected_shutdown(creations, ProxyState::ShuttingDown(shutdown), resolution)
            }
        }
    }

    fn successor_shutdown_resolved(
        creations: CreationSequence,
        replaces: WorkerAttempt,
        predecessor: PredecessorReturn<W>,
        departure: WorkerStopping<W>,
        result: WorkerStartResult<W, P>,
        resolution: EstablishedShutdownResolved<W::Protocol>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match departure.shutdown_resolved(resolution) {
            Ok(ControlFlow::Continue(departure)) => (
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::ReturningSuccessor {
                        replaces,
                        predecessor,
                        departure,
                        result,
                    }),
                    creations,
                },
                Actions::cont(),
            ),
            Ok(ControlFlow::Break(departure)) => {
                Self::successor_departed(creations, replaces, predecessor, result, departure)
            }
            Err((departure, input)) => Self::unexpected_shutdown(
                creations,
                ProxyState::ShuttingDown(ProxyShutdown::ReturningSuccessor {
                    replaces,
                    predecessor,
                    departure,
                    result,
                }),
                input,
            ),
        }
    }

    fn successor_departed(
        creations: CreationSequence,
        replaces: WorkerAttempt,
        predecessor: PredecessorReturn<W>,
        result: WorkerStartResult<W, P>,
        departure: StoppedWorker<W>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let retirement = WorkerStartRetirement::ReadyWorker { result, departure };
        match predecessor {
            PredecessorReturn::Awaiting { shutdown } => (
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::WaitingForPredecessor {
                        replaces,
                        shutdown,
                        retirement,
                    }),
                    creations,
                },
                Actions::cont(),
            ),
            PredecessorReturn::Returned { resolution } => Self::stop_with(
                creations,
                ProxyRetirement::ReplacementAfterPredecessor {
                    replaces,
                    predecessor: resolution,
                    retirement,
                },
            ),
        }
    }

    fn shutdown_worker_stopped(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match shutdown {
            ProxyShutdown::ReturningWorker(departure) => match departure.worker_stopped(stopped) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningWorker(departure)),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown,
                    stopped,
                })) => Self::finish_shutdown(creations, worker, shutdown, stopped),
                Err((departure, input)) => Self::unexpected_stop(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::ReturningWorker(departure)),
                    input,
                ),
            },
            ProxyShutdown::ReturningWorkerStart {
                kind,
                departure,
                failure,
            } => match departure.worker_stopped(stopped) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningWorkerStart {
                            kind,
                            departure,
                            failure,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown,
                    stopped,
                })) => {
                    let result = Self::worker_return_result(worker, failure, shutdown, stopped);
                    Self::finish_start_retirement(
                        creations,
                        kind,
                        WorkerStartRetirement::Result(result),
                    )
                }
                Err((departure, input)) => Self::unexpected_stop(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::ReturningWorkerStart {
                        kind,
                        departure,
                        failure,
                    }),
                    input,
                ),
            },
            ProxyShutdown::ReturningPredecessor {
                departure,
                replaces,
                successor,
            } => match departure.worker_stopped(stopped) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningPredecessor {
                            departure,
                            replaces,
                            successor,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(StoppedWorker {
                    worker,
                    shutdown,
                    stopped,
                })) => Self::finish_cancelled_replacement(
                    creations, replaces, successor, worker, shutdown, stopped,
                ),
                Err((departure, input)) => Self::unexpected_stop(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::ReturningPredecessor {
                        departure,
                        replaces,
                        successor,
                    }),
                    input,
                ),
            },
            ProxyShutdown::ReturningSuccessor {
                replaces,
                predecessor,
                departure,
                result,
            } => match departure.worker_stopped(stopped) {
                Ok(ControlFlow::Continue(departure)) => (
                    StableProxy {
                        state: ProxyState::ShuttingDown(ProxyShutdown::ReturningSuccessor {
                            replaces,
                            predecessor,
                            departure,
                            result,
                        }),
                        creations,
                    },
                    Actions::cont(),
                ),
                Ok(ControlFlow::Break(departure)) => {
                    Self::successor_departed(creations, replaces, predecessor, result, departure)
                }
                Err((departure, input)) => Self::unexpected_stop(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::ReturningSuccessor {
                        replaces,
                        predecessor,
                        departure,
                        result,
                    }),
                    input,
                ),
            },
            ProxyShutdown::Initializing {
                kind,
                initialization,
            } => match initialization {
                WorkerInitializationShutdown::Departing {
                    initialization,
                    departure,
                } => match Self::continue_initialization_shutdown(
                    creations,
                    kind,
                    Self::rejoin_initialization_departure(
                        initialization,
                        departure.worker_stopped(stopped),
                    ),
                ) {
                    Ok(changed) => changed,
                    Err((creations, shutdown, input)) => {
                        Self::unexpected_stop(creations, ProxyState::ShuttingDown(shutdown), input)
                    }
                },
                initialization => Self::unexpected_stop(
                    creations,
                    ProxyState::ShuttingDown(ProxyShutdown::Initializing {
                        kind,
                        initialization,
                    }),
                    stopped,
                ),
            },
            ProxyShutdown::Activating { kind, activation } => {
                let changed = match activation {
                    WorkerActivationShutdown::Departing {
                        activation,
                        departure,
                    } => Self::rejoin_activation_departure(
                        activation,
                        departure.worker_stopped(stopped),
                    ),
                    activation => Err((activation, stopped)),
                };
                match Self::continue_activation_shutdown(creations, kind, changed) {
                    Ok(changed) => changed,
                    Err((creations, shutdown, input)) => {
                        Self::unexpected_stop(creations, ProxyState::ShuttingDown(shutdown), input)
                    }
                }
            }
            shutdown => {
                Self::unexpected_stop(creations, ProxyState::ShuttingDown(shutdown), stopped)
            }
        }
    }

    fn finish_shutdown(
        creations: CreationSequence,
        worker: CurrentWorker<W>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        Self::stop_with(
            creations,
            ProxyRetirement::Worker(StoppedWorker {
                worker,
                shutdown,
                stopped,
            }),
        )
    }

    fn finish_start_retirement(
        creations: CreationSequence,
        kind: WorkerStartKind,
        retirement: WorkerStartRetirement<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match kind {
            WorkerStartKind::Initial => Self::stop_with(
                creations,
                ProxyRetirement::WorkerStart {
                    replaces: None,
                    retirement,
                },
            ),
            WorkerStartKind::Replacement {
                replaces,
                predecessor_shutdown: PredecessorShutdown::Settled,
            } => Self::stop_with(
                creations,
                ProxyRetirement::WorkerStart {
                    replaces: Some(replaces),
                    retirement,
                },
            ),
            WorkerStartKind::Replacement {
                replaces,
                predecessor_shutdown: PredecessorShutdown::Awaiting { id },
            } => (
                StableProxy {
                    state: ProxyState::ShuttingDown(ProxyShutdown::WaitingForPredecessor {
                        replaces,
                        shutdown: id,
                        retirement,
                    }),
                    creations,
                },
                Actions::cont(),
            ),
        }
    }

    fn unexpected_worker_start(
        creations: CreationSequence,
        shutdown: ProxyShutdown<W, P>,
        workers: CreationsSettled<BehaviorAddr<W>, StopOnShutdown<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        (
            StableProxy {
                state: ProxyState::ShuttingDown(shutdown),
                creations,
            },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerStart {
                phase: ProxyPhase::ShuttingDown,
                workers,
            }),
        )
    }

    fn request_initialization(initialization: InitializeWorker<W, P>) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::one(initialization),
            worker_activations: InterpreterRequests::empty(),
            worker_shutdowns: InterpreterRequests::empty(),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::empty(),
            diagnostics: InterpreterRequests::empty(),
        })
    }

    fn initialize_and_shutdown(
        initialization: InitializeWorker<W, P>,
        shutdown: ShutdownEstablished<StopOnShutdown<W>, Here>,
    ) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::one(initialization),
            worker_activations: InterpreterRequests::empty(),
            worker_shutdowns: InterpreterRequests::one(shutdown),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::empty(),
            diagnostics: InterpreterRequests::empty(),
        })
    }

    fn finish_cancelled_replacement(
        creations: CreationSequence,
        replaces: WorkerAttempt,
        successor: WorkerAttempt,
        predecessor: CurrentWorker<W>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        Self::stop_with(
            creations,
            ProxyRetirement::ReplacementCancelled {
                replaces,
                successor,
                predecessor: StoppedWorker {
                    worker: predecessor,
                    shutdown,
                    stopped,
                },
            },
        )
    }

    fn stop_with(
        creations: CreationSequence,
        retirement: ProxyRetirement<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        (
            StableProxy {
                state: ProxyState::Stopped(retirement),
                creations,
            },
            Actions::stop(),
        )
    }
}

// Worker activation transitions.
impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, behavior::ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    fn worker_activated(
        current: StableProxy<W, P>,
        input: WorkerActivation<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Starting(WorkerStart {
                kind,
                phase:
                    WorkerStartPhase::Activating {
                        worker,
                        progress,
                        stopped,
                    },
            }) => match Self::admit_activation(&progress, &worker, input) {
                Ok(input) => match progress {
                    ActivationProgress::WaitingForStart(activation) => Self::activation_waiting(
                        current.creations,
                        kind,
                        worker,
                        activation,
                        stopped,
                        input,
                    ),
                    ActivationProgress::Running(activation) => Self::activation_running(
                        current.creations,
                        kind,
                        worker,
                        activation,
                        stopped,
                        input,
                    ),
                },
                Err(input) => Self::unexpected_activation(
                    current.creations,
                    ProxyState::Starting(WorkerStart {
                        kind,
                        phase: WorkerStartPhase::Activating {
                            worker,
                            progress,
                            stopped,
                        },
                    }),
                    input,
                ),
            },
            ProxyState::ShuttingDown(shutdown) => {
                Self::shutdown_worker_activated(current.creations, shutdown, input)
            }
            state => Self::unexpected_activation(current.creations, state, input),
        }
    }

    fn activation_waiting(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        activation: ActivationAttempt,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
        input: WorkerActivation<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match input.into_started() {
            Ok(()) => (
                StableProxy {
                    state: ProxyState::Starting(WorkerStart {
                        kind,
                        phase: WorkerStartPhase::Activating {
                            worker,
                            progress: ActivationProgress::Running(activation),
                            stopped,
                        },
                    }),
                    creations,
                },
                Actions::cont(),
            ),
            Err(input) => match input.into_start_rejection() {
                Ok((request, reason)) => match stopped {
                    Some(stopped) => Self::worker_start_empty(
                        creations,
                        kind,
                        worker.attempt.clone(),
                        WorkerStartResult::Unavailable {
                            attempt: worker.attempt,
                            drain: ProxyDrain::ActivationStartRejected {
                                request,
                                reason,
                                shutdown: None,
                                stopped,
                            },
                        },
                    ),
                    None => Self::return_worker(
                        creations,
                        kind,
                        worker,
                        PreReadyFailure::ActivationStart { request, reason },
                    ),
                },
                Err(input) => Self::unexpected_activation(
                    creations,
                    ProxyState::Starting(WorkerStart {
                        kind,
                        phase: WorkerStartPhase::Activating {
                            worker,
                            progress: ActivationProgress::WaitingForStart(activation),
                            stopped,
                        },
                    }),
                    input,
                ),
            },
        }
    }

    fn activation_running(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        activation: ActivationAttempt,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
        input: WorkerActivation<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match input.into_ready() {
            Ok(readiness) => match stopped {
                Some(stopped) => Self::worker_start_empty(
                    creations,
                    kind,
                    worker.attempt.clone(),
                    WorkerStartResult::Unavailable {
                        attempt: worker.attempt,
                        drain: ProxyDrain::ActivationCompleted { readiness, stopped },
                    },
                ),
                None => {
                    let attempt = worker.attempt.clone();
                    Self::worker_start_ready(
                        creations,
                        kind,
                        worker,
                        WorkerStartResult::Ready { attempt, readiness },
                    )
                }
            },
            Err(input) => match input.into_rejection() {
                Ok(rejection) => match stopped {
                    Some(stopped) => Self::worker_start_empty(
                        creations,
                        kind,
                        worker.attempt.clone(),
                        WorkerStartResult::Unavailable {
                            attempt: worker.attempt,
                            drain: ProxyDrain::ActivationRejected {
                                rejection,
                                shutdown: None,
                                stopped,
                            },
                        },
                    ),
                    None => Self::return_worker(
                        creations,
                        kind,
                        worker,
                        PreReadyFailure::Activation { rejection },
                    ),
                },
                Err(input) => Self::unexpected_activation(
                    creations,
                    ProxyState::Starting(WorkerStart {
                        kind,
                        phase: WorkerStartPhase::Activating {
                            worker,
                            progress: ActivationProgress::Running(activation),
                            stopped,
                        },
                    }),
                    input,
                ),
            },
        }
    }

    fn unexpected_activation(
        creations: CreationSequence,
        state: ProxyState<W, P>,
        activation: WorkerActivation<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let phase = state.phase();
        (
            StableProxy { state, creations },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerActivation { phase, activation }),
        )
    }
}

// Worker initialization transitions.
impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, behavior::ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    fn worker_initialized(
        current: StableProxy<W, P>,
        initialization: WorkerInitializationReport<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Starting(WorkerStart {
                kind,
                phase: WorkerStartPhase::Initializing { worker, stopped },
            }) => match worker.admit_initialization(initialization) {
                Ok(initialization) => Self::initialization_resolved(
                    current.creations,
                    kind,
                    worker,
                    stopped,
                    initialization,
                ),
                Err(initialization) => Self::unexpected_start_initialization(
                    current.creations,
                    kind,
                    worker,
                    stopped,
                    initialization,
                ),
            },
            ProxyState::ShuttingDown(shutdown) => {
                Self::shutdown_worker_initialized(current.creations, shutdown, initialization)
            }
            state => {
                let phase = state.phase();
                (
                    StableProxy {
                        state,
                        creations: current.creations,
                    },
                    Self::diagnose(ProxyDiagnostic::UnexpectedWorkerInitialization {
                        phase,
                        initialization,
                    }),
                )
            }
        }
    }

    fn initialization_resolved(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
        initialization: WorkerInitializationReport<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match initialization {
            WorkerInitializationReport::ReadyForActivation {
                activation, permit, ..
            } => match stopped {
                Some(stopped) => Self::worker_start_empty(
                    creations,
                    kind,
                    worker.attempt.clone(),
                    WorkerStartResult::Unavailable {
                        attempt: worker.attempt,
                        drain: ProxyDrain::InitializationCompleted {
                            permit,
                            activation,
                            stopped,
                        },
                    },
                ),
                None => {
                    let request = BeginActivation::new(activation, permit);
                    let activation = request.attempt();
                    (
                        StableProxy {
                            state: ProxyState::Starting(WorkerStart {
                                kind,
                                phase: WorkerStartPhase::Activating {
                                    worker,
                                    progress: ActivationProgress::WaitingForStart(activation),
                                    stopped: None,
                                },
                            }),
                            creations,
                        },
                        Self::begin_activation(request),
                    )
                }
            },
            WorkerInitializationReport::EffectsRejected {
                activation,
                failure,
                ..
            } => match stopped {
                Some(stopped) => Self::worker_start_empty(
                    creations,
                    kind,
                    worker.attempt.clone(),
                    WorkerStartResult::Unavailable {
                        attempt: worker.attempt,
                        drain: ProxyDrain::InitializationRejected {
                            failure,
                            activation,
                            shutdown: None,
                            stopped,
                        },
                    },
                ),
                None => Self::return_worker(
                    creations,
                    kind,
                    worker,
                    PreReadyFailure::InitializationEffects {
                        failure,
                        activation,
                    },
                ),
            },
            WorkerInitializationReport::Stopped {
                worker: worker_attempt,
                initialization,
                activation,
                stopped: returned_stop,
            } => match worker.admit_stop(returned_stop) {
                Ok(returned_stop) => {
                    let drain = ProxyDrain::InitializationStopped {
                        activation,
                        observed: stopped,
                        returned: returned_stop,
                    };
                    Self::worker_start_empty(
                        creations,
                        kind,
                        worker.attempt.clone(),
                        WorkerStartResult::Unavailable {
                            attempt: worker.attempt,
                            drain,
                        },
                    )
                }
                Err(returned_stop) => Self::unexpected_start_initialization(
                    creations,
                    kind,
                    worker,
                    stopped,
                    WorkerInitializationReport::Stopped {
                        worker: worker_attempt,
                        initialization,
                        activation,
                        stopped: returned_stop,
                    },
                ),
            },
        }
    }

    fn unexpected_start_initialization(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        stopped: Option<ChildStopped<BehaviorAddr<W>>>,
        initialization: WorkerInitializationReport<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let phase = ProxyPhase::Initializing;
        (
            StableProxy {
                state: ProxyState::Starting(WorkerStart {
                    kind,
                    phase: WorkerStartPhase::Initializing { worker, stopped },
                }),
                creations,
            },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerInitialization {
                phase,
                initialization,
            }),
        )
    }

    fn begin_activation(request: BeginActivation<W, P>) -> ProxyActions<W, P> {
        Actions::send(ProxyEffects {
            worker_observations: InterpreterRequests::empty(),
            worker_initializations: InterpreterRequests::empty(),
            worker_activations: InterpreterRequests::one(request),
            worker_shutdowns: InterpreterRequests::empty(),
            worker_deliveries: Vec::new(),
            owner_outcomes: InterpreterRequests::empty(),
            diagnostics: InterpreterRequests::empty(),
        })
    }

    fn return_worker(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        failure: PreReadyFailure<W, P>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let (departure, shutdown) = WorkerStopping::begin(worker);
        (
            StableProxy {
                state: ProxyState::Starting(WorkerStart {
                    kind,
                    phase: WorkerStartPhase::ReturningWorker { departure, failure },
                }),
                creations,
            },
            Self::request_worker_shutdown(shutdown),
        )
    }
}

// Worker shutdown settlement transitions.
impl<W, P> StableProxy<W, P>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
    <BehaviorAddr<W> as Address>::Nonce: Copy + Eq,
    StopOnShutdown<W>:
        Behavior<Protocol = W::Protocol, Error = W::Error, Ph = W::Ph, Birth = W::Birth>,
    <StopOnShutdown<W> as Behavior>::Event: InjectEvent<ShutdownRequested, Here>,
    <StopOnShutdown<W> as Behavior>::Sends: SendSettlements,
    <W::Birth as BirthMode>::Child: ChildCreationProduct<BehaviorAddr<W>, behavior::ChildHead>,
    EstablishedRecipient<W::Protocol>: Send,
{
    fn worker_shutdown_resolved(
        current: StableProxy<W, P>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match current.state {
            ProxyState::Starting(WorkerStart {
                kind:
                    WorkerStartKind::Replacement {
                        replaces,
                        predecessor_shutdown: PredecessorShutdown::Awaiting { id },
                    },
                phase,
            }) if shutdown.id() == id => (
                StableProxy {
                    state: ProxyState::Starting(WorkerStart {
                        kind: WorkerStartKind::Replacement {
                            replaces,
                            predecessor_shutdown: PredecessorShutdown::Settled,
                        },
                        phase,
                    }),
                    creations: current.creations,
                },
                Actions::cont(),
            ),
            ProxyState::Starting(WorkerStart { kind, phase }) => {
                Self::worker_start_shutdown_resolved(current.creations, kind, phase, shutdown)
            }
            ProxyState::Replacing(replacement) => {
                Self::replacement_shutdown_resolved(current.creations, replacement, shutdown)
            }
            ProxyState::ShuttingDown(proxy_shutdown) => {
                Self::shutdown_resolved(current.creations, proxy_shutdown, shutdown)
            }
            state => Self::unexpected_shutdown(current.creations, state, shutdown),
        }
    }

    fn worker_start_shutdown_resolved(
        creations: CreationSequence,
        kind: WorkerStartKind,
        phase: WorkerStartPhase<W, P>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        match phase {
            WorkerStartPhase::ReturningWorker { departure, failure } => {
                match departure.shutdown_resolved(shutdown) {
                    Ok(ControlFlow::Continue(departure)) => (
                        StableProxy {
                            state: ProxyState::Starting(WorkerStart {
                                kind,
                                phase: WorkerStartPhase::ReturningWorker { departure, failure },
                            }),
                            creations,
                        },
                        Actions::cont(),
                    ),
                    Ok(ControlFlow::Break(StoppedWorker {
                        worker,
                        shutdown,
                        stopped,
                    })) => {
                        Self::worker_returned(creations, kind, worker, failure, shutdown, stopped)
                    }
                    Err((departure, input)) => Self::unexpected_shutdown(
                        creations,
                        ProxyState::Starting(WorkerStart {
                            kind,
                            phase: WorkerStartPhase::ReturningWorker { departure, failure },
                        }),
                        input,
                    ),
                }
            }
            phase => Self::unexpected_shutdown(
                creations,
                ProxyState::Starting(WorkerStart { kind, phase }),
                shutdown,
            ),
        }
    }

    fn unexpected_shutdown(
        creations: CreationSequence,
        state: ProxyState<W, P>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let phase = state.phase();
        (
            StableProxy { state, creations },
            Self::diagnose(ProxyDiagnostic::UnexpectedWorkerShutdown { phase, shutdown }),
        )
    }

    fn worker_returned(
        creations: CreationSequence,
        kind: WorkerStartKind,
        worker: CurrentWorker<W>,
        failure: PreReadyFailure<W, P>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> (StableProxy<W, P>, ProxyActions<W, P>) {
        let worker_attempt = worker.attempt.clone();
        let result = Self::worker_return_result(worker, failure, shutdown, stopped);
        Self::worker_start_empty(creations, kind, worker_attempt, result)
    }

    fn worker_return_result(
        worker: CurrentWorker<W>,
        failure: PreReadyFailure<W, P>,
        shutdown: EstablishedShutdownResolved<W::Protocol>,
        stopped: ChildStopped<BehaviorAddr<W>>,
    ) -> WorkerStartResult<W, P> {
        let worker_attempt = worker.attempt;
        let result = match failure {
            PreReadyFailure::InitializationEffects {
                failure,
                activation,
            } => WorkerStartResult::Unavailable {
                attempt: worker_attempt,
                drain: ProxyDrain::InitializationRejected {
                    failure,
                    activation,
                    shutdown: Some(shutdown),
                    stopped,
                },
            },
            PreReadyFailure::ActivationStart { request, reason } => {
                WorkerStartResult::Unavailable {
                    attempt: worker_attempt,
                    drain: ProxyDrain::ActivationStartRejected {
                        request,
                        reason,
                        shutdown: Some(shutdown),
                        stopped,
                    },
                }
            }
            PreReadyFailure::Activation { rejection } => WorkerStartResult::Unavailable {
                attempt: worker_attempt,
                drain: ProxyDrain::ActivationRejected {
                    rejection,
                    shutdown: Some(shutdown),
                    stopped,
                },
            },
        };
        result
    }
}
