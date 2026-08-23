//! Phased child-topology shutdown.

use crate::{ChildShutdownRejected, ChildShutdownRejection, ChildStopped, ShutdownChild};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, ChildHead, ChildRole, ChildRoute,
    ChildTail, Here, InjectEvent, Inside, InterpreterRequests, SendEffects, SendLayer,
};
use behavior::{User, UserEvent};
use std::time::Duration;

/// Validated ordered shutdown phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownPlan<N> {
    phases: Vec<Vec<N>>,
}

impl<N: Copy + Eq> ShutdownPlan<N> {
    /// Validate non-empty phases and globally unique child nonces.
    ///
    /// # Errors
    /// Returns the offending phase or duplicate nonce.
    pub fn new(phases: impl IntoIterator<Item = Vec<N>>) -> Result<Self, ShutdownPlanError<N>> {
        let phases: Vec<_> = phases.into_iter().collect();
        let mut seen = Vec::new();
        for (phase, children) in phases.iter().enumerate() {
            if children.is_empty() {
                return Err(ShutdownPlanError::EmptyPhase { phase });
            }
            for &nonce in children {
                if seen.contains(&nonce) {
                    return Err(ShutdownPlanError::DuplicateChild(nonce));
                }
                seen.push(nonce);
            }
        }
        Ok(Self { phases })
    }

    #[must_use]
    pub fn phases(&self) -> &[Vec<N>] {
        &self.phases
    }
}

/// Invalid static shutdown topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShutdownPlanError<N> {
    #[error("shutdown phase {phase} has no children")]
    EmptyPhase { phase: usize },
    #[error("a child occurs in more than one shutdown position")]
    DuplicateChild(N),
}

/// Dependency topology compiled into ordered shutdown phases.
///
/// Each `(dependent, dependency)` edge means the dependent must stop before
/// the dependency. Independent nodes share a phase in declaration order.
pub struct ShutdownTree<N> {
    plan: ShutdownPlan<N>,
}

impl<N: Copy + Eq> ShutdownTree<N> {
    /// Validate a closed acyclic topology and derive shutdown layers.
    ///
    /// # Errors
    /// Returns duplicate/unknown child evidence or `Cycle`.
    pub fn new(
        nodes: impl IntoIterator<Item = N>,
        edges: impl IntoIterator<Item = (N, N)>,
    ) -> Result<Self, ShutdownTreeError<N>> {
        let nodes: Vec<_> = nodes.into_iter().collect();
        let mut unique = Vec::new();
        for &node in &nodes {
            if unique.contains(&node) {
                return Err(ShutdownTreeError::DuplicateChild(node));
            }
            unique.push(node);
        }
        let edges: Vec<_> = edges.into_iter().collect();
        for &(dependent, dependency) in &edges {
            if !nodes.contains(&dependent) {
                return Err(ShutdownTreeError::UnknownChild(dependent));
            }
            if !nodes.contains(&dependency) {
                return Err(ShutdownTreeError::UnknownChild(dependency));
            }
        }
        let mut remaining = nodes;
        let mut phases = Vec::new();
        while !remaining.is_empty() {
            let phase: Vec<_> = remaining
                .iter()
                .copied()
                .filter(|candidate| {
                    !edges.iter().any(|(dependent, dependency)| {
                        dependency == candidate && remaining.contains(dependent)
                    })
                })
                .collect();
            if phase.is_empty() {
                return Err(ShutdownTreeError::Cycle);
            }
            remaining.retain(|node| !phase.contains(node));
            phases.push(phase);
        }
        Ok(Self {
            plan: ShutdownPlan { phases },
        })
    }

    #[must_use]
    pub fn into_plan(self) -> ShutdownPlan<N> {
        self.plan
    }
}

/// Invalid dependency-ordered shutdown topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ShutdownTreeError<N> {
    #[error("a child is declared more than once")]
    DuplicateChild(N),
    #[error("a dependency edge names an undeclared child")]
    UnknownChild(N),
    #[error("the shutdown dependency topology contains a cycle")]
    Cycle,
}

/// One member of a closed heterogeneous child-protocol sum.
///
/// A root gives the recursive sum a topology-specific alias. `Child` selects
/// the protocol at this position; `Other` selects one of the remaining
/// protocols. The value carries only the creator-local nonce, never an erased
/// actor, address, request, or runtime protocol key.
///
/// A child whose event algebra has no direct shutdown owner cannot enter a
/// validated heterogeneous plan:
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};
/// use behavior_actors::{HeterogeneousShutdownPlan, NoShutdownTargets, ShutdownChoice};
/// struct Plain;
/// impl behavior::Protocol for Plain { type Addr = MailAddr; type Msg = (); }
/// impl Behavior for Plain {
///     type Protocol = Self;
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
/// type Targets = ShutdownChoice<Plain, NoShutdownTargets<MailAddr>>;
/// let _ = HeterogeneousShutdownPlan::new([vec![Targets::child(1)]]);
/// ```
pub enum ShutdownChoice<C: Behavior, Tail> {
    Child {
        nonce: <crate::BehaviorAddr<C> as Address>::Nonce,
        child: core::marker::PhantomData<fn() -> C>,
    },
    Other(Tail),
}

/// Uninhabited end of a heterogeneous shutdown choice.
pub struct NoShutdownTargets<A: Address> {
    never: behavior::Never,
    address: core::marker::PhantomData<fn() -> A>,
}

impl<A: Address> Copy for NoShutdownTargets<A> {}
impl<A: Address> Clone for NoShutdownTargets<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: Behavior, Tail> ShutdownChoice<C, Tail> {
    #[must_use]
    pub const fn child(nonce: <crate::BehaviorAddr<C> as Address>::Nonce) -> Self {
        Self::Child {
            nonce,
            child: core::marker::PhantomData,
        }
    }

    #[must_use]
    pub const fn other(target: Tail) -> Self {
        Self::Other(target)
    }
}

/// Structural construction of an existing [`ShutdownChoice`] at `Position`.
///
/// Implementations preserve the route nonce exactly. `ChildHead` selects the
/// current branch; `ChildTail<P>` delegates to the existing tail. This trait
/// introduces no alternate target product, runtime lookup, or shutdown effect.
pub trait ShutdownTargetAt<Child: Behavior, Position>: named_shutdown::Target + Sized {
    /// Lower one typed child route into its statically selected branch.
    fn shutdown_target_at<Role>(route: ChildRoute<Child, Role>) -> Self;
}

mod named_shutdown {
    pub trait Target {}
}

impl<Child: Behavior, Tail> named_shutdown::Target for ShutdownChoice<Child, Tail> {}

impl<Child: Behavior, Tail> ShutdownTargetAt<Child, ChildHead> for ShutdownChoice<Child, Tail> {
    fn shutdown_target_at<Role>(route: ChildRoute<Child, Role>) -> Self {
        Self::child(route.nonce())
    }
}

impl<Head, Tail, Child, Position> ShutdownTargetAt<Child, ChildTail<Position>>
    for ShutdownChoice<Head, Tail>
where
    Head: Behavior,
    Child: Behavior,
    Tail: ShutdownTargetAt<Child, Position>,
{
    fn shutdown_target_at<Role>(route: ChildRoute<Child, Role>) -> Self {
        Self::other(Tail::shutdown_target_at(route))
    }
}

