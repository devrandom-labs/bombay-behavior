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
pub type TerminationReaction<B> = fn(
    &mut B,
    PeerStopped<crate::BehaviorAddr<B>>,
) -> Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

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

/// Exact rejection from the observation wrapper.
pub enum TerminationMonitorError<E, Fact> {
    /// The wrapped behavior rejected its own event.
    Inner(E),
    /// A returned observation fact does not belong to the current phase or
    /// configured relationship.
    UnexpectedFact {
        observation: TerminationObservation,
        fact: Fact,
    },
}

impl<E: core::fmt::Debug, Fact> core::fmt::Debug for TerminationMonitorError<E, Fact> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Inner(error) => formatter.debug_tuple("Inner").field(error).finish(),
            Self::UnexpectedFact { observation, .. } => formatter
                .debug_struct("UnexpectedFact")
                .field("observation", observation)
                .field("fact", &"<retained>")
                .finish(),
        }
    }
}

impl<E, Fact> core::fmt::Display for TerminationMonitorError<E, Fact> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Inner(_) => formatter.write_str("wrapped behavior rejected its event"),
            Self::UnexpectedFact { .. } => {
                formatter.write_str("observation fact does not match the active relationship phase")
            }
        }
    }
}

impl<E, Fact> std::error::Error for TerminationMonitorError<E, Fact>
where
    E: std::error::Error + 'static,
    Fact: 'static,
{
}

pub(crate) mod sealed {
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
        Self::Fact,
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
        Self::Fact,
    > {
        if observation != TerminationObservation::Observing || fact.peer != self.peer {
            return Err(fact);
        }
        Ok(((self.react)(inner, fact), TerminationObservation::Observed))
    }
}

/// Action-producing reaction to the matching exact terminal fact.
///
/// The value is always [`EstablishedObservation::Stopped`]. The complete enum
/// keeps the protocol and correlation visible in the callback type without
/// introducing a parallel terminal payload.
pub type EstablishedTerminationReaction<B, P> = fn(
    &mut B,
    EstablishedObservation<P>,
) -> Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

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
        Self::Fact,
    > {
        if fact.id() != self.id {
            return Err(fact);
        }
        match (observation, &fact) {
            (TerminationObservation::Requested, EstablishedObservation::Started { .. }) => {
                Ok((Actions::cont(), TerminationObservation::Observing))
            }
            (
                TerminationObservation::Requested,
                EstablishedObservation::Rejected {
                    operation, reason, ..
                },
            )
            | (
                TerminationObservation::Observing,
                EstablishedObservation::Rejected {
                    operation, reason, ..
                },
            ) => Ok((
                Actions::cont(),
                TerminationObservation::Rejected {
                    operation: *operation,
                    reason: *reason,
                },
            )),
            (TerminationObservation::Observing, EstablishedObservation::Cancelled { .. }) => {
                Ok((Actions::cont(), TerminationObservation::Cancelled))
            }
            (TerminationObservation::Observing, EstablishedObservation::Stopped { .. }) => {
                Ok(((self.react)(inner, fact), TerminationObservation::Observed))
            }
            _ => Err(fact),
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
/// Reactions are infallible because they receive mutable access to `B`: a
/// fallible callback could change `B` and then reject the same fact, violating
/// transition atomicity. Ordinary delegated `B` transitions retain `B::Error`.
///
/// ```compile_fail,E0308
/// # use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};
/// # use behavior_actors::{PeerStopped, TerminationMonitor};
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = MailAddr; type Msg = (); }
/// # impl Behavior for App {
/// #   type Protocol = Self; type Event = User<MailAddr, ()>; type Sends = Vec<Never>;
/// #   type Ph = Never; type Error = Never; type Birth = NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// fn fallible(_: &mut App, _: PeerStopped<MailAddr>) -> behavior::BehaviorActed<App> {
///     Ok(Actions::cont())
/// }
/// let _ = TerminationMonitor::new(App, MailAddr(1), fallible);
/// ```
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
    type Error = TerminationMonitorError<B::Error, Target::Fact>;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions =
            behavior::initialize(&mut self.inner).map_err(TerminationMonitorError::Inner)?;
        Ok(Self::wrap(
            actions,
            InterpreterRequests::one(self.target.request()),
        ))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(fact) => {
                let (actions, next) = self
                    .target
                    .react(&mut self.inner, self.observation, fact)
                    .map_err(|fact| TerminationMonitorError::UnexpectedFact {
                        observation: self.observation,
                        fact,
                    })?;
                self.observation = next;
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty()))
                .map_err(TerminationMonitorError::Inner),
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

    #[allow(clippy::needless_pass_by_value)]
    fn reap(
        _: &mut Probe,
        stopped: PeerStopped<MailAddr>,
    ) -> Actions<MailAddr, Never, Vec<u8>, Births<()>> {
        assert_eq!(stopped.peer, MailAddr(4));
        assert_eq!(stopped.outcome, Err(Crash::Panicked));
        Actions::new(vec![9], vec![Create::birth(8, ())], Step::Continue)
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

        let duplicate = PeerStopped::new(MailAddr(4), Err(Crash::Panicked));
        assert!(matches!(
            active.on_path(duplicate.clone()),
            Err(TerminationMonitorError::UnexpectedFact {
                observation: TerminationObservation::Observed,
                fact,
            }) if fact == duplicate
        ));
    }

    #[test]
    fn unmatched_terminal_fact_is_returned_complete_and_user_actions_still_delegate() {
        let mut active = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap()
            .behavior;
        let unmatched = PeerStopped::new(MailAddr(5), Ok(Exit::Normal));
        assert!(matches!(
            active.on_path(unmatched.clone()),
            Err(TerminationMonitorError::UnexpectedFact {
                observation: TerminationObservation::Observing,
                fact,
            }) if fact == unmatched
        ));

        let delegated = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(delegated.sends.inner, [7]);
        assert!(delegated.sends.owned.is_empty());
    }
}
