//! Fleet coordination for supervised stable proxy actors.

use std::time::Duration;

use super::super::domain::{FixedFleetOwnership, FleetError, OwnershipError, OwnershipFold};
use super::super::policy::{
    ReportSupervisionFailure, RestartPolicy, Strategy, SupervisionFailure,
    SupervisionFailureReaction, retire_on_supervision_failure,
};
use super::super::protocol::SupervisionEvent;
use super::proxy::{Proxy, ProxyWithParent};
use crate::Become;
use crate::protocol::{ObserveChild, ObserveCreation, ShutdownChild};
use crate::{Own, SendInput};
use behavior::{
    Actions, Address, Behavior, Births, Delivery, InterpreterRequests, SendEffects, SendLayer,
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
pub struct SupervisorSends<A, C, ParentPath = behavior::Here>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    /// Requests to observe every accepted stable proxy creation.
    pub child_observations: InterpreterRequests<ObserveChild<A>>,
    /// Requests for the committed result of every staged stable proxy creation.
    pub creation_observations: InterpreterRequests<ObserveCreation<A>>,
    /// Commands asking stable proxies to install fresh worker incarnations.
    pub replacement_commands: Vec<Delivery<Proxy<C>>>,
    /// Typed terminal supervision failures for the local runtime observer.
    pub failure_reports: InterpreterRequests<ReportSupervisionFailure<A>>,
    /// Orderly shutdown requests for stable proxies owned by this supervisor.
    pub shutdowns: InterpreterRequests<ShutdownChild<ProxyWithParent<C, ParentPath>>>,
}

impl<A, C, ParentPath> SendEffects for SupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn empty() -> Self {
        Self {
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            replacement_commands: Vec::new(),
            failure_reports: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.replacement_commands.extend(other.replacement_commands);
        self.failure_reports.append(other.failure_reports);
        self.shutdowns.append(other.shutdowns);
    }
}

impl<A, Event, C, ParentPath> behavior::SendsFor<SupervisionEvent<Event>>
    for SupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    Event: behavior::UserEvent<Addr = A>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    InterpreterRequests<ObserveChild<A>>: behavior::SendsFor<SupervisionEvent<Event>>,
    InterpreterRequests<ObserveCreation<A>>: behavior::SendsFor<SupervisionEvent<Event>>,
    InterpreterRequests<ShutdownChild<ProxyWithParent<C, ParentPath>>>:
        behavior::SendsFor<SupervisionEvent<Event>>,
{
}

impl<Interpreter, RootEvent, Path, A, C, ParentPath>
    behavior::InterpretSends<Interpreter, RootEvent, Path> for SupervisorSends<A, C, ParentPath>
where
    Interpreter: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    InterpreterRequests<ObserveChild<A>>: behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ObserveCreation<A>>: behavior::InterpretSends<Interpreter, RootEvent, Path>,
    Vec<Delivery<Proxy<C>>>: behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ReportSupervisionFailure<A>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<ProxyWithParent<C, ParentPath>>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    SupervisorSends<A, C, ParentPath>: Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            self.child_observations.interpret(interpreter).await?;
            self.creation_observations.interpret(interpreter).await?;
            self.replacement_commands.interpret(interpreter).await?;
            self.failure_reports.interpret(interpreter).await?;
            self.shutdowns.interpret(interpreter).await
        }
    }
}

impl<A, C, ParentPath> SendInput<ObserveCreation<A>, Own> for SupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: ObserveCreation<A>) {
        self.creation_observations.send(input);
    }
}

impl<A, C, ParentPath> SendInput<ObserveChild<A>, Own> for SupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: ObserveChild<A>) {
        self.child_observations.send(input);
    }
}

impl<A, C, ParentPath> SendInput<Delivery<Proxy<C>>, Own> for SupervisorSends<A, C, ParentPath>
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

impl<A, C, ParentPath> SendInput<ReportSupervisionFailure<A>, Own>
    for SupervisorSends<A, C, ParentPath>
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

impl<A, C, ParentPath> SendInput<ShutdownChild<ProxyWithParent<C, ParentPath>>, Own>
    for SupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn emit(&mut self, input: ShutdownChild<ProxyWithParent<C, ParentPath>>) {
        self.shutdowns.send(input);
    }
}

