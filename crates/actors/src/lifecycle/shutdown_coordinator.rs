//! Phased child-topology shutdown.

use crate::{ChildShutdownRejected, ChildShutdownRejection, ChildStopped, ShutdownChild};
use behavior::{
    ActionItem, ActionItemResult, Actions, Address, Behavior, BehaviorActed, BirthMode, ChildHead,
    ChildRole, ChildTail, ClassifySettlement, CreationId, EventIngress, Here, InjectEvent, Inside,
    Interpretation, InterpretationProgress, InterpreterRequests, ItemSettlement, SendEffects,
    SendLayer, SettledItem, SourceCustody, SourceProgress, SourceSettlementCustody, finish_item,
    prepare_item,
};
use behavior::{User, UserEvent};

/// Validated ordered shutdown phases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShutdownPlan<N> {
    phases: Vec<Vec<N>>,
}

impl<N: Copy + Eq> ShutdownPlan<N> {
    /// Validate non-empty phases and globally unique child IDs.
    ///
    /// # Errors
    /// Returns the offending phase or duplicate child ID.
    pub fn new(phases: impl IntoIterator<Item = Vec<N>>) -> Result<Self, ShutdownPlanError<N>> {
        let phases: Vec<_> = phases.into_iter().collect();
        let mut seen = Vec::new();
        for (phase, children) in phases.iter().enumerate() {
            if children.is_empty() {
                return Err(ShutdownPlanError::EmptyPhase { phase });
            }
            for &child in children {
                if seen.contains(&child) {
                    return Err(ShutdownPlanError::DuplicateChild(child));
                }
                seen.push(child);
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
/// protocols. The value carries only the creator-local creation ID, never an erased
/// actor, address, request, or runtime protocol key.
///
/// A child whose event algebra has no direct shutdown owner cannot enter a
/// validated heterogeneous plan:
///
/// ```compile_fail
/// struct Plain;
/// impl behavior::Protocol for Plain { type Addr = behavior::MailAddr; type Msg = (); }
/// impl behavior::Behavior for Plain {
///     type Protocol = Self;
///     type Event = behavior::User<behavior::MailAddr, ()>;
///     type Sends = Vec<behavior::Never>;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// type Targets = behavior_actors::ShutdownChoice<Plain, behavior_actors::NoShutdownTargets<behavior::MailAddr>>;
/// let child = behavior::CreationSequence::new()
///     .issue()
///     .expect("the first child creation ID exists");
/// let _ = behavior_actors::HeterogeneousShutdownPlan::new([vec![Targets::child(child)]]);
/// ```
pub enum ShutdownChoice<C: Behavior, Tail> {
    Child {
        creation: CreationId,
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
    pub const fn child(creation: CreationId) -> Self {
        Self::Child {
            creation,
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
/// Implementations preserve the creation ID exactly. `ChildHead` selects the
/// current branch; `ChildTail<P>` delegates to the existing tail. This trait
/// introduces no alternate target product, runtime lookup, or shutdown effect.
pub trait ShutdownTargetAt<Child: Behavior, Position>: named_shutdown::Target + Sized {
    /// Lower one creator-local child ID into its statically selected branch.
    fn shutdown_target_at(creation: CreationId) -> Self;
}

mod named_shutdown {
    pub trait Target {}
}

impl<Child: Behavior, Tail> named_shutdown::Target for ShutdownChoice<Child, Tail> {}

impl<Child: Behavior, Tail> ShutdownTargetAt<Child, ChildHead> for ShutdownChoice<Child, Tail> {
    fn shutdown_target_at(creation: CreationId) -> Self {
        Self::child(creation)
    }
}

impl<Head, Tail, Child, Position> ShutdownTargetAt<Child, ChildTail<Position>>
    for ShutdownChoice<Head, Tail>
where
    Head: Behavior,
    Child: Behavior,
    Tail: ShutdownTargetAt<Child, Position>,
{
    fn shutdown_target_at(creation: CreationId) -> Self {
        Self::other(Tail::shutdown_target_at(creation))
    }
}

/// Lower one Behavior-owned named child ID into an existing heterogeneous
/// shutdown target sum.
///
/// `Parent` fixes the [`ChildRole`] implementation, allowing the compiler to
/// select the exact structural position even when several roles share one
/// child behavior type. An unrelated role cannot select a target. The creation
/// ID remains opaque creator-local correlation; it is not statically branded
/// with the selected role.
///
/// ```compile_fail
/// struct Worker;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = behavior::Never,
/// )]
/// impl Worker {
///     fn receive(
///         &mut self,
///         _: behavior::MailAddr,
///         message: behavior::Never,
///     ) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct Parent;
/// #[behavior::behavior(
///     addr = behavior::MailAddr,
///     message = behavior::Never,
///     births = { primary: Worker, fallback: Worker },
///     creation_settlements = retain_for_retirement,
/// )]
/// impl Parent {
///     fn receive(
///         &mut self,
///         _: behavior::MailAddr,
///         message: behavior::Never,
///     ) -> behavior::BehaviorActed<Self> {
///         match message {}
///     }
/// }
/// struct UnrelatedRole;
/// type Targets = behavior_actors::ShutdownChoice<
///     Worker,
///     behavior_actors::ShutdownChoice<
///         Worker,
///         behavior_actors::NoShutdownTargets<behavior::MailAddr>,
///     >,
/// >;
/// let mut creations = behavior::CreationSequence::new();
/// let child = creations.issue().expect("fixture creation ID");
/// let _: Targets =
///     behavior_actors::shutdown_target::<Parent, _, Targets>(UnrelatedRole, child);
/// ```
#[must_use]
pub fn shutdown_target<Parent, Role, Targets>(_: Role, creation: CreationId) -> Targets
where
    Parent: Behavior,
    Role: ChildRole<Parent>,
    Targets: ShutdownTargetAt<Role::Child, Role::Position>,
{
    Targets::shutdown_target_at(creation)
}

impl<C, Tail> Copy for ShutdownChoice<C, Tail>
where
    C: Behavior,
    <behavior::BehaviorAddr<C> as Address>::Nonce: Copy,
    Tail: Copy,
{
}

impl<C, Tail> Clone for ShutdownChoice<C, Tail>
where
    C: Behavior,
    <behavior::BehaviorAddr<C> as Address>::Nonce: Copy,
    Tail: Copy,
{
    fn clone(&self) -> Self {
        *self
    }
}

/// Settlement shape of one closed heterogeneous shutdown choice.
#[doc(hidden)]
pub enum HeterogeneousShutdownItem<Child, Tail> {
    Child(Child),
    Other(Tail),
}

impl<Child, Tail> behavior::ClassifySettlement for HeterogeneousShutdownItem<Child, Tail>
where
    Child: behavior::ClassifySettlement,
    Tail: behavior::ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        match self {
            Self::Child(child) => child.settlement_status(),
            Self::Other(other) => other.settlement_status(),
        }
    }
}

pub(crate) mod heterogeneous {
    use super::*;

    pub trait Selection: Sized + Send {
        type Addr: Address;
        fn creation(&self) -> CreationId;
    }

    #[doc(hidden)]
    pub trait ChoiceSettlements<Occurrence>: Sized + Send {
        type Settlements: Send + ClassifySettlement;
        type InterpretationCustody;
        fn prepare_interpretation(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        );
        fn finish_interpretation(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        );
        fn unattempted(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        );
    }

    pub trait InterpretChoice<I, E, Path, Occurrence>: ChoiceSettlements<Occurrence> {
        fn settle(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
            interpreter: &mut I,
        ) -> impl core::future::Future<Output = ()> + Send;
    }

    impl<A: Address> Selection for NoShutdownTargets<A> {
        type Addr = A;
        fn creation(&self) -> CreationId {
            match self.never {}
        }
    }

    impl<A: Address, Occurrence> ChoiceSettlements<Occurrence> for NoShutdownTargets<A> {
        type Settlements = behavior::Never;
        type InterpretationCustody = Self;
        fn prepare_interpretation(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        ) {
            match progress.take() {
                Some(
                    InterpretationProgress::Original(original)
                    | InterpretationProgress::Interpreting(original),
                ) => match original.never {},
                retained => *progress = retained,
            }
        }
        fn finish_interpretation(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        ) {
            <Self as ChoiceSettlements<Occurrence>>::prepare_interpretation(progress);
        }
        fn unattempted(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        ) {
            <Self as ChoiceSettlements<Occurrence>>::prepare_interpretation(progress);
        }
    }

    impl<I: Send, E, Path, A: Address, Occurrence> InterpretChoice<I, E, Path, Occurrence>
        for NoShutdownTargets<A>
    {
        async fn settle(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
            _: &mut I,
        ) {
            <Self as ChoiceSettlements<Occurrence>>::prepare_interpretation(progress);
        }
    }

    impl<C, Tail> Selection for ShutdownChoice<C, Tail>
    where
        C: Behavior,
        C::Event: InjectEvent<crate::ShutdownRequested, Here>,
        Tail: Selection<Addr = behavior::BehaviorAddr<C>>,
    {
        type Addr = behavior::BehaviorAddr<C>;
        fn creation(&self) -> CreationId {
            match self {
                Self::Child { creation, .. } => *creation,
                Self::Other(target) => target.creation(),
            }
        }
    }

    impl<C, Tail, Occurrence> ChoiceSettlements<Occurrence> for ShutdownChoice<C, Tail>
    where
        C: Behavior,
        ShutdownChild<C, Occurrence>: ActionItem,
        Tail: ChoiceSettlements<ChildTail<Occurrence>>,
    {
        type Settlements = HeterogeneousShutdownItem<
            ActionItemResult<ShutdownChild<C, Occurrence>>,
            Tail::Settlements,
        >;
        type InterpretationCustody = HeterogeneousShutdownItem<
            (
                Option<ShutdownChild<C, Occurrence>>,
                Option<
                    ItemSettlement<
                        ShutdownChild<C, Occurrence>,
                        <ShutdownChild<C, Occurrence> as ActionItem>::Accepted,
                        <ShutdownChild<C, Occurrence> as ActionItem>::Rejection,
                        <ShutdownChild<C, Occurrence> as ActionItem>::Prerequisite,
                    >,
                >,
            ),
            Option<InterpretationProgress<Tail, Tail::InterpretationCustody, Tail::Settlements>>,
        >;
        fn prepare_interpretation(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        ) {
            *progress = match progress.take() {
                Some(InterpretationProgress::Original(Self::Child { creation, .. })) => Some(
                    InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Child((
                        Some(ShutdownChild::new(creation)),
                        None,
                    ))),
                ),
                Some(InterpretationProgress::Original(Self::Other(target))) => Some(
                    InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Other(Some(
                        InterpretationProgress::Original(target),
                    ))),
                ),
                retained => retained,
            };
        }
        fn finish_interpretation(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        ) {
            let complete = matches!(
                progress,
                Some(InterpretationProgress::Interpreting(
                    HeterogeneousShutdownItem::Child((None, Some(_)))
                )) | Some(InterpretationProgress::Interpreting(
                    HeterogeneousShutdownItem::Other(Some(InterpretationProgress::Completed(_)))
                ))
            );
            if !complete {
                return;
            }
            *progress = match progress.take() {
                Some(InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Child((
                    None,
                    Some(received),
                )))) => {
                    let received = match received {
                        received @ ItemSettlement::Corrupt { .. } => Interpretation::Corrupt(
                            HeterogeneousShutdownItem::Child(SettledItem::Attempted(received)),
                        ),
                        received => Interpretation::Complete(HeterogeneousShutdownItem::Child(
                            SettledItem::Attempted(received),
                        )),
                    };
                    Some(InterpretationProgress::Completed(received))
                }
                Some(InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Other(
                    Some(InterpretationProgress::Completed(received)),
                ))) => Some(InterpretationProgress::Completed(
                    received.map(HeterogeneousShutdownItem::Other),
                )),
                retained => retained,
            };
        }
        fn unattempted(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
        ) {
            if let Some(InterpretationProgress::Original(Self::Child { .. })) = progress {
                *progress = match progress.take() {
                    Some(InterpretationProgress::Original(Self::Child { creation, .. })) => {
                        Some(InterpretationProgress::Completed(Interpretation::Complete(
                            HeterogeneousShutdownItem::Child(SettledItem::Unattempted(
                                ShutdownChild::new(creation),
                            )),
                        )))
                    }
                    retained => retained,
                };
                return;
            }
            <Self as ChoiceSettlements<Occurrence>>::prepare_interpretation(progress);
            if let Some(InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Other(
                target,
            ))) = progress
            {
                Tail::unattempted(target);
            }
            <Self as ChoiceSettlements<Occurrence>>::finish_interpretation(progress);
        }
    }

    impl<I, E, Path, C, Tail, Occurrence> InterpretChoice<I, E, Path, Occurrence>
        for ShutdownChoice<C, Tail>
    where
        I: behavior::InterpretItem<ShutdownChild<C, Occurrence>, E, Path> + Send,
        C: Behavior,
        <behavior::BehaviorAddr<C> as Address>::Nonce: Send,
        Tail: InterpretChoice<I, E, Path, ChildTail<Occurrence>>,
        Tail::InterpretationCustody: Send,
    {
        async fn settle(
            progress: &mut Option<
                InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
            >,
            interpreter: &mut I,
        ) {
            <Self as ChoiceSettlements<Occurrence>>::prepare_interpretation(progress);
            match progress {
                Some(InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Child((
                    input,
                    received,
                )))) => {
                    if matches!((&*input, &*received), (Some(_), None)) {
                        <I as behavior::InterpretItem<ShutdownChild<C,Occurrence>,E,Path>>::interpret_item(interpreter,input,received).await;
                    }
                }
                Some(InterpretationProgress::Interpreting(HeterogeneousShutdownItem::Other(
                    target,
                ))) => {
                    Tail::settle(target, interpreter).await;
                    Tail::finish_interpretation(target);
                }
                _ => return,
            }
            <Self as ChoiceSettlements<Occurrence>>::finish_interpretation(progress);
        }
    }
}

