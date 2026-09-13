//! Fixed-roster proxy ownership during initialization.

mod outcome;
mod start;
mod stop;

use behavior::{
    Behavior, BehaviorAddr, CreateChild, CreationSequence, Creations, EndpointAddress,
    EstablishedRecipient,
};

use crate::atomic::RoleName;
use crate::{ObserveChild, ProxyOperation, StableProxy};

use super::member::RosterOwner;
use super::protocol::{CapabilityResult, FixedSnapshot, UnavailablePhase};
use super::role::{MemberRole, RosterPosition};
use super::shutdown::FixedShutdown;
use super::{ActivationPlan, FixedSupervisorError, PreparedWorker};
pub(super) use crate::atomic::proxy_creation::{
    StableProxyCreation as ProxyBirthResolution,
    StableProxyCreationSettlement as ProxyCreationSettlement,
    identity as proxy_settlement_identity, resolve as resolve_proxy_creation,
};
pub(super) use outcome::InitialProxyDecision;
pub(super) use start::ProxyStartingMember;
pub(super) use stop::{ProxyStoppingMember, RetiredProxyMember};

pub(super) enum FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    New(Vec<PreparedWorker<Role, Worker, Plan>>),
    Operating(Vec<RosterOwner<Role, Worker, Plan>>),
    ShuttingDown(FixedShutdown<Role, Worker, Plan>),
    Terminating {
        #[expect(dead_code, reason = "retained by the stopped supervisor")]
        owners: Vec<RosterOwner<Role, Worker, Plan>>,
        #[expect(dead_code, reason = "retained by the stopped supervisor")]
        prepared: Vec<crate::WorkerSubmission<Worker, Plan>>,
    },
    Stopped,
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn snapshot(&self) -> Option<FixedSnapshot<Worker::Protocol>> {
        let mut members = Vec::new();
        match self {
            Self::Operating(owners) => {
                for owner in owners {
                    owner.append_status(&mut members);
                }
            }
            Self::ShuttingDown(shutdown) => shutdown.append_status(&mut members),
            Self::New(_) | Self::Terminating { .. } | Self::Stopped => return None,
        }
        members.sort_by_key(|(position, _)| *position);
        Some(FixedSnapshot {
            members: members.into_iter().map(|(_, status)| status).collect(),
        })
    }

    pub(super) fn capability(
        &self,
        role: Role,
    ) -> Result<CapabilityResult<Role, Worker::Protocol>, Role>
    where
        Role: Eq,
    {
        let current: Option<Result<EstablishedRecipient<Worker::Protocol>, UnavailablePhase>> =
            match self {
                Self::Operating(owners) => owners.iter().find_map(|owner| owner.capability(&role)),
                Self::ShuttingDown(shutdown) => shutdown.capability(&role),
                Self::New(_) | Self::Terminating { .. } | Self::Stopped => return Err(role),
            };
        Ok(match current {
            Some(Ok(proxy)) => CapabilityResult::Ready { role, proxy },
            Some(Err(phase)) => CapabilityResult::Unavailable { role, phase },
            None => CapabilityResult::UnknownRole { submitted: role },
        })
    }

    pub(super) fn live_proxy_role(&self, child: behavior::CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::Operating(owners) => owners.iter().find_map(|owner| owner.live_proxy_role(child)),
            Self::ShuttingDown(shutdown) => shutdown.live_proxy_role(child),
            Self::New(_) | Self::Terminating { .. } | Self::Stopped => None,
        }
    }

    pub(super) const fn new(members: Vec<PreparedWorker<Role, Worker, Plan>>) -> Self {
        Self::New(members)
    }

    pub(super) fn begin<Preparation>(
        self,
        creations: &mut CreationSequence,
    ) -> Result<
        (
            Self,
            Vec<ObserveChild<Worker::Protocol, behavior::ChildHead>>,
            Creations<CreateChild<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>>,
        ),
        (Self, FixedSupervisorError<Role, Worker, Plan, Preparation>),
    > {
        match self {
            Self::New(members) => {
                let ids: Option<Vec<_>> = (0..members.len()).map(|_| creations.issue()).collect();
                let Some(ids) = ids else {
                    return Err((
                        Self::Stopped,
                        FixedSupervisorError::ProxyCreationsExhausted { members },
                    ));
                };
                let mut observations = Vec::with_capacity(members.len());
                let mut proxies = Creations::empty();
                let operating = members
                    .into_iter()
                    .zip(ids)
                    .enumerate()
                    .map(|(position, (member, creation))| {
                        observations.push(ObserveChild::new(creation));
                        proxies.extend([CreateChild::birth(
                            creation,
                            StableProxy::<Worker, Plan>::activated(),
                        )]);
                        let (witness, operation) =
                            ProxyOperation::initial(creation, member.submission);
                        RosterOwner::starting(ProxyStartingMember::CreatingProxy {
                            role: MemberRole::declared(RosterPosition::new(position), member.role),
                            witness,
                            operation,
                        })
                    })
                    .collect();
                Ok((Self::Operating(operating), observations, proxies))
            }
            roster => Err((roster, FixedSupervisorError::InitializationUnavailable)),
        }
    }
}
