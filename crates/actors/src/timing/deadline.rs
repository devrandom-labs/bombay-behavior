//! Pure one-shot time composition. Scheduling is a request to the emitting
//! actor's local clock service.

use std::time::Instant;

use super::domain::OneShotSchedule;
use super::event::TimedEvent;
use crate::Step;
use crate::protocol::{ScheduleAt, TimerId};
use behavior::{
    Actions, Address, Become, Behavior, BirthMode, EventLayer, InterpreterRequests, SendEffects,
    SendLayer,
};

pub type DeadlineEvent<E> = TimedEvent<E>;

/// Infallible reaction to one accepted deadline.
///
/// ```compile_fail,E0308
/// # use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};
/// # use behavior_actors::{Deadline, TimerId};
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = MailAddr; type Msg = (); }
/// # impl Behavior for App {
/// #   type Protocol = Self; type Event = User<MailAddr, ()>; type Sends = Vec<Never>;
/// #   type Ph = Never; type Error = Never; type Birth = NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// fn fallible(_: &mut App) -> Result<behavior::Become, Never> {
///     Ok(behavior::Step::Continue)
/// }
/// let _ = Deadline::new(App, TimerId(1), None, fallible);
/// ```
pub type DeadlineReaction<B> = fn(&mut B) -> Become;

pub(crate) type DeadlineActions<B> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<ScheduleAt>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

pub struct Deadline<B: Behavior> {
    inner: B,
    schedule: OneShotSchedule,
    on_reached: DeadlineReaction<B>,
}

impl<B: Behavior> Deadline<B> {
    /// Wrap `inner` with one optional absolute deadline and pure reaction.
    ///
    /// Initialization stages the schedule when `at` is present. Clock access
    /// and timer delivery remain interpreter capabilities.
    /// Nested timers remain independently addressable even when they reuse the
    /// same `(id, generation)`: each emitted schedule selects this wrapper's
    /// structural ingress destination.
    /// The reaction is infallible because it receives mutable access to the
    /// wrapped behavior; ordinary delegated transitions retain its error type.
    #[must_use]
    pub fn new(
        inner: B,
        id: TimerId,
        at: Option<Instant>,
        on_reached: DeadlineReaction<B>,
    ) -> Self {
        Self {
            inner,
            schedule: OneShotSchedule::new(id, at),
            on_reached,
        }
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for Deadline<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B> crate::StashStatus for Deadline<B>
where
    B: Behavior + crate::StashStatus,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Deadline<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = DeadlineEvent<B::Event>;
    type Sends = SendLayer<InterpreterRequests<ScheduleAt>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> Result<DeadlineActions<B>, B::Error> {
        let actions = behavior::initialize(&mut self.inner)?;
        let own = if matches!(actions.become_, Step::Stop(_)) {
            self.schedule.cancel();
            InterpreterRequests::empty()
        } else {
            self.schedule.request().map_or_else(
                InterpreterRequests::empty,
                |(id, generation, at)| {
                    InterpreterRequests::one(ScheduleAt::new(id, generation, at))
                },
            )
        };
        Ok(Self::wrap(actions, own))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<DeadlineActions<B>, B::Error> {
        match event {
            EventLayer::Owned(event) if self.schedule.accept(event.id, event.generation) => {
                let become_ = match (self.on_reached)(&mut self.inner) {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::just(become_))
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
            EventLayer::Inner(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)?;
                if matches!(actions.become_, Step::Stop(_)) {
                    self.schedule.cancel();
                }
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
        }
    }
}

impl<B: Behavior> Deadline<B> {
    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        own: InterpreterRequests<ScheduleAt>,
    ) -> DeadlineActions<B> {
        actions.map_sends(|inner| SendLayer::new(own, inner))
    }
}
