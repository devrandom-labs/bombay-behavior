//! Exact terminal-outcome propagation from a statically selected actor.

use crate::{
    ChildStopped, ObserveChild, ObservePeer, PeerStopped, ReportTerminalOutcome, TerminalOutcome,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EventLayer, Here, InterpretSends,
    InterpreterRequest, InterpreterRequests, ReturnsToEmitter, SendEffects, SendInterpreter,
    SendLayer,
};

/// A statically selected source of one authoritative terminal fact.
///
/// Implementations identify both the interpreter request and the exact fact
/// returned to this composition.  This keeps owned-child and known-peer
/// selection concrete without a runtime target enum or topology lookup.
pub trait TerminationTarget<A: Address>: Copy {
    type Fact;
    type Request: InterpreterRequest<ReturnToEmitter = ReturnsToEmitter<Self::Fact, Here>>;

    fn request(self) -> Self::Request;
    fn matches(self, fact: &Self::Fact) -> bool;
    fn outcome(fact: Self::Fact) -> TerminalOutcome<A>;
}

/// Select one exact generation from the emitting actor's child namespace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildTermination<A: Address> {
    pub nonce: A::Nonce,
}

impl<A: Address> ChildTermination<A> {
    #[must_use]
    pub const fn new(nonce: A::Nonce) -> Self {
        Self { nonce }
    }
}

impl<A> TerminationTarget<A> for ChildTermination<A>
where
    A: Address,
    A::Nonce: Copy + Eq,
{
    type Fact = ChildStopped<A>;
    type Request = ObserveChild<A>;

    fn request(self) -> Self::Request {
        ObserveChild::new(self.nonce)
    }

    fn matches(self, fact: &Self::Fact) -> bool {
        fact.nonce == self.nonce
    }

    fn outcome(fact: Self::Fact) -> TerminalOutcome<A> {
        fact.outcome
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
    type Fact = PeerStopped<A>;
    type Request = ObservePeer<A>;

    fn request(self) -> Self::Request {
        ObservePeer::new(self.peer)
    }

    fn matches(self, fact: &Self::Fact) -> bool {
        fact.peer == self.peer
    }

    fn outcome(fact: Self::Fact) -> TerminalOutcome<A> {
        fact.outcome
    }
}

/// Explicit disposition of an accepted authoritative terminal fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalDisposition {
    /// The composition's documented policy consumes the fact locally.
    Discharge,
    /// Publish the fact unchanged as this actor's terminal outcome and stop.
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

/// Named effects owned by [`PropagateTermination`].
pub struct TerminalPropagationSends<A, Request>
where
    A: Address,
{
    pub observations: InterpreterRequests<Request>,
    pub reports: InterpreterRequests<ReportTerminalOutcome<A>>,
}

impl<A: Address, Request> SendEffects for TerminalPropagationSends<A, Request> {
    fn empty() -> Self {
        Self {
            observations: InterpreterRequests::empty(),
            reports: InterpreterRequests::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.observations.append(other.observations);
        self.reports.append(other.reports);
    }
}

impl<A, Request, Event> behavior::SendsFor<Event> for TerminalPropagationSends<A, Request>
where
    A: Address,
    InterpreterRequests<Request>: behavior::SendsFor<Event>,
    InterpreterRequests<ReportTerminalOutcome<A>>: behavior::SendsFor<Event>,
{
}

impl<Interpreter, RootEvent, Path, A, Request> InterpretSends<Interpreter, RootEvent, Path>
    for TerminalPropagationSends<A, Request>
where
    Interpreter: SendInterpreter + Send,
    A: Address,
    Request: Send,
    InterpreterRequests<Request>: InterpretSends<Interpreter, RootEvent, Path>,
    InterpreterRequests<ReportTerminalOutcome<A>>: InterpretSends<Interpreter, RootEvent, Path>,
    TerminalPropagationSends<A, Request>: Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            self.observations.interpret(interpreter).await?;
            self.reports.interpret(interpreter).await
        }
    }
}

