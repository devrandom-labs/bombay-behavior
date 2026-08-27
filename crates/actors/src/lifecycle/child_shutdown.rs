//! Shutdown phases derived from committed direct-child creation results.

use core::marker::PhantomData;

use behavior::{
    Actions, Address, Behavior, BirthMode, BirthNodeMapper, ChildChoice, ChildHead, ChildRoute,
    ChildTail, CreationKind, EventLayer, FoldBirthNode, FoldedBirthNode, InterpreterRequests,
    Never, ResolveChildOccurrence, SendEffects, SendLayer, Step,
};

use super::shutdown_coordinator::heterogeneous;
use super::{
    HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan, NoShutdownTargets,
    ReportShutdownPlan, ShutdownChoice, ShutdownPlanError, ShutdownTargetAt,
};
use crate::{CreationResolved, ObserveCreation, Protocol};

/// Creation state expected by the shutdown-plan composition when one
/// authoritative creation fact arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildCreationExpectation<N> {
    /// The event named no child position in the declared birth product.
    UnknownPosition,
    /// No creation for this declared child has been staged yet.
    NotRequested,
    /// This exact staged creation is awaiting its committed result.
    Awaiting { nonce: N, kind: CreationKind<N> },
    /// A successful result has already established this child route.
    Established { nonce: N },
    /// Every required route was established and the plan was already reported.
    PlanReported,
}

