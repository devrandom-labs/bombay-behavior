//! Phased child-topology shutdown.

use crate::protocol::forward::forward_event_lane;
use crate::{
    ChildShutdownRejected, ChildShutdownRejection, ChildStopped, Own, RouteInput, SendInput,
    ShutdownChild,
};
use behavior::{Actions, Address, Behavior, BehaviorActed, BirthMode, SendAlgebra, ServiceSends};
use behavior::{User, UserEvent};

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

impl<E: UserEvent> crate::EventInput<crate::ShutdownRequested> for ShutdownCoordinatorEvent<E> {
    fn inject(value: crate::ShutdownRequested) -> Self {
        Self::Requested(value)
    }
}
impl<E: UserEvent> crate::RouteInput<crate::ShutdownRequested> for ShutdownCoordinatorEvent<E> {
    fn route(value: crate::ShutdownRequested) -> Result<Self, crate::ShutdownRequested> {
        Ok(Self::Requested(value))
    }
}
impl<E: UserEvent> crate::EventInput<ChildStopped<E::Addr>> for ShutdownCoordinatorEvent<E> {
    fn inject(value: ChildStopped<E::Addr>) -> Self {
        Self::ChildStopped(value)
    }
}
impl<E: UserEvent> crate::RouteInput<ChildStopped<E::Addr>> for ShutdownCoordinatorEvent<E> {
    fn route(value: ChildStopped<E::Addr>) -> Result<Self, ChildStopped<E::Addr>> {
        Ok(Self::ChildStopped(value))
    }
}
impl<E: UserEvent> crate::EventInput<ChildShutdownRejected<<E::Addr as Address>::Nonce>>
    for ShutdownCoordinatorEvent<E>
{
    fn inject(value: ChildShutdownRejected<<E::Addr as Address>::Nonce>) -> Self {
        Self::ChildRejected(value)
    }
}
impl<E: UserEvent> crate::RouteInput<ChildShutdownRejected<<E::Addr as Address>::Nonce>>
    for ShutdownCoordinatorEvent<E>
{
    fn route(
        value: ChildShutdownRejected<<E::Addr as Address>::Nonce>,
    ) -> Result<Self, ChildShutdownRejected<<E::Addr as Address>::Nonce>> {
        Ok(Self::ChildRejected(value))
    }
}
forward_event_lane!(ShutdownCoordinatorEvent, crate::TimerElapsed);
forward_event_lane!(ShutdownCoordinatorEvent, crate::PeerStopped<E::Addr>);
forward_event_lane!(ShutdownCoordinatorEvent, crate::WorkerStopped<E::Addr>);
forward_event_lane!(ShutdownCoordinatorEvent, crate::CreationResolved<E::Addr>);
forward_event_lane!(
    ShutdownCoordinatorEvent,
    crate::WorkerCreationResolved<<E::Addr as Address>::Nonce>
);

/// Named effects added by homogeneous coordinated shutdown.
///
/// `C` remains in the shutdown lane so an interpreter can select the concrete
/// hosted child namespace without a registry, downcast, or erased envelope.
pub struct ShutdownCoordinatorSends<C: Behavior, S> {
    /// Sends emitted by the wrapped behavior.
    pub behavior: S,
    /// Local requests to shut down children in the active phase.
    pub shutdowns: ServiceSends<ShutdownChild<C>>,
}

impl<C: Behavior, S: Clone> Clone for ShutdownCoordinatorSends<C, S> {
    fn clone(&self) -> Self {
        Self {
            behavior: self.behavior.clone(),
            shutdowns: self.shutdowns.clone(),
        }
    }
}

impl<C: Behavior, S: PartialEq> PartialEq for ShutdownCoordinatorSends<C, S> {
    fn eq(&self, other: &Self) -> bool {
        self.behavior == other.behavior && self.shutdowns == other.shutdowns
    }
}

impl<C: Behavior, S: Eq> Eq for ShutdownCoordinatorSends<C, S> {}

impl<C: Behavior, S: core::fmt::Debug> core::fmt::Debug for ShutdownCoordinatorSends<C, S>
where
    <crate::BehaviorAddr<C> as Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShutdownCoordinatorSends")
            .field("behavior", &self.behavior)
            .field("shutdowns", &self.shutdowns)
            .finish()
    }
}
impl<C: Behavior, S: SendAlgebra> SendAlgebra for ShutdownCoordinatorSends<C, S> {
    fn empty() -> Self {
        Self {
            behavior: S::empty(),
            shutdowns: ServiceSends::empty(),
        }
    }
    fn append(&mut self, other: Self) {
        self.behavior.append(other.behavior);
        self.shutdowns.append(other.shutdowns);
    }
}
impl<C: Behavior, S> SendInput<ShutdownChild<C>, Own> for ShutdownCoordinatorSends<C, S> {
    fn emit(&mut self, input: ShutdownChild<C>) {
        self.shutdowns.send(input);
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
/// typed [`ShutdownChild<C>`] request per member in plan order. A phase advances
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
/// use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, Protocol, User};
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
/// require_behavior(ShutdownCoordinator::<Plain, Plain>::new(Plain, plan));
/// ```
pub struct ShutdownCoordinator<B: Behavior, C: Behavior>
where
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    inner: B,
    plan: ShutdownPlan<<crate::BehaviorAddr<B> as Address>::Nonce>,
    state: ShutdownState<<crate::BehaviorAddr<B> as Address>::Nonce>,
    child: core::marker::PhantomData<fn() -> C>,
}

type ShutdownCoordinatorActions<B, C> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    ShutdownCoordinatorSends<C, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior, C: Behavior> ShutdownCoordinator<B, C>
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
    ) -> ShutdownCoordinatorActions<B, C> {
        actions.map_sends(|behavior| ShutdownCoordinatorSends {
            behavior,
            shutdowns: ServiceSends::empty(),
        })
    }

    fn phase_actions(&self, phase: usize) -> ShutdownCoordinatorActions<B, C> {
        let shutdowns = ServiceSends::new(
            self.plan.phases[phase]
                .iter()
                .copied()
                .map(ShutdownChild::<C>::new)
                .collect(),
        );
        Actions::send(ShutdownCoordinatorSends {
            behavior: B::Sends::empty(),
            shutdowns,
        })
    }
}

