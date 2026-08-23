//! Action-producing peer termination observation.

use crate::{
    EstablishedObservation, ObservationId, ObservationOperation, ObservationRejection,
    ObserveEstablished, ObservePeer, PeerStopped, WatchEvent,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EndpointAddress, EstablishedRecipient,
    EventLayer, InterpreterRequest, InterpreterRequests, ReturnsToEmitter, SendEffects, SendLayer,
};

/// Pure fold applied to the exact matching terminal fact.
pub type TerminationReaction<B> =
    fn(&mut B, PeerStopped<crate::BehaviorAddr<B>>) -> BehaviorActed<B>;

/// Cleanup specialization of an action-producing terminal reaction.
pub type CleanupReaction<B> = TerminationReaction<B>;

/// Lifecycle-publication specialization of an action-producing terminal reaction.
pub type LifecyclePublication<B> = TerminationReaction<B>;

/// Complete consumption phase of one exact terminal observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationObservation {
    /// An exact observation request was emitted but not yet accepted.
    Requested,
    /// The configured peer's terminal fact has not been accepted.
    Observing,
    /// One matching terminal fact was accepted and cannot be folded again.
    Observed,
    /// The exact observation relationship was cancelled before termination.
    Cancelled,
    /// The exact observation operation was rejected.
    Rejected {
        operation: ObservationOperation,
        reason: ObservationRejection,
    },
}

mod sealed {
    pub trait TerminationObservationTarget<B: behavior::Behavior> {}
}

/// Static observation and reaction policy for [`TerminationMonitorWith`].
pub trait TerminationObservationTarget<B: Behavior>:
    sealed::TerminationObservationTarget<B>
{
    type Fact;
    type Request: InterpreterRequest<ReturnToEmitter = ReturnsToEmitter<Self::Fact, behavior::Here>>;

    fn request(&self) -> Self::Request;
    fn react(
        &mut self,
        inner: &mut B,
        observation: TerminationObservation,
        fact: Self::Fact,
    ) -> Result<
        (
            Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
            TerminationObservation,
        ),
        B::Error,
    >;
}

/// Address-selected legacy termination observation.
pub struct LogicalTerminationTarget<B: Behavior> {
    peer: crate::BehaviorAddr<B>,
    react: TerminationReaction<B>,
}

impl<B: Behavior> sealed::TerminationObservationTarget<B> for LogicalTerminationTarget<B> {}

impl<B: Behavior> TerminationObservationTarget<B> for LogicalTerminationTarget<B> {
    type Fact = PeerStopped<crate::BehaviorAddr<B>>;
    type Request = ObservePeer<crate::BehaviorAddr<B>>;

    fn request(&self) -> Self::Request {
        ObservePeer::new(self.peer)
    }

    fn react(
        &mut self,
        inner: &mut B,
        observation: TerminationObservation,
        fact: Self::Fact,
    ) -> Result<
        (
            Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
            TerminationObservation,
        ),
        B::Error,
    > {
        if observation != TerminationObservation::Observing || fact.peer != self.peer {
            return Ok((Actions::cont(), observation));
        }
        Ok(((self.react)(inner, fact)?, TerminationObservation::Observed))
    }
}

/// Action-producing reaction to the matching exact terminal fact.
///
/// The value is always [`EstablishedObservation::Stopped`]. The complete enum
/// keeps the protocol and correlation visible in the callback type without
/// introducing a parallel terminal payload.
pub type EstablishedTerminationReaction<B, P> =
    fn(&mut B, EstablishedObservation<P>) -> BehaviorActed<B>;

/// Exact-incarnation termination observation policy.
pub struct EstablishedTerminationTarget<B: Behavior, P>
where
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    id: ObservationId,
    peer: EstablishedRecipient<P>,
    react: EstablishedTerminationReaction<B, P>,
}

impl<B, P> sealed::TerminationObservationTarget<B> for EstablishedTerminationTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
}

