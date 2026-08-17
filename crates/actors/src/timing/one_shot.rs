//! Pure relative one-shot timer composition.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::protocol::{ScheduleAfter, TimerElapsed, TimerId};
use crate::{Own, RouteInput, SendInput, Step};
use behavior::{Actions, Address, Behavior, BehaviorActed, BirthMode, SendAlgebra, ServiceSends};

/// Complete event sum accepted by [`OneShot`].
pub type OneShotEvent<E> = TimedEvent<E>;

/// Pure fold invoked for the one accepted timer generation.
pub type OneShotReaction<B> = fn(&mut B) -> BehaviorActed<B>;

/// Named send product contributed by [`OneShot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneShotSends<Sends> {
    /// Sends emitted by the wrapped behavior or timer reaction.
    pub behavior: Sends,
    /// Relative schedule requests interpreted by Bombay Timers.
    pub schedules: ServiceSends<ScheduleAfter>,
}

impl<Sends: SendAlgebra> SendAlgebra for OneShotSends<Sends> {
    fn empty() -> Self {
        Self {
            behavior: Sends::empty(),
            schedules: ServiceSends::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.behavior.append(other.behavior);
        self.schedules.append(other.schedules);
    }
}

impl<Sends> SendInput<ScheduleAfter, Own> for OneShotSends<Sends> {
    fn emit(&mut self, input: ScheduleAfter) {
        self.schedules.send(input);
    }
}

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

    fn schedule(&mut self) -> ServiceSends<ScheduleAfter> {
        self.lease
            .arm()
            .map_or_else(ServiceSends::empty, |generation| {
                ServiceSends::one(ScheduleAfter::new(self.id, generation, self.after))
            })
    }

    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        schedules: ServiceSends<ScheduleAfter>,
    ) -> Actions<B::Addr, B::Ph, OneShotSends<B::Sends>, B::Birth> {
        actions.map_sends(|behavior| OneShotSends {
            behavior,
            schedules,
        })
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for OneShot<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, A, Ph, Sends, Br> behavior::Protocol for OneShot<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: RouteInput<TimerElapsed>,
{
    type Addr = A;
    type Msg = B::Msg;
}

impl<B, A, Ph, Sends, Br> Behavior for OneShot<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: RouteInput<TimerElapsed>,
{
    type Event = OneShotEvent<B::Event>;
    type Sends = OneShotSends<Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        let schedules = if matches!(actions.become_, Step::Stop(_)) {
            self.lease.disarm();
            ServiceSends::empty()
        } else {
            self.schedule()
        };
        Ok(Self::wrap(actions, schedules))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            TimedEvent::Elapsed(elapsed)
                if elapsed.id == self.id && self.lease.accept(elapsed.generation) =>
            {
                let actions = (self.on_elapsed)(&mut self.inner)?;
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            TimedEvent::Elapsed(elapsed) => match B::Event::route(elapsed) {
                Ok(inner) => behavior::delegate_transition(&mut self.inner, inner)
                    .map(|actions| Self::wrap(actions, ServiceSends::empty())),
                Err(_) => Ok(Actions::cont()),
            },
            TimedEvent::Behavior(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, ServiceSends::empty())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
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
        assert!(initialized.actions.sends.behavior.is_empty());
        assert_eq!(
            initialized.actions.sends.schedules.as_slice(),
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
            .on(TimerElapsed::new(TimerId(8), crate::TimerGeneration(0)))
            .unwrap();
        assert!(wrong_id.sends == OneShotSends::empty());
        assert_eq!(active.base().elapsed, 0);

        let fired = active
            .on(TimerElapsed::new(TimerId(7), crate::TimerGeneration(0)))
            .unwrap();
        assert!(fired.sends == OneShotSends::empty());
        assert_eq!(active.base().elapsed, 1);

        active
            .on(TimerElapsed::new(TimerId(7), crate::TimerGeneration(0)))
            .unwrap();
        assert_eq!(active.base().elapsed, 1);
    }
}