pub(crate) type SupervisorActions<B, C, ParentPath = behavior::Here> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<SupervisorSends<crate::BehaviorAddr<B>, C, ParentPath>, <B as Behavior>::Sends>,
    Births<ProxyWithParent<C, ParentPath>>,
>;

/// A controlled supervisor-fold failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SuperviseError<E, N> {
    /// The supervised behavior rejected its fold.
    #[error("supervised behavior rejected the transition")]
    Behavior(#[source] E),
    /// The supervisor's child topology rejected the operation.
    #[error(transparent)]
    Fleet(#[from] FleetError<N>),
    /// The configured worker factory did not define a requested fleet index.
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    /// An owned proxy rejected terminal subtree shutdown.
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: N,
        reason: crate::ChildShutdownRejection,
    },
    /// A creation result carried provenance different from the pending request.
    #[error("stable-child creation provenance did not match the pending request")]
    CreationProvenanceMismatch {
        nonce: N,
        expected: crate::CreationKind<N>,
        observed: crate::CreationKind<N>,
    },
    /// A worker-incarnation result carried provenance different from the pending request.
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        proxy: N,
        worker: N,
        expected: crate::CreationKind<N>,
        observed: crate::CreationKind<N>,
    },
}

fn map_ownership_error<E, N>(error: OwnershipError<N>) -> SuperviseError<E, N> {
    match error {
        OwnershipError::Fleet(error) => SuperviseError::Fleet(error),
        OwnershipError::FactoryIndex { index } => SuperviseError::FactoryIndex { index },
        OwnershipError::ChildShutdownRejected { nonce, reason } => {
            SuperviseError::ChildShutdownRejected { nonce, reason }
        }
        OwnershipError::CreationProvenanceMismatch {
            nonce,
            expected,
            observed,
        } => SuperviseError::CreationProvenanceMismatch {
            nonce,
            expected,
            observed,
        },
        OwnershipError::WorkerCreationProvenanceMismatch {
            proxy,
            worker,
            expected,
            observed,
        } => SuperviseError::WorkerCreationProvenanceMismatch {
            proxy,
            worker,
            expected,
            observed,
        },
    }
}

/// Application behavior composed with fixed stable-proxy ownership.
///
/// Shutdown cancels replacement decisions, waits for pending proxy installation
/// to resolve, requests each established proxy exactly once, and stops only
/// after every matching [`crate::ChildStopped`] fact. New inner creations that
/// were already accepted during the drain join the same awaiting set.
///
/// `B::Birth = Births<C>` is the application's real additional-child lane:
/// every creation emitted by `B` is adopted into this ownership domain. When
/// no application transition creates `C`, use standalone [`crate::Supervisor`]
/// instead of manufacturing an inert `B`.
pub struct SuperviseWithParent<B: Behavior, C: Behavior<Ph = Never>, ParentPath>
where
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
{
    inner: B,
    ownership: FixedFleetOwnership<crate::BehaviorAddr<B>, C, ParentPath>,
    on_failure: SupervisionFailureReaction<B>,
}

/// A fixed supervisor whose proxy reports target its direct event layer.
pub type Supervise<B, C> = SuperviseWithParent<B, C, behavior::Here>;

impl<B, C> SuperviseWithParent<B, C, behavior::Here>
where
    B: Behavior<Birth = Births<C>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    /// Construct a fixed supervisor whose proxy reports target this behavior's
    /// direct supervision event layer.
    pub fn new(
        inner: B,
        topology: ChildTopology<<crate::BehaviorAddr<B> as Address>::Nonce, C>,
        restart: RestartConfiguration,
    ) -> Result<Self, FleetError<<crate::BehaviorAddr<B> as Address>::Nonce>> {
        Self::with_parent(inner, topology, restart, crate::ProxyParentIngress::new())
    }
}

impl<B, C, ParentPath> crate::BehaviorBase for SuperviseWithParent<B, C, ParentPath>
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

impl<B, C, ParentPath> crate::StashStatus for SuperviseWithParent<B, C, ParentPath>
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

