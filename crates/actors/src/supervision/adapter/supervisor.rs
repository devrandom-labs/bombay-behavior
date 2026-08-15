//! Fleet coordination for supervised stable proxy actors.

use std::time::Duration;

use super::super::domain::{Fleet, FleetError, RestartBudget};
use super::super::policy::{
    ReportSupervisionFailure, RestartPolicy, Strategy, SupervisionFailure,
    SupervisionFailureReaction, retire_on_supervision_failure,
};
use super::super::protocol::{ProxyCommand, SupervisionEvent};
use super::proxy::Proxy;
use crate::protocol::{
    ChildStopped, CreationResolved, ObserveChild, WorkerCreationResolved, WorkerStopped,
};
use crate::{Become, Exit, SupervisionFailureReason};
use crate::{Own, RouteInput, SendInput};
use behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, ServiceSends,
};
use behavior::{Never, Step};

/// A fixed supervised topology and the pure factory for its ordered slots.
///
/// The nonce sequence is the topology's stable semantic identity. Its position
/// selects the matching factory input but is never itself used as an address
/// or inferred as a nonce.
pub struct ChildTopology<N, C> {
    pub(crate) nonces: Vec<N>,
    pub(crate) build: fn(usize) -> Option<C>,
}

impl<N, C> ChildTopology<N, C> {
    /// Define an ordered topology from explicit creator-local nonces.
    #[must_use]
    pub fn new(nonces: impl IntoIterator<Item = N>, build: fn(usize) -> Option<C>) -> Self {
        Self {
            nonces: nonces.into_iter().collect(),
            build,
        }
    }

    /// Define `count` ordered slots through one explicit nonce function.
    #[must_use]
    pub fn indexed(nonces: fn(usize) -> N, count: usize, build: fn(usize) -> Option<C>) -> Self {
        Self::new((0..count).map(nonces), build)
    }

    /// Return the configured nonce sequence in birth order.
    #[must_use]
    pub fn nonces(&self) -> &[N] {
        &self.nonces
    }

    /// Return the number of configured slots.
    #[must_use]
    pub fn len(&self) -> usize {
        self.nonces.len()
    }

    /// Report whether the topology contains no slots.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.nonces.is_empty()
    }
}

/// Supervision strategy, eligibility, and restart-budget policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartConfiguration {
    /// Candidate-selection strategy after an eligible stop.
    pub strategy: Strategy,
    /// Exit classification eligible for replacement.
    pub policy: RestartPolicy,
    /// Maximum accepted replacements inside `window`.
    pub maximum: u32,
    /// Inclusive restart-budget window.
    pub window: Duration,
}

impl RestartConfiguration {
    /// Define a complete restart policy.
    #[must_use]
    pub const fn new(
        strategy: Strategy,
        policy: RestartPolicy,
        maximum: u32,
        window: Duration,
    ) -> Self {
        Self {
            strategy,
            policy,
            maximum,
            window,
        }
    }
}

/// Named effect lanes emitted by a supervised behavior.
pub struct SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    /// Sends emitted by the supervised domain behavior.
    pub behavior: Sends,
    /// Requests to observe every accepted stable proxy creation.
    pub child_observations: ServiceSends<ObserveChild<A::Nonce>>,
    /// Commands asking stable proxies to install fresh worker incarnations.
    pub replacement_commands: Vec<Delivery<Proxy<C>>>,
    /// Typed terminal supervision failures for the local runtime observer.
    pub failure_reports: ServiceSends<ReportSupervisionFailure<A>>,
}

impl<A, Sends, C> SendAlgebra for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    Sends: SendAlgebra,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn empty() -> Self {
        Self {
            behavior: Sends::empty(),
            child_observations: ServiceSends::empty(),
            replacement_commands: Vec::new(),
            failure_reports: ServiceSends::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.behavior.append(other.behavior);
        self.child_observations.append(other.child_observations);
        self.replacement_commands.extend(other.replacement_commands);
        self.failure_reports.append(other.failure_reports);
    }
}

impl<A, Sends, C> SendInput<ObserveChild<A::Nonce>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn emit(&mut self, input: ObserveChild<A::Nonce>) {
        self.child_observations.send(input);
    }
}

impl<A, Sends, C> SendInput<Delivery<Proxy<C>>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn emit(&mut self, input: Delivery<Proxy<C>>) {
        self.replacement_commands.push(input);
    }
}

impl<A, Sends, C> SendInput<ReportSupervisionFailure<A>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn emit(&mut self, input: ReportSupervisionFailure<A>) {
        self.failure_reports.send(input);
    }
}

pub(crate) type SupervisorActions<B, C> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    SupervisorSends<<B as Behavior>::Addr, <B as Behavior>::Sends, C>,
    Births<Proxy<C>>,
>;

/// A controlled supervisor-fold failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SupervisorError<E, N> {
    /// The supervised behavior rejected its fold.
    #[error("supervised behavior rejected the transition")]
    Behavior(#[source] E),
    /// The supervisor's child topology rejected the operation.
    #[error(transparent)]
    Fleet(#[from] FleetError<N>),
    /// The configured worker factory did not define a requested fleet index.
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
}

enum ReplacementDecision<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    Retire,
    Replace(Vec<Delivery<Proxy<C>>>),
    Failed(SupervisionFailure<A>),
}

