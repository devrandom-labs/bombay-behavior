//! Construction values for one fixed, ordered supervisor roster.

use core::mem;
use core::ops::ControlFlow;

use crate::{
    DeliveryRoute, DiagnosticAction, DiagnosticDisposition, ObserveChild, ReplyRoute, ScheduleAfter,
};
use behavior::{
    ActionItemResult, Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr,
    BehaviorBase, Births, ChildHead, CreationId, CreationSequence, Creations, CreationsSettled,
    EndpointAddress, EstablishedActor, InitializationTurn, InterpreterRequests, MessageProtocol,
    Never, SendEffects, SourceActions, Step, User,
};

use crate::lifecycle::ActorDrainPolicy;
use crate::proxy::{ProxyOperation, StableProxy};
use crate::templates::roster::RoleName;
use crate::worker::{PreparedWorker, RestartLimit, RestartRelease, StartupLimit, StartupPlan};

mod config;
mod diagnostic;
mod event;
mod lifecycle;
mod member;
mod protocol;
mod proxy;
mod recovery;
mod requests;
mod restart;
mod role;
mod shutdown;

pub use config::{Config, FixedBuilder, FixedConstructionRejected, new};

use crate::templates::worker::{PrepareWorkers, WorkerPreparation, WorkerSource};
pub use diagnostic::{
    FixedDiagnostic, ProxyInputFailure, ProxyOutcomeFailure, RecoveryDenied,
    RestartScheduleFailure, WorkerPreparationFailure, WorkerPreparationFailureReason,
    WorkerUnavailable,
};
pub use event::FixedSupervisorEvent;
pub use lifecycle::{FixedLifecycle, FixedLifecycleEvent, FixedLifecycleRoute};
pub use protocol::{CapabilityResult, FixedCommand, FixedSnapshot, MemberStatus, UnavailablePhase};
pub use requests::FixedSupervisorRequests;
pub use restart::RecoveryDenialReason;

use member::{OnlineMember, RosterOwner};
use proxy::{FixedRoster, InitialProxyDecision};
use recovery::{
    AcceptedWorkerStop, PreparationAcceptance, PreparedRecoveryDecision, RecoveryChoice,
    RecoveryFailure, RecoveryTicket, ReplacementCompletion, RestartScheduleAdmission,
    SupervisorRecovery, UnrecoveredMember, WorkerPreparationRetirement, WorkerStopDecision,
};
use shutdown::FixedShutdown;

/// Ordered member selection for one coordinated automatic recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Strategy {
    /// Recover only the role whose worker stopped.
    OneForOne,
    /// Recover every restartable role in declaration order.
    OneForAll,
    /// Recover the stopped role and every restartable role declared after it.
    RestForOne,
}

/// Member selection, restart allowance, and delay for automatic recovery.
///
/// These choices are independent of the worker source and eligibility selected
/// by [`Recovery::permanent`] or [`Recovery::transient`]. Each delay has already
/// been validated by [`RestartRelease`]; constructing this product performs no
/// work, starts no worker, and changes no restart budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryPolicy {
    /// Select the ordered roles that participate in a recovery.
    pub strategy: Strategy,
    /// Bound replacement workers admitted in one sliding window.
    pub limit: RestartLimit,
    /// Select when admitted replacements may be released.
    pub delay: RestartRelease,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecoveryDecision<Source> {
    Permanent {
        source: Source,
        policy: RecoveryPolicy,
    },
    Transient {
        source: Source,
        policy: RecoveryPolicy,
    },
    Temporary,
}

/// Complete eligibility, worker source, and policy for recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Recovery<Source> {
    decision: RecoveryDecision<Source>,
}

impl<Source> Recovery<Source> {
    /// Recover after normal or abnormal worker stop through this source.
    #[must_use]
    pub const fn permanent(source: Source, policy: RecoveryPolicy) -> Self {
        Self {
            decision: RecoveryDecision::Permanent { source, policy },
        }
    }

    /// Recover only after abnormal worker stop through this source.
    #[must_use]
    pub const fn transient(source: Source, policy: RecoveryPolicy) -> Self {
        Self {
            decision: RecoveryDecision::Transient { source, policy },
        }
    }
}

impl Recovery<behavior::Never> {
    /// Never recover a stopped worker automatically.
    #[must_use]
    pub const fn temporary() -> Self {
        Self {
            decision: RecoveryDecision::Temporary,
        }
    }
}

/// Fixed-topology reaction when a stable proxy can no longer serve its role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReaction {
    /// Retire only the unavailable roster member.
    RetireMember,
    /// Stop the complete supervisor.
    StopSupervisor,
}

/// A fixed supervisor whose complete initial roster has been prepared.
pub struct FixedSupervisor<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute>
where
    Role: Send + Sync,
    Source: WorkerSource<Role, Worker, Plan>,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    roster: FixedRoster<Role, Worker, Plan>,
    creations: CreationSequence,
    startup: StartupLimit,
    recovery: SupervisorRecovery<Source, WorkerPreparationRetirement<Source, Role, Worker, Plan>>,
    failure_reaction: FailureReaction,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
    lifecycle: Option<LifecycleRoute>,
}

