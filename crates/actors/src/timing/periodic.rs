//! Pure generation-safe periodic timer composition.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::Step;
use crate::protocol::{ScheduleAfter, TimerId};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EventLayer, InterpreterRequests,
    SendEffects, SendLayer,
};

/// Complete event sum accepted by [`Periodic`].
pub type PeriodicEvent<E> = TimedEvent<E>;

/// Infallible fold invoked for each accepted periodic generation.
///
/// ```compile_fail,E0308
/// # use std::time::Duration;
/// # use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};
/// # use behavior_actors::{Periodic, TimerId};
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = MailAddr; type Msg = (); }
/// # impl Behavior for App {
/// #   type Protocol = Self; type Event = User<MailAddr, ()>; type Sends = Vec<Never>;
/// #   type Ph = Never; type Error = Never; type Birth = NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// fn fallible(_: &mut App) -> behavior::BehaviorActed<App> { Ok(Actions::cont()) }
/// let _ = Periodic::new(App, TimerId(1), Duration::from_secs(1), fallible);
/// ```
pub type PeriodicReaction<B> = fn(
    &mut B,
) -> Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

/// Repeatedly notify a wrapped behavior at a relative interval.
///
/// Initialization preserves inner effects and appends generation zero. Each
/// matching timer event is consumed once, folds `on_elapsed`, and—only when
/// that fold continues—appends the next generation. Stale, duplicate, and
/// wrong-ID observations are inert unless the inner protocol accepts them.
/// Termination emits no replacement. Generation exhaustion leaves the
/// timer retired while preserving the inner continuation. These are Bombay
/// policies; clock access and sleeping remain `bombay-timers` capabilities.
/// Reactions are infallible because they receive mutable access to the wrapped
/// behavior; ordinary delegated transitions retain the wrapped error type.
pub struct Periodic<B: Behavior> {
    inner: B,
    id: TimerId,
    every: Duration,
    lease: TimerLease,
    on_elapsed: PeriodicReaction<B>,
}

impl<B: Behavior> Periodic<B> {
    /// Wrap `inner` with a relative periodic timer and pure reaction.
    ///
    /// Accepted timer generations are rearmed only after a continuing
    /// reaction. Clock access and scheduling remain interpreter capabilities.
    #[must_use]
    pub fn new(inner: B, id: TimerId, every: Duration, on_elapsed: PeriodicReaction<B>) -> Self {
        Self {
            inner,
            id,
            every,
            lease: TimerLease::new(),
            on_elapsed,
        }
    }

    fn schedule(&mut self) -> InterpreterRequests<ScheduleAfter> {
        self.lease
            .arm()
            .map_or_else(InterpreterRequests::empty, |generation| {
                InterpreterRequests::one(ScheduleAfter::new(self.id, generation, self.every))
            })
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        schedules: InterpreterRequests<ScheduleAfter>,
    ) -> Actions<
        crate::BehaviorAddr<B>,
        B::Ph,
        SendLayer<InterpreterRequests<ScheduleAfter>, B::Sends>,
        B::Birth,
    > {
        actions.map_sends(|inner| SendLayer::new(schedules, inner))
    }

    fn wrap_and_rearm(
        &mut self,
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> Actions<
        crate::BehaviorAddr<B>,
        B::Ph,
        SendLayer<InterpreterRequests<ScheduleAfter>, B::Sends>,
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

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for Periodic<B> {
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
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = PeriodicEvent<B::Event>;
    type Sends = SendLayer<InterpreterRequests<ScheduleAfter>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(self.wrap_and_rearm(actions))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(elapsed)
                if elapsed.id == self.id && self.lease.accept(elapsed.generation) =>
            {
                let actions = (self.on_elapsed)(&mut self.inner);
                Ok(self.wrap_and_rearm(actions))
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
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
    use crate::{Activate as _, TimerElapsed};
    use behavior::{MailAddr, Never, NoBirths, Step, User};

    struct Probe(usize);

    impl crate::BehaviorBase for Probe {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Probe {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Probe {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn tick(probe: &mut Probe) -> Actions<MailAddr, Never, Vec<Never>, NoBirths> {
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
        let initialized = crate::Periodic::new(Probe(0), TimerId(2), every, tick)
            .initialize()
            .unwrap();
        assert_eq!(
            initialized.actions.sends.owned.as_slice(),
            [ScheduleAfter::new(
                TimerId(2),
                crate::TimerGeneration(0),
                every
            )]
        );
        let mut active = initialized.behavior;

        let first = active
            .on_path(TimerElapsed::new(TimerId(2), crate::TimerGeneration(0)))
            .unwrap();
        assert_eq!(
            first.sends.owned.as_slice(),
            [ScheduleAfter::new(
                TimerId(2),
                crate::TimerGeneration(1),
                every
            )]
        );
        assert!(matches!(first.become_, Step::Continue));
        assert_eq!(active.base().0, 1);

        let duplicate = active
            .on_path(TimerElapsed::new(TimerId(2), crate::TimerGeneration(0)))
            .unwrap();
        assert!(duplicate.sends.owned.is_empty());
        assert_eq!(active.base().0, 1);

        let stopped = active
            .on_path(TimerElapsed::new(TimerId(2), crate::TimerGeneration(1)))
            .unwrap();
        assert!(stopped.sends.owned.is_empty());
        assert!(matches!(stopped.become_, Step::Stop(_)));
        assert_eq!(active.base().0, 2);
    }
}