/// Propagate a selected actor's exact terminal fact through this actor's
/// terminal boundary.
///
/// The target determines which authoritative observation is installed.  A
/// matching fact is accepted once.  The configured policy either discharges
/// it locally or emits [`ReportTerminalOutcome`] unchanged and stops in the
/// same [`Actions`] value.  Publication-before-stop is interpreter policy;
/// the actor-model nucleus does not define lifecycle observation or failure
/// escalation.
///
/// The target's address family is part of the static contract:
///
/// ```compile_fail
/// use bombay_behavior_actors::{ChildTermination, MailAddr};
/// let _ = ChildTermination::<MailAddr>::new("not a MailAddr child nonce");
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
        TerminalPropagationSends<
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
        owned: TerminalPropagationSends<crate::BehaviorAddr<B>, Target::Request>,
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
    Target::Fact: Send,
    Target::Request: Send,
{
    type Protocol = B::Protocol;
    type Event = EventLayer<Target::Fact, B::Event>;
    type Sends = SendLayer<TerminalPropagationSends<A, Target::Request>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        let mut owned = TerminalPropagationSends::empty();
        owned.observations.send(self.target.request());
        Ok(Self::wrap(actions, owned))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(fact)
                if self.state == TerminalPropagationState::Observing
                    && self.target.matches(&fact) =>
            {
                let outcome = Target::outcome(fact);
                match (self.policy)(&outcome) {
                    TerminalDisposition::Discharge => {
                        self.state = TerminalPropagationState::Discharged;
                        Ok(Actions::cont())
                    }
                    TerminalDisposition::Propagate => {
                        self.state = TerminalPropagationState::Propagated;
                        let mut owned = TerminalPropagationSends::empty();
                        owned.reports.send(ReportTerminalOutcome::new(outcome));
                        Ok(Actions::new(
                            SendLayer::new(owned, Sends::empty()),
                            Vec::new(),
                            crate::Step::Stop(behavior::Stopped),
                        ))
                    }
                }
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, TerminalPropagationSends::empty())),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::{Activate as _, Crash, Exit, RestartDenial, SupervisionFailureReason};
    use behavior::{Births, Create, MailAddr, Never, Step, User};
    use proptest::prelude::*;

    struct Probe;

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
        type Birth = Births<u8>;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::new(
                vec![1],
                vec![Create::birth(7, 9)],
                Step::Continue,
            ))
        }

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    fn child(
        policy: TerminalPropagationPolicy<MailAddr>,
    ) -> PropagateTermination<Probe, ChildTermination<MailAddr>> {
        PropagateTermination::new(Probe, ChildTermination::new(7), policy)
    }

    #[test]
    fn initialization_preserves_inner_effects_and_observes_the_exact_child() {
        let initialized = child(propagate_all).initialize().unwrap();

        assert_eq!(initialized.actions.sends.inner, [1]);
        assert_eq!(initialized.actions.creates, [Create::birth(7, 9)]);
        assert!(matches!(initialized.actions.become_, Step::Continue));
        assert_eq!(
            initialized.actions.sends.owned.observations.as_slice(),
            [ObserveChild::new(7)]
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
            Err(Crash::Failed),
            Err(Crash::EnvironmentFailed),
            Err(Crash::Panicked),
            Err(Crash::Cancelled),
        ];

        for outcome in outcomes {
            let mut active = child(propagate_all).initialize().unwrap().behavior;
            let actions = active
                .transition(EventLayer::Owned(ChildStopped::new(
                    7,
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

            let duplicate = active
                .transition(EventLayer::Owned(ChildStopped::new(
                    7,
                    outcome,
                    Instant::now(),
                )))
                .unwrap();
            assert!(duplicate.sends.owned.reports.is_empty());
        }
    }

    #[test]
    fn abnormal_policy_explicitly_discharges_normal_outcomes() {
        for outcome in [Ok(Exit::Normal), Ok(Exit::Collected)] {
            let mut active = child(propagate_abnormal).initialize().unwrap().behavior;
            let actions = active
                .transition(EventLayer::Owned(ChildStopped::new(
                    7,
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
    fn unmatched_facts_are_inert_and_inner_events_preserve_inner_actions() {
        let mut active = child(propagate_all).initialize().unwrap().behavior;
        let unrelated = active
            .transition(EventLayer::Owned(ChildStopped::new(
                8,
                Err(Crash::Failed),
                Instant::now(),
            )))
            .unwrap();
        assert!(unrelated.sends.owned.reports.is_empty());
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
        let mut initialized =
            PropagateTermination::new(Probe, PeerTermination::new(MailAddr(4)), propagate_all)
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
            tag in 0_u8..9,
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
                5 => Err(Crash::Failed),
                6 => Err(Crash::EnvironmentFailed),
                7 => Err(Crash::Panicked),
                _ => Err(Crash::Cancelled),
            };
            let mut active = child(propagate_all).initialize().unwrap().behavior;
            let first = active.transition(EventLayer::Owned(ChildStopped::new(
                7,
                outcome,
                Instant::now(),
            ))).unwrap();
            prop_assert_eq!(
                first.sends.owned.reports.as_slice(),
                [ReportTerminalOutcome::new(outcome)]
            );
            let duplicate = active.transition(EventLayer::Owned(ChildStopped::new(
                7,
                outcome,
                Instant::now(),
            ))).unwrap();
            prop_assert!(duplicate.sends.owned.reports.is_empty());
        }
    }
}
