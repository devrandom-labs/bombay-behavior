//! Exact terminal-outcome propagation from a statically selected actor.

use crate::{
    ChildStopped, ObserveChild, ObservePeer, PeerStopped, ReportTerminalOutcome, TerminalOutcome,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, CreationId, EventLayer, Here,
    InterpretSends, InterpreterRequest, InterpreterRequests, Protocol, ReturnsToEmitter,
    SendEffects, SendLayer,
};

/// A statically selected source of one authoritative terminal report.
///
/// Implementations identify both the interpreter request and the exact report
/// returned to this composition.  This keeps owned-child and known-peer
/// selection concrete without a runtime target enum or topology lookup.
pub trait TerminationTarget<A: Address>: Copy {
    type Report;
    type Request: InterpreterRequest<ReturnToEmitter = ReturnsToEmitter<Self::Report, Here>>;

    fn request(self) -> Self::Request;
    fn matches(self, report: &Self::Report) -> bool;
    fn outcome(report: Self::Report) -> TerminalOutcome<A>;
}

/// Select one exact generation from the emitting actor's child namespace.
pub struct ChildTermination<P: Protocol, Occurrence> {
    pub child: CreationId,
    protocol: core::marker::PhantomData<fn() -> P>,
    occurrence: core::marker::PhantomData<fn() -> Occurrence>,
}

impl<P: Protocol, Occurrence> Copy for ChildTermination<P, Occurrence> {}

impl<P: Protocol, Occurrence> Clone for ChildTermination<P, Occurrence> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol, Occurrence> PartialEq for ChildTermination<P, Occurrence> {
    fn eq(&self, other: &Self) -> bool {
        self.child == other.child
    }
}

impl<P: Protocol, Occurrence> Eq for ChildTermination<P, Occurrence> {}

impl<P: Protocol, Occurrence> core::fmt::Debug for ChildTermination<P, Occurrence> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ChildTermination")
            .field("child", &self.child)
            .finish()
    }
}

impl<P: Protocol, Occurrence> ChildTermination<P, Occurrence> {
    #[must_use]
    pub const fn new(child: CreationId) -> Self {
        Self {
            child,
            protocol: core::marker::PhantomData,
            occurrence: core::marker::PhantomData,
        }
    }
}

impl<P, Occurrence> TerminationTarget<P::Addr> for ChildTermination<P, Occurrence>
where
    P: Protocol,
{
    type Report = ChildStopped<P::Addr>;
    type Request = ObserveChild<P, Occurrence>;

    fn request(self) -> Self::Request {
        ObserveChild::new(self.child)
    }

    fn matches(self, report: &Self::Report) -> bool {
        report.child == self.child
    }

    fn outcome(report: Self::Report) -> TerminalOutcome<P::Addr> {
        report.outcome
    }
}

/// Select one exact incarnation at an established peer address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerTermination<A: Address> {
    pub peer: A,
}

impl<A: Address> PeerTermination<A> {
    #[must_use]
    pub const fn new(peer: A) -> Self {
        Self { peer }
    }
}

impl<A> TerminationTarget<A> for PeerTermination<A>
where
    A: Address + Copy + Eq,
{
    type Report = PeerStopped<A>;
    type Request = ObservePeer<A>;

    fn request(self) -> Self::Request {
        ObservePeer::new(self.peer)
    }

    fn matches(self, report: &Self::Report) -> bool {
        report.peer == self.peer
    }

    fn outcome(report: Self::Report) -> TerminalOutcome<A> {
        report.outcome
    }
}

/// Explicit disposition of an accepted authoritative terminal report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalDisposition {
    /// The composition's documented policy consumes the report locally.
    Discharge,
    /// Publish the report unchanged as this actor's terminal outcome and stop.
    Propagate,
}

/// Pure policy deciding whether one exact terminal outcome crosses this
/// composition's terminal boundary.
pub type TerminalPropagationPolicy<A> = fn(&TerminalOutcome<A>) -> TerminalDisposition;

