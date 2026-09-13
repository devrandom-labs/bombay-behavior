//! Fixed-roster shutdown and retained member ownership.

mod member;

use core::ops::ControlFlow;
use std::collections::VecDeque;

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, ChildReport, CreationId, CreationSettlement,
    CreationsSettled, EndpointAddress, EstablishedRecipient, Step,
};

use crate::atomic::{PrepareWorkers, RoleName, WorkerSource};
use crate::{
    ChildStopped, ProxyInputResult, ProxyOperation, ProxyOutcome, ScheduleAfter, StableProxy,
};

use super::super::drain::ShutdownDeadline;
use super::super::schedule::ScheduleKey;
use super::member::RosterOwner;
use super::protocol::{MemberStatus, UnavailablePhase};
use super::proxy::{
    FixedRoster, ProxyBirthResolution, ProxyStartingMember, ProxyStoppingMember,
    proxy_settlement_identity, resolve_proxy_creation,
};
use super::recovery::{EmptyMember, StoppedMember, UnissuedReplacements};
use super::recovery::{
    PreparedParticipant, RecoveryBatch, RecoveryMember, RecoveryParticipant, RecoveryRetirement,
    RecoveryRosterOwner, RestartReleaseState,
};
use super::role::RosterPosition;
use super::{ActivationPlan, ActorDrainPolicy};
pub(super) use member::{FixedProxyOwnership, FixedShutdownMember};

pub(super) struct FixedShutdown<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    members: Vec<FixedShutdownMember<Role, Worker, Plan>>,
    unrouted_proxies: Option<CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>>,
    cancelled_recoveries: Vec<CancelledRecovery>,
    deadline: ShutdownDeadline,
}

impl<Role, Worker, Plan> FixedShutdown<Role, Worker, Plan>
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
        for member in &self.members {
            member.append_status(members);
        }
    }

    pub(super) fn capability(
        &self,
        expected: &Role,
    ) -> Option<Result<EstablishedRecipient<Worker::Protocol>, UnavailablePhase>>
    where
        Role: Eq,
    {
        self.members
            .iter()
            .find_map(|member| member.capability_phase(expected))
            .map(Err)
    }
}

pub(super) enum CancelledRecovery {
    Awaiting(ScheduleKey),
    Returned(
        #[expect(dead_code, reason = "retained by the stopped supervisor")]
        ActionItemResult<ScheduleAfter>,
    ),
}

impl CancelledRecovery {
    fn accept_schedule(
        self,
        input: ActionItemResult<ScheduleAfter>,
    ) -> Result<Self, (Self, ActionItemResult<ScheduleAfter>)> {
        let Self::Awaiting(timer) = self else {
            return Err((self, input));
        };
        match timer.admit_result(input) {
            Ok(input) => Ok(Self::Returned(input)),
            Err(input) => Err((Self::Awaiting(timer), input)),
        }
    }
}

impl<Role, Worker, Plan> RecoveryParticipant<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn begin_shutdown(
        self,
        prepared: Option<crate::WorkerSubmission<Worker, Plan>>,
    ) -> (
        FixedShutdownMember<Role, Worker, Plan>,
        ProxyOperation<behavior::Here, Worker, Plan>,
    ) {
        let (role, creation, proxy, ownership) = match self {
            Self::Stopped(StoppedMember {
                role,
                creation,
                proxy,
                worker,
                readiness,
                stopped,
            }) => (
                role,
                creation,
                proxy,
                FixedProxyOwnership::WorkerStopped {
                    worker,
                    readiness,
                    stopped: Some(stopped),
                    prepared,
                },
            ),
            Self::InitialInputDispatched {
                role,
                creation,
                proxy,
                witness,
            } => (
                role,
                creation,
                proxy,
                FixedProxyOwnership::InitialInputDispatched { witness, prepared },
            ),
            Self::AwaitingInitialOutcome {
                role,
                creation,
                proxy,
                operation,
            } => (
                role,
                creation,
                proxy,
                FixedProxyOwnership::InitialOutcomeWaiting {
                    operation,
                    prepared,
                },
            ),
            Self::Online(super::member::OnlineMember {
                role,
                creation,
                proxy,
                worker,
                readiness,
            }) => (
                role,
                creation,
                proxy,
                FixedProxyOwnership::Ready {
                    worker,
                    readiness,
                    prepared,
                },
            ),
        };
        let (proxy, operation) = ProxyStoppingMember::begin(role, creation, proxy);
        (
            FixedShutdownMember::Proxy {
                proxy: ControlFlow::Continue(proxy),
                ownership,
                preparation: None,
            },
            operation,
        )
    }
}