impl<B, P> TerminationObservationTarget<B> for EstablishedTerminationTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    type Fact = EstablishedObservation<P>;
    type Request = ObserveEstablished<P>;

    fn request(&self) -> Self::Request {
        ObserveEstablished::new(self.id, self.peer.clone())
    }

    fn react(
        &mut self,
        inner: &mut B,
        observation: TerminationObservation,
        fact: Self::Fact,
    ) -> Result<
        (
            Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
            TerminationObservation,
        ),
        B::Error,
    > {
        if fact.id() != self.id
            || matches!(
                observation,
                TerminationObservation::Observed
                    | TerminationObservation::Cancelled
                    | TerminationObservation::Rejected { .. }
            )
        {
            return Ok((Actions::cont(), observation));
        }
        match &fact {
            EstablishedObservation::Started { .. } => {
                Ok((Actions::cont(), TerminationObservation::Observing))
            }
            EstablishedObservation::Cancelled { .. } => {
                Ok((Actions::cont(), TerminationObservation::Cancelled))
            }
            EstablishedObservation::Rejected {
                operation, reason, ..
            } => Ok((
                Actions::cont(),
                TerminationObservation::Rejected {
                    operation: *operation,
                    reason: *reason,
                },
            )),
            EstablishedObservation::Stopped { .. } => {
                Ok(((self.react)(inner, fact)?, TerminationObservation::Observed))
            }
        }
    }
}

/// Observe one peer and fold its terminal fact into complete behavior actions.
///
/// Unlike [`crate::Watch`], the reaction returns the wrapped behavior's full
/// [`Actions`]. Cleanup communications, lifecycle publication, fresh
/// creations, and termination therefore remain explicit behavior decisions.
/// The selected target owns either late-bound logical-name observation or
/// exact-incarnation observation; the runtime delivers the target's declared
/// typed fact through the interpreter-request return path.
pub struct TerminationMonitorWith<B: Behavior, Target: TerminationObservationTarget<B>> {
    inner: B,
    target: Target,
    observation: TerminationObservation,
}

/// Address-selected action-producing termination monitor.
pub type TerminationMonitor<B> = TerminationMonitorWith<B, LogicalTerminationTarget<B>>;

/// Exact-incarnation action-producing termination monitor.
pub type EstablishedTerminationMonitor<B, P> =
    TerminationMonitorWith<B, EstablishedTerminationTarget<B, P>>;

/// A termination monitor configured to emit explicit cleanup actions.
///
/// Cleanup uses the monitor's exact-once observation phase and differs only in
/// the supplied action-producing policy, so it is a named specialization and
/// not a second state machine.
pub type Reaper<B> = TerminationMonitor<B>;

/// A termination monitor configured to publish explicit lifecycle actions.
///
/// Subscriber membership remains in the wrapped behavior or a composed typed
/// topic; no ambient lifecycle side channel is introduced.
pub type LifecyclePublisher<B> = TerminationMonitor<B>;
type TerminationMonitorActions<B, Target> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<
        InterpreterRequests<<Target as TerminationObservationTarget<B>>::Request>,
        <B as Behavior>::Sends,
    >,
    <B as Behavior>::Birth,
>;

impl<B: Behavior> TerminationMonitorWith<B, LogicalTerminationTarget<B>> {
    /// Construct an action-producing observation definition.
    #[must_use]
    pub const fn new(
        inner: B,
        peer: crate::BehaviorAddr<B>,
        on_stopped: TerminationReaction<B>,
    ) -> Self {
        Self {
            inner,
            target: LogicalTerminationTarget {
                peer,
                react: on_stopped,
            },
            observation: TerminationObservation::Observing,
        }
    }
}

impl<B, P> TerminationMonitorWith<B, EstablishedTerminationTarget<B, P>>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    #[must_use]
    pub fn established(
        inner: B,
        id: ObservationId,
        peer: EstablishedRecipient<P>,
        react: EstablishedTerminationReaction<B, P>,
    ) -> Self {
        Self {
            inner,
            target: EstablishedTerminationTarget { id, peer, react },
            observation: TerminationObservation::Requested,
        }
    }
}

