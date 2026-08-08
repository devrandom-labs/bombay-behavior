//! Typed graceful-shutdown composition.
//!
//! Shutdown is a Bombay policy expressed as an ordinary behavior transition,
//! not an additional actor-model effect. An interpreter may construct the
//! shutdown lane, but ingress closure and mailbox ordering remain interpreter
//! concerns.

use crate::behavior::{Actions, Address, Behavior, BirthMode, SendAlgebra, User, UserEvent};
use crate::protocol::{
    ChildEvent, ChildStopped, PeerEvent, PeerStopped, ShutdownEvent, ShutdownRequested, TimeEvent,
    TimeReached, WorkerEvent, WorkerStopped,
};
use crate::{Exit, Step};

/// The complete protocol of a behavior that supports graceful shutdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownProtocol<E> {
    Inner(E),
    ShutdownRequested(ShutdownRequested),
}

impl<E> ShutdownEvent for ShutdownProtocol<E> {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self> {
        Some(Self::ShutdownRequested(event))
    }
}

impl<E: UserEvent> UserEvent for ShutdownProtocol<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            shutdown @ Self::ShutdownRequested(_) => Err(shutdown),
        }
    }
}

impl<E: TimeEvent> TimeEvent for ShutdownProtocol<E> {
    fn time_reached(event: TimeReached) -> Option<Self> {
        E::time_reached(event).map(Self::Inner)
    }
}

impl<E: PeerEvent<A>, A: Address> PeerEvent<A> for ShutdownProtocol<E> {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self> {
        E::peer_stopped(event).map(Self::Inner)
    }
}

impl<E: ChildEvent<A>, A: Address> ChildEvent<A> for ShutdownProtocol<E> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        E::child_stopped(event).map(Self::Inner)
    }
}

impl<E: WorkerEvent<A>, A: Address> WorkerEvent<A> for ShutdownProtocol<E> {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self> {
        E::worker_stopped(event).map(Self::Inner)
    }
}

/// Stop normally when the shutdown lane is received.
pub struct StopOnShutdown<B> {
    inner: B,
}

impl<B> StopOnShutdown<B> {
    #[must_use]
    pub fn new(inner: B) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

/// A final shutdown fold. Its sends and fresh creations are retained, while
/// its become verdict is replaced with `Stop(Normal)`.
pub type ShutdownReaction<B> = fn(
    &mut B,
    ShutdownRequested,
) -> Result<
    Actions<
        <B as Behavior>::Addr,
        <B as Behavior>::Ph,
        <B as Behavior>::Sends,
        <B as Behavior>::Birth,
    >,
    <B as Behavior>::Error,
>;

/// Run one explicit final fold and then stop normally.
pub struct FinalizeOnShutdown<B: Behavior> {
    inner: B,
    finalize: ShutdownReaction<B>,
}

impl<B: Behavior> FinalizeOnShutdown<B> {
    #[must_use]
    pub fn new(inner: B, finalize: ShutdownReaction<B>) -> Self {
        Self { inner, finalize }
    }

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

macro_rules! impl_shutdown_behavior {
    ($wrapper:ident, $shutdown:expr) => {
        impl<B, A, Ph, Sends, Br> Behavior for $wrapper<B>
        where
            A: Address + Send,
            Sends: SendAlgebra,
            Br: BirthMode,
            B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br> + Send,
            B::Event: Send,
            B::Msg: Send,
        {
            type Addr = A;
            type Msg = B::Msg;
            type Event = ShutdownProtocol<B::Event>;
            type Sends = Sends;
            type Ph = Ph;
            type Error = B::Error;
            type Birth = Br;

            async fn init(&mut self) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
                self.inner.init().await
            }

            async fn step(
                &mut self,
                event: Self::Event,
            ) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
                match event {
                    ShutdownProtocol::Inner(event) => self.inner.step(event).await,
                    ShutdownProtocol::ShutdownRequested(request) => $shutdown(self, request),
                }
            }
        }
    };
}

impl_shutdown_behavior!(StopOnShutdown, |_this: &mut StopOnShutdown<B>, _request| {
    Ok(Actions::stop(Exit::Normal))
});

impl_shutdown_behavior!(
    FinalizeOnShutdown,
    |this: &mut FinalizeOnShutdown<B>, request| {
        let actions = (this.finalize)(&mut this.inner, request)?;
        Ok(Actions {
            sends: actions.sends,
            creates: actions.creates,
            become_: Step::Stop(Exit::Normal),
        })
    }
);
