//! Pure peer-observation composition over an ordinary monitor actor protocol.

use crate::protocol::forward::forward_event_lane;
use crate::protocol::{ObservePeer, PeerStopped};
use crate::{Crash, Exit, Step};
use crate::{Own, RouteInput, SendInput};
use behavior::{
    Actions, Address, Become, Behavior, BirthMode, SendAlgebra, ServiceSends, User, UserEvent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent<E: UserEvent> {
    Behavior(E),
    PeerStopped(PeerStopped<E::Addr>),
}

impl<E: UserEvent> crate::RouteInput<PeerStopped<E::Addr>> for WatchEvent<E> {
    fn route(event: PeerStopped<E::Addr>) -> Result<Self, PeerStopped<E::Addr>> {
        Ok(Self::PeerStopped(event))
    }
}

impl<E: UserEvent> crate::EventInput<PeerStopped<E::Addr>> for WatchEvent<E> {
    fn inject(event: PeerStopped<E::Addr>) -> Self {
        Self::PeerStopped(event)
    }
}

impl<E: UserEvent> UserEvent for WatchEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Behavior(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Behavior(event) => event.into_user().map_err(Self::Behavior),
            stopped @ Self::PeerStopped(_) => Err(stopped),
        }
    }
}

forward_event_lane!(WatchEvent, crate::TimerElapsed);
forward_event_lane!(WatchEvent, crate::ChildStopped<E::Addr>);
forward_event_lane!(WatchEvent, crate::WorkerStopped<E::Addr>);
forward_event_lane!(
    WatchEvent,
    crate::CreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    WatchEvent,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(WatchEvent, crate::ShutdownRequested);

pub type LinkReaction<B> = fn(
    &mut B,
    <B as crate::Protocol>::Addr,
    &Result<Exit<<B as crate::Protocol>::Addr>, Crash>,
) -> Result<Become, <B as Behavior>::Error>;

/// A mutual-lifecycle-policy specialization uses the same typed observation
/// algebra as [`Watch`]; reciprocity is established by applying it at both
/// endpoints rather than by a privileged runtime link table.
pub type Link<B> = Watch<B>;

/// Named effect lanes added by [`Watch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchSends<A: Address, Sends> {
    /// Sends emitted by the wrapped behavior or its pure stop reaction.
    pub behavior: Sends,
    /// Peer-observation requests interpreted by the local observation capability.
    pub observations: ServiceSends<ObservePeer<A>>,
}

impl<A: Address, Sends: SendAlgebra> SendAlgebra for WatchSends<A, Sends> {
    fn empty() -> Self {
        Self {
            behavior: Sends::empty(),
            observations: ServiceSends::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.behavior.append(other.behavior);
        self.observations.append(other.observations);
    }
}

impl<A: Address, Sends> SendInput<ObservePeer<A>, Own> for WatchSends<A, Sends> {
    fn emit(&mut self, input: ObservePeer<A>) {
        self.observations.send(input);
    }
}

pub(crate) type WatchActions<B> = Actions<
    <B as crate::Protocol>::Addr,
    <B as Behavior>::Ph,
    WatchSends<<B as crate::Protocol>::Addr, <B as Behavior>::Sends>,
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
    peer: B::Addr,
    on_stopped: LinkReaction<B>,
}

impl<B: Behavior> Watch<B> {
    /// Wrap `inner` with one statically addressed peer observation.
    ///
    /// Initialization emits the observation request after preserving the
    /// wrapped behavior's initialization effects. A matching terminal fact is
    /// folded exactly once through `on_stopped`.
    #[must_use]
    pub fn new(inner: B, peer: B::Addr, on_stopped: LinkReaction<B>) -> Self {
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

impl<B, A, Ph, Sends, Br> behavior::Protocol for Watch<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: crate::RouteInput<PeerStopped<A>>,
{
    type Addr = A;
    type Msg = B::Msg;
}

impl<B, A, Ph, Sends, Br> Behavior for Watch<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: crate::RouteInput<PeerStopped<A>>,
{
    type Event = WatchEvent<B::Event>;
    type Sends = WatchSends<A, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> Result<WatchActions<B>, B::Error> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(Self::wrap(actions, ServiceSends::one(self.peer.into())))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<WatchActions<B>, B::Error> {
        match event {
            WatchEvent::PeerStopped(event) if event.peer == self.peer => {
                let become_ = match (self.on_stopped)(&mut self.inner, event.peer, &event.outcome)?
                {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::new(Self::Sends::empty(), Vec::new(), become_))
            }
            WatchEvent::PeerStopped(event) => match B::Event::route(event) {
                Ok(inner) => behavior::delegate_transition(&mut self.inner, inner)
                    .map(|actions| Self::wrap(actions, ServiceSends::empty())),
                Err(_) => Ok(Actions::cont()),
            },
            WatchEvent::Behavior(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, ServiceSends::empty())),
        }
    }
}

impl<B: Behavior> Watch<B> {
    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ObservePeer<B::Addr>>,
    ) -> WatchActions<B> {
        actions.map_sends(|behavior| WatchSends {
            behavior,
            observations: own,
        })
    }
}

/// Stop when the monitor reports an abnormal outcome.
///
/// # Errors
/// This supplied policy never creates a controlled error.
pub fn stop_on_abnormal_death<B: Behavior>(
    _behavior: &mut B,
    peer: B::Addr,
    outcome: &Result<Exit<B::Addr>, Crash>,
) -> Result<Become, B::Error> {
    Ok(if let Ok(Exit::Normal | Exit::Collected) = outcome {
        Step::Continue
    } else {
        let _ = peer;
        Step::Stop(crate::Stopped)
    })
}
