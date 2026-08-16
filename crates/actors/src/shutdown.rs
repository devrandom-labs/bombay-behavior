//! Typed graceful-shutdown composition.
//!
//! Shutdown is a Bombay policy expressed as an ordinary behavior transition,
//! not an additional actor-model effect. An interpreter may construct the
//! shutdown lane, but ingress closure and mailbox ordering remain interpreter
//! concerns.

use crate::Step;
use crate::protocol::ShutdownRequested;
use crate::protocol::forward::forward_event_lane;
use behavior::{Actions, Address, Behavior, BirthMode, SendAlgebra, User, UserEvent};

/// The complete protocol of a behavior that supports graceful shutdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownProtocol<E> {
    Behavior(E),
    ShutdownRequested(ShutdownRequested),
}

impl<E: UserEvent> crate::RouteInput<ShutdownRequested> for ShutdownProtocol<E> {
    fn route(event: ShutdownRequested) -> Result<Self, ShutdownRequested> {
        Ok(Self::ShutdownRequested(event))
    }
}

impl<E: UserEvent> crate::EventInput<ShutdownRequested> for ShutdownProtocol<E> {
    fn inject(event: ShutdownRequested) -> Self {
        Self::ShutdownRequested(event)
    }
}

impl<E: UserEvent> UserEvent for ShutdownProtocol<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Behavior(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Behavior(event) => event.into_user().map_err(Self::Behavior),
            shutdown @ Self::ShutdownRequested(_) => Err(shutdown),
        }
    }
}

forward_event_lane!(ShutdownProtocol, crate::TimerElapsed);
forward_event_lane!(ShutdownProtocol, crate::PeerStopped<E::Addr>);
forward_event_lane!(ShutdownProtocol, crate::ChildStopped<E::Addr>);
forward_event_lane!(ShutdownProtocol, crate::WorkerStopped<E::Addr>);
forward_event_lane!(
    ShutdownProtocol,
    crate::CreationResolved<<E::Addr as crate::Address>::Nonce>
);
forward_event_lane!(
    ShutdownProtocol,
    crate::WorkerCreationResolved<<E::Addr as crate::Address>::Nonce>
);

/// Stop normally when the shutdown lane is received.
pub struct StopOnShutdown<B> {
    inner: B,
}

impl<B> StopOnShutdown<B> {
    /// Wrap `inner` so the first typed shutdown request stops it normally.
    #[must_use]
    pub const fn new(inner: B) -> Self {
        Self { inner }
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for StopOnShutdown<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B: crate::StashStatus> crate::StashStatus for StopOnShutdown<B> {
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
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
    /// Wrap `inner` with one pure finalization fold on typed shutdown.
    ///
    /// The final fold's sends and creations are preserved and the wrapper then
    /// stops normally regardless of the fold's continuation verdict.
    #[must_use]
    pub const fn new(inner: B, finalize: ShutdownReaction<B>) -> Self {
        Self { inner, finalize }
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for FinalizeOnShutdown<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B> crate::StashStatus for FinalizeOnShutdown<B>
where
    B: Behavior + crate::StashStatus,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

macro_rules! impl_shutdown_behavior {
    ($wrapper:ident, $shutdown:expr) => {
        impl<B, A, Ph, Sends, Br> Behavior for $wrapper<B>
        where
            A: Address,
            Sends: SendAlgebra,
            Br: BirthMode,
            B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
        {
            type Addr = A;
            type Msg = B::Msg;
            type Event = ShutdownProtocol<B::Event>;
            type Sends = Sends;
            type Ph = Ph;
            type Error = B::Error;
            type Birth = Br;

            fn init(
                &mut self,
                _: crate::InitializationTurn,
            ) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
                behavior::initialize(&mut self.inner)
            }

            fn transition(
                &mut self,
                _: crate::ActiveTurn,
                event: Self::Event,
            ) -> Result<Actions<A, Ph, Sends, Br>, B::Error> {
                match event {
                    ShutdownProtocol::Behavior(event) => {
                        behavior::delegate_transition(&mut self.inner, event)
                    }
                    ShutdownProtocol::ShutdownRequested(request) => $shutdown(self, request),
                }
            }
        }
    };
}

impl_shutdown_behavior!(StopOnShutdown, |_this: &mut StopOnShutdown<B>, _request| {
    Ok(Actions::stop())
});

impl_shutdown_behavior!(
    FinalizeOnShutdown,
    |this: &mut FinalizeOnShutdown<B>, request| {
        let actions = (this.finalize)(&mut this.inner, request)?;
        Ok(actions.map_become(|_| Step::Stop(behavior::Stopped)))
    }
);