/// Lower one Behavior-owned named child route into an existing heterogeneous
/// shutdown target sum.
///
/// `Parent` fixes the [`ChildRole`] implementation, allowing the compiler to
/// select the exact structural position even when several roles share one
/// child behavior type. The role value and route must carry the same nominal
/// `Role`; exchanging either is rejected at compile time.
///
/// ```compile_fail
/// use behavior_actors::{
///     BehaviorActed, ChildRoute, MailAddr, Never, NoShutdownTargets,
///     ShutdownChoice, shutdown_target,
/// };
/// struct Worker;
/// #[behavior_actors::behavior(addr = MailAddr, message = Never)]
/// impl Worker {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Parent;
/// #[behavior_actors::behavior(addr = MailAddr, message = Never, births = {
///     primary: Worker,
///     fallback: Worker,
/// })]
/// impl Parent {
///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// type Targets = ShutdownChoice<Worker, ShutdownChoice<Worker, NoShutdownTargets<MailAddr>>>;
/// let routes = ParentChildrenRoutes::new(1, 2);
/// let _: Targets = shutdown_target::<Parent, _, Targets>(ParentChild::Primary, routes.fallback);
/// ```
#[must_use]
pub fn shutdown_target<Parent, Role, Targets>(
    _: Role,
    route: ChildRoute<Role::Child, Role>,
) -> Targets
where
    Parent: Behavior,
    Role: ChildRole<Parent>,
    Targets: ShutdownTargetAt<Role::Child, Role::Position>,
{
    Targets::shutdown_target_at(route)
}

impl<C, Tail> Copy for ShutdownChoice<C, Tail>
where
    C: Behavior,
    <crate::BehaviorAddr<C> as Address>::Nonce: Copy,
    Tail: Copy,
{
}

impl<C, Tail> Clone for ShutdownChoice<C, Tail>
where
    C: Behavior,
    <crate::BehaviorAddr<C> as Address>::Nonce: Copy,
    Tail: Copy,
{
    fn clone(&self) -> Self {
        *self
    }
}

mod heterogeneous {
    use super::*;

    pub trait Selection: Sized + Send {
        type Addr: Address;
        fn nonce(&self) -> <Self::Addr as Address>::Nonce;
    }

    pub trait Interpret<I, E, Path, Occurrence>: Send
    where
        I: behavior::SendInterpreter,
    {
        fn interpret(
            self,
            interpreter: &mut I,
        ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send;
    }

    impl<A: Address> Selection for NoShutdownTargets<A> {
        type Addr = A;
        fn nonce(&self) -> A::Nonce {
            match self.never {}
        }
    }

    impl<I, E, Path, A, Occurrence> Interpret<I, E, Path, Occurrence> for NoShutdownTargets<A>
    where
        I: behavior::SendInterpreter,
        A: Address,
    {
        async fn interpret(self, _: &mut I) -> Result<(), I::Error> {
            match self.never {}
        }
    }

    impl<C, Tail> Selection for ShutdownChoice<C, Tail>
    where
        C: Behavior,
        C::Event: InjectEvent<crate::ShutdownRequested, Here>,
        <crate::BehaviorAddr<C> as Address>::Nonce: Send,
        Tail: Selection<Addr = crate::BehaviorAddr<C>>,
    {
        type Addr = crate::BehaviorAddr<C>;
        fn nonce(&self) -> <Self::Addr as Address>::Nonce {
            match self {
                Self::Child { nonce, .. } => *nonce,
                Self::Other(target) => target.nonce(),
            }
        }
    }

    impl<I, E, Path, C, Tail, Occurrence> Interpret<I, E, Path, Occurrence> for ShutdownChoice<C, Tail>
    where
        I: behavior::SendInterpreter
            + behavior::InterpretRequest<ShutdownChild<C, Occurrence>, E, Path>,
        C: Behavior,
        <crate::BehaviorAddr<C> as Address>::Nonce: Send,
        Tail: Interpret<I, E, Path, ChildTail<Occurrence>>,
    {
        async fn interpret(self, interpreter: &mut I) -> Result<(), I::Error> {
            match self {
                Self::Child { nonce, .. } => <I as behavior::InterpretRequest<
                    ShutdownChild<C, Occurrence>,
                    E,
                    Path,
                >>::interpret_request(
                    interpreter, ShutdownChild::new(nonce)
                )
                .await,
                Self::Other(target) => target.interpret(interpreter).await,
            }
        }
    }
}

/// The concrete coordinated application-terminal stack fixed by
/// [`coordinated_terminal_application`].
pub type CoordinatedTerminalApplication<B, S, ObservedOccurrence> = crate::PropagateTermination<
    crate::OneShot<HeterogeneousShutdownCoordinator<B, S>>,
    crate::ChildTermination<crate::BehaviorAddr<B>, ObservedOccurrence>,
>;

/// Construct coordinated heterogeneous shutdown inside a one-shot trigger,
/// observed by exact child-terminal propagation.
///
/// This is a derived Bombay construction policy, not an actor-model law. It
/// fixes only wrapper order. The validated plan, timer identity and duration,
/// pure timeout reaction, observed child nonce, and terminal policy govern
/// their designated existing templates without inference or reclassification.
#[must_use]
pub fn coordinated_terminal_application<B, S, ObservedOccurrence>(
    behavior: B,
    shutdown_plan: HeterogeneousShutdownPlan<S>,
    timer_id: crate::TimerId,
    shutdown_after: Duration,
    request_shutdown: crate::OneShotReaction<HeterogeneousShutdownCoordinator<B, S>>,
    observed_child: crate::ChildTermination<crate::BehaviorAddr<B>, ObservedOccurrence>,
    terminal_policy: crate::TerminalPropagationPolicy<crate::BehaviorAddr<B>>,
) -> CoordinatedTerminalApplication<B, S, ObservedOccurrence>
where
    B: Behavior,
    S: heterogeneous::Selection<Addr = crate::BehaviorAddr<B>> + Copy,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    crate::PropagateTermination::new(
        crate::OneShot::new(
            HeterogeneousShutdownCoordinator::new(behavior, shutdown_plan),
            timer_id,
            shutdown_after,
            request_shutdown,
        ),
        observed_child,
        terminal_policy,
    )
}

/// Validated shutdown phases over an arbitrary closed child-protocol sum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeterogeneousShutdownPlan<T: heterogeneous::Selection> {
    phases: Vec<Vec<T>>,
}

impl<T> HeterogeneousShutdownPlan<T>
where
    T: heterogeneous::Selection,
    <T::Addr as Address>::Nonce: Copy + Eq,
{
    /// Validate non-empty phases and global uniqueness in the creator's one
    /// child namespace, including collisions across protocol lanes.
    pub fn new(
        phases: impl IntoIterator<Item = Vec<T>>,
    ) -> Result<Self, ShutdownPlanError<<T::Addr as Address>::Nonce>> {
        let phases: Vec<_> = phases.into_iter().collect();
        let mut seen = Vec::new();
        for (phase, children) in phases.iter().enumerate() {
            if children.is_empty() {
                return Err(ShutdownPlanError::EmptyPhase { phase });
            }
            for child in children {
                let nonce = heterogeneous::Selection::nonce(child);
                if seen.contains(&nonce) {
                    return Err(ShutdownPlanError::DuplicateChild(nonce));
                }
                seen.push(nonce);
            }
        }
        Ok(Self { phases })
    }

    #[must_use]
    pub fn phases(&self) -> &[Vec<T>] {
        &self.phases
    }
}

/// Ordered heterogeneous shutdown requests. Static dispatch occurs per item,
/// preserving phase declaration order across protocol alternatives.
pub struct HeterogeneousShutdownSends<T> {
    requests: Vec<T>,
}

impl<T> SendEffects for HeterogeneousShutdownSends<T> {
    fn empty() -> Self {
        Self {
            requests: Vec::new(),
        }
    }

    fn append(&mut self, other: Self) {
        self.requests.extend(other.requests);
    }
}

impl<E, T: Send> behavior::SendsFor<E> for HeterogeneousShutdownSends<T> {}

impl<I, E, Path, T> behavior::InterpretSends<I, E, Path> for HeterogeneousShutdownSends<T>
where
    I: behavior::SendInterpreter,
    T: heterogeneous::Interpret<I, E, Path, ChildHead>,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            for request in self.requests {
                request.interpret(interpreter).await?;
            }
            Ok(())
        }
    }
}

