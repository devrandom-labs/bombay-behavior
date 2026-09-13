//! Fixed-roster proxy birth, authorization, and owner-input settlement.

use core::ops::ControlFlow;
use std::collections::VecDeque;

use behavior::{
    Behavior, BehaviorAddr, CreationId, CreationKind, CreationSettlement, CreationsSettled,
    EndpointAddress, EstablishedActor, ItemSettlement, SettledItem,
};

use crate::atomic::RoleName;
use crate::atomic::stable_proxy::ProxyOperationWitness;
use crate::{ProxyInputResult, ProxyOperation, ProxyOperationId, StableProxy};

use super::super::member::RosterOwner;
use super::super::protocol::{MemberStatus, UnavailablePhase};
use super::super::role::{MemberRole, RosterPosition};
use super::super::{ActivationPlan, FixedSupervisorError, FixedSupervisorEvent};
use super::{
    FixedRoster, ProxyBirthResolution, ProxyCreationSettlement, proxy_settlement_identity,
    resolve_proxy_creation,
};

pub(in super::super) enum ProxyStartingMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    CreatingProxy {
        role: MemberRole<Role>,
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
    },
    WaitingForAuthorization {
        role: MemberRole<Role>,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
    },
    InputDispatched {
        role: MemberRole<Role>,
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        witness: ProxyOperationWitness,
    },
    AwaitingProxyOutcome {
        role: MemberRole<Role>,
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        operation: ProxyOperationId,
    },
    ProxyInputRejected {
        role: MemberRole<Role>,
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    },
    ProxyBirthRejected {
        role: MemberRole<Role>,
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
        settlement: ProxyCreationSettlement<Worker, Plan>,
    },
}