impl<B: Behavior, Target: TerminationObservationTarget<B>> TerminationMonitorWith<B, Target> {
    /// Return whether this monitor awaits or has consumed its terminal fact.
    #[must_use]
    pub const fn observation(&self) -> TerminationObservation {
        self.observation
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        observations: InterpreterRequests<Target::Request>,
    ) -> TerminationMonitorActions<B, Target> {
        actions.map_sends(|inner| SendLayer::new(observations, inner))
    }
}

impl<B, Target> crate::BehaviorBase for TerminationMonitorWith<B, Target>
where
    B: Behavior + crate::BehaviorBase,
    Target: TerminationObservationTarget<B>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, Target> crate::StashStatus for TerminationMonitorWith<B, Target>
where
    B: Behavior + crate::StashStatus,
    Target: TerminationObservationTarget<B>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, Target, A, Ph, Sends, Br> Behavior for TerminationMonitorWith<B, Target>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    Target: TerminationObservationTarget<B>,
{
    type Protocol = B::Protocol;
    type Event = WatchEvent<B::Event, Target::Fact>;
    type Sends = SendLayer<InterpreterRequests<Target::Request>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(Self::wrap(
            actions,
            InterpreterRequests::one(self.target.request()),
        ))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(fact) => {
                let (actions, next) = self.target.react(&mut self.inner, self.observation, fact)?;
                self.observation = next;
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use crate::{Crash, Exit};
    use behavior::{Births, Create, MailAddr, Never, Step, User};

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
        type Birth = Births<()>;

        fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![1]))
        }

        fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "termination reactions preserve the behavior's fallible signature"
    )]
    #[allow(clippy::needless_pass_by_value)]
    fn reap(_: &mut Probe, stopped: PeerStopped<MailAddr>) -> BehaviorActed<Probe> {
        assert_eq!(stopped.peer, MailAddr(4));
        assert_eq!(stopped.outcome, Err(Crash::Panicked));
        Ok(Actions::new(
            vec![9],
            vec![Create::birth(8, ())],
            Step::Continue,
        ))
    }

    #[test]
    fn matching_terminal_fact_preserves_complete_reaction_actions() {
        let initialized = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap();
        assert_eq!(initialized.actions.sends.inner, [1]);
        assert_eq!(
            initialized.actions.sends.owned,
            InterpreterRequests::one(crate::ObservePeer::new(MailAddr(4)))
        );

        let mut active = initialized.behavior;
        let actions = active
            .on_path(PeerStopped::new(MailAddr(4), Err(Crash::Panicked)))
            .unwrap();
        assert_eq!(actions.sends.inner, [9]);
        assert!(actions.sends.owned.is_empty());
        assert_eq!(actions.creates, [Create::birth(8, ())]);
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(active.observation(), TerminationObservation::Observed);

        let duplicate = active
            .on_path(PeerStopped::new(MailAddr(4), Err(Crash::Panicked)))
            .unwrap();
        assert_eq!(duplicate.sends, SendLayer::empty());
        assert!(duplicate.creates.is_empty());
    }

    #[test]
    fn unmatched_terminal_fact_is_inert_and_user_actions_still_delegate() {
        let mut active = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap()
            .behavior;
        let unmatched = active
            .on_path(PeerStopped::new(MailAddr(5), Ok(Exit::Normal)))
            .unwrap();
        assert_eq!(unmatched.sends, SendLayer::empty());
        assert!(unmatched.creates.is_empty());

        let delegated = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(delegated.sends.inner, [7]);
        assert!(delegated.sends.owned.is_empty());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Rejected;

    struct Fallible;

    impl behavior::Protocol for Fallible {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Fallible {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Rejected;
        type Birth = behavior::NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn reject(_: &mut Fallible, _: PeerStopped<MailAddr>) -> BehaviorActed<Fallible> {
        Err(Rejected)
    }

    #[test]
    fn rejected_reaction_does_not_commit_observation_consumption() {
        let mut active = TerminationMonitor::new(Fallible, MailAddr(4), reject)
            .initialize()
            .unwrap()
            .behavior;
        assert_eq!(
            active.on_path(PeerStopped::new(MailAddr(4), Err(Crash::Failed))),
            Err(Rejected)
        );
        assert_eq!(active.observation(), TerminationObservation::Observing);
    }
}