#[doc(hidden)]
pub use heterogeneous::ChoiceSettlements as HeterogeneousShutdownChoiceSettlement;

/// Validated shutdown phases over an arbitrary closed child-protocol sum.
#[derive(Clone, PartialEq, Eq)]
pub struct HeterogeneousShutdownPlan<T: heterogeneous::Selection> {
    phases: Vec<Vec<T>>,
}

impl<T: heterogeneous::Selection> core::fmt::Debug for HeterogeneousShutdownPlan<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("HeterogeneousShutdownPlan")
            .field("phase_count", &self.phases.len())
            .finish_non_exhaustive()
    }
}

impl<T> HeterogeneousShutdownPlan<T>
where
    T: heterogeneous::Selection,
{
    /// Validate non-empty phases and global uniqueness in the creator's one
    /// child namespace, including collisions across protocol lanes.
    pub fn new(
        phases: impl IntoIterator<Item = Vec<T>>,
    ) -> Result<Self, ShutdownPlanError<CreationId>> {
        let phases: Vec<_> = phases.into_iter().collect();
        let mut seen = Vec::new();
        for (phase, children) in phases.iter().enumerate() {
            if children.is_empty() {
                return Err(ShutdownPlanError::EmptyPhase { phase });
            }
            for child in children {
                let creation = heterogeneous::Selection::creation(child);
                if seen.contains(&creation) {
                    return Err(ShutdownPlanError::DuplicateChild(creation));
                }
                seen.push(creation);
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

impl<T> HeterogeneousShutdownSends<T> {
    /// Borrow the phase-ordered shutdown selections emitted by this turn.
    ///
    /// The order is the declaration order of the active phase. Retained
    /// selections from later phases are not exposed until that phase starts.
    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.requests
    }
}

impl<E, T: Send> behavior::SendsFor<E> for HeterogeneousShutdownSends<T> {}

impl<T> behavior::ClassifySettlement for HeterogeneousShutdownSends<T>
where
    T: behavior::ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        self.requests.settlement_status()
    }
}

impl<T> behavior::SendSettlements for HeterogeneousShutdownSends<T>
where
    T: heterogeneous::ChoiceSettlements<ChildHead>,
{
    type Settlements = HeterogeneousShutdownSends<T::Settlements>;
    type SourceCustody = Self::Settlements;
    type InterpretationCustody =
        Vec<Option<InterpretationProgress<T, T::InterpretationCustody, T::Settlements>>>;
    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        *progress = match progress.take() {
            Some(InterpretationProgress::Original(original)) => {
                Some(InterpretationProgress::Interpreting(
                    original
                        .requests
                        .into_iter()
                        .map(|request| Some(InterpretationProgress::Original(request)))
                        .collect(),
                ))
            }
            retained => retained,
        };
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        let Some(InterpretationProgress::Interpreting(requests)) = progress.as_ref() else {
            return;
        };
        if !requests
            .iter()
            .all(|request| matches!(request, Some(InterpretationProgress::Completed(_))))
        {
            return;
        }
        let Some(InterpretationProgress::Interpreting(requests)) = progress.take() else {
            return;
        };
        let mut remaining = requests.into_iter();
        let mut settled = Vec::with_capacity(remaining.len());
        while let Some(request) = remaining.next() {
            match request {
                Some(InterpretationProgress::Completed(Interpretation::Complete(received))) => {
                    settled.push(Interpretation::Complete(received))
                }
                Some(InterpretationProgress::Completed(Interpretation::Corrupt(received))) => {
                    settled.push(Interpretation::Corrupt(received));
                }
                request => {
                    *progress = Some(InterpretationProgress::Interpreting(
                        settled
                            .into_iter()
                            .map(|received| Some(InterpretationProgress::Completed(received)))
                            .chain(core::iter::once(request))
                            .chain(remaining)
                            .collect(),
                    ));
                    return;
                }
            }
        }
        let disposition = settled.iter().fold(
            Interpretation::Complete(()),
            |disposition, received| match (disposition, received) {
                (Interpretation::Corrupt(()), _) | (_, Interpretation::Corrupt(_)) => {
                    Interpretation::Corrupt(())
                }
                (Interpretation::Complete(()), Interpretation::Complete(_)) => {
                    Interpretation::Complete(())
                }
            },
        );
        let requests = settled
            .into_iter()
            .map(Interpretation::into_settlement)
            .collect();
        let received = HeterogeneousShutdownSends { requests };
        *progress = Some(InterpretationProgress::Completed(
            disposition.map(|()| received),
        ));
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        <Self as behavior::SendSettlements>::prepare_interpretation(progress);
        if let Some(InterpretationProgress::Interpreting(requests)) = progress {
            for request in requests {
                T::unattempted(request);
            }
        }
        <Self as behavior::SendSettlements>::finish_interpretation(progress);
    }
}

