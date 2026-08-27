//! Fleet coordination for supervised stable proxy actors.

use std::time::Duration;

use super::super::Backoff;
use super::super::domain::{FixedFleetOwnership, FleetError, OwnershipError, OwnershipFold};
use super::super::policy::{
    ReportSupervisionFailure, RestartPolicy, Strategy, SupervisionFailure,
    SupervisionFailureReaction, retire_on_supervision_failure,
};
use super::super::protocol::SupervisionEvent;
use crate::Become;
use crate::protocol::{
    ObserveChild, ObserveCreation, ReplacementRequested, ScheduleAfter, ShutdownChild,
};
use crate::{Own, SendInput};
use behavior::{
    Actions, Address, Behavior, BehaviorLayer, BirthMode, BirthNodeAppend, Births, ChildInput,
    ChildInputIngress, InterpreterRequests, SendEffects, SendLayer,
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

/// When an admitted replacement communication is emitted.
///
/// Restart delay is a Bombay supervision policy. It does not change replacement
/// eligibility, budget admission, creation provenance, or stable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartTiming {
    /// Emit the accepted replacement communication in the stopping worker's
    /// transition.
    Immediate,
    /// Retain the complete accepted replacement batch until its exact timer
    /// generation is observed.
    Delayed(Backoff),
}

/// Supervision strategy, eligibility, restart budget, and emission timing.
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
    /// Timing of an admitted replacement batch.
    pub timing: RestartTiming,
}

impl RestartConfiguration {
    /// Define a complete restart policy.
    #[must_use]
    pub const fn new(
        strategy: Strategy,
        policy: RestartPolicy,
        maximum: u32,
        window: Duration,
        timing: RestartTiming,
    ) -> Self {
        Self {
            strategy,
            policy,
            maximum,
            window,
            timing,
        }
    }
}

/// Named effect lanes emitted by a supervised behavior.
pub struct SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    pub child_observations: InterpreterRequests<ObserveChild<A, behavior::ChildHead>>,
    pub creation_observations: InterpreterRequests<ObserveCreation<A, behavior::ChildHead>>,
    pub schedules: InterpreterRequests<ScheduleAfter>,
    pub replacement_inputs:
        Vec<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>>,
    pub failure_reports: InterpreterRequests<ReportSupervisionFailure<A>>,
    pub shutdowns: InterpreterRequests<ShutdownChild<Stable, behavior::ChildHead>>,
}

impl<A, C, Stable> SendEffects for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn empty() -> Self {
        Self {
            child_observations: InterpreterRequests::empty(),
            creation_observations: InterpreterRequests::empty(),
            schedules: InterpreterRequests::empty(),
            replacement_inputs: Vec::new(),
            failure_reports: InterpreterRequests::empty(),
            shutdowns: InterpreterRequests::empty(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.schedules.append(other.schedules);
        self.replacement_inputs
            .append(&mut other.replacement_inputs);
        self.failure_reports.append(other.failure_reports);
        self.shutdowns.append(other.shutdowns);
    }
}

impl<A, Event, C, Stable> behavior::SendsFor<Event> for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    InterpreterRequests<ObserveChild<A, behavior::ChildHead>>: behavior::SendsFor<Event>,
    InterpreterRequests<ObserveCreation<A, behavior::ChildHead>>: behavior::SendsFor<Event>,
    InterpreterRequests<ScheduleAfter>: behavior::SendsFor<Event>,
    InterpreterRequests<ShutdownChild<Stable, behavior::ChildHead>>: behavior::SendsFor<Event>,
{
}

impl<Interpreter, RootEvent, Path, A, C, Stable>
    behavior::InterpretSends<Interpreter, RootEvent, Path> for SupervisorSends<A, C, Stable>
where
    Interpreter: behavior::SendInterpreter,
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    InterpreterRequests<ObserveChild<A, behavior::ChildHead>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ObserveCreation<A, behavior::ChildHead>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ScheduleAfter>: behavior::InterpretSends<Interpreter, RootEvent, Path>,
    Vec<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ReportSupervisionFailure<A>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ShutdownChild<Stable, behavior::ChildHead>>:
        behavior::InterpretSends<Interpreter, RootEvent, Path>,
    SupervisorSends<A, C, Stable>: Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            self.child_observations.interpret(interpreter).await?;
            self.creation_observations.interpret(interpreter).await?;
            self.schedules.interpret(interpreter).await?;
            self.replacement_inputs.interpret(interpreter).await?;
            self.failure_reports.interpret(interpreter).await?;
            self.shutdowns.interpret(interpreter).await
        }
    }
}

