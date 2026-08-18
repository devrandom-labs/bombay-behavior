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
    ChildStopped, CreationResolved, ObserveChild, ObserveCreation, WorkerCreationResolved,
    WorkerStopped,
};
use crate::{Become, Exit, SupervisionFailureReason};
use crate::{Own, RouteInput, SendInput};
use behavior::{Actions, Address, Behavior, Births, Create, Delivery, SendAlgebra, ServiceSends};
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
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    /// Sends emitted by the supervised domain behavior.
    pub behavior: Sends,
    /// Requests to observe every accepted stable proxy creation.
    pub child_observations: ServiceSends<ObserveChild<A::Nonce>>,
    /// Requests for the committed result of every staged stable proxy creation.
    pub creation_observations: ServiceSends<ObserveCreation<A::Nonce>>,
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
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn empty() -> Self {
        Self {
            behavior: Sends::empty(),
            child_observations: ServiceSends::empty(),
            creation_observations: ServiceSends::empty(),
            replacement_commands: Vec::new(),
            failure_reports: ServiceSends::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.behavior.append(other.behavior);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.replacement_commands.extend(other.replacement_commands);
        self.failure_reports.append(other.failure_reports);
    }
}

impl<A, Sends, C> SendInput<ObserveCreation<A::Nonce>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: ObserveCreation<A::Nonce>) {
        self.creation_observations.send(input);
    }
}

impl<A, Sends, C> SendInput<ObserveChild<A::Nonce>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: ObserveChild<A::Nonce>) {
        self.child_observations.send(input);
    }
}

impl<A, Sends, C> SendInput<Delivery<Proxy<C>>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: Delivery<Proxy<C>>) {
        self.replacement_commands.push(input);
    }
}

impl<A, Sends, C> SendInput<ReportSupervisionFailure<A>, Own> for SupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: ReportSupervisionFailure<A>) {
        self.failure_reports.send(input);
    }
}

pub(crate) type SupervisorActions<B, C> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SupervisorSends<crate::BehaviorAddr<B>, <B as Behavior>::Sends, C>,
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
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    Retire,
    Replace(Vec<Delivery<Proxy<C>>>),
    Failed(SupervisionFailure<A>),
}

type ReplacementResult<B, C> = Result<
    ReplacementDecision<crate::BehaviorAddr<B>, C>,
    SupervisorError<<B as Behavior>::Error, <crate::BehaviorAddr<B> as Address>::Nonce>,
>;

type WrappedSupervisorResult<B, C> = Result<
    SupervisorActions<B, C>,
    SupervisorError<<B as Behavior>::Error, <crate::BehaviorAddr<B> as Address>::Nonce>,
>;

pub struct Supervisor<B: Behavior, C: Behavior<Ph = Never>>
where
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    inner: B,
    fleet: Fleet<<crate::BehaviorAddr<B> as Address>::Nonce>,
    build: fn(usize) -> Option<C>,
    strategy: Strategy,
    policy: RestartPolicy,
    budget: RestartBudget,
    on_failure: SupervisionFailureReaction<B>,
}