enum ProxyInputAuthorization<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Issued {
        member: RosterOwner<Role, Worker, Plan>,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
    },
    Continue(RosterOwner<Role, Worker, Plan>),
    Wait(RosterOwner<Role, Worker, Plan>),
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn accept_births<Preparation>(
        self,
        proxies: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    ) -> Result<Self, (Self, FixedSupervisorError<Role, Worker, Plan, Preparation>)> {
        match self {
            Self::Operating(members) => {
                let settlements = match proxies.into_settlement() {
                    CreationSettlement::Settled(settlements) => settlements,
                    settlement => {
                        let roster = Self::Operating(members);
                        let proxies = CreationsSettled::new(settlement);
                        return Err((
                            roster,
                            FixedSupervisorError::ProxyCreationRejected { proxies },
                        ));
                    }
                };
                if settlements.len() != members.len()
                    || !settlements
                        .iter()
                        .zip(&members)
                        .all(|(settlement, member)| {
                            let (creation, kind) = proxy_settlement_identity(settlement);
                            kind == CreationKind::Birth && member.awaits_birth(creation)
                        })
                {
                    let roster = Self::Operating(members);
                    let input = FixedSupervisorEvent::ProxyCreationsSettled(CreationsSettled::new(
                        CreationSettlement::Settled(settlements),
                    ));
                    return Err((roster, FixedSupervisorError::InputRejected { input }));
                }

                let mut remaining = members.into_iter();
                let mut creating = Vec::new();
                while let Some(member) = remaining.next() {
                    match member.into_creating_proxy() {
                        Ok(proxy) => creating.push(proxy),
                        Err(member) => {
                            let mut members = creating
                                .into_iter()
                                .map(|(role, witness, operation)| {
                                    RosterOwner::Starting(ProxyStartingMember::CreatingProxy {
                                        role,
                                        witness,
                                        operation,
                                    })
                                })
                                .collect::<Vec<_>>();
                            members.push(member);
                            members.extend(remaining);
                            let roster = Self::Operating(members);
                            let input = FixedSupervisorEvent::ProxyCreationsSettled(
                                CreationsSettled::new(CreationSettlement::Settled(settlements)),
                            );
                            return Err((roster, FixedSupervisorError::InputRejected { input }));
                        }
                    }
                }
                let members = creating
                    .into_iter()
                    .zip(settlements)
                    .map(|((role, witness, operation), settlement)| {
                        RosterOwner::Starting(ProxyStartingMember::proxy_created(
                            role, witness, operation, settlement,
                        ))
                    })
                    .collect();
                Ok(Self::Operating(members))
            }
            roster => {
                let input = FixedSupervisorEvent::ProxyCreationsSettled(proxies);
                Err((roster, FixedSupervisorError::InputRejected { input }))
            }
        }
    }

    pub(in super::super) fn authorize<Preparation>(
        self,
        maximum: usize,
    ) -> Result<
        (Self, Vec<ProxyOperation<behavior::Here, Worker, Plan>>),
        (Self, FixedSupervisorError<Role, Worker, Plan, Preparation>),
    > {
        match self {
            Self::Operating(mut members) => {
                members.sort_by_key(RosterOwner::first_roster_position);
                let occupied = members.iter().map(RosterOwner::authorization_count).sum();
                let mut available = match maximum.checked_sub(occupied) {
                    Some(available) => available,
                    None => {
                        let roster = Self::Operating(members);
                        return Err((
                            roster,
                            FixedSupervisorError::AuthorizationStateRejected { occupied, maximum },
                        ));
                    }
                };
                let mut remaining = VecDeque::from(members);
                let mut next = Vec::new();
                let mut operations = Vec::new();
                loop {
                    let member = match remaining.pop_front() {
                        Some(member) => member,
                        None => return Ok((Self::Operating(next), operations)),
                    };
                    match available {
                        0 => {
                            next.push(member);
                            next.extend(remaining);
                            return Ok((Self::Operating(next), operations));
                        }
                        _ => match member.authorize() {
                            ProxyInputAuthorization::Issued { member, operation } => {
                                available -= 1;
                                operations.push(operation);
                                remaining.push_front(member);
                            }
                            ProxyInputAuthorization::Continue(member) => next.push(member),
                            ProxyInputAuthorization::Wait(member) => {
                                next.push(member);
                                next.extend(remaining);
                                return Ok((Self::Operating(next), operations));
                            }
                        },
                    }
                }
            }
            roster => Ok((roster, Vec::new())),
        }
    }

    pub(in super::super) fn accept_operation<Preparation>(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        (Self, Option<RoleName<Role>>),
        (Self, FixedSupervisorError<Role, Worker, Plan, Preparation>),
    > {
        match self {
            Self::Operating(mut members) => {
                let mut settlement = settlement;
                for declaration in 0..members.len() {
                    let member = members.swap_remove(declaration);
                    match member.accept_operation(settlement) {
                        Ok((member, retired)) => {
                            members.push(member);
                            let end = members.len() - 1;
                            members.swap(declaration, end);
                            return Ok((Self::Operating(members), retired));
                        }
                        Err((member, returned)) => {
                            members.push(member);
                            let end = members.len() - 1;
                            members.swap(declaration, end);
                            settlement = returned;
                        }
                    }
                }
                let roster = Self::Operating(members);
                let input = FixedSupervisorEvent::ProxyInputSettled(settlement);
                Err((roster, FixedSupervisorError::InputRejected { input }))
            }
            roster => {
                let input = FixedSupervisorEvent::ProxyInputSettled(settlement);
                Err((roster, FixedSupervisorError::InputRejected { input }))
            }
        }
    }
}