impl<A, C, Stable> SendInput<ObserveCreation<A, behavior::ChildHead>, Own>
    for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn emit(&mut self, input: ObserveCreation<A, behavior::ChildHead>) {
        self.creation_observations.send(input);
    }
}

impl<A, C, Stable> SendInput<ObserveChild<A, behavior::ChildHead>, Own>
    for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn emit(&mut self, input: ObserveChild<A, behavior::ChildHead>) {
        self.child_observations.send(input);
    }
}

impl<A, C, Stable> SendInput<ScheduleAfter, Own> for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn emit(&mut self, input: ScheduleAfter) {
        self.schedules.send(input);
    }
}

impl<A, C, Stable>
    SendInput<ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>, Own>
    for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn emit(&mut self, input: ChildInput<Stable, C, ReplacementRequested<C>, behavior::ChildHead>) {
        self.replacement_inputs.push(input);
    }
}

impl<A, C, Stable> SendInput<ReportSupervisionFailure<A>, Own> for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn emit(&mut self, input: ReportSupervisionFailure<A>) {
        self.failure_reports.send(input);
    }
}

impl<A, C, Stable> SendInput<ShutdownChild<Stable, behavior::ChildHead>, Own>
    for SupervisorSends<A, C, Stable>
where
    A: Address,
    C: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
    Stable: Behavior<Ph = Never, Protocol: crate::Protocol<Addr = A>>,
{
    fn emit(&mut self, input: ShutdownChild<Stable, behavior::ChildHead>) {
        self.shutdowns.send(input);
    }
}

/// A controlled supervisor-fold failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SuperviseError<E, A: Address> {
    /// The supervised behavior rejected its fold.
    #[error("supervised behavior rejected the transition")]
    Behavior(#[source] E),
    /// The supervisor's child topology rejected the operation.
    #[error(transparent)]
    Fleet(#[from] FleetError<A::Nonce>),
    /// The configured worker factory did not define a requested fleet index.
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    /// An owned proxy rejected terminal subtree shutdown.
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected(crate::ChildShutdownRejected<A::Nonce>),
    #[error("a creation result does not belong to a pending stable-child creation")]
    UnexpectedCreation(crate::CreationResolved<A>),
    #[error("a stable-child stop does not belong to the current ownership state")]
    UnexpectedChildStopped(crate::ChildStopped<A>),
    #[error("a worker stop does not belong to the current worker incarnation")]
    UnexpectedWorkerStopped(crate::WorkerStopped<A>),
    #[error("a worker creation result does not belong to a pending worker creation")]
    UnexpectedWorkerCreation(crate::WorkerCreationResolved<A::Nonce>),
    #[error("a child-shutdown rejection does not belong to an outstanding shutdown request")]
    UnexpectedChildShutdownRejection(crate::ChildShutdownRejected<A::Nonce>),
    /// A creation result carried provenance different from the pending request.
    #[error("stable-child creation provenance did not match the pending request")]
    CreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::CreationResolved<A>,
    },
    /// A worker fact proves that a stable proxy creation reported as rejected
    /// had nevertheless existed.
    #[error("a rejected stable-proxy creation contradicts its worker-creation fact")]
    ContradictoryStableAndWorkerCreation {
        proxy: crate::CreationResolved<A>,
        worker: crate::WorkerCreationResolved<A::Nonce>,
    },
    /// A worker stop proves that a stable proxy creation reported as rejected
    /// had nevertheless existed.
    #[error("a rejected stable-proxy creation contradicts its worker-stop fact")]
    ContradictoryStableCreationAndWorkerStop {
        proxy: crate::CreationResolved<A>,
        worker: crate::WorkerStopped<A>,
    },
    /// A worker-incarnation result carried provenance different from the pending request.
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::WorkerCreationResolved<A::Nonce>,
    },
    #[error("an exact delayed-restart timer contradicts the retained worker phase")]
    DelayedReplacementStateMismatch {
        event: crate::TimerElapsed,
        child: A::Nonce,
    },
}