/// Controlled rejection while establishing a fixed supervisor's proxy roster.
///
/// No variant loses a prepared worker submission or a returned runtime input.
/// Namespace exhaustion retires every locally reserved child route and returns
/// the complete prepared roster partitioned around the exact rejected role.
pub enum FixedSupervisorError<Role, Worker, Plan, PreparationStart, PreparationReturn>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// Initialization was invoked after the new roster had already advanced.
    InitializationUnavailable,
    /// The supervisor cannot issue IDs for its complete initial proxy batch.
    ProxyCreationsExhausted {
        /// Complete prepared roster in declaration order.
        members: Vec<PreparedWorker<Role, Worker, Plan>>,
    },
    /// Bombay returned the complete initial proxy batch before every proxy committed.
    ProxyCreationRejected {
        /// Exact routed or unrouted batch returned by the interpreter.
        proxies: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    },
    /// A typed runtime or application input was not valid in the current phase.
    InputRejected {
        /// Complete returned input, unchanged.
        input: FixedSupervisorEvent<Role, Worker, Plan, PreparationStart, PreparationReturn>,
    },
    /// The stored roster contradicted its aggregate phase.
    ///
    /// This is controlled corruption evidence rather than a production panic;
    /// every prepared member remains owned here.
    RosterStateRejected {
        /// Complete prepared roster in declaration order.
        members: Vec<PreparedWorker<Role, Worker, Plan>>,
    },
    /// Stored occupied authorizations exceeded the validated maximum.
    AuthorizationStateRejected {
        /// Occupied operations observed in member state.
        occupied: usize,
        /// Positive configured maximum.
        maximum: usize,
    },
    /// An operating-roster transition produced a non-creating startup member.
    OperatingStateRejected,
    /// An accepted initial proxy outcome could not rejoin its operating roster.
    InitialOutcomeContradiction {
        /// Complete exact proxy report that exposed the contradiction.
        report: behavior::ChildReport<crate::proxy::ProxyOutcome<Worker, Plan>>,
    },
}

impl<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute> BehaviorBase
    for FixedSupervisor<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute>
where
    Role: Send + Sync,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute>
    FixedSupervisor<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute>