impl<Host, RootEvent, T> SourceSettlementCustody<Host, RootEvent> for HeterogeneousShutdownSends<T>
where
    T: Send,
{
    type Custody = Self;

    fn prepare_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original)) => Some(SourceProgress::Completed(
                SourceCustody::Exhausted(original),
            )),
            retained => retained,
        };
    }

    fn offer_next_to_source(
        _: &mut Self::Custody,
        _: &mut Host,
    ) -> impl core::future::Future<Output = ()> + Send {
        core::future::ready(())
    }

    fn finish_source(progress: &mut Option<SourceProgress<Self, Self::Custody>>) {
        *progress = match progress.take() {
            Some(SourceProgress::Original(original) | SourceProgress::Offering(original)) => Some(
                SourceProgress::Completed(SourceCustody::Exhausted(original)),
            ),
            retained => retained,
        };
    }
}

impl<I, E, Path, T> behavior::InterpretSends<I, E, Path> for HeterogeneousShutdownSends<T>
where
    I: Send,
    T: heterogeneous::InterpretChoice<I, E, Path, ChildHead>,
    T::InterpretationCustody: Send,
{
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = ()> + Send {
        async move {
            <Self as behavior::SendSettlements>::prepare_interpretation(progress);
            let Some(InterpretationProgress::Interpreting(requests)) = progress else {
                return;
            };
            let mut requests = requests.iter_mut();
            while let Some(request) = requests.next() {
                if !matches!(request, Some(InterpretationProgress::Completed(_))) {
                    T::settle(request, interpreter).await;
                    T::finish_interpretation(request);
                }
                match request {
                    Some(InterpretationProgress::Completed(Interpretation::Complete(_))) => {}
                    Some(InterpretationProgress::Completed(Interpretation::Corrupt(_))) => {
                        for untouched in requests {
                            T::unattempted(untouched);
                        }
                        break;
                    }
                    _ => return,
                }
            }
            <Self as behavior::SendSettlements>::finish_interpretation(progress);
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
    /// One plan phase is active and owns its outstanding child IDs.
    Stopping {
        plan: P,
        phase: usize,
        awaiting: Vec<N>,
    },
    /// Every configured phase completed, or the installed plan was empty.
    Completed,
}

enum ShutdownMove<P> {
    None,
    StartPhase { plan: P, phase: usize },
    Stop,
}

/// Install one validated plan into a coordinator that was started without it.
///
/// The plan type is part of the coordinator's event sum. Homogeneous and
/// heterogeneous plans therefore cannot be confused at installation:
///
/// ```compile_fail
/// struct Probe;
/// impl behavior::Protocol for Probe { type Addr = behavior::MailAddr; type Msg = (); }
/// impl behavior::Behavior for Probe {
///     type Protocol = Self;
///     type Event = behavior::User<behavior::MailAddr, ()>;
///     type Sends = Vec<behavior::Never>;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
/// type Targets = behavior_actors::ShutdownChoice<behavior_actors::StopOnShutdown<Probe>, behavior_actors::NoShutdownTargets<behavior::MailAddr>>;
/// let mut coordinator = behavior_actors::Activate::initialize(
///     behavior_actors::ShutdownCoordinator::<Probe, behavior_actors::StopOnShutdown<Probe>, behavior::ChildHead>::awaiting_plan(Probe)
/// ).unwrap().behavior;
/// let heterogeneous = behavior_actors::HeterogeneousShutdownPlan::<Targets>::new([]).unwrap();
/// coordinator.on_path(behavior_actors::InstallShutdownPlan::new(heterogeneous)).unwrap();
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

/// Interpreter request reporting one validated plan to its owning coordinator.
///
/// The request owns the plan. Interpretation enqueues one ordinary event for
/// the same actor incarnation through its source-indexed [`EventIngress`].
pub struct ReportShutdownPlan<P> {
    installation: InstallShutdownPlan<P>,
}

impl<P: core::fmt::Debug> core::fmt::Debug for ReportShutdownPlan<P> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReportShutdownPlan")
            .field("plan", &self.installation.plan)
            .finish()
    }
}

