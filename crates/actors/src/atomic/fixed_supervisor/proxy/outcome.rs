//! Exact StableProxy outcome correlation for fixed-roster members.

use behavior::{Behavior, BehaviorAddr, ChildReport, EndpointAddress};

use crate::atomic::RoleName;
use crate::{InitialWorkerOutcome, ProxyOutcome, StableProxy, WorkerStartResult};

use super::super::member::{OnlineMember, RosterOwner};
use super::super::{ActivationPlan, FixedSupervisorError, FixedSupervisorEvent};
use super::FixedRoster;
use super::start::ProxyStartingMember;
use super::stop::ProxyStoppingMember;

pub(in super::super) enum InitialProxyDecision<Role, Worker, Plan, Preparation>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Ready {
        owners: Vec<RosterOwner<Role, Worker, Plan>>,
        member: OnlineMember<Role, Worker, Plan>,
    },
    Failed {
        owners: Vec<RosterOwner<Role, Worker, Plan>>,
        child: behavior::CreationId,
        role: RoleName<Role>,
        outcome: ProxyOutcome<Worker, Plan>,
    },
    Rejected {
        roster: FixedRoster<Role, Worker, Plan>,
        rejection: FixedSupervisorError<Role, Worker, Plan, Preparation>,
    },
}

enum MemberInitialDecision<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Ready(OnlineMember<Role, Worker, Plan>),
    Failed {
        member: RosterOwner<Role, Worker, Plan>,
        role: RoleName<Role>,
        outcome: ProxyOutcome<Worker, Plan>,
    },
    Rejected {
        member: RosterOwner<Role, Worker, Plan>,
        outcome: ProxyOutcome<Worker, Plan>,
    },
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn accept_initial_outcome<Preparation>(
        self,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    ) -> InitialProxyDecision<Role, Worker, Plan, Preparation> {
        match self {
            Self::Operating(mut members) => {
                match members
                    .iter()
                    .position(|member| member.awaits_initial_outcome(report.child))
                {
                    Some(declaration) => {
                        let member = members.remove(declaration);
                        match member.accept_initial_outcome(report.report) {
                            MemberInitialDecision::Ready(member) => InitialProxyDecision::Ready {
                                owners: members,
                                member,
                            },
                            MemberInitialDecision::Failed {
                                member,
                                role,
                                outcome,
                            } => {
                                members.insert(declaration, member);
                                InitialProxyDecision::Failed {
                                    owners: members,
                                    child: report.child,
                                    role,
                                    outcome,
                                }
                            }
                            MemberInitialDecision::Rejected { member, outcome } => {
                                members.insert(declaration, member);
                                let roster = Self::Operating(members);
                                let input = FixedSupervisorEvent::ProxyReported(ChildReport::new(
                                    report.child,
                                    outcome,
                                ));
                                InitialProxyDecision::Rejected {
                                    roster,
                                    rejection: FixedSupervisorError::InputRejected { input },
                                }
                            }
                        }
                    }
                    None => {
                        let roster = Self::Operating(members);
                        let input = FixedSupervisorEvent::ProxyReported(report);
                        InitialProxyDecision::Rejected {
                            roster,
                            rejection: FixedSupervisorError::InputRejected { input },
                        }
                    }
                }
            }
            roster => {
                let input = FixedSupervisorEvent::ProxyReported(report);
                InitialProxyDecision::Rejected {
                    roster,
                    rejection: FixedSupervisorError::InputRejected { input },
                }
            }
        }
    }

    pub(in super::super) fn begin_stop_after_initial_failure(
        self,
        child: behavior::CreationId,
    ) -> Result<(Self, crate::ProxyOperation<behavior::Here, Worker, Plan>), Self> {
        match self {
            Self::Operating(mut members) => {
                let declaration = match members
                    .iter()
                    .position(|member| member.awaits_initial_outcome(child))
                {
                    Some(declaration) => declaration,
                    None => return Err(Self::Operating(members)),
                };
                let member = members.remove(declaration);
                match member.begin_stop_after_initial_failure() {
                    Ok((member, shutdown)) => {
                        members.insert(declaration, member);
                        Ok((Self::Operating(members), shutdown))
                    }
                    Err(member) => {
                        members.insert(declaration, member);
                        Err(Self::Operating(members))
                    }
                }
            }
            roster => Err(roster),
        }
    }
}

impl<Role, Worker, Plan> RosterOwner<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn awaits_initial_outcome(&self, child: behavior::CreationId) -> bool {
        matches!(
            self,
            Self::Starting(ProxyStartingMember::AwaitingProxyOutcome { creation, .. })
                if *creation == child
        )
    }

    fn accept_initial_outcome(
        self,
        outcome: ProxyOutcome<Worker, Plan>,
    ) -> MemberInitialDecision<Role, Worker, Plan> {
        match self {
            Self::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                role,
                creation,
                proxy,
                operation,
            }) => match outcome {
                ProxyOutcome::Initial {
                    outcome:
                        InitialWorkerOutcome::Resolved {
                            result: WorkerStartResult::Ready { attempt, readiness },
                        },
                } => {
                    drop(operation);
                    MemberInitialDecision::Ready(OnlineMember {
                        role,
                        creation,
                        proxy,
                        worker: attempt,
                        readiness,
                    })
                }
                outcome @ ProxyOutcome::Initial { .. } => {
                    let diagnostic_role = role.name();
                    MemberInitialDecision::Failed {
                        member: Self::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                            role,
                            creation,
                            proxy,
                            operation,
                        }),
                        role: diagnostic_role,
                        outcome,
                    }
                }
                outcome => MemberInitialDecision::Rejected {
                    member: Self::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                        role,
                        creation,
                        proxy,
                        operation,
                    }),
                    outcome,
                },
            },
            member => MemberInitialDecision::Rejected { member, outcome },
        }
    }

    fn begin_stop_after_initial_failure(
        self,
    ) -> Result<(Self, crate::ProxyOperation<behavior::Here, Worker, Plan>), Self> {
        match self {
            Self::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                role,
                creation,
                proxy,
                operation,
            }) => {
                drop(operation);
                let (stopping, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                Ok((Self::Stopping(stopping), shutdown))
            }
            member => Err(member),
        }
    }
}
