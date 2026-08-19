//! Shared fixed-fleet ownership state machine.

use super::{Fleet, FleetError, RestartBudget};
use crate::protocol::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ObserveChild, ObserveCreation,
    ShutdownChild, WorkerCreationResolved, WorkerStopped,
};
use crate::supervision::adapter::{
    ChildTopology, Proxy, ProxyWithParent, RestartConfiguration, SupervisorSends,
};
use crate::supervision::{RestartPolicy, SupervisionFailure};
use crate::{Exit, ReportSupervisionFailure, SupervisionFailureReason};
use behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, InterpreterRequests, Never, SendEffects,
    Step,
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
    pub failure: Option<SupervisionFailure<A>>,
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
            failure: None,
        }
    }
    fn failed(actions: OwnedActions<A, C, P>, failure: SupervisionFailure<A>) -> Self {
        Self {
            actions,
            failure: Some(failure),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum OwnershipError<N> {
    #[error(transparent)]
    Fleet(#[from] FleetError<N>),
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: N,
        reason: crate::ChildShutdownRejection,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Installation {
    Pending,
    Established,
    ShutdownRequested,
    Rejected,
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
    Replace(Vec<Delivery<Proxy<C>>>),
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
    installations: Vec<(A::Nonce, Installation)>,
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
            .map(|n| (n, Installation::Pending))
            .collect();
        Ok(Self {
            fleet: Fleet::configured(topology.nonces)?,
            build: topology.build,
            budget: RestartBudget::new(restart.maximum, restart.window),
            restart,
            parent,
            installations,
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

    pub fn initialize(&mut self) -> Result<OwnedActions<A, C, P>, OwnershipError<A::Nonce>> {
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
        sends.child_observations =
            InterpreterRequests::new(nonces.iter().copied().map(ObserveChild::new).collect());
        sends.creation_observations =
            InterpreterRequests::new(nonces.iter().copied().map(ObserveCreation::new).collect());
        Ok(Actions::new(
            sends,
            children
                .into_iter()
                .map(|(nonce, child)| {
                    Create::birth(nonce, ProxyWithParent::with_parent(child, self.parent))
                })
                .collect(),
            Step::Continue,
        ))
    }

    pub fn adopt(
        &mut self,
        creates: Vec<Create<A, C>>,
    ) -> Result<OwnedActions<A, C, P>, OwnershipError<A::Nonce>> {
        for create in &creates {
            self.fleet.register(create.nonce)?;
            self.installations
                .push((create.nonce, Installation::Pending));
            if let Shutdown::Draining { awaiting } = &mut self.shutdown {
                awaiting.push(create.nonce);
            }
        }
        let mut sends = SupervisorSends::empty();
        sends.child_observations =
            InterpreterRequests::new(creates.iter().map(|c| ObserveChild::new(c.nonce)).collect());
        sends.creation_observations = InterpreterRequests::new(
            creates
                .iter()
                .map(|c| ObserveCreation::new(c.nonce))
                .collect(),
        );
        Ok(Actions::new(
            sends,
            creates
                .into_iter()
                .map(|c| {
                    Create::new(
                        c.nonce,
                        ProxyWithParent::with_parent(c.child, self.parent),
                        c.kind,
                    )
                })
                .collect(),
            Step::Continue,
        ))
    }

    fn replacement(
        &mut self,
        event: &WorkerStopped<A>,
    ) -> Result<Replacement<A, C>, OwnershipError<A::Nonce>> {
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
            .replacements(event.proxy, self.restart.strategy)?;
        let replacements = candidates
            .iter()
            .map(|candidate| {
                (self.build)(candidate.index)
                    .map(|child| (candidate.nonce, child))
                    .ok_or(OwnershipError::FactoryIndex {
                        index: candidate.index,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Err(reason) = self.budget.admit(event.at, candidates.len()) {
            self.fleet.retire(event.proxy)?;
            return Ok(Replacement::Failed(SupervisionFailure::new(
                event.proxy,
                event.outcome,
                SupervisionFailureReason::RestartDenied(reason),
            )));
        }
        for candidate in &candidates {
            self.fleet.replacement_requested(candidate.nonce)?;
        }
        Ok(Replacement::Replace(
            replacements
                .into_iter()
                .map(|(nonce, child)| {
                    Delivery::local_child(
                        behavior::ChildRecipient::new(nonce),
                        crate::ProxyCommand::Replace(child),
                    )
                })
                .collect(),
        ))
    }

    pub fn worker_stopped(
        &mut self,
        event: WorkerStopped<A>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A::Nonce>> {
        if self.is_shutting_down() || !self.fleet.contains(event.proxy) {
            return Ok(OwnershipFold::actions(Actions::cont()));
        }
        match self.replacement(&event)? {
            Replacement::Retire => Ok(OwnershipFold::actions(Actions::cont())),
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
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A::Nonce>> {
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
            return Ok(OwnershipFold::actions(Actions::cont()));
        }
        if !self.fleet.contains(event.nonce) {
            return Ok(OwnershipFold::actions(Actions::cont()));
        }
        self.fleet.retire(event.nonce)?;
        let failure = SupervisionFailure::new(
            event.nonce,
            event.outcome,
            SupervisionFailureReason::StableChildStopped,
        );
        let mut sends = SupervisorSends::empty();
        sends
            .failure_reports
            .send(ReportSupervisionFailure::new(failure));
        Ok(OwnershipFold::failed(Actions::send(sends), failure))
    }

    pub fn creation_resolved(&mut self, event: CreationResolved<A>) -> OwnershipFold<A, C, P> {
        let draining = self.is_shutting_down();
        let Some((_, state)) = self
            .installations
            .iter_mut()
            .find(|(n, _)| *n == event.nonce)
        else {
            return OwnershipFold::actions(Actions::cont());
        };
        if draining && *state != Installation::Pending {
            return OwnershipFold::actions(Actions::cont());
        }
        *state = if event.result.is_ok() {
            Installation::Established
        } else {
            Installation::Rejected
        };
        self.fleet
            .resolve_creation(event.nonce, event.result.map(|_| ()));
        if let Shutdown::Draining { awaiting } = &mut self.shutdown {
            if event.result.is_err() {
                awaiting.retain(|n| *n != event.nonce);
                return OwnershipFold::actions(if awaiting.is_empty() {
                    Actions::stop()
                } else {
                    Actions::cont()
                });
            }
            *state = Installation::ShutdownRequested;
            let mut sends = SupervisorSends::empty();
            sends
                .shutdowns
                .send(ShutdownChild::<ProxyWithParent<C, P>>::new(event.nonce));
            return OwnershipFold::actions(Actions::send(sends));
        }
        OwnershipFold::actions(Actions::cont())
    }

    pub fn worker_creation_resolved(
        &self,
        _: WorkerCreationResolved<A::Nonce>,
    ) -> OwnershipFold<A, C, P> {
        OwnershipFold::actions(Actions::cont())
    }

    pub fn shutdown(&mut self) -> OwnershipFold<A, C, P> {
        if self.is_shutting_down() {
            return OwnershipFold::actions(Actions::cont());
        }
        let awaiting = self
            .installations
            .iter()
            .filter_map(|(n, s)| (*s != Installation::Rejected).then_some(*n))
            .collect::<Vec<_>>();
        if awaiting.is_empty() {
            return OwnershipFold::actions(Actions::stop());
        }
        let mut sends = SupervisorSends::empty();
        for (nonce, state) in &mut self.installations {
            if *state == Installation::Established {
                *state = Installation::ShutdownRequested;
                sends
                    .shutdowns
                    .send(ShutdownChild::<ProxyWithParent<C, P>>::new(*nonce));
            }
        }
        self.shutdown = Shutdown::Draining { awaiting };
        OwnershipFold::actions(Actions::send(sends))
    }

    pub fn child_shutdown_rejected(
        &self,
        event: ChildShutdownRejected<A::Nonce>,
    ) -> Result<OwnershipFold<A, C, P>, OwnershipError<A::Nonce>> {
        if matches!(&self.shutdown, Shutdown::Draining { awaiting } if awaiting.contains(&event.nonce))
        {
            Err(OwnershipError::ChildShutdownRejected {
                nonce: event.nonce,
                reason: event.reason,
            })
        } else {
            Ok(OwnershipFold::actions(Actions::cont()))
        }
    }
}