impl<B, C, ParentPath> SuperviseWithParent<B, C, ParentPath>
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
    pub fn with_parent(
        inner: B,
        topology: ChildTopology<<crate::BehaviorAddr<B> as Address>::Nonce, C>,
        restart: RestartConfiguration,
        proxy_parent: crate::ProxyParentIngress<crate::BehaviorAddr<B>, ParentPath>,
    ) -> Result<Self, FleetError<<crate::BehaviorAddr<B> as Address>::Nonce>> {
        Ok(Self {
            inner,
            ownership: FixedFleetOwnership::new(topology, restart, proxy_parent)?,
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
        SuperviseError<core::convert::Infallible, <crate::BehaviorAddr<B> as Address>::Nonce>,
    > {
        Ok(self.ownership.is_alive(nonce)?)
    }

    #[must_use]
    pub fn child_count(&self) -> usize {
        self.ownership.child_count()
    }

    #[must_use]
    pub fn restarts_in_window(&self) -> usize {
        self.ownership.restarts_in_window()
    }

    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.ownership.is_shutting_down()
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
    ) -> Result<
        SupervisorActions<B, C, ParentPath>,
        SuperviseError<B::Error, <crate::BehaviorAddr<B> as Address>::Nonce>,
    > {
        let become_ = if self.is_shutting_down() {
            Step::Continue
        } else {
            actions.become_
        };
        let owned = self
            .ownership
            .adopt(actions.creates)
            .map_err(map_ownership_error)?;
        Ok(Actions::new(
            SendLayer::new(owned.sends, actions.sends),
            owned.creates,
            become_,
        ))
    }

    fn wrap_ownership(
        &mut self,
        fold: OwnershipFold<crate::BehaviorAddr<B>, C, ParentPath>,
    ) -> Result<
        SupervisorActions<B, C, ParentPath>,
        SuperviseError<B::Error, <crate::BehaviorAddr<B> as Address>::Nonce>,
    > {
        let become_ = match fold.failure {
            Some(failure) => self
                .react_to_failure(&failure)
                .map_err(SuperviseError::Behavior)?,
            None => match fold.actions.become_ {
                Step::Continue => Step::Continue,
                Step::Goto(never) => match never {},
                Step::Stop(exit) => Step::Stop(exit),
            },
        };
        Ok(Actions::new(
            SendLayer::new(fold.actions.sends, B::Sends::empty()),
            fold.actions.creates,
            become_,
        ))
    }
}

impl<B, C, A, Ph, Sends, ParentPath> Behavior for SuperviseWithParent<B, C, ParentPath>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Births<C>>,
    B::Protocol: crate::Protocol<Addr = A>,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    type Protocol = B::Protocol;
    type Event = SupervisionEvent<B::Event>;
    type Sends = SendLayer<SupervisorSends<A, C, ParentPath>, Sends>;
    type Ph = Ph;
    type Error = SuperviseError<B::Error, A::Nonce>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<SupervisorActions<B, C, ParentPath>, Self::Error> {
        let actions = behavior::initialize(&mut self.inner).map_err(SuperviseError::Behavior)?;
        let mut actions = self.wrap(actions)?;
        let configured = self.ownership.initialize().map_err(map_ownership_error)?;
        actions.sends.owned.append(configured.sends);
        actions.creates.extend(configured.creates);
        Ok(actions)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<SupervisorActions<B, C, ParentPath>, Self::Error> {
        match event {
            SupervisionEvent::WorkerStopped(event) => self
                .ownership
                .worker_stopped(event)
                .map_err(map_ownership_error)
                .and_then(|fold| self.wrap_ownership(fold)),
            SupervisionEvent::ChildStopped(event) => self
                .ownership
                .child_stopped(event)
                .map_err(map_ownership_error)
                .and_then(|fold| self.wrap_ownership(fold)),
            SupervisionEvent::CreationResolved(event) => self
                .ownership
                .creation_resolved(event)
                .map_err(map_ownership_error)
                .and_then(|fold| self.wrap_ownership(fold)),
            SupervisionEvent::WorkerCreationResolved(event) => self
                .ownership
                .worker_creation_resolved(event)
                .map_err(map_ownership_error)
                .and_then(|fold| self.wrap_ownership(fold)),
            SupervisionEvent::Behavior(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)
                    .map_err(SuperviseError::Behavior)?;
                self.wrap(actions)
            }
            SupervisionEvent::ShutdownRequested(_) => {
                let fold = self.ownership.shutdown();
                self.wrap_ownership(fold)
            }
            SupervisionEvent::ChildShutdownRejected(event) => self
                .ownership
                .child_shutdown_rejected(event)
                .map_err(map_ownership_error)
                .and_then(|fold| self.wrap_ownership(fold)),
        }
    }
}