impl<B, C> crate::BehaviorBase for Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>> + crate::BehaviorBase,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, C> crate::StashStatus for Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>> + crate::StashStatus,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, C> Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
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
        topology: ChildTopology<<crate::BehaviorAddr<B> as Address>::Nonce, C>,
        restart: RestartConfiguration,
    ) -> Result<Self, FleetError<<crate::BehaviorAddr<B> as Address>::Nonce>> {
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
        nonce: <crate::BehaviorAddr<B> as Address>::Nonce,
    ) -> Result<
        bool,
        SupervisorError<core::convert::Infallible, <crate::BehaviorAddr<B> as Address>::Nonce>,
    > {
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

    fn replacement_decision(
        &mut self,
        event: &WorkerStopped<crate::BehaviorAddr<B>>,
    ) -> ReplacementResult<B, C> {
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
                    Delivery::local_child(
                        behavior::ChildRecipient::new(nonce),
                        ProxyCommand::Replace(child),
                    )
                })
                .collect(),
        ))
    }

    fn react_to_failure(
        &mut self,
        failure: &SupervisionFailure<crate::BehaviorAddr<B>>,
    ) -> Result<Become<B::Ph>, B::Error> {
        Ok(match (self.on_failure)(&mut self.inner, failure)? {
            Step::Continue => Step::Continue,
            Step::Goto(never) => match never {},
            Step::Stop(exit) => Step::Stop(exit),
        })
    }

    fn wrap(
        &mut self,
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, Births<C>>,
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
                creation_observations: ServiceSends::new(
                    actions
                        .creates
                        .iter()
                        .map(|create| ObserveCreation::new(create.nonce))
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
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Births<C>>,
    B::Protocol: crate::Protocol<Addr = A>,
    B::Event: crate::RouteInput<ChildStopped<A>>
        + crate::RouteInput<WorkerStopped<A>>
        + crate::RouteInput<CreationResolved<A>>
        + crate::RouteInput<WorkerCreationResolved<A::Nonce>>,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    type Protocol = B::Protocol;
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
            .extend(configured.iter().copied().map(ObserveChild::new));
        actions
            .sends
            .creation_observations
            .extend(configured.into_iter().map(ObserveCreation::new));
        Ok(actions)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<SupervisorActions<B, C>, Self::Error> {
        match event {
            SupervisionEvent::WorkerStopped(event) => {
                if !self.fleet.contains(event.proxy) {
                    return match B::Event::route(event) {
                        Ok(event) => {
                            let actions = behavior::delegate_transition(&mut self.inner, event)
                                .map_err(SupervisorError::Behavior)?;
                            self.wrap(actions)
                        }
                        Err(_) => Ok(Actions::cont()),
                    };
                }
                let decision = self.replacement_decision(&event)?;
                match decision {
                    ReplacementDecision::Retire => Ok(Actions::cont()),
                    ReplacementDecision::Replace(replacements) => Ok(Actions::new(
                        SupervisorSends {
                            behavior: B::Sends::empty(),
                            child_observations: ServiceSends::empty(),
                            creation_observations: ServiceSends::empty(),
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
                            creation_observations: ServiceSends::empty(),
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
                if !self.fleet.contains(event.nonce) {
                    return match B::Event::route(event) {
                        Ok(event) => {
                            let actions = behavior::delegate_transition(&mut self.inner, event)
                                .map_err(SupervisorError::Behavior)?;
                            self.wrap(actions)
                        }
                        Err(_) => Ok(Actions::cont()),
                    };
                }
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
                        creation_observations: ServiceSends::empty(),
                        replacement_commands: Vec::new(),
                        failure_reports: ServiceSends::one(ReportSupervisionFailure::new(failure)),
                    },
                    Vec::new(),
                    self.react_to_failure(&failure)
                        .map_err(SupervisorError::Behavior)?,
                ))
            }
            SupervisionEvent::CreationResolved(event) => {
                self.fleet
                    .resolve_creation(event.nonce, event.result.map(|_| ()));
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

#[cfg(test)]
mod ownership_tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{Activate as _, Crash, Exit, MailAddr, User};

    struct Child;

    impl crate::Protocol for Child {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = behavior::NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[derive(Default)]
    struct Parent {
        forwarded: usize,
    }

    impl crate::Protocol for Parent {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl crate::BehaviorBase for Parent {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl Behavior for Parent {
        type Protocol = Self;
        type Event = SupervisionEvent<User<MailAddr, ()>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<Child>;

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            if matches!(
                event,
                SupervisionEvent::ChildStopped(_) | SupervisionEvent::WorkerStopped(_)
            ) {
                self.forwarded += 1;
            }
            Ok(Actions::cont())
        }
    }

    fn child(_: usize) -> Option<Child> {
        Some(Child)
    }

    #[test]
    fn unrelated_stop_facts_route_to_the_nested_owner() {
        let definition = Supervisor::new(
            Parent::default(),
            behavior::ChildTopology::indexed(|_| 1, 1, child),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
            ),
        )
        .unwrap();
        let mut active = definition.initialize().unwrap().behavior;

        active
            .on(ChildStopped::new(9, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        active
            .on(WorkerStopped::new(
                9,
                10,
                Err(Crash::Failed),
                Instant::now(),
            ))
            .unwrap();

        assert_eq!(active.base().forwarded, 2);
        assert_eq!(active.child_count(), 1);
    }
}
