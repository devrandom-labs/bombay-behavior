//! Pure relative one-shot timer composition.

use std::time::Duration;

use super::domain::{TimerAdmission, TimerLease};
use super::event::TimedReaction;
use crate::protocol::{OwnedScheduleAfter, OwnedTimerElapsed};
use behavior::Step;
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EventLayer, InterpreterRequests,
    SendEffects, SendLayer,
};

/// Infallible fold invoked for the one accepted timer generation.
///
/// ```compile_fail,E0308
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = behavior::MailAddr; type Msg = (); }
/// # impl behavior::Behavior for App {
/// #   type Protocol = Self; type Event = behavior::User<behavior::MailAddr, ()>; type Sends = Vec<behavior::Never>;
/// #   type Ph = behavior::Never; type Error = behavior::Never; type Birth = behavior::NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// # }
/// fn fallible(_: &mut App) -> behavior::BehaviorActed<App> { Ok(behavior::Actions::cont()) }
/// let _ = behavior_actors::OneShot::new(App, std::time::Duration::ZERO, fallible);
/// ```
/// Notify a wrapped behavior once after a relative delay.
///
/// Initialization first preserves the wrapped initialization actions, then
/// emits one generation-tagged `ScheduleAfter` request. A matching elapsed
/// event is consumed exactly once and folds `on_elapsed`; stale and duplicate
/// generations are inert unless the wrapped event sum independently accepts
/// them. Reaction sends, creations, and termination are preserved.
/// Reactions are infallible because they receive mutable access to the wrapped
/// behavior; ordinary delegated transitions retain the wrapped error type.
/// The timer never rearms. Generation exhaustion disables scheduling without
/// affecting the wrapped fold. These timer and ordering rules are Bombay
/// policy; sleeping and clock interpretation belong to `bombay-timers`.
pub struct OneShot<B: Behavior> {
    inner: B,
    after: Duration,
    lease: TimerLease,
    on_elapsed: TimedReaction<B>,
}

impl<B: Behavior> OneShot<B> {
    /// Construct a relative one-shot wrapper definition.
    #[must_use]
    pub fn new(inner: B, after: Duration, on_elapsed: TimedReaction<B>) -> Self {
        Self {
            inner,
            after,
            lease: TimerLease::new(),
            on_elapsed,
        }
    }

    fn schedule(&mut self) -> InterpreterRequests<OwnedScheduleAfter> {
        self.lease
            .arm()
            .map_or_else(InterpreterRequests::empty, |generation| {
                InterpreterRequests::one(OwnedScheduleAfter::new(generation, self.after))
            })
    }

    fn wrap(
        actions: Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        schedules: InterpreterRequests<OwnedScheduleAfter>,
    ) -> Actions<
        behavior::BehaviorAddr<B>,
        B::Ph,
        SendLayer<InterpreterRequests<OwnedScheduleAfter>, B::Sends>,
        B::Birth,
    > {
        actions.map_sends(|inner| SendLayer::new(schedules, inner))
    }
}

impl<B: Behavior + behavior::BehaviorBase> behavior::BehaviorBase for OneShot<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for OneShot<B>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: behavior::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = EventLayer<OwnedTimerElapsed, B::Event>;
    type Sends = SendLayer<InterpreterRequests<OwnedScheduleAfter>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        let schedules = if matches!(actions.become_, Step::Stop(_)) {
            self.lease.disarm();
            InterpreterRequests::empty()
        } else {
            self.schedule()
        };
        Ok(Self::wrap(actions, schedules))
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(elapsed) => match self.lease.accept(elapsed.generation) {
                TimerAdmission::Accepted => {
                    let actions = (self.on_elapsed)(&mut self.inner);
                    Ok(Self::wrap(actions, InterpreterRequests::empty()))
                }
                TimerAdmission::Ignored => Ok(Actions::cont()),
            },
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::{MailAddr, Never, NoBirths, Step, User};

    struct Reminder {
        elapsed: usize,
    }

    impl behavior::BehaviorBase for Reminder {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Reminder {
        type Addr = MailAddr;
        type Msg = Never;
    }

    impl Behavior for Reminder {
        type Protocol = Self;
        type Event = User<MailAddr, Never>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: behavior::ActiveTurn,
            event: Self::Event,
        ) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    fn mark(probe: &mut Reminder) -> Actions<MailAddr, Never, Vec<Never>, NoBirths> {
        probe.elapsed += 1;
        Actions::cont()
    }

    #[test]
    fn initialization_schedules_then_matching_generation_fires_once() {
        let delay = Duration::from_millis(20);
        let initialized = crate::OneShot::new(Reminder { elapsed: 0 }, delay, mark)
            .initialize()
            .unwrap();
        assert!(initialized.actions.sends.inner.is_empty());
        assert_eq!(
            initialized.actions.sends.owned.as_slice(),
            [OwnedScheduleAfter::new(crate::TimerGeneration(0), delay)]
        );
        assert!(initialized.actions.creates.is_empty());
        assert!(matches!(initialized.actions.become_, Step::Continue));

        let mut active = initialized.behavior;
        let wrong_generation = active
            .on_path(OwnedTimerElapsed::new(crate::TimerGeneration(1)))
            .unwrap();
        assert!(wrong_generation.sends == SendLayer::empty());
        assert!(wrong_generation.creates.is_empty());
        assert!(matches!(wrong_generation.become_, Step::Continue));
        assert_eq!(active.base().elapsed, 0);

        let fired = active
            .on_path(OwnedTimerElapsed::new(crate::TimerGeneration(0)))
            .unwrap();
        assert!(fired.sends == SendLayer::empty());
        assert!(fired.creates.is_empty());
        assert!(matches!(fired.become_, Step::Continue));
        assert_eq!(active.base().elapsed, 1);

        let duplicate = active
            .on_path(OwnedTimerElapsed::new(crate::TimerGeneration(0)))
            .unwrap();
        assert_eq!(duplicate.sends, SendLayer::empty());
        assert!(duplicate.creates.is_empty());
        assert!(matches!(duplicate.become_, Step::Continue));
        assert_eq!(active.base().elapsed, 1);
    }
}
