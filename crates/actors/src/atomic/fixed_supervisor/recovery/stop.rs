//! Exact ready-worker stop admission for one fixed roster owner.

use core::cmp::Ordering;

use behavior::{
    Behavior, BehaviorAddr, ChildReport, CreationId, EndpointAddress, EstablishedActor,
};

use crate::atomic::worker::{StopKind, stop_kind};
use crate::atomic::{PrepareWorkers, RoleName, WorkerSource};
use crate::{ChildStopped, ProxyOutcome, StableProxy, WorkerAttempt};

use super::super::member::{OnlineMember, RosterOwner};
use super::super::proxy::FixedRoster;
use super::super::role::MemberRole;
use super::super::{ActivationPlan, Strategy};
use super::{
    PreparingRecovery, RecoveryParticipant, RecoveryPreparation, RecoveryRosterOwner,
    SupervisorRecovery,
};

pub(in super::super) struct StoppedMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) role: MemberRole<Role>,
    pub(in super::super) creation: CreationId,
    pub(in super::super) proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    pub(in super::super) worker: WorkerAttempt,
    pub(in super::super) readiness: Plan::Ready,
    pub(in super::super) stopped: ChildStopped<BehaviorAddr<Worker>>,
}

pub(in super::super) struct EmptyMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) role: MemberRole<Role>,
    pub(in super::super) creation: CreationId,
    pub(in super::super) proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    pub(in super::super) worker: WorkerAttempt,
    pub(in super::super) readiness: Plan::Ready,
    pub(in super::super) stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
}

impl<Role, Worker, Plan> StoppedMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        (self.creation == child).then(|| self.role.name())
    }

    fn into_empty(
        self,
    ) -> (
        EmptyMember<Role, Worker, Plan>,
        ChildStopped<BehaviorAddr<Worker>>,
    ) {
        let Self {
            role,
            creation,
            proxy,
            worker,
            readiness,
            stopped,
        } = self;
        (
            EmptyMember {
                role,
                creation,
                proxy,
                worker,
                readiness,
                stopped: None,
            },
            stopped,
        )
    }

    pub(in super::super) fn retain_stop(self) -> EmptyMember<Role, Worker, Plan> {
        let (mut member, stopped) = self.into_empty();
        member.stopped = Some(stopped);
        member
    }
}

impl<Role, Worker, Plan> EmptyMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn position(&self) -> super::super::role::RosterPosition {
        self.role.position()
    }

    pub(in super::super) fn role(&self) -> &Role {
        self.role.role()
    }

    pub(in super::super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        (self.creation == child).then(|| self.role.name())
    }
}

pub(in super::super) struct AcceptedWorkerStop<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    owners: Vec<RosterOwner<Role, Worker, Plan>>,
    member: StoppedMember<Role, Worker, Plan>,
}

pub(in super::super) enum WorkerStopDecision<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Accepted(AcceptedWorkerStop<Role, Worker, Plan>),
    Rejected {
        roster: FixedRoster<Role, Worker, Plan>,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    },
}

