//! Phased child-topology shutdown.

use crate::{ChildShutdownRejected, ChildShutdownRejection, ChildStopped, ShutdownChild};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, ChildHead, ChildRole, ChildRoute,
    ChildTail, Here, InjectEvent, Inside, InterpreterRequests, SendEffects, SendLayer,
};
use behavior::{User, UserEvent};
use std::time::Duration;

/// Validated ordered shutdown phases.
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

/// Complete phase of coordinated shutdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownState<N> {
    Running,
    Stopping { phase: usize, awaiting: Vec<N> },
    Completed,
}

/// Event sum accepted by [`ShutdownCoordinator`].
#[derive(Clone, PartialEq, Eq)]
pub enum ShutdownCoordinatorEvent<E: UserEvent> {
    Behavior(E),
    Requested(crate::ShutdownRequested),
    ChildStopped(ChildStopped<E::Addr>),
    ChildRejected(ChildShutdownRejected<<E::Addr as Address>::Nonce>),
}

impl<E: UserEvent> UserEvent for ShutdownCoordinatorEvent<E> {
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

impl<E: UserEvent> behavior::ComposedEvent for ShutdownCoordinatorEvent<E> {
    type Inner = E;

    fn from_inner(event: E) -> Self {
        Self::Behavior(event)
    }
}

impl<E: UserEvent> InjectEvent<crate::ShutdownRequested, Here> for ShutdownCoordinatorEvent<E> {
    fn inject_at(value: crate::ShutdownRequested) -> Self {
        Self::Requested(value)
    }
}
impl<E: UserEvent> InjectEvent<ChildStopped<E::Addr>, Here> for ShutdownCoordinatorEvent<E> {
    fn inject_at(value: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<E: UserEvent> InjectEvent<ChildShutdownRejected<<E::Addr as Address>::Nonce>, Here>
    for ShutdownCoordinatorEvent<E>
{
    fn inject_at(value: ChildShutdownRejected<<E::Addr as Address>::Nonce>) -> Self {
        Self::ChildRejected(value)
    }
}

impl<E, Input, Path> InjectEvent<Input, Inside<Path>> for ShutdownCoordinatorEvent<E>
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
    plan: ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
    state: ShutdownState<<crate::BehaviorAddr<B> as Address>::Nonce>,
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
            plan,
            state: ShutdownState::Running,
            child: core::marker::PhantomData,
        }
    }
    #[must_use]
    pub fn state(&self) -> &ShutdownState<<crate::BehaviorAddr<B> as Address>::Nonce> {
        &self.state
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        actions.map_sends(|inner| SendLayer::new(InterpreterRequests::empty(), inner))
    }

    fn phase_actions(&self, phase: usize) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        let shutdowns = InterpreterRequests::new(
            self.plan.phases[phase]
                .iter()
                .copied()
                .map(ShutdownChild::<C, Occurrence>::new)
                .collect(),
        );
        Actions::send(SendLayer::new(shutdowns, B::Sends::empty()))
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
    type Event = ShutdownCoordinatorEvent<B::Event>;
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
            ShutdownCoordinatorEvent::Requested(_)
                if matches!(self.state, ShutdownState::Running) =>
            {
                if self.plan.phases.is_empty() {
                    self.state = ShutdownState::Completed;
                    return Ok(Actions::stop());
                }
                self.state = ShutdownState::Stopping {
                    phase: 0,
                    awaiting: self.plan.phases[0].clone(),
                };
                Ok(self.phase_actions(0))
            }
            ShutdownCoordinatorEvent::Requested(_) => Ok(Actions::cont()),
            ShutdownCoordinatorEvent::ChildStopped(stopped) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&stopped.nonce));
                if !matching {
                    return Ok(Actions::cont());
                }
                let ShutdownState::Stopping { phase, awaiting } = &mut self.state else {
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
                if next == self.plan.phases.len() {
                    self.state = ShutdownState::Completed;
                    Ok(Actions::stop())
                } else {
                    self.state = ShutdownState::Stopping {
                        phase: next,
                        awaiting: self.plan.phases[next].clone(),
                    };
                    Ok(self.phase_actions(next))
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
    plan: HeterogeneousShutdownPlan<T>,
    state: ShutdownState<<crate::BehaviorAddr<B> as Address>::Nonce>,
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
            plan,
            state: ShutdownState::Running,
        }
    }

    #[must_use]
    pub fn state(&self) -> &ShutdownState<<crate::BehaviorAddr<B> as Address>::Nonce> {
        &self.state
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> HeterogeneousShutdownActions<B, T> {
        actions.map_sends(|inner| SendLayer::new(HeterogeneousShutdownSends::empty(), inner))
    }

    fn phase_actions(&self, phase: usize) -> HeterogeneousShutdownActions<B, T> {
        let sends = HeterogeneousShutdownSends {
            requests: self.plan.phases[phase].clone(),
        };
        Actions::send(SendLayer::new(sends, B::Sends::empty()))
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
    type Event = ShutdownCoordinatorEvent<B::Event>;
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
            ShutdownCoordinatorEvent::Requested(_)
                if matches!(self.state, ShutdownState::Running) =>
            {
                if self.plan.phases.is_empty() {
                    self.state = ShutdownState::Completed;
                    return Ok(Actions::stop());
                }
                self.state = ShutdownState::Stopping {
                    phase: 0,
                    awaiting: self.plan.phases[0]
                        .iter()
                        .copied()
                        .map(|target| heterogeneous::Selection::nonce(&target))
                        .collect(),
                };
                Ok(self.phase_actions(0))
            }
            ShutdownCoordinatorEvent::Requested(_) => Ok(Actions::cont()),
            ShutdownCoordinatorEvent::ChildStopped(stopped) => {
                let ShutdownState::Stopping { phase, awaiting } = &mut self.state else {
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
                if next == self.plan.phases.len() {
                    self.state = ShutdownState::Completed;
                    Ok(Actions::stop())
                } else {
                    self.state = ShutdownState::Stopping {
                        phase: next,
                        awaiting: self.plan.phases[next]
                            .iter()
                            .copied()
                            .map(|target| heterogeneous::Selection::nonce(&target))
                            .collect(),
                    };
                    Ok(self.phase_actions(next))
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
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 0,
                awaiting: vec![1, 2]
            }
        );

        let one = active.on_path(stopped(2)).unwrap();
        assert_eq!(one.sends, SendLayer::empty());
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 0,
                awaiting: vec![1]
            }
        );

        let second = active.on_path(stopped(1)).unwrap();
        assert_eq!(second.sends.owned.as_slice(), [ShutdownChild::new(3)]);
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 1,
                awaiting: vec![3]
            }
        );

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
        type Event = ShutdownCoordinatorEvent<User<MailAddr, u8>>;

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
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 0,
                awaiting: vec![2]
            }
        );
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
}