#[cfg(test)]
mod ownership_tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{
        Activate as _, BehaviorActed, ChildStopped, ChildTopology, Crash, Exit, MailAddr, User,
        WorkerStopped,
    };
    use behavior::Create;

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
                &event,
                SupervisionEvent::ChildStopped(_) | SupervisionEvent::WorkerStopped(_)
            ) {
                self.forwarded += 1;
            }
            if matches!(&event, SupervisionEvent::Behavior(_)) {
                return Ok(Actions::new(
                    Vec::new(),
                    vec![Create::birth(2, Child)],
                    Step::Stop(behavior::Stopped),
                ));
            }
            Ok(Actions::cont())
        }
    }

    fn child(_: usize) -> Option<Child> {
        Some(Child)
    }

    #[test]
    fn ownership_path_distinguishes_stale_outer_facts_from_inner_facts() {
        let definition = Supervise::new(
            Parent::default(),
            ChildTopology::indexed(|_| 1, 1, child),
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
            .transition(SupervisionEvent::ChildStopped(ChildStopped::new(
                9,
                Ok(Exit::Normal),
                Instant::now(),
            )))
            .unwrap();
        active
            .transition(SupervisionEvent::WorkerStopped(WorkerStopped::new(
                9,
                10,
                Err(Crash::Failed),
                Instant::now(),
            )))
            .unwrap();

        assert_eq!(active.base().forwarded, 0);

        active
            .transition(SupervisionEvent::Behavior(SupervisionEvent::ChildStopped(
                ChildStopped::new(9, Ok(Exit::Normal), Instant::now()),
            )))
            .unwrap();
        assert_eq!(active.base().forwarded, 1);
        assert_eq!(active.child_count(), 1);
    }

    #[test]
    fn fixed_supervisor_drains_installed_and_pending_proxies_before_stopping() {
        let definition = Supervise::new(
            Parent::default(),
            ChildTopology::indexed(|index| u64::try_from(index).unwrap(), 2, child),
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
            .on(crate::CreationResolved::birth(0, MailAddr(10)))
            .unwrap();

        let requested = active.on(crate::ShutdownRequested).unwrap();
        assert_eq!(
            requested.sends.owned.shutdowns.as_slice(),
            [ShutdownChild::new(0)]
        );
        assert!(
            active
                .on(crate::ShutdownRequested)
                .unwrap()
                .sends
                .owned
                .shutdowns
                .is_empty()
        );
        assert!(matches!(
            active.on(crate::ChildShutdownRejected::new(
                0,
                crate::ChildShutdownRejection::AlreadyStopping,
            )),
            Err(SuperviseError::ChildShutdownRejected {
                nonce: 0,
                reason: crate::ChildShutdownRejection::AlreadyStopping,
            })
        ));
        active
            .on(crate::CreationResolved::birth(1, MailAddr(11)))
            .unwrap();
        let created_during_drain = active
            .transition(SupervisionEvent::Behavior(SupervisionEvent::Behavior(
                User::new(MailAddr(1), ()),
            )))
            .unwrap();
        assert_eq!(created_during_drain.creates.len(), 1);
        assert!(matches!(created_during_drain.become_, Step::Continue));
        let installed_during_drain = active
            .on(crate::CreationResolved::birth(2, MailAddr(12)))
            .unwrap();
        assert_eq!(
            installed_during_drain.sends.owned.shutdowns.as_slice(),
            [ShutdownChild::new(2)]
        );
        assert!(matches!(
            active
                .on(ChildStopped::new(0, Ok(Exit::Normal), Instant::now()))
                .unwrap()
                .become_,
            Step::Continue
        ));
        assert!(matches!(
            active
                .on(ChildStopped::new(1, Ok(Exit::Normal), Instant::now()))
                .unwrap()
                .become_,
            Step::Continue
        ));
        assert!(matches!(
            active
                .on(ChildStopped::new(2, Ok(Exit::Normal), Instant::now()))
                .unwrap()
                .become_,
            Step::Stop(behavior::Stopped)
        ));
    }
}