type ReplacementResult<B, C> = Result<
    ReplacementDecision<<B as Behavior>::Addr, C>,
    SupervisorError<<B as Behavior>::Error, <<B as Behavior>::Addr as Address>::Nonce>,
>;

type WrappedSupervisorResult<B, C> = Result<
    SupervisorActions<B, C>,
    SupervisorError<<B as Behavior>::Error, <<B as Behavior>::Addr as Address>::Nonce>,
>;

pub struct Supervisor<B: Behavior, C: Behavior<Ph = Never, Addr = B::Addr>> {
    inner: B,
    fleet: Fleet<<B::Addr as Address>::Nonce>,
    build: fn(usize) -> Option<C>,
    strategy: Strategy,
    policy: RestartPolicy,
    budget: RestartBudget,
    on_failure: SupervisionFailureReaction<B>,
}

impl<B, C> crate::BehaviorBase for Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>> + crate::BehaviorBase,
    <B::Addr as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, C> crate::StashStatus for Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>> + crate::StashStatus,
    <B::Addr as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, C> Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>>,
    <B::Addr as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    /// Construct the concrete supervisor behavior directly.
    ///
    /// [`ChildTopology`] owns stable child routes and their factory;
    /// [`RestartConfiguration`] owns the complete restart policy. Construction
    /// is therefore independent of wrapper nesting.
    ///
    /// # Errors
    /// Returns the first typed topology rejection.
    pub fn new(
        inner: B,
        topology: ChildTopology<<B::Addr as Address>::Nonce, C>,
        restart: RestartConfiguration,
    ) -> Result<Self, FleetError<<B::Addr as Address>::Nonce>> {
        let fleet = Fleet::configured(topology.nonces)?;
        Ok(Self {
            inner,
            fleet,
            build: topology.build,
            strategy: restart.strategy,
            policy: restart.policy,
            budget: RestartBudget::new(restart.maximum, restart.window),
            on_failure: retire_on_supervision_failure::<B>,
        })
    }

    #[must_use]
    pub fn with_strategy(mut self, strategy: Strategy) -> Self {
        self.strategy = strategy;
        self
    }

    #[must_use]
    pub fn with_policy(mut self, policy: RestartPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn with_budget(mut self, max: u32, window: Duration) -> Self {
        self.budget = RestartBudget::new(max, window);
        self
    }

    #[must_use]
    /// Replace the pure reaction used for typed supervision failures.
    pub fn with_failure_reaction(mut self, reaction: SupervisionFailureReaction<B>) -> Self {
        self.on_failure = reaction;
        self
    }

    /// Report whether a known supervised proxy is alive.
    ///
    /// # Errors
    /// Returns the unknown nonce when it is not part of this topology.
    pub fn is_alive(
        &self,
        nonce: <B::Addr as Address>::Nonce,
    ) -> Result<bool, SupervisorError<core::convert::Infallible, <B::Addr as Address>::Nonce>> {
        Ok(self.fleet.is_available(nonce)?)
    }

    #[must_use]
    pub fn child_count(&self) -> usize {
        self.fleet.len()
    }

    #[must_use]
    pub fn restarts_in_window(&self) -> usize {
        self.budget.admitted()
    }

    fn replacement_decision(&mut self, event: &WorkerStopped<B::Addr>) -> ReplacementResult<B, C> {
        let policy = self.policy;
        let strategy = self.strategy;
        let eligible = match policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => {
                !matches!(&event.outcome, Ok(Exit::Normal | Exit::Collected))
            }
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            self.fleet.retire(event.proxy)?;
            return Ok(ReplacementDecision::Retire);
        }
        let candidates = self.fleet.replacements(event.proxy, strategy)?;
        let replacements = candidates
            .iter()
            .map(|candidate| {
                (self.build)(candidate.index)
                    .map(|child| (candidate.nonce, child))
                    .ok_or(SupervisorError::FactoryIndex {
                        index: candidate.index,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Err(reason) = self.budget.admit(event.at, candidates.len()) {
            self.fleet.retire(event.proxy)?;
            return Ok(ReplacementDecision::Failed(SupervisionFailure::new(
                event.proxy,
                event.outcome,
                SupervisionFailureReason::RestartDenied(reason),
            )));
        }
        for candidate in &candidates {
            self.fleet.replacement_requested(candidate.nonce)?;
        }
        Ok(ReplacementDecision::Replace(
            replacements
                .into_iter()
                .map(|(nonce, child)| {
                    Delivery::new(Recipient::child(nonce), ProxyCommand::Replace(child))
                })
                .collect(),
        ))
    }

    fn react_to_failure(
        &mut self,
        failure: &SupervisionFailure<B::Addr>,
    ) -> Result<Become<B::Ph>, B::Error> {
        Ok(match (self.on_failure)(&mut self.inner, failure)? {
            Step::Continue => Step::Continue,
            Step::Goto(never) => match never {},
            Step::Stop(exit) => Step::Stop(exit),
        })
    }

    fn wrap(
        &mut self,
        actions: Actions<B::Addr, B::Ph, B::Sends, Births<C>>,
    ) -> WrappedSupervisorResult<B, C> {
        let fleet = &mut self.fleet;
        for create in &actions.creates {
            fleet.register(create.nonce)?;
        }
        Ok(Actions::new(
            SupervisorSends {
                behavior: actions.sends,
                child_observations: ServiceSends::new(
                    actions
                        .creates
                        .iter()
                        .map(|create| ObserveChild::new(create.nonce))
                        .collect(),
                ),
                replacement_commands: Vec::new(),
                failure_reports: ServiceSends::empty(),
            },
            actions
                .creates
                .into_iter()
                .map(|create| Create::new(create.nonce, Proxy::new(create.child), create.kind))
                .collect(),
            actions.become_,
        ))
    }
}

impl<B, C, A, Ph, Sends> Behavior for Supervisor<B, C>
where
    A: Address,
    Sends: SendAlgebra,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Births<C>>,
    B::Event: crate::RouteInput<ChildStopped<A>>
        + crate::RouteInput<CreationResolved<A::Nonce>>
        + crate::RouteInput<WorkerCreationResolved<A::Nonce>>,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = SupervisionEvent<B::Event>;
    type Sends = SupervisorSends<A, Sends, C>;
    type Ph = Ph;
    type Error = SupervisorError<B::Error, A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<SupervisorActions<B, C>, Self::Error> {
        let configured: Vec<_> = self.fleet.configured_nonces().collect();
        let workers = configured
            .iter()
            .copied()
            .enumerate()
            .map(|(index, nonce)| {
                (self.build)(index)
                    .map(|worker| (nonce, worker))
                    .ok_or(SupervisorError::FactoryIndex { index })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let actions = behavior::initialize(&mut self.inner).map_err(SupervisorError::Behavior)?;
        let mut actions = self.wrap(actions)?;
        actions.creates.extend(
            workers
                .into_iter()
                .map(|(nonce, worker)| Create::birth(nonce, Proxy::new(worker))),
        );
        actions
            .sends
            .child_observations
            .extend(configured.into_iter().map(ObserveChild::new));
        Ok(actions)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<SupervisorActions<B, C>, Self::Error> {
        match event {
            SupervisionEvent::WorkerStopped(event) => {
                let decision = self.replacement_decision(&event)?;
                match decision {
                    ReplacementDecision::Retire => Ok(Actions::cont()),
                    ReplacementDecision::Replace(replacements) => Ok(Actions::new(
                        SupervisorSends {
                            behavior: B::Sends::empty(),
                            child_observations: ServiceSends::empty(),
                            replacement_commands: replacements,
                            failure_reports: ServiceSends::empty(),
                        },
                        Vec::new(),
                        Step::Continue,
                    )),
                    ReplacementDecision::Failed(failure) => Ok(Actions::new(
                        SupervisorSends {
                            behavior: B::Sends::empty(),
                            child_observations: ServiceSends::empty(),
                            replacement_commands: Vec::new(),
                            failure_reports: ServiceSends::one(ReportSupervisionFailure::new(
                                failure,
                            )),
                        },
                        Vec::new(),
                        self.react_to_failure(&failure)
                            .map_err(SupervisorError::Behavior)?,
                    )),
                }
            }
            SupervisionEvent::ChildStopped(event) => {
                self.fleet.retire(event.nonce)?;
                let failure = SupervisionFailure::new(
                    event.nonce,
                    event.outcome,
                    SupervisionFailureReason::StableChildStopped,
                );
                Ok(Actions::new(
                    SupervisorSends {
                        behavior: B::Sends::empty(),
                        child_observations: ServiceSends::empty(),
                        replacement_commands: Vec::new(),
                        failure_reports: ServiceSends::one(ReportSupervisionFailure::new(failure)),
                    },
                    Vec::new(),
                    self.react_to_failure(&failure)
                        .map_err(SupervisorError::Behavior)?,
                ))
            }
            SupervisionEvent::CreationResolved(event) => {
                self.fleet.resolve_creation(event.nonce, event.result);
                if let Ok(event) = B::Event::route(event) {
                    let actions = behavior::delegate_transition(&mut self.inner, event)
                        .map_err(SupervisorError::Behavior)?;
                    self.wrap(actions)
                } else {
                    Ok(Actions::cont())
                }
            }
            SupervisionEvent::WorkerCreationResolved(event) => {
                // Worker realization does not change the stable proxy's
                // liveness. The typed result remains distinct from a proxy
                // terminal observation.
                if let Ok(event) = B::Event::route(event) {
                    let actions = behavior::delegate_transition(&mut self.inner, event)
                        .map_err(SupervisorError::Behavior)?;
                    self.wrap(actions)
                } else {
                    Ok(Actions::cont())
                }
            }
            SupervisionEvent::Behavior(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)
                    .map_err(SupervisorError::Behavior)?;
                self.wrap(actions)
            }
        }
    }
}
