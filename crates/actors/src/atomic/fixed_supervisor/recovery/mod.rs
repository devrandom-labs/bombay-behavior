//! Fixed-supervisor recovery policy and affine worker-source ownership.

mod correlation;
mod stop;

use std::collections::VecDeque;
use std::ops::ControlFlow;

use behavior::{
    ActionItemResult, Behavior, BehaviorAddr, ChildReport, CreationId, EndpointAddress,
    ItemSettlement, SettledItem,
};

use crate::atomic::worker::{
    PreparationTicket, StopKind, WorkerPreparationOutcome, preparation_result_accepts,
};
use crate::atomic::{PrepareWorkers, RoleName, WorkerSource};
use crate::{
    ChildInputReason, ChildStopped, ProxyInputResult, ProxyOperation, ProxyOperationId,
    ReplacementOutcome, ScheduleAfter, ScheduleAfterRejection, StableProxy, TimerElapsed,
    WorkerAttempt,
};

use super::diagnostic::{RecoveryDenied, RestartScheduleFailure, WorkerPreparationFailure};
use super::member::{OnlineMember, RosterOwner};
use super::protocol::{MemberStatus, UnavailablePhase};
use super::proxy::{FixedRoster, ProxyStartingMember, ProxyStoppingMember, RetiredProxyMember};
use super::restart::{RecoveryDenialReason, RestartAdmission, RestartBudget, admit_restart};
use super::role::RosterPosition;
use super::shutdown::FixedShutdown;
use super::shutdown::{FixedProxyOwnership, FixedShutdownMember};
use super::{Recovery, RecoveryDecision, RestartLimit, RestartRelease, Strategy};
use correlation::{AcceptedRecoveryCorrelations, RecoveryCorrelations, RecoveryTimer};

use super::super::schedule::ScheduleKey;
pub(super) use correlation::RecoveryTicket;
pub(super) use stop::{AcceptedWorkerStop, EmptyMember, StoppedMember, WorkerStopDecision};

#[derive(Clone, Copy)]
enum AutomaticEligibility {
    Permanent,
    Transient,
}

#[derive(Clone, Copy)]
struct RecoverySettings {
    eligibility: AutomaticEligibility,
    strategy: Strategy,
    limit: RestartLimit,
    release: RestartRelease,
}

enum WorkerSourceCustody<Source, PreparationReturn> {
    Available(Source),
    PreparingWorkers,
    Retirement(PreparationReturn),
}

struct AutomaticRecovery<Source, PreparationReturn> {
    settings: RecoverySettings,
    source: WorkerSourceCustody<Source, PreparationReturn>,
    budget: RestartBudget,
    correlations: RecoveryCorrelations,
}

enum SupervisorRecoveryState<Source, PreparationReturn> {
    Automatic(AutomaticRecovery<Source, PreparationReturn>),
    Temporary,
}

pub(super) struct SupervisorRecovery<Source, PreparationReturn> {
    state: SupervisorRecoveryState<Source, PreparationReturn>,
}

pub(super) enum RecoveryChoice<Source, PreparationReturn> {
    LeaveEmpty(SupervisorRecovery<Source, PreparationReturn>),
    SourceUnavailable(SupervisorRecovery<Source, PreparationReturn>),
    Prepare(RecoveryPreparation<Source>),
}

pub(super) struct RecoveryPreparation<Source> {
    settings: RecoverySettings,
    source: Source,
    budget: RestartBudget,
    correlations: RecoveryCorrelations,
}

pub(super) struct AwaitingWorkerSource {
    settings: RecoverySettings,
    budget: RestartBudget,
    correlations: RecoveryCorrelations,
}

pub(super) enum PreparationAcceptance<Role, Worker, Plan, Source>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Prepared {
        owners: Vec<RosterOwner<Role, Worker, Plan>>,
        position: usize,
        prepared: PreparedRecovery<Role, Worker, Plan>,
        source: Source,
    },
    Failed(FailedPreparation<Role, Worker, Plan, Source>),
}

pub(super) struct FailedPreparation<Role, Worker, Plan, Source>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    owners: Vec<RosterOwner<Role, Worker, Plan>>,
    member: StoppedMember<Role, Worker, Plan>,
    source: Source,
    diagnostic: WorkerPreparationFailure<
        Role,
        Worker,
        Plan,
        Source::WorkerRejection,
        Source::SourceRejection,
    >,
}

pub(super) struct UnrecoveredMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    proxy: ProxyStoppingMember<Role, Worker, Plan>,
    ownership: FixedProxyOwnership<Worker, Plan>,
}

impl<Source, PreparationReturn> From<Recovery<Source>>
    for SupervisorRecovery<Source, PreparationReturn>
{
    fn from(recovery: Recovery<Source>) -> Self {
        match recovery.decision {
            RecoveryDecision::Permanent {
                source,
                strategy,
                limit,
                release,
            } => Self {
                state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                    settings: RecoverySettings {
                        eligibility: AutomaticEligibility::Permanent,
                        strategy,
                        limit,
                        release,
                    },
                    source: WorkerSourceCustody::Available(source),
                    budget: RestartBudget::empty(),
                    correlations: RecoveryCorrelations::new(),
                }),
            },
            RecoveryDecision::Transient {
                source,
                strategy,
                limit,
                release,
            } => Self {
                state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                    settings: RecoverySettings {
                        eligibility: AutomaticEligibility::Transient,
                        strategy,
                        limit,
                        release,
                    },
                    source: WorkerSourceCustody::Available(source),
                    budget: RestartBudget::empty(),
                    correlations: RecoveryCorrelations::new(),
                }),
            },
            RecoveryDecision::Temporary => Self {
                state: SupervisorRecoveryState::Temporary,
            },
        }
    }
}

impl<Source, PreparationReturn> SupervisorRecovery<Source, PreparationReturn> {
    pub(super) const fn temporary() -> Self {
        Self {
            state: SupervisorRecoveryState::Temporary,
        }
    }

    pub(super) fn choose(self, stop: StopKind) -> RecoveryChoice<Source, PreparationReturn> {
        match self.state {
            SupervisorRecoveryState::Temporary => RecoveryChoice::LeaveEmpty(Self {
                state: SupervisorRecoveryState::Temporary,
            }),
            SupervisorRecoveryState::Automatic(recovery)
                if matches!(
                    (recovery.settings.eligibility, stop),
                    (AutomaticEligibility::Transient, StopKind::Normal)
                ) =>
            {
                RecoveryChoice::LeaveEmpty(Self {
                    state: SupervisorRecoveryState::Automatic(recovery),
                })
            }
            SupervisorRecoveryState::Automatic(AutomaticRecovery {
                settings,
                source: WorkerSourceCustody::Available(source),
                budget,
                correlations,
            }) => RecoveryChoice::Prepare(RecoveryPreparation {
                settings,
                source,
                budget,
                correlations,
            }),
            SupervisorRecoveryState::Automatic(
                recovery @ AutomaticRecovery {
                    source: WorkerSourceCustody::PreparingWorkers,
                    ..
                },
            )
            | SupervisorRecoveryState::Automatic(
                recovery @ AutomaticRecovery {
                    source: WorkerSourceCustody::Retirement(_),
                    ..
                },
            ) => RecoveryChoice::SourceUnavailable(Self {
                state: SupervisorRecoveryState::Automatic(recovery),
            }),
        }
    }