/// Complete phase of coordinated shutdown, including plan installation.
///
/// The installed plan remains owned by the exact phase in which it is valid.
/// A shutdown request received before installation has its own state and is
/// discharged immediately when a plan later arrives. No correlated readiness
/// flag or optional plan can describe a contradictory combination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownState<P, N> {
    /// Child establishment has not yet produced the plan.
    AwaitingPlan,
    /// Shutdown was requested while child establishment was incomplete.
    AwaitingPlanAfterShutdown,
    /// The validated plan is installed and shutdown has not been requested.
    Ready { plan: P },
    /// One plan phase is active and owns its outstanding child nonces.
    Stopping {
        plan: P,
        phase: usize,
        awaiting: Vec<N>,
    },
    /// Every configured phase completed, or the installed plan was empty.
    Completed,
}

/// Install one validated plan into a coordinator that was started without it.
///
/// The plan type is part of the coordinator's event sum. Homogeneous and
/// heterogeneous plans therefore cannot be confused at installation:
///
/// ```compile_fail
/// use behavior::{Actions, Activate, Behavior, ChildHead, MailAddr, Never, NoBirths, User};
/// use behavior_actors::{
///     HeterogeneousShutdownPlan, InstallShutdownPlan, NoShutdownTargets, ShutdownChoice,
///     ShutdownCoordinator, StopOnShutdown,
/// };
/// struct Probe;
/// impl behavior::Protocol for Probe { type Addr = MailAddr; type Msg = (); }
/// impl Behavior for Probe {
///     type Protocol = Self;
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
/// type Targets = ShutdownChoice<StopOnShutdown<Probe>, NoShutdownTargets<MailAddr>>;
/// let mut coordinator = ShutdownCoordinator::<Probe, StopOnShutdown<Probe>, ChildHead>::awaiting_plan(Probe)
///     .initialize().unwrap().behavior;
/// let heterogeneous = HeterogeneousShutdownPlan::<Targets>::new([]).unwrap();
/// coordinator.on_path(InstallShutdownPlan::new(heterogeneous)).unwrap();
/// ```
pub struct InstallShutdownPlan<P> {
    plan: P,
}

impl<P> InstallShutdownPlan<P> {
    /// Construct the explicit plan-installation input.
    #[must_use]
    pub const fn new(plan: P) -> Self {
        Self { plan }
    }

    /// Consume the input into the validated plan it owns.
    #[must_use]
    pub fn into_plan(self) -> P {
        self.plan
    }
}

/// Exact parent ingress for installing a creation-dependent shutdown plan.
///
/// A topology owner receives this capability from the composition that wraps
/// it in a shutdown coordinator. After committed child-creation facts have
/// supplied the required routes, the owner emits [`ReportShutdownPlan`] in its
/// ordinary [`Actions`] sends product. `Path` selects the coordinator's plan
/// lane in the final root event algebra; neither the behavior nor the
/// interpreter searches by payload type.
///
/// This is a derived Bombay parent-report construction. It represents an
/// explicit communication to a known ancestor ingress and introduces no
/// ambient mutation, allocation, or Behavior Core effect.
pub struct ShutdownPlanIngress<P, Path> {
    install: behavior::Ingress<InstallShutdownPlan<P>, Path>,
}

impl<P, Path> Copy for ShutdownPlanIngress<P, Path> {}

impl<P, Path> Clone for ShutdownPlanIngress<P, Path> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P, Path> core::fmt::Debug for ShutdownPlanIngress<P, Path> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ShutdownPlanIngress")
    }
}

impl<P, Path> PartialEq for ShutdownPlanIngress<P, Path> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<P, Path> Eq for ShutdownPlanIngress<P, Path> {}

impl<P, Path> ShutdownPlanIngress<P, Path> {
    /// Select the coordinator plan lane at this exact structural path.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            install: behavior::Ingress::new(),
        }
    }

    /// Lift the selected coordinator lane through one new outer event layer.
    #[must_use]
    pub const fn inside(self) -> ShutdownPlanIngress<P, behavior::Inside<Path>> {
        ShutdownPlanIngress {
            install: self.install.inside(),
        }
    }

    /// Consume a validated plan into one explicit parent-report request.
    #[must_use]
    pub fn report(self, plan: P) -> ReportShutdownPlan<P, Path> {
        ReportShutdownPlan {
            ingress: self.install,
            installation: InstallShutdownPlan::new(plan),
        }
    }
}

impl<P, Path> Default for ShutdownPlanIngress<P, Path> {
    fn default() -> Self {
        Self::new()
    }
}

/// Interpreter request reporting one validated plan to its owning coordinator.
///
/// The request owns the plan and the exact ancestor ingress together. It is
/// emitted through `Actions`; interpretation enqueues one ordinary event for
/// the same actor incarnation. It has no return-to-emitter continuation because
/// its destination is the explicitly selected ancestor layer.
///
/// A report cannot be interpreted against an event algebra whose coordinator
/// lane is at another structural path:
///
/// ```compile_fail
/// use behavior::{Here, Inside, MailAddr, User};
/// use behavior_actors::{
///     ReportShutdownPlan, ShutdownCoordinatorEvent, ShutdownPlan, ShutdownPlanIngress,
/// };
/// type Plan = ShutdownPlan<u64>;
/// type Event = ShutdownCoordinatorEvent<User<MailAddr, ()>, Plan>;
/// let report: ReportShutdownPlan<Plan, Inside<Here>> =
///     ShutdownPlanIngress::<Plan, Here>::new()
///         .inside()
///         .report(ShutdownPlan::new([]).unwrap());
/// let _: Event = report.into_event();
/// ```
pub struct ReportShutdownPlan<P, Path> {
    ingress: behavior::Ingress<InstallShutdownPlan<P>, Path>,
    installation: InstallShutdownPlan<P>,
}

impl<P: core::fmt::Debug, Path> core::fmt::Debug for ReportShutdownPlan<P, Path> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReportShutdownPlan")
            .field("ingress", &self.ingress)
            .field("plan", &self.installation.plan)
            .finish()
    }
}

impl<P: PartialEq, Path> PartialEq for ReportShutdownPlan<P, Path> {
    fn eq(&self, other: &Self) -> bool {
        self.ingress == other.ingress && self.installation.plan == other.installation.plan
    }
}

impl<P: Eq, Path> Eq for ReportShutdownPlan<P, Path> {}

impl<P, Path> ReportShutdownPlan<P, Path> {
    /// Borrow the complete validated plan without changing the request.
    #[must_use]
    pub const fn plan(&self) -> &P {
        &self.installation.plan
    }

    /// Build the exact root event selected by the supplied ancestor ingress.
    ///
    /// An interpreter uses this after accepting the request. The static
    /// `InjectEvent` proof rejects a stale or incorrectly reindexed path at
    /// compile time.
    #[must_use]
    pub fn into_event<Event>(self) -> Event
    where
        Event: InjectEvent<InstallShutdownPlan<P>, Path>,
    {
        self.ingress.event(self.installation)
    }
}

impl<P, Path> behavior::InterpreterRequest for ReportShutdownPlan<P, Path> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
}

/// Event sum accepted by [`ShutdownCoordinator`].
pub enum ShutdownCoordinatorEvent<E: UserEvent, P> {
    Behavior(E),
    Plan(InstallShutdownPlan<P>),
    Requested(crate::ShutdownRequested),
    ChildStopped(ChildStopped<E::Addr>),
    ChildRejected(ChildShutdownRejected<<E::Addr as Address>::Nonce>),
}

