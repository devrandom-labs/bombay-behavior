//! One fixed-roster member and the values valid in its current phase.

use behavior::{
    Behavior, BehaviorAddr, CreationId, EndpointAddress, EstablishedActor, EstablishedRecipient,
};

use crate::atomic::RoleName;
use crate::{StableProxy, WorkerAttempt};

use super::ActivationPlan;
use super::protocol::{MemberStatus, UnavailablePhase};
use super::proxy::{ProxyStartingMember, ProxyStoppingMember, RetiredProxyMember};
use super::recovery::{EmptyMember, RecoveryRosterOwner, UnrecoveredMember};
use super::role::{MemberRole, RosterPosition};

pub(super) struct OnlineMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) role: MemberRole<Role>,
    pub(super) creation: CreationId,
    pub(super) proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    pub(super) worker: WorkerAttempt,
    pub(super) readiness: Plan::Ready,
}

pub(super) enum RosterOwner<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Starting(ProxyStartingMember<Role, Worker, Plan>),
    Stopping(ProxyStoppingMember<Role, Worker, Plan>),
    Online(OnlineMember<Role, Worker, Plan>),
    Empty(EmptyMember<Role, Worker, Plan>),
    Recovery(RecoveryRosterOwner<Role, Worker, Plan>),
    Unrecovered(UnrecoveredMember<Role, Worker, Plan>),
    Retired(RetiredProxyMember<Role, Worker, Plan>),
}

impl<Role, Worker, Plan> RosterOwner<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn append_status(
        &self,
        members: &mut Vec<(RosterPosition, MemberStatus<Worker::Protocol>)>,
    ) {
        match self {
            Self::Starting(member) => member.append_status(members),
            Self::Stopping(member) => {
                members.push((member.position(), UnavailablePhase::Stopping.status()))
            }
            Self::Online(member) => members.push((
                member.role.position(),
                MemberStatus::Ready {
                    proxy: member.proxy.recipient(),
                },
            )),
            Self::Empty(member) => {
                members.push((member.position(), UnavailablePhase::Empty.status()));
            }
            Self::Recovery(recovery) => recovery.append_status(members),
            Self::Unrecovered(member) => {
                members.push((member.position(), UnavailablePhase::Stopping.status()))
            }
            Self::Retired(member) => {
                members.push((member.position(), UnavailablePhase::Retired.status()))
            }
        }
    }

    pub(super) fn capability(
        &self,
        expected: &Role,
    ) -> Option<Result<EstablishedRecipient<Worker::Protocol>, UnavailablePhase>>
    where
        Role: Eq,
    {
        let (role, current) = match self {
            Self::Starting(member) => {
                let (role, phase) = member.role_phase();
                (role.role(), Err(phase))
            }
            Self::Stopping(member) => (member.role(), Err(UnavailablePhase::Stopping)),
            Self::Online(member) => (member.role.role(), Ok(member.proxy.recipient())),
            Self::Empty(member) => (member.role(), Err(UnavailablePhase::Empty)),
            Self::Recovery(recovery) => {
                return recovery
                    .find_role(expected)
                    .map(|_| Err(UnavailablePhase::Recovering));
            }
            Self::Unrecovered(member) => (member.role(), Err(UnavailablePhase::Stopping)),
            Self::Retired(member) => (member.role(), Err(UnavailablePhase::Retired)),
        };
        (role == expected).then_some(current)
    }

    pub(super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::Starting(member) => member.live_proxy_role(child),
            Self::Stopping(member) => member.live_proxy_role(child),
            Self::Online(member) if member.creation == child => Some(member.role.name()),
            Self::Empty(member) => member.live_proxy_role(child),
            Self::Recovery(recovery) => recovery.live_proxy_role(child),
            Self::Unrecovered(member) => member.live_proxy_role(child),
            Self::Online(_) | Self::Retired(_) => None,
        }
    }

    pub(super) fn first_roster_position(&self) -> Option<RosterPosition> {
        match self {
            Self::Starting(member) => Some(member.position()),
            Self::Stopping(member) => Some(member.position()),
            Self::Online(member) => Some(member.role.position()),
            Self::Empty(member) => Some(member.role.position()),
            Self::Recovery(recovery) => recovery.first_roster_position(),
            Self::Unrecovered(member) => Some(member.position()),
            Self::Retired(member) => Some(member.position()),
        }
    }

    pub(super) fn last_roster_position(&self) -> Option<RosterPosition> {
        match self {
            Self::Starting(member) => Some(member.position()),
            Self::Stopping(member) => Some(member.position()),
            Self::Online(member) => Some(member.role.position()),
            Self::Empty(member) => Some(member.role.position()),
            Self::Recovery(recovery) => recovery.last_roster_position(),
            Self::Unrecovered(member) => Some(member.position()),
            Self::Retired(member) => Some(member.position()),
        }
    }

    pub(super) const fn starting(member: ProxyStartingMember<Role, Worker, Plan>) -> Self {
        Self::Starting(member)
    }
}