    pub(super) fn expect_source(self) -> Result<AwaitingWorkerSource, Self> {
        match self.state {
            SupervisorRecoveryState::Automatic(AutomaticRecovery {
                settings,
                source: WorkerSourceCustody::PreparingWorkers,
                budget,
                correlations,
            }) => Ok(AwaitingWorkerSource {
                settings,
                budget,
                correlations,
            }),
            state => Err(Self { state }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerSourceCustody;

    struct Workshop;
    struct CompletePreparationReturn;

    #[test]
    fn worker_source_has_exactly_one_current_custodian() {
        let available =
            WorkerSourceCustody::<Workshop, CompletePreparationReturn>::Available(Workshop);
        let preparing =
            WorkerSourceCustody::<Workshop, CompletePreparationReturn>::PreparingWorkers;
        let retirement = WorkerSourceCustody::<Workshop, CompletePreparationReturn>::Retirement(
            CompletePreparationReturn,
        );

        match available {
            WorkerSourceCustody::Available(source) => {
                let Workshop = source;
            }
            WorkerSourceCustody::PreparingWorkers | WorkerSourceCustody::Retirement(_) => {
                panic!("the supervisor owns the available worker source")
            }
        }
        match preparing {
            WorkerSourceCustody::PreparingWorkers => {}
            WorkerSourceCustody::Available(_) | WorkerSourceCustody::Retirement(_) => {
                panic!("the emitted preparation owns the worker source")
            }
        }
        match retirement {
            WorkerSourceCustody::Retirement(returned) => {
                let CompletePreparationReturn = returned;
            }
            WorkerSourceCustody::Available(_) | WorkerSourceCustody::PreparingWorkers => {
                panic!("the retiring supervisor owns the complete return")
            }
        }
    }

    const _: () = {
        #[expect(dead_code, reason = "compile-only exhaustive ownership contract")]
        fn recovery_ownership_is_current<Role, Worker, Plan>(
            owner: super::RecoveryRosterOwner<Role, Worker, Plan>,
        ) where
            Worker: behavior::Behavior,
            Plan: crate::ActivationPlan,
            behavior::BehaviorAddr<Worker>: behavior::EndpointAddress,
            crate::StableProxy<Worker, Plan>: behavior::Behavior<Protocol = Worker::Protocol>,
        {
            match owner {
                super::RecoveryRosterOwner::Preparing(_)
                | super::RecoveryRosterOwner::Admitted(_) => {}
            }
        }
    };
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior + Send,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn accept_preparation<Source>(
        self,
        input: ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    ) -> Result<
        PreparationAcceptance<Role, Worker, Plan, Source>,
        (
            Self,
            ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        ),
    >
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Role: Send + Sync,
    {
        let Self::Operating(mut owners) = self else {
            return Err((self, input));
        };
        let position = match owners.iter().position(|owner| {
            matches!(
                owner,
                RosterOwner::Recovery(RecoveryRosterOwner::Preparing(recovery))
                    if recovery.accepts_result(&input)
            )
        }) {
            Some(position) => position,
            None => return Err((Self::Operating(owners), input)),
        };
        let owner = owners.remove(position);
        let RosterOwner::Recovery(RecoveryRosterOwner::Preparing(recovery)) = owner else {
            owners.insert(position, owner);
            return Err((Self::Operating(owners), input));
        };
        let decision = match input {
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)) => {
                recovery.accept(preparation)
            }
            SettledItem::Attempted(ItemSettlement::Rejected { item, reason }) => {
                let (source, first, remaining) = item.into_parts();
                let (restored_peers, trigger_role, stopped_trigger) =
                    recovery.restore_peers_after_preparation_failure();
                RecoveryPreparationDecision::Failed {
                    restored_peers,
                    stopped_trigger,
                    source,
                    diagnostic: WorkerPreparationFailure::source_rejected(
                        trigger_role,
                        reason,
                        selected_roles(first, remaining),
                    ),
                }
            }
            SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }) => {
                let (source, first, remaining) = item.into_parts();
                let (restored_peers, trigger_role, stopped_trigger) =
                    recovery.restore_peers_after_preparation_failure();
                RecoveryPreparationDecision::Failed {
                    restored_peers,
                    stopped_trigger,
                    source,
                    diagnostic: WorkerPreparationFailure::interpreter_fault(
                        trigger_role,
                        fault,
                        selected_roles(first, remaining),
                    ),
                }
            }
            SettledItem::Unattempted(item) => {
                let (source, first, remaining) = item.into_parts();
                let (restored_peers, trigger_role, stopped_trigger) =
                    recovery.restore_peers_after_preparation_failure();
                RecoveryPreparationDecision::Failed {
                    restored_peers,
                    stopped_trigger,
                    source,
                    diagnostic: WorkerPreparationFailure::unattempted(
                        trigger_role,
                        selected_roles(first, remaining),
                    ),
                }
            }
            SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                match prerequisite {}
            }
        };
        match decision {
            RecoveryPreparationDecision::Prepared { recovery, source } => {
                Ok(PreparationAcceptance::Prepared {
                    owners,
                    position,
                    prepared: recovery,
                    source,
                })
            }
            RecoveryPreparationDecision::Failed {
                restored_peers,
                stopped_trigger,
                source,
                diagnostic,
            } => {
                owners.extend(restored_peers);
                Ok(PreparationAcceptance::Failed(FailedPreparation {
                    owners,
                    member: stopped_trigger,
                    source,
                    diagnostic,
                }))
            }
            RecoveryPreparationDecision::Rejected {
                recovery,
                preparation,
            } => {
                owners.insert(
                    position,
                    RosterOwner::Recovery(RecoveryRosterOwner::Preparing(recovery)),
                );
                Err((
                    Self::Operating(owners),
                    SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
                ))
            }
        }
    }
}

fn selected_roles<Role>(
    first: RoleName<Role>,
    mut remaining: Vec<RoleName<Role>>,
) -> Vec<RoleName<Role>> {
    remaining.insert(0, first);
    remaining
}

impl<Role, Worker, Plan, Source> FailedPreparation<Role, Worker, Plan, Source>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn into_owners(
        self,
    ) -> (
        Vec<RosterOwner<Role, Worker, Plan>>,
        Source,
        WorkerPreparationFailure<
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
    ) {
        let Self {
            mut owners,
            member,
            source,
            diagnostic,
        } = self;
        owners.push(RosterOwner::Empty(member.retain_stop()));
        (owners, source, diagnostic)
    }

    pub(super) fn terminate(
        self,
    ) -> (
        FixedRoster<Role, Worker, Plan>,
        Source,
        WorkerPreparationFailure<
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
    ) {
        let (owners, source, diagnostic) = self.into_owners();
        (
            FixedRoster::Terminating {
                owners,
                prepared: Vec::new(),
            },
            source,
            diagnostic,
        )
    }

    pub(super) fn stop_supervisor(
        self,
        actor_drain: super::ActorDrainPolicy,
    ) -> (
        FixedShutdown<Role, Worker, Plan>,
        Source,
        WorkerPreparationFailure<
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
        Vec<ProxyOperation<behavior::Here, Worker, Plan>>,
        Option<ScheduleAfter>,
    ) {
        let (owners, source, diagnostic) = self.into_owners();
        let (shutdown, operations, schedule) = FixedShutdown::begin(owners, actor_drain);
        (
            shutdown,
            source,
            diagnostic,
            operations
                .into_iter()
                .map(|(_, operation)| operation)
                .collect(),
            schedule,
        )
    }

    pub(super) fn retire_member(
        self,
    ) -> (
        FixedRoster<Role, Worker, Plan>,
        Source,
        WorkerPreparationFailure<
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
        ProxyOperation<behavior::Here, Worker, Plan>,
    ) {
        let Self {
            mut owners,
            member,
            source,
            diagnostic,
        } = self;
        let (member, operation) = UnrecoveredMember::begin(member);
        owners.push(RosterOwner::Unrecovered(member));
        (
            FixedRoster::Operating(owners),
            source,
            diagnostic,
            operation,
        )
    }
}

impl<Role, Worker, Plan> UnrecoveredMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn role(&self) -> &Role {
        self.proxy.role()
    }

    pub(super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        self.proxy.live_proxy_role(child)
    }

    pub(super) fn position(&self) -> RosterPosition {
        self.proxy.position()
    }

    fn begin(
        member: StoppedMember<Role, Worker, Plan>,
    ) -> (Self, ProxyOperation<behavior::Here, Worker, Plan>) {
        let StoppedMember {
            role,
            creation,
            proxy,
            worker,
            readiness,
            stopped,
        } = member;
        let (proxy, operation) = ProxyStoppingMember::begin(role, creation, proxy);
        (
            Self {
                proxy,
                ownership: FixedProxyOwnership::WorkerStopped {
                    worker,
                    readiness,
                    stopped: Some(stopped),
                    prepared: None,
                },
            },
            operation,
        )
    }

    pub(super) fn begin_replacement_failure(
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        worker: WorkerAttempt,
        readiness: Plan::Ready,
        stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
    ) -> (Self, ProxyOperation<behavior::Here, Worker, Plan>) {
        let ownership = match stopped {
            Some(stopped) => FixedProxyOwnership::WorkerStopped {
                worker,
                readiness,
                stopped: Some(stopped),
                prepared: None,
            },
            None => FixedProxyOwnership::Ready {
                worker,
                readiness,
                prepared: None,
            },
        };
        let (proxy, operation) = ProxyStoppingMember::begin(role, creation, proxy);
        (Self { proxy, ownership }, operation)
    }

    pub(super) fn begin_after_worker_stop_transfer(
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
    ) -> (Self, ProxyOperation<behavior::Here, Worker, Plan>) {
        let (proxy, operation) = ProxyStoppingMember::begin(role, creation, proxy);
        (
            Self {
                proxy,
                ownership: FixedProxyOwnership::Unavailable,
            },
            operation,
        )
    }

    pub(super) fn accepts_stop(&self, stopped: &ChildStopped<BehaviorAddr<Worker>>) -> bool {
        self.proxy.accepts_stop(stopped)
    }

    pub(super) fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        ControlFlow<RetiredProxyMember<Role, Worker, Plan>, Self>,
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        let Self { proxy, ownership } = self;
        match proxy.accept_operation(settlement) {
            Ok(ControlFlow::Continue(proxy)) => {
                Ok(ControlFlow::Continue(Self { proxy, ownership }))
            }
            Ok(ControlFlow::Break(retired)) => {
                drop(ownership);
                Ok(ControlFlow::Break(retired))
            }
            Err((proxy, settlement)) => Err((Self { proxy, ownership }, settlement)),
        }
    }

    pub(super) fn accept_stop(
        self,
        stopped_proxy: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Result<
        ControlFlow<RetiredProxyMember<Role, Worker, Plan>, Self>,
        (Self, ChildStopped<BehaviorAddr<Worker>>),
    > {
        let Self { proxy, ownership } = self;
        match proxy.accept_stop(stopped_proxy) {
            Ok(ControlFlow::Continue(proxy)) => {
                Ok(ControlFlow::Continue(Self { proxy, ownership }))
            }
            Ok(ControlFlow::Break(retired)) => {
                drop(ownership);
                Ok(ControlFlow::Break(retired))
            }
            Err((proxy, stopped_proxy)) => Err((Self { proxy, ownership }, stopped_proxy)),
        }
    }

    pub(super) fn into_shutdown(self) -> FixedShutdownMember<Role, Worker, Plan> {
        FixedShutdownMember::Proxy {
            proxy: ControlFlow::Continue(self.proxy),
            ownership: self.ownership,
            preparation: None,
        }
    }
}

