//! Peer observation as a specialized use of the action-producing termination
//! monitor.

use crate::lifecycle::termination_monitor::sealed;
use crate::protocol::{
    EstablishedObservation, ObservationId, ObserveEstablished, ObservePeer, PeerStopped,
};
use crate::{Crash, Exit, Step, TerminationMonitorWith, TerminationObservation};
use behavior::{
    Actions, Become, Behavior, EndpointAddress, EstablishedRecipient, EventLayer, UserEvent,
};

fn lift_become<Ph>(become_: Become) -> behavior::Step<Ph, behavior::Stopped> {
    match become_ {
        Step::Continue => Step::Continue,
        Step::Goto(never) => match never {},
        Step::Stop(stopped) => Step::Stop(stopped),
    }
}

/// Complete event sum accepted by observation compositions.
pub type WatchEvent<E, Fact = PeerStopped<<E as UserEvent>::Addr>> = EventLayer<Fact, E>;

/// Infallible reaction to one matching logical peer stop.
pub type LinkReaction<B> =
    fn(&mut B, crate::BehaviorAddr<B>, &Result<Exit<crate::BehaviorAddr<B>>, Crash>) -> Become;

/// Address-selected become-only observation policy.
pub struct LogicalWatchTarget<B: Behavior> {
    peer: crate::BehaviorAddr<B>,
    on_stopped: LinkReaction<B>,
}

impl<B: Behavior> sealed::TerminationObservationTarget<B> for LogicalWatchTarget<B> {}

impl<B: Behavior> crate::TerminationObservationTarget<B> for LogicalWatchTarget<B> {
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
        let become_ = lift_become((self.on_stopped)(inner, fact.peer, &fact.outcome));
        // A logical address may denote a later incarnation. Unlike an exact
        // one-shot termination relationship, a watch remains active after
        // each matching stop fact.
        Ok((Actions::just(become_), TerminationObservation::Observing))
    }
}

/// Complete become-only reaction to one exact established observation fact.
pub type EstablishedWatchReaction<B, P> = fn(&mut B, EstablishedObservation<P>) -> Become;

/// Exact-incarnation become-only observation policy.
pub struct EstablishedWatchTarget<B: Behavior, P>
where
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    id: ObservationId,
    peer: EstablishedRecipient<P>,
    react: EstablishedWatchReaction<B, P>,
}

impl<B, P> sealed::TerminationObservationTarget<B> for EstablishedWatchTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
}

impl<B, P> crate::TerminationObservationTarget<B> for EstablishedWatchTarget<B, P>
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
            (TerminationObservation::Requested, EstablishedObservation::Started { .. }) => Ok((
                Actions::just(lift_become((self.react)(inner, fact))),
                TerminationObservation::Observing,
            )),
            (
                TerminationObservation::Requested | TerminationObservation::Observing,
                EstablishedObservation::Rejected {
                    operation, reason, ..
                },
            ) => {
                let operation = *operation;
                let reason = *reason;
                Ok((
                    Actions::just(lift_become((self.react)(inner, fact))),
                    TerminationObservation::Rejected { operation, reason },
                ))
            }
            (TerminationObservation::Observing, EstablishedObservation::Cancelled { .. }) => Ok((
                Actions::just(lift_become((self.react)(inner, fact))),
                TerminationObservation::Cancelled,
            )),
            (TerminationObservation::Observing, EstablishedObservation::Stopped { .. }) => Ok((
                Actions::just(lift_become((self.react)(inner, fact))),
                TerminationObservation::Observed,
            )),
            _ => Err(fact),
        }
    }
}

/// A logical-address peer watch using the shared termination-monitor fold.
///
/// The reaction is deliberately infallible because it receives mutable access
/// to the wrapped behavior. A fallible reaction could mutate the behavior and
/// then reject the same fact, which cannot be represented as an atomic fold.
///
/// ```compile_fail,E0308
/// # use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, Step, User};
/// # use behavior_actors::{Crash, Exit, Watch};
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = MailAddr; type Msg = (); }
/// # impl Behavior for App {
/// #   type Protocol = Self; type Event = User<MailAddr, ()>; type Sends = Vec<Never>;
/// #   type Ph = Never; type Error = Never; type Birth = NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// fn fallible(
///     _: &mut App,
///     _: MailAddr,
///     _: &Result<Exit<MailAddr>, Crash>,
/// ) -> Result<behavior::Become, u8> {
///     Ok(Step::Continue)
/// }
/// let _ = Watch::new(App, MailAddr(1), fallible);
/// ```
pub type Watch<B> = TerminationMonitorWith<B, LogicalWatchTarget<B>>;

/// An exact-incarnation peer watch.
pub type EstablishedWatch<B, P> = TerminationMonitorWith<B, EstablishedWatchTarget<B, P>>;

impl<B: Behavior> TerminationMonitorWith<B, LogicalWatchTarget<B>> {
    /// Observe one logical peer and apply a become-only reaction once.
    #[must_use]
    pub const fn new(inner: B, peer: crate::BehaviorAddr<B>, on_stopped: LinkReaction<B>) -> Self {
        Self::with_target(
            inner,
            LogicalWatchTarget { peer, on_stopped },
            TerminationObservation::Observing,
        )
    }
}

impl<B, P> TerminationMonitorWith<B, EstablishedWatchTarget<B, P>>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    /// Observe one exact installed peer and apply a become-only reaction once.
    #[must_use]
    pub fn established(
        inner: B,
        id: ObservationId,
        peer: EstablishedRecipient<P>,
        react: EstablishedWatchReaction<B, P>,
    ) -> Self {
        Self::with_target(
            inner,
            EstablishedWatchTarget { id, peer, react },
            TerminationObservation::Requested,
        )
    }
}

/// Stop when the monitor reports an abnormal outcome.
pub fn stop_on_abnormal_death<B: Behavior>(
    _behavior: &mut B,
    _peer: crate::BehaviorAddr<B>,
    outcome: &Result<Exit<crate::BehaviorAddr<B>>, Crash>,
) -> Become {
    if let Ok(Exit::Normal | Exit::Collected) = outcome {
        Step::Continue
    } else {
        Step::Stop(crate::Stopped)
    }
}