pub(crate) fn map_ownership_error<E, A: Address>(error: OwnershipError<A>) -> SuperviseError<E, A> {
    match error {
        OwnershipError::Fleet(error) => SuperviseError::Fleet(error),
        OwnershipError::FactoryIndex { index } => SuperviseError::FactoryIndex { index },
        OwnershipError::ChildShutdownRejected(event) => {
            SuperviseError::ChildShutdownRejected(event)
        }
        OwnershipError::UnexpectedCreation(event) => SuperviseError::UnexpectedCreation(event),
        OwnershipError::UnexpectedChildStopped(event) => {
            SuperviseError::UnexpectedChildStopped(event)
        }
        OwnershipError::UnexpectedWorkerStopped(event) => {
            SuperviseError::UnexpectedWorkerStopped(event)
        }
        OwnershipError::UnexpectedWorkerCreation(event) => {
            SuperviseError::UnexpectedWorkerCreation(event)
        }
        OwnershipError::UnexpectedChildShutdownRejection(event) => {
            SuperviseError::UnexpectedChildShutdownRejection(event)
        }
        OwnershipError::CreationProvenanceMismatch { expected, observed } => {
            SuperviseError::CreationProvenanceMismatch { expected, observed }
        }
        OwnershipError::ContradictoryStableAndWorkerCreation { proxy, worker } => {
            SuperviseError::ContradictoryStableAndWorkerCreation { proxy, worker }
        }
        OwnershipError::ContradictoryStableCreationAndWorkerStop { proxy, worker } => {
            SuperviseError::ContradictoryStableCreationAndWorkerStop { proxy, worker }
        }
        OwnershipError::WorkerCreationProvenanceMismatch { expected, observed } => {
            SuperviseError::WorkerCreationProvenanceMismatch { expected, observed }
        }
        OwnershipError::DelayedReplacementStateMismatch { event, child } => {
            SuperviseError::DelayedReplacementStateMismatch { event, child }
        }
    }
}

/// Application behavior composed with fixed stable-proxy ownership.
///
/// Shutdown cancels replacement decisions, waits for pending proxy installation
/// to resolve, requests each established proxy exactly once, and stops only
/// after every matching [`crate::ChildStopped`] fact.
///
/// The inner behavior's birth algebra is preserved unchanged. Fixed supervised
/// children are appended as a distinct structural occurrence; an arbitrary
/// inner birth is never reinterpreted as a member of the fixed topology.
///
/// This composition preserves `B`'s public protocol and does not itself issue
/// a stable-child domain capability. If `B` owns a route to one of the
/// configured stable children, its event algebra must explicitly accept the
/// complete [`crate::ProxyUnavailable`] value at that exact child occurrence.
/// The common supervision event then routes the parent report into `B` as an
/// ordinary successful domain transition; expected unavailability is never a
/// supervision-fold failure. A lifecycle-only use needs no such event lane.
pub struct Supervise<B: Behavior, C: Behavior<Ph = Never>, L>
where
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    inner: B,
    ownership: FixedFleetOwnership<crate::BehaviorAddr<B>, C, L::Output>,
    layer: L,
    on_failure: SupervisionFailureReaction<B>,
}

impl<B, C, L> Supervise<B, C, L>
where
    B: Behavior,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    /// Construct fixed stable-proxy ownership around `inner`.
    ///
    pub fn new(
        inner: B,
        topology: ChildTopology<<crate::BehaviorAddr<B> as Address>::Nonce, C>,
        restart: RestartConfiguration,
        layer: L,
    ) -> Result<Self, FleetError<<crate::BehaviorAddr<B> as Address>::Nonce>> {
        Ok(Self {
            inner,
            ownership: FixedFleetOwnership::new(topology, restart)?,
            layer,
            on_failure: retire_on_supervision_failure::<B>,
        })
    }
}

impl<B, C, L> crate::BehaviorBase for Supervise<B, C, L>
where
    B: Behavior + crate::BehaviorBase,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, C, L> crate::StashStatus for Supervise<B, C, L>
where
    B: Behavior + crate::StashStatus,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, C, L> Supervise<B, C, L>