impl<E: UserEvent, P> UserEvent for ShutdownCoordinatorEvent<E, P> {
    type Addr = E::Addr;
    type Message = E::Message;
    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Behavior(E::user(from, message))
    }
    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Behavior(e) => e.into_user().map_err(Self::Behavior),
            other => Err(other),
        }
    }
}

impl<E: UserEvent, P> behavior::ComposedEvent for ShutdownCoordinatorEvent<E, P> {
    type Inner = E;

    fn from_inner(event: E) -> Self {
        Self::Behavior(event)
    }
}

impl<E: UserEvent, P> InjectEvent<InstallShutdownPlan<P>, Here> for ShutdownCoordinatorEvent<E, P> {
    fn inject_at(value: InstallShutdownPlan<P>) -> Self {
        Self::Plan(value)
    }
}

impl<E: UserEvent, P> InjectEvent<crate::ShutdownRequested, Here>
    for ShutdownCoordinatorEvent<E, P>
{
    fn inject_at(value: crate::ShutdownRequested) -> Self {
        Self::Requested(value)
    }
}
impl<E: UserEvent, P> InjectEvent<ChildStopped<E::Addr>, Here> for ShutdownCoordinatorEvent<E, P> {
    fn inject_at(value: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<E: UserEvent, P> InjectEvent<ChildShutdownRejected<<E::Addr as Address>::Nonce>, Here>
    for ShutdownCoordinatorEvent<E, P>
{
    fn inject_at(value: ChildShutdownRejected<<E::Addr as Address>::Nonce>) -> Self {
        Self::ChildRejected(value)
    }
}

impl<E, P, Input, Path> InjectEvent<Input, Inside<Path>> for ShutdownCoordinatorEvent<E, P>
where
    E: UserEvent + InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Behavior(E::inject_at(input))
    }
}

/// Controlled coordinated-shutdown failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShutdownCoordinatorError<E, N> {
    #[error("wrapped behavior rejected its transition")]
    Behavior(#[source] E),
    #[error("child shutdown was rejected")]
    ChildRejected {
        nonce: N,
        reason: ChildShutdownRejection,
    },
    /// A plan was supplied after one had already been installed or completed.
    #[error("shutdown plan was already installed")]
    PlanAlreadyInstalled,
}

/// Pure phased shutdown wrapper over an explicitly validated homogeneous child
/// topology.
///
/// `B` is the wrapped coordinator behavior and `C` is the one concrete child
/// protocol addressed by every nonce in the plan. Starting a phase emits one
/// typed [`ShutdownChild<C, Occurrence>`] request per member in plan order. A phase advances
/// only after every matching [`ChildStopped`] fact arrives. An acceptance
/// rejection returns [`ShutdownCoordinatorError::ChildRejected`] without
/// changing the phase; stale facts are delegated when the inner protocol
/// accepts them and are otherwise inert. This is Bombay lifecycle policy, not
/// an actor-model allocation or ordering guarantee.
///
/// The fold introduces no panic conditions.
///
/// A child protocol without the shutdown input cannot form an executable
/// coordinator:
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, ChildHead, MailAddr, Never, NoBirths, Protocol, User};
/// use behavior_actors::{ShutdownCoordinator, ShutdownPlan};
///
/// struct Plain;
/// impl Protocol for Plain {
///     type Addr = MailAddr;
///     type Msg = ();
/// }
/// impl Behavior for Plain {
///     type Event = User<MailAddr, ()>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Ok(Actions::cont())
///     }
/// }
///
/// fn require_behavior<B: Behavior>(_: B) {}
/// let plan = ShutdownPlan::new([vec![1]]).unwrap();
/// require_behavior(ShutdownCoordinator::<Plain, Plain, ChildHead>::new(Plain, plan));
/// ```
pub struct ShutdownCoordinator<B: Behavior, C: Behavior, Occurrence>
where
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    inner: B,
    state: ShutdownState<
        ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
        <crate::BehaviorAddr<B> as Address>::Nonce,
    >,
    child: core::marker::PhantomData<fn() -> (C, Occurrence)>,
}

type ShutdownCoordinatorActions<B, C, Occurrence> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<ShutdownChild<C, Occurrence>>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior, C: Behavior, Occurrence> ShutdownCoordinator<B, C, Occurrence>
where
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    #[must_use]
    pub const fn new(
        inner: B,
        plan: ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
    ) -> Self {
        Self {
            inner,
            state: ShutdownState::Ready { plan },
            child: core::marker::PhantomData,
        }
    }

    /// Start the wrapper before committed child creation can supply its plan.
    ///
    /// [`InstallShutdownPlan`] later installs exactly one validated plan. A
    /// shutdown request received first is retained by the state machine and
    /// begins that plan immediately on installation.
    #[must_use]
    pub const fn awaiting_plan(inner: B) -> Self {
        Self {
            inner,
            state: ShutdownState::AwaitingPlan,
            child: core::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn state(
        &self,
    ) -> &ShutdownState<
        ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
        <crate::BehaviorAddr<B> as Address>::Nonce,
    > {
        &self.state
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        actions.map_sends(|inner| SendLayer::new(InterpreterRequests::empty(), inner))
    }

    fn phase_actions(
        plan: &ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
        phase: usize,
    ) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        let shutdowns = InterpreterRequests::new(
            plan.phases[phase]
                .iter()
                .copied()
                .map(ShutdownChild::<C, Occurrence>::new)
                .collect(),
        );
        Actions::send(SendLayer::new(shutdowns, B::Sends::empty()))
    }

    fn start_plan(
        &mut self,
        plan: ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
    ) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        if plan.phases.is_empty() {
            self.state = ShutdownState::Completed;
            return Actions::stop();
        }
        let actions = Self::phase_actions(&plan, 0);
        self.state = ShutdownState::Stopping {
            awaiting: plan.phases[0].clone(),
            plan,
            phase: 0,
        };
        actions
    }
}

impl<B, C, Occurrence> crate::BehaviorBase for ShutdownCoordinator<B, C, Occurrence>
where
    B: Behavior + crate::BehaviorBase,
    C: Behavior,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    type Base = B::Base;
    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, C, Occurrence> crate::StashStatus for ShutdownCoordinator<B, C, Occurrence>
where
    B: Behavior + crate::StashStatus,
    C: Behavior,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, C, Occurrence, A, Ph, S, Br> Behavior for ShutdownCoordinator<B, C, Occurrence>