where
    Role: Eq + Send + Sync,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    EstablishedActor<StableProxy<Worker, Plan>>: Clone + Send,
    DiagnosticRoute: crate::DiagnosticRoute<FixedDiagnostic<Role, Worker, Plan, Source>> + Clone,
    LifecycleRoute: FixedLifecycleRoute<Role, Worker, Plan> + Clone,
    LifecycleRoute::Sends: behavior::SendsFor<
            FixedSupervisorEvent<
                Role,
                Worker,
                Plan,
                ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
                WorkerPreparation<Source, Role, Worker, Plan>,
            >,
        >,
{
    fn authorize_roster(
        &self,
        roster: FixedRoster<Role, Worker, Plan>,
    ) -> Result<
        (
            FixedRoster<Role, Worker, Plan>,
            SourceActions<ProxyOperation<behavior::Here, Worker, Plan>>,
        ),
        (
            FixedRoster<Role, Worker, Plan>,
            FixedSupervisorError<
                Role,
                Worker,
                Plan,
                ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
                WorkerPreparation<Source, Role, Worker, Plan>,
            >,
        ),
    > {
        let (roster, operations) = match roster.authorize(self.startup.maximum()) {
            Ok(authorized) => authorized,
            Err(rejected) => return Err(rejected),
        };
        let mut proxy_operations = SourceActions::empty();
        for operation in operations {
            proxy_operations.send(operation);
        }
        Ok((roster, proxy_operations))
    }

    fn authorize_waiting(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let (next, proxy_operations) = match self.authorize_roster(roster) {
            Ok(authorized) => authorized,
            Err((next, rejection)) => {
                self.roster = next;
                return Err(rejection);
            }
        };
        self.roster = next;
        let mut requests = FixedSupervisorRequests::empty();
        requests.proxy_operations = proxy_operations;
        Ok(Actions::send(requests))
    }

    fn member_retired(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        role: RoleName<Role>,
    ) -> BehaviorActed<Self> {
        let mut actions = match self.authorize_waiting(roster) {
            Ok(actions) => actions,
            Err(rejection) => return Err(rejection),
        };
        match self.lifecycle.clone() {
            Some(route) => {
                actions.sends.lifecycle = route.deliver(FixedLifecycle::member_retired(role));
            }
            None => {}
        }
        Ok(actions)
    }

    fn admit_recovery(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        recovery: SupervisorRecovery<
            Source,
            WorkerPreparationRetirement<Source, Role, Worker, Plan>,
        >,
        schedule: Option<ScheduleAfter>,
    ) -> BehaviorActed<Self> {
        let (roster, operations) = match roster.authorize(self.startup.maximum()) {
            Ok(authorized) => authorized,
            Err((roster, rejection)) => {
                self.roster = roster;
                self.recovery = recovery;
                return Err(rejection);
            }
        };
        self.roster = roster;
        self.recovery = recovery;
        let mut proxy_operations = SourceActions::empty();
        for operation in operations {
            proxy_operations.send(operation);
        }
        let mut restart_schedules = SourceActions::empty();
        match schedule {
            Some(schedule) => restart_schedules.send(schedule),
            None => {}
        }
        let mut requests = FixedSupervisorRequests::empty();
        requests.proxy_operations = proxy_operations;
        requests.restart_schedules = restart_schedules;
        Ok(Actions::send(requests))
    }

    fn accept_restart_schedule(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        input: ActionItemResult<ScheduleAfter>,
    ) -> BehaviorActed<Self> {
        let roster = match roster {
            FixedRoster::ShuttingDown(shutdown) => {
                return match shutdown.accept_schedule(input) {
                    Ok(shutdown) => self.retain_shutdown(shutdown, Vec::new()),
                    Err((shutdown, input)) => {
                        self.roster = FixedRoster::ShuttingDown(shutdown);
                        Err(FixedSupervisorError::InputRejected {
                            input: FixedSupervisorEvent::RestartScheduleSettled(input),
                        })
                    }
                };
            }
            roster => roster,
        };
        match roster.accept_restart_schedule(input) {
            RestartScheduleAdmission::Waiting(roster) => self.authorize_waiting(roster),
            RestartScheduleAdmission::Rejected(rejected) => {
                let (failed, diagnostic) = rejected.into_parts();
                self.apply_recovery_failure(
                    failed,
                    FixedDiagnostic::RestartScheduleFailed(diagnostic),
                )
            }
            RestartScheduleAdmission::Unrelated { roster, input } => {
                self.roster = roster;
                Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::RestartScheduleSettled(input),
                })
            }
        }
    }

    fn apply_recovery_failure(
        &mut self,
        failed: RecoveryFailure<Role, Worker, Plan>,
        diagnostic: FixedDiagnostic<Role, Worker, Plan, Source>,
    ) -> BehaviorActed<Self> {
        match (&self.diagnostics, self.failure_reaction) {
            (DiagnosticDisposition::Terminate, _) => {
                self.roster = failed.terminate();
                let mut requests = FixedSupervisorRequests::empty();
                requests.diagnostics =
                    InterpreterRequests::one(DiagnosticAction::terminal(diagnostic));
                Ok(Actions::new(
                    requests,
                    Creations::empty(),
                    Step::Stop(behavior::Stopped),
                ))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::RetireMember) => {
                let (roster, operation) = failed.retire_member();
                self.roster = roster;
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(operation);
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(Actions::send(requests))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::StopSupervisor) => {
                let (shutdown, operations, schedule) = failed.stop_supervisor(self.actor_drain);
                self.roster = FixedRoster::ShuttingDown(shutdown);
                let mut proxy_operations = SourceActions::empty();
                for operation in operations {
                    proxy_operations.send(operation);
                }
                let mut restart_schedules = SourceActions::empty();
                match schedule {
                    Some(schedule) => restart_schedules.send(schedule),
                    None => {}
                }
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.restart_schedules = restart_schedules;
                requests.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(Actions::send(requests))
            }
        }
    }

    fn accept_restart_elapsed(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        elapsed: crate::TimerElapsed,
    ) -> BehaviorActed<Self> {
        let roster = match roster {
            FixedRoster::ShuttingDown(shutdown) => {
                return match shutdown.accept_elapsed(elapsed) {
                    Ok(shutdown) => self.retain_shutdown(shutdown, Vec::new()),
                    Err((shutdown, elapsed)) => {
                        self.roster = FixedRoster::ShuttingDown(shutdown);
                        Err(FixedSupervisorError::InputRejected {
                            input: FixedSupervisorEvent::RestartElapsed(elapsed),
                        })
                    }
                };
            }
            roster => roster,
        };
        match roster.accept_restart_elapsed(elapsed) {
            Ok(roster) => self.authorize_waiting(roster),
            Err((roster, elapsed)) => {
                self.roster = roster;
                Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::RestartElapsed(elapsed),
                })
            }
        }
    }

    fn retain_shutdown(
        &mut self,
        shutdown: FixedShutdown<Role, Worker, Plan>,
        operations: Vec<ProxyOperation<behavior::Here, Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        let (shutdown, become_) = shutdown.into_step(self.recovery.retirement());
        self.roster = FixedRoster::ShuttingDown(shutdown);
        let mut proxy_operations = SourceActions::empty();
        for operation in operations {
            proxy_operations.send(operation);
        }
        let mut requests = FixedSupervisorRequests::empty();
        requests.proxy_operations = proxy_operations;
        Ok(Actions::new(requests, Creations::empty(), become_))
    }

    fn accept_proxy_birth(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        proxies: CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        match roster {
            FixedRoster::ShuttingDown(shutdown) => match shutdown.accept_births(proxies) {
                Ok((shutdown, operations)) => self.retain_shutdown(shutdown, operations),
                Err((shutdown, proxies)) => {
                    self.roster = FixedRoster::ShuttingDown(shutdown);
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::ProxyCreationsSettled(proxies),
                    })
                }
            },
            roster => {
                let next = match roster.accept_births(proxies) {
                    Ok(next) => next,
                    Err((next, rejection)) => {
                        self.roster = next;
                        return Err(rejection);
                    }
                };
                self.authorize_waiting(next)
            }
        }
    }

    fn accept_proxy_operation(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        settlement: crate::ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        match roster {
            FixedRoster::ShuttingDown(shutdown) => match shutdown.accept_operation(settlement) {
                Ok(shutdown) => self.retain_shutdown(shutdown, Vec::new()),
                Err((shutdown, settlement)) => {
                    self.roster = FixedRoster::ShuttingDown(shutdown);
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::ProxyInputSettled(settlement),
                    })
                }
            },
            roster => {
                let (roster, settlement) = match roster.accept_replacement_operation(settlement) {
                    Ok(ControlFlow::Continue(roster)) => return self.authorize_waiting(roster),
                    Ok(ControlFlow::Break((owners, completed))) => {
                        return self.accept_replacement_completion(owners, completed);
                    }
                    Err(returned) => returned,
                };
                let (next, retired) = match roster.accept_operation(settlement) {
                    Ok(next) => next,
                    Err((next, rejection)) => {
                        self.roster = next;
                        return Err(rejection);
                    }
                };
                match retired {
                    Some(role) => self.member_retired(next, role),
                    None => self.authorize_waiting(next),
                }
            }
        }
    }

    fn accept_proxy_report(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        report: behavior::ChildReport<crate::proxy::ProxyOutcome<Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        let behavior::ChildReport {
            child,
            report: outcome,
        } = report;
        let report = match outcome {
            crate::proxy::ProxyOutcome::Unavailable {
                sender,
                phase,
                command,
            } => {
                return self.accept_unavailable(roster, child, sender, phase, command);
            }
            outcome => behavior::ChildReport::new(child, outcome),
        };
        match roster {
            FixedRoster::ShuttingDown(shutdown) => match shutdown.accept_outcome(report) {
                Ok(shutdown) => self.retain_shutdown(shutdown, Vec::new()),
                Err((shutdown, report)) => {
                    self.roster = FixedRoster::ShuttingDown(shutdown);
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::ProxyReported(report),
                    })
                }
            },
            roster => {
                let (roster, report) = match roster.accept_recovery_report(report) {
                    Ok(ControlFlow::Continue(roster)) => return self.authorize_waiting(roster),
                    Ok(ControlFlow::Break((owners, completed))) => {
                        return self.accept_replacement_completion(owners, completed);
                    }
                    Err(returned) => returned,
                };
                match report.report {
                    outcome @ crate::proxy::ProxyOutcome::WorkerStopped { .. } => {
                        let report = behavior::ChildReport::new(report.child, outcome);
                        match roster.accept_worker_stop(report) {
                            WorkerStopDecision::Accepted(stopped) => {
                                self.accept_worker_stop(stopped)
                            }
                            WorkerStopDecision::Rejected { roster, report } => {
                                self.roster = roster;
                                Err(FixedSupervisorError::InputRejected {
                                    input: FixedSupervisorEvent::ProxyReported(report),
                                })
                            }
                        }
                    }
                    outcome => {
                        let report = behavior::ChildReport::new(report.child, outcome);
                        match roster.accept_initial_outcome(report) {
                            InitialProxyDecision::Ready { mut owners, member } => {
                                let lifecycle = match self.lifecycle.clone() {
                                    Some(route_to_lifecycle) => {
                                        route_to_lifecycle.deliver(FixedLifecycle::started(
                                            member.role.name(),
                                            member.proxy.clone(),
                                        ))
                                    }
                                    None => LifecycleRoute::Sends::empty(),
                                };
                                owners.push(RosterOwner::Online(member));
                                let (roster, proxy_operations) =
                                    match self.authorize_roster(FixedRoster::Operating(owners)) {
                                        Ok(authorized) => authorized,
                                        Err((roster, rejection)) => {
                                            self.roster = roster;
                                            return Err(rejection);
                                        }
                                    };
                                self.roster = roster;
                                let mut requests = FixedSupervisorRequests::empty();
                                requests.proxy_operations = proxy_operations;
                                requests.lifecycle = lifecycle;
                                Ok(Actions::send(requests))
                            }
                            InitialProxyDecision::Failed {
                                owners,
                                child,
                                role,
                                outcome,
                            } => self.accept_initial_failure(owners, child, role, outcome),
                            InitialProxyDecision::Rejected { roster, rejection } => {
                                self.roster = roster;
                                Err(rejection)
                            }
                        }
                    }
                }
            }
        }
    }

    fn accept_unavailable(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        child: CreationId,
        sender: BehaviorAddr<Worker>,
        phase: crate::proxy::ProxyPhase,
        command: <Worker::Protocol as behavior::Protocol>::Msg,
    ) -> BehaviorActed<Self> {
        let role = match roster.live_proxy_role(child) {
            Some(role) => role,
            None => {
                self.roster = roster;
                return Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::ProxyReported(behavior::ChildReport::new(
                        child,
                        crate::proxy::ProxyOutcome::Unavailable {
                            sender,
                            phase,
                            command,
                        },
                    )),
                });
            }
        };
        let unavailable = WorkerUnavailable::new(role, sender, phase, command);
        self.roster = roster;
        match self.lifecycle.clone() {
            Some(route_to_lifecycle) => {
                let mut requests = FixedSupervisorRequests::empty();
                requests.lifecycle =
                    route_to_lifecycle.deliver(FixedLifecycle::unavailable(unavailable));
                Ok(Actions::send(requests))
            }
            None => {
                let become_ = match &self.diagnostics {
                    DiagnosticDisposition::DeliverTo(_) => Step::Continue,
                    DiagnosticDisposition::Terminate => Step::Stop(behavior::Stopped),
                };
                let diagnostic = FixedDiagnostic::WorkerUnavailable(unavailable);
                let mut requests = FixedSupervisorRequests::empty();
                requests.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(Actions::new(requests, Creations::empty(), become_))
            }
        }
    }

    fn accept_replacement_completion(
        &mut self,
        mut owners: Vec<RosterOwner<Role, Worker, Plan>>,
        completed: ReplacementCompletion<Role, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        match completed {
            ReplacementCompletion::Restarted {
                role,
                creation,
                proxy,
                previous_readiness,
                stopped,
                worker,
                readiness,
                recovery,
            } => {
                let name = role.name();
                let lifecycle =
                    match self.lifecycle.clone() {
                        Some(route_to_lifecycle) => {
                            let mut lifecycle = LifecycleRoute::Sends::empty();
                            lifecycle.append(route_to_lifecycle.clone().deliver(
                                FixedLifecycle::worker_stopped_after_admission(
                                    name.clone(),
                                    stopped,
                                    recovery.0,
                                ),
                            ));
                            lifecycle.append(route_to_lifecycle.deliver(
                                FixedLifecycle::restarted(name, proxy.clone(), recovery.0),
                            ));
                            lifecycle
                        }
                        None => {
                            let _retired_predecessor = stopped;
                            LifecycleRoute::Sends::empty()
                        }
                    };
                drop(previous_readiness);
                owners.push(RosterOwner::Online(OnlineMember {
                    role,
                    creation,
                    proxy,
                    worker,
                    readiness,
                }));
                let (roster, proxy_operations) =
                    match self.authorize_roster(FixedRoster::Operating(owners)) {
                        Ok(authorized) => authorized,
                        Err((roster, rejection)) => {
                            self.roster = roster;
                            return Err(rejection);
                        }
                    };
                self.roster = roster;
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.lifecycle = lifecycle;
                Ok(Actions::send(requests))
            }
            ReplacementCompletion::InputRejected {
                role,
                creation,
                proxy,
                previous,
                previous_readiness,
                stopped,
                operation,
                reason,
                recovery,
            } => {
                let diagnostic = FixedDiagnostic::ProxyInputRejected(ProxyInputFailure::new(
                    role.name(),
                    operation,
                    reason,
                ));
                self.apply_replacement_failure(
                    owners,
                    OnlineMember {
                        role,
                        creation,
                        proxy,
                        worker: previous,
                        readiness: previous_readiness,
                    },
                    stopped,
                    recovery,
                    diagnostic,
                )
            }
            ReplacementCompletion::ProxyFailed {
                role,
                creation,
                proxy,
                previous,
                previous_readiness,
                stopped,
                outcome,
                recovery,
            } => {
                let diagnostic = FixedDiagnostic::ProxyOutcomeFailed(ProxyOutcomeFailure::new(
                    role.name(),
                    crate::proxy::ProxyOutcome::Replacement { outcome },
                ));
                self.apply_replacement_failure(
                    owners,
                    OnlineMember {
                        role,
                        creation,
                        proxy,
                        worker: previous,
                        readiness: previous_readiness,
                    },
                    Some(stopped),
                    recovery,
                    diagnostic,
                )
            }
        }
    }

    fn apply_replacement_failure(
        &mut self,
        mut owners: Vec<RosterOwner<Role, Worker, Plan>>,
        previous: OnlineMember<Role, Worker, Plan>,
        stopped: Option<crate::ChildStopped<BehaviorAddr<Worker>>>,
        recovery: RecoveryTicket,
        diagnostic: FixedDiagnostic<Role, Worker, Plan, Source>,
    ) -> BehaviorActed<Self> {
        let OnlineMember {
            role,
            creation,
            proxy,
            worker,
            readiness,
        } = previous;
        let position = role.position();
        let (member, operation, lifecycle) = match (stopped, self.lifecycle.clone()) {
            (Some(stopped), Some(route_to_lifecycle)) => {
                let lifecycle =
                    route_to_lifecycle.deliver(FixedLifecycle::worker_stopped_after_admission(
                        role.name(),
                        stopped,
                        recovery.0,
                    ));
                drop(worker);
                drop(readiness);
                let (member, operation) =
                    UnrecoveredMember::begin_after_worker_stop_transfer(role, creation, proxy);
                (member, operation, lifecycle)
            }
            (stopped, None) | (stopped, Some(_)) => {
                let (member, operation) = UnrecoveredMember::begin_replacement_failure(
                    role, creation, proxy, worker, readiness, stopped,
                );
                (member, operation, LifecycleRoute::Sends::empty())
            }
        };
        owners.push(RosterOwner::Unrecovered(member));
        match (&self.diagnostics, self.failure_reaction) {
            (DiagnosticDisposition::Terminate, _) => {
                self.roster = FixedRoster::Terminating {
                    owners,
                    prepared: Vec::new(),
                };
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(operation);
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.lifecycle = lifecycle;
                requests.diagnostics =
                    InterpreterRequests::one(DiagnosticAction::terminal(diagnostic));
                Ok(Actions::new(
                    requests,
                    Creations::empty(),
                    Step::Stop(behavior::Stopped),
                ))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::RetireMember) => {
                self.roster = FixedRoster::Operating(owners);
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(operation);
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.lifecycle = lifecycle;
                requests.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(Actions::send(requests))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::StopSupervisor) => {
                let (shutdown, mut operations, schedule) =
                    FixedShutdown::begin(owners, self.actor_drain);
                operations.push((position, operation));
                operations.sort_by_key(|(position, _)| *position);
                self.roster = FixedRoster::ShuttingDown(shutdown);
                let mut proxy_operations = SourceActions::empty();
                for (_, operation) in operations {
                    proxy_operations.send(operation);
                }
                let mut restart_schedules = SourceActions::empty();
                match schedule {
                    Some(schedule) => restart_schedules.send(schedule),
                    None => {}
                }
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.restart_schedules = restart_schedules;
                requests.lifecycle = lifecycle;
                requests.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(Actions::send(requests))
            }
        }
    }

    fn accept_worker_stop(
        &mut self,
        stopped: AcceptedWorkerStop<Role, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let recovery = mem::replace(&mut self.recovery, SupervisorRecovery::temporary());
        match recovery.choose(stopped.kind()) {
            RecoveryChoice::LeaveEmpty(recovery) => {
                self.recovery = recovery;
                match self.lifecycle.clone() {
                    Some(route_to_lifecycle) => {
                        let (roster, role, worker_stop) = stopped.release_stop();
                        self.roster = roster;
                        let mut requests = FixedSupervisorRequests::empty();
                        requests.lifecycle = route_to_lifecycle
                            .deliver(FixedLifecycle::worker_stopped_ineligible(role, worker_stop));
                        Ok(Actions::send(requests))
                    }
                    None => {
                        self.roster = stopped.discharge_stop();
                        Ok(Actions::cont())
                    }
                }
            }
            RecoveryChoice::SourceUnavailable(recovery) => {
                self.recovery = recovery;
                let (roster, report) = stopped.reject();
                self.roster = roster;
                Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::ProxyReported(report),
                })
            }
            RecoveryChoice::Prepare(preparation) => match stopped.begin_recovery(preparation) {
                Ok((recovery, roster, request)) => {
                    self.recovery = recovery;
                    self.roster = roster;
                    let mut worker_preparations = SourceActions::empty();
                    worker_preparations.send(request);
                    let mut requests = FixedSupervisorRequests::empty();
                    requests.worker_preparations = worker_preparations;
                    Ok(Actions::send(requests))
                }
                Err((stopped, preparation)) => {
                    self.recovery = preparation.restore();
                    let (roster, report) = stopped.reject();
                    self.roster = roster;
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::ProxyReported(report),
                    })
                }
            },
        }
    }

    fn apply_failed_preparation(
        &mut self,
        awaiting: recovery::AwaitingWorkerSource,
        failed: recovery::FailedPreparation<Role, Worker, Plan, Source>,
    ) -> BehaviorActed<Self> {
        match (&self.diagnostics, self.failure_reaction) {
            (DiagnosticDisposition::Terminate, _) => {
                let (roster, source, failure) = failed.terminate();
                self.recovery = awaiting.restore(source);
                self.roster = roster;
                let mut requests = FixedSupervisorRequests::empty();
                requests.diagnostics = InterpreterRequests::one(DiagnosticAction::terminal(
                    FixedDiagnostic::WorkerPreparationFailed(failure),
                ));
                Ok(Actions::new(
                    requests,
                    Creations::empty(),
                    Step::Stop(behavior::Stopped),
                ))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::RetireMember) => {
                let (roster, source, failure, operation) = failed.retire_member();
                self.recovery = awaiting.restore(source);
                self.roster = roster;
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(operation);
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.diagnostics = InterpreterRequests::one(
                    self.diagnostics
                        .action(FixedDiagnostic::WorkerPreparationFailed(failure)),
                );
                Ok(Actions::send(requests))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::StopSupervisor) => {
                let (shutdown, source, failure, operations, schedule) =
                    failed.stop_supervisor(self.actor_drain);
                self.recovery = awaiting.restore(source);
                self.roster = FixedRoster::ShuttingDown(shutdown);
                let mut proxy_operations = SourceActions::empty();
                for operation in operations {
                    proxy_operations.send(operation);
                }
                let mut restart_schedules = SourceActions::empty();
                match schedule {
                    Some(schedule) => restart_schedules.send(schedule),
                    None => {}
                }
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.restart_schedules = restart_schedules;
                requests.diagnostics = InterpreterRequests::one(
                    self.diagnostics
                        .action(FixedDiagnostic::WorkerPreparationFailed(failure)),
                );
                Ok(Actions::send(requests))
            }
        }
    }

    fn accept_worker_preparation_start(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        input: ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        let recovery = mem::replace(&mut self.recovery, SupervisorRecovery::temporary());
        let awaiting = match recovery.expect_source() {
            Ok(awaiting) => awaiting,
            Err(recovery) => {
                self.recovery = recovery;
                self.roster = roster;
                return Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::WorkerPreparationStarted(input),
                });
            }
        };
        let roster = match roster {
            FixedRoster::ShuttingDown(shutdown) => {
                return match shutdown.accept_preparation_start(input) {
                    Ok(ControlFlow::Continue(shutdown)) => {
                        self.recovery = awaiting.waiting();
                        self.retain_shutdown(shutdown, Vec::new())
                    }
                    Ok(ControlFlow::Break((shutdown, returned))) => {
                        self.recovery = awaiting
                            .retain_for_retirement(WorkerPreparationRetirement::Start(returned));
                        self.retain_shutdown(shutdown, Vec::new())
                    }
                    Err((shutdown, input)) => {
                        self.recovery = awaiting.waiting();
                        self.roster = FixedRoster::ShuttingDown(shutdown);
                        Err(FixedSupervisorError::InputRejected {
                            input: FixedSupervisorEvent::WorkerPreparationStarted(input),
                        })
                    }
                };
            }
            roster => roster,
        };
        match roster.accept_preparation_start(input) {
            Ok(ControlFlow::Continue(roster)) => {
                self.recovery = awaiting.waiting();
                self.roster = roster;
                Ok(Actions::cont())
            }
            Ok(ControlFlow::Break(failed)) => self.apply_failed_preparation(awaiting, failed),
            Err((roster, input)) => {
                self.recovery = awaiting.waiting();
                self.roster = roster;
                Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::WorkerPreparationStarted(input),
                })
            }
        }
    }

    fn accept_worker_preparation_return(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        returned: WorkerPreparation<Source, Role, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let recovery = mem::replace(&mut self.recovery, SupervisorRecovery::temporary());
        let awaiting = match recovery.expect_source() {
            Ok(awaiting) => awaiting,
            Err(recovery) => {
                self.recovery = recovery;
                self.roster = roster;
                return Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::WorkerPreparationReturned(returned),
                });
            }
        };
        let roster = match roster {
            FixedRoster::ShuttingDown(shutdown) => {
                return match shutdown.accept_preparation_return(returned) {
                    Ok((shutdown, returned)) => {
                        self.recovery = awaiting
                            .retain_for_retirement(WorkerPreparationRetirement::Returned(returned));
                        self.retain_shutdown(shutdown, Vec::new())
                    }
                    Err((shutdown, returned)) => {
                        self.recovery = awaiting.waiting();
                        self.roster = FixedRoster::ShuttingDown(shutdown);
                        Err(FixedSupervisorError::InputRejected {
                            input: FixedSupervisorEvent::WorkerPreparationReturned(returned),
                        })
                    }
                };
            }
            roster => roster,
        };
        match roster.accept_preparation_return(returned) {
            Ok(PreparationAcceptance::Prepared {
                owners,
                position,
                prepared,
                source,
            }) => match awaiting.decide_prepared(owners, position, prepared, source) {
                PreparedRecoveryDecision::Admitted {
                    roster,
                    recovery,
                    schedule,
                } => self.admit_recovery(roster, recovery, schedule),
                PreparedRecoveryDecision::Denied {
                    failed,
                    recovery,
                    diagnostic,
                } => {
                    self.recovery = recovery;
                    self.apply_recovery_failure(failed, FixedDiagnostic::RecoveryDenied(diagnostic))
                }
            },
            Ok(PreparationAcceptance::Failed(failed)) => {
                self.apply_failed_preparation(awaiting, failed)
            }
            Err((roster, returned)) => {
                self.recovery = awaiting.waiting();
                self.roster = roster;
                Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::WorkerPreparationReturned(returned),
                })
            }
        }
    }

    fn accept_initial_failure(
        &mut self,
        owners: Vec<RosterOwner<Role, Worker, Plan>>,
        child: CreationId,
        role: RoleName<Role>,
        outcome: crate::proxy::ProxyOutcome<Worker, Plan>,
    ) -> BehaviorActed<Self> {
        match (&self.diagnostics, self.failure_reaction) {
            (DiagnosticDisposition::Terminate, _) => {
                self.roster = FixedRoster::Terminating {
                    owners,
                    prepared: Vec::new(),
                };
                let diagnostic =
                    FixedDiagnostic::ProxyOutcomeFailed(ProxyOutcomeFailure::new(role, outcome));
                let mut requests = FixedSupervisorRequests::empty();
                requests.diagnostics =
                    InterpreterRequests::one(DiagnosticAction::terminal(diagnostic));
                Ok(Actions::new(
                    requests,
                    Creations::empty(),
                    Step::Stop(behavior::Stopped),
                ))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::RetireMember) => {
                let roster = FixedRoster::Operating(owners);
                let (roster, shutdown) = match roster.begin_stop_after_initial_failure(child) {
                    Ok(started) => started,
                    Err(roster) => {
                        self.roster = roster;
                        return Err(FixedSupervisorError::InitialOutcomeContradiction {
                            report: behavior::ChildReport::new(child, outcome),
                        });
                    }
                };
                self.roster = roster;
                let diagnostic =
                    FixedDiagnostic::ProxyOutcomeFailed(ProxyOutcomeFailure::new(role, outcome));
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(shutdown);
                let mut requests = FixedSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(Actions::send(requests))
            }
            (DiagnosticDisposition::DeliverTo(_), FailureReaction::StopSupervisor) => {
                let roster = FixedRoster::Operating(owners);
                let (shutdown, operations, schedule) = match roster.begin_shutdown(self.actor_drain)
                {
                    Ok(started) => started,
                    Err(roster) => {
                        self.roster = roster;
                        return Err(FixedSupervisorError::InitialOutcomeContradiction {
                            report: behavior::ChildReport::new(child, outcome),
                        });
                    }
                };
                let diagnostic =
                    FixedDiagnostic::ProxyOutcomeFailed(ProxyOutcomeFailure::new(role, outcome));
                let mut actions = match self.retain_shutdown(shutdown, operations) {
                    Ok(actions) => actions,
                    Err(rejection) => return Err(rejection),
                };
                match schedule {
                    Some(schedule) => actions.sends.restart_schedules.send(schedule),
                    None => {}
                }
                actions.sends.diagnostics =
                    InterpreterRequests::one(self.diagnostics.action(diagnostic));
                Ok(actions)
            }
        }
    }

    fn accept_proxy_exit(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        stopped: crate::ChildStopped<BehaviorAddr<Worker>>,
    ) -> BehaviorActed<Self> {
        match roster {
            FixedRoster::ShuttingDown(shutdown) => match shutdown.accept_stop(stopped) {
                Ok(shutdown) => self.retain_shutdown(shutdown, Vec::new()),
                Err((shutdown, stopped)) => {
                    self.roster = FixedRoster::ShuttingDown(shutdown);
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::ProxyStopped(stopped),
                    })
                }
            },
            roster => {
                let (next, retired) = match roster.accept_proxy_stop(stopped) {
                    Ok(next) => next,
                    Err((next, rejection)) => {
                        self.roster = next;
                        return Err(rejection);
                    }
                };
                match retired {
                    Some(role) => self.member_retired(next, role),
                    None => self.authorize_waiting(next),
                }
            }
        }
    }

    fn accept_command(
        &mut self,
        roster: FixedRoster<Role, Worker, Plan>,
        from: BehaviorAddr<Worker>,
        message: FixedCommand<BehaviorAddr<Worker>, Role, Worker::Protocol>,
    ) -> BehaviorActed<Self> {
        match message {
            FixedCommand::Shutdown => match roster.begin_shutdown(self.actor_drain) {
                Ok((shutdown, operations, schedule)) => {
                    let mut actions = match self.retain_shutdown(shutdown, operations) {
                        Ok(actions) => actions,
                        Err(rejection) => return Err(rejection),
                    };
                    match schedule {
                        Some(schedule) => actions.sends.restart_schedules.send(schedule),
                        None => {}
                    }
                    Ok(actions)
                }
                Err(roster) => {
                    self.roster = roster;
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::Command(User::new(
                            from,
                            FixedCommand::Shutdown,
                        )),
                    })
                }
            },
            FixedCommand::Status { reply_to } => match roster.snapshot() {
                Some(snapshot) => {
                    self.roster = roster;
                    let mut requests = FixedSupervisorRequests::empty();
                    requests.status_replies = reply_to.deliver(snapshot);
                    Ok(Actions::send(requests))
                }
                None => {
                    self.roster = roster;
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::Command(User::new(
                            from,
                            FixedCommand::Status { reply_to },
                        )),
                    })
                }
            },
            FixedCommand::Capability { role, reply_to } => match roster.capability(role) {
                Ok(result) => {
                    self.roster = roster;
                    let mut requests = FixedSupervisorRequests::empty();
                    requests.capability_replies = reply_to.deliver(result);
                    Ok(Actions::send(requests))
                }
                Err(role) => {
                    self.roster = roster;
                    Err(FixedSupervisorError::InputRejected {
                        input: FixedSupervisorEvent::Command(User::new(
                            from,
                            FixedCommand::Capability { role, reply_to },
                        )),
                    })
                }
            },
        }
    }

    fn unexpected_input(
        &mut self,
        input: FixedSupervisorEvent<
            Role,
            Worker,
            Plan,
            ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
            WorkerPreparation<Source, Role, Worker, Plan>,
        >,
    ) -> BehaviorActed<Self> {
        let become_ = match &self.diagnostics {
            DiagnosticDisposition::DeliverTo(_) => Step::Continue,
            DiagnosticDisposition::Terminate => Step::Stop(behavior::Stopped),
        };
        let mut requests = FixedSupervisorRequests::empty();
        requests.diagnostics = InterpreterRequests::one(
            self.diagnostics
                .action(FixedDiagnostic::UnexpectedInput { input }),
        );
        Ok(Actions::new(requests, Creations::empty(), become_))
    }
}