/// Failure while turning committed child creations into shutdown phases.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChildShutdownPlanError<E, A: Address> {
    /// The application rejected its own transition.
    #[error("application behavior rejected the transition")]
    Behavior(#[source] E),
    /// More than one creation was emitted for a role that denotes one child.
    #[error("more than one configured creation was emitted for child position {position}")]
    DuplicateCreation { position: usize },
    /// Initialization did not stage the child declared by this role.
    #[error("configured child position {position} was not created during initialization")]
    MissingCreation { position: usize },
    /// A creation result did not match the request observed at this position.
    #[error("creation result did not match child position {position}")]
    UnexpectedCreationResult {
        position: usize,
        expected: ChildCreationExpectation<A::Nonce>,
        observed: CreationResolved<A>,
    },
    /// The interpreter rejected one required child creation.
    #[error("required child creation was rejected")]
    CreationRejected {
        position: usize,
        observed: CreationResolved<A>,
    },
    /// Committed child routes did not form a valid shutdown plan.
    #[error(transparent)]
    InvalidPlan(ShutdownPlanError<A::Nonce>),
    /// A plan declaration was evaluated before every required creation had
    /// committed. This variant is unreachable through the public fold and is
    /// retained to keep the plan-building operation total.
    #[error("required child position {position} has not committed")]
    ChildNotEstablished { position: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Activate as _, ChildStopped, Exit, InstallShutdownPlan, ShutdownCoordinatorError,
        ShutdownRequested, ShutdownState, StopOnShutdown,
    };
    use behavior::{
        BehaviorActed, BehaviorBase, Births, ChildOccurrence, ChildRole, Children, Create,
        DeclaredChildOccurrence, Here, Inside, MailAddr, NoBirths, NoSends, User,
    };

    struct Store;
    struct Gateway;

    macro_rules! child {
        ($child:ty) => {
            impl Protocol for $child {
                type Addr = MailAddr;
                type Msg = Never;
            }

            impl Behavior for $child {
                type Protocol = Self;
                type Event = User<MailAddr, Never>;
                type Sends = NoSends;
                type Ph = Never;
                type Error = Never;
                type Birth = NoBirths;

                fn transition(
                    &mut self,
                    _: crate::ActiveTurn,
                    event: Self::Event,
                ) -> BehaviorActed<Self> {
                    match event.message {}
                }
            }
        };
    }

    child!(Store);
    child!(Gateway);

    type StoreChild = StopOnShutdown<Store>;
    type GatewayChild = StopOnShutdown<Gateway>;
    type ChildrenNode = ChildChoice<GatewayChild, ChildChoice<StoreChild, Never>>;

    struct StoreRole;
    struct GatewayRole;
    #[derive(Clone, Copy)]
    enum InitialChildren {
        Complete,
        MissingStore,
        DuplicateStore,
    }

    struct Application(InitialChildren);

    impl Application {
        const fn complete() -> Self {
            Self(InitialChildren::Complete)
        }

        const fn missing_store() -> Self {
            Self(InitialChildren::MissingStore)
        }

        const fn duplicate_store() -> Self {
            Self(InitialChildren::DuplicateStore)
        }
    }
    struct EmptyApplication;

    impl Protocol for EmptyApplication {
        type Addr = MailAddr;
        type Msg = Never;
    }

    impl Behavior for EmptyApplication {
        type Protocol = Self;
        type Event = User<MailAddr, Never>;
        type Sends = NoSends;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    impl Protocol for Application {
        type Addr = MailAddr;
        type Msg = Never;
    }

    impl BehaviorBase for Application {
        type Base = Self;

        fn base(&self) -> &Self::Base {
            self
        }
    }

    impl ChildRole<Application> for StoreRole {
        type Child = StoreChild;
        type Position = ChildTail<ChildHead>;
    }

    impl ChildOccurrence<Application> for StoreRole {
        type Resolution = DeclaredChildOccurrence;
    }

    impl ChildRole<Application> for GatewayRole {
        type Child = GatewayChild;
        type Position = ChildHead;
    }

    impl ChildOccurrence<Application> for GatewayRole {
        type Resolution = DeclaredChildOccurrence;
    }

    impl Behavior for Application {
        type Protocol = Self;
        type Event = User<MailAddr, Never>;
        type Sends = NoSends;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<ChildrenNode>;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            let creates = match self.0 {
                InitialChildren::Complete => Children::<MailAddr>::new()
                    .child_at(
                        ChildRoute::<StoreChild, StoreRole>::new(11),
                        StopOnShutdown::new(Store),
                    )
                    .child_at(
                        ChildRoute::<GatewayChild, GatewayRole>::new(12),
                        StopOnShutdown::new(Gateway),
                    )
                    .into_creates()
                    .expect("fixture child nonces are distinct"),
                InitialChildren::MissingStore => vec![Create::birth(
                    12,
                    ChildChoice::Head(StopOnShutdown::new(Gateway)),
                )],
                InitialChildren::DuplicateStore => {
                    let store = || ChildChoice::Tail(ChildChoice::Head(StopOnShutdown::new(Store)));
                    vec![
                        Create::birth(11, store()),
                        Create::birth(13, store()),
                        Create::birth(12, ChildChoice::Head(StopOnShutdown::new(Gateway))),
                    ]
                }
            };
            Ok(Actions::create(creates))
        }

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    fn phase_nonces<T>(plan: &HeterogeneousShutdownPlan<T>) -> Vec<u64>
    where
        T: heterogeneous::Selection<Addr = MailAddr>,
    {
        plan.phases()
            .iter()
            .map(|phase| heterogeneous::Selection::nonce(&phase[0]))
            .collect()
    }

    macro_rules! assert_transition_effect_counts {
        ($actions:expr, shutdowns = $shutdowns:expr, reports = $reports:expr) => {{
            let actions = &$actions;
            assert_eq!(actions.sends.owned.as_slice().len(), $shutdowns);
            assert!(actions.sends.inner.owned.is_empty());
            assert!(actions.sends.inner.inner.owned.is_empty());
            assert_eq!(actions.sends.inner.inner.inner.owned.len(), $reports);
            assert!(matches!(actions.sends.inner.inner.inner.inner, NoSends));
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
        }};
    }

    #[test]
    fn plan_before_and_after_shutdown_preserve_declared_phase_order() {
        let mut plan_first = shutdown_after_children(Application::complete())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize()
            .unwrap()
            .behavior;
        let first_creation = plan_first
            .on_path::<_, Inside<Inside<Here>>>(CreationResolved::birth(11, MailAddr(101)))
            .unwrap();
        assert_transition_effect_counts!(first_creation, shutdowns = 0, reports = 0);
        let reported = plan_first
            .on_path::<_, Inside<Here>>(CreationResolved::birth(12, MailAddr(102)))
            .unwrap();
        let report = reported.sends.inner.inner.inner.owned.as_slice()[0]
            .plan()
            .clone();
        assert_transition_effect_counts!(reported, shutdowns = 0, reports = 1);
        assert_eq!(phase_nonces(&report), [11, 12]);
        let installed = plan_first.on(InstallShutdownPlan::new(report)).unwrap();
        assert_transition_effect_counts!(installed, shutdowns = 0, reports = 0);
        let started = plan_first.on(ShutdownRequested).unwrap();
        assert_transition_effect_counts!(started, shutdowns = 1, reports = 0);
        assert_eq!(
            heterogeneous::Selection::nonce(&started.sends.owned.as_slice()[0]),
            11
        );
        let ShutdownState::Stopping { plan, phase, .. } = plan_first.state() else {
            panic!("plan-first shutdown did not start");
        };
        assert_eq!(*phase, 0);
        assert_eq!(phase_nonces(plan), [11, 12]);
        let ShutdownState::Stopping { awaiting, .. } = plan_first.state() else {
            unreachable!();
        };
        assert_eq!(awaiting, &[11]);
        let advanced = plan_first
            .on(ChildStopped::new(
                11,
                Ok(Exit::Normal),
                std::time::Instant::now(),
            ))
            .unwrap();
        assert_transition_effect_counts!(advanced, shutdowns = 1, reports = 0);
        assert_eq!(
            heterogeneous::Selection::nonce(&advanced.sends.owned.as_slice()[0]),
            12
        );
        let ShutdownState::Stopping { awaiting, .. } = plan_first.state() else {
            panic!("second declared phase did not start");
        };
        assert_eq!(awaiting, &[12]);

        let mut shutdown_first = shutdown_after_children(Application::complete())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize()
            .unwrap()
            .behavior;
        let waiting = shutdown_first.on(ShutdownRequested).unwrap();
        assert_transition_effect_counts!(waiting, shutdowns = 0, reports = 0);
        assert!(matches!(
            shutdown_first.state(),
            ShutdownState::AwaitingPlanAfterShutdown
        ));
        let first_creation = shutdown_first
            .on_path::<_, Inside<Inside<Here>>>(CreationResolved::birth(11, MailAddr(101)))
            .unwrap();
        assert_transition_effect_counts!(first_creation, shutdowns = 0, reports = 0);
        let reported = shutdown_first
            .on_path::<_, Inside<Here>>(CreationResolved::birth(12, MailAddr(102)))
            .unwrap();
        let report = reported.sends.inner.inner.inner.owned.as_slice()[0]
            .plan()
            .clone();
        assert_transition_effect_counts!(reported, shutdowns = 0, reports = 1);
        let started = shutdown_first.on(InstallShutdownPlan::new(report)).unwrap();
        assert_transition_effect_counts!(started, shutdowns = 1, reports = 0);
        assert_eq!(
            heterogeneous::Selection::nonce(&started.sends.owned.as_slice()[0]),
            11
        );
        let ShutdownState::Stopping { plan, phase, .. } = shutdown_first.state() else {
            panic!("shutdown-first plan did not start");
        };
        assert_eq!(*phase, 0);
        assert_eq!(phase_nonces(plan), [11, 12]);
    }

    #[test]
    fn reversing_declarations_reverses_the_plan() {
        let initialized = shutdown_after_children(Application::complete())
            .shutdown_phase(GatewayRole)
            .shutdown_phase(StoreRole)
            .finish()
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        let first_creation = active
            .on_path::<_, Inside<Inside<Here>>>(CreationResolved::birth(11, MailAddr(101)))
            .unwrap();
        assert_transition_effect_counts!(first_creation, shutdowns = 0, reports = 0);
        let reported = active
            .on_path::<_, Inside<Here>>(CreationResolved::birth(12, MailAddr(102)))
            .unwrap();
        let plan = reported.sends.inner.inner.inner.owned.as_slice()[0]
            .plan()
            .clone();
        assert_transition_effect_counts!(reported, shutdowns = 0, reports = 1);
        assert_eq!(phase_nonces(&plan), [12, 11]);
        let installed = active.on(InstallShutdownPlan::new(plan)).unwrap();
        assert_transition_effect_counts!(installed, shutdowns = 0, reports = 0);
        let started = active.on(ShutdownRequested).unwrap();
        assert_transition_effect_counts!(started, shutdowns = 1, reports = 0);
        assert_eq!(
            heterogeneous::Selection::nonce(&started.sends.owned.as_slice()[0]),
            12
        );
        let ShutdownState::Stopping { awaiting, .. } = active.state() else {
            panic!("reversed first phase did not start");
        };
        assert_eq!(awaiting, &[12]);
        let advanced = active
            .on(ChildStopped::new(
                12,
                Ok(Exit::Normal),
                std::time::Instant::now(),
            ))
            .unwrap();
        assert_transition_effect_counts!(advanced, shutdowns = 1, reports = 0);
        assert_eq!(
            heterogeneous::Selection::nonce(&advanced.sends.owned.as_slice()[0]),
            11
        );
        let ShutdownState::Stopping { awaiting, .. } = active.state() else {
            panic!("reversed second phase did not start");
        };
        assert_eq!(awaiting, &[11]);
    }

    #[test]
    fn rejected_creation_remains_a_typed_planning_failure() {
        let mut active = shutdown_after_children(Application::complete())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize()
            .unwrap()
            .behavior;
        let result = active.on_path::<_, Inside<Inside<Here>>>(CreationResolved::new(
            11,
            CreationKind::Birth,
            Err(behavior::CreationRejection::EnvironmentFailed),
        ));
        let Err(error) = result else {
            panic!("rejected creation unexpectedly produced actions");
        };
        assert!(matches!(
            error,
            ShutdownCoordinatorError::Behavior(ChildShutdownPlanError::CreationRejected {
                position: 1,
                observed: CreationResolved {
                    nonce: 11,
                    kind: CreationKind::Birth,
                    result: Err(behavior::CreationRejection::EnvironmentFailed),
                },
            })
        ));
    }

    #[test]
    fn configured_children_are_total_and_unique_before_any_observation_is_emitted() {
        let missing = shutdown_after_children(Application::missing_store())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize();
        assert!(matches!(
            missing,
            Err(ShutdownCoordinatorError::Behavior(
                ChildShutdownPlanError::MissingCreation { position: 1 }
            ))
        ));

        let duplicate = shutdown_after_children(Application::duplicate_store())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize();
        assert!(matches!(
            duplicate,
            Err(ShutdownCoordinatorError::Behavior(
                ChildShutdownPlanError::DuplicateCreation { position: 1 }
            ))
        ));
    }

    #[test]
    fn mismatched_and_stale_creation_facts_are_returned_complete() {
        let mut mismatched = shutdown_after_children(Application::complete())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize()
            .unwrap()
            .behavior;
        let observed = CreationResolved::replacement_incarnation(91, 90, MailAddr(191));
        let Err(ShutdownCoordinatorError::Behavior(
            ChildShutdownPlanError::UnexpectedCreationResult {
                position,
                expected,
                observed: returned,
            },
        )) = mismatched.on_path::<_, Inside<Inside<Here>>>(observed)
        else {
            panic!("mismatched fact was not returned through the typed failure");
        };
        assert_eq!(position, 1);
        assert_eq!(
            expected,
            ChildCreationExpectation::Awaiting {
                nonce: 11,
                kind: CreationKind::Birth,
            }
        );
        assert_eq!(returned, observed);

        let mut stale = shutdown_after_children(Application::complete())
            .shutdown_phase(StoreRole)
            .shutdown_phase(GatewayRole)
            .finish()
            .initialize()
            .unwrap()
            .behavior;
        let first_creation = stale
            .on_path::<_, Inside<Inside<Here>>>(CreationResolved::birth(11, MailAddr(101)))
            .unwrap();
        assert_transition_effect_counts!(first_creation, shutdowns = 0, reports = 0);
        let reported = stale
            .on_path::<_, Inside<Here>>(CreationResolved::birth(12, MailAddr(102)))
            .unwrap();
        assert_transition_effect_counts!(reported, shutdowns = 0, reports = 1);
        let observed = CreationResolved::birth(12, MailAddr(202));
        let Err(ShutdownCoordinatorError::Behavior(
            ChildShutdownPlanError::UnexpectedCreationResult {
                position,
                expected,
                observed: returned,
            },
        )) = stale.on_path::<_, Inside<Here>>(observed)
        else {
            panic!("stale fact was silently discarded");
        };
        assert_eq!(position, 0);
        assert_eq!(expected, ChildCreationExpectation::PlanReported);
        assert_eq!(returned, observed);
    }

    #[test]
    fn an_application_without_children_reports_the_empty_plan_during_initialization() {
        let initialized = shutdown_after_children(EmptyApplication)
            .finish()
            .initialize()
            .unwrap();
        let report = &initialized.actions.sends.inner.owned.as_slice()[0];
        assert!(report.plan().phases().is_empty());
    }
}

/// Start declaring shutdown phases for every direct child role of `application`.
///
/// Each call to [`ChildShutdownPhases::shutdown_phase`] consumes one declared
/// role from the available child product. [`ChildShutdownPhases::finish`] is
/// therefore available only after every direct child appears exactly once.
#[must_use]
pub fn shutdown_after_children<B>(
    application: B,
) -> ChildShutdownPhases<B, AvailableFor<B>, NoPhases>
where
    B: Behavior,
    <B::Birth as BirthMode>::Child: FoldBirthNode<AvailableChildren>,
{
    ChildShutdownPhases {
        application,
        available: PhantomData,
        phases: PhantomData,
    }
}

/// Inferred builder for statically complete child shutdown phases.
pub struct ChildShutdownPhases<B, Available, Phases> {
    application: B,
    available: PhantomData<fn() -> Available>,
    phases: PhantomData<fn() -> Phases>,
}

/// Generic consumer operation for entering the one inferred shutdown-phase builder.
///
/// This operation is the associated-output form of [`shutdown_after_children`].
/// It exists so framework code can carry the returned builder without naming
/// its closed initial availability proof; it does not introduce another
/// planner or phase representation.
pub trait BeginShutdownPhases: Behavior + Sized {
    type Output;

    #[must_use]
    fn begin_shutdown_phases(self) -> Self::Output;
}

impl<B> BeginShutdownPhases for B
where
    B: Behavior,
    <B::Birth as BirthMode>::Child: FoldBirthNode<AvailableChildren>,
{
    type Output = ChildShutdownPhases<B, AvailableFor<B>, NoPhases>;

    fn begin_shutdown_phases(self) -> Self::Output {
        shutdown_after_children(self)
    }
}

/// Generic consumer operation for appending one statically valid shutdown phase.
///
/// Framework code can carry [`Output`](Self::Output) without naming the
/// builder's private availability proof. Implementations do not plan or store
/// phases independently; they delegate to [`ChildShutdownPhases::shutdown_phase`].
pub trait DeclareShutdownPhase<Role>: Sized {
    type Output;

    #[must_use]
    fn shutdown_phase(self, role: Role) -> Self::Output;
}

/// Generic consumer operation for finishing a complete shutdown declaration.
///
/// This trait is implemented only after every declared direct child has been
/// assigned exactly once. Its associated output preserves the complete
/// heterogeneous coordinator type without asking a framework to reproduce the
/// builder's typestate vocabulary.
pub trait FinishShutdownPhases: Sized {
    type Output;

    #[must_use]
    fn finish(self) -> Self::Output;
}

#[allow(
    private_bounds,
    reason = "the public inferred builder hides its closed type-level phase proof"
)]
impl<B, Available, Phases> ChildShutdownPhases<B, Available, Phases>
where
    B: Behavior,
{
    /// Append one child as the next shutdown phase.
    ///
    /// Foreign roles, roles declared for another application, wrongly typed
    /// roles, and duplicate roles have no applicable implementation.
    ///
    /// A duplicate role is rejected independently:
    ///
    /// ```compile_fail,E0277
    /// use behavior_actors::{
    ///     BehaviorActed, MailAddr, Never, StopOnShutdown, shutdown_after_children,
    /// };
    /// struct Worker;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never)]
    /// impl Worker {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// type ManagedWorker = StopOnShutdown<Worker>;
    /// struct Application;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never, births = {
    ///     store: ManagedWorker,
    ///     gateway: ManagedWorker,
    /// })]
    /// impl Application {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// let _ = shutdown_after_children(Application)
    ///     .shutdown_phase(ApplicationChild::Store)
    ///     .shutdown_phase(ApplicationChild::Store);
    /// ```
    ///
    /// A role that belongs to no child declaration is rejected independently:
    ///
    /// ```compile_fail,E0277
    /// use behavior_actors::{
    ///     BehaviorActed, MailAddr, Never, StopOnShutdown, shutdown_after_children,
    /// };
    /// struct Worker;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never)]
    /// impl Worker {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// type ManagedWorker = StopOnShutdown<Worker>;
    /// struct Application;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never, births = {
    ///     store: ManagedWorker,
    ///     gateway: ManagedWorker,
    /// })]
    /// impl Application {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// struct ForeignRole;
    /// let _ = shutdown_after_children(Application).shutdown_phase(ForeignRole);
    /// ```
    ///
    /// A route value cannot stand in for its declared role:
    ///
    /// ```compile_fail,E0277
    /// use behavior_actors::{
    ///     BehaviorActed, MailAddr, Never, StopOnShutdown, shutdown_after_children,
    /// };
    /// struct Worker;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never)]
    /// impl Worker {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// type ManagedWorker = StopOnShutdown<Worker>;
    /// struct Application;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never, births = {
    ///     store: ManagedWorker,
    ///     gateway: ManagedWorker,
    /// })]
    /// impl Application {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// let routes = ApplicationChildrenRoutes::new(1, 2);
    /// let _ = shutdown_after_children(Application).shutdown_phase(routes.store);
    /// ```
    #[must_use]
    pub fn shutdown_phase<Role>(
        self,
        _: Role,
    ) -> ChildShutdownPhases<
        B,
        <Available as AssignAt<<B as ResolveChildOccurrence<Role>>::Position>>::Assigned,
        Phase<Role, Phases>,
    >
    where
        B: ResolveChildOccurrence<Role>,
        Available: AssignAt<<B as ResolveChildOccurrence<Role>>::Position>,
    {
        ChildShutdownPhases {
            application: self.application,
            available: PhantomData,
            phases: PhantomData,
        }
    }
}

