//! One fixed member's retained ownership during supervisor shutdown.

use core::ops::ControlFlow;

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, ChildReport, CreationId, CreationKind,
    EndpointAddress, ItemSettlement, SettledItem,
};

use crate::atomic::stable_proxy::ProxyOperationWitness;
use crate::atomic::worker::{PreparationTicket, preparation_result_accepts};
use crate::atomic::{PrepareWorkers, RoleName, WorkerSource};
use crate::{
    ChildStopped, ProxyInputResult, ProxyOperation, ProxyOperationId, ProxyOutcome, StableProxy,
    WorkerAttempt,
};

use super::super::ActivationPlan;
use super::super::protocol::{MemberStatus, UnavailablePhase};
use super::super::proxy::{ProxyCreationSettlement, ProxyStoppingMember, RetiredProxyMember};
use super::super::recovery::{ReplacementResponse, WorkerReplacement};
use super::super::role::{MemberRole, RosterPosition};

pub(in super::super) enum FixedShutdownMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    AwaitingProxyBirth {
        role: MemberRole<Role>,
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
    },
    Proxy {
        proxy: ControlFlow<
            RetiredProxyMember<Role, Worker, Plan>,
            ProxyStoppingMember<Role, Worker, Plan>,
        >,
        ownership: FixedProxyOwnership<Worker, Plan>,
        preparation: Option<PreparationTicket>,
    },
    #[expect(dead_code, reason = "retained until supervisor retirement")]
    AbsentProxy {
        role: MemberRole<Role>,
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
        settlement: Option<ProxyCreationSettlement<Worker, Plan>>,
    },
}

pub(in super::super) enum FixedProxyOwnership<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    #[expect(dead_code, reason = "retained until supervisor retirement")]
    Ready {
        worker: WorkerAttempt,
        readiness: Plan::Ready,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    },
    #[expect(dead_code, reason = "retained until supervisor retirement")]
    WorkerStopped {
        worker: WorkerAttempt,
        readiness: Plan::Ready,
        stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    },
    #[expect(dead_code, reason = "retained until supervisor retirement")]
    InitialInputCancelled {
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
    },
    InitialInputDispatched {
        witness: ProxyOperationWitness,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    },
    InitialOutcomeWaiting {
        operation: ProxyOperationId,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    },
    #[expect(dead_code, reason = "retained until supervisor retirement")]
    InitialOutcomeReceived {
        operation: ProxyOperationId,
        outcome: ProxyOutcome<Worker, Plan>,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    },
    #[expect(dead_code, reason = "retained until supervisor retirement")]
    InitialInputRejected {
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    },
    Replacement(WorkerReplacement<Worker, Plan>),
    Unavailable,
}