impl<Role, Worker, Plan> EmptyMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn retire_prepared_worker(
        self,
        prepared: crate::WorkerSubmission<Worker, Plan>,
    ) -> (
        ProxyStoppingMember<Role, Worker, Plan>,
        FixedProxyOwnership<Worker, Plan>,
        ProxyOperation<behavior::Here, Worker, Plan>,
    ) {
        let Self {
            role,
            creation,
            proxy,
            worker,
            readiness,
            stopped,
        } = self;
        let (proxy, operation) = ProxyStoppingMember::begin(role, creation, proxy);
        (
            proxy,
            FixedProxyOwnership::WorkerStopped {
                worker,
                readiness,
                stopped,
                prepared: Some(prepared),
            },
            operation,
        )
    }
}

impl AwaitingWorkerSource {
    pub(super) fn waiting<Source, PreparationReturn>(
        self,
    ) -> SupervisorRecovery<Source, PreparationReturn> {
        SupervisorRecovery {
            state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                settings: self.settings,
                source: WorkerSourceCustody::PreparingWorkers,
                budget: self.budget,
                correlations: self.correlations,
            }),
        }
    }

    pub(super) fn restore<Source, PreparationReturn>(
        self,
        source: Source,
    ) -> SupervisorRecovery<Source, PreparationReturn> {
        SupervisorRecovery {
            state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                settings: self.settings,
                source: WorkerSourceCustody::Available(source),
                budget: self.budget,
                correlations: self.correlations,
            }),
        }
    }

    pub(super) fn retain_for_retirement<Source, PreparationReturn>(
        self,
        returned: PreparationReturn,
    ) -> SupervisorRecovery<Source, PreparationReturn> {
        SupervisorRecovery {
            state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                settings: self.settings,
                source: WorkerSourceCustody::Retirement(returned),
                budget: self.budget,
                correlations: self.correlations,
            }),
        }
    }
}

impl<Source> RecoveryPreparation<Source> {
    pub(super) const fn strategy(&self) -> Strategy {
        self.settings.strategy
    }

    pub(super) fn restore<PreparationReturn>(
        self,
    ) -> SupervisorRecovery<Source, PreparationReturn> {
        SupervisorRecovery {
            state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                settings: self.settings,
                source: WorkerSourceCustody::Available(self.source),
                budget: self.budget,
                correlations: self.correlations,
            }),
        }
    }

    pub(super) fn begin<Role, Worker, Plan, PreparationReturn>(
        self,
        first: RoleName<Role>,
        remaining: Vec<RoleName<Role>>,
    ) -> (
        SupervisorRecovery<Source, PreparationReturn>,
        PreparationTicket,
        PrepareWorkers<Source, Role, Worker, Plan>,
    )
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Worker: Behavior + Send,
        Plan: super::ActivationPlan,
    {
        let (expected, request) = PrepareWorkers::new(self.source, first, remaining);
        (
            SupervisorRecovery {
                state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
                    settings: self.settings,
                    source: WorkerSourceCustody::PreparingWorkers,
                    budget: self.budget,
                    correlations: self.correlations,
                }),
            },
            expected,
            request,
        )
    }
}

pub(super) enum RecoveryParticipant<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Stopped(StoppedMember<Role, Worker, Plan>),
    InitialInputDispatched {
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        witness: crate::atomic::stable_proxy::ProxyOperationWitness,
    },
    AwaitingInitialOutcome {
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        operation: crate::ProxyOperationId,
    },
    Online(OnlineMember<Role, Worker, Plan>),
}

pub(super) struct PreparedParticipant<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) member: RecoveryParticipant<Role, Worker, Plan>,
    pub(super) submission: crate::WorkerSubmission<Worker, Plan>,
}

impl<Role, Worker, Plan> RecoveryParticipant<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn role(&self) -> &Role {
        match self {
            Self::Stopped(member) => member.role.role(),
            Self::InitialInputDispatched { role, .. }
            | Self::AwaitingInitialOutcome { role, .. } => role.role(),
            Self::Online(member) => member.role.role(),
        }
    }

    fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::Stopped(member) => member.live_proxy_role(child),
            Self::InitialInputDispatched { role, creation, .. }
            | Self::AwaitingInitialOutcome { role, creation, .. }
                if *creation == child =>
            {
                Some(role.name())
            }
            Self::Online(member) if member.creation == child => Some(member.role.name()),
            Self::InitialInputDispatched { .. }
            | Self::AwaitingInitialOutcome { .. }
            | Self::Online(_) => None,
        }
    }

    pub(super) fn select(
        owner: RosterOwner<Role, Worker, Plan>,
    ) -> Result<Self, RosterOwner<Role, Worker, Plan>> {
        match owner {
            RosterOwner::Starting(ProxyStartingMember::InputDispatched {
                role,
                creation,
                proxy,
                witness,
            }) => Ok(Self::InitialInputDispatched {
                role,
                creation,
                proxy,
                witness,
            }),
            RosterOwner::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                role,
                creation,
                proxy,
                operation,
            }) => Ok(Self::AwaitingInitialOutcome {
                role,
                creation,
                proxy,
                operation,
            }),
            RosterOwner::Online(member) => Ok(Self::Online(member)),
            owner => Err(owner),
        }
    }

    fn name(&self) -> RoleName<Role> {
        match self {
            Self::Stopped(member) => member.role.name(),
            Self::InitialInputDispatched { role, .. }
            | Self::AwaitingInitialOutcome { role, .. } => role.name(),
            Self::Online(member) => member.role.name(),
        }
    }

    pub(super) fn position(&self) -> RosterPosition {
        match self {
            Self::Stopped(member) => member.role.position(),
            Self::InitialInputDispatched { role, .. }
            | Self::AwaitingInitialOutcome { role, .. } => role.position(),
            Self::Online(member) => member.role.position(),
        }
    }

    fn into_stopped(self) -> Result<StoppedMember<Role, Worker, Plan>, Self> {
        match self {
            Self::Stopped(member) => Ok(member),
            member @ (Self::InitialInputDispatched { .. }
            | Self::AwaitingInitialOutcome { .. }
            | Self::Online(_)) => Err(member),
        }
    }

    pub(super) fn release(self) -> RosterOwner<Role, Worker, Plan> {
        match self {
            Self::Stopped(member) => RosterOwner::Empty(member.retain_stop()),
            Self::InitialInputDispatched {
                role,
                creation,
                proxy,
                witness,
            } => RosterOwner::Starting(ProxyStartingMember::InputDispatched {
                role,
                creation,
                proxy,
                witness,
            }),
            Self::AwaitingInitialOutcome {
                role,
                creation,
                proxy,
                operation,
            } => RosterOwner::Starting(ProxyStartingMember::AwaitingProxyOutcome {
                role,
                creation,
                proxy,
                operation,
            }),
            Self::Online(member) => RosterOwner::Online(member),
        }
    }

    fn prepared(
        self,
        submission: crate::WorkerSubmission<Worker, Plan>,
    ) -> PreparedParticipant<Role, Worker, Plan> {
        PreparedParticipant {
            member: self,
            submission,
        }
    }
}

pub(super) struct PreparingRecovery<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    expected: PreparationTicket,
    before_trigger: Vec<RecoveryParticipant<Role, Worker, Plan>>,
    trigger: StoppedMember<Role, Worker, Plan>,
    after_trigger: Vec<RecoveryParticipant<Role, Worker, Plan>>,
}

pub(super) struct PreparedRecovery<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    before_trigger: Vec<PreparedParticipant<Role, Worker, Plan>>,
    trigger: StoppedMember<Role, Worker, Plan>,
    trigger_submission: crate::WorkerSubmission<Worker, Plan>,
    after_trigger: Vec<PreparedParticipant<Role, Worker, Plan>>,
}

pub(super) struct RecoveryFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    owners: Vec<RosterOwner<Role, Worker, Plan>>,
    replacements: UnissuedReplacements<Role, Worker, Plan>,
}

pub(super) struct UnissuedReplacements<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) peers: Vec<PreparedParticipant<Role, Worker, Plan>>,
    pub(super) trigger: EmptyMember<Role, Worker, Plan>,
    pub(super) trigger_submission: crate::WorkerSubmission<Worker, Plan>,
}

pub(super) enum RestartReleaseState {
    Ready,
    Scheduling { timer: ScheduleKey },
    Waiting { timer: ScheduleKey },
}

pub(super) enum RecoveryMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Waiting {
        member: RecoveryParticipant<Role, Worker, Plan>,
        submission: crate::WorkerSubmission<Worker, Plan>,
    },
    Replacing {
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        replacement: WorkerReplacement<Worker, Plan>,
    },
}

pub(super) struct WorkerReplacement<Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) previous: WorkerAttempt,
    pub(super) readiness: Plan::Ready,
    pub(super) stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
    pub(super) response: ReplacementResponse<Worker, Plan>,
}

pub(super) enum ReplacementResponse<Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    InputPending(crate::atomic::stable_proxy::ProxyOperationWitness),
    OutcomePending(ProxyOperationId),
    OutcomeReturned(ReplacementOutcome<Worker, Plan>),
    InputRejected {
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
        reason: ChildInputReason,
    },
}

pub(super) enum ReplacementCompletion<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Restarted {
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        previous_readiness: Plan::Ready,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
        worker: WorkerAttempt,
        readiness: Plan::Ready,
        recovery: RecoveryTicket,
    },
    InputRejected {
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        previous: WorkerAttempt,
        previous_readiness: Plan::Ready,
        stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
        operation: ProxyOperation<behavior::Here, Worker, Plan>,
        reason: ChildInputReason,
        recovery: RecoveryTicket,
    },
    ProxyFailed {
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        previous: WorkerAttempt,
        previous_readiness: Plan::Ready,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
        outcome: ReplacementOutcome<Worker, Plan>,
        recovery: RecoveryTicket,
    },
}

pub(super) struct RecoveryBatch<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) ticket: RecoveryTicket,
    pub(super) trigger: RosterPosition,
    pub(super) release: RestartReleaseState,
    pub(super) members: VecDeque<RecoveryMember<Role, Worker, Plan>>,
}