#[allow(
    private_bounds,
    reason = "the public consumer operation preserves the builder's closed availability proof"
)]
impl<B, Available, Phases, Role> DeclareShutdownPhase<Role>
    for ChildShutdownPhases<B, Available, Phases>
where
    B: Behavior + ResolveChildOccurrence<Role>,
    Available: AssignAt<<B as ResolveChildOccurrence<Role>>::Position>,
{
    type Output = ChildShutdownPhases<
        B,
        <Available as AssignAt<<B as ResolveChildOccurrence<Role>>::Position>>::Assigned,
        Phase<Role, Phases>,
    >;

    fn shutdown_phase(self, role: Role) -> Self::Output {
        ChildShutdownPhases::shutdown_phase(self, role)
    }
}

#[allow(
    private_bounds,
    reason = "the public inferred builder hides its closed plan-construction proof"
)]
impl<B, Available, Phases> ChildShutdownPhases<B, Available, Phases>
where
    B: Behavior,
    B::Birth: BirthMode,
    Available: AllAssigned,
    <B::Birth as BirthMode>::Child: FoldBirthNode<ShutdownTargets<B>>,
    Phases: BuildShutdownPlan<B, TargetsFor<B>>,
    TargetsFor<B>:
        super::shutdown_coordinator::heterogeneous::Selection<Addr = crate::BehaviorAddr<B>> + Copy,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
{
    /// Complete the application after proving that every child role occurs in
    /// exactly one declared phase.
    ///
    /// The final actor's source-indexed event ingress carries the plan through
    /// arbitrary outer layers without exposing a structural path.
    ///
    /// ```compile_fail,E0599
    /// use behavior_actors::{
    ///     BehaviorActed, MailAddr, Never, StopOnShutdown, shutdown_after_children,
    /// };
    /// struct Worker;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never)]
    /// impl Worker {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// type ManagedWorker = StopOnShutdown<Worker>;
    /// struct Application;
    /// #[behavior_actors::behavior(addr = MailAddr, message = Never, births = {
    ///     store: ManagedWorker,
    ///     gateway: ManagedWorker,
    /// })]
    /// impl Application {
    ///     fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
    ///         match message {}
    ///     }
    /// }
    /// // `gateway` remains unassigned, so `finish` does not exist here.
    /// let incomplete = shutdown_after_children(Application)
    ///     .shutdown_phase(ApplicationChild::Store)
    ///     .finish();
    /// ```
    #[must_use]
    pub fn finish(
        self,
    ) -> HeterogeneousShutdownCoordinator<ChildShutdownPlan<B, Phases>, TargetsFor<B>>
    where
        ChildShutdownPlan<B, Phases>: Behavior<Protocol = B::Protocol>,
    {
        HeterogeneousShutdownCoordinator::awaiting_plan(ChildShutdownPlan::new(self.application))
    }
}