impl<Role, Worker, Plan> RecoveryMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn begin_shutdown(
        self,
    ) -> (
        FixedShutdownMember<Role, Worker, Plan>,
        ProxyOperation<behavior::Here, Worker, Plan>,
    ) {
        match self {
            Self::Waiting { member, submission } => member.begin_shutdown(Some(submission)),
            Self::Replacing {
                role,
                creation,
                proxy,
                replacement,
            } => {
                let (proxy, operation) = ProxyStoppingMember::begin(role, creation, proxy);
                (
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::Replacement(replacement),
                        preparation: None,
                    },
                    operation,
                )
            }
        }
    }
}

impl<Role, Worker, Plan> RecoveryBatch<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn begin_shutdown(
        self,
    ) -> (
        Vec<FixedShutdownMember<Role, Worker, Plan>>,
        Vec<ProxyOperation<behavior::Here, Worker, Plan>>,
        Option<CancelledRecovery>,
    ) {
        let Self {
            ticket: _,
            trigger: _,
            release,
            members: participants,
        } = self;
        let cancelled = match release {
            RestartReleaseState::Scheduling { timer } => Some(CancelledRecovery::Awaiting(timer)),
            RestartReleaseState::Ready | RestartReleaseState::Waiting { .. } => None,
        };
        let mut members = Vec::new();
        let mut operations = Vec::new();
        for participant in participants {
            let (member, operation) = participant.begin_shutdown();
            members.push(member);
            operations.push(operation);
        }
        (members, operations, cancelled)
    }
}

impl<Role, Worker, Plan> PreparedParticipant<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn begin_shutdown(
        self,
    ) -> (
        FixedShutdownMember<Role, Worker, Plan>,
        ProxyOperation<behavior::Here, Worker, Plan>,
    ) {
        self.member.begin_shutdown(Some(self.submission))
    }
}