/// Propagate every terminal outcome.
#[must_use]
pub const fn propagate_all<A: Address>(_: &TerminalOutcome<A>) -> TerminalDisposition {
    TerminalDisposition::Propagate
}

/// Propagate crashes and abnormal exits while deliberately discharging normal
/// and collected termination.
#[must_use]
pub const fn propagate_abnormal<A: Address>(outcome: &TerminalOutcome<A>) -> TerminalDisposition {
    match outcome {
        Ok(crate::Exit::Normal | crate::Exit::Collected) => TerminalDisposition::Discharge,
        Ok(crate::Exit::LinkDied(_) | crate::Exit::SupervisionFailed(_)) | Err(_) => {
            TerminalDisposition::Propagate
        }
    }
}

/// Complete phase of one terminal-propagation definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPropagationState {
    Observing,
    Discharged,
    Propagated,
}

/// Exact rejection from terminal propagation.
pub enum TerminationPropagationError<E, Report> {
    /// The wrapped behavior rejected its own event.
    Inner(E),
    /// A returned terminal report does not match the configured source or the
    /// still-observing phase.
    UnexpectedReport {
        state: TerminalPropagationState,
        report: Report,
    },
}

impl<E: core::fmt::Debug, Report> core::fmt::Debug for TerminationPropagationError<E, Report> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Inner(error) => formatter.debug_tuple("Inner").field(error).finish(),
            Self::UnexpectedReport { state, .. } => formatter
                .debug_struct("UnexpectedReport")
                .field("state", state)
                .field("report", &"<retained>")
                .finish(),
        }
    }
}

impl<E, Report> core::fmt::Display for TerminationPropagationError<E, Report> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Inner(_) => formatter.write_str("wrapped behavior rejected its event"),
            Self::UnexpectedReport { .. } => {
                formatter.write_str("terminal report does not match the active propagation source")
            }
        }
    }
}

impl<E, Report> std::error::Error for TerminationPropagationError<E, Report>
where
    E: std::error::Error + 'static,
    Report: 'static,
{
}

/// Named effects owned by [`PropagateTermination`].
pub struct TerminalPropagationSends<Observations, Reports> {
    pub observations: Observations,
    pub reports: Reports,
}

impl<Observations: SendEffects, Reports: SendEffects> SendEffects
    for TerminalPropagationSends<Observations, Reports>
{
    fn empty() -> Self {
        Self {
            observations: Observations::empty(),
            reports: Reports::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.observations.append(other.observations);
        self.reports.append(other.reports);
    }
}

impl<Observations, Reports, Event> behavior::SendsFor<Event>
    for TerminalPropagationSends<Observations, Reports>
where
    Observations: SendEffects + behavior::SendsFor<Event>,
    Reports: SendEffects + behavior::SendsFor<Event>,
{
}

impl<Observations, Reports> behavior::ClassifySettlement
    for TerminalPropagationSends<Observations, Reports>
where
    Observations: behavior::ClassifySettlement,
    Reports: behavior::ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        self.observations
            .settlement_status()
            .combine(self.reports.settlement_status())
    }
}

impl<Observations, Reports> behavior::SendSettlements
    for TerminalPropagationSends<Observations, Reports>
where
    Observations: behavior::SendSettlements,
    Reports: behavior::SendSettlements,
{
    type Settlements = TerminalPropagationSends<Observations::Settlements, Reports::Settlements>;

    fn unattempted(self) -> Self::Settlements {
        TerminalPropagationSends {
            observations: self.observations.unattempted(),
            reports: self.reports.unattempted(),
        }
    }
}

impl<Interpreter, RootEvent, Path, Observations, Reports>
    InterpretSends<Interpreter, RootEvent, Path> for TerminalPropagationSends<Observations, Reports>