#[allow(
    private_bounds,
    reason = "the public consumer operation preserves the builder's closed finishing proof"
)]
impl<B, Available, Phases> FinishShutdownPhases for ChildShutdownPhases<B, Available, Phases>
where
    B: Behavior,
    B::Birth: BirthMode,
    Available: AllAssigned,
    <B::Birth as BirthMode>::Child: FoldBirthNode<ShutdownTargets<B>>,
    Phases: BuildShutdownPlan<B, TargetsFor<B>>,
    TargetsFor<B>:
        super::shutdown_coordinator::heterogeneous::Selection<Addr = crate::BehaviorAddr<B>> + Copy,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq,
    ChildShutdownPlan<B, Phases>: Behavior<Protocol = B::Protocol>,
{
    type Output = HeterogeneousShutdownCoordinator<ChildShutdownPlan<B, Phases>, TargetsFor<B>>;

    fn finish(self) -> Self::Output {
        ChildShutdownPhases::finish(self)
    }
}

/// Type-level marker for the absence of declared phases.
pub struct NoPhases;

/// One declared phase appended after `Earlier` phases.
pub struct Phase<Role, Earlier>(PhantomData<fn() -> (Role, Earlier)>);

/// Mapper producing the child product consumed by phase declarations.
pub struct AvailableChildren;