impl<Role, Worker, Plan> FixedShutdown<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn begin_with_unissued_replacements(
        owners: Vec<RosterOwner<Role, Worker, Plan>>,
        replacements: UnissuedReplacements<Role, Worker, Plan>,
        actor_drain: ActorDrainPolicy,
    ) -> (
        Self,
        Vec<(RosterPosition, ProxyOperation<behavior::Here, Worker, Plan>)>,
        Option<ScheduleAfter>,
    ) {
        let (mut shutdown, mut operations, schedule) = Self::begin(owners, actor_drain);
        let UnissuedReplacements {
            peers,
            trigger,
            trigger_submission,
        } = replacements;
        for participant in peers {
            let (member, operation) = participant.begin_shutdown();
            operations.push((member.position(), operation));
            shutdown.members.push(member);
        }
        let (proxy, ownership, operation) = trigger.retire_prepared_worker(trigger_submission);
        let member = FixedShutdownMember::Proxy {
            proxy: ControlFlow::Continue(proxy),
            ownership,
            preparation: None,
        };
        operations.push((member.position(), operation));
        shutdown.members.push(member);
        shutdown.members.sort_by_key(FixedShutdownMember::position);
        operations.sort_by_key(|(position, _)| *position);
        (shutdown, operations, schedule)
    }

    pub(super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        self.members
            .iter()
            .find_map(|member| member.live_proxy_role(child))
    }

    pub(super) fn begin(
        members: Vec<RosterOwner<Role, Worker, Plan>>,
        actor_drain: ActorDrainPolicy,
    ) -> (
        Self,
        Vec<(RosterPosition, ProxyOperation<behavior::Here, Worker, Plan>)>,
        Option<ScheduleAfter>,
    ) {
        let mut operations = Vec::new();
        let mut remaining = members
            .into_iter()
            .map(|member| (member, None))
            .collect::<VecDeque<_>>();
        let mut members = Vec::new();
        let mut cancelled_recoveries = Vec::new();
        while let Some((member, preparation)) = remaining.pop_front() {
            let member = match member {
                RosterOwner::Starting(ProxyStartingMember::WaitingForAuthorization {
                    role,
                    proxy,
                    witness,
                    operation,
                }) => {
                    let creation = operation.creation();
                    let position = role.position();
                    let (proxy, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                    operations.push((position, shutdown));
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::InitialInputCancelled {
                            witness,
                            operation,
                        },
                        preparation,
                    }
                }
                RosterOwner::Starting(ProxyStartingMember::InputDispatched {
                    role,
                    creation,
                    proxy,
                    witness,
                }) => {
                    let position = role.position();
                    let (proxy, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                    operations.push((position, shutdown));
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::InitialInputDispatched {
                            witness,
                            prepared: None,
                        },
                        preparation,
                    }
                }
                RosterOwner::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                    role,
                    creation,
                    proxy,
                    operation,
                }) => {
                    let position = role.position();
                    let (proxy, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                    operations.push((position, shutdown));
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::InitialOutcomeWaiting {
                            operation,
                            prepared: None,
                        },
                        preparation,
                    }
                }
                RosterOwner::Starting(ProxyStartingMember::ProxyInputRejected {
                    role,
                    creation,
                    proxy,
                    settlement,
                }) => {
                    let position = role.position();
                    let (proxy, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                    operations.push((position, shutdown));
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::InitialInputRejected {
                            settlement,
                            prepared: None,
                        },
                        preparation,
                    }
                }
                RosterOwner::Starting(ProxyStartingMember::ProxyBirthRejected {
                    role,
                    witness,
                    operation,
                    settlement,
                }) => FixedShutdownMember::AbsentProxy {
                    role,
                    witness,
                    operation,
                    settlement: Some(settlement),
                },
                RosterOwner::Online(super::member::OnlineMember {
                    role,
                    creation,
                    proxy,
                    worker,
                    readiness,
                }) => {
                    let position = role.position();
                    let (proxy, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                    operations.push((position, shutdown));
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::Ready {
                            worker,
                            readiness,
                            prepared: None,
                        },
                        preparation,
                    }
                }
                RosterOwner::Empty(EmptyMember {
                    role,
                    creation,
                    proxy,
                    worker,
                    readiness,
                    stopped,
                }) => {
                    let position = role.position();
                    let (proxy, shutdown) = ProxyStoppingMember::begin(role, creation, proxy);
                    operations.push((position, shutdown));
                    FixedShutdownMember::Proxy {
                        proxy: ControlFlow::Continue(proxy),
                        ownership: FixedProxyOwnership::WorkerStopped {
                            worker,
                            readiness,
                            stopped,
                            prepared: None,
                        },
                        preparation,
                    }
                }
                RosterOwner::Recovery(RecoveryRosterOwner::Preparing(recovery)) => {
                    let (expected, before_trigger, trigger, after_trigger) =
                        recovery.into_shutdown();
                    for participant in after_trigger.into_iter().rev() {
                        remaining.push_front((participant.release(), None));
                    }
                    remaining
                        .push_front((RosterOwner::Empty(trigger.retain_stop()), Some(expected)));
                    for participant in before_trigger.into_iter().rev() {
                        remaining.push_front((participant.release(), None));
                    }
                    continue;
                }
                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)) => {
                    let (recovery_members, recovery_operations, cancelled) =
                        recovery.begin_shutdown();
                    for (member, operation) in recovery_members.into_iter().zip(recovery_operations)
                    {
                        let position = member.position();
                        members.push(member);
                        operations.push((position, operation));
                    }
                    match cancelled {
                        Some(cancelled) => cancelled_recoveries.push(cancelled),
                        None => {}
                    }
                    continue;
                }
                RosterOwner::Unrecovered(member) => member.into_shutdown(),
                RosterOwner::Stopping(proxy) => FixedShutdownMember::Proxy {
                    proxy: ControlFlow::Continue(proxy),
                    ownership: FixedProxyOwnership::Unavailable,
                    preparation,
                },
                RosterOwner::Retired(proxy) => FixedShutdownMember::Proxy {
                    proxy: ControlFlow::Break(proxy),
                    ownership: FixedProxyOwnership::Unavailable,
                    preparation,
                },
                RosterOwner::Starting(ProxyStartingMember::CreatingProxy {
                    role,
                    witness,
                    operation,
                }) => FixedShutdownMember::AwaitingProxyBirth {
                    role,
                    witness,
                    operation,
                },
            };
            members.push(member);
        }
        members.sort_by_key(FixedShutdownMember::position);
        operations.sort_by_key(|(position, _)| *position);
        let (deadline, schedule) = ShutdownDeadline::begin(actor_drain);
        (
            Self {
                members,
                unrouted_proxies: None,
                cancelled_recoveries,
                deadline,
            },
            operations,
            schedule,
        )
    }

    pub(super) fn into_step(
        self,
        recovery: RecoveryRetirement,
    ) -> (Self, Step<behavior::Never, behavior::Stopped>) {
        match &self.deadline {
            ShutdownDeadline::NotScheduled(_) | ShutdownDeadline::Elapsed(_) => {
                (self, Step::Stop(behavior::Stopped))
            }
            ShutdownDeadline::Unlimited
            | ShutdownDeadline::Scheduling(_)
            | ShutdownDeadline::Waiting(_) => match self
                .cancelled_recoveries
                .iter()
                .find(|recovery| matches!(recovery, CancelledRecovery::Awaiting(_)))
            {
                Some(_) => (self, Step::Continue),
                None => match self.members.iter().find(|member| member.is_unresolved()) {
                    Some(_) => (self, Step::Continue),
                    None => match recovery {
                        RecoveryRetirement::Ready => (self, Step::Stop(behavior::Stopped)),
                        RecoveryRetirement::WaitingForWorkerSource => (self, Step::Continue),
                    },
                },
            },
        }
    }

    pub(super) fn accept_schedule(
        self,
        input: ActionItemResult<ScheduleAfter>,
    ) -> Result<Self, (Self, ActionItemResult<ScheduleAfter>)> {
        let Self {
            members,
            unrouted_proxies,
            cancelled_recoveries,
            mut deadline,
        } = self;
        let mut remaining_recoveries = cancelled_recoveries.into_iter();
        let mut cancelled_recoveries = Vec::with_capacity(remaining_recoveries.len());
        let mut input = input;
        while let Some(recovery) = remaining_recoveries.next() {
            match recovery.accept_schedule(input) {
                Ok(recovery) => {
                    cancelled_recoveries.push(recovery);
                    cancelled_recoveries.extend(remaining_recoveries);
                    return Ok(Self {
                        members,
                        unrouted_proxies,
                        cancelled_recoveries,
                        deadline,
                    });
                }
                Err((recovery, returned)) => {
                    cancelled_recoveries.push(recovery);
                    input = returned;
                }
            }
        }
        match deadline.accept_schedule(input) {
            Ok(()) => Ok(Self {
                members,
                unrouted_proxies,
                cancelled_recoveries,
                deadline,
            }),
            Err(input) => Err((
                Self {
                    members,
                    unrouted_proxies,
                    cancelled_recoveries,
                    deadline,
                },
                input,
            )),
        }
    }

    pub(super) fn accept_elapsed(
        self,
        elapsed: crate::TimerElapsed,
    ) -> Result<Self, (Self, crate::TimerElapsed)> {
        let Self {
            members,
            unrouted_proxies,
            cancelled_recoveries,
            mut deadline,
        } = self;
        match deadline.accept_elapsed(elapsed) {
            Ok(()) => Ok(Self {
                members,
                unrouted_proxies,
                cancelled_recoveries,
                deadline,
            }),
            Err(elapsed) => Err((
                Self {
                    members,
                    unrouted_proxies,
                    cancelled_recoveries,
                    deadline,
                },
                elapsed,
            )),
        }
    }

    pub(super) fn accept_preparation<Source>(
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
        let Self {
            members,
            unrouted_proxies,
            cancelled_recoveries,
            deadline,
        } = self;
        let mut remaining_members = members.into_iter();
        let mut members = Vec::with_capacity(remaining_members.len());
        let mut result = result;
        while let Some(member) = remaining_members.next() {
            match member.accept_preparation(result) {
                Ok((member, result)) => {
                    members.push(member);
                    members.extend(remaining_members);
                    return Ok((
                        Self {
                            members,
                            unrouted_proxies,
                            cancelled_recoveries,
                            deadline,
                        },
                        result,
                    ));
                }
                Err((member, returned)) => {
                    members.push(member);
                    result = returned;
                }
            }
        }
        Err((
            Self {
                members,
                unrouted_proxies,
                cancelled_recoveries,
                deadline,
            },
            result,
        ))
    }

    pub(super) fn accept_births(
        self,
        proxies: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    ) -> Result<
        (Self, Vec<ProxyOperation<behavior::Here, Worker, Plan>>),
        (
            Self,
            CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
        ),
    > {
        let Self {
            members,
            unrouted_proxies,
            cancelled_recoveries,
            deadline,
        } = self;
        if unrouted_proxies.is_some() {
            return Err((
                Self {
                    members,
                    unrouted_proxies,
                    cancelled_recoveries,
                    deadline,
                },
                proxies,
            ));
        }

        let settlement = proxies.into_settlement();
        match &settlement {
            CreationSettlement::Settled(settlements)
                if settlements.len() == members.len()
                    && settlements
                        .iter()
                        .zip(&members)
                        .all(|(settlement, member)| {
                            let (creation, kind) = proxy_settlement_identity(settlement);
                            member.awaits_creation(creation, kind)
                        }) => {}
            CreationSettlement::Rejected { creations, .. }
            | CreationSettlement::Corrupt { creations, .. }
                if creations.len() == members.len()
                    && creations.iter().zip(&members).all(|(creation, member)| {
                        member.awaits_creation(creation.id(), creation.kind())
                    }) => {}
            _ => {
                return Err((
                    Self {
                        members,
                        unrouted_proxies: None,
                        cancelled_recoveries,
                        deadline,
                    },
                    CreationsSettled::new(settlement),
                ));
            }
        }

        let mut remaining = members.into_iter();
        let mut awaiting = Vec::new();
        while let Some(member) = remaining.next() {
            match member.into_awaiting_proxy() {
                Ok(member) => awaiting.push(member),
                Err(member) => {
                    let mut members = awaiting
                        .into_iter()
                        .map(
                            |(role, witness, operation)| FixedShutdownMember::AwaitingProxyBirth {
                                role,
                                witness,
                                operation,
                            },
                        )
                        .collect::<Vec<_>>();
                    members.push(member);
                    members.extend(remaining);
                    return Err((
                        Self {
                            members,
                            unrouted_proxies: None,
                            cancelled_recoveries,
                            deadline,
                        },
                        CreationsSettled::new(settlement),
                    ));
                }
            }
        }

        let mut members = Vec::with_capacity(awaiting.len());
        let mut operations = Vec::new();
        let unrouted_proxies = match settlement {
            CreationSettlement::Settled(settlements) => {
                for ((role, witness, operation), settlement) in
                    awaiting.into_iter().zip(settlements)
                {
                    match resolve_proxy_creation(settlement) {
                        ProxyBirthResolution::Committed(proxy) => {
                            let creation = operation.creation();
                            let (proxy, shutdown) =
                                ProxyStoppingMember::begin(role, creation, proxy);
                            members.push(FixedShutdownMember::Proxy {
                                proxy: ControlFlow::Continue(proxy),
                                ownership: FixedProxyOwnership::InitialInputCancelled {
                                    witness,
                                    operation,
                                },
                                preparation: None,
                            });
                            operations.push(shutdown);
                        }
                        ProxyBirthResolution::Rejected(settlement) => {
                            members.push(FixedShutdownMember::AbsentProxy {
                                role,
                                witness,
                                operation,
                                settlement: Some(settlement),
                            });
                        }
                    }
                }
                None
            }
            CreationSettlement::Rejected { creations, reason } => {
                for (role, witness, operation) in awaiting {
                    members.push(FixedShutdownMember::AbsentProxy {
                        role,
                        witness,
                        operation,
                        settlement: None,
                    });
                }
                Some(CreationsSettled::new(CreationSettlement::Rejected {
                    creations,
                    reason,
                }))
            }
            CreationSettlement::Corrupt { creations, fault } => {
                for (role, witness, operation) in awaiting {
                    members.push(FixedShutdownMember::AbsentProxy {
                        role,
                        witness,
                        operation,
                        settlement: None,
                    });
                }
                Some(CreationsSettled::new(CreationSettlement::Corrupt {
                    creations,
                    fault,
                }))
            }
        };
        Ok((
            Self {
                members,
                unrouted_proxies,
                cancelled_recoveries,
                deadline,
            },
            operations,
        ))
    }

    pub(super) fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<Self, (Self, ProxyInputResult<behavior::Here, Worker, Plan>)> {
        let Self {
            members,
            unrouted_proxies,
            cancelled_recoveries,
            deadline,
        } = self;
        let mut remaining_members = members.into_iter();
        let mut members = Vec::with_capacity(remaining_members.len());
        let mut settlement = settlement;
        while let Some(member) = remaining_members.next() {
            match member.accept_operation(settlement) {
                Ok(member) => {
                    members.push(member);
                    members.extend(remaining_members);
                    return Ok(Self {
                        members,
                        unrouted_proxies,
                        cancelled_recoveries,
                        deadline,
                    });
                }
                Err((member, returned)) => {
                    members.push(member);
                    settlement = returned;
                }
            }
        }
        Err((
            Self {
                members,
                unrouted_proxies,
                cancelled_recoveries,
                deadline,
            },
            settlement,
        ))
    }

    pub(super) fn accept_outcome(
        mut self,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    ) -> Result<Self, (Self, ChildReport<ProxyOutcome<Worker, Plan>>)> {
        let declaration = match self
            .members
            .iter()
            .position(|member| member.owns_outcome_source(report.child))
        {
            Some(declaration) => declaration,
            None => return Err((self, report)),
        };
        let member = self.members.remove(declaration);
        match member.accept_outcome(report) {
            Ok(member) => {
                self.members.insert(declaration, member);
                Ok(self)
            }
            Err((member, report)) => {
                self.members.insert(declaration, member);
                Err((self, report))
            }
        }
    }

    pub(super) fn accept_stop(
        mut self,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Result<Self, (Self, ChildStopped<BehaviorAddr<Worker>>)> {
        let declaration = match self
            .members
            .iter()
            .position(|member| member.accepts_stop(&stopped))
        {
            Some(declaration) => declaration,
            None => return Err((self, stopped)),
        };
        let member = self.members.remove(declaration);
        match member.accept_stop(stopped) {
            Ok(member) => {
                self.members.insert(declaration, member);
                Ok(self)
            }
            Err((member, stopped)) => {
                self.members.insert(declaration, member);
                Err((self, stopped))
            }
        }
    }
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn begin_shutdown(
        self,
        actor_drain: ActorDrainPolicy,
    ) -> Result<
        (
            FixedShutdown<Role, Worker, Plan>,
            Vec<ProxyOperation<behavior::Here, Worker, Plan>>,
            Option<ScheduleAfter>,
        ),
        Self,
    > {
        match self {
            Self::Operating(members) => {
                let (shutdown, operations, schedule) = FixedShutdown::begin(members, actor_drain);
                Ok((
                    shutdown,
                    operations
                        .into_iter()
                        .map(|(_, operation)| operation)
                        .collect(),
                    schedule,
                ))
            }
            Self::ShuttingDown(shutdown) => Ok((shutdown, Vec::new(), None)),
            roster => Err(roster),
        }
    }
}
