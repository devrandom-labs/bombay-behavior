//! Pure peer-observation composition over an ordinary monitor actor protocol.

use crate::protocol::{ObservePeer, PeerStopped};
use crate::{Crash, Exit, Step};
use behavior::{
    Actions, Address, Become, Behavior, BirthMode, EventLayer, InterpreterRequests, SendEffects,
    SendLayer, UserEvent,
};

pub type WatchEvent<E> = EventLayer<PeerStopped<<E as UserEvent>::Addr>, E>;

pub type LinkReaction<B> = fn(
    &mut B,
    crate::BehaviorAddr<B>,
    &Result<Exit<crate::BehaviorAddr<B>>, Crash>,
) -> Result<Become, <B as Behavior>::Error>;

/// A mutual-lifecycle-policy specialization uses the same typed observation
/// algebra as [`Watch`]; reciprocity is established by applying it at both
/// endpoints rather than by a privileged runtime link table.
pub type Link<B> = Watch<B>;

pub(crate) type WatchActions<B> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<ObservePeer<crate::BehaviorAddr<B>>>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

/// A pure peer-observation transformation.
///
/// Initialization emits exactly one [`ObservePeer`] request after preserving
/// the inner initialization effects. A matching [`PeerStopped`] result invokes
/// the configured reaction whether the interpreter produced it immediately
/// from authoritative retained termination or after observing a live
/// incarnation. The transformation retains no runtime observation handle or
/// lifecycle flag; exact-incarnation selection belongs to the interpreter.
pub struct Watch<B: Behavior> {
    inner: B,
    peer: crate::BehaviorAddr<B>,
    on_stopped: LinkReaction<B>,
}

impl<B: Behavior> Watch<B> {
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
            peer,
            on_stopped,
        }
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for Watch<B> {
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
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = WatchEvent<B::Event>;
    type Sends = SendLayer<InterpreterRequests<ObservePeer<A>>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> Result<WatchActions<B>, B::Error> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(Self::wrap(
            actions,
            InterpreterRequests::one(self.peer.into()),
        ))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<WatchActions<B>, B::Error> {
        match event {
            EventLayer::Owned(event) if event.peer == self.peer => {
                let become_ = match (self.on_stopped)(&mut self.inner, event.peer, &event.outcome)?
                {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::new(Self::Sends::empty(), Vec::new(), become_))
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty())),
        }
    }
}

impl<B: Behavior> Watch<B> {
    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        own: InterpreterRequests<ObservePeer<crate::BehaviorAddr<B>>>,
    ) -> WatchActions<B> {
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