/// One not-yet-declared direct child.
pub struct Unassigned<Child, Tail>(PhantomData<fn() -> (Child, Tail)>);

/// One direct child already assigned to a phase.
pub struct Assigned<Child, Tail>(PhantomData<fn() -> (Child, Tail)>);

/// End of the direct-child declaration product.
pub struct NoChildren;

impl BirthNodeMapper for AvailableChildren {
    type Empty = NoChildren;
    type Mapped<Position, Child: Behavior, Tail> = Unassigned<Child, Tail>;
}

/// Statically consume the child at one structural position.
pub trait AssignAt<Position> {
    type Assigned;
}

impl<Child, Tail> AssignAt<ChildHead> for Unassigned<Child, Tail> {
    type Assigned = Assigned<Child, Tail>;
}

impl<Child, Tail, Position> AssignAt<ChildTail<Position>> for Unassigned<Child, Tail>
where
    Tail: AssignAt<Position>,
{
    type Assigned = Unassigned<Child, <Tail as AssignAt<Position>>::Assigned>;
}

impl<Child, Tail, Position> AssignAt<ChildTail<Position>> for Assigned<Child, Tail>
where
    Tail: AssignAt<Position>,
{
    type Assigned = Assigned<Child, <Tail as AssignAt<Position>>::Assigned>;
}

/// Proof that no direct child remains unassigned.
pub trait AllAssigned {}

impl AllAssigned for NoChildren {}

impl<Child, Tail: AllAssigned> AllAssigned for Assigned<Child, Tail> {}

/// Mapper producing the coordinator's existing heterogeneous target sum.
pub struct ShutdownTargets<B: Behavior>(PhantomData<fn() -> B>);