enum RecoveryScheduleAdmission<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Waiting(RecoveryBatch<Role, Worker, Plan>),
    Rejected {
        prepared: PreparedRecovery<Role, Worker, Plan>,
        request: ScheduleAfter,
        reason: ScheduleAfterRejection,
    },
    Unrelated {
        recovery: RecoveryBatch<Role, Worker, Plan>,
        input: ActionItemResult<ScheduleAfter>,
    },
}

pub(super) enum RestartScheduleAdmission<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Waiting(FixedRoster<Role, Worker, Plan>),
    Rejected(RejectedRestartSchedule<Role, Worker, Plan>),
    Unrelated {
        roster: FixedRoster<Role, Worker, Plan>,
        input: ActionItemResult<ScheduleAfter>,
    },
}

pub(super) struct RejectedRestartSchedule<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    failed: RecoveryFailure<Role, Worker, Plan>,
    trigger: RoleName<Role>,
    request: ScheduleAfter,
    reason: ScheduleAfterRejection,
}

pub(super) enum PreparedRecoveryDecision<Role, Worker, Plan, Source, PreparationReturn>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Admitted {
        roster: FixedRoster<Role, Worker, Plan>,
        recovery: SupervisorRecovery<Source, PreparationReturn>,
        schedule: Option<ScheduleAfter>,
    },
    Denied {
        failed: RecoveryFailure<Role, Worker, Plan>,
        recovery: SupervisorRecovery<Source, PreparationReturn>,
        diagnostic: RecoveryDenied<Role, Worker>,
    },
}

pub(super) enum RecoveryRosterOwner<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Preparing(PreparingRecovery<Role, Worker, Plan>),
    Admitted(RecoveryBatch<Role, Worker, Plan>),
}

impl<Role, Worker, Plan> PreparedParticipant<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn admit(self) -> RecoveryMember<Role, Worker, Plan> {
        RecoveryMember::Waiting {
            member: self.member,
            submission: self.submission,
        }
    }

    fn cancel(self) -> RosterOwner<Role, Worker, Plan> {
        let Self {
            member,
            submission: _,
        } = self;
        member.release()
    }
}

impl<Role, Worker, Plan> RecoveryFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn into_terminal_ownership(
        self,
    ) -> (
        Vec<RosterOwner<Role, Worker, Plan>>,
        Vec<crate::WorkerSubmission<Worker, Plan>>,
    ) {
        let Self {
            mut owners,
            replacements:
                UnissuedReplacements {
                    peers,
                    trigger,
                    trigger_submission,
                },
        } = self;
        let mut prepared = Vec::new();
        for participant in peers {
            let PreparedParticipant { member, submission } = participant;
            owners.push(member.release());
            prepared.push(submission);
        }
        owners.push(RosterOwner::Empty(trigger));
        prepared.push(trigger_submission);
        (owners, prepared)
    }

    pub(super) fn terminate(self) -> FixedRoster<Role, Worker, Plan> {
        let (owners, prepared) = self.into_terminal_ownership();
        FixedRoster::Terminating { owners, prepared }
    }

    pub(super) fn stop_supervisor(
        self,
        actor_drain: super::ActorDrainPolicy,
    ) -> (
        FixedShutdown<Role, Worker, Plan>,
        Vec<ProxyOperation<behavior::Here, Worker, Plan>>,
        Option<ScheduleAfter>,
    ) {
        let Self {
            owners,
            replacements,
        } = self;
        let (shutdown, operations, schedule) =
            FixedShutdown::begin_with_unissued_replacements(owners, replacements, actor_drain);
        (
            shutdown,
            operations
                .into_iter()
                .map(|(_, operation)| operation)
                .collect(),
            schedule,
        )
    }

    pub(super) fn retire_member(
        self,
    ) -> (
        FixedRoster<Role, Worker, Plan>,
        ProxyOperation<behavior::Here, Worker, Plan>,
    ) {
        let Self {
            mut owners,
            replacements:
                UnissuedReplacements {
                    peers,
                    trigger,
                    trigger_submission,
                },
        } = self;
        owners.extend(peers.into_iter().map(PreparedParticipant::cancel));
        let (proxy, ownership, operation) = trigger.retire_prepared_worker(trigger_submission);
        owners.push(RosterOwner::Unrecovered(UnrecoveredMember {
            proxy,
            ownership,
        }));
        (FixedRoster::Operating(owners), operation)
    }
}

impl<Role, Worker, Plan> RejectedRestartSchedule<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn into_parts(
        self,
    ) -> (
        RecoveryFailure<Role, Worker, Plan>,
        RestartScheduleFailure<Role>,
    ) {
        (
            self.failed,
            RestartScheduleFailure::new(self.trigger, self.request, self.reason),
        )
    }
}

impl<Role, Worker, Plan> PreparedRecovery<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn worker_count(&self) -> core::num::NonZeroUsize {
        let workers = self
            .before_trigger
            .len()
            .checked_add(1)
            .and_then(|count| count.checked_add(self.after_trigger.len()))
            .expect("owned prepared-worker collections fit in usize");
        core::num::NonZeroUsize::new(workers)
            .expect("a prepared recovery always owns its triggering worker")
    }

    fn from_admitted(
        trigger: RosterPosition,
        mut members: Vec<PreparedParticipant<Role, Worker, Plan>>,
    ) -> Result<Self, Vec<PreparedParticipant<Role, Worker, Plan>>> {
        let trigger_index = match members
            .iter()
            .position(|member| member.member.position() == trigger)
        {
            Some(position) => position,
            None => return Err(members),
        };
        let PreparedParticipant { member, submission } = members.remove(trigger_index);
        let trigger = match member.into_stopped() {
            Ok(trigger) => trigger,
            Err(member) => {
                members.insert(trigger_index, PreparedParticipant { member, submission });
                return Err(members);
            }
        };
        let after_trigger = members.split_off(trigger_index);
        Ok(Self {
            before_trigger: members,
            trigger,
            trigger_submission: submission,
            after_trigger,
        })
    }

    fn admit(
        self,
        ticket: RecoveryTicket,
        timer: RecoveryTimer,
    ) -> RecoveryBatch<Role, Worker, Plan> {
        let release = match timer {
            RecoveryTimer::Immediate => RestartReleaseState::Ready,
            RecoveryTimer::Delayed(timer) => RestartReleaseState::Scheduling { timer },
        };
        let trigger = self.trigger.role.position();
        let mut members = self
            .before_trigger
            .into_iter()
            .map(PreparedParticipant::admit)
            .collect::<VecDeque<_>>();
        members.push_back(
            PreparedParticipant {
                member: RecoveryParticipant::Stopped(self.trigger),
                submission: self.trigger_submission,
            }
            .admit(),
        );
        members.extend(
            self.after_trigger
                .into_iter()
                .map(PreparedParticipant::admit),
        );
        RecoveryBatch {
            ticket,
            trigger,
            release,
            members,
        }
    }
}