impl<P: PartialEq> PartialEq for ReportShutdownPlan<P> {
    fn eq(&self, other: &Self) -> bool {
        self.installation.plan == other.installation.plan
    }
}

impl<P: Eq> Eq for ReportShutdownPlan<P> {}

impl<P> ReportShutdownPlan<P> {
    #[must_use]
    pub fn new(plan: P) -> Self {
        Self {
            installation: InstallShutdownPlan::new(plan),
        }
    }

    /// Borrow the complete validated plan without changing the request.
    #[must_use]
    pub const fn plan(&self) -> &P {
        &self.installation.plan
    }

    /// Build the exact root event selected for the current actor source.
    #[must_use]
    pub fn into_event<Event>(self) -> Event
    where
        Event: EventIngress<Here, InstallShutdownPlan<P>>,
    {
        Event::ingress(self.installation)
    }
}

impl<P> behavior::InterpreterRequest for ReportShutdownPlan<P> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<P: Send> behavior::ActionItem for ReportShutdownPlan<P> {
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;

    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }

    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        match custody {
            (input @ Some(_), received @ None) => Some((input, received)),
            _ => None,
        }
    }

    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = ();
    type Rejection = behavior::Never;
    type Prerequisite = behavior::Never;
}

/// Event sum accepted by [`ShutdownCoordinator`].
pub enum ShutdownCoordinatorEvent<E: UserEvent, P> {
    Behavior(E),
    Plan(InstallShutdownPlan<P>),
    Requested(crate::ShutdownRequested),
    ChildStopped(ChildStopped<E::Addr>),
    ChildRejected(ChildShutdownRejected),
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

impl<E: UserEvent, P> EventIngress<Here, InstallShutdownPlan<P>>
    for ShutdownCoordinatorEvent<E, P>
{
    fn ingress(value: InstallShutdownPlan<P>) -> Self {
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
impl<E: UserEvent, P> InjectEvent<ChildShutdownRejected, Here> for ShutdownCoordinatorEvent<E, P> {
    fn inject_at(value: ChildShutdownRejected) -> Self {
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
pub enum ShutdownCoordinatorError<E, A: Address, P> {
    #[error("wrapped behavior rejected its transition")]
    Behavior(#[source] E),
    #[error("child shutdown was rejected")]
    ChildRejected {
        child: CreationId,
        reason: ChildShutdownRejection,
    },
    /// A child-stop fact did not name a child awaited by the current phase.
    #[error("child-stop fact does not belong to the active shutdown phase")]
    UnexpectedChildStopped(ChildStopped<A>),
    /// A child-shutdown rejection did not name a child awaited by the current phase.
    #[error("child-shutdown rejection does not belong to the active shutdown phase")]
    UnexpectedChildRejection {
        child: CreationId,
        reason: ChildShutdownRejection,
    },
    /// A plan was supplied after one had already been installed or completed.
    #[error("shutdown plan was already installed")]
    PlanAlreadyInstalled(P),
}

/// Pure phased shutdown wrapper over an explicitly validated homogeneous child
/// topology.
///
/// `B` is the wrapped coordinator behavior and `C` is the one concrete child
/// protocol selected by every creation ID in the plan. Starting a phase emits one
/// typed [`ShutdownChild<C, Occurrence>`] request per member in plan order. A phase advances
/// only after every matching [`ChildStopped`] fact arrives. An acceptance
/// rejection returns [`ShutdownCoordinatorError::ChildRejected`] without
/// changing the phase. A stale or foreign child fact is returned intact as a
/// typed error. This is Bombay lifecycle policy, not an actor-model allocation
/// or ordering guarantee.
///
/// The fold introduces no panic conditions.
///
/// A child protocol without the shutdown input cannot form an executable
/// coordinator:
///
/// ```compile_fail
///
/// struct Plain;
/// impl behavior::Protocol for Plain {
///     type Addr = behavior::MailAddr;
///     type Msg = ();
/// }
/// impl behavior::Behavior for Plain {
///     type Protocol = Self;
///     type Event = behavior::User<behavior::MailAddr, ()>;
///     type Sends = Vec<behavior::Never>;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///         Ok(behavior::Actions::cont())
///     }
/// }
///
/// fn require_behavior<B: behavior::Behavior>(_: B) {}
/// let child = behavior::CreationSequence::new()
///     .issue()
///     .expect("the first child creation ID exists");
/// let plan = behavior_actors::ShutdownPlan::new([vec![child]]).unwrap();
/// require_behavior(behavior_actors::ShutdownCoordinator::<Plain, Plain, behavior::ChildHead>::new(Plain, plan));
/// ```
pub struct ShutdownCoordinator<B: Behavior, C: Behavior, Occurrence>
where
    C::Protocol: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
{
    inner: B,
    state: ShutdownState<ShutdownPlan<CreationId>, CreationId>,
    child: core::marker::PhantomData<fn() -> (C, Occurrence)>,
}

type ShutdownCoordinatorActions<B, C, Occurrence> = Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<ShutdownChild<C, Occurrence>>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior, C: Behavior, Occurrence> ShutdownCoordinator<B, C, Occurrence>
where
    C::Protocol: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
{
    #[must_use]
    pub const fn new(inner: B, plan: ShutdownPlan<CreationId>) -> Self {
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
    pub fn state(&self) -> &ShutdownState<ShutdownPlan<CreationId>, CreationId> {
        &self.state
    }

    fn wrap(
        actions: Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        actions.map_sends(|inner| SendLayer::new(InterpreterRequests::empty(), inner))
    }

    fn phase_actions(
        plan: &ShutdownPlan<CreationId>,
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
        plan: ShutdownPlan<CreationId>,
    ) -> ShutdownMove<ShutdownPlan<CreationId>> {
        if plan.phases.is_empty() {
            self.state = ShutdownState::Completed;
            return ShutdownMove::Stop;
        }
        let awaiting = plan.phases[0].clone();
        let selected = plan.clone();
        self.state = ShutdownState::Stopping {
            plan,
            phase: 0,
            awaiting,
        };
        ShutdownMove::StartPhase {
            plan: selected,
            phase: 0,
        }
    }

    fn install_plan(
        &mut self,
        plan: ShutdownPlan<CreationId>,
    ) -> Result<ShutdownMove<ShutdownPlan<CreationId>>, ShutdownPlan<CreationId>> {
        match self.state {
            ShutdownState::AwaitingPlan => {
                self.state = ShutdownState::Ready { plan };
                Ok(ShutdownMove::None)
            }
            ShutdownState::AwaitingPlanAfterShutdown => Ok(self.start_plan(plan)),
            ShutdownState::Ready { .. }
            | ShutdownState::Stopping { .. }
            | ShutdownState::Completed => Err(plan),
        }
    }

    fn request_shutdown(&mut self) -> ShutdownMove<ShutdownPlan<CreationId>> {
        let ready = match &self.state {
            ShutdownState::AwaitingPlan => {
                self.state = ShutdownState::AwaitingPlanAfterShutdown;
                None
            }
            ShutdownState::Ready { plan } => Some(plan.clone()),
            ShutdownState::AwaitingPlanAfterShutdown
            | ShutdownState::Stopping { .. }
            | ShutdownState::Completed => None,
        };
        ready.map_or(ShutdownMove::None, |plan| self.start_plan(plan))
    }

    fn child_stopped(&mut self, child: CreationId) -> ShutdownMove<ShutdownPlan<CreationId>> {
        let ShutdownState::Stopping {
            plan,
            phase,
            awaiting,
        } = &mut self.state
        else {
            return ShutdownMove::None;
        };
        let Some(position) = awaiting.iter().position(|candidate| *candidate == child) else {
            return ShutdownMove::None;
        };
        awaiting.remove(position);
        if !awaiting.is_empty() {
            return ShutdownMove::None;
        }
        let next = *phase + 1;
        if next == plan.phases.len() {
            self.state = ShutdownState::Completed;
            ShutdownMove::Stop
        } else {
            *phase = next;
            *awaiting = plan.phases[next].clone();
            ShutdownMove::StartPhase {
                plan: plan.clone(),
                phase: next,
            }
        }
    }

    fn move_actions(
        next: ShutdownMove<ShutdownPlan<CreationId>>,
    ) -> ShutdownCoordinatorActions<B, C, Occurrence> {
        match next {
            ShutdownMove::None => Actions::cont(),
            ShutdownMove::Stop => Actions::stop(),
            ShutdownMove::StartPhase { plan, phase } => Self::phase_actions(&plan, phase),
        }
    }
}

impl<B, C, Occurrence> behavior::BehaviorBase for ShutdownCoordinator<B, C, Occurrence>
where
    B: Behavior + behavior::BehaviorBase,
    C: Behavior,
    C::Protocol: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
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
    C::Protocol: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, C, Occurrence, A, Ph, S, Br> Behavior for ShutdownCoordinator<B, C, Occurrence>
where
    A: Address,
    S: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = S, Birth = Br>,
    B::Protocol: behavior::Protocol<Addr = A>,
    C: Behavior,
    C::Protocol: behavior::Protocol<Addr = A>,
    C::Event: InjectEvent<crate::ShutdownRequested, Here>,
{
    type Protocol = B::Protocol;
    type Event = ShutdownCoordinatorEvent<B::Event, ShutdownPlan<CreationId>>;
    type Sends = SendLayer<InterpreterRequests<ShutdownChild<C, Occurrence>>, S>;
    type Ph = Ph;
    type Error = ShutdownCoordinatorError<B::Error, A, ShutdownPlan<CreationId>>;
    type Birth = Br;
    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(Self::wrap)
            .map_err(ShutdownCoordinatorError::Behavior)
    }
    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ShutdownCoordinatorEvent::Plan(installation) => {
                let next = self
                    .install_plan(installation.into_plan())
                    .map_err(ShutdownCoordinatorError::PlanAlreadyInstalled)?;
                Ok(Self::move_actions(next))
            }
            ShutdownCoordinatorEvent::Requested(_) => {
                let next = self.request_shutdown();
                Ok(Self::move_actions(next))
            }
            ShutdownCoordinatorEvent::ChildStopped(stopped) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&stopped.child));
                if !matching {
                    return Err(ShutdownCoordinatorError::UnexpectedChildStopped(stopped));
                }
                let next = self.child_stopped(stopped.child);
                Ok(Self::move_actions(next))
            }
            ShutdownCoordinatorEvent::ChildRejected(rejected) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&rejected.child));
                if matching {
                    Err(ShutdownCoordinatorError::ChildRejected {
                        child: rejected.child,
                        reason: rejected.reason,
                    })
                } else {
                    Err(ShutdownCoordinatorError::UnexpectedChildRejection {
                        child: rejected.child,
                        reason: rejected.reason,
                    })
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
/// creator-local creation ID because the child namespace is globally unique. This
/// phased ordering is Bombay policy, not an actor-model guarantee.
pub struct HeterogeneousShutdownCoordinator<B: Behavior, T>
where
    T: heterogeneous::Selection<Addr = behavior::BehaviorAddr<B>>,
{
    inner: B,
    state: ShutdownState<HeterogeneousShutdownPlan<T>, CreationId>,
}

type HeterogeneousShutdownActions<B, T> = Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<HeterogeneousShutdownSends<T>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior, T> HeterogeneousShutdownCoordinator<B, T>
where
    T: heterogeneous::Selection<Addr = behavior::BehaviorAddr<B>> + Copy,
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
    pub fn state(&self) -> &ShutdownState<HeterogeneousShutdownPlan<T>, CreationId> {
        &self.state
    }

    fn wrap(
        actions: Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
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

    fn phase_creations(plan: &HeterogeneousShutdownPlan<T>, phase: usize) -> Vec<CreationId> {
        plan.phases[phase]
            .iter()
            .map(heterogeneous::Selection::creation)
            .collect()
    }

    fn start_plan(
        &mut self,
        plan: HeterogeneousShutdownPlan<T>,
    ) -> ShutdownMove<HeterogeneousShutdownPlan<T>> {
        if plan.phases.is_empty() {
            self.state = ShutdownState::Completed;
            return ShutdownMove::Stop;
        }
        let awaiting = Self::phase_creations(&plan, 0);
        let selected = plan.clone();
        self.state = ShutdownState::Stopping {
            plan,
            phase: 0,
            awaiting,
        };
        ShutdownMove::StartPhase {
            plan: selected,
            phase: 0,
        }
    }

    fn install_plan(
        &mut self,
        plan: HeterogeneousShutdownPlan<T>,
    ) -> Result<ShutdownMove<HeterogeneousShutdownPlan<T>>, HeterogeneousShutdownPlan<T>> {
        match self.state {
            ShutdownState::AwaitingPlan => {
                self.state = ShutdownState::Ready { plan };
                Ok(ShutdownMove::None)
            }
            ShutdownState::AwaitingPlanAfterShutdown => Ok(self.start_plan(plan)),
            ShutdownState::Ready { .. }
            | ShutdownState::Stopping { .. }
            | ShutdownState::Completed => Err(plan),
        }
    }

    fn request_shutdown(&mut self) -> ShutdownMove<HeterogeneousShutdownPlan<T>> {
        let ready = match &self.state {
            ShutdownState::AwaitingPlan => {
                self.state = ShutdownState::AwaitingPlanAfterShutdown;
                None
            }
            ShutdownState::Ready { plan } => Some(plan.clone()),
            ShutdownState::AwaitingPlanAfterShutdown
            | ShutdownState::Stopping { .. }
            | ShutdownState::Completed => None,
        };
        ready.map_or(ShutdownMove::None, |plan| self.start_plan(plan))
    }

    fn child_stopped(&mut self, child: CreationId) -> ShutdownMove<HeterogeneousShutdownPlan<T>> {
        let ShutdownState::Stopping {
            plan,
            phase,
            awaiting,
        } = &mut self.state
        else {
            return ShutdownMove::None;
        };
        let Some(position) = awaiting.iter().position(|candidate| *candidate == child) else {
            return ShutdownMove::None;
        };
        awaiting.remove(position);
        if !awaiting.is_empty() {
            return ShutdownMove::None;
        }
        let next = *phase + 1;
        if next == plan.phases.len() {
            self.state = ShutdownState::Completed;
            ShutdownMove::Stop
        } else {
            *phase = next;
            *awaiting = Self::phase_creations(plan, next);
            ShutdownMove::StartPhase {
                plan: plan.clone(),
                phase: next,
            }
        }
    }

    fn move_actions(
        next: ShutdownMove<HeterogeneousShutdownPlan<T>>,
    ) -> HeterogeneousShutdownActions<B, T> {
        match next {
            ShutdownMove::None => Actions::cont(),
            ShutdownMove::Stop => Actions::stop(),
            ShutdownMove::StartPhase { plan, phase } => Self::phase_actions(&plan, phase),
        }
    }
}

impl<B, T> behavior::BehaviorBase for HeterogeneousShutdownCoordinator<B, T>
where
    B: Behavior + behavior::BehaviorBase,
    T: heterogeneous::Selection<Addr = behavior::BehaviorAddr<B>>,
{
    type Base = B::Base;
    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, T> crate::StashStatus for HeterogeneousShutdownCoordinator<B, T>
where
    B: Behavior + crate::StashStatus,
    T: heterogeneous::Selection<Addr = behavior::BehaviorAddr<B>>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, T, A, Ph, Sends, Br> Behavior for HeterogeneousShutdownCoordinator<B, T>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: behavior::Protocol<Addr = A>,
    T: heterogeneous::Selection<Addr = A> + Copy,
{
    type Protocol = B::Protocol;
    type Event = ShutdownCoordinatorEvent<B::Event, HeterogeneousShutdownPlan<T>>;
    type Sends = SendLayer<HeterogeneousShutdownSends<T>, Sends>;
    type Ph = Ph;
    type Error = ShutdownCoordinatorError<B::Error, A, HeterogeneousShutdownPlan<T>>;
    type Birth = Br;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(Self::wrap)
            .map_err(ShutdownCoordinatorError::Behavior)
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ShutdownCoordinatorEvent::Plan(installation) => {
                let next = self
                    .install_plan(installation.into_plan())
                    .map_err(ShutdownCoordinatorError::PlanAlreadyInstalled)?;
                Ok(Self::move_actions(next))
            }
            ShutdownCoordinatorEvent::Requested(_) => {
                let next = self.request_shutdown();
                Ok(Self::move_actions(next))
            }
            ShutdownCoordinatorEvent::ChildStopped(stopped) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&stopped.child));
                if !matching {
                    return Err(ShutdownCoordinatorError::UnexpectedChildStopped(stopped));
                }
                let next = self.child_stopped(stopped.child);
                Ok(Self::move_actions(next))
            }
            ShutdownCoordinatorEvent::ChildRejected(rejected) => {
                let matching = matches!(&self.state, ShutdownState::Stopping { awaiting, .. } if awaiting.contains(&rejected.child));
                if matching {
                    Err(ShutdownCoordinatorError::ChildRejected {
                        child: rejected.child,
                        reason: rejected.reason,
                    })
                } else {
                    Err(ShutdownCoordinatorError::UnexpectedChildRejection {
                        child: rejected.child,
                        reason: rejected.reason,
                    })
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

/// Homogeneous dependency-ordered shutdown over one concrete child protocol.
#[cfg(test)]
mod tests {
    use core::future::Future;
    use std::time::Instant;

    use super::heterogeneous::Selection;
    use super::*;
    use crate::Activate as _;
    use crate::{Exit, ShutdownRequested};
    use behavior::{CreationSequence, MailAddr, Never, NoBirths, NoSends, Step};

    struct Probe;

    impl behavior::BehaviorBase for Probe {
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
        fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![1]))
        }
        fn transition(
            &mut self,
            _: behavior::ActiveTurn,
            event: Self::Event,
        ) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    struct NamedParent;

    #[behavior::behavior(
        addr = MailAddr,
        message = Never,
        births = {
            primary: crate::StopOnShutdown<Probe>,
            pool: crate::StopOnShutdown<Probe>,
            fallback: crate::StopOnShutdown<Probe>,
        },
        creation_settlements = retain_for_retirement,
    )]
    impl NamedParent {
        fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
            match message {}
        }
    }

    #[derive(Clone, Copy)]
    struct ApplicationCreations {
        primary: CreationId,
        pool: CreationId,
        fallback: CreationId,
        unrelated: CreationId,
    }

    impl ApplicationCreations {
        fn issue() -> Self {
            let mut sequence = CreationSequence::new();
            let primary = sequence.issue().expect("the primary creation ID exists");
            let pool = sequence.issue().expect("the pool creation ID exists");
            let fallback = sequence.issue().expect("the fallback creation ID exists");
            let unrelated = sequence.issue().expect("the unrelated creation ID exists");
            Self {
                primary,
                pool,
                fallback,
                unrelated,
            }
        }
    }

    fn stopped(child: CreationId) -> ChildStopped<MailAddr> {
        ChildStopped::new(child, Ok(Exit::Normal), Instant::now())
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
        let plan = tree.into_plan();
        assert_eq!(plan.phases(), &[vec![1, 2], vec![3], vec![4]]);
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
        let children = ApplicationCreations::issue();
        let plan = ShutdownPlan::new([
            vec![children.primary, children.pool],
            vec![children.fallback],
        ])
        .unwrap();
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
            [
                ShutdownChild::new(children.primary),
                ShutdownChild::new(children.pool),
            ]
        );
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[children.primary, children.pool]
        ));

        let one = active.on_path(stopped(children.pool)).unwrap();
        assert_eq!(one.sends, SendLayer::empty());
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[children.primary]
        ));

        let second = active.on_path(stopped(children.primary)).unwrap();
        assert_eq!(
            second.sends.owned.as_slice(),
            [ShutdownChild::new(children.fallback)]
        );
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 1,
                awaiting,
                ..
            } if awaiting == &[children.fallback]
        ));

        let complete = active.on_path(stopped(children.fallback)).unwrap();
        assert!(matches!(complete.become_, Step::Stop(_)));
        assert_eq!(active.state(), &ShutdownState::Completed);
    }

    #[test]
    fn heterogeneous_phases_preserve_cross_protocol_order_and_await_the_union() {
        type SupervisorChild = crate::StopOnShutdown<Probe>;
        type PoolChild = crate::StopOnShutdown<Probe>;
        type RootTargets = ShutdownChoice<
            SupervisorChild,
            ShutdownChoice<PoolChild, ShutdownChoice<SupervisorChild, NoShutdownTargets<MailAddr>>>,
        >;
        let children = ApplicationCreations::issue();
        let primary = shutdown_target::<NamedParent, _, RootTargets>(
            NamedParentChild::Primary,
            children.primary,
        );
        let pool =
            shutdown_target::<NamedParent, _, RootTargets>(NamedParentChild::Pool, children.pool);
        let fallback = shutdown_target::<NamedParent, _, RootTargets>(
            NamedParentChild::Fallback,
            children.fallback,
        );

        assert!(matches!(
            primary,
            ShutdownChoice::Other(ShutdownChoice::Other(ShutdownChoice::Child {
                creation,
                ..
            })) if creation == children.primary
        ));
        assert!(matches!(
            pool,
            ShutdownChoice::Other(ShutdownChoice::Child { creation, .. })
                if creation == children.pool
        ));
        assert!(matches!(
            fallback,
            ShutdownChoice::Child { creation, .. } if creation == children.fallback
        ));

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
                .map(|target| target.creation())
                .collect::<Vec<_>>(),
            [children.primary, children.pool]
        );
        assert!(matches!(first.sends.inner, NoSends));
        assert!(first.creates.is_empty());
        assert!(matches!(first.become_, Step::Continue));
        let retained = active.on_path(stopped(children.pool)).unwrap();
        assert!(retained.sends.owned.requests.is_empty());
        assert!(matches!(retained.sends.inner, NoSends));
        assert!(retained.creates.is_empty());
        assert!(matches!(retained.become_, Step::Continue));
        let second = active.on_path(stopped(children.primary)).unwrap();
        assert_eq!(second.sends.owned.requests[0].creation(), children.fallback);
        assert!(matches!(second.sends.inner, NoSends));
        assert!(second.creates.is_empty());
        assert!(matches!(second.become_, Step::Continue));
        let completed = active.on_path(stopped(children.fallback)).unwrap();
        assert!(completed.sends.owned.requests.is_empty());
        assert!(matches!(completed.sends.inner, NoSends));
        assert!(completed.creates.is_empty());
        assert!(matches!(completed.become_, Step::Stop(_)));
    }

    #[test]
    fn heterogeneous_plan_rejects_cross_protocol_creation_collisions() {
        type SupervisorChild = crate::StopOnShutdown<Probe>;
        type PoolChild = crate::StopOnShutdown<Probe>;
        type RootTargets = ShutdownChoice<
            SupervisorChild,
            ShutdownChoice<PoolChild, ShutdownChoice<SupervisorChild, NoShutdownTargets<MailAddr>>>,
        >;
        let children = ApplicationCreations::issue();
        assert!(matches!(
            HeterogeneousShutdownPlan::new([vec![
                shutdown_target::<NamedParent, _, RootTargets>(
                    NamedParentChild::Primary,
                    children.primary,
                ),
                shutdown_target::<NamedParent, _, RootTargets>(
                    NamedParentChild::Pool,
                    children.primary,
                ),
            ]]),
            Err(ShutdownPlanError::DuplicateChild(creation)) if creation == children.primary
        ));
    }

    #[tokio::test]
    async fn heterogeneous_requests_interpret_once_each_in_cross_protocol_plan_order() {
        type PrimaryWorker = crate::StopOnShutdown<Probe>;
        type PoolWorker = crate::StopOnShutdown<Probe>;
        type Targets = ShutdownChoice<
            PrimaryWorker,
            ShutdownChoice<PoolWorker, ShutdownChoice<PrimaryWorker, NoShutdownTargets<MailAddr>>>,
        >;
        type Event =
            ShutdownCoordinatorEvent<User<MailAddr, u8>, HeterogeneousShutdownPlan<Targets>>;

        struct Recording(Vec<CreationId>);
        impl behavior::InterpretItem<ShutdownChild<PrimaryWorker, ChildHead>, Event, Here> for Recording {
            fn interpret_item<'a>(
                &'a mut self,
                input: &'a mut Option<ShutdownChild<PrimaryWorker, ChildHead>>,
                received: &'a mut Option<
                    <ShutdownChild<PrimaryWorker, ChildHead> as ActionItem>::Reply,
                >,
            ) -> impl Future<Output = ()> + Send + 'a
            where
                ShutdownChild<PrimaryWorker, ChildHead>: 'a,
            {
                async move {
                    if received.is_some() {
                        return;
                    }
                    let Some(request) = input.take() else {
                        return;
                    };
                    self.0.push(request.child);
                    *received = Some(ItemSettlement::Accepted(()));
                }
            }
        }
        impl behavior::InterpretItem<ShutdownChild<PoolWorker, ChildTail<ChildHead>>, Event, Here>
            for Recording
        {
            fn interpret_item<'a>(
                &'a mut self,
                input: &'a mut Option<ShutdownChild<PoolWorker, ChildTail<ChildHead>>>,
                received: &'a mut Option<
                    <ShutdownChild<PoolWorker, ChildTail<ChildHead>> as ActionItem>::Reply,
                >,
            ) -> impl Future<Output = ()> + Send + 'a
            where
                ShutdownChild<PoolWorker, ChildTail<ChildHead>>: 'a,
            {
                async move {
                    if received.is_some() {
                        return;
                    }
                    let Some(request) = input.take() else {
                        return;
                    };
                    self.0.push(request.child);
                    *received = Some(ItemSettlement::Accepted(()));
                }
            }
        }
        impl
            behavior::InterpretItem<
                ShutdownChild<PrimaryWorker, ChildTail<ChildTail<ChildHead>>>,
                Event,
                Here,
            > for Recording
        {
            fn interpret_item<'a>(
                &'a mut self,
                input: &'a mut Option<
                    ShutdownChild<PrimaryWorker, ChildTail<ChildTail<ChildHead>>>,
                >,
                received: &'a mut Option<<ShutdownChild<PrimaryWorker, ChildTail<ChildTail<ChildHead>>> as ActionItem>::Reply>,
            ) -> impl Future<Output = ()> + Send + 'a
            where
                ShutdownChild<PrimaryWorker, ChildTail<ChildTail<ChildHead>>>: 'a,
            {
                async move {
                    if received.is_some() {
                        return;
                    }
                    let Some(request) = input.take() else {
                        return;
                    };
                    self.0.push(request.child);
                    *received = Some(ItemSettlement::Accepted(()));
                }
            }
        }

        let children = ApplicationCreations::issue();
        let sends = HeterogeneousShutdownSends::<Targets> {
            requests: vec![
                shutdown_target::<NamedParent, _, Targets>(NamedParentChild::Pool, children.pool),
                shutdown_target::<NamedParent, _, Targets>(
                    NamedParentChild::Fallback,
                    children.fallback,
                ),
                shutdown_target::<NamedParent, _, Targets>(
                    NamedParentChild::Primary,
                    children.primary,
                ),
            ],
        };
        let mut interpreter = Recording(Vec::new());
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as behavior::InterpretSends<_, Event, Here>>::interpret(&mut progress, &mut interpreter)
            .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual heterogeneous shutdown plan must finish each whole request");
        };
        assert!(matches!(settlement, behavior::Interpretation::Complete(_)));
        assert_eq!(
            interpreter.0,
            [children.pool, children.fallback, children.primary]
        );
    }

    #[test]
    fn coordinator_routes_shutdown_to_children_before_root_stop() {
        let children = ApplicationCreations::issue();
        let plan = ShutdownPlan::new([vec![children.primary]]).unwrap();
        let initialized =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap();
        let mut active = initialized.behavior;

        let actions = active.on(ShutdownRequested).unwrap();

        assert_eq!(
            actions.sends.owned.as_slice(),
            [ShutdownChild::new(children.primary)]
        );
        assert!(actions.sends.inner.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }

    #[test]
    fn stale_child_stops_are_returned_while_repeated_shutdown_is_idempotent() {
        let children = ApplicationCreations::issue();
        let plan = ShutdownPlan::new([vec![children.primary, children.pool]]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        let before_start = stopped(children.unrelated);
        assert_eq!(
            active.on_path(before_start),
            Err(ShutdownCoordinatorError::UnexpectedChildStopped(
                before_start
            ))
        );
        let started = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(started.sends.owned.as_slice().len(), 2);
        assert!(started.sends.inner.is_empty());
        assert!(started.creates.is_empty());
        assert!(matches!(started.become_, Step::Continue));
        let repeated = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(repeated.sends, SendLayer::empty());
        assert!(repeated.creates.is_empty());
        assert!(matches!(repeated.become_, Step::Continue));
        let retained = active.on_path(stopped(children.primary)).unwrap();
        assert_eq!(retained.sends, SendLayer::empty());
        assert!(retained.creates.is_empty());
        assert!(matches!(retained.become_, Step::Continue));
        let duplicate = stopped(children.primary);
        assert_eq!(
            active.on_path(duplicate),
            Err(ShutdownCoordinatorError::UnexpectedChildStopped(duplicate))
        );
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[children.pool]
        ));
    }

    #[test]
    fn matching_rejection_is_typed_and_does_not_mutate_phase() {
        let children = ApplicationCreations::issue();
        let plan = ShutdownPlan::new([vec![children.primary]]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        let started = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(
            started.sends.owned.as_slice(),
            [ShutdownChild::new(children.primary)]
        );
        assert!(started.sends.inner.is_empty());
        assert!(started.creates.is_empty());
        assert!(matches!(started.become_, Step::Continue));
        let before = active.state().clone();
        assert_eq!(
            active.on_path(ChildShutdownRejected::new(
                children.primary,
                ChildShutdownRejection::NotEstablished
            )),
            Err(ShutdownCoordinatorError::ChildRejected {
                child: children.primary,
                reason: ChildShutdownRejection::NotEstablished
            })
        );
        assert_eq!(active.state(), &before);
    }

    #[test]
    fn empty_plan_stops_immediately_and_user_actions_preserve_named_lanes() {
        let plan = ShutdownPlan::<CreationId>::new([]).unwrap();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::new(Probe, plan)
                .initialize()
                .unwrap()
                .behavior;
        let user = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(user.sends.inner, [7]);
        assert!(user.sends.owned.is_empty());
        assert!(user.creates.is_empty());
        assert!(matches!(user.become_, Step::Continue));
        let stopped = active.on_path(ShutdownRequested).unwrap();
        assert!(stopped.sends.owned.is_empty());
        assert!(stopped.sends.inner.is_empty());
        assert!(stopped.creates.is_empty());
        assert!(matches!(stopped.become_, Step::Stop(_)));
    }

    #[test]
    fn homogeneous_plan_acceptance_is_an_explicit_one_way_lifecycle() {
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        assert!(matches!(active.state(), ShutdownState::AwaitingPlan));

        let children = ApplicationCreations::issue();
        let plan = ShutdownPlan::new([
            vec![children.primary, children.pool],
            vec![children.fallback],
        ])
        .unwrap();
        let installed = active
            .on_path(InstallShutdownPlan::new(plan.clone()))
            .unwrap();
        assert_eq!(installed.sends, SendLayer::empty());
        assert!(matches!(
            active.state(),
            ShutdownState::Ready { plan: installed } if installed == &plan
        ));
        assert_eq!(
            active
                .on_path(InstallShutdownPlan::new(plan.clone()))
                .unwrap_err(),
            ShutdownCoordinatorError::PlanAlreadyInstalled(plan)
        );
    }

    #[test]
    fn topology_owner_reports_a_plan_as_an_ordinary_typed_event() {
        type Event = ShutdownCoordinatorEvent<User<MailAddr, u8>, ShutdownPlan<CreationId>>;

        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        let children = ApplicationCreations::issue();
        let plan = ShutdownPlan::new([vec![children.primary, children.pool]]).unwrap();
        let report: ReportShutdownPlan<_> = ReportShutdownPlan::new(plan.clone());
        let event: Event = report.into_event();

        let installed = active.transition(event).unwrap();

        assert_eq!(installed.sends, SendLayer::empty());
        assert!(matches!(
            active.state(),
            ShutdownState::Ready { plan: installed } if installed == &plan
        ));
    }

    #[test]
    fn shutdown_before_homogeneous_plan_is_retained_and_empty_plan_stops() {
        let children = ApplicationCreations::issue();
        let mut active =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        let waiting = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(waiting.sends, SendLayer::empty());
        assert!(waiting.creates.is_empty());
        assert!(matches!(waiting.become_, Step::Continue));
        assert!(matches!(
            active.state(),
            ShutdownState::AwaitingPlanAfterShutdown
        ));
        let started = active
            .on_path(InstallShutdownPlan::new(
                ShutdownPlan::new([vec![children.primary, children.pool]]).unwrap(),
            ))
            .unwrap();
        assert_eq!(
            started.sends.owned.as_slice(),
            [
                ShutdownChild::new(children.primary),
                ShutdownChild::new(children.pool),
            ]
        );
        assert!(started.sends.inner.is_empty());
        assert!(started.creates.is_empty());
        assert!(matches!(started.become_, Step::Continue));

        let mut empty =
            ShutdownCoordinator::<Probe, crate::StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
                Probe,
            )
            .initialize()
            .unwrap()
            .behavior;
        let waiting = empty.on_path(ShutdownRequested).unwrap();
        assert_eq!(waiting.sends, SendLayer::empty());
        assert!(waiting.creates.is_empty());
        assert!(matches!(waiting.become_, Step::Continue));
        let stopped = empty
            .on_path(InstallShutdownPlan::new(ShutdownPlan::new([]).unwrap()))
            .unwrap();
        assert!(matches!(stopped.become_, Step::Stop(_)));
        assert_eq!(empty.state(), &ShutdownState::Completed);
    }

    #[test]
    fn heterogeneous_plan_can_be_installed_after_exact_children_are_selected() {
        type PrimaryWorker = crate::StopOnShutdown<Probe>;
        type PoolWorker = crate::StopOnShutdown<Probe>;
        type Targets = ShutdownChoice<
            PrimaryWorker,
            ShutdownChoice<PoolWorker, ShutdownChoice<PrimaryWorker, NoShutdownTargets<MailAddr>>>,
        >;

        let mut active =
            HeterogeneousShutdownCoordinator::<NamedParent, Targets>::awaiting_plan(NamedParent)
                .initialize()
                .unwrap()
                .behavior;
        let waiting = active.on_path(ShutdownRequested).unwrap();
        assert!(waiting.sends.owned.requests.is_empty());
        assert!(matches!(waiting.sends.inner, NoSends));
        assert!(waiting.creates.is_empty());
        assert!(matches!(waiting.become_, Step::Continue));
        let children = ApplicationCreations::issue();
        let plan = HeterogeneousShutdownPlan::new([vec![
            shutdown_target::<NamedParent, _, Targets>(NamedParentChild::Pool, children.pool),
            shutdown_target::<NamedParent, _, Targets>(NamedParentChild::Primary, children.primary),
        ]])
        .unwrap();
        let started = active.on_path(InstallShutdownPlan::new(plan)).unwrap();
        assert_eq!(
            started
                .sends
                .owned
                .requests
                .iter()
                .map(|target| target.creation())
                .collect::<Vec<_>>(),
            [children.pool, children.primary]
        );
        assert!(matches!(started.sends.inner, NoSends));
        assert!(started.creates.is_empty());
        assert!(matches!(started.become_, Step::Continue));
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping {
                phase: 0,
                awaiting,
                ..
            } if awaiting == &[children.pool, children.primary]
        ));
    }
}
