//! Pure peer-observation composition over an ordinary monitor actor protocol.

use crate::protocol::{
    EstablishedObservation, ObservationId, ObserveEstablished, ObservePeer, PeerStopped,
};
use crate::{Crash, Exit, Step};
use behavior::{
    Actions, Address, Become, Behavior, BirthMode, EndpointAddress, EstablishedRecipient,
    EventLayer, InterpreterRequest, InterpreterRequests, ReturnsToEmitter, SendEffects, SendLayer,
    UserEvent,
};

pub type WatchEvent<E, Fact = PeerStopped<<E as UserEvent>::Addr>> = EventLayer<Fact, E>;

pub type LinkReaction<B> = fn(
    &mut B,
    crate::BehaviorAddr<B>,
    &Result<Exit<crate::BehaviorAddr<B>>, Crash>,
) -> Result<Become, <B as Behavior>::Error>;

/// A mutual-lifecycle-policy specialization uses the same typed observation
/// algebra as [`Watch`]; reciprocity is established by applying it at both
/// endpoints rather than by a privileged runtime link table.
pub type Link<B> = Watch<B>;

mod sealed {
    pub trait WatchTarget<B: behavior::Behavior> {}
}

/// Static peer-observation policy owned by one [`WatchWith`] composition.
pub trait WatchTarget<B: Behavior>: sealed::WatchTarget<B> {
    type Fact;
    type Request: InterpreterRequest<ReturnToEmitter = ReturnsToEmitter<Self::Fact, behavior::Here>>;

    fn request(&self) -> Self::Request;
    fn react(&mut self, inner: &mut B, fact: Self::Fact) -> Result<Become, B::Error>;
}

/// Address-selected legacy observation policy.
pub struct LogicalWatchTarget<B: Behavior> {
    peer: crate::BehaviorAddr<B>,
    on_stopped: LinkReaction<B>,
}

impl<B: Behavior> sealed::WatchTarget<B> for LogicalWatchTarget<B> {}

impl<B: Behavior> WatchTarget<B> for LogicalWatchTarget<B> {
    type Fact = PeerStopped<crate::BehaviorAddr<B>>;
    type Request = ObservePeer<crate::BehaviorAddr<B>>;

    fn request(&self) -> Self::Request {
        ObservePeer::new(self.peer)
    }

    fn react(&mut self, inner: &mut B, fact: Self::Fact) -> Result<Become, B::Error> {
        if fact.peer == self.peer {
            (self.on_stopped)(inner, fact.peer, &fact.outcome)
        } else {
            Ok(Step::Continue)
        }
    }
}

/// Complete reaction to one exact observation fact.
pub type EstablishedWatchReaction<B, P> =
    fn(&mut B, EstablishedObservation<P>) -> Result<Become, <B as Behavior>::Error>;

/// Exact-incarnation observation policy.
pub struct EstablishedWatchTarget<B: Behavior, P>
where
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    id: ObservationId,
    peer: EstablishedRecipient<P>,
    react: EstablishedWatchReaction<B, P>,
}

impl<B, P> sealed::WatchTarget<B> for EstablishedWatchTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
}

impl<B, P> WatchTarget<B> for EstablishedWatchTarget<B, P>
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

    fn react(&mut self, inner: &mut B, fact: Self::Fact) -> Result<Become, B::Error> {
        if fact.id() == self.id {
            (self.react)(inner, fact)
        } else {
            Ok(Step::Continue)
        }
    }
}

pub(crate) type WatchActions<B, Target> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<<Target as WatchTarget<B>>::Request>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

/// A pure peer-observation transformation.
///
/// Initialization emits exactly one statically selected observation request
/// after preserving the inner initialization effects. Logical mode accepts a
/// matching [`PeerStopped`] fact; established mode accepts only facts carrying
/// its exact [`ObservationId`] and capability protocol. The transformation
/// retains no runtime observation handle or lifecycle flag.
pub struct WatchWith<B: Behavior, Target: WatchTarget<B>> {
    inner: B,
    target: Target,
}

/// A watch that selects its peer by logical address.
pub type Watch<B> = WatchWith<B, LogicalWatchTarget<B>>;

/// A watch over one exact installed protocol incarnation.
pub type EstablishedWatch<B, P> = WatchWith<B, EstablishedWatchTarget<B, P>>;

impl<B: Behavior> WatchWith<B, LogicalWatchTarget<B>> {
    /// Wrap `inner` with one statically addressed peer observation.
    ///
    /// Initialization emits the observation request after preserving the
    /// wrapped behavior's initialization effects. A matching terminal fact is
    /// folded exactly once through `on_stopped`.
    /// Nested watchers remain independently addressable even when they name
    /// the same peer: each observation request selects this wrapper's exact
    /// structural ingress destination.
    #[must_use]
    pub fn new(inner: B, peer: crate::BehaviorAddr<B>, on_stopped: LinkReaction<B>) -> Self {
        Self {
            inner,
            target: LogicalWatchTarget { peer, on_stopped },
        }
    }
}

impl<B, P> WatchWith<B, EstablishedWatchTarget<B, P>>
where
    B: Behavior,
    P: behavior::Protocol<Addr = crate::BehaviorAddr<B>>,
    crate::BehaviorAddr<B>: EndpointAddress,
{
    /// Observe one exact installed peer and expose every relationship fact to
    /// the supplied pure reaction.
    #[must_use]
    pub fn established(
        inner: B,
        id: ObservationId,
        peer: EstablishedRecipient<P>,
        react: EstablishedWatchReaction<B, P>,
    ) -> Self {
        Self {
            inner,
            target: EstablishedWatchTarget { id, peer, react },
        }
    }
}

impl<B, Target> crate::BehaviorBase for WatchWith<B, Target>
where
    B: Behavior + crate::BehaviorBase,
    Target: WatchTarget<B>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, Target> crate::StashStatus for WatchWith<B, Target>
where
    B: Behavior + crate::StashStatus,
    Target: WatchTarget<B>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, Target, A, Ph, Sends, Br> Behavior for WatchWith<B, Target>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    Target: WatchTarget<B>,
{
    type Protocol = B::Protocol;
    type Event = WatchEvent<B::Event, Target::Fact>;
    type Sends = SendLayer<InterpreterRequests<Target::Request>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> Result<WatchActions<B, Target>, B::Error> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(Self::wrap(
            actions,
            InterpreterRequests::one(self.target.request()),
        ))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<WatchActions<B, Target>, B::Error> {
        match event {
            EventLayer::Owned(event) => {
                let become_ = match self.target.react(&mut self.inner, event)? {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::new(Self::Sends::empty(), Vec::new(), become_))
            }
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty())),
        }
    }
}

impl<B: Behavior, Target: WatchTarget<B>> WatchWith<B, Target> {
    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        own: InterpreterRequests<Target::Request>,
    ) -> WatchActions<B, Target> {
        actions.map_sends(|inner| SendLayer::new(own, inner))
    }
}

/// Stop when the monitor reports an abnormal outcome.
///
/// # Errors
/// This supplied policy never creates a controlled error.
pub fn stop_on_abnormal_death<B: Behavior>(
    _behavior: &mut B,
    peer: crate::BehaviorAddr<B>,
    outcome: &Result<Exit<crate::BehaviorAddr<B>>, Crash>,
) -> Result<Become, B::Error> {
    Ok(if let Ok(Exit::Normal | Exit::Collected) = outcome {
        Step::Continue
    } else {
        let _ = peer;
        Step::Stop(crate::Stopped)
    })
}