impl<B: Behavior> BirthNodeMapper for ShutdownTargets<B> {
    type Empty = NoShutdownTargets<crate::BehaviorAddr<B>>;
    type Mapped<Position, Child: Behavior, Tail> = ShutdownChoice<Child, Tail>;
}

/// Complete shutdown target sum derived from an application's direct births.
pub type TargetsFor<B> =
    FoldedBirthNode<<<B as Behavior>::Birth as BirthMode>::Child, ShutdownTargets<B>>;

/// Complete role-availability product derived from direct births.
pub type AvailableFor<B> =
    FoldedBirthNode<<<B as Behavior>::Birth as BirthMode>::Child, AvailableChildren>;

trait PositionNumber {
    const INDEX: usize;
}

impl PositionNumber for ChildHead {
    const INDEX: usize = 0;
}

impl<Position: PositionNumber> PositionNumber for ChildTail<Position> {
    const INDEX: usize = Position::INDEX + 1;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildCreation<N> {
    NotRequested,
    Awaiting { nonce: N, kind: CreationKind<N> },
    Established { nonce: N },
}

impl<N: Copy> ChildCreation<N> {
    fn expectation(self) -> ChildCreationExpectation<N> {
        match self {
            Self::NotRequested => ChildCreationExpectation::NotRequested,
            Self::Awaiting { nonce, kind } => ChildCreationExpectation::Awaiting { nonce, kind },
            Self::Established { nonce } => ChildCreationExpectation::Established { nonce },
        }
    }
}

enum Planning<N> {
    Collecting(Vec<ChildCreation<N>>),
    Reported,
}

enum PlannedEvent<E, A: Address> {
    Application(E),
    Creation {
        position: usize,
        result: CreationResolved<A>,
    },
}

/// Internal application composition that observes child creations and reports
/// one existing heterogeneous shutdown plan to its outer coordinator.
pub struct ChildShutdownPlan<B: Behavior, Phases> {
    application: B,
    planning: Planning<<crate::BehaviorAddr<B> as Address>::Nonce>,
    phases: PhantomData<fn() -> Phases>,
}

impl<B: Behavior, Phases> ChildShutdownPlan<B, Phases> {
    fn new(application: B) -> Self {
        Self {
            application,
            planning: Planning::Collecting(Vec::new()),
            phases: PhantomData,
        }
    }
}

impl<B, Phases> crate::BehaviorBase for ChildShutdownPlan<B, Phases>
where
    B: Behavior + crate::BehaviorBase,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.application.base()
    }
}

impl<B, Phases> crate::StashStatus for ChildShutdownPlan<B, Phases>
where
    B: Behavior + crate::StashStatus,
{
    fn stashed_messages(&self) -> usize {
        self.application.stashed_messages()
    }
}

type Plan<Targets> = HeterogeneousShutdownPlan<Targets>;
type ReportPlan<Targets> = ReportShutdownPlan<Plan<Targets>>;
type BaseSends<B, Targets> =
    SendLayer<InterpreterRequests<ReportPlan<Targets>>, <B as Behavior>::Sends>;

struct PlanningChildren;
struct PlannedEnd;
struct PlannedChild<Position, Child, Tail>(PhantomData<fn() -> (Position, Child, Tail)>);

impl BirthNodeMapper for PlanningChildren {
    type Empty = PlannedEnd;
    type Mapped<Position, Child: Behavior, Tail> = PlannedChild<Position, Child, Tail>;
}

trait PlanChildren<Node, A: Address, Event, Sends> {
    type Events;
    type Sends: SendEffects;

    const COUNT: usize;

    fn empty_sends(inner: Sends) -> Self::Sends;

    fn observe(
        child: &Node,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        sends: &mut Self::Sends,
        requested: &mut Vec<(usize, A::Nonce, CreationKind<A::Nonce>)>,
    );

    fn read(event: Self::Events) -> PlannedEvent<Event, A>;
}

impl<A, Event, Sends> PlanChildren<Never, A, Event, Sends> for PlannedEnd
where
    A: Address,
    Sends: SendEffects,
{
    type Events = EventLayer<Never, Event>;
    type Sends = Sends;
    const COUNT: usize = 0;

    fn empty_sends(inner: Sends) -> Self::Sends {
        inner
    }

    fn observe(
        child: &Never,
        _: A::Nonce,
        _: CreationKind<A::Nonce>,
        _: &mut Self::Sends,
        _: &mut Vec<(usize, A::Nonce, CreationKind<A::Nonce>)>,
    ) {
        match *child {}
    }

    fn read(event: Self::Events) -> PlannedEvent<Event, A> {
        match event {
            EventLayer::Owned(never) => match never {},
            EventLayer::Inner(event) => PlannedEvent::Application(event),
        }
    }
}

impl<A, Position, Event, Sends, Child> PlanChildren<Child, A, Event, Sends>
    for PlannedChild<Position, Child, PlannedEnd>
where
    A: Address,
    Position: PositionNumber,
    Sends: SendEffects,
    Child: Behavior,
    Child::Protocol: Protocol<Addr = A>,
{
    type Events = EventLayer<CreationResolved<A>, EventLayer<Never, Event>>;
    type Sends = SendLayer<InterpreterRequests<ObserveCreation<A, Position>>, Sends>;
    const COUNT: usize = 1;

    fn empty_sends(inner: Sends) -> Self::Sends {
        SendLayer::new(InterpreterRequests::empty(), inner)
    }

    fn observe(
        _: &Child,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        sends: &mut Self::Sends,
        requested: &mut Vec<(usize, A::Nonce, CreationKind<A::Nonce>)>,
    ) {
        sends.owned.send(ObserveCreation::new(nonce));
        requested.push((Position::INDEX, nonce, kind));
    }

    fn read(event: Self::Events) -> PlannedEvent<Event, A> {
        match event {
            EventLayer::Owned(result) => PlannedEvent::Creation {
                position: Position::INDEX,
                result,
            },
            EventLayer::Inner(EventLayer::Owned(never)) => match never {},
            EventLayer::Inner(EventLayer::Inner(event)) => PlannedEvent::Application(event),
        }
    }
}

