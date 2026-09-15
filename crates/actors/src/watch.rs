//! Recurring logical-name peer observation.

use crate::protocol::{ObservePeer, PeerStopped};
use crate::{Crash, Exit, TerminationMonitorError, TerminationObservation};
use behavior::Step;
use behavior::{
    Actions, Address, Become, Behavior, BehaviorActed, BirthMode, EventLayer, InterpreterRequests,
    SendEffects, SendLayer, UserEvent,
};

/// Complete event sum accepted by a logical peer watch.
pub type WatchEvent<E, Report = PeerStopped<<E as UserEvent>::Addr>> = EventLayer<Report, E>;

/// Infallible reaction to one matching logical peer stop.
pub type LinkReaction<B> = fn(
    &mut B,
    behavior::BehaviorAddr<B>,
    &Result<Exit<behavior::BehaviorAddr<B>>, Crash>,
) -> Become;

/// Observe a logical peer name across any number of later incarnations.
///
/// A logical name can denote another incarnation after a stop, so every
/// matching report is accepted and the watch remains observing. This recurrence
/// law is distinct from [`crate::TerminationMonitor`], which consumes one
/// correlated terminal relationship. Initialization emits one typed
/// [`ObservePeer`] request after preserving the inner initialization effects.
pub struct Watch<B: Behavior> {
    inner: B,
    peer: behavior::BehaviorAddr<B>,
    on_stopped: LinkReaction<B>,
}

type WatchActions<B> = Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<ObservePeer<behavior::BehaviorAddr<B>>>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

impl<B: Behavior> Watch<B> {
    /// Wrap `inner` with recurring observation of one logical peer name.
    #[must_use]
    pub const fn new(
        inner: B,
        peer: behavior::BehaviorAddr<B>,
        on_stopped: LinkReaction<B>,
    ) -> Self {
        Self {
            inner,
            peer,
            on_stopped,
        }
    }

    fn wrap(
        actions: Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        observations: InterpreterRequests<ObservePeer<behavior::BehaviorAddr<B>>>,
    ) -> WatchActions<B> {
        actions.map_sends(|inner| SendLayer::new(observations, inner))
    }
}

impl<B> behavior::BehaviorBase for Watch<B>
where
    B: Behavior + behavior::BehaviorBase,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B> crate::StashStatus for Watch<B>
where
    B: Behavior + crate::StashStatus,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Watch<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: behavior::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = WatchEvent<B::Event>;
    type Sends = SendLayer<InterpreterRequests<ObservePeer<A>>, Sends>;
    type Ph = Ph;
    type Error = TerminationMonitorError<B::Error, PeerStopped<A>>;
    type Birth = Br;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        let actions =
            behavior::initialize(&mut self.inner).map_err(TerminationMonitorError::Inner)?;
        Ok(Self::wrap(
            actions,
            InterpreterRequests::one(ObservePeer::new(self.peer)),
        ))
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(report) if report.peer == self.peer => {
                let become_ = match (self.on_stopped)(&mut self.inner, report.peer, &report.outcome)
                {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(stopped) => Step::Stop(stopped),
                };
                Ok(Self::wrap(
                    Actions::just(become_),
                    InterpreterRequests::empty(),
                ))
            }
            EventLayer::Owned(report) => Err(TerminationMonitorError::UnexpectedReport {
                observation: TerminationObservation::Observing,
                report,
            }),
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty()))
                .map_err(TerminationMonitorError::Inner),
        }
    }
}

/// Stop when a logical watch reports an abnormal outcome.
pub fn stop_on_abnormal_death<B: Behavior>(
    _behavior: &mut B,
    _peer: behavior::BehaviorAddr<B>,
    outcome: &Result<Exit<behavior::BehaviorAddr<B>>, Crash>,
) -> Become {
    if let Ok(Exit::Normal | Exit::Collected) = outcome {
        Step::Continue
    } else {
        Step::Stop(behavior::Stopped)
    }
}
