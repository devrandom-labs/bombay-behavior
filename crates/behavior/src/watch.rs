//! Pure peer-observation composition over an ordinary monitor actor protocol.

use crate::behavior::{
    Actions, Address, Become, Behavior, BirthMode, SendAlgebra, ServiceSends, User, UserEvent,
};
use crate::protocol::forward::forward_event_lane;
use crate::protocol::{ObservePeer, PeerEvent, PeerStopped};
use crate::{Crash, Exit, Step};
use crate::{Inner, Own, SendInput};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent<E: UserEvent> {
    Inner(E),
    PeerStopped(PeerStopped<E::Addr>),
}

impl<E: UserEvent> PeerEvent for WatchEvent<E> {
    fn peer_stopped(event: PeerStopped<E::Addr>) -> Option<Self> {
        Some(Self::PeerStopped(event))
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
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            stopped @ Self::PeerStopped(_) => Err(stopped),
        }
    }
}

forward_event_lane!(WatchEvent, TimeEvent, time_reached, crate::TimerElapsed);
forward_event_lane!(
    WatchEvent,
    ChildEvent,
    child_stopped,
    crate::ChildStopped<E::Addr>
);
forward_event_lane!(
    WatchEvent,
    WorkerEvent,
    worker_stopped,
    crate::WorkerStopped<E::Addr>
);
forward_event_lane!(
    WatchEvent,
    CreationEvent,
    creation_resolved,
    crate::CreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    WatchEvent,
    WorkerCreationEvent,
    worker_creation_resolved,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    WatchEvent,
    ShutdownEvent,
    shutdown_requested,
    crate::ShutdownRequested
);

pub type LinkReaction<B> = fn(
    &mut B,
    <B as Behavior>::Addr,
    &Result<Exit<<B as Behavior>::Addr>, Crash>,
) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

/// Named effect lanes added by [`Watch`].
pub struct WatchSends<A: Address, Sends> {
    pub behavior: Sends,
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

impl<A: Address, Sends, Input, Path> SendInput<Input, Inner<Path>> for WatchSends<A, Sends>
where
    Sends: SendInput<Input, Path>,
{
    fn emit(&mut self, input: Input) {
        <Sends as SendInput<Input, Path>>::emit(&mut self.behavior, input);
    }
}

pub type WatchActions<B> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    WatchSends<<B as Behavior>::Addr, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

pub struct Watch<B: Behavior> {
    inner: B,
    peer: B::Addr,
    on_stopped: LinkReaction<B>,
}

impl<B: Behavior> Watch<B> {
    #[must_use]
    pub fn new(inner: B, peer: B::Addr, on_stopped: LinkReaction<B>) -> Self {
        Self {
            inner,
            peer,
            on_stopped,
        }
    }

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Watch<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: PeerEvent,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = WatchEvent<B::Event>;
    type Sends = WatchSends<A, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self) -> Result<WatchActions<B>, B::Error> {
        let actions = self.inner.init()?;
        Ok(Self::wrap(actions, ServiceSends::one(self.peer.into())))
    }

    fn transition(&mut self, event: Self::Event) -> Result<WatchActions<B>, B::Error> {
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
            WatchEvent::PeerStopped(event) => match B::Event::peer_stopped(event) {
                Some(inner) => self
                    .inner
                    .transition(inner)
                    .map(|actions| Self::wrap(actions, ServiceSends::empty())),
                None => Ok(Actions::cont()),
            },
            WatchEvent::Inner(event) => self
                .inner
                .transition(event)
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
) -> Result<Become<B::Addr>, B::Error> {
    Ok(match outcome {
        Ok(Exit::Normal | Exit::Collected) => Step::Continue,
        _ => Step::Stop(Exit::LinkDied(peer)),
    })
}
