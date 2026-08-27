//! Typed graceful-shutdown composition.
//!
//! Shutdown is a Bombay policy expressed as an ordinary behavior transition,
//! not an additional actor-model effect. An interpreter may construct the
//! shutdown lane, but ingress closure and mailbox ordering remain interpreter
//! concerns.

use crate::Step;
use crate::protocol::ShutdownRequested;
use behavior::{
    Actions, Address, Behavior, BirthMode, EventLayer, NoSends, SendEffects, SendLayer,
};

/// Internal event sum of a behavior that supports graceful shutdown.
pub type ShutdownEvent<E> = EventLayer<ShutdownRequested, E>;

/// Stop normally when the shutdown lane is received.
pub struct StopOnShutdown<B> {
    inner: B,
}

impl<B> StopOnShutdown<B> {
    /// Wrap `inner` so the first typed shutdown request stops it normally.
    /// This wrapper owns every shutdown request it adds. It therefore composes
    /// over any behavior without requiring the inner event algebra to already
    /// accept shutdown. When shutdown wrappers are nested, the outermost
    /// wrapper owns the request.
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
) -> Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

/// Run one explicit final fold and then stop normally.
///
/// The finalization reaction is infallible because it receives mutable access
/// to `B`: a fallible reaction could change `B` and then reject the same
/// shutdown fact, violating transition atomicity. Ordinary delegated `B`
/// transitions retain `B::Error`.
///
/// ```compile_fail,E0308
/// # use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};
/// # use behavior_actors::{FinalizeOnShutdown, ShutdownRequested};
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = MailAddr; type Msg = (); }
/// # impl Behavior for App {
/// #   type Protocol = Self; type Event = User<MailAddr, ()>; type Sends = Vec<Never>;
/// #   type Ph = Never; type Error = Never; type Birth = NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// fn fallible(_: &mut App, _: ShutdownRequested) -> behavior::BehaviorActed<App> {
///     Ok(Actions::cont())
/// }
/// let _ = FinalizeOnShutdown::new(App, fallible);
/// ```
pub struct FinalizeOnShutdown<B: Behavior> {
    inner: B,
    finalize: ShutdownReaction<B>,
}

impl<B: Behavior> FinalizeOnShutdown<B> {
    /// Wrap `inner` with one pure finalization fold on typed shutdown.
    ///
    /// The final fold's sends and creations are preserved and the wrapper then
    /// stops normally regardless of the fold's continuation verdict.
    /// Like [`StopOnShutdown`], this wrapper owns the lane it adds; an outer
    /// shutdown wrapper therefore takes precedence over this reaction.
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
            Sends: SendEffects + behavior::SendsFor<B::Event>,
            Br: BirthMode,
            B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
            B::Protocol: crate::Protocol<Addr = A>,
        {
            type Protocol = B::Protocol;
            type Event = ShutdownEvent<B::Event>;
            type Sends = SendLayer<NoSends, Sends>;
            type Ph = Ph;
            type Error = B::Error;
            type Birth = Br;

            fn init(
                &mut self,
                _: crate::InitializationTurn,
            ) -> Result<Actions<A, Ph, Self::Sends, Br>, B::Error> {
                behavior::initialize(&mut self.inner)
                    .map(|actions| actions.map_sends(|inner| SendLayer::new(NoSends, inner)))
            }

            fn transition(
                &mut self,
                _: crate::ActiveTurn,
                event: Self::Event,
            ) -> Result<Actions<A, Ph, Self::Sends, Br>, B::Error> {
                match event {
                    EventLayer::Inner(event) => {
                        behavior::delegate_transition(&mut self.inner, event).map(|actions| {
                            actions.map_sends(|inner| SendLayer::new(NoSends, inner))
                        })
                    }
                    EventLayer::Owned(request) => $shutdown(self, request),
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
        let actions = (this.finalize)(&mut this.inner, request);
        Ok(actions
            .map_become(|_| Step::Stop(behavior::Stopped))
            .map_sends(|inner| SendLayer::new(NoSends, inner)))
    }
);