impl<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute> Behavior
    for FixedSupervisor<Role, Worker, Plan, Source, DiagnosticRoute, LifecycleRoute>
where
    Role: Eq + Send + Sync,
    Worker: Behavior + Send,
    Plan: StartupPlan,
    Source: WorkerSource<Role, Worker, Plan>,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    EstablishedActor<StableProxy<Worker, Plan>>: Clone + Send,
    DiagnosticRoute: crate::DiagnosticRoute<FixedDiagnostic<Role, Worker, Plan, Source>> + Clone,
    LifecycleRoute: FixedLifecycleRoute<Role, Worker, Plan> + Clone,
    LifecycleRoute::Sends: behavior::SendsFor<
            FixedSupervisorEvent<
                Role,
                Worker,
                Plan,
                ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
                WorkerPreparation<Source, Role, Worker, Plan>,
            >,
        >,
{
    type Protocol = MessageProtocol<
        BehaviorAddr<Worker>,
        FixedCommand<BehaviorAddr<Worker>, Role, Worker::Protocol>,
    >;
    type Event = FixedSupervisorEvent<
        Role,
        Worker,
        Plan,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        WorkerPreparation<Source, Role, Worker, Plan>,
    >;
    type Sends = FixedSupervisorRequests<
        InterpreterRequests<ObserveChild<Worker::Protocol, ChildHead>>,
        SourceActions<PrepareWorkers<Source, Role, Worker, Plan>>,
        SourceActions<ProxyOperation<behavior::Here, Worker, Plan>>,
        SourceActions<ScheduleAfter>,
        LifecycleRoute::Sends,
        <ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, FixedSnapshot<Worker::Protocol>>,
        > as DeliveryRoute>::Sends,
        <ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                CapabilityResult<Role, Worker::Protocol>,
            >,
        > as DeliveryRoute>::Sends,
        InterpreterRequests<
            DiagnosticAction<DiagnosticRoute, FixedDiagnostic<Role, Worker, Plan, Source>>,
        >,
    >;
    type Ph = Never;
    type Error = FixedSupervisorError<
        Role,
        Worker,
        Plan,
        ActionItemResult<PrepareWorkers<Source, Role, Worker, Plan>>,
        WorkerPreparation<Source, Role, Worker, Plan>,
    >;
    type Birth = Births<StableProxy<Worker, Plan>>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        let roster = mem::replace(&mut self.roster, FixedRoster::Stopped);
        match roster.begin(&mut self.creations) {
            Ok((next, observations, proxies)) => {
                self.roster = next;
                let mut sends = FixedSupervisorRequests::empty();
                sends.proxy_observations = InterpreterRequests::new(observations);
                Ok(Actions::new(sends, proxies, Step::Continue))
            }
            Err((roster, rejection)) => {
                self.roster = roster;
                Err(rejection)
            }
        }
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        let roster = mem::replace(&mut self.roster, FixedRoster::Stopped);
        let acted = match input {
            FixedSupervisorEvent::ProxyCreationsSettled(proxies) => {
                self.accept_proxy_birth(roster, proxies)
            }
            FixedSupervisorEvent::ProxyInputSettled(settlement) => {
                self.accept_proxy_operation(roster, settlement)
            }
            FixedSupervisorEvent::ProxyReported(report) => self.accept_proxy_report(roster, report),
            FixedSupervisorEvent::ProxyDiagnosed(report) => {
                self.roster = roster;
                Err(FixedSupervisorError::InputRejected {
                    input: FixedSupervisorEvent::ProxyDiagnosed(report),
                })
            }
            FixedSupervisorEvent::ProxyStopped(stopped) => self.accept_proxy_exit(roster, stopped),
            FixedSupervisorEvent::WorkerPreparationStarted(started) => {
                self.accept_worker_preparation_start(roster, started)
            }
            FixedSupervisorEvent::WorkerPreparationReturned(returned) => {
                self.accept_worker_preparation_return(roster, returned)
            }
            FixedSupervisorEvent::RestartScheduleSettled(input) => {
                self.accept_restart_schedule(roster, input)
            }
            FixedSupervisorEvent::RestartElapsed(elapsed) => {
                self.accept_restart_elapsed(roster, elapsed)
            }
            FixedSupervisorEvent::Command(User { from, message }) => {
                self.accept_command(roster, from, message)
            }
        };
        match acted {
            Err(FixedSupervisorError::InputRejected { input }) => self.unexpected_input(input),
            acted => acted,
        }
    }
}
