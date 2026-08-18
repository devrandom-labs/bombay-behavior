//! Pure relative one-shot timer composition.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::Step;
use crate::protocol::{ScheduleAfter, TimerId};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EventLayer, InterpreterRequests,
    SendEffects, SendLayer,
};

/// Complete event sum accepted by [`OneShot`].
pub type OneShotEvent<E> = TimedEvent<E>;

/// Pure fold invoked for the one accepted timer generation.
pub type OneShotReaction<B> = fn(&mut B) -> BehaviorActed<B>;

/// Notify a wrapped behavior once after a relative delay.
///
/// Initialization first preserves the wrapped initialization actions, then
/// emits one generation-tagged `ScheduleAfter` request. A matching elapsed
/// event is consumed exactly once and folds `on_elapsed`; stale and duplicate
/// generations are inert unless the wrapped event sum independently accepts
/// them. Reaction sends, creations, errors, and termination are preserved.
/// The timer never rearms. Generation exhaustion disables scheduling without
/// affecting the wrapped fold. These timer and ordering rules are Bombay
/// policy; sleeping and clock interpretation belong to `bombay-timers`.
pub struct OneShot<B: Behavior> {
    inner: B,
    id: TimerId,
    after: Duration,
    lease: TimerLease,
    on_elapsed: OneShotReaction<B>,
}

impl<B: Behavior> OneShot<B> {
    /// Construct a relative one-shot wrapper definition.
    #[must_use]
    pub fn new(inner: B, id: TimerId, after: Duration, on_elapsed: OneShotReaction<B>) -> Self {
        Self {
            inner,
            id,
            after,
            lease: TimerLease::new(),
            on_elapsed,
        }
    }

    fn schedule(&mut self) -> InterpreterRequests<ScheduleAfter> {
        self.lease
            .arm()
            .map_or_else(InterpreterRequests::empty, |generation| {
                InterpreterRequests::one(ScheduleAfter::new(self.id, generation, self.after))
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
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for OneShot<B> {
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
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = OneShotEvent<B::Event>;
    type Sends = SendLayer<InterpreterRequests<ScheduleAfter>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        let schedules = if matches!(actions.become_, Step::Stop(_)) {
            self.lease.disarm();
            InterpreterRequests::empty()
        } else {
            self.schedule()
        };
        Ok(Self::wrap(actions, schedules))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(elapsed)
                if elapsed.id == self.id && self.lease.accept(elapsed.generation) =>
            {
                let actions = (self.on_elapsed)(&mut self.inner)?;
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Activate as _, TimerElapsed};
    use behavior::{MailAddr, Never, NoBirths, Step, User};

    struct Probe {
        elapsed: usize,
    }

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

    #[allow(
        clippy::unnecessary_wraps,
        reason = "reaction type is deliberately fallible"
    )]
    fn mark(probe: &mut Probe) -> BehaviorActed<Probe> {
        probe.elapsed += 1;
        Ok(Actions::cont())
    }

    #[test]
    fn initialization_schedules_then_matching_generation_fires_once() {
        let delay = Duration::from_millis(20);
        let initialized = crate::OneShot::new(Probe { elapsed: 0 }, TimerId(7), delay, mark)
            .initialize()
            .unwrap();
        assert!(initialized.actions.sends.inner.is_empty());
        assert_eq!(
            initialized.actions.sends.owned.as_slice(),
            [ScheduleAfter::new(
                TimerId(7),
                crate::TimerGeneration(0),
                delay
            )]
        );
        assert!(initialized.actions.creates.is_empty());
        assert!(matches!(initialized.actions.become_, Step::Continue));

        let mut active = initialized.behavior;
        let wrong_id = active
            .on_path(TimerElapsed::new(TimerId(8), crate::TimerGeneration(0)))
            .unwrap();
        assert!(wrong_id.sends == SendLayer::empty());
        assert_eq!(active.base().elapsed, 0);

        let fired = active
            .on_path(TimerElapsed::new(TimerId(7), crate::TimerGeneration(0)))
            .unwrap();
        assert!(fired.sends == SendLayer::empty());
        assert_eq!(active.base().elapsed, 1);

        active
            .on_path(TimerElapsed::new(TimerId(7), crate::TimerGeneration(0)))
            .unwrap();
        assert_eq!(active.base().elapsed, 1);
    }
}
