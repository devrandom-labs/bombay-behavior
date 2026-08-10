//! Fleet coordination for supervised stable proxy actors.

use std::time::Duration;

use super::super::domain::{Fleet, RestartBudget};
use super::super::policy::{
    RestartPolicy, Strategy, SupervisionFailure, SupervisionFailureReaction,
    retire_on_supervision_failure,
};
use super::super::protocol::{ProxyCommand, SupervisionEvent};
use super::proxy::Proxy;
use crate::behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, ServiceSends,
};
use crate::next::{Never, Step};
use crate::protocol::{
    ChildEvent, CreationEvent, ObserveChild, WorkerCreationEvent, WorkerStopped,
};
use crate::{Become, Exit, SupervisionFailureReason};

/// Named effect lanes emitted by a supervised behavior.
pub struct SupervisorSends<A: Address, Sends, C: Behavior<Addr = A>> {
    pub behavior: Sends,
    pub child_observations: ServiceSends<ObserveChild<A::Nonce>>,
    pub replacement_commands: Vec<Delivery<A, ProxyCommand<C>>>,
}

impl<A, Sends, C> SendAlgebra for SupervisorSends<A, Sends, C>
where
    A: Address,
    Sends: SendAlgebra,
    C: Behavior<Addr = A>,
{
    fn empty() -> Self {
        Self {
            behavior: Sends::empty(),
            child_observations: ServiceSends::empty(),
            replacement_commands: Vec::new(),
        }
    }

    fn append(&mut self, other: Self) {
        self.behavior.append(other.behavior);
        self.child_observations.append(other.child_observations);
        self.replacement_commands.extend(other.replacement_commands);
    }
}

pub type SupervisorActions<B, C> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    SupervisorSends<<B as Behavior>::Addr, <B as Behavior>::Sends, C>,
    Births<Proxy<C>>,
>;

enum ReplacementDecision<A: Address, C: Behavior<Addr = A>> {
    Retire,
    Replace(Vec<Delivery<A, ProxyCommand<C>>>),
    Failed(SupervisionFailure<A>),
}

pub struct Supervisor<B: Behavior, C: Behavior<Ph = Never, Addr = B::Addr>> {
    inner: B,
    fleet: Fleet<<B::Addr as Address>::Nonce>,
    build: fn(usize) -> C,
    strategy: Strategy,
    policy: RestartPolicy,
    budget: RestartBudget,
    on_failure: SupervisionFailureReaction<B>,
}

impl<B, C> Supervisor<B, C>
where
    B: Behavior<Birth = Births<C>>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    #[allow(clippy::too_many_arguments, reason = "hidden by Compose")]
    /// Construct the concrete supervisor behavior hidden by `Compose`.
    ///
    /// # Panics
    /// Panics when configured child nonces are not unique. Such a topology
    /// would violate creator-local child routing and creation freshness before
    /// the behavior could return an initialization result.
    #[must_use]
    pub fn new(
        inner: B,
        nonces: fn(usize) -> <B::Addr as Address>::Nonce,
        count: usize,
        build: fn(usize) -> C,
        strategy: Strategy,
        policy: RestartPolicy,
        max_restarts: u32,
        window: Duration,
    ) -> Self {
        let fleet = Fleet::configured((0..count).map(nonces))
            .unwrap_or_else(|_| panic!("configured child nonces must be fresh"));
        Self {
            inner,
            fleet,
            build,
            strategy,
            policy,
            budget: RestartBudget::new(max_restarts, window),
            on_failure: retire_on_supervision_failure::<B>,
        }
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

    #[must_use]
    /// Report whether a known supervised proxy is alive.
    ///
    /// # Panics
    /// Panics when `nonce` is not part of this supervisor topology.
    pub fn is_alive(&self, nonce: <B::Addr as Address>::Nonce) -> bool {
        self.fleet
            .is_available(nonce)
            .unwrap_or_else(|_| panic!("unknown supervised nonce"))
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
        event: &WorkerStopped<B::Addr>,
    ) -> ReplacementDecision<B::Addr, C> {
        let eligible = match self.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => {
                !matches!(&event.outcome, Ok(Exit::Normal | Exit::Collected))
            }
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            self.fleet
                .retire(event.proxy)
                .unwrap_or_else(|_| panic!("unknown supervised nonce"));
            return ReplacementDecision::Retire;
        }
        let candidates = self
            .fleet
            .replacements(event.proxy, self.strategy)
            .unwrap_or_else(|_| panic!("unknown supervised nonce"));
        if let Err(reason) = self.budget.admit(event.at, candidates.len()) {
            self.fleet
                .retire(event.proxy)
                .unwrap_or_else(|_| panic!("unknown supervised nonce"));
            return ReplacementDecision::Failed(SupervisionFailure::new(
                event.proxy,
                event.outcome,
                SupervisionFailureReason::RestartDenied(reason),
            ));
        }
        for candidate in &candidates {
            self.fleet
                .replacement_requested(candidate.nonce)
                .unwrap_or_else(|_| unreachable!("candidate belongs to fleet"));
        }
        ReplacementDecision::Replace(
            candidates
                .into_iter()
                .map(|candidate| {
                    Delivery::new(
                        Recipient::child(candidate.nonce),
                        ProxyCommand::Replace((self.build)(candidate.index)),
                    )
                })
                .collect(),
        )
    }

    fn react_to_failure(
        &mut self,
        failure: &SupervisionFailure<B::Addr>,
    ) -> Result<Become<B::Addr, B::Ph>, B::Error> {
        Ok(match (self.on_failure)(&mut self.inner, failure)? {
            Step::Continue => Step::Continue,
            Step::Goto(never) => match never {},
            Step::Stop(exit) => Step::Stop(exit),
        })
    }

    fn wrap(
        &mut self,
        actions: Actions<B::Addr, B::Ph, B::Sends, Births<C>>,
    ) -> SupervisorActions<B, C> {
        let born: Vec<_> = actions.creates.iter().map(|create| create.nonce).collect();
        for create in &actions.creates {
            self.fleet
                .register(create.nonce)
                .unwrap_or_else(|_| panic!("a child birth nonce must be fresh"));
        }
        Actions::new(
            SupervisorSends {
                behavior: actions.sends,
                child_observations: ServiceSends::new(
                    born.into_iter().map(ObserveChild::new).collect(),
                ),
                replacement_commands: Vec::new(),
            },
            actions
                .creates
                .into_iter()
                .map(|create| Create::new(create.nonce, Proxy::new(create.child), create.kind))
                .collect(),
            actions.become_,
        )
    }
}