impl<Role, Worker, Plan> RecoveryMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn role(&self) -> &Role {
        match self {
            Self::Waiting { member, .. } => member.role(),
            Self::Replacing { role, .. } => role.role(),
        }
    }

    fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::Waiting { member, .. } => member.live_proxy_role(child),
            Self::Replacing { role, creation, .. } if *creation == child => Some(role.name()),
            Self::Replacing { .. } => None,
        }
    }

    fn position(&self) -> RosterPosition {
        match self {
            Self::Waiting { member, .. } => member.position(),
            Self::Replacing { role, .. } => role.position(),
        }
    }

    fn authorization_count(&self) -> usize {
        match self {
            Self::Waiting { .. } => 0,
            Self::Replacing { replacement, .. } => match replacement.response {
                ReplacementResponse::InputPending(_) | ReplacementResponse::OutcomePending(_) => 1,
                ReplacementResponse::OutcomeReturned(_)
                | ReplacementResponse::InputRejected { .. } => 0,
            },
        }
    }

    fn authorize(self) -> (Self, Option<ProxyOperation<behavior::Here, Worker, Plan>>) {
        let Self::Waiting { member, submission } = self else {
            return (self, None);
        };
        match member {
            RecoveryParticipant::Stopped(StoppedMember {
                role,
                creation,
                proxy,
                worker,
                readiness,
                stopped,
            }) => {
                let (witness, operation) = ProxyOperation::replacement(creation, submission);
                (
                    Self::Replacing {
                        role,
                        creation,
                        proxy,
                        replacement: WorkerReplacement {
                            previous: worker,
                            readiness,
                            stopped: Some(stopped),
                            response: ReplacementResponse::InputPending(witness),
                        },
                    },
                    Some(operation),
                )
            }
            RecoveryParticipant::Online(OnlineMember {
                role,
                creation,
                proxy,
                worker,
                readiness,
            }) => {
                let (witness, operation) = ProxyOperation::replacement(creation, submission);
                (
                    Self::Replacing {
                        role,
                        creation,
                        proxy,
                        replacement: WorkerReplacement {
                            previous: worker,
                            readiness,
                            stopped: None,
                            response: ReplacementResponse::InputPending(witness),
                        },
                    },
                    Some(operation),
                )
            }
            member @ (RecoveryParticipant::InitialInputDispatched { .. }
            | RecoveryParticipant::AwaitingInitialOutcome { .. }) => {
                (Self::Waiting { member, submission }, None)
            }
        }
    }

    fn into_prepared(self) -> Result<PreparedParticipant<Role, Worker, Plan>, Self> {
        match self {
            Self::Waiting { member, submission } => Ok(PreparedParticipant { member, submission }),
            member => Err(member),
        }
    }

    fn accept_replacement_operation(
        self,
        recovery: RecoveryTicket,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        ControlFlow<ReplacementCompletion<Role, Worker, Plan>, Self>,
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        match self {
            Self::Replacing {
                role,
                creation,
                proxy,
                replacement,
            } => match replacement.accept_operation(settlement) {
                Ok(replacement) => Ok(Self::replacement_progress(
                    role,
                    creation,
                    proxy,
                    replacement,
                    recovery,
                )),
                Err((replacement, settlement)) => Err((
                    Self::Replacing {
                        role,
                        creation,
                        proxy,
                        replacement,
                    },
                    settlement,
                )),
            },
            member => Err((member, settlement)),
        }
    }

    fn accept_recovery_report(
        self,
        recovery: RecoveryTicket,
        report: ChildReport<crate::ProxyOutcome<Worker, Plan>>,
    ) -> Result<
        ControlFlow<ReplacementCompletion<Role, Worker, Plan>, Self>,
        (Self, ChildReport<crate::ProxyOutcome<Worker, Plan>>),
    > {
        match (self, report.report) {
            (
                Self::Waiting {
                    member:
                        RecoveryParticipant::Online(OnlineMember {
                            role,
                            creation,
                            proxy,
                            worker,
                            readiness,
                        }),
                    submission,
                },
                crate::ProxyOutcome::WorkerStopped {
                    worker: reported,
                    stopped,
                },
            ) if creation == report.child && worker == reported => {
                Ok(ControlFlow::Continue(Self::Waiting {
                    member: RecoveryParticipant::Stopped(StoppedMember {
                        role,
                        creation,
                        proxy,
                        worker,
                        readiness,
                        stopped,
                    }),
                    submission,
                }))
            }
            (
                Self::Replacing {
                    role,
                    creation,
                    proxy,
                    replacement,
                },
                crate::ProxyOutcome::Replacement { outcome },
            ) if creation == report.child => match replacement.accept_outcome(outcome) {
                Ok(replacement) => Ok(Self::replacement_progress(
                    role,
                    creation,
                    proxy,
                    replacement,
                    recovery,
                )),
                Err((replacement, outcome)) => Err((
                    Self::Replacing {
                        role,
                        creation,
                        proxy,
                        replacement,
                    },
                    ChildReport::new(report.child, crate::ProxyOutcome::Replacement { outcome }),
                )),
            },
            (
                Self::Replacing {
                    role,
                    creation,
                    proxy,
                    replacement,
                },
                crate::ProxyOutcome::WorkerStopped { worker, stopped },
            ) if creation == report.child => {
                match replacement.accept_worker_stop(worker, stopped) {
                    Ok(replacement) => Ok(Self::replacement_progress(
                        role,
                        creation,
                        proxy,
                        replacement,
                        recovery,
                    )),
                    Err((replacement, worker, stopped)) => Err((
                        Self::Replacing {
                            role,
                            creation,
                            proxy,
                            replacement,
                        },
                        ChildReport::new(
                            report.child,
                            crate::ProxyOutcome::WorkerStopped { worker, stopped },
                        ),
                    )),
                }
            }
            (member, outcome) => Err((member, ChildReport::new(report.child, outcome))),
        }
    }

    fn replacement_progress(
        role: super::role::MemberRole<Role>,
        creation: CreationId,
        proxy: behavior::EstablishedActor<StableProxy<Worker, Plan>>,
        replacement: WorkerReplacement<Worker, Plan>,
        recovery: RecoveryTicket,
    ) -> ControlFlow<ReplacementCompletion<Role, Worker, Plan>, Self> {
        let WorkerReplacement {
            previous,
            readiness: previous_readiness,
            stopped,
            response,
        } = replacement;
        match (stopped, response) {
            (
                Some(stopped),
                ReplacementResponse::OutcomeReturned(ReplacementOutcome::Resolved {
                    result:
                        crate::WorkerStartResult::Ready {
                            attempt: worker,
                            readiness,
                        },
                    ..
                }),
            ) => ControlFlow::Break(ReplacementCompletion::Restarted {
                role,
                creation,
                proxy,
                previous_readiness,
                stopped,
                worker,
                readiness,
                recovery,
            }),
            (Some(stopped), ReplacementResponse::OutcomeReturned(outcome)) => {
                ControlFlow::Break(ReplacementCompletion::ProxyFailed {
                    role,
                    creation,
                    proxy,
                    previous,
                    previous_readiness,
                    stopped,
                    outcome,
                    recovery,
                })
            }
            (stopped, ReplacementResponse::InputRejected { operation, reason }) => {
                ControlFlow::Break(ReplacementCompletion::InputRejected {
                    role,
                    creation,
                    proxy,
                    previous,
                    previous_readiness,
                    stopped,
                    operation,
                    reason,
                    recovery,
                })
            }
            (stopped, response) => ControlFlow::Continue(Self::Replacing {
                role,
                creation,
                proxy,
                replacement: WorkerReplacement {
                    previous,
                    readiness: previous_readiness,
                    stopped,
                    response,
                },
            }),
        }
    }
}

impl<Worker, Plan> WorkerReplacement<Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<Self, (Self, ProxyInputResult<behavior::Here, Worker, Plan>)> {
        let Self {
            previous,
            readiness,
            stopped,
            response,
        } = self;
        let ReplacementResponse::InputPending(witness) = response else {
            return Err((
                Self {
                    previous,
                    readiness,
                    stopped,
                    response,
                },
                settlement,
            ));
        };
        match settlement {
            SettledItem::Attempted(ItemSettlement::Accepted(receipt)) => {
                match witness.admit_receipt(receipt) {
                    Ok(receipt) => {
                        let (_, _, operation) = receipt.into_parts();
                        Ok(Self {
                            previous,
                            readiness,
                            stopped,
                            response: ReplacementResponse::OutcomePending(operation),
                        })
                    }
                    Err((witness, receipt)) => Err((
                        Self {
                            previous,
                            readiness,
                            stopped,
                            response: ReplacementResponse::InputPending(witness),
                        },
                        SettledItem::Attempted(ItemSettlement::Accepted(receipt)),
                    )),
                }
            }
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: operation,
                reason,
            }) => match witness.admit_rejection(operation, reason) {
                Ok((operation, reason)) => Ok(Self {
                    previous,
                    readiness,
                    stopped,
                    response: ReplacementResponse::InputRejected { operation, reason },
                }),
                Err((witness, operation, reason)) => Err((
                    Self {
                        previous,
                        readiness,
                        stopped,
                        response: ReplacementResponse::InputPending(witness),
                    },
                    SettledItem::Attempted(ItemSettlement::Rejected {
                        item: operation,
                        reason,
                    }),
                )),
            },
            settlement @ (SettledItem::Attempted(
                ItemSettlement::Blocked { .. } | ItemSettlement::Corrupt { .. },
            )
            | SettledItem::Unattempted(_)) => Err((
                Self {
                    previous,
                    readiness,
                    stopped,
                    response: ReplacementResponse::InputPending(witness),
                },
                settlement,
            )),
        }
    }

    pub(super) fn accept_outcome(
        self,
        outcome: ReplacementOutcome<Worker, Plan>,
    ) -> Result<Self, (Self, ReplacementOutcome<Worker, Plan>)> {
        let Self {
            previous,
            readiness,
            stopped,
            response,
        } = self;
        let ReplacementResponse::OutcomePending(operation) = response else {
            return Err((
                Self {
                    previous,
                    readiness,
                    stopped,
                    response,
                },
                outcome,
            ));
        };
        match &outcome {
            ReplacementOutcome::WorkerAttemptsExhausted { replaces, .. }
            | ReplacementOutcome::CancelledBeforeBirth { replaces, .. }
            | ReplacementOutcome::Resolved { replaces, .. }
                if replaces == &previous =>
            {
                drop(operation);
                Ok(Self {
                    previous,
                    readiness,
                    stopped,
                    response: ReplacementResponse::OutcomeReturned(outcome),
                })
            }
            ReplacementOutcome::NotReplaceable { .. }
            | ReplacementOutcome::WorkerAttemptsExhausted { .. }
            | ReplacementOutcome::CancelledBeforeBirth { .. }
            | ReplacementOutcome::Resolved { .. } => Err((
                Self {
                    previous,
                    readiness,
                    stopped,
                    response: ReplacementResponse::OutcomePending(operation),
                },
                outcome,
            )),
        }
    }

    pub(super) fn accept_worker_stop(
        self,
        worker: WorkerAttempt,
        stopped_worker: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Result<Self, (Self, WorkerAttempt, ChildStopped<BehaviorAddr<Worker>>)> {
        let Self {
            previous,
            readiness,
            stopped,
            response,
        } = self;
        match stopped {
            None if worker == previous => Ok(Self {
                previous,
                readiness,
                stopped: Some(stopped_worker),
                response,
            }),
            stopped => Err((
                Self {
                    previous,
                    readiness,
                    stopped,
                    response,
                },
                worker,
                stopped_worker,
            )),
        }
    }
}

fn into_prepared_members<Role, Worker, Plan>(
    mut members: VecDeque<RecoveryMember<Role, Worker, Plan>>,
) -> Result<
    Vec<PreparedParticipant<Role, Worker, Plan>>,
    VecDeque<RecoveryMember<Role, Worker, Plan>>,
>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let mut prepared = Vec::with_capacity(members.len());
    while let Some(member) = members.pop_front() {
        match member.into_prepared() {
            Ok(member) => prepared.push(member),
            Err(member) => {
                let mut restored = prepared
                    .into_iter()
                    .map(PreparedParticipant::admit)
                    .collect::<VecDeque<_>>();
                restored.push_back(member);
                restored.append(&mut members);
                return Err(restored);
            }
        }
    }
    Ok(prepared)
}