where
    B: Behavior,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
    <B::Birth as BirthMode>::Child: BirthNodeAppend<L::Output>,
{
    #[must_use]
    /// Replace the pure reaction used for typed supervision failures.
    pub fn with_failure_reaction(mut self, reaction: SupervisionFailureReaction<B>) -> Self {
        self.on_failure = reaction;
        self
    }

    /// Report whether the interpreter has established the stable proxy.
    ///
    /// # Errors
    /// Returns the unknown nonce when it is not part of this topology.
    pub fn is_established(
        &self,
        nonce: <crate::BehaviorAddr<B> as Address>::Nonce,
    ) -> Result<bool, SuperviseError<core::convert::Infallible, crate::BehaviorAddr<B>>> {
        Ok(self.ownership.is_established(nonce)?)
    }

    /// Report whether the slot remains eligible for automatic replacement.
    pub fn is_restartable(
        &self,
        nonce: <crate::BehaviorAddr<B> as Address>::Nonce,
    ) -> Result<bool, SuperviseError<core::convert::Infallible, crate::BehaviorAddr<B>>> {
        Ok(self.ownership.is_restartable(nonce)?)
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
    pub fn pending_restarts(&self) -> usize {
        self.ownership.pending_restarts()
    }

    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.ownership.is_shutting_down()
    }

    fn react_to_failure(
        &self,
        failure: &SupervisionFailure<crate::BehaviorAddr<B>>,
    ) -> Become<B::Ph> {
        match (self.on_failure)(&self.inner, failure) {
            Step::Continue => Step::Continue,
            Step::Goto(never) => match never {},
            Step::Stop(exit) => Step::Stop(exit),
        }
    }

    fn combine_failure_reactions(
        &self,
        failure: Option<&SupervisionFailure<crate::BehaviorAddr<B>>>,
        accepted: Become<B::Ph>,
    ) -> Become<B::Ph> {
        if failure.is_some_and(|failure| matches!(self.react_to_failure(failure), Step::Stop(_))) {
            Step::Stop(behavior::Stopped)
        } else {
            accepted
        }
    }

    fn wrap(
        &mut self,
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> Actions<
        crate::BehaviorAddr<B>,
        B::Ph,
        SendLayer<SupervisorSends<crate::BehaviorAddr<B>, C, L::Output>, B::Sends>,
        Births<<<B::Birth as BirthMode>::Child as BirthNodeAppend<L::Output>>::Output>,
    > {
        let become_ = if self.is_shutting_down() {
            Step::Continue
        } else {
            actions.become_
        };
        let creates =
            <<B::Birth as BirthMode>::Child as BirthNodeAppend<L::Output>>::append_creations(
                actions.creates,
                Vec::new(),
            );
        Actions::new(
            SendLayer::new(SupervisorSends::empty(), actions.sends),
            creates,
            become_,
        )
    }

    fn wrap_ownership(
        &mut self,
        fold: OwnershipFold<crate::BehaviorAddr<B>, C, L::Output>,
    ) -> Result<
        Actions<
            crate::BehaviorAddr<B>,
            B::Ph,
            SendLayer<SupervisorSends<crate::BehaviorAddr<B>, C, L::Output>, B::Sends>,
            Births<<<B::Birth as BirthMode>::Child as BirthNodeAppend<L::Output>>::Output>,
        >,
        SuperviseError<B::Error, crate::BehaviorAddr<B>>,
    > {
        let accepted = match fold.actions.become_ {
            Step::Continue => Step::Continue,
            Step::Goto(never) => match never {},
            Step::Stop(exit) => Step::Stop(exit),
        };
        let become_ = self.combine_failure_reactions(fold.failure.as_ref(), accepted);
        let creates =
            <<B::Birth as BirthMode>::Child as BirthNodeAppend<L::Output>>::append_creations(
                Vec::new(),
                fold.actions.creates,
            );
        Ok(Actions::new(
            SendLayer::new(fold.actions.sends, B::Sends::empty()),
            creates,
            become_,
        ))
    }
}

impl<B, C, L, A, Ph, Sends> Behavior for Supervise<B, C, L>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    B: Behavior<Ph = Ph, Sends = Sends>,
    B::Protocol: crate::Protocol<Addr = A>,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    L: BehaviorLayer<C>,
    L::Output: Behavior<Ph = Never, Protocol = C::Protocol>,
    <L::Output as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
    <B::Birth as BirthMode>::Child: BirthNodeAppend<L::Output>,
{
    type Protocol = B::Protocol;
    type Event = SupervisionEvent<B::Event>;
    type Sends = SendLayer<SupervisorSends<A, C, L::Output>, Sends>;
    type Ph = Ph;
    type Error = SuperviseError<B::Error, A>;
    type Birth = Births<<<B::Birth as BirthMode>::Child as BirthNodeAppend<L::Output>>::Output>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        // Validate and build the configured topology before running the
        // application's initialization fold. A rejected factory index must
        // not leave application state initialized with no Actions to commit.
        let configured = self
            .ownership
            .initialize(&self.layer)
            .map_err(map_ownership_error)?;
        let inner = behavior::initialize(&mut self.inner).map_err(SuperviseError::Behavior)?;
        let creates =
            <<B::Birth as BirthMode>::Child as BirthNodeAppend<L::Output>>::append_creations(
                inner.creates,
                configured.creates,
            );
        Ok(Actions::new(
            SendLayer::new(configured.sends, inner.sends),
            creates,
            inner.become_,
        ))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
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
            SupervisionEvent::TimerElapsed(event) => self
                .ownership
                .timer_elapsed(event)
                .map_err(map_ownership_error)
                .and_then(|fold| self.wrap_ownership(fold)),
            SupervisionEvent::Behavior(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)
                    .map_err(SuperviseError::Behavior)?;
                Ok(self.wrap(actions))
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
        type Msg = Never;
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<MailAddr, Never>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = behavior::NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
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
                crate::RestartTiming::Immediate,
            ),
            crate::Proxy::new,
        )
        .unwrap();
        let mut active = definition.initialize().unwrap().behavior;

        let child_fact = ChildStopped::new(9, Ok(Exit::Normal), Instant::now());
        assert!(matches!(
            active.transition(SupervisionEvent::ChildStopped(child_fact)),
            Err(SuperviseError::UnexpectedChildStopped(returned)) if returned == child_fact
        ));
        let worker_fact = WorkerStopped::new(9, 10, Err(Crash::Failed), Instant::now());
        assert!(matches!(
            active.transition(SupervisionEvent::WorkerStopped(worker_fact.clone())),
            Err(SuperviseError::UnexpectedWorkerStopped(returned)) if returned == worker_fact
        ));

        assert_eq!(active.base().forwarded, 0);

        let delegated = active
            .transition(SupervisionEvent::Behavior(SupervisionEvent::ChildStopped(
                ChildStopped::new(9, Ok(Exit::Normal), Instant::now()),
            )))
            .unwrap();
        assert!(delegated.sends.owned.child_observations.is_empty());
        assert!(delegated.sends.owned.creation_observations.is_empty());
        assert!(delegated.sends.owned.replacement_inputs.is_empty());
        assert!(delegated.sends.owned.failure_reports.is_empty());
        assert!(delegated.sends.owned.schedules.is_empty());
        assert!(delegated.sends.owned.shutdowns.is_empty());
        assert!(delegated.sends.inner.is_empty());
        assert!(delegated.creates.is_empty());
        assert!(matches!(delegated.become_, Step::Continue));
        assert_eq!(active.base().forwarded, 1);
        assert_eq!(active.child_count(), 1);
    }

    #[test]
    fn application_birth_remains_an_independent_structural_occurrence() {
        let definition = Supervise::new(
            Parent::default(),
            ChildTopology::indexed(|_| 2, 1, child),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
                crate::RestartTiming::Immediate,
            ),
            crate::Proxy::new,
        )
        .unwrap();
        let mut active = definition.initialize().unwrap().behavior;

        let acted = active
            .transition(SupervisionEvent::Behavior(SupervisionEvent::Behavior(
                User::new(MailAddr(1), ()),
            )))
            .unwrap();

        assert_eq!(acted.creates.len(), 1);
        assert_eq!(acted.creates[0].nonce, 2);
        assert!(matches!(acted.become_, Step::Stop(behavior::Stopped)));
        assert!(acted.sends.owned.failure_reports.is_empty());
        assert_eq!(active.child_count(), 1);
    }

    #[test]
    fn final_pending_creation_rejection_completes_shutdown_even_when_failure_policy_retires() {
        let definition = Supervise::new(
            Parent::default(),
            ChildTopology::indexed(|_| 1, 1, child),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
                crate::RestartTiming::Immediate,
            ),
            crate::Proxy::new,
        )
        .unwrap();
        let mut active = definition.initialize().unwrap().behavior;
        let waiting = active.on(crate::ShutdownRequested).unwrap();
        assert!(waiting.sends.owned.child_observations.is_empty());
        assert!(waiting.sends.owned.creation_observations.is_empty());
        assert!(waiting.sends.owned.replacement_inputs.is_empty());
        assert!(waiting.sends.owned.failure_reports.is_empty());
        assert!(waiting.sends.owned.schedules.is_empty());
        assert!(waiting.sends.owned.shutdowns.is_empty());
        assert!(waiting.sends.inner.is_empty());
        assert!(waiting.creates.is_empty());
        assert!(matches!(waiting.become_, Step::Continue));

        let acted = active
            .on(crate::CreationResolved::rejected(
                1,
                crate::CreationKind::Birth,
                crate::CreationRejection::EnvironmentFailed,
            ))
            .unwrap();
        assert!(matches!(acted.become_, Step::Stop(behavior::Stopped)));
        assert_eq!(acted.sends.owned.failure_reports.as_slice().len(), 1);
    }

    #[test]
    fn fixed_supervisor_drains_only_owned_proxies_and_preserves_inner_creation_facts() {
        let definition = Supervise::new(
            Parent::default(),
            ChildTopology::indexed(|index| u64::try_from(index).unwrap(), 2, child),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                1,
                Duration::from_secs(1),
                crate::RestartTiming::Immediate,
            ),
            crate::Proxy::new,
        )
        .unwrap();
        let mut active = definition.initialize().unwrap().behavior;
        let established = active
            .on(crate::CreationResolved::birth(0, MailAddr(10)))
            .unwrap();
        assert!(established.sends.owned.child_observations.is_empty());
        assert!(established.sends.owned.creation_observations.is_empty());
        assert!(established.sends.owned.replacement_inputs.is_empty());
        assert!(established.sends.owned.failure_reports.is_empty());
        assert!(established.sends.owned.schedules.is_empty());
        assert!(established.sends.owned.shutdowns.is_empty());
        assert!(established.sends.inner.is_empty());
        assert!(established.creates.is_empty());
        assert!(matches!(established.become_, Step::Continue));

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
            Err(SuperviseError::ChildShutdownRejected(event))
                if event == crate::ChildShutdownRejected::new(
                    0,
                    crate::ChildShutdownRejection::AlreadyStopping,
                )
        ));
        let pending_established = active
            .on(crate::CreationResolved::birth(1, MailAddr(11)))
            .unwrap();
        assert_eq!(pending_established.sends.owned.shutdowns.len(), 1);
        assert_eq!(pending_established.sends.owned.shutdowns[0].nonce, 1);
        assert!(
            pending_established
                .sends
                .owned
                .child_observations
                .is_empty()
        );
        assert!(
            pending_established
                .sends
                .owned
                .creation_observations
                .is_empty()
        );
        assert!(
            pending_established
                .sends
                .owned
                .replacement_inputs
                .is_empty()
        );
        assert!(pending_established.sends.owned.failure_reports.is_empty());
        assert!(pending_established.sends.owned.schedules.is_empty());
        assert!(pending_established.sends.inner.is_empty());
        assert!(pending_established.creates.is_empty());
        assert!(matches!(pending_established.become_, Step::Continue));
        let created_during_drain = active
            .transition(SupervisionEvent::Behavior(SupervisionEvent::Behavior(
                User::new(MailAddr(1), ()),
            )))
            .unwrap();
        assert_eq!(created_during_drain.creates.len(), 1);
        assert!(matches!(created_during_drain.become_, Step::Continue));
        let inner_created_during_drain = active
            .on_path::<_, behavior::Inside<behavior::Here>>(crate::CreationResolved::birth(
                2,
                MailAddr(12),
            ))
            .unwrap();
        assert!(inner_created_during_drain.sends.owned.shutdowns.is_empty());
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
            Step::Stop(behavior::Stopped)
        ));
    }
}
