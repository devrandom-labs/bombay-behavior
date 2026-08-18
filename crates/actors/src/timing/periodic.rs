//! Pure generation-safe periodic timer composition.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::protocol::{ScheduleAfter, TimerElapsed, TimerId};
use crate::{Own, RouteInput, SendInput, Step};
use behavior::{Actions, Address, Behavior, BehaviorActed, BirthMode, SendAlgebra, ServiceSends};

/// Complete event sum accepted by [`Periodic`].
pub type PeriodicEvent<E> = TimedEvent<E>;

/// Pure fold invoked for each accepted periodic generation.
pub type PeriodicReaction<B> = fn(&mut B) -> BehaviorActed<B>;

/// Named send product contributed by [`Periodic`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodicSends<Sends> {
    /// Sends emitted by the wrapped behavior or periodic reaction.
    pub behavior: Sends,
    /// Relative schedule requests interpreted by Bombay Timers.
    pub schedules: ServiceSends<ScheduleAfter>,
}

impl<Sends: SendAlgebra> SendAlgebra for PeriodicSends<Sends> {
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

impl<Sends> SendInput<ScheduleAfter, Own> for PeriodicSends<Sends> {
    fn emit(&mut self, input: ScheduleAfter) {
        self.schedules.send(input);
    }
}

/// Repeatedly notify a wrapped behavior at a relative interval.
///
/// Initialization preserves inner effects and appends generation zero. Each
/// matching timer event is consumed once, folds `on_elapsed`, and—only when
/// that fold continues—appends the next generation. Stale, duplicate, and
/// wrong-ID observations are inert unless the inner protocol accepts them.
/// Errors consume the delivered generation and emit no replacement schedule;
/// termination also emits no replacement. Generation exhaustion leaves the
/// timer retired while preserving the inner continuation. These are Bombay
/// policies; clock access and sleeping remain `bombay-timers` capabilities.
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

    fn schedule(&mut self) -> ServiceSends<ScheduleAfter> {
        self.lease
            .arm()
            .map_or_else(ServiceSends::empty, |generation| {
                ServiceSends::one(ScheduleAfter::new(self.id, generation, self.every))
            })
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        schedules: ServiceSends<ScheduleAfter>,
    ) -> Actions<crate::BehaviorAddr<B>, B::Ph, PeriodicSends<B::Sends>, B::Birth> {
        actions.map_sends(|behavior| PeriodicSends {
            behavior,
            schedules,
        })
    }

    fn wrap_and_rearm(
        &mut self,
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
    ) -> Actions<crate::BehaviorAddr<B>, B::Ph, PeriodicSends<B::Sends>, B::Birth> {
        let schedules = if matches!(actions.become_, Step::Stop(_)) {
            self.lease.disarm();
            ServiceSends::empty()
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
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
    B::Event: RouteInput<TimerElapsed>,
{
    type Protocol = B::Protocol;
    type Event = PeriodicEvent<B::Event>;
    type Sends = PeriodicSends<Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> BehaviorActed<Self> {
        let actions = behavior::initialize(&mut self.inner)?;
        Ok(self.wrap_and_rearm(actions))
    }

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            TimedEvent::Elapsed(elapsed)
                if elapsed.id == self.id && self.lease.accept(elapsed.generation) =>
            {
                let actions = (self.on_elapsed)(&mut self.inner)?;
                Ok(self.wrap_and_rearm(actions))
            }
            TimedEvent::Elapsed(elapsed) => match B::Event::route(elapsed) {
                Ok(inner) => {
                    let actions = behavior::delegate_transition(&mut self.inner, inner)?;
                    Ok(Self::wrap(actions, ServiceSends::empty()))
                }
                Err(_) => Ok(Actions::cont()),
            },
            TimedEvent::Behavior(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)?;
                if matches!(actions.become_, Step::Stop(_)) {
                    self.lease.disarm();
                }
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
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

    #[allow(
        clippy::unnecessary_wraps,
        reason = "reaction type is deliberately fallible"
    )]
    fn tick(probe: &mut Probe) -> BehaviorActed<Probe> {
        probe.0 += 1;
        Ok(if probe.0 == 2 {
            Actions::stop()
        } else {
            Actions::cont()
        })
    }

    #[test]
    fn accepted_ticks_rearm_until_the_reaction_stops() {
        let every = Duration::from_secs(3);
        let initialized = crate::Periodic::new(Probe(0), TimerId(2), every, tick)
            .initialize()
            .unwrap();
        assert_eq!(
            initialized.actions.sends.schedules.as_slice(),
            [ScheduleAfter::new(
                TimerId(2),
                crate::TimerGeneration(0),
                every
            )]
        );
        let mut active = initialized.behavior;

        let first = active
            .on(TimerElapsed::new(TimerId(2), crate::TimerGeneration(0)))
            .unwrap();
        assert_eq!(
            first.sends.schedules.as_slice(),
            [ScheduleAfter::new(
                TimerId(2),
                crate::TimerGeneration(1),
                every
            )]
        );
        assert!(matches!(first.become_, Step::Continue));
        assert_eq!(active.base().0, 1);

        let duplicate = active
            .on(TimerElapsed::new(TimerId(2), crate::TimerGeneration(0)))
            .unwrap();
        assert!(duplicate.sends.schedules.is_empty());
        assert_eq!(active.base().0, 1);

        let stopped = active
            .on(TimerElapsed::new(TimerId(2), crate::TimerGeneration(1)))
            .unwrap();
        assert!(stopped.sends.schedules.is_empty());
        assert!(matches!(stopped.become_, Step::Stop(_)));
        assert_eq!(active.base().0, 2);
    }
}