fn authorize_recovery_sequence<Role, Worker, Plan>(
    mut members: VecDeque<RecoveryMember<Role, Worker, Plan>>,
) -> (
    VecDeque<RecoveryMember<Role, Worker, Plan>>,
    Option<ProxyOperation<behavior::Here, Worker, Plan>>,
)
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let mut examined = VecDeque::new();
    while let Some(member) = members.pop_front() {
        let (member, operation) = member.authorize();
        examined.push_back(member);
        match operation {
            Some(operation) => {
                examined.append(&mut members);
                return (examined, Some(operation));
            }
            None => {}
        }
    }
    (examined, None)
}

impl<Role, Worker, Plan> RecoveryBatch<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    fn authorization_count(&self) -> usize {
        self.members
            .iter()
            .map(RecoveryMember::authorization_count)
            .sum()
    }

    fn authorize(self) -> (Self, Option<ProxyOperation<behavior::Here, Worker, Plan>>) {
        let Self {
            ticket,
            trigger,
            release,
            members,
        } = self;
        match release {
            release @ (RestartReleaseState::Scheduling { .. }
            | RestartReleaseState::Waiting { .. }) => (
                Self {
                    ticket,
                    trigger,
                    release,
                    members,
                },
                None,
            ),
            RestartReleaseState::Ready => {
                let (members, operation) = authorize_recovery_sequence(members);
                (
                    Self {
                        ticket,
                        trigger,
                        release: RestartReleaseState::Ready,
                        members,
                    },
                    operation,
                )
            }
        }
    }

    fn accept_replacement_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        ControlFlow<(Self, ReplacementCompletion<Role, Worker, Plan>), Self>,
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        let Self {
            ticket,
            trigger,
            release,
            mut members,
        } = self;
        let mut examined = VecDeque::new();
        let mut settlement = settlement;
        while let Some(member) = members.pop_front() {
            match member.accept_replacement_operation(ticket, settlement) {
                Ok(ControlFlow::Continue(member)) => {
                    examined.push_back(member);
                    examined.append(&mut members);
                    return Ok(ControlFlow::Continue(Self {
                        ticket,
                        trigger,
                        release,
                        members: examined,
                    }));
                }
                Ok(ControlFlow::Break(completed)) => {
                    examined.append(&mut members);
                    return Ok(ControlFlow::Break((
                        Self {
                            ticket,
                            trigger,
                            release,
                            members: examined,
                        },
                        completed,
                    )));
                }
                Err((member, returned)) => {
                    examined.push_back(member);
                    settlement = returned;
                }
            }
        }
        Err((
            Self {
                ticket,
                trigger,
                release,
                members: examined,
            },
            settlement,
        ))
    }

    fn accept_recovery_report(
        self,
        report: ChildReport<crate::ProxyOutcome<Worker, Plan>>,
    ) -> Result<
        ControlFlow<(Self, ReplacementCompletion<Role, Worker, Plan>), Self>,
        (Self, ChildReport<crate::ProxyOutcome<Worker, Plan>>),
    > {
        let Self {
            ticket,
            trigger,
            release,
            mut members,
        } = self;
        let mut examined = VecDeque::new();
        let mut report = report;
        while let Some(member) = members.pop_front() {
            match member.accept_recovery_report(ticket, report) {
                Ok(ControlFlow::Continue(member)) => {
                    examined.push_back(member);
                    examined.append(&mut members);
                    return Ok(ControlFlow::Continue(Self {
                        ticket,
                        trigger,
                        release,
                        members: examined,
                    }));
                }
                Ok(ControlFlow::Break(completed)) => {
                    examined.append(&mut members);
                    return Ok(ControlFlow::Break((
                        Self {
                            ticket,
                            trigger,
                            release,
                            members: examined,
                        },
                        completed,
                    )));
                }
                Err((member, returned)) => {
                    examined.push_back(member);
                    report = returned;
                }
            }
        }
        Err((
            Self {
                ticket,
                trigger,
                release,
                members: examined,
            },
            report,
        ))
    }

    fn accept_schedule(
        self,
        input: ActionItemResult<ScheduleAfter>,
    ) -> RecoveryScheduleAdmission<Role, Worker, Plan> {
        let Self {
            ticket,
            trigger,
            release,
            members,
        } = self;
        match (release, input) {
            (
                RestartReleaseState::Scheduling { timer },
                SettledItem::Attempted(ItemSettlement::Accepted(scheduled)),
            ) if timer.id == scheduled.id && timer.generation == scheduled.generation => {
                RecoveryScheduleAdmission::Waiting(Self {
                    ticket,
                    trigger,
                    release: RestartReleaseState::Waiting { timer },
                    members,
                })
            }
            (
                release @ RestartReleaseState::Scheduling { timer },
                SettledItem::Attempted(ItemSettlement::Rejected { item, reason }),
            ) if timer.id == item.id && timer.generation == item.generation => {
                let prepared = match into_prepared_members(members) {
                    Ok(prepared) => prepared,
                    Err(members) => {
                        return RecoveryScheduleAdmission::Unrelated {
                            recovery: Self {
                                ticket,
                                trigger,
                                release,
                                members,
                            },
                            input: SettledItem::Attempted(ItemSettlement::Rejected {
                                item,
                                reason,
                            }),
                        };
                    }
                };
                let prepared = match PreparedRecovery::from_admitted(trigger, prepared) {
                    Ok(prepared) => prepared,
                    Err(prepared) => {
                        return RecoveryScheduleAdmission::Unrelated {
                            recovery: Self {
                                ticket,
                                trigger,
                                release,
                                members: prepared
                                    .into_iter()
                                    .map(PreparedParticipant::admit)
                                    .collect(),
                            },
                            input: SettledItem::Attempted(ItemSettlement::Rejected {
                                item,
                                reason,
                            }),
                        };
                    }
                };
                RecoveryScheduleAdmission::Rejected {
                    prepared,
                    request: item,
                    reason,
                }
            }
            (release, input) => RecoveryScheduleAdmission::Unrelated {
                recovery: Self {
                    ticket,
                    trigger,
                    release,
                    members,
                },
                input,
            },
        }
    }

    fn accept_elapsed(self, elapsed: TimerElapsed) -> Result<Self, (Self, TimerElapsed)> {
        let Self {
            ticket,
            trigger,
            release,
            members,
        } = self;
        match release {
            RestartReleaseState::Waiting { timer }
                if timer.id == elapsed.id && timer.generation == elapsed.generation =>
            {
                Ok(Self {
                    ticket,
                    trigger,
                    release: RestartReleaseState::Ready,
                    members,
                })
            }
            release => Err((
                Self {
                    ticket,
                    trigger,
                    release,
                    members,
                },
                elapsed,
            )),
        }
    }
}