impl<Role, Worker, Plan> FixedShutdownMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn append_status(
        &self,
        members: &mut Vec<(RosterPosition, MemberStatus<Worker::Protocol>)>,
    ) {
        let (position, phase) = match self {
            Self::AwaitingProxyBirth { role, .. } => (role.position(), UnavailablePhase::Stopping),
            Self::AbsentProxy { role, .. } => (role.position(), UnavailablePhase::Retired),
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ..
            } => (proxy.position(), UnavailablePhase::Stopping),
            Self::Proxy {
                proxy: ControlFlow::Break(proxy),
                ..
            } => (proxy.position(), UnavailablePhase::Retired),
        };
        members.push((position, phase.status()));
    }

    pub(in super::super) fn capability_phase(&self, expected: &Role) -> Option<UnavailablePhase>
    where
        Role: Eq,
    {
        let (role, phase) = match self {
            Self::AwaitingProxyBirth { role, .. } => (role.role(), UnavailablePhase::Stopping),
            Self::AbsentProxy { role, .. } => (role.role(), UnavailablePhase::Retired),
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ..
            } => (proxy.role(), UnavailablePhase::Stopping),
            Self::Proxy {
                proxy: ControlFlow::Break(proxy),
                ..
            } => (proxy.role(), UnavailablePhase::Retired),
        };
        (role == expected).then_some(phase)
    }

    pub(in super::super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ..
            } => proxy.live_proxy_role(child),
            Self::Proxy {
                proxy: ControlFlow::Break(_),
                ..
            } => None,
            Self::AwaitingProxyBirth { .. } | Self::AbsentProxy { .. } => None,
        }
    }

    pub(in super::super) fn position(&self) -> RosterPosition {
        match self {
            Self::AwaitingProxyBirth { role, .. } | Self::AbsentProxy { role, .. } => {
                role.position()
            }
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ..
            } => proxy.position(),
            Self::Proxy {
                proxy: ControlFlow::Break(proxy),
                ..
            } => proxy.position(),
        }
    }

    pub(in super::super) fn is_unresolved(&self) -> bool {
        match self {
            Self::AwaitingProxyBirth { .. }
            | Self::Proxy {
                proxy: ControlFlow::Continue(_),
                ..
            } => true,
            Self::Proxy {
                proxy: ControlFlow::Break(_),
                ownership:
                    FixedProxyOwnership::Replacement(WorkerReplacement {
                        response:
                            ReplacementResponse::InputPending(_)
                            | ReplacementResponse::OutcomePending(_),
                        ..
                    }),
                ..
            } => true,
            Self::Proxy {
                proxy: ControlFlow::Break(_),
                preparation: Some(_),
                ..
            } => true,
            Self::Proxy {
                proxy: ControlFlow::Break(_),
                preparation: None,
                ..
            }
            | Self::AbsentProxy { .. } => false,
        }
    }

    pub(in super::super) fn awaits_creation(
        &self,
        creation: CreationId,
        kind: CreationKind,
    ) -> bool {
        matches!(
            self,
            Self::AwaitingProxyBirth { operation, .. }
                if operation.creation() == creation && kind == CreationKind::Birth
        )
    }

    pub(in super::super) fn into_awaiting_proxy(
        self,
    ) -> Result<
        (
            MemberRole<Role>,
            ProxyOperationWitness,
            ProxyOperation<behavior::Here, Worker, Plan>,
        ),
        Self,
    > {
        match self {
            Self::AwaitingProxyBirth {
                role,
                witness,
                operation,
            } => Ok((role, witness, operation)),
            member => Err(member),
        }
    }

    pub(in super::super) fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<Self, (Self, ProxyInputResult<behavior::Here, Worker, Plan>)> {
        match self {
            Self::Proxy {
                proxy,
                ownership,
                preparation,
            } => {
                let (proxy, settlement) = match proxy {
                    ControlFlow::Continue(proxy) => match proxy.accept_operation(settlement) {
                        Ok(proxy) => {
                            return Ok(Self::Proxy {
                                proxy,
                                ownership,
                                preparation,
                            });
                        }
                        Err((proxy, settlement)) => (ControlFlow::Continue(proxy), settlement),
                    },
                    proxy @ ControlFlow::Break(_) => (proxy, settlement),
                };
                match ownership.accept_input(settlement) {
                    Ok(ownership) => Ok(Self::Proxy {
                        proxy,
                        ownership,
                        preparation,
                    }),
                    Err((ownership, settlement)) => Err((
                        Self::Proxy {
                            proxy,
                            ownership,
                            preparation,
                        },
                        settlement,
                    )),
                }
            }
            member => Err((member, settlement)),
        }
    }

    pub(in super::super) fn owns_outcome_source(&self, child: CreationId) -> bool {
        match self {
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ..
            } => proxy.accepts_child(child),
            Self::Proxy {
                proxy: ControlFlow::Break(proxy),
                ..
            } => proxy.accepts_child(child),
            Self::AwaitingProxyBirth { .. } | Self::AbsentProxy { .. } => false,
        }
    }

    pub(in super::super) fn accept_outcome(
        self,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    ) -> Result<Self, (Self, ChildReport<ProxyOutcome<Worker, Plan>>)> {
        match self {
            Self::Proxy {
                proxy,
                ownership,
                preparation,
            } => match ownership.accept_outcome(report.report) {
                Ok(ownership) => Ok(Self::Proxy {
                    proxy,
                    ownership,
                    preparation,
                }),
                Err((ownership, outcome)) => Err((
                    Self::Proxy {
                        proxy,
                        ownership,
                        preparation,
                    },
                    ChildReport::new(report.child, outcome),
                )),
            },
            member => Err((member, report)),
        }
    }

    pub(in super::super) fn accepts_stop(
        &self,
        stopped: &ChildStopped<BehaviorAddr<Worker>>,
    ) -> bool {
        matches!(
            self,
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ..
            } if proxy.accepts_stop(stopped)
        )
    }

    pub(in super::super) fn accept_stop(
        self,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Result<Self, (Self, ChildStopped<BehaviorAddr<Worker>>)> {
        match self {
            Self::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ownership,
                preparation,
            } => match proxy.accept_stop(stopped) {
                Ok(proxy) => Ok(Self::Proxy {
                    proxy,
                    ownership,
                    preparation,
                }),
                Err((proxy, stopped)) => Err((
                    Self::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership,
                        preparation,
                    },
                    stopped,
                )),
            },
            Self::Proxy {
                proxy: ControlFlow::Break(proxy),
                ownership,
                preparation,
            } => Err((
                Self::Proxy {
                    proxy: ControlFlow::Break(proxy),
                    ownership,
                    preparation,
                },
                stopped,
            )),
            member => Err((member, stopped)),
        }
    }

    pub(in super::super) fn accept_preparation<Source>(
        self,
        result: ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    ) -> Result<
        (
            Self,
            ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        ),
        (
            Self,
            ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        ),
    >
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Role: Send + Sync,
        Worker: Send,
    {
        match self {
            Self::Proxy {
                proxy,
                ownership,
                preparation: Some(expected),
            } if preparation_result_accepts(&result, &expected) => Ok((
                Self::Proxy {
                    proxy,
                    ownership,
                    preparation: None,
                },
                result,
            )),
            member => Err((member, result)),
        }
    }
}