impl<A, Position, Event, Sends, Head, Tail, TailPlan>
    PlanChildren<ChildChoice<Head, Tail>, A, Event, Sends>
    for PlannedChild<Position, Head, TailPlan>
where
    A: Address,
    Position: PositionNumber,
    Sends: SendEffects,
    Head: Behavior,
    Head::Protocol: Protocol<Addr = A>,
    TailPlan: PlanChildren<Tail, A, Event, Sends>,
{
    type Events = EventLayer<CreationResolved<A>, TailPlan::Events>;
    type Sends = SendLayer<InterpreterRequests<ObserveCreation<A, Position>>, TailPlan::Sends>;

    const COUNT: usize = 1 + TailPlan::COUNT;

    fn empty_sends(inner: Sends) -> Self::Sends {
        SendLayer::new(InterpreterRequests::empty(), TailPlan::empty_sends(inner))
    }

    fn observe(
        child: &ChildChoice<Head, Tail>,
        nonce: A::Nonce,
        kind: CreationKind<A::Nonce>,
        sends: &mut Self::Sends,
        requested: &mut Vec<(usize, A::Nonce, CreationKind<A::Nonce>)>,
    ) {
        match child {
            ChildChoice::Head(_) => {
                sends.owned.send(ObserveCreation::new(nonce));
                requested.push((Position::INDEX, nonce, kind));
            }
            ChildChoice::Tail(tail) => {
                TailPlan::observe(tail, nonce, kind, &mut sends.inner, requested);
            }
        }
    }

    fn read(event: Self::Events) -> PlannedEvent<Event, A> {
        match event {
            EventLayer::Owned(result) => PlannedEvent::Creation {
                position: Position::INDEX,
                result,
            },
            EventLayer::Inner(event) => TailPlan::read(event),
        }
    }
}

trait BuildShutdownPlan<B: Behavior, Targets> {
    fn build(
        children: &[ChildCreation<<crate::BehaviorAddr<B> as Address>::Nonce>],
    ) -> Result<Vec<Vec<Targets>>, ChildShutdownPlanError<B::Error, crate::BehaviorAddr<B>>>;
}

impl<B: Behavior, Targets> BuildShutdownPlan<B, Targets> for NoPhases {
    fn build(
        _: &[ChildCreation<<crate::BehaviorAddr<B> as Address>::Nonce>],
    ) -> Result<Vec<Vec<Targets>>, ChildShutdownPlanError<B::Error, crate::BehaviorAddr<B>>> {
        Ok(Vec::new())
    }
}

impl<B, Targets, Role, Earlier> BuildShutdownPlan<B, Targets> for Phase<Role, Earlier>
where
    B: Behavior + ResolveChildOccurrence<Role>,
    crate::BehaviorAddr<B>: Address,
    <B as ResolveChildOccurrence<Role>>::Position: PositionNumber,
    <<B as ResolveChildOccurrence<Role>>::Child as Behavior>::Protocol:
        Protocol<Addr = crate::BehaviorAddr<B>>,
    Targets: ShutdownTargetAt<
            <B as ResolveChildOccurrence<Role>>::Child,
            <B as ResolveChildOccurrence<Role>>::Position,
        >,
    Earlier: BuildShutdownPlan<B, Targets>,
{
    fn build(
        children: &[ChildCreation<<crate::BehaviorAddr<B> as Address>::Nonce>],
    ) -> Result<Vec<Vec<Targets>>, ChildShutdownPlanError<B::Error, crate::BehaviorAddr<B>>> {
        let mut phases = Earlier::build(children)?;
        let position = <<B as ResolveChildOccurrence<Role>>::Position as PositionNumber>::INDEX;
        let Some(ChildCreation::Established { nonce }) = children.get(position) else {
            return Err(ChildShutdownPlanError::ChildNotEstablished { position });
        };
        let route = ChildRoute::<
            <B as ResolveChildOccurrence<Role>>::Child,
            <B as ResolveChildOccurrence<Role>>::Position,
        >::new(*nonce);
        phases.push(vec![Targets::shutdown_target_at(route)]);
        Ok(phases)
    }
}

impl<B, Phases, A, Ph, Sends, Br, Node, Shape, Targets, Events, Planned> Behavior
    for ChildShutdownPlan<B, Phases>
where
    A: Address,
    A::Nonce: Copy + Eq,
    Sends: SendEffects,
    Br: BirthMode<Child = Node>,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: Protocol<Addr = A>,
    Node: FoldBirthNode<ShutdownTargets<B>, Folded = Targets>
        + FoldBirthNode<PlanningChildren, Folded = Shape>,
    Shape: PlanChildren<Node, A, B::Event, BaseSends<B, Targets>, Events = Events, Sends = Planned>,
    Targets: heterogeneous::Selection<Addr = A> + Copy,
    Phases: BuildShutdownPlan<B, Targets>,
    Events: behavior::UserEvent<Addr = A, Message = <B::Protocol as Protocol>::Msg>,
    Planned: SendEffects + behavior::SendsFor<Events>,
{
    type Protocol = B::Protocol;
    type Event = Events;
    type Sends = Planned;
    type Ph = Ph;
    type Error = ChildShutdownPlanError<B::Error, A>;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.application)
            .map_err(ChildShutdownPlanError::Behavior)?;
        self.wrap_initialization(actions)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match Shape::read(event) {
            PlannedEvent::Application(event) => {
                let actions = behavior::delegate_transition(&mut self.application, event)
                    .map_err(ChildShutdownPlanError::Behavior)?;
                Ok(self.wrap_transition(actions))
            }
            PlannedEvent::Creation { position, result } => self.creation_resolved(position, result),
        }
    }
}