impl<B, C> crate::BehaviorBase for ShutdownCoordinator<B, C>
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

impl<B, C> crate::StashStatus for ShutdownCoordinator<B, C>
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

impl<B, C, A, Ph, S, Br> Behavior for ShutdownCoordinator<B, C>
where
    A: Address,
    A::Nonce: Copy + Eq,
    S: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = S, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    C: Behavior,
    C::Protocol: crate::Protocol<Addr = A>,
    C::Event: crate::EventInput<crate::ShutdownRequested>,
    B::Event:
        crate::RouteInput<ChildStopped<A>> + crate::RouteInput<ChildShutdownRejected<A::Nonce>>,
{
    type Protocol = B::Protocol;
    type Event = ShutdownCoordinatorEvent<B::Event>;
    type Sends = ShutdownCoordinatorSends<C, S>;
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
                    return match B::Event::route(stopped) {
                        Ok(inner) => behavior::delegate_transition(&mut self.inner, inner)
                            .map(Self::wrap)
                            .map_err(ShutdownCoordinatorError::Behavior),
                        Err(_) => Ok(Actions::cont()),
                    };
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
                    match B::Event::route(rejected) {
                        Ok(inner) => behavior::delegate_transition(&mut self.inner, inner)
                            .map(Self::wrap)
                            .map_err(ShutdownCoordinatorError::Behavior),
                        Err(_) => Ok(Actions::cont()),
                    }
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
pub type TreeShutdown<B, C> = ShutdownCoordinator<B, C>;

#[cfg(test)]
mod tests {
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

    fn stopped(nonce: u64) -> ChildStopped<MailAddr> {
        ChildStopped::new(nonce, Ok(Exit::Normal), Instant::now())
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
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>>::new(Probe, plan)
                .initialize()
                .unwrap();
        assert_eq!(initialized.actions.sends.behavior, [1]);
        assert!(initialized.actions.sends.shutdowns.is_empty());
        let mut active = initialized.behavior;

        let first = active.on(ShutdownRequested).unwrap();
        assert_eq!(
            first.sends.shutdowns.as_slice(),
            [ShutdownChild::new(1), ShutdownChild::new(2)]
        );
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 0,
                awaiting: vec![1, 2]
            }
        );

        let one = active.on(stopped(2)).unwrap();
        assert_eq!(one.sends, ShutdownCoordinatorSends::empty());
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 0,
                awaiting: vec![1]
            }
        );

        let second = active.on(stopped(1)).unwrap();
        assert_eq!(second.sends.shutdowns.as_slice(), [ShutdownChild::new(3)]);
        assert_eq!(
            active.state(),
            &ShutdownState::Stopping {
                phase: 1,
                awaiting: vec![3]
            }
        );

        let done = active.on(stopped(3)).unwrap();
        assert!(matches!(done.become_, Step::Stop(_)));
        assert_eq!(active.state(), &ShutdownState::Completed);
    }

    #[test]
    fn guardian_routes_shutdown_to_the_coordinator_before_applying_root_stop() {
        let plan = ShutdownPlan::new([vec![7]]).unwrap();
        let initialized = crate::Guardian::new(ShutdownCoordinator::<
            Probe,
            crate::StopOnShutdown<Probe>,
        >::new(Probe, plan))
        .initialize()
        .unwrap();
        let mut active = initialized.behavior;

        let actions = active.on(ShutdownRequested).unwrap();

        assert_eq!(actions.sends.shutdowns.as_slice(), [ShutdownChild::new(7)]);
        assert!(matches!(actions.become_, Step::Continue));
    }

    #[test]
    fn duplicates_stale_children_and_repeated_shutdown_are_inert() {
        let plan = ShutdownPlan::new([vec![1, 2]]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        active.on(stopped(9)).unwrap();
        active.on(ShutdownRequested).unwrap();
        assert_eq!(
            active.on(ShutdownRequested).unwrap().sends,
            ShutdownCoordinatorSends::empty()
        );
        active.on(stopped(1)).unwrap();
        assert_eq!(
            active.on(stopped(1)).unwrap().sends,
            ShutdownCoordinatorSends::empty()
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
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        active.on(ShutdownRequested).unwrap();
        let before = active.state().clone();
        assert_eq!(
            active.on(ChildShutdownRejected::new(
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
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        let user = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(user.sends.behavior, [7]);
        assert!(user.sends.shutdowns.is_empty());
        assert!(matches!(
            active.on(ShutdownRequested).unwrap().become_,
            Step::Stop(_)
        ));
    }
}