impl<Role, Worker, Plan> ProxyStartingMember<Role, Worker, Plan>
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
        let (role, phase) = self.role_phase();
        members.push((role.position(), phase.status()));
    }

    pub(in super::super) fn role_phase(&self) -> (&MemberRole<Role>, UnavailablePhase) {
        let (role, phase) = match self {
            Self::CreatingProxy { role, .. } | Self::ProxyBirthRejected { role, .. } => {
                (role, UnavailablePhase::CreatingProxy)
            }
            Self::WaitingForAuthorization { role, .. } => {
                (role, UnavailablePhase::WaitingForActivation)
            }
            Self::InputDispatched { role, .. }
            | Self::AwaitingProxyOutcome { role, .. }
            | Self::ProxyInputRejected { role, .. } => (role, UnavailablePhase::AwaitingProxy),
        };
        (role, phase)
    }

    pub(in super::super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::WaitingForAuthorization {
                role, operation, ..
            } if operation.creation() == child => Some(role.name()),
            Self::InputDispatched { role, creation, .. }
            | Self::AwaitingProxyOutcome { role, creation, .. }
            | Self::ProxyInputRejected { role, creation, .. }
                if *creation == child =>
            {
                Some(role.name())
            }
            Self::CreatingProxy { .. }
            | Self::WaitingForAuthorization { .. }
            | Self::InputDispatched { .. }
            | Self::AwaitingProxyOutcome { .. }
            | Self::ProxyInputRejected { .. }
            | Self::ProxyBirthRejected { .. } => None,
        }
    }

    pub(in super::super) fn position(&self) -> RosterPosition {
        match self {
            Self::CreatingProxy { role, .. }
            | Self::WaitingForAuthorization { role, .. }
            | Self::InputDispatched { role, .. }
            | Self::AwaitingProxyOutcome { role, .. }
            | Self::ProxyInputRejected { role, .. }
            | Self::ProxyBirthRejected { role, .. } => role.position(),
        }
    }

    fn awaits_birth(&self, creation: CreationId) -> bool {
        matches!(
            self,
            Self::CreatingProxy { operation, .. } if operation.creation() == creation
        )
    }

    fn into_creating_proxy(
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
            Self::CreatingProxy {
                role,
                witness,
                operation,
            } => Ok((role, witness, operation)),
            member => Err(member),
        }
    }

    fn proxy_created(
        role: MemberRole<Role>,
        witness: ProxyOperationWitness,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
        settlement: ProxyCreationSettlement<Worker, Plan>,
    ) -> Self {
        match resolve_proxy_creation(settlement) {
            ProxyBirthResolution::Committed(proxy) => Self::WaitingForAuthorization {
                role,
                proxy,
                witness,
                operation,
            },
            ProxyBirthResolution::Rejected(settlement) => Self::ProxyBirthRejected {
                role,
                witness,
                operation,
                settlement,
            },
        }
    }

    fn authorization_count(&self) -> usize {
        match self {
            Self::InputDispatched { .. } | Self::AwaitingProxyOutcome { .. } => 1,
            Self::CreatingProxy { .. }
            | Self::WaitingForAuthorization { .. }
            | Self::ProxyInputRejected { .. }
            | Self::ProxyBirthRejected { .. } => 0,
        }
    }

    fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<Self, (Self, ProxyInputResult<behavior::Here, Worker, Plan>)> {
        match self {
            Self::InputDispatched {
                role,
                creation,
                proxy,
                witness,
            } => match witness.admit(settlement) {
                Ok(settlement) => Ok(match settlement {
                    SettledItem::Attempted(ItemSettlement::Accepted(accepted)) => {
                        let (creation, proxy, operation) = accepted.into_parts();
                        Self::AwaitingProxyOutcome {
                            role,
                            creation,
                            proxy,
                            operation,
                        }
                    }
                    settlement => Self::ProxyInputRejected {
                        role,
                        creation,
                        proxy,
                        settlement,
                    },
                }),
                Err((witness, settlement)) => Err((
                    Self::InputDispatched {
                        role,
                        creation,
                        proxy,
                        witness,
                    },
                    settlement,
                )),
            },
            member => Err((member, settlement)),
        }
    }

    fn authorize(self) -> ProxyInputAuthorization<Role, Worker, Plan> {
        match self {
            Self::WaitingForAuthorization {
                role,
                proxy,
                witness,
                operation,
            } => {
                let creation = operation.creation();
                ProxyInputAuthorization::Issued {
                    member: RosterOwner::Starting(Self::InputDispatched {
                        role,
                        creation,
                        proxy,
                        witness,
                    }),
                    operation,
                }
            }
            member @ Self::CreatingProxy { .. } | member @ Self::ProxyBirthRejected { .. } => {
                ProxyInputAuthorization::Wait(RosterOwner::Starting(member))
            }
            member @ Self::ProxyInputRejected { .. } => {
                ProxyInputAuthorization::Wait(RosterOwner::Starting(member))
            }
            member @ Self::InputDispatched { .. } | member @ Self::AwaitingProxyOutcome { .. } => {
                ProxyInputAuthorization::Continue(RosterOwner::Starting(member))
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
    fn awaits_birth(&self, creation: CreationId) -> bool {
        match self {
            Self::Starting(member) => member.awaits_birth(creation),
            Self::Stopping(_)
            | Self::Online(_)
            | Self::Empty(_)
            | Self::Recovery(_)
            | Self::Unrecovered(_)
            | Self::Retired(_) => false,
        }
    }

    fn into_creating_proxy(
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
            Self::Starting(member) => member.into_creating_proxy().map_err(Self::Starting),
            member => Err(member),
        }
    }

    pub(in super::super) fn authorization_count(&self) -> usize {
        match self {
            Self::Starting(member) => member.authorization_count(),
            Self::Recovery(recovery) => recovery.authorization_count(),
            Self::Stopping(_)
            | Self::Online(_)
            | Self::Empty(_)
            | Self::Unrecovered(_)
            | Self::Retired(_) => 0,
        }
    }

    fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        (Self, Option<RoleName<Role>>),
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        match self {
            Self::Starting(member) => match member.accept_operation(settlement) {
                Ok(member) => Ok((Self::Starting(member), None)),
                Err((member, settlement)) => Err((Self::Starting(member), settlement)),
            },
            Self::Stopping(member) => match member.accept_operation(settlement) {
                Ok(ControlFlow::Continue(stopping)) => Ok((Self::Stopping(stopping), None)),
                Ok(ControlFlow::Break(retired)) => {
                    let role = retired.role_name();
                    Ok((Self::Retired(retired), Some(role)))
                }
                Err((member, settlement)) => Err((Self::Stopping(member), settlement)),
            },
            Self::Unrecovered(member) => match member.accept_operation(settlement) {
                Ok(ControlFlow::Continue(member)) => Ok((Self::Unrecovered(member), None)),
                Ok(ControlFlow::Break(retired)) => {
                    let role = retired.role_name();
                    Ok((Self::Retired(retired), Some(role)))
                }
                Err((member, settlement)) => Err((Self::Unrecovered(member), settlement)),
            },
            member @ Self::Recovery(_)
            | member @ Self::Online(_)
            | member @ Self::Empty(_)
            | member @ Self::Retired(_) => Err((member, settlement)),
        }
    }

    fn authorize(self) -> ProxyInputAuthorization<Role, Worker, Plan> {
        match self {
            Self::Starting(member) => member.authorize(),
            Self::Recovery(recovery) => {
                let (recovery, operation) = recovery.authorize();
                match operation {
                    Some(operation) => ProxyInputAuthorization::Issued {
                        member: Self::Recovery(recovery),
                        operation,
                    },
                    None => ProxyInputAuthorization::Continue(Self::Recovery(recovery)),
                }
            }
            member @ Self::Stopping(_)
            | member @ Self::Online(_)
            | member @ Self::Empty(_)
            | member @ Self::Unrecovered(_)
            | member @ Self::Retired(_) => ProxyInputAuthorization::Continue(member),
        }
    }
}