#[allow(
    private_bounds,
    reason = "the public compiler representation hides its closed planning fold"
)]
impl<B, Phases, A, Ph, Sends, Br, Node, Shape, Targets, Events, Planned>
    ChildShutdownPlan<B, Phases>
where
    A: Address,
    A::Nonce: Copy + Eq,
    Sends: SendEffects,
    Br: BirthMode<Child = Node>,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: Protocol<Addr = A>,
    Node: FoldBirthNode<ShutdownTargets<B>, Folded = Targets>
        + FoldBirthNode<PlanningChildren, Folded = Shape>,
    Shape: PlanChildren<Node, A, B::Event, BaseSends<B, Targets>, Events = Events, Sends = Planned>,
    Targets: heterogeneous::Selection<Addr = A> + Copy,
    Phases: BuildShutdownPlan<B, Targets>,
    Events: behavior::UserEvent<Addr = A, Message = <B::Protocol as Protocol>::Msg>,
    Planned: SendEffects + behavior::SendsFor<Events>,
{
    fn wrap_initialization(
        &mut self,
        actions: Actions<A, Ph, Sends, Br>,
    ) -> Result<Actions<A, Ph, Planned, Br>, ChildShutdownPlanError<B::Error, A>> {
        let Actions {
            sends,
            creates,
            become_,
        } = actions;
        let reports = if Shape::COUNT == 0 {
            let phases = Phases::build(&[])?;
            let plan = HeterogeneousShutdownPlan::new(phases)
                .map_err(ChildShutdownPlanError::InvalidPlan)?;
            self.planning = Planning::Reported;
            InterpreterRequests::one(crate::ReportShutdownPlan::new(plan))
        } else {
            InterpreterRequests::empty()
        };
        let base = SendLayer::new(reports, sends);
        let mut planned = Shape::empty_sends(base);
        let mut requested = Vec::new();
        for creation in &creates {
            Shape::observe(
                &creation.child,
                creation.nonce,
                creation.kind,
                &mut planned,
                &mut requested,
            );
        }
        if Shape::COUNT != 0 {
            let mut next = vec![ChildCreation::NotRequested; Shape::COUNT];
            for (position, nonce, kind) in requested {
                let Some(state) = next.get_mut(position) else {
                    return Err(ChildShutdownPlanError::DuplicateCreation { position });
                };
                if !matches!(state, ChildCreation::NotRequested) {
                    return Err(ChildShutdownPlanError::DuplicateCreation { position });
                }
                *state = ChildCreation::Awaiting { nonce, kind };
            }
            if let Some(position) = next
                .iter()
                .position(|child| matches!(child, ChildCreation::NotRequested))
            {
                return Err(ChildShutdownPlanError::MissingCreation { position });
            }
            self.planning = Planning::Collecting(next);
        }
        Ok(Actions::new(planned, creates, become_))
    }

    fn wrap_transition(&self, actions: Actions<A, Ph, Sends, Br>) -> Actions<A, Ph, Planned, Br> {
        let Actions {
            sends,
            creates,
            become_,
        } = actions;
        let base = SendLayer::new(InterpreterRequests::empty(), sends);
        Actions::new(Shape::empty_sends(base), creates, become_)
    }

    fn creation_resolved(
        &mut self,
        position: usize,
        result: CreationResolved<A>,
    ) -> Result<Actions<A, Ph, Planned, Br>, ChildShutdownPlanError<B::Error, A>> {
        let Planning::Collecting(children) = &self.planning else {
            return Err(ChildShutdownPlanError::UnexpectedCreationResult {
                position,
                expected: ChildCreationExpectation::PlanReported,
                observed: result,
            });
        };
        let Some(state) = children.get(position).copied() else {
            return Err(ChildShutdownPlanError::UnexpectedCreationResult {
                position,
                expected: ChildCreationExpectation::UnknownPosition,
                observed: result,
            });
        };
        let ChildCreation::Awaiting { nonce, kind } = state else {
            return Err(ChildShutdownPlanError::UnexpectedCreationResult {
                position,
                expected: state.expectation(),
                observed: result,
            });
        };
        if result.nonce != nonce || result.kind != kind {
            return Err(ChildShutdownPlanError::UnexpectedCreationResult {
                position,
                expected: ChildCreationExpectation::Awaiting { nonce, kind },
                observed: result,
            });
        }
        let established = match result.result {
            Ok(_) => ChildCreation::Established { nonce },
            Err(_) => {
                return Err(ChildShutdownPlanError::CreationRejected {
                    position,
                    observed: result,
                });
            }
        };
        let mut next = children.clone();
        next[position] = established;
        if !next
            .iter()
            .all(|child| matches!(child, ChildCreation::Established { .. }))
        {
            self.planning = Planning::Collecting(next);
            return Ok(Actions::cont());
        }
        let phases = Phases::build(&next)?;
        let plan =
            HeterogeneousShutdownPlan::new(phases).map_err(ChildShutdownPlanError::InvalidPlan)?;
        let base = SendLayer::new(
            InterpreterRequests::one(crate::ReportShutdownPlan::new(plan)),
            B::Sends::empty(),
        );
        let sends = Shape::empty_sends(base);
        self.planning = Planning::Reported;
        Ok(Actions::new(sends, Vec::new(), Step::Continue))
    }
}
