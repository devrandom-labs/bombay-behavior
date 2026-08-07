//! Pure peer-observation composition over an ordinary monitor actor protocol.

use crate::behavior::{
    Actions, Address, Become, Behavior, BirthMode, SendAlgebra, SendProduct, ServiceSends, User,
    UserEvent,
};
use crate::deadlined::{TimeEvent, TimeReached};
use crate::shutdown::{ShutdownEvent, ShutdownRequested};
use crate::supervising::{ChildEvent, ChildStopped, WorkerEvent, WorkerStopped};
use crate::{Crash, Exit, Step};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservePeer<A> {
    pub peer: A,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerStopped<A: Address> {
    pub peer: A,
    pub outcome: Result<Exit<A>, Crash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent<E, A: Address> {
    Inner(E),
    PeerStopped(PeerStopped<A>),
}

pub trait PeerEvent<A: Address>: Sized {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self>;
}

impl<E, A: Address> PeerEvent<A> for WatchEvent<E, A> {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self> {
        Some(Self::PeerStopped(event))
    }
}

impl<E: UserEvent, A: Address> UserEvent for WatchEvent<E, A> {
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

impl<E: TimeEvent, A: Address> TimeEvent for WatchEvent<E, A> {
    fn time_reached(event: TimeReached) -> Option<Self> {
        E::time_reached(event).map(Self::Inner)
    }
}

impl<E: ChildEvent<A>, A: Address> ChildEvent<A> for WatchEvent<E, A> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        E::child_stopped(event).map(Self::Inner)
    }
}

impl<E: WorkerEvent<A>, A: Address> WorkerEvent<A> for WatchEvent<E, A> {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self> {
        E::worker_stopped(event).map(Self::Inner)
    }
}

impl<E: ShutdownEvent, A: Address> ShutdownEvent for WatchEvent<E, A> {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self> {
        E::shutdown_requested(event).map(Self::Inner)
    }
}

pub type LinkReaction<B> = fn(
    &mut B,
    <B as Behavior>::Addr,
    &Result<Exit<<B as Behavior>::Addr>, Crash>,
) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

pub type WatchSends<B> =
    SendProduct<<B as Behavior>::Sends, ServiceSends<ObservePeer<<B as Behavior>::Addr>>>;

pub type WatchActions<B> =
    Actions<<B as Behavior>::Addr, <B as Behavior>::Ph, WatchSends<B>, <B as Behavior>::Birth>;

pub struct Watching<B: Behavior> {
    inner: B,
    peer: B::Addr,
    on_stopped: LinkReaction<B>,
}

impl<B: Behavior> Watching<B> {
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

impl<B, A, Ph, Sends, Br> Behavior for Watching<B>
where
    A: Address + Send,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<
            Addr = A,
            Ph = Ph,
            Sends = Sends,
            Birth = Br,
            Effect = Actions<A, Ph, Sends, Br>,
            Done = Exit<A>,
        > + Send,
    B::Event: PeerEvent<B::Addr> + Send,
    B::Msg: Send,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = WatchEvent<B::Event, B::Addr>;
    type Sends = SendProduct<Sends, ServiceSends<ObservePeer<A>>>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;
    type Effect = Actions<A, Ph, Self::Sends, Br>;
    type Done = Exit<A>;

    async fn init(&mut self) -> Result<Self::Effect, B::Error> {
        let actions = self.inner.init().await?;
        Ok(Self::wrap(
            actions,
            ServiceSends::one(ObservePeer { peer: self.peer }),
        ))
    }

    async fn step(&mut self, event: Self::Event) -> Result<Self::Effect, B::Error> {
        match event {
            WatchEvent::PeerStopped(event) if event.peer == self.peer => {
                let become_ = match (self.on_stopped)(&mut self.inner, event.peer, &event.outcome)?
                {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions {
                    sends: Self::Sends::empty(),
                    creates: Vec::new(),
                    become_,
                })
            }
            WatchEvent::PeerStopped(event) => match B::Event::peer_stopped(event) {
                Some(inner) => self
                    .inner
                    .step(inner)
                    .await
                    .map(|actions| Self::wrap(actions, ServiceSends::empty())),
                None => Ok(Actions::cont()),
            },
            WatchEvent::Inner(event) => self
                .inner
                .step(event)
                .await
                .map(|actions| Self::wrap(actions, ServiceSends::empty())),
        }
    }
}

impl<B: Behavior> Watching<B> {
    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ObservePeer<B::Addr>>,
    ) -> WatchActions<B> {
        Actions {
            sends: SendProduct {
                inner: actions.sends,
                own,
            },
            creates: actions.creates,
            become_: actions.become_,
        }
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