fn select_recovery_peers<Role, Worker, Plan>(
    owners: Vec<RosterOwner<Role, Worker, Plan>>,
) -> Result<Vec<RecoveryParticipant<Role, Worker, Plan>>, Vec<RosterOwner<Role, Worker, Plan>>>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let mut remaining = owners.into_iter();
    let mut selected = Vec::new();
    loop {
        let owner = match remaining.next() {
            Some(owner) => owner,
            None => return Ok(selected),
        };
        match RecoveryParticipant::select(owner) {
            Ok(participant) => selected.push(participant),
            Err(owner) => {
                let mut restored = selected
                    .into_iter()
                    .map(RecoveryParticipant::release)
                    .collect::<Vec<_>>();
                restored.push(owner);
                restored.extend(remaining);
                return Err(restored);
            }
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
    fn owns_online_proxy(&self, child: CreationId) -> bool {
        matches!(self, Self::Online(member) if member.creation == child)
    }
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn accept_worker_stop(
        self,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    ) -> WorkerStopDecision<Role, Worker, Plan> {
        let Self::Operating(mut owners) = self else {
            return WorkerStopDecision::Rejected {
                roster: self,
                report,
            };
        };
        let declaration = match owners
            .iter()
            .position(|owner| owner.owns_online_proxy(report.child))
        {
            Some(declaration) => declaration,
            None => {
                return WorkerStopDecision::Rejected {
                    roster: Self::Operating(owners),
                    report,
                };
            }
        };
        let owner = owners.remove(declaration);
        let RosterOwner::Online(OnlineMember {
            role,
            creation,
            proxy,
            worker,
            readiness,
        }) = owner
        else {
            owners.insert(declaration, owner);
            return WorkerStopDecision::Rejected {
                roster: Self::Operating(owners),
                report,
            };
        };
        let ProxyOutcome::WorkerStopped {
            worker: reported_worker,
            stopped,
        } = report.report
        else {
            owners.insert(
                declaration,
                RosterOwner::Online(OnlineMember {
                    role,
                    creation,
                    proxy,
                    worker,
                    readiness,
                }),
            );
            return WorkerStopDecision::Rejected {
                roster: Self::Operating(owners),
                report,
            };
        };
        if reported_worker != worker {
            owners.insert(
                declaration,
                RosterOwner::Online(OnlineMember {
                    role,
                    creation,
                    proxy,
                    worker,
                    readiness,
                }),
            );
            return WorkerStopDecision::Rejected {
                roster: Self::Operating(owners),
                report: ChildReport::new(
                    report.child,
                    ProxyOutcome::WorkerStopped {
                        worker: reported_worker,
                        stopped,
                    },
                ),
            };
        }
        WorkerStopDecision::Accepted(AcceptedWorkerStop {
            owners,
            member: StoppedMember {
                role,
                creation,
                proxy,
                worker,
                readiness,
                stopped,
            },
        })
    }
}

impl<Role, Worker, Plan> AcceptedWorkerStop<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn kind(&self) -> StopKind {
        stop_kind(&self.member.stopped.outcome)
    }

    pub(in super::super) fn discharge_stop(self) -> FixedRoster<Role, Worker, Plan> {
        let mut owners = self.owners;
        let (member, _) = self.member.into_empty();
        owners.push(RosterOwner::Empty(member));
        FixedRoster::Operating(owners)
    }

    pub(in super::super) fn release_stop(
        self,
    ) -> (
        FixedRoster<Role, Worker, Plan>,
        RoleName<Role>,
        ChildStopped<BehaviorAddr<Worker>>,
    ) {
        let mut owners = self.owners;
        let (member, stopped) = self.member.into_empty();
        let role = member.role.name();
        owners.push(RosterOwner::Empty(member));
        (FixedRoster::Operating(owners), role, stopped)
    }

    pub(in super::super) fn reject(
        self,
    ) -> (
        FixedRoster<Role, Worker, Plan>,
        ChildReport<ProxyOutcome<Worker, Plan>>,
    ) {
        let StoppedMember {
            role,
            creation,
            proxy,
            worker,
            readiness,
            stopped,
        } = self.member;
        let child = creation;
        let mut owners = self.owners;
        owners.push(RosterOwner::Online(OnlineMember {
            role,
            creation,
            proxy,
            worker: worker.clone(),
            readiness,
        }));
        (
            FixedRoster::Operating(owners),
            ChildReport::new(child, ProxyOutcome::WorkerStopped { worker, stopped }),
        )
    }

    pub(in super::super) fn begin_recovery<Source, PreparationReturn>(
        self,
        preparation: RecoveryPreparation<Source>,
    ) -> Result<
        (
            SupervisorRecovery<Source, PreparationReturn>,
            FixedRoster<Role, Worker, Plan>,
            PrepareWorkers<Source, Role, Worker, Plan>,
        ),
        (Self, RecoveryPreparation<Source>),
    >
    where
        Role: Send + Sync,
        Worker: Send,
        Source: WorkerSource<Role, Worker, Plan>,
    {
        let strategy = preparation.strategy();
        let Self { mut owners, member } = self;
        let trigger = member.role.position();
        owners.sort_by_key(RosterOwner::last_roster_position);
        let (mut owners, before_trigger, after_trigger) = match strategy {
            Strategy::OneForOne => (owners, Vec::new(), Vec::new()),
            Strategy::OneForAll => {
                let selected = match select_recovery_peers(owners) {
                    Ok(selected) => selected,
                    Err(owners) => {
                        return Err((Self { owners, member }, preparation));
                    }
                };
                let mut before_trigger = Vec::new();
                let mut after_trigger = Vec::new();
                for participant in selected {
                    match participant.position().cmp(&trigger) {
                        Ordering::Less => before_trigger.push(participant),
                        Ordering::Equal | Ordering::Greater => after_trigger.push(participant),
                    }
                }
                (Vec::new(), before_trigger, after_trigger)
            }
            Strategy::RestForOne => {
                let mut before_trigger = Vec::new();
                let mut selected_owners = Vec::new();
                for owner in owners {
                    match owner.last_roster_position() {
                        Some(position) => match position.cmp(&trigger) {
                            Ordering::Less => before_trigger.push(owner),
                            Ordering::Equal | Ordering::Greater => selected_owners.push(owner),
                        },
                        None => selected_owners.push(owner),
                    }
                }
                let after_trigger = match select_recovery_peers(selected_owners) {
                    Ok(selected) => selected,
                    Err(selected_owners) => {
                        before_trigger.extend(selected_owners);
                        return Err((
                            Self {
                                owners: before_trigger,
                                member,
                            },
                            preparation,
                        ));
                    }
                };
                (before_trigger, Vec::new(), after_trigger)
            }
        };
        let (recovery, selected, request) =
            PreparingRecovery::begin(preparation, before_trigger, member, after_trigger);
        owners.push(RosterOwner::Recovery(RecoveryRosterOwner::Preparing(
            selected,
        )));
        Ok((recovery, FixedRoster::Operating(owners), request))
    }
}

#[cfg(test)]
mod compile_contract {
    #[expect(
        dead_code,
        reason = "compile-only proof that the exact stop owns its classification"
    )]
    fn accepted_stop_owns_one_terminal_cause<Role, Worker, Plan>(
        accepted: super::AcceptedWorkerStop<Role, Worker, Plan>,
    ) where
        Worker: behavior::Behavior,
        Plan: crate::ActivationPlan,
        behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
        crate::StableProxy<Worker, Plan>:
            behavior::Behavior<Protocol = <Worker as behavior::Behavior>::Protocol>,
    {
        let super::AcceptedWorkerStop {
            owners: _,
            member: _,
        } = accepted;
    }
}
