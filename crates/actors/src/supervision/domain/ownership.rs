//! Shared fixed-fleet ownership state machine.

use super::{Fleet, FleetError, RestartBudget, SlotRegistrationError};
use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ObserveChild, ObserveCreation,
    ShutdownChild, WorkerCreationResolved, WorkerStopped,
};
use crate::supervision::adapter::{
    ChildTopology, ProxyWithParent, RestartConfiguration, SupervisorSends,
};
use crate::supervision::{RestartPolicy, SupervisionFailure};
use crate::{CreationKind, Exit, ReportSupervisionFailure, StableSlotRejection};
use behavior::{
    Actions, Address, Behavior, Births, ChildDelivery, ChildRoute, Create, InterpreterRequests,
    Never, SendEffects, Step,
};

type OwnedActions<A, C, P> =
    Actions<A, Never, SupervisorSends<A, C, P>, Births<ProxyWithParent<C, P>>>;

pub(crate) struct OwnershipFold<A, C, P>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    pub actions: OwnedActions<A, C, P>,
    pub failures: Vec<SupervisionFailure<A>>,
}

impl<A, C, P> OwnershipFold<A, C, P>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn actions(actions: OwnedActions<A, C, P>) -> Self {
        Self {
            actions,
            failures: Vec::new(),
        }
    }
    fn failed(actions: OwnedActions<A, C, P>, failure: SupervisionFailure<A>) -> Self {
        Self {
            actions,
            failures: vec![failure],
        }
    }

    fn failed_many(actions: OwnedActions<A, C, P>, failures: Vec<SupervisionFailure<A>>) -> Self {
        Self { actions, failures }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum OwnershipError<A: Address> {
    #[error(transparent)]
    Fleet(#[from] FleetError<A::Nonce>),
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected(ChildShutdownRejected<A::Nonce>),
    #[error("a creation result does not belong to a pending stable-child creation")]
    UnexpectedCreation(CreationResolved<A>),
    #[error("a stable-child stop does not belong to the current ownership state")]
    UnexpectedChildStopped(ChildStopped<A>),
    #[error("a worker stop does not belong to the current worker incarnation")]
    UnexpectedWorkerStopped(WorkerStopped<A>),
    #[error("a worker creation result does not belong to a pending worker creation")]
    UnexpectedWorkerCreation(WorkerCreationResolved<A::Nonce>),
    #[error("a child-shutdown rejection does not belong to an outstanding shutdown request")]
    UnexpectedChildShutdownRejection(ChildShutdownRejected<A::Nonce>),
    #[error("stable-child creation provenance did not match the pending request")]
    CreationProvenanceMismatch {
        expected: CreationKind<A::Nonce>,
        observed: CreationResolved<A>,
    },
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        expected: CreationKind<A::Nonce>,
        observed: WorkerCreationResolved<A::Nonce>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Installation<N> {
    Pending {
        kind: CreationKind<N>,
    },
    Established,
    ShutdownRequested,
    Rejected {
        kind: CreationKind<N>,
        rejection: crate::CreationRejection,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkerInstallation<N> {
    AwaitingInitial,
    Running { worker: N },
    ReplacementQueuedUnknown,
    ReplacementQueued { worker: N },
    AwaitingReplacement { replaces: N },
    Retired,
}

enum Shutdown<N> {
    Running,
    Draining { awaiting: Vec<N> },
}

enum Replacement<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    Retire,
    Replace(Vec<ChildDelivery<crate::Proxy<C>, behavior::ChildHead>>),
    Failed(SupervisionFailure<A>),
}

/// The sole owner of fixed topology, installation, restart and drain state.
pub(crate) struct FixedFleetOwnership<A, C, P>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fleet: Fleet<A::Nonce>,
    build: fn(usize) -> Option<C>,
    restart: RestartConfiguration,
    budget: RestartBudget,
    parent: crate::ProxyParentIngress<A, P>,
    installations: Vec<(A::Nonce, Installation<A::Nonce>)>,
    workers: Vec<(A::Nonce, WorkerInstallation<A::Nonce>)>,
    shutdown: Shutdown<A::Nonce>,
}

impl<A, C, P> FixedFleetOwnership<A, C, P>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        restart: RestartConfiguration,
        parent: crate::ProxyParentIngress<A, P>,
    ) -> Result<Self, FleetError<A::Nonce>> {
        let installations = topology
            .nonces
            .iter()
            .copied()
            .map(|n| {
                (
                    n,
                    Installation::Pending {
                        kind: CreationKind::Birth,
                    },
                )
            })
            .collect();
        let workers = topology
            .nonces
            .iter()
            .copied()
            .map(|n| (n, WorkerInstallation::AwaitingInitial))
            .collect();
        Ok(Self {
            fleet: Fleet::configured(topology.nonces)?,
            build: topology.build,
            budget: RestartBudget::new(restart.maximum, restart.window),
            restart,
            parent,
            installations,
            workers,
            shutdown: Shutdown::Running,
        })
    }

    pub fn is_alive(&self, nonce: A::Nonce) -> Result<bool, FleetError<A::Nonce>> {
        self.fleet.is_available(nonce)
    }
    pub fn child_count(&self) -> usize {
        self.fleet.len()
    }
    pub fn restarts_in_window(&self) -> usize {
        self.budget.admitted()
    }
    pub fn is_shutting_down(&self) -> bool {
        matches!(self.shutdown, Shutdown::Draining { .. })
    }

    fn worker(&self, proxy: A::Nonce) -> Option<WorkerInstallation<A::Nonce>> {
        self.workers
            .iter()
            .find_map(|(nonce, state)| (*nonce == proxy).then_some(*state))
    }

    fn set_worker(&mut self, proxy: A::Nonce, next: WorkerInstallation<A::Nonce>) {
        if let Some((_, state)) = self.workers.iter_mut().find(|(nonce, _)| *nonce == proxy) {
            *state = next;
        }
    }

    pub fn initialize(&mut self) -> Result<OwnedActions<A, C, P>, OwnershipError<A>> {
        let nonces: Vec<_> = self.fleet.configured_nonces().collect();
        let children = nonces
            .iter()
            .copied()
            .enumerate()
            .map(|(index, nonce)| {
                (self.build)(index)
                    .map(|child| (nonce, child))
                    .ok_or(OwnershipError::FactoryIndex { index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sends = SupervisorSends::empty();
        sends.child_observations = InterpreterRequests::new(
            nonces
                .iter()
                .copied()
                .map(|nonce| {
                    ObserveChild::at(
                        ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(nonce),
                    )
                })
                .collect(),
        );
        sends.creation_observations = InterpreterRequests::new(
            nonces
                .iter()
                .copied()
                .map(|nonce| {
                    ObserveCreation::at(
                        ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(nonce),
                    )
                })
                .collect(),
        );
        Ok(Actions::new(
            sends,
            children
                .into_iter()
                .map(|(nonce, child)| {
                    ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(nonce)
                        .birth(ProxyWithParent::with_parent(child, self.parent))
                })
                .collect(),
            Step::Continue,
        ))
    }

    pub fn adopt(&mut self, creates: Vec<Create<A, C>>) -> OwnershipFold<A, C, P> {
        let mut accepted = Vec::new();
        let mut failures = Vec::new();
        for create in creates {
            let rejection = match self.fleet.register(create.nonce) {
                Ok(()) => None,
                Err(SlotRegistrationError::DuplicateChild(_)) => {
                    Some(StableSlotRejection::DuplicateNonce)
                }
                Err(SlotRegistrationError::SequenceExhausted) => {
                    Some(StableSlotRejection::SequenceExhausted)
                }
            };
            if let Some(rejection) = rejection {
                failures.push(SupervisionFailure::stable_child_not_accepted(
                    create.nonce,
                    create.kind,
                    rejection,
                ));
            } else {
                accepted.push(create);
            }
        }
        for create in &accepted {
            self.installations
                .push((create.nonce, Installation::Pending { kind: create.kind }));
            self.workers
                .push((create.nonce, WorkerInstallation::AwaitingInitial));
            if let Shutdown::Draining { awaiting } = &mut self.shutdown {
                awaiting.push(create.nonce);
            }
        }
        let mut sends = SupervisorSends::empty();
        sends.child_observations = InterpreterRequests::new(
            accepted
                .iter()
                .map(|creation| {
                    ObserveChild::at(
                        ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(
                            creation.nonce,
                        ),
                    )
                })
                .collect(),
        );
        sends.creation_observations = InterpreterRequests::new(
            accepted
                .iter()
                .map(|creation| {
                    ObserveCreation::at(
                        ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(
                            creation.nonce,
                        ),
                    )
                })
                .collect(),
        );
        let births = accepted
            .into_iter()
            .map(|creation| {
                ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(creation.nonce).stage(
                    ProxyWithParent::with_parent(creation.child, self.parent),
                    creation.kind,
                )
            })
            .collect();
        for failure in failures.iter().copied() {
            sends
                .failure_reports
                .send(ReportSupervisionFailure::new(failure));
        }
        OwnershipFold::failed_many(Actions::new(sends, births, Step::Continue), failures)
    }

    fn replacement(
        &mut self,
        event: &WorkerStopped<A>,
    ) -> Result<Replacement<A, C>, OwnershipError<A>> {
        let eligible = match self.restart.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => {
                !matches!(&event.outcome, Ok(Exit::Normal | Exit::Collected))
            }
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            self.fleet.retire(event.proxy)?;
            return Ok(Replacement::Retire);
        }
        let candidates = self
            .fleet
            .replacements(event.proxy, self.restart.strategy)?
            .into_iter()
            .filter(|candidate| {
                candidate.nonce == event.proxy
                    || matches!(
                        self.worker(candidate.nonce),
                        Some(
                            WorkerInstallation::AwaitingInitial
                                | WorkerInstallation::Running { .. }
                        )
                    )
            })
            .collect::<Vec<_>>();
        let replacements = candidates
            .iter()
            .map(|candidate| {
                (self.build)(candidate.index)
                    .map(|child| (candidate.nonce, child))
                    .ok_or((candidate.nonce, candidate.index))
            })
            .collect::<Result<Vec<_>, _>>();
        let replacements = match replacements {
            Ok(replacements) => replacements,
            Err((child, index)) => {
                self.fleet.retire(event.proxy)?;
                self.set_worker(event.proxy, WorkerInstallation::Retired);
                return Ok(Replacement::Failed(
                    SupervisionFailure::worker_factory_rejected(child, index),
                ));
            }
        };
        if let Err(reason) = self.budget.admit(event.at, candidates.len()) {
            self.fleet.retire(event.proxy)?;
            self.set_worker(event.proxy, WorkerInstallation::Retired);
            return Ok(Replacement::Failed(SupervisionFailure::restart_denied(
                event.proxy,
                event.outcome,
                reason,
            )));
        }
        for candidate in &candidates {
            self.fleet.replacement_requested(candidate.nonce)?;
            let next = if candidate.nonce == event.proxy {
                WorkerInstallation::AwaitingReplacement {
                    replaces: event.worker,
                }
            } else {
                match self.worker(candidate.nonce) {
                    Some(WorkerInstallation::AwaitingInitial) => {
                        WorkerInstallation::ReplacementQueuedUnknown
                    }
                    Some(WorkerInstallation::Running { worker }) => {
                        WorkerInstallation::ReplacementQueued { worker }
                    }
                    _ => continue,
                }
            };
            self.set_worker(candidate.nonce, next);
        }
        Ok(Replacement::Replace(
            replacements
                .into_iter()
                .map(|(nonce, child)| {
                    let route =
                        ChildRoute::<ProxyWithParent<C, P>, behavior::ChildHead>::new(nonce);
                    ChildDelivery::at(route, crate::ProxyCommand::Replace(child))
                })
                .collect(),
        ))
    }

    pub(crate) fn validate_worker_stopped(
        &self,
        event: &WorkerStopped<A>,
    ) -> Result<(), OwnershipError<A>> {
        if !self.fleet.contains(event.proxy) {
            return Err(OwnershipError::UnexpectedWorkerStopped(event.clone()));
        }
        let belongs = match self.worker(event.proxy) {
            Some(WorkerInstallation::AwaitingInitial)
            | Some(WorkerInstallation::ReplacementQueuedUnknown) => true,
            Some(WorkerInstallation::Running { worker })
            | Some(WorkerInstallation::ReplacementQueued { worker }) => worker == event.worker,
            Some(WorkerInstallation::AwaitingReplacement { .. } | WorkerInstallation::Retired)
            | None => false,
        };
        if belongs {
            Ok(())
        } else {
            Err(OwnershipError::UnexpectedWorkerStopped(event.clone()))
        }
    }

    pub fn worker_stopped(
        &mut self,
        event: WorkerStopped<A>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A>> {
        self.validate_worker_stopped(&event)?;
        if self.is_shutting_down() {
            self.set_worker(event.proxy, WorkerInstallation::Retired);
            return Ok(OwnershipFold::actions(Actions::cont()));
        }
        match self.worker(event.proxy) {
            Some(WorkerInstallation::ReplacementQueuedUnknown) => {
                self.set_worker(
                    event.proxy,
                    WorkerInstallation::AwaitingReplacement {
                        replaces: event.worker,
                    },
                );
                return Ok(OwnershipFold::actions(Actions::cont()));
            }
            Some(WorkerInstallation::ReplacementQueued { worker }) if worker == event.worker => {
                self.set_worker(
                    event.proxy,
                    WorkerInstallation::AwaitingReplacement {
                        replaces: event.worker,
                    },
                );
                return Ok(OwnershipFold::actions(Actions::cont()));
            }
            Some(WorkerInstallation::Running { worker }) if worker == event.worker => {}
            Some(WorkerInstallation::AwaitingInitial) => {
                // An exact worker-stop report proves that this incarnation was
                // installed even if its earlier creation report has not yet
                // been folded. No identity or provenance is inferred.
                self.set_worker(
                    event.proxy,
                    WorkerInstallation::Running {
                        worker: event.worker,
                    },
                );
            }
            _ => return Err(OwnershipError::UnexpectedWorkerStopped(event)),
        }
        match self.replacement(&event)? {
            Replacement::Retire => {
                self.set_worker(event.proxy, WorkerInstallation::Retired);
                Ok(OwnershipFold::actions(Actions::cont()))
            }
            Replacement::Replace(commands) => {
                let mut sends = SupervisorSends::empty();
                sends.replacement_commands = commands;
                Ok(OwnershipFold::actions(Actions::send(sends)))
            }
            Replacement::Failed(failure) => {
                let mut sends = SupervisorSends::empty();
                sends
                    .failure_reports
                    .send(ReportSupervisionFailure::new(failure));
                Ok(OwnershipFold::failed(Actions::send(sends), failure))
            }
        }
    }

    pub fn child_stopped(
        &mut self,
        event: ChildStopped<A>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A>> {
        if let Shutdown::Draining { awaiting } = &mut self.shutdown {
            if awaiting.contains(&event.nonce) {
                awaiting.retain(|n| *n != event.nonce);
                self.fleet.retire(event.nonce)?;
                return Ok(OwnershipFold::actions(if awaiting.is_empty() {
                    Actions::stop()
                } else {
                    Actions::cont()
                }));
            }
            return Err(OwnershipError::UnexpectedChildStopped(event));
        }
        if !self.fleet.contains(event.nonce) {
            return Err(OwnershipError::UnexpectedChildStopped(event));
        }
        self.fleet.retire(event.nonce)?;
        let failure = SupervisionFailure::stable_child_stopped(event.nonce, event.outcome);
        let mut sends = SupervisorSends::empty();
        sends
            .failure_reports
            .send(ReportSupervisionFailure::new(failure));
        Ok(OwnershipFold::failed(Actions::send(sends), failure))
    }

    pub fn creation_resolved(
        &mut self,
        event: CreationResolved<A>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A>> {
        let Some(position) = self
            .installations
            .iter()
            .position(|(n, _)| *n == event.nonce)
        else {
            return Err(OwnershipError::UnexpectedCreation(event));
        };
        let Installation::Pending { kind: expected } = self.installations[position].1 else {
            return Err(OwnershipError::UnexpectedCreation(event));
        };
        if event.kind != expected {
            return Err(OwnershipError::CreationProvenanceMismatch {
                expected,
                observed: event,
            });
        }
        self.installations[position].1 = match event.result {
            Ok(_) => Installation::Established,
            Err(rejection) => Installation::Rejected {
                kind: event.kind,
                rejection,
            },
        };
        self.fleet
            .resolve_creation(event.nonce, event.result.map(|_| ()));
        let failure = event.result.err().map(|rejection| {
            self.set_worker(event.nonce, WorkerInstallation::Retired);
            SupervisionFailure::stable_child_creation_rejected(event.nonce, event.kind, rejection)
        });
        let mut sends = SupervisorSends::empty();
        if let Some(failure) = failure {
            sends
                .failure_reports
                .send(ReportSupervisionFailure::new(failure));
        }
        if let Shutdown::Draining { awaiting } = &mut self.shutdown {
            if let Some(failure) = failure {
                awaiting.retain(|n| *n != event.nonce);
                let actions = if awaiting.is_empty() {
                    Actions::new(sends, Vec::new(), Step::Stop(behavior::Stopped))
                } else {
                    Actions::send(sends)
                };
                return Ok(OwnershipFold::failed(actions, failure));
            }
            self.installations[position].1 = Installation::ShutdownRequested;
            sends.shutdowns.send(ShutdownChild::at(ChildRoute::<
                ProxyWithParent<C, P>,
                behavior::ChildHead,
            >::new(event.nonce)));
            return Ok(OwnershipFold::actions(Actions::send(sends)));
        }
        Ok(match failure {
            Some(failure) => OwnershipFold::failed(Actions::send(sends), failure),
            None => OwnershipFold::actions(Actions::cont()),
        })
    }

    pub fn worker_creation_resolved(
        &mut self,
        event: WorkerCreationResolved<A::Nonce>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A>> {
        let expected = self.validate_worker_creation_resolved(&event)?;
        Ok(match event.result {
            Ok(()) => {
                self.set_worker(
                    event.proxy,
                    WorkerInstallation::Running {
                        worker: event.worker,
                    },
                );
                OwnershipFold::actions(Actions::cont())
            }
            Err(rejection) => {
                self.fleet.retire(event.proxy)?;
                self.set_worker(event.proxy, WorkerInstallation::Retired);
                let failure = SupervisionFailure::worker_creation_rejected(
                    event.proxy,
                    event.worker,
                    expected,
                    rejection,
                );
                let mut sends = SupervisorSends::empty();
                sends
                    .failure_reports
                    .send(ReportSupervisionFailure::new(failure));
                OwnershipFold::failed(Actions::send(sends), failure)
            }
        })
    }

    pub(crate) fn validate_worker_creation_resolved(
        &self,
        event: &WorkerCreationResolved<A::Nonce>,
    ) -> Result<CreationKind<A::Nonce>, OwnershipError<A>> {
        if !self.fleet.contains(event.proxy) {
            return Err(OwnershipError::UnexpectedWorkerCreation(*event));
        }
        let expected = match self.worker(event.proxy) {
            Some(WorkerInstallation::AwaitingInitial) => CreationKind::Birth,
            Some(WorkerInstallation::AwaitingReplacement { replaces }) => {
                CreationKind::ReplacementIncarnation { replaces }
            }
            _ => return Err(OwnershipError::UnexpectedWorkerCreation(*event)),
        };
        if event.kind != expected {
            return Err(OwnershipError::WorkerCreationProvenanceMismatch {
                expected,
                observed: *event,
            });
        }
        Ok(expected)
    }

    pub fn shutdown(&mut self) -> OwnershipFold<A, C, P> {
        if self.is_shutting_down() {
            return OwnershipFold::actions(Actions::cont());
        }
        let awaiting = self
            .installations
            .iter()
            .filter_map(|(n, s)| (!matches!(s, Installation::Rejected { .. })).then_some(*n))
            .collect::<Vec<_>>();
        if awaiting.is_empty() {
            return OwnershipFold::actions(Actions::stop());
        }
        let mut sends = SupervisorSends::empty();
        for (nonce, state) in &mut self.installations {
            if *state == Installation::Established {
                *state = Installation::ShutdownRequested;
                sends.shutdowns.send(ShutdownChild::at(ChildRoute::<
                    ProxyWithParent<C, P>,
                    behavior::ChildHead,
                >::new(*nonce)));
            }
        }
        self.shutdown = Shutdown::Draining { awaiting };
        OwnershipFold::actions(Actions::send(sends))
    }

    pub fn child_shutdown_rejected(
        &self,
        event: ChildShutdownRejected<A::Nonce>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A>> {
        if matches!(&self.shutdown, Shutdown::Draining { awaiting } if awaiting.contains(&event.nonce))
        {
            Err(OwnershipError::ChildShutdownRejected(event))
        } else {
            Err(OwnershipError::UnexpectedChildShutdownRejection(event))
        }
    }
}