impl<B, C, A, Ph, Sends> Behavior for Supervisor<B, C>
where
    A: Address,
    Sends: SendAlgebra,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Births<C>>,
    B::Event: ChildEvent + CreationEvent + WorkerCreationEvent,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = SupervisionEvent<B::Event>;
    type Sends = SupervisorSends<A, Sends, C>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self) -> Result<SupervisorActions<B, C>, B::Error> {
        let actions = self.inner.init()?;
        let mut actions = self.wrap(actions);
        actions.creates.extend(
            self.fleet
                .configured_nonces()
                .enumerate()
                .map(|(index, nonce)| Create::birth(nonce, Proxy::new((self.build)(index)))),
        );
        actions
            .sends
            .child_observations
            .extend(self.fleet.configured_nonces().map(ObserveChild::new));
        Ok(actions)
    }

    fn transition(&mut self, event: Self::Event) -> Result<SupervisorActions<B, C>, B::Error> {
        match event {
            SupervisionEvent::WorkerStopped(event) => {
                let decision = self.replacement_decision(&event);
                match decision {
                    ReplacementDecision::Retire => Ok(Actions::cont()),
                    ReplacementDecision::Replace(replacements) => Ok(Actions::new(
                        SupervisorSends {
                            behavior: B::Sends::empty(),
                            child_observations: ServiceSends::empty(),
                            replacement_commands: replacements,
                        },
                        Vec::new(),
                        Step::Continue,
                    )),
                    ReplacementDecision::Failed(failure) => {
                        Ok(Actions::just(self.react_to_failure(&failure)?))
                    }
                }
            }
            SupervisionEvent::ChildStopped(event) => {
                self.fleet
                    .retire(event.nonce)
                    .unwrap_or_else(|_| panic!("unknown supervised nonce"));
                let failure = SupervisionFailure::new(
                    event.nonce,
                    event.outcome,
                    SupervisionFailureReason::StableChildStopped,
                );
                Ok(Actions::just(self.react_to_failure(&failure)?))
            }
            SupervisionEvent::CreationResolved(event) => {
                self.fleet.resolve_creation(event.nonce, event.result);
                if let Some(event) = B::Event::creation_resolved(event) {
                    let actions = self.inner.transition(event)?;
                    Ok(self.wrap(actions))
                } else {
                    Ok(Actions::cont())
                }
            }
            SupervisionEvent::WorkerCreationResolved(event) => {
                // Worker realization does not change the stable proxy's
                // liveness. The typed result remains distinct from a proxy
                // terminal observation.
                if let Some(event) = B::Event::worker_creation_resolved(event) {
                    let actions = self.inner.transition(event)?;
                    Ok(self.wrap(actions))
                } else {
                    Ok(Actions::cont())
                }
            }
            SupervisionEvent::Inner(event) => {
                let actions = self.inner.transition(event)?;
                Ok(self.wrap(actions))
            }
        }
    }
}