where
    A: Address,
    A::Nonce: Copy + Eq,
    S: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = S, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    C: Behavior,
    C::Protocol: crate::Protocol<Addr = A>,
    C::Event: InjectEvent<crate::ShutdownRequested, Here>,
{
    type Protocol = B::Protocol;
    type Event = ShutdownCoordinatorEvent<
        B::Event,
        ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
    >;
    type Sends = SendLayer<InterpreterRequests<ShutdownChild<C, Occurrence>>, S>;
    type Ph = Ph;
    type Error = ShutdownCoordinatorError<B::Error, A::Nonce>;
    type Birth = Br;
    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(Self::wrap)
            .map_err(ShutdownCoordinatorError::Behavior)
    }
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ShutdownCoordinatorEvent::Plan(installation) => match self.state {
                ShutdownState::AwaitingPlan => {
                    self.state = ShutdownState::Ready {
                        plan: installation.into_plan(),
                    };
                    Ok(Actions::cont())
                }
                ShutdownState::AwaitingPlanAfterShutdown => {
                    Ok(self.start_plan(installation.into_plan()))
                }
                ShutdownState::Ready { .. }
                | ShutdownState::Stopping { .. }
                | ShutdownState::Completed => Err(ShutdownCoordinatorError::PlanAlreadyInstalled),
            },
            ShutdownCoordinatorEvent::Requested(_) => match &self.state {
                ShutdownState::AwaitingPlan => {
                    self.state = ShutdownState::AwaitingPlanAfterShutdown;
                    Ok(Actions::cont())
                }
                ShutdownState::Ready { plan } => Ok(self.start_plan(plan.clone())),
                ShutdownState::AwaitingPlanAfterShutdown
                | ShutdownState::Stopping { .. }
                | ShutdownState::Completed => Ok(Actions::cont()),
            },
            ShutdownCoordinatorEvent::ChildStopped(stopped) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&stopped.nonce));
                if !matching {
                    return Ok(Actions::cont());
                }
                let ShutdownState::Stopping {
                    plan,
                    phase,
                    awaiting,
                } = &mut self.state
                else {
                    return Ok(Actions::cont());
                };
                let Some(position) = awaiting.iter().position(|n| *n == stopped.nonce) else {
                    return Ok(Actions::cont());
                };
                awaiting.remove(position);
                if !awaiting.is_empty() {
                    return Ok(Actions::cont());
                }
                let next = *phase + 1;
                if next == plan.phases.len() {
                    self.state = ShutdownState::Completed;
                    Ok(Actions::stop())
                } else {
                    let actions = Self::phase_actions(plan, next);
                    let plan = plan.clone();
                    self.state = ShutdownState::Stopping {
                        awaiting: plan.phases[next].clone(),
                        plan,
                        phase: next,
                    };
                    Ok(actions)
                }
            }
            ShutdownCoordinatorEvent::ChildRejected(rejected) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&rejected.nonce));
                if matching {
                    Err(ShutdownCoordinatorError::ChildRejected {
                        nonce: rejected.nonce,
                        reason: rejected.reason,
                    })
                } else {
                    Ok(Actions::cont())
                }
            }
            ShutdownCoordinatorEvent::Behavior(inner) => {
                behavior::delegate_transition(&mut self.inner, inner)
                    .map(Self::wrap)
                    .map_err(ShutdownCoordinatorError::Behavior)
            }
        }
    }
}

/// Pure phased shutdown over an arbitrary closed child-protocol sum.
///
/// Each choice is interpreted in plan order through its exact concrete
/// `ShutdownChild<C, Occurrence>` request. Phase completion consumes the shared
/// creator-local nonce because the child namespace is globally unique. This
/// phased ordering is Bombay policy, not an actor-model guarantee.
pub struct HeterogeneousShutdownCoordinator<B: Behavior, T>
where
    T: heterogeneous::Selection<Addr = crate::BehaviorAddr<B>>,
{
    inner: B,
    state: ShutdownState<HeterogeneousShutdownPlan<T>, <crate::BehaviorAddr<B> as Address>::Nonce>,
}

type HeterogeneousShutdownActions<B, T> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<HeterogeneousShutdownSends<T>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior, T> HeterogeneousShutdownCoordinator<B, T>
where
    T: heterogeneous::Selection<Addr = crate::BehaviorAddr<B>> + Copy,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    #[must_use]
    pub const fn new(inner: B, plan: HeterogeneousShutdownPlan<T>) -> Self {
        Self {
            inner,
            state: ShutdownState::Ready { plan },
        }
    }

    /// Start the wrapper before committed heterogeneous children supply a plan.
    #[must_use]
    pub const fn awaiting_plan(inner: B) -> Self {
        Self {
            inner,
            state: ShutdownState::AwaitingPlan,
        }
    }

    #[must_use]
    pub fn state(
        &self,
    ) -> &ShutdownState<HeterogeneousShutdownPlan<T>, <crate::BehaviorAddr<B> as Address>::Nonce>
    {
        &self.state
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> HeterogeneousShutdownActions<B, T> {
        actions.map_sends(|inner| SendLayer::new(HeterogeneousShutdownSends::empty(), inner))
    }

    fn phase_actions(
        plan: &HeterogeneousShutdownPlan<T>,
        phase: usize,
    ) -> HeterogeneousShutdownActions<B, T> {
        let sends = HeterogeneousShutdownSends {
            requests: plan.phases[phase].clone(),
        };
        Actions::send(SendLayer::new(sends, B::Sends::empty()))
    }

    fn start_plan(
        &mut self,
        plan: HeterogeneousShutdownPlan<T>,
    ) -> HeterogeneousShutdownActions<B, T> {
        if plan.phases.is_empty() {
            self.state = ShutdownState::Completed;
            return Actions::stop();
        }
        let actions = Self::phase_actions(&plan, 0);
        self.state = ShutdownState::Stopping {
            awaiting: plan.phases[0]
                .iter()
                .map(heterogeneous::Selection::nonce)
                .collect(),
            plan,
            phase: 0,
        };
        actions
    }
}

impl<B, T> crate::BehaviorBase for HeterogeneousShutdownCoordinator<B, T>
where
    B: Behavior + crate::BehaviorBase,
    T: heterogeneous::Selection<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    type Base = B::Base;
    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, T> crate::StashStatus for HeterogeneousShutdownCoordinator<B, T>
where
    B: Behavior + crate::StashStatus,
    T: heterogeneous::Selection<Addr = crate::BehaviorAddr<B>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, T, A, Ph, Sends, Br> Behavior for HeterogeneousShutdownCoordinator<B, T>
where
    A: Address,
    A::Nonce: Copy + Eq,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    T: heterogeneous::Selection<Addr = A> + Copy,
{
    type Protocol = B::Protocol;
    type Event = ShutdownCoordinatorEvent<B::Event, HeterogeneousShutdownPlan<T>>;
    type Sends = SendLayer<HeterogeneousShutdownSends<T>, Sends>;
    type Ph = Ph;
    type Error = ShutdownCoordinatorError<B::Error, A::Nonce>;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(Self::wrap)
            .map_err(ShutdownCoordinatorError::Behavior)
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ShutdownCoordinatorEvent::Plan(installation) => match self.state {
                ShutdownState::AwaitingPlan => {
                    self.state = ShutdownState::Ready {
                        plan: installation.into_plan(),
                    };
                    Ok(Actions::cont())
                }
                ShutdownState::AwaitingPlanAfterShutdown => {
                    Ok(self.start_plan(installation.into_plan()))
                }
                ShutdownState::Ready { .. }
                | ShutdownState::Stopping { .. }
                | ShutdownState::Completed => Err(ShutdownCoordinatorError::PlanAlreadyInstalled),
            },
            ShutdownCoordinatorEvent::Requested(_) => match &self.state {
                ShutdownState::AwaitingPlan => {
                    self.state = ShutdownState::AwaitingPlanAfterShutdown;
                    Ok(Actions::cont())
                }
                ShutdownState::Ready { plan } => Ok(self.start_plan(plan.clone())),
                ShutdownState::AwaitingPlanAfterShutdown
                | ShutdownState::Stopping { .. }
                | ShutdownState::Completed => Ok(Actions::cont()),
            },
            ShutdownCoordinatorEvent::ChildStopped(stopped) => {
                let ShutdownState::Stopping {
                    plan,
                    phase,
                    awaiting,
                } = &mut self.state
                else {
                    return Ok(Actions::cont());
                };
                let Some(position) = awaiting.iter().position(|nonce| *nonce == stopped.nonce)
                else {
                    return Ok(Actions::cont());
                };
                awaiting.remove(position);
                if !awaiting.is_empty() {
                    return Ok(Actions::cont());
                }
                let next = *phase + 1;
                if next == plan.phases.len() {
                    self.state = ShutdownState::Completed;
                    Ok(Actions::stop())
                } else {
                    let actions = Self::phase_actions(plan, next);
                    let plan = plan.clone();
                    self.state = ShutdownState::Stopping {
                        awaiting: plan.phases[next]
                            .iter()
                            .map(heterogeneous::Selection::nonce)
                            .collect(),
                        plan,
                        phase: next,
                    };
                    Ok(actions)
                }
            }
            ShutdownCoordinatorEvent::ChildRejected(rejected) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&rejected.nonce));
                if matching {
                    Err(ShutdownCoordinatorError::ChildRejected {
                        nonce: rejected.nonce,
                        reason: rejected.reason,
                    })
                } else {
                    Ok(Actions::cont())
                }
            }
            ShutdownCoordinatorEvent::Behavior(inner) => {
                behavior::delegate_transition(&mut self.inner, inner)
                    .map(Self::wrap)
                    .map_err(ShutdownCoordinatorError::Behavior)
            }
        }
    }
}