impl<Role, Worker, Plan> RecoveryRosterOwner<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn append_status(
        &self,
        members: &mut Vec<(RosterPosition, MemberStatus<Worker::Protocol>)>,
    ) {
        match self {
            Self::Preparing(recovery) => {
                members.extend(
                    recovery
                        .before_trigger
                        .iter()
                        .map(|member| (member.position(), UnavailablePhase::Recovering.status())),
                );
                members.push((
                    recovery.trigger.role.position(),
                    UnavailablePhase::Recovering.status(),
                ));
                members.extend(
                    recovery
                        .after_trigger
                        .iter()
                        .map(|member| (member.position(), UnavailablePhase::Recovering.status())),
                );
            }
            Self::Admitted(recovery) => members.extend(
                recovery
                    .members
                    .iter()
                    .map(|member| (member.position(), UnavailablePhase::Recovering.status())),
            ),
        }
    }

    pub(super) fn find_role(&self, expected: &Role) -> Option<&Role>
    where
        Role: Eq,
    {
        match self {
            Self::Preparing(recovery) => recovery
                .before_trigger
                .iter()
                .map(RecoveryParticipant::role)
                .chain(core::iter::once(recovery.trigger.role.role()))
                .chain(recovery.after_trigger.iter().map(RecoveryParticipant::role))
                .find(|role| *role == expected),
            Self::Admitted(recovery) => recovery
                .members
                .iter()
                .map(RecoveryMember::role)
                .find(|role| *role == expected),
        }
    }

    pub(super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match self {
            Self::Preparing(recovery) => recovery
                .before_trigger
                .iter()
                .find_map(|member| member.live_proxy_role(child))
                .or_else(|| recovery.trigger.live_proxy_role(child))
                .or_else(|| {
                    recovery
                        .after_trigger
                        .iter()
                        .find_map(|member| member.live_proxy_role(child))
                }),
            Self::Admitted(recovery) => recovery
                .members
                .iter()
                .find_map(|member| member.live_proxy_role(child)),
        }
    }

    pub(super) fn first_roster_position(&self) -> Option<RosterPosition> {
        match self {
            Self::Preparing(recovery) => recovery
                .before_trigger
                .iter()
                .map(RecoveryParticipant::position)
                .chain(core::iter::once(recovery.trigger.role.position()))
                .chain(
                    recovery
                        .after_trigger
                        .iter()
                        .map(RecoveryParticipant::position),
                )
                .min(),
            Self::Admitted(recovery) => recovery.members.iter().map(RecoveryMember::position).min(),
        }
    }

    pub(super) fn last_roster_position(&self) -> Option<RosterPosition> {
        match self {
            Self::Preparing(recovery) => recovery
                .before_trigger
                .iter()
                .map(RecoveryParticipant::position)
                .chain(core::iter::once(recovery.trigger.role.position()))
                .chain(
                    recovery
                        .after_trigger
                        .iter()
                        .map(RecoveryParticipant::position),
                )
                .max(),
            Self::Admitted(recovery) => recovery.members.iter().map(RecoveryMember::position).max(),
        }
    }

    pub(super) fn authorization_count(&self) -> usize {
        match self {
            Self::Preparing(_) => 0,
            Self::Admitted(recovery) => recovery.authorization_count(),
        }
    }

    pub(super) fn authorize(self) -> (Self, Option<ProxyOperation<behavior::Here, Worker, Plan>>) {
        match self {
            Self::Admitted(recovery) => {
                let (recovery, operation) = recovery.authorize();
                (Self::Admitted(recovery), operation)
            }
            recovery @ Self::Preparing(_) => (recovery, None),
        }
    }

    fn accept_elapsed(self, elapsed: TimerElapsed) -> Result<Self, (Self, TimerElapsed)> {
        match self {
            Self::Admitted(recovery) => match recovery.accept_elapsed(elapsed) {
                Ok(recovery) => Ok(Self::Admitted(recovery)),
                Err((recovery, elapsed)) => Err((Self::Admitted(recovery), elapsed)),
            },
            recovery @ Self::Preparing(_) => Err((recovery, elapsed)),
        }
    }
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn accept_replacement_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        ControlFlow<
            (
                Vec<RosterOwner<Role, Worker, Plan>>,
                ReplacementCompletion<Role, Worker, Plan>,
            ),
            Self,
        >,
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        let Self::Operating(mut owners) = self else {
            return Err((self, settlement));
        };
        let mut settlement = settlement;
        for declaration in 0..owners.len() {
            let owner = owners.remove(declaration);
            match owner {
                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)) => {
                    match recovery.accept_replacement_operation(settlement) {
                        Ok(ControlFlow::Continue(recovery)) => {
                            owners.insert(
                                declaration,
                                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                            );
                            return Ok(ControlFlow::Continue(Self::Operating(owners)));
                        }
                        Ok(ControlFlow::Break((recovery, completed))) => {
                            match recovery.members.front() {
                                Some(_) => owners.insert(
                                    declaration,
                                    RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                                ),
                                None => {}
                            }
                            return Ok(ControlFlow::Break((owners, completed)));
                        }
                        Err((recovery, returned)) => {
                            owners.insert(
                                declaration,
                                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                            );
                            settlement = returned;
                        }
                    }
                }
                owner => owners.insert(declaration, owner),
            }
        }
        Err((Self::Operating(owners), settlement))
    }

    pub(super) fn accept_recovery_report(
        self,
        report: ChildReport<crate::ProxyOutcome<Worker, Plan>>,
    ) -> Result<
        ControlFlow<
            (
                Vec<RosterOwner<Role, Worker, Plan>>,
                ReplacementCompletion<Role, Worker, Plan>,
            ),
            Self,
        >,
        (Self, ChildReport<crate::ProxyOutcome<Worker, Plan>>),
    > {
        let Self::Operating(mut owners) = self else {
            return Err((self, report));
        };
        let mut report = report;
        for declaration in 0..owners.len() {
            let owner = owners.remove(declaration);
            match owner {
                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)) => {
                    match recovery.accept_recovery_report(report) {
                        Ok(ControlFlow::Continue(recovery)) => {
                            owners.insert(
                                declaration,
                                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                            );
                            return Ok(ControlFlow::Continue(Self::Operating(owners)));
                        }
                        Ok(ControlFlow::Break((recovery, completed))) => {
                            match recovery.members.front() {
                                Some(_) => owners.insert(
                                    declaration,
                                    RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                                ),
                                None => {}
                            }
                            return Ok(ControlFlow::Break((owners, completed)));
                        }
                        Err((recovery, returned)) => {
                            owners.insert(
                                declaration,
                                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                            );
                            report = returned;
                        }
                    }
                }
                owner => owners.insert(declaration, owner),
            }
        }
        Err((Self::Operating(owners), report))
    }

    pub(super) fn accept_restart_schedule(
        self,
        input: ActionItemResult<ScheduleAfter>,
    ) -> RestartScheduleAdmission<Role, Worker, Plan> {
        let Self::Operating(mut owners) = self else {
            return RestartScheduleAdmission::Unrelated {
                roster: self,
                input,
            };
        };
        let mut input = input;
        for declaration in 0..owners.len() {
            let owner = owners.remove(declaration);
            match owner {
                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)) => {
                    match recovery.accept_schedule(input) {
                        RecoveryScheduleAdmission::Waiting(recovery) => {
                            owners.insert(
                                declaration,
                                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                            );
                            return RestartScheduleAdmission::Waiting(Self::Operating(owners));
                        }
                        RecoveryScheduleAdmission::Rejected {
                            prepared,
                            request,
                            reason,
                        } => {
                            let trigger = prepared.trigger.role.name();
                            return RestartScheduleAdmission::Rejected(RejectedRestartSchedule {
                                failed: retain_rejected_schedule(owners, prepared),
                                trigger,
                                request,
                                reason,
                            });
                        }
                        RecoveryScheduleAdmission::Unrelated {
                            recovery,
                            input: returned,
                        } => {
                            owners.insert(
                                declaration,
                                RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                            );
                            input = returned;
                        }
                    }
                }
                owner => owners.insert(declaration, owner),
            }
        }
        RestartScheduleAdmission::Unrelated {
            roster: Self::Operating(owners),
            input,
        }
    }

    pub(super) fn accept_restart_elapsed(
        self,
        elapsed: TimerElapsed,
    ) -> Result<Self, (Self, TimerElapsed)> {
        let Self::Operating(mut owners) = self else {
            return Err((self, elapsed));
        };
        let mut elapsed = elapsed;
        for declaration in 0..owners.len() {
            let owner = owners.remove(declaration);
            match owner {
                RosterOwner::Recovery(recovery) => match recovery.accept_elapsed(elapsed) {
                    Ok(recovery) => {
                        owners.insert(declaration, RosterOwner::Recovery(recovery));
                        return Ok(Self::Operating(owners));
                    }
                    Err((recovery, returned)) => {
                        owners.insert(declaration, RosterOwner::Recovery(recovery));
                        elapsed = returned;
                    }
                },
                owner => owners.insert(declaration, owner),
            }
        }
        Err((Self::Operating(owners), elapsed))
    }
}

enum RecoveryPreparationDecision<Role, Worker, Plan, Source>
where
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Prepared {
        recovery: PreparedRecovery<Role, Worker, Plan>,
        source: Source,
    },
    Failed {
        restored_peers: Vec<RosterOwner<Role, Worker, Plan>>,
        stopped_trigger: StoppedMember<Role, Worker, Plan>,
        source: Source,
        diagnostic: WorkerPreparationFailure<
            Role,
            Worker,
            Plan,
            Source::WorkerRejection,
            Source::SourceRejection,
        >,
    },
    Rejected {
        recovery: PreparingRecovery<Role, Worker, Plan>,
        preparation: super::WorkerPreparation<Source, Role, Worker, Plan>,
    },
}

