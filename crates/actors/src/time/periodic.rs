//! Pure generation-safe periodic timer composition.

use std::time::Duration;

use super::domain::{TimerAdmission, TimerLease};
use super::event::TimedReaction;
use crate::protocol::{OwnedScheduleAfter, OwnedTimerElapsed};
use behavior::Step;
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EventLayer, InterpreterRequests,
    SendEffects, SendLayer,
};

/// Infallible fold invoked for each accepted periodic generation.
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
/// let _ = behavior_actors::Periodic::new(App, std::time::Duration::from_secs(1), fallible);
/// ```
/// Repeatedly notify a wrapped behavior at a relative interval.
///
/// Initialization preserves inner effects and appends generation zero. Each
/// matching timer event is consumed once, folds `on_elapsed`, and—only when
/// that fold continues—appends the next generation. Stale, duplicate, and
/// foreign-generation observations are inert unless the inner protocol accepts them.
/// Termination emits no replacement. Generation exhaustion leaves the
/// timer retired while preserving the inner continuation. These are Bombay
/// policies; clock access and sleeping remain `bombay-timers` capabilities.
/// Reactions are infallible because they receive mutable access to the wrapped
/// behavior; ordinary delegated transitions retain the wrapped error type.
pub struct Periodic<B: Behavior> {
    inner: B,
    every: Duration,
    lease: TimerLease,
    on_elapsed: TimedReaction<B>,
}

impl<B: Behavior> Periodic<B> {
    /// Wrap `inner` with a relative periodic timer and pure reaction.
    ///
    /// Accepted timer generations are rearmed only after a continuing
    /// reaction. Clock access and scheduling remain interpreter capabilities.
    #[must_use]
    pub fn new(inner: B, every: Duration, on_elapsed: TimedReaction<B>) -> Self {
        Self {
            inner,
            every,
            lease: TimerLease::new(),
            on_elapsed,
        }
    }

    fn schedule(&mut self) -> InterpreterRequests<OwnedScheduleAfter> {
        self.lease
            .arm()
            .map_or_else(InterpreterRequests::empty, |generation| {
                InterpreterRequests::one(OwnedScheduleAfter::new(generation, self.every))
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

    fn wrap_and_rearm(
        &mut self,
        actions: Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> Actions<
        behavior::BehaviorAddr<B>,
        B::Ph,
        SendLayer<InterpreterRequests<OwnedScheduleAfter>, B::Sends>,
        B::Birth,
    > {
        let schedules = if matches!(actions.become_, Step::Stop(_)) {
            self.lease.disarm();
            InterpreterRequests::empty()
        } else {
            self.schedule()
        };
        Self::wrap(actions, schedules)
    }
}

impl<B: Behavior + behavior::BehaviorBase> behavior::BehaviorBase for Periodic<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Periodic<B>
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
        Ok(self.wrap_and_rearm(actions))
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(elapsed) => match self.lease.accept(elapsed.generation) {
                TimerAdmission::Accepted => {
                    let actions = (self.on_elapsed)(&mut self.inner);
                    Ok(self.wrap_and_rearm(actions))
                }
                TimerAdmission::Ignored => Ok(Actions::cont()),
            },
            EventLayer::Inner(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)?;
                if matches!(actions.become_, Step::Stop(_)) {
                    self.lease.disarm();
                }
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::{MailAddr, Never, NoBirths, Step, User};

    struct Bell(usize);

    impl behavior::BehaviorBase for Bell {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Bell {
        type Addr = MailAddr;
        type Msg = Never;
    }

    impl Behavior for Bell {
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

    fn tick(probe: &mut Bell) -> Actions<MailAddr, Never, Vec<Never>, NoBirths> {
        probe.0 += 1;
        if probe.0 == 2 {
            Actions::stop()
        } else {
            Actions::cont()
        }
    }

    #[test]
    fn accepted_ticks_rearm_until_the_reaction_stops() {
        let every = Duration::from_secs(3);
        let initialized = crate::Periodic::new(Bell(0), every, tick)
            .initialize()
            .unwrap();
        assert_eq!(
            initialized.actions.sends.owned.as_slice(),
            [OwnedScheduleAfter::new(crate::TimerGeneration(0), every)]
        );
        assert!(initialized.actions.sends.inner.is_empty());
        assert!(initialized.actions.creates.is_empty());
        assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut active = initialized.behavior;

        let first = active
            .on_path(OwnedTimerElapsed::new(crate::TimerGeneration(0)))
            .unwrap();
        assert_eq!(
            first.sends.owned.as_slice(),
            [OwnedScheduleAfter::new(crate::TimerGeneration(1), every)]
        );
        assert!(matches!(first.become_, Step::Continue));
        assert!(first.sends.inner.is_empty());
        assert!(first.creates.is_empty());
        assert_eq!(active.base().0, 1);

        let duplicate = active
            .on_path(OwnedTimerElapsed::new(crate::TimerGeneration(0)))
            .unwrap();
        assert!(duplicate.sends.owned.is_empty());
        assert!(duplicate.sends.inner.is_empty());
        assert!(duplicate.creates.is_empty());
        assert!(matches!(duplicate.become_, Step::Continue));
        assert_eq!(active.base().0, 1);

        let stopped = active
            .on_path(OwnedTimerElapsed::new(crate::TimerGeneration(1)))
            .unwrap();
        assert!(stopped.sends.owned.is_empty());
        assert!(matches!(stopped.become_, Step::Stop(_)));
        assert!(stopped.sends.inner.is_empty());
        assert!(stopped.creates.is_empty());
        assert_eq!(active.base().0, 2);
    }
}