/// Homogeneous dependency-ordered shutdown uses the same validated phase machine.
pub type TreeShutdown<B, C, Occurrence> = ShutdownCoordinator<B, C, Occurrence>;

#[cfg(test)]
mod tests {
    use core::future::Future;
    use std::time::Instant;

    use super::*;
    use crate::Activate as _;
    use crate::{Exit, ShutdownRequested};
    use behavior::{MailAddr, Never, NoBirths, Step};

    struct Probe;

    impl crate::BehaviorBase for Probe {
        type Base = Self;
        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Probe {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Probe {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<u8>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![1]))
        }
        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    struct NamedParent;

    #[behavior::behavior(
        addr = MailAddr,
        message = Never,
        births = {
            primary: crate::StopOnShutdown<Probe>,
            pool: crate::Guardian<Probe>,
            fallback: crate::StopOnShutdown<Probe>,
        },
    )]
    impl NamedParent {
        fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
            match message {}
        }
    }

    fn stopped(nonce: u64) -> ChildStopped<MailAddr> {
        ChildStopped::new(nonce, Ok(Exit::Normal), Instant::now())
    }

    type RecipeTargets = ShutdownChoice<crate::StopOnShutdown<Probe>, NoShutdownTargets<MailAddr>>;
    type RecipeSubject = CoordinatedTerminalApplication<Probe, RecipeTargets, ChildHead>;

    fn request_recipe_shutdown(
        coordinator: &mut HeterogeneousShutdownCoordinator<Probe, RecipeTargets>,
    ) -> BehaviorActed<HeterogeneousShutdownCoordinator<Probe, RecipeTargets>> {
        behavior::delegate_transition(
            coordinator,
            ShutdownCoordinatorEvent::Requested(ShutdownRequested),
        )
    }

    fn recipe_plan() -> HeterogeneousShutdownPlan<RecipeTargets> {
        HeterogeneousShutdownPlan::new([
            vec![RecipeTargets::child(1)],
            vec![RecipeTargets::child(2)],
        ])
        .unwrap()
    }

    fn recipe_subject(policy: crate::TerminalPropagationPolicy<MailAddr>) -> RecipeSubject {
        coordinated_terminal_application(
            Probe,
            recipe_plan(),
            crate::TimerId(17),
            Duration::from_secs(5),
            request_recipe_shutdown,
            crate::ChildTermination::<MailAddr, ChildHead>::new(9),
            policy,
        )
    }

    fn manual_subject(policy: crate::TerminalPropagationPolicy<MailAddr>) -> RecipeSubject {
        crate::PropagateTermination::new(
            crate::OneShot::new(
                HeterogeneousShutdownCoordinator::new(Probe, recipe_plan()),
                crate::TimerId(17),
                Duration::from_secs(5),
                request_recipe_shutdown,
            ),
            crate::ChildTermination::<MailAddr, ChildHead>::new(9),
            policy,
        )
    }

    fn assert_coordinated_turn_equal(
        recipe: crate::BehaviorActed<RecipeSubject>,
        manual: crate::BehaviorActed<RecipeSubject>,
    ) {
        match (recipe, manual) {
            (Ok(recipe), Ok(manual)) => {
                assert!(
                    recipe.sends.owned.observations.as_slice()
                        == manual.sends.owned.observations.as_slice()
                );
                assert_eq!(
                    recipe.sends.owned.reports.len(),
                    manual.sends.owned.reports.len()
                );
                for (recipe, manual) in recipe
                    .sends
                    .owned
                    .reports
                    .iter()
                    .zip(&manual.sends.owned.reports)
                {
                    assert!(recipe.outcome == manual.outcome);
                }
                assert!(recipe.sends.inner.owned.as_slice() == manual.sends.inner.owned.as_slice());
                assert_eq!(
                    recipe.sends.inner.inner.owned.requests.len(),
                    manual.sends.inner.inner.owned.requests.len()
                );
                let recipe_requests = recipe
                    .sends
                    .inner
                    .inner
                    .owned
                    .requests
                    .iter()
                    .map(heterogeneous::Selection::nonce)
                    .collect::<Vec<_>>();
                let manual_requests = manual
                    .sends
                    .inner
                    .inner
                    .owned
                    .requests
                    .iter()
                    .map(heterogeneous::Selection::nonce)
                    .collect::<Vec<_>>();
                assert_eq!(recipe_requests, manual_requests);
                assert_eq!(
                    recipe.sends.inner.inner.inner,
                    manual.sends.inner.inner.inner
                );
                assert_eq!(recipe.creates.len(), manual.creates.len());
                assert_eq!(
                    matches!(recipe.become_, Step::Stop(_)),
                    matches!(manual.become_, Step::Stop(_))
                );
            }
            (Err(recipe), Err(manual)) => assert_eq!(recipe, manual),
            (Ok(_), Err(error)) => panic!("manual stack alone rejected the turn: {error:?}"),
            (Err(error), Ok(_)) => panic!("recipe stack alone rejected the turn: {error:?}"),
        }
    }

    #[test]
    fn coordinated_terminal_recipe_has_exact_type_and_manual_initialization_trace() {
        type Expected = CoordinatedTerminalApplication<Probe, RecipeTargets, ChildHead>;
        fn exact(_: Expected) {}

        exact(coordinated_terminal_application(
            Probe,
            recipe_plan(),
            crate::TimerId(17),
            Duration::from_secs(5),
            request_recipe_shutdown,
            crate::ChildTermination::<MailAddr, ChildHead>::new(9),
            crate::propagate_abnormal,
        ));

        let recipe = recipe_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let manual = manual_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        assert_coordinated_turn_equal(Ok(recipe.actions), Ok(manual.actions));
    }

    #[test]
    fn coordinated_terminal_recipe_and_manual_stack_have_identical_complete_traces() {
        let recipe = recipe_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let manual = manual_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let mut recipe = recipe.behavior;
        let mut manual = manual.behavior;

        let wrong_at = Instant::now();
        assert_coordinated_turn_equal(
            recipe.on(ChildStopped::new(8, Ok(Exit::Normal), wrong_at)),
            manual.on(ChildStopped::new(8, Ok(Exit::Normal), wrong_at)),
        );
        assert_coordinated_turn_equal(
            recipe.on_path(ShutdownRequested),
            manual.on_path(ShutdownRequested),
        );
        let phase_at = Instant::now();
        assert_coordinated_turn_equal(
            recipe.on_path::<_, behavior::Inside<behavior::Inside<behavior::Here>>>(
                ChildStopped::new(1, Ok(Exit::Normal), phase_at),
            ),
            manual.on_path::<_, behavior::Inside<behavior::Inside<behavior::Here>>>(
                ChildStopped::new(1, Ok(Exit::Normal), phase_at),
            ),
        );
        let final_phase_at = Instant::now();
        assert_coordinated_turn_equal(
            recipe.on_path::<_, behavior::Inside<behavior::Inside<behavior::Here>>>(
                ChildStopped::new(2, Ok(Exit::Normal), final_phase_at),
            ),
            manual.on_path::<_, behavior::Inside<behavior::Inside<behavior::Here>>>(
                ChildStopped::new(2, Ok(Exit::Normal), final_phase_at),
            ),
        );

        let recipe = recipe_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let manual = manual_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let mut recipe = recipe.behavior;
        let mut manual = manual.behavior;
        let outcome = Err(crate::Crash::Panicked);
        let abnormal_at = Instant::now();
        assert_coordinated_turn_equal(
            recipe.on(ChildStopped::new(9, outcome, abnormal_at)),
            manual.on(ChildStopped::new(9, outcome, abnormal_at)),
        );

        let recipe = recipe_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let manual = manual_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let mut recipe = recipe.behavior;
        let mut manual = manual.behavior;
        let normal_at = Instant::now();
        assert_coordinated_turn_equal(
            recipe.on(ChildStopped::new(9, Ok(Exit::Normal), normal_at)),
            manual.on(ChildStopped::new(9, Ok(Exit::Normal), normal_at)),
        );
    }

    #[test]
    fn omitting_terminal_propagation_has_a_distinct_type_and_initialization_trace() {
        type Incomplete = crate::OneShot<HeterogeneousShutdownCoordinator<Probe, RecipeTargets>>;
        assert_ne!(
            core::any::type_name::<RecipeSubject>(),
            core::any::type_name::<Incomplete>()
        );

        let recipe = recipe_subject(crate::propagate_abnormal)
            .initialize()
            .unwrap();
        let incomplete = crate::OneShot::new(
            HeterogeneousShutdownCoordinator::new(Probe, recipe_plan()),
            crate::TimerId(17),
            Duration::from_secs(5),
            request_recipe_shutdown,
        )
        .initialize()
        .unwrap();

        assert_eq!(recipe.actions.sends.owned.observations.len(), 1);
        assert_eq!(
            recipe.actions.sends.inner.owned.as_slice(),
            incomplete.actions.sends.owned.as_slice()
        );
        assert_eq!(
            recipe.actions.sends.inner.inner.inner,
            incomplete.actions.sends.inner.inner
        );
    }

    #[test]
    fn coordinated_terminal_recipe_preserves_normal_discharge_policy() {
        let mut normal = coordinated_terminal_application(
            Probe,
            recipe_plan(),
            crate::TimerId(17),
            Duration::from_secs(5),
            request_recipe_shutdown,
            crate::ChildTermination::<MailAddr, ChildHead>::new(9),
            crate::propagate_abnormal,
        )
        .initialize()
        .unwrap()
        .behavior;
        let discharged = normal.on(stopped(9)).unwrap();
        assert!(discharged.sends.owned.reports.is_empty());
        assert!(matches!(discharged.become_, Step::Continue));
    }

    #[test]
    fn plan_rejects_empty_phases_and_duplicate_children() {
        assert!(matches!(
            ShutdownPlan::<u64>::new([vec![]]),
            Err(ShutdownPlanError::EmptyPhase { phase: 0 })
        ));
        assert!(matches!(
            ShutdownPlan::new([vec![1, 2], vec![2]]),
            Err(ShutdownPlanError::DuplicateChild(2))
        ));
    }

    #[test]
    fn tree_derives_stable_dependency_layers_and_rejects_invalid_graphs() {
        let tree = ShutdownTree::new([1, 2, 3, 4], [(1, 3), (2, 3), (3, 4)]).unwrap();
        assert_eq!(tree.into_plan().phases(), &[vec![1, 2], vec![3], vec![4]]);
        assert!(matches!(
            ShutdownTree::new([1, 1], []),
            Err(ShutdownTreeError::DuplicateChild(1))
        ));
        assert!(matches!(
            ShutdownTree::new([1], [(1, 2)]),
            Err(ShutdownTreeError::UnknownChild(2))
        ));
        assert!(matches!(
            ShutdownTree::new([1, 2], [(1, 2), (2, 1)]),
            Err(ShutdownTreeError::Cycle)
        ));
    }

    #[test]
    fn phases_advance_only_after_every_current_child_stops() {
        let plan = ShutdownPlan::new([vec![1, 2], vec![3]]).unwrap();
        let initialized =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap();
        assert_eq!(initialized.actions.sends.inner, [1]);
        assert!(initialized.actions.sends.owned.is_empty());
        let mut active = initialized.behavior;

        let first = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(
            first.sends.owned.as_slice(),
            [ShutdownChild::new(1), ShutdownChild::new(2)]
        );
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[1, 2]
        ));

        let one = active.on_path(stopped(2)).unwrap();
        assert_eq!(one.sends, SendLayer::empty());
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[1]
        ));

        let second = active.on_path(stopped(1)).unwrap();
        assert_eq!(second.sends.owned.as_slice(), [ShutdownChild::new(3)]);
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 1,
                awaiting,
                ..
            } if awaiting == &[3]
        ));

        let done = active.on_path(stopped(3)).unwrap();
        assert!(matches!(done.become_, Step::Stop(_)));
        assert_eq!(active.state(), &ShutdownState::Completed);
    }

    #[test]
    fn heterogeneous_phases_preserve_cross_protocol_order_and_await_the_union() {
        type SupervisorChild = crate::StopOnShutdown<Probe>;
        type PoolChild = crate::Guardian<Probe>;
        type RootTargets = ShutdownChoice<
            SupervisorChild,
            ShutdownChoice<PoolChild, ShutdownChoice<SupervisorChild, NoShutdownTargets<MailAddr>>>,
        >;
        let routes = NamedParentChildrenRoutes::new(1, 2, 3);
        let primary = shutdown_target::<NamedParent, _, RootTargets>(
            NamedParentChild::Primary,
            routes.primary,
        );
        let pool =
            shutdown_target::<NamedParent, _, RootTargets>(NamedParentChild::Pool, routes.pool);
        let fallback = shutdown_target::<NamedParent, _, RootTargets>(
            NamedParentChild::Fallback,
            routes.fallback,
        );

        assert!(matches!(
            primary,
            ShutdownChoice::Other(ShutdownChoice::Other(ShutdownChoice::Child {
                nonce: 1,
                ..
            }))
        ));
        assert!(matches!(
            pool,
            ShutdownChoice::Other(ShutdownChoice::Child { nonce: 2, .. })
        ));
        assert!(matches!(fallback, ShutdownChoice::Child { nonce: 3, .. }));

        let plan = HeterogeneousShutdownPlan::new([vec![primary, pool], vec![fallback]]).unwrap();
        let mut active =
            HeterogeneousShutdownCoordinator::<NamedParent, RootTargets>::new(NamedParent, plan)
                .initialize()
                .unwrap()
                .behavior;

        let first = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(
            first
                .sends
                .owned
                .requests
                .iter()
                .map(heterogeneous::Selection::nonce)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        active.on_path(stopped(2)).unwrap();
        let second = active.on_path(stopped(1)).unwrap();
        assert_eq!(
            heterogeneous::Selection::nonce(&second.sends.owned.requests[0]),
            3
        );
        assert!(matches!(
            active.on_path(stopped(3)).unwrap().become_,
            Step::Stop(_)
        ));
    }

    #[test]
    fn heterogeneous_plan_rejects_cross_protocol_nonce_collisions() {
        type SupervisorChild = crate::StopOnShutdown<Probe>;
        type PoolChild = crate::Guardian<Probe>;
        type RootTargets = ShutdownChoice<
            SupervisorChild,
            ShutdownChoice<PoolChild, ShutdownChoice<SupervisorChild, NoShutdownTargets<MailAddr>>>,
        >;
        let routes = NamedParentChildrenRoutes::new(1, 1, 3);
        assert!(matches!(
            HeterogeneousShutdownPlan::new([vec![
                shutdown_target::<NamedParent, _, RootTargets>(
                    NamedParentChild::Primary,
                    routes.primary,
                ),
                shutdown_target::<NamedParent, _, RootTargets>(NamedParentChild::Pool, routes.pool,),
            ]]),
            Err(ShutdownPlanError::DuplicateChild(1))
        ));
    }

    #[tokio::test]
    async fn heterogeneous_requests_interpret_once_each_in_cross_protocol_plan_order() {
        type First = crate::StopOnShutdown<Probe>;
        type Second = crate::Guardian<Probe>;
        type Targets = ShutdownChoice<
            First,
            ShutdownChoice<Second, ShutdownChoice<First, NoShutdownTargets<MailAddr>>>,
        >;
        type Event =
            ShutdownCoordinatorEvent<User<MailAddr, u8>, HeterogeneousShutdownPlan<Targets>>;

        struct Recording(Vec<u64>);
        impl behavior::SendInterpreter for Recording {
            type Error = Never;
        }
        impl behavior::InterpretRequest<ShutdownChild<First, ChildHead>, Event, Here> for Recording {
            fn interpret_request(
                &mut self,
                request: ShutdownChild<First, ChildHead>,
            ) -> impl Future<Output = Result<(), Never>> + Send {
                async move {
                    self.0.push(request.nonce);
                    Ok(())
                }
            }
        }
        impl behavior::InterpretRequest<ShutdownChild<Second, ChildTail<ChildHead>>, Event, Here>
            for Recording
        {
            fn interpret_request(
                &mut self,
                request: ShutdownChild<Second, ChildTail<ChildHead>>,
            ) -> impl Future<Output = Result<(), Never>> + Send {
                async move {
                    self.0.push(request.nonce);
                    Ok(())
                }
            }
        }
        impl
            behavior::InterpretRequest<
                ShutdownChild<First, ChildTail<ChildTail<ChildHead>>>,
                Event,
                Here,
            > for Recording
        {
            fn interpret_request(
                &mut self,
                request: ShutdownChild<First, ChildTail<ChildTail<ChildHead>>>,
            ) -> impl Future<Output = Result<(), Never>> + Send {
                async move {
                    self.0.push(request.nonce);
                    Ok(())
                }
            }
        }

        let routes = NamedParentChildrenRoutes::new(1, 2, 3);
        let sends = HeterogeneousShutdownSends::<Targets> {
            requests: vec![
                shutdown_target::<NamedParent, _, Targets>(NamedParentChild::Pool, routes.pool),
                shutdown_target::<NamedParent, _, Targets>(
                    NamedParentChild::Fallback,
                    routes.fallback,
                ),
                shutdown_target::<NamedParent, _, Targets>(
                    NamedParentChild::Primary,
                    routes.primary,
                ),
            ],
        };
        let mut interpreter = Recording(Vec::new());
        <_ as behavior::InterpretSends<_, Event, Here>>::interpret(sends, &mut interpreter)
            .await
            .unwrap();
        assert_eq!(interpreter.0, [2, 3, 1]);
    }

    #[test]
    fn guardian_routes_shutdown_to_the_coordinator_before_applying_root_stop() {
        let plan = ShutdownPlan::new([vec![7]]).unwrap();
        let initialized = crate::Guardian::coordinated(ShutdownCoordinator::<
            Probe,
            crate::StopOnShutdown<Probe>,
            ChildHead,
        >::new(Probe, plan))
        .initialize()
        .unwrap();
        let mut active = initialized.behavior;

        let actions = active.on(ShutdownRequested).unwrap();

        assert_eq!(
            actions.sends.inner.owned.as_slice(),
            [ShutdownChild::new(7)]
        );
        assert!(matches!(actions.become_, Step::Continue));
    }

    #[test]
    fn duplicates_stale_children_and_repeated_shutdown_are_inert() {
        let plan = ShutdownPlan::new([vec![1, 2]]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        active.on_path(stopped(9)).unwrap();
        active.on_path(ShutdownRequested).unwrap();
        assert_eq!(
            active.on_path(ShutdownRequested).unwrap().sends,
            SendLayer::empty()
        );
        active.on_path(stopped(1)).unwrap();
        assert_eq!(
            active.on_path(stopped(1)).unwrap().sends,
            SendLayer::empty()
        );
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[2]
        ));
    }

    #[test]
    fn matching_rejection_is_typed_and_does_not_mutate_phase() {
        let plan = ShutdownPlan::new([vec![1]]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        active.on_path(ShutdownRequested).unwrap();
        let before = active.state().clone();
        assert_eq!(
            active.on_path(ChildShutdownRejected::new(
                1,
                ChildShutdownRejection::NotEstablished
            )),
            Err(ShutdownCoordinatorError::ChildRejected {
                nonce: 1,
                reason: ChildShutdownRejection::NotEstablished
            })
        );
        assert_eq!(active.state(), &before);
    }

    #[test]
    fn empty_plan_stops_immediately_and_user_actions_preserve_named_lanes() {
        let plan = ShutdownPlan::<u64>::new([]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        let user = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(user.sends.inner, [7]);
        assert!(user.sends.owned.is_empty());
        assert!(matches!(
            active.on_path(ShutdownRequested).unwrap().become_,
            Step::Stop(_)
        ));
    }

    #[test]
    fn homogeneous_plan_installation_is_an_explicit_one_way_lifecycle() {
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        assert!(matches!(active.state(), ShutdownState::AwaitingPlan));

        let plan = ShutdownPlan::new([vec![1, 2], vec![3]]).unwrap();
        let installed = active
            .on_path(InstallShutdownPlan::new(plan.clone()))
            .unwrap();
        assert_eq!(installed.sends, SendLayer::empty());
        assert!(matches!(
            active.state(),
            ShutdownState::Ready { plan: installed } if installed == &plan
        ));
        assert_eq!(
            active.on_path(InstallShutdownPlan::new(plan)).unwrap_err(),
            ShutdownCoordinatorError::PlanAlreadyInstalled
        );
    }

    #[test]
    fn shutdown_before_homogeneous_plan_is_retained_and_empty_plan_stops() {
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        assert_eq!(
            active.on_path(ShutdownRequested).unwrap().sends,
            SendLayer::empty()
        );
        assert!(matches!(
            active.state(),
            ShutdownState::AwaitingPlanAfterShutdown
        ));
        let started = active
            .on_path(InstallShutdownPlan::new(
                ShutdownPlan::new([vec![4, 5]]).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            started.sends.owned.as_slice(),
            [ShutdownChild::new(4), ShutdownChild::new(5)]
        );

        let mut empty =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        empty.on_path(ShutdownRequested).unwrap();
        let stopped = empty
            .on_path(InstallShutdownPlan::new(ShutdownPlan::new([]).unwrap()))
            .unwrap();
        assert!(matches!(stopped.become_, Step::Stop(_)));
        assert_eq!(empty.state(), &ShutdownState::Completed);
    }

    #[test]
    fn heterogeneous_plan_can_be_installed_after_exact_child_routes_exist() {
        type First = crate::StopOnShutdown<Probe>;
        type Second = crate::Guardian<Probe>;
        type Targets = ShutdownChoice<
            First,
            ShutdownChoice<Second, ShutdownChoice<First, NoShutdownTargets<MailAddr>>>,
        >;

        let mut active =
            HeterogeneousShutdownCoordinator::<NamedParent, Targets>::awaiting_plan(NamedParent)
                .initialize()
                .unwrap()
                .behavior;
        active.on_path(ShutdownRequested).unwrap();
        let routes = NamedParentChildrenRoutes::new(11, 12, 13);
        let plan = HeterogeneousShutdownPlan::new([vec![
            shutdown_target::<NamedParent, _, Targets>(NamedParentChild::Pool, routes.pool),
            shutdown_target::<NamedParent, _, Targets>(NamedParentChild::Primary, routes.primary),
        ]])
        .unwrap();
        let started = active.on_path(InstallShutdownPlan::new(plan)).unwrap();
        assert_eq!(
            started
                .sends
                .owned
                .requests
                .iter()
                .map(heterogeneous::Selection::nonce)
                .collect::<Vec<_>>(),
            [12, 11]
        );
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[12, 11]
        ));
    }
}