impl<Role, Worker, Plan> PreparingRecovery<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) fn begin<Source, PreparationReturn>(
        preparation: RecoveryPreparation<Source>,
        before_trigger: Vec<RecoveryParticipant<Role, Worker, Plan>>,
        trigger: StoppedMember<Role, Worker, Plan>,
        after_trigger: Vec<RecoveryParticipant<Role, Worker, Plan>>,
    ) -> (
        SupervisorRecovery<Source, PreparationReturn>,
        Self,
        PrepareWorkers<Source, Role, Worker, Plan>,
    )
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Role: Send + Sync,
        Worker: Send,
    {
        let (first, remaining) = Self::selected_names(&before_trigger, &trigger, &after_trigger);
        let (recovery, expected, request) = preparation.begin(first, remaining);
        (
            recovery,
            Self::selected(before_trigger, trigger, after_trigger, expected),
            request,
        )
    }

    pub(super) fn selected(
        before_trigger: Vec<RecoveryParticipant<Role, Worker, Plan>>,
        trigger: StoppedMember<Role, Worker, Plan>,
        after_trigger: Vec<RecoveryParticipant<Role, Worker, Plan>>,
        expected: PreparationTicket,
    ) -> Self {
        Self {
            expected,
            before_trigger,
            trigger,
            after_trigger,
        }
    }

    fn selected_names(
        before_trigger: &[RecoveryParticipant<Role, Worker, Plan>],
        trigger: &StoppedMember<Role, Worker, Plan>,
        after_trigger: &[RecoveryParticipant<Role, Worker, Plan>],
    ) -> (RoleName<Role>, Vec<RoleName<Role>>) {
        match before_trigger.split_first() {
            Some((first, before)) => (
                first.name(),
                before
                    .iter()
                    .map(RecoveryParticipant::name)
                    .chain(core::iter::once(trigger.role.name()))
                    .chain(after_trigger.iter().map(RecoveryParticipant::name))
                    .collect(),
            ),
            None => (
                trigger.role.name(),
                after_trigger
                    .iter()
                    .map(RecoveryParticipant::name)
                    .collect(),
            ),
        }
    }

    pub(super) fn into_shutdown(
        self,
    ) -> (
        PreparationTicket,
        Vec<RecoveryParticipant<Role, Worker, Plan>>,
        StoppedMember<Role, Worker, Plan>,
        Vec<RecoveryParticipant<Role, Worker, Plan>>,
    ) {
        let Self {
            expected,
            before_trigger,
            trigger,
            after_trigger,
        } = self;
        (expected, before_trigger, trigger, after_trigger)
    }

    fn accepts_result<Source>(
        &self,
        input: &ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    ) -> bool
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Role: Send + Sync,
        Worker: Send,
    {
        preparation_result_accepts(input, &self.expected)
    }

    fn accept<Source>(
        self,
        preparation: super::WorkerPreparation<Source, Role, Worker, Plan>,
    ) -> RecoveryPreparationDecision<Role, Worker, Plan, Source>
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Worker: Send,
    {
        if !preparation.accepts(&self.expected) {
            return RecoveryPreparationDecision::Rejected {
                recovery: self,
                preparation,
            };
        }
        let (ticket, outcome) = preparation.into_parts();
        match outcome {
            WorkerPreparationOutcome::Prepared {
                source,
                mut members,
            } => {
                let selected = match self
                    .before_trigger
                    .len()
                    .checked_add(1)
                    .and_then(|selected| selected.checked_add(self.after_trigger.len()))
                {
                    Some(selected) => selected,
                    None => {
                        return RecoveryPreparationDecision::Rejected {
                            recovery: self,
                            preparation: super::WorkerPreparation::from_parts(
                                ticket,
                                WorkerPreparationOutcome::Prepared { source, members },
                            ),
                        };
                    }
                };
                match members.len().cmp(&selected) {
                    core::cmp::Ordering::Equal => {}
                    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
                        return RecoveryPreparationDecision::Rejected {
                            recovery: self,
                            preparation: super::WorkerPreparation::from_parts(
                                ticket,
                                WorkerPreparationOutcome::Prepared { source, members },
                            ),
                        };
                    }
                }
                let mut trigger_and_after =
                    members.split_off(self.before_trigger.len()).into_iter();
                let trigger_prepared = match trigger_and_after.next() {
                    Some(prepared) => prepared,
                    None => {
                        return RecoveryPreparationDecision::Rejected {
                            recovery: self,
                            preparation: super::WorkerPreparation::from_parts(
                                ticket,
                                WorkerPreparationOutcome::Prepared { source, members },
                            ),
                        };
                    }
                };
                let Self {
                    expected: _,
                    before_trigger,
                    trigger,
                    after_trigger,
                } = self;
                let before_trigger = before_trigger
                    .into_iter()
                    .zip(members)
                    .map(|(participant, prepared)| participant.prepared(prepared.submission))
                    .collect();
                let after_trigger = after_trigger
                    .into_iter()
                    .zip(trigger_and_after)
                    .map(|(participant, prepared)| participant.prepared(prepared.submission))
                    .collect();
                RecoveryPreparationDecision::Prepared {
                    recovery: PreparedRecovery {
                        before_trigger,
                        trigger,
                        trigger_submission: trigger_prepared.submission,
                        after_trigger,
                    },
                    source,
                }
            }
            WorkerPreparationOutcome::WorkerRejected {
                source,
                prepared,
                failed_role,
                reason,
                remaining,
            } => {
                let (restored_peers, trigger_role, stopped_trigger) =
                    self.restore_peers_after_preparation_failure();
                RecoveryPreparationDecision::Failed {
                    restored_peers,
                    stopped_trigger,
                    source,
                    diagnostic: WorkerPreparationFailure::worker_rejected(
                        trigger_role,
                        prepared,
                        failed_role,
                        reason,
                        remaining,
                    ),
                }
            }
        }
    }

    fn restore_peers_after_preparation_failure(
        self,
    ) -> (
        Vec<RosterOwner<Role, Worker, Plan>>,
        RoleName<Role>,
        StoppedMember<Role, Worker, Plan>,
    ) {
        let Self {
            expected: _,
            mut before_trigger,
            trigger,
            after_trigger,
        } = self;
        before_trigger.extend(after_trigger);
        let restored_peers = before_trigger
            .into_iter()
            .map(RecoveryParticipant::release)
            .collect();
        let trigger_role = trigger.role.name();
        (restored_peers, trigger_role, trigger)
    }
}

impl AwaitingWorkerSource {
    pub(super) fn decide_prepared<Role, Worker, Plan, Source, PreparationReturn>(
        self,
        mut owners: Vec<RosterOwner<Role, Worker, Plan>>,
        position: usize,
        mut prepared: PreparedRecovery<Role, Worker, Plan>,
        source: Source,
    ) -> PreparedRecoveryDecision<Role, Worker, Plan, Source, PreparationReturn>
    where
        Source: WorkerSource<Role, Worker, Plan>,
        Role: Send + Sync,
        Worker: Behavior + Send,
        Plan: super::ActivationPlan,
        BehaviorAddr<Worker>: EndpointAddress,
        StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    {
        let Self {
            settings,
            budget,
            correlations,
        } = self;
        let replacements = prepared.worker_count();
        let recovery_ticket = match correlations.propose_recovery() {
            Ok(proposal) => proposal,
            Err(denial) => {
                let (failed, diagnostic) = deny_prepared_recovery(owners, prepared, denial.reason);
                return PreparedRecoveryDecision::Denied {
                    failed,
                    recovery: available_recovery(settings, source, budget, denial.unchanged),
                    diagnostic,
                };
            }
        };
        let observed_at = prepared.trigger.stopped.at;
        match admit_restart(
            &mut prepared.trigger.role,
            budget,
            settings.limit,
            settings.release,
            observed_at,
            replacements,
        ) {
            RestartAdmission::Proposed(proposal) => {
                match recovery_ticket.select_release(proposal.release()) {
                    Ok(proposed_correlations) => {
                        let AcceptedRecoveryCorrelations {
                            correlations,
                            recovery: ticket,
                            timer,
                            schedule,
                        } = proposed_correlations.accept();
                        let budget = proposal.accept();
                        let recovery = prepared.admit(ticket, timer);
                        owners.insert(
                            position,
                            RosterOwner::Recovery(RecoveryRosterOwner::Admitted(recovery)),
                        );
                        PreparedRecoveryDecision::Admitted {
                            roster: FixedRoster::Operating(owners),
                            recovery: available_recovery(settings, source, budget, correlations),
                            schedule,
                        }
                    }
                    Err(denial) => {
                        let budget = proposal.decline();
                        let (failed, diagnostic) =
                            deny_prepared_recovery(owners, prepared, denial.reason);
                        PreparedRecoveryDecision::Denied {
                            failed,
                            recovery: available_recovery(
                                settings,
                                source,
                                budget,
                                denial.unchanged,
                            ),
                            diagnostic,
                        }
                    }
                }
            }
            RestartAdmission::Denied { budget, reason } => {
                let correlations = recovery_ticket.decline();
                let (failed, diagnostic) = deny_prepared_recovery(owners, prepared, reason);
                PreparedRecoveryDecision::Denied {
                    failed,
                    recovery: available_recovery(settings, source, budget, correlations),
                    diagnostic,
                }
            }
        }
    }
}

fn deny_prepared_recovery<Role, Worker, Plan>(
    owners: Vec<RosterOwner<Role, Worker, Plan>>,
    rejected: PreparedRecovery<Role, Worker, Plan>,
    reason: RecoveryDenialReason,
) -> (
    RecoveryFailure<Role, Worker, Plan>,
    RecoveryDenied<Role, Worker>,
)
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let PreparedRecovery {
        mut before_trigger,
        trigger,
        trigger_submission,
        after_trigger,
    } = rejected;
    before_trigger.extend(after_trigger);
    let StoppedMember {
        role,
        creation,
        proxy,
        worker,
        readiness,
        stopped,
    } = trigger;
    let trigger = EmptyMember {
        role,
        creation,
        proxy,
        worker,
        readiness,
        stopped: None,
    };
    let role = trigger.role.name();
    (
        RecoveryFailure {
            owners,
            replacements: UnissuedReplacements {
                peers: before_trigger,
                trigger,
                trigger_submission,
            },
        },
        RecoveryDenied::new(role, stopped, reason),
    )
}

fn retain_rejected_schedule<Role, Worker, Plan>(
    owners: Vec<RosterOwner<Role, Worker, Plan>>,
    rejected: PreparedRecovery<Role, Worker, Plan>,
) -> RecoveryFailure<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: super::ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    let PreparedRecovery {
        mut before_trigger,
        trigger,
        trigger_submission,
        after_trigger,
    } = rejected;
    before_trigger.extend(after_trigger);
    RecoveryFailure {
        owners,
        replacements: UnissuedReplacements {
            peers: before_trigger,
            trigger: trigger.retain_stop(),
            trigger_submission,
        },
    }
}

fn available_recovery<Source, PreparationReturn>(
    settings: RecoverySettings,
    source: Source,
    budget: RestartBudget,
    correlations: RecoveryCorrelations,
) -> SupervisorRecovery<Source, PreparationReturn> {
    SupervisorRecovery {
        state: SupervisorRecoveryState::Automatic(AutomaticRecovery {
            settings,
            source: WorkerSourceCustody::Available(source),
            budget,
            correlations,
        }),
    }
}

pub(super) enum RecoveryRetirement {
    Ready,
    WaitingForWorkerSource,
}

impl<Source, PreparationReturn> SupervisorRecovery<Source, PreparationReturn> {
    pub(super) fn retirement(&self) -> RecoveryRetirement {
        match &self.state {
            SupervisorRecoveryState::Automatic(AutomaticRecovery {
                source: WorkerSourceCustody::PreparingWorkers,
                ..
            }) => RecoveryRetirement::WaitingForWorkerSource,
            SupervisorRecoveryState::Automatic(AutomaticRecovery {
                source: WorkerSourceCustody::Available(_),
                ..
            })
            | SupervisorRecoveryState::Automatic(AutomaticRecovery {
                source: WorkerSourceCustody::Retirement(_),
                ..
            })
            | SupervisorRecoveryState::Temporary => RecoveryRetirement::Ready,
        }
    }
}