where
    Interpreter: Send,
    Observations: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    Reports: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = behavior::Interpretation<Self::Settlements>> + Send
    {
        async move {
            behavior::settle_in_order(self.observations, self.reports, interpreter)
                .await
                .map(|(observations, reports)| TerminalPropagationSends {
                    observations,
                    reports,
                })
        }
    }
}

type PropagationSends<A, Request> = TerminalPropagationSends<
    InterpreterRequests<Request>,
    InterpreterRequests<ReportTerminalOutcome<A>>,
>;

/// Propagate a selected actor's exact terminal report through this actor's
/// terminal boundary.
///
/// The target determines which authoritative observation is installed.  A
/// matching report is accepted once.  The configured policy either discharges
/// it locally or emits [`ReportTerminalOutcome`] unchanged and stops in the
/// same [`Actions`] value.  Publication-before-stop is interpreter policy;
/// the actor-model nucleus does not define lifecycle observation or failure
/// escalation.
///
/// A child selection requires a concrete protocol and an opaque creation ID:
///
/// ```compile_fail
/// use behavior_actors::{ChildTermination, MailAddr, Never, Protocol};
///
/// struct Worker;
/// impl Protocol for Worker {
///     type Addr = MailAddr;
///     type Msg = Never;
/// }
///
/// let _ = ChildTermination::<Worker, behavior::ChildHead>::new("not a creation ID");
/// ```
pub struct PropagateTermination<B: Behavior, Target> {
    inner: B,
    target: Target,
    policy: TerminalPropagationPolicy<crate::BehaviorAddr<B>>,
    state: TerminalPropagationState,
}

type PropagationActions<B, Target> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<
        PropagationSends<
            crate::BehaviorAddr<B>,
            <Target as TerminationTarget<crate::BehaviorAddr<B>>>::Request,
        >,
        <B as Behavior>::Sends,
    >,
    <B as Behavior>::Birth,
>;

impl<B, Target> PropagateTermination<B, Target>
where
    B: Behavior,
    Target: TerminationTarget<crate::BehaviorAddr<B>>,
{
    #[must_use]
    pub const fn new(
        inner: B,
        target: Target,
        policy: TerminalPropagationPolicy<crate::BehaviorAddr<B>>,
    ) -> Self {
        Self {
            inner,
            target,
            policy,
            state: TerminalPropagationState::Observing,
        }
    }

    #[must_use]
    pub const fn state(&self) -> TerminalPropagationState {
        self.state
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        owned: PropagationSends<crate::BehaviorAddr<B>, Target::Request>,
    ) -> PropagationActions<B, Target> {
        actions.map_sends(|inner| SendLayer::new(owned, inner))
    }
}

impl<B, Target> crate::BehaviorBase for PropagateTermination<B, Target>
where
    B: Behavior + crate::BehaviorBase,
    Target: TerminationTarget<crate::BehaviorAddr<B>>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, Target> crate::StashStatus for PropagateTermination<B, Target>