impl<Worker, Plan> FixedProxyOwnership<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn accept_input(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<Self, (Self, ProxyInputResult<behavior::Here, Worker, Plan>)> {
        match self {
            Self::InitialInputDispatched { witness, prepared } => match witness.admit(settlement) {
                Ok(settlement) => Ok(match settlement {
                    SettledItem::Attempted(ItemSettlement::Accepted(accepted)) => {
                        let (_, _, operation) = accepted.into_parts();
                        Self::InitialOutcomeWaiting {
                            operation,
                            prepared,
                        }
                    }
                    settlement => Self::InitialInputRejected {
                        settlement,
                        prepared,
                    },
                }),
                Err((witness, settlement)) => Err((
                    Self::InitialInputDispatched { witness, prepared },
                    settlement,
                )),
            },
            Self::Replacement(replacement) => match replacement.accept_operation(settlement) {
                Ok(replacement) => Ok(Self::Replacement(replacement)),
                Err((replacement, settlement)) => Err((Self::Replacement(replacement), settlement)),
            },
            ownership => Err((ownership, settlement)),
        }
    }

    fn accept_outcome(
        self,
        outcome: ProxyOutcome<Worker, Plan>,
    ) -> Result<Self, (Self, ProxyOutcome<Worker, Plan>)> {
        match (self, outcome) {
            (
                Self::InitialOutcomeWaiting {
                    operation,
                    prepared,
                },
                outcome @ ProxyOutcome::Initial { .. },
            ) => Ok(Self::InitialOutcomeReceived {
                operation,
                outcome,
                prepared,
            }),
            (Self::Replacement(replacement), ProxyOutcome::Replacement { outcome }) => {
                match replacement.accept_outcome(outcome) {
                    Ok(replacement) => Ok(Self::Replacement(replacement)),
                    Err((replacement, outcome)) => Err((
                        Self::Replacement(replacement),
                        ProxyOutcome::Replacement { outcome },
                    )),
                }
            }
            (Self::Replacement(replacement), ProxyOutcome::WorkerStopped { worker, stopped }) => {
                match replacement.accept_worker_stop(worker, stopped) {
                    Ok(replacement) => Ok(Self::Replacement(replacement)),
                    Err((replacement, worker, stopped)) => Err((
                        Self::Replacement(replacement),
                        ProxyOutcome::WorkerStopped { worker, stopped },
                    )),
                }
            }
            (ownership, outcome) => Err((ownership, outcome)),
        }
    }
}