where
    B: Behavior + crate::StashStatus,
    Target: TerminationTarget<crate::BehaviorAddr<B>>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, Target, A, Ph, Sends, Br> Behavior for PropagateTermination<B, Target>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    Target: TerminationTarget<A>,
    Target::Report: Send,
    Target::Request: Send,
{
    type Protocol = B::Protocol;
    type Event = EventLayer<Target::Report, B::Event>;
    type Sends = SendLayer<PropagationSends<A, Target::Request>, Sends>;
    type Ph = Ph;
    type Error = TerminationPropagationError<B::Error, Target::Report>;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions =
            behavior::initialize(&mut self.inner).map_err(TerminationPropagationError::Inner)?;
        let mut owned: PropagationSends<A, Target::Request> = TerminalPropagationSends::empty();
        owned.observations.send(self.target.request());
        Ok(Self::wrap(actions, owned))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(report)
                if self.state == TerminalPropagationState::Observing
                    && self.target.matches(&report) =>
            {
                let outcome = Target::outcome(report);
                match (self.policy)(&outcome) {
                    TerminalDisposition::Discharge => {
                        self.state = TerminalPropagationState::Discharged;
                        Ok(Actions::cont())
                    }
                    TerminalDisposition::Propagate => {
                        self.state = TerminalPropagationState::Propagated;
                        let mut owned: PropagationSends<A, Target::Request> =
                            TerminalPropagationSends::empty();
                        owned.reports.send(ReportTerminalOutcome::new(outcome));
                        Ok(Actions::new(
                            SendLayer::new(owned, Sends::empty()),
                            behavior::Creations::empty(),
                            crate::Step::Stop(behavior::Stopped),
                        ))
                    }
                }
            }
            EventLayer::Owned(report) => Err(TerminationPropagationError::UnexpectedReport {
                state: self.state,
                report,
            }),
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, TerminalPropagationSends::empty()))
                .map_err(TerminationPropagationError::Inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::{
        Activate as _, AllocationRejection, Crash, CreationRejection, Exit, RestartDenial,
        RestartReleaseFailure, SupervisionFailureReason,
    };
    use behavior::{
        Births, CreateChild, CreationSequence, Creations, MailAddr, Never, NoBirths, NoSends, Step,
        User,
    };
    use proptest::prelude::*;

    struct Probe {
        worker: CreationId,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Worker;

    impl behavior::Protocol for Worker {
        type Addr = MailAddr;
        type Msg = Never;
    }

    impl crate::BehaviorBase for Worker {
        type Base = Self;

        fn base(&self) -> &Self::Base {
            self
        }
    }

    impl Behavior for Worker {
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

    impl behavior::Protocol for Probe {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl crate::BehaviorBase for Probe {
        type Base = Self;

        fn base(&self) -> &Self::Base {
            self
        }
    }

    impl Behavior for Probe {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<u8>;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<Worker>;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::new(
                vec![1],
                Creations::one(CreateChild::birth(self.worker, Worker)),
                Step::Continue,
            ))
        }

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    fn worker_creation() -> CreationId {
        CreationSequence::new()
            .issue()
            .expect("the first worker creation ID exists")
    }

    fn child(
        worker: CreationId,
        policy: TerminalPropagationPolicy<MailAddr>,
    ) -> PropagateTermination<Probe, ChildTermination<Worker, behavior::ChildHead>> {
        PropagateTermination::new(Probe { worker }, ChildTermination::new(worker), policy)
    }

    #[test]
    fn initialization_preserves_inner_effects_and_observes_the_exact_child() {
        let worker = worker_creation();
        let initialized = child(worker, propagate_all).initialize().unwrap();

        assert_eq!(initialized.actions.sends.inner, [1]);
        assert_eq!(
            initialized.actions.creates,
            Creations::one(CreateChild::birth(worker, Worker))
        );
        assert!(matches!(initialized.actions.become_, Step::Continue));
        assert_eq!(
            initialized.actions.sends.owned.observations.as_slice(),
            [ObserveChild::new(worker)]
        );
        assert!(initialized.actions.sends.owned.reports.is_empty());
    }

    #[test]
    fn every_terminal_variant_is_propagated_without_reclassification() {
        let outcomes = [
            Ok(Exit::Normal),
            Ok(Exit::Collected),
            Ok(Exit::LinkDied(MailAddr(4))),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildStopped,
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                    restarts_in_window: 2,
                    replacements_requested: 3,
                    maximum_restarts: 4,
                }),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::RestartDenied(RestartDenial::ReleaseRejected(
                    RestartReleaseFailure::DurationOverflow,
                )),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::RestartDenied(RestartDenial::AttemptSequenceExhausted),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::RestartDenied(RestartDenial::TimerGenerationExhausted),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::RestartDenied(RestartDenial::TimerIdentityExhausted),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildCreationRejected(
                    CreationRejection::Allocation(AllocationRejection::Exhausted),
                ),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildCreationRejected(
                    CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed),
                ),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildCreationRejected(
                    CreationRejection::InitializationFailed,
                ),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::StableChildCreationRejected(
                    CreationRejection::EnvironmentFailed,
                ),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::WorkerCreationRejected(CreationRejection::Allocation(
                    AllocationRejection::Exhausted,
                )),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::WorkerCreationRejected(CreationRejection::Allocation(
                    AllocationRejection::AddressAlreadyClaimed,
                )),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::WorkerCreationRejected(
                    CreationRejection::InitializationFailed,
                ),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::WorkerCreationRejected(
                    CreationRejection::EnvironmentFailed,
                ),
            )),
            Ok(Exit::SupervisionFailed(
                SupervisionFailureReason::WorkerFactoryRejected,
            )),
            Err(Crash::Failed),
            Err(Crash::EnvironmentFailed),
            Err(Crash::Panicked),
            Err(Crash::Cancelled),
        ];

        for outcome in outcomes {
            let worker = worker_creation();
            let mut active = child(worker, propagate_all).initialize().unwrap().behavior;
            let actions = active
                .transition(EventLayer::Owned(ChildStopped::new(
                    worker,
                    outcome,
                    Instant::now(),
                )))
                .unwrap();

            assert_eq!(
                actions.sends.owned.reports.as_slice(),
                [ReportTerminalOutcome::new(outcome)]
            );
            assert!(actions.sends.owned.observations.is_empty());
            assert!(actions.sends.inner.is_empty());
            assert!(actions.creates.is_empty());
            assert!(matches!(actions.become_, Step::Stop(_)));
            assert_eq!(active.state(), TerminalPropagationState::Propagated);

            let duplicate = ChildStopped::new(worker, outcome, Instant::now());
            assert!(matches!(
                active.transition(EventLayer::Owned(duplicate)),
                Err(TerminationPropagationError::UnexpectedReport {
                    state: TerminalPropagationState::Propagated,
                    report,
                }) if report == duplicate
            ));
        }
    }

    #[test]
    fn abnormal_policy_explicitly_discharges_normal_outcomes() {
        for outcome in [Ok(Exit::Normal), Ok(Exit::Collected)] {
            let worker = worker_creation();
            let mut active = child(worker, propagate_abnormal)
                .initialize()
                .unwrap()
                .behavior;
            let actions = active
                .transition(EventLayer::Owned(ChildStopped::new(
                    worker,
                    outcome,
                    Instant::now(),
                )))
                .unwrap();

            assert!(actions.sends.owned.reports.is_empty());
            assert!(matches!(actions.become_, Step::Continue));
            assert_eq!(active.state(), TerminalPropagationState::Discharged);
        }
    }

    #[test]
    fn unmatched_reports_are_returned_and_inner_events_preserve_inner_actions() {
        let mut creations = CreationSequence::new();
        let worker = creations.issue().expect("the worker creation ID exists");
        let unrelated_worker = creations
            .issue()
            .expect("the unrelated worker creation ID exists");
        let mut active = child(worker, propagate_all).initialize().unwrap().behavior;
        let unrelated = ChildStopped::new(unrelated_worker, Err(Crash::Failed), Instant::now());
        assert!(matches!(
            active.transition(EventLayer::Owned(unrelated)),
            Err(TerminationPropagationError::UnexpectedReport {
                state: TerminalPropagationState::Observing,
                report,
            }) if report == unrelated
        ));
        assert_eq!(active.state(), TerminalPropagationState::Observing);

        let delegated = active
            .transition(EventLayer::Inner(User::new(MailAddr(3), 6)))
            .unwrap();
        assert_eq!(delegated.sends.inner, [6]);
        assert!(delegated.sends.owned.observations.is_empty());
        assert!(delegated.sends.owned.reports.is_empty());
    }

    #[test]
    fn peer_target_uses_the_same_propagation_law() {
        let mut initialized = PropagateTermination::new(
            Probe {
                worker: worker_creation(),
            },
            PeerTermination::new(MailAddr(4)),
            propagate_all,
        )
        .initialize()
        .unwrap();
        assert_eq!(
            initialized.actions.sends.owned.observations.as_slice(),
            [ObservePeer::new(MailAddr(4))]
        );

        let outcome = Err(Crash::Panicked);
        let actions = initialized
            .behavior
            .transition(EventLayer::Owned(PeerStopped::new(MailAddr(4), outcome)))
            .unwrap();
        assert_eq!(
            actions.sends.owned.reports.as_slice(),
            [ReportTerminalOutcome::new(outcome)]
        );
    }

    proptest! {
        #[test]
        fn arbitrary_terminal_payload_is_conserved_once(
            tag in 0_u8..22,
            peer in any::<u64>(),
            admitted in any::<usize>(),
            requested in any::<usize>(),
            maximum in any::<u32>(),
        ) {
            let outcome = match tag {
                0 => Ok(Exit::Normal),
                1 => Ok(Exit::Collected),
                2 => Ok(Exit::LinkDied(MailAddr(peer))),
                3 => Ok(Exit::SupervisionFailed(
                    SupervisionFailureReason::StableChildStopped,
                )),
                4 => Ok(Exit::SupervisionFailed(
                    SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                        restarts_in_window: admitted,
                        replacements_requested: requested,
                        maximum_restarts: maximum,
                    }),
                )),
                5 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::RestartDenied(RestartDenial::ReleaseRejected(RestartReleaseFailure::DurationOverflow)))),
                6 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::RestartDenied(RestartDenial::AttemptSequenceExhausted))),
                7 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::RestartDenied(RestartDenial::TimerGenerationExhausted))),
                8 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::RestartDenied(RestartDenial::TimerIdentityExhausted))),
                9 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::StableChildCreationRejected(CreationRejection::Allocation(AllocationRejection::Exhausted)))),
                10 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::StableChildCreationRejected(CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed)))),
                11 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::StableChildCreationRejected(CreationRejection::InitializationFailed))),
                12 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::StableChildCreationRejected(CreationRejection::EnvironmentFailed))),
                13 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::WorkerCreationRejected(CreationRejection::Allocation(AllocationRejection::Exhausted)))),
                14 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::WorkerCreationRejected(CreationRejection::Allocation(AllocationRejection::AddressAlreadyClaimed)))),
                15 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::WorkerCreationRejected(CreationRejection::InitializationFailed))),
                16 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::WorkerCreationRejected(CreationRejection::EnvironmentFailed))),
                17 => Ok(Exit::SupervisionFailed(SupervisionFailureReason::WorkerFactoryRejected)),
                18 => Err(Crash::Failed),
                19 => Err(Crash::EnvironmentFailed),
                20 => Err(Crash::Panicked),
                _ => Err(Crash::Cancelled),
            };
            let worker = worker_creation();
            let mut active = child(worker, propagate_all).initialize().unwrap().behavior;
            let first = active.transition(EventLayer::Owned(ChildStopped::new(
                worker,
                outcome,
                Instant::now(),
            ))).unwrap();
            prop_assert_eq!(
                first.sends.owned.reports.as_slice(),
                [ReportTerminalOutcome::new(outcome)]
            );
            let duplicate = ChildStopped::new(
                worker,
                outcome,
                Instant::now(),
            );
            let rejected = matches!(
                active.transition(EventLayer::Owned(duplicate)),
                Err(TerminationPropagationError::UnexpectedReport {
                    state: TerminalPropagationState::Propagated,
                    report,
                }) if report == duplicate
            );
            prop_assert!(rejected);
        }
    }
}
