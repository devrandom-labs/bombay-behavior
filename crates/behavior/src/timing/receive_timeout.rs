//! Pure receive-inactivity composition. Relative scheduling is a request to
//! the emitting actor's interpreter and never observes a clock here.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::Step;
use crate::behavior::{
    Actions, Address, Behavior, BirthMode, SendAlgebra, ServiceSends, UserEvent,
};
use crate::protocol::{ScheduleAfter, TimerElapsed, TimerId};
use crate::{Own, RouteInput, SendInput};

pub type ReceiveTimeoutEvent<E> = TimedEvent<E>;

pub type ReceiveTimeoutReaction<B> = fn(
    &mut B,
) -> Result<
    Actions<
        <B as Behavior>::Addr,
        <B as Behavior>::Ph,
        <B as Behavior>::Sends,
        <B as Behavior>::Birth,
    >,
    <B as Behavior>::Error,
>;

/// Named effect lanes added by [`ReceiveTimeout`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiveTimeoutSends<Sends> {
    pub behavior: Sends,
    pub schedules: ServiceSends<ScheduleAfter>,
}

impl<Sends: SendAlgebra> SendAlgebra for ReceiveTimeoutSends<Sends> {
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

impl<Sends> SendInput<ScheduleAfter, Own> for ReceiveTimeoutSends<Sends> {
    fn emit(&mut self, input: ScheduleAfter) {
        self.schedules.send(input);
    }
}

pub(crate) type ReceiveTimeoutActions<B> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    ReceiveTimeoutSends<<B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

/// A pure one-notification-per-idle-period receive timeout.
///
/// Only successful user communications are activity. Timer, peer, child,
/// worker, and shutdown service events compose through this wrapper but never
/// rearm it. A matching timeout consumes the live generation before invoking
/// the reaction; if that reaction continues, the timeout remains unarmed until
/// another successful continuing user communication.
pub struct ReceiveTimeout<B: Behavior> {
    inner: B,
    id: TimerId,
    after: Duration,
    timer: TimerLease,
    on_elapsed: ReceiveTimeoutReaction<B>,
}

impl<B: Behavior> ReceiveTimeout<B> {
    #[must_use]
    pub(crate) fn new(
        inner: B,
        id: TimerId,
        after: Duration,
        on_elapsed: ReceiveTimeoutReaction<B>,
    ) -> Self {
        Self {
            inner,
            id,
            after,
            timer: TimerLease::new(),
            on_elapsed,
        }
    }

    fn schedule(&mut self) -> ServiceSends<ScheduleAfter> {
        self.timer
            .arm()
            .map_or_else(ServiceSends::empty, |generation| {
                ServiceSends::one(ScheduleAfter::new(self.id, generation, self.after))
            })
    }

    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ScheduleAfter>,
    ) -> ReceiveTimeoutActions<B> {
        actions.map_sends(|behavior| ReceiveTimeoutSends {
            behavior,
            schedules: own,
        })
    }

    fn terminal(actions: &Actions<B::Addr, B::Ph, B::Sends, B::Birth>) -> bool {
        matches!(actions.become_, Step::Stop(_))
    }
}

impl<B: Behavior + crate::BehaviorBase> crate::BehaviorBase for ReceiveTimeout<B> {
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B> crate::StashStatus for ReceiveTimeout<B>
where
    B: Behavior + crate::StashStatus,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, A, Ph, Sends, Br> Behavior for ReceiveTimeout<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: crate::RouteInput<TimerElapsed>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = ReceiveTimeoutEvent<B::Event>;
    type Sends = ReceiveTimeoutSends<Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        let actions = crate::calculus::initialize(&mut self.inner)?;
        let own = if Self::terminal(&actions) {
            self.timer.disarm();
            ServiceSends::empty()
        } else {
            self.schedule()
        };
        Ok(Self::wrap(actions, own))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        match event {
            ReceiveTimeoutEvent::Elapsed(elapsed)
                if elapsed.id == self.id && self.timer.accept(elapsed.generation) =>
            {
                let actions = (self.on_elapsed)(&mut self.inner)?;
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            ReceiveTimeoutEvent::Elapsed(elapsed) if elapsed.id == self.id => Ok(Actions::cont()),
            ReceiveTimeoutEvent::Elapsed(elapsed) => {
                let Ok(inner) = B::Event::route(elapsed) else {
                    return Ok(Actions::cont());
                };
                let actions = crate::calculus::delegate_transition(&mut self.inner, inner)?;
                if Self::terminal(&actions) {
                    self.timer.disarm();
                }
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            ReceiveTimeoutEvent::Behavior(event) => match event.into_user() {
                Ok(user) => {
                    let event = B::Event::user(user.from, user.message);
                    let actions = crate::calculus::delegate_transition(&mut self.inner, event)?;
                    let own = if Self::terminal(&actions) {
                        self.timer.disarm();
                        ServiceSends::empty()
                    } else {
                        self.schedule()
                    };
                    Ok(Self::wrap(actions, own))
                }
                Err(service) => {
                    let actions = crate::calculus::delegate_transition(&mut self.inner, service)?;
                    if Self::terminal(&actions) {
                        self.timer.disarm();
                    }
                    Ok(Self::wrap(actions, ServiceSends::empty()))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Acted, MailAddr, Never, NoBirths, TimerGeneration, User};

    struct Count(u8);

    impl Behavior for Count {
        type Addr = MailAddr;
        type Msg = ();
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            self.0 += 1;
            Ok(Actions::cont())
        }
    }

    type CountBehavior = Count;

    impl crate::BehaviorBase for Count {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "the reaction fixture must implement the fallible reaction signature"
    )]
    fn elapsed(_inner: &mut CountBehavior) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }

    #[tokio::test]
    async fn exhaustion_retires_only_the_timer_and_preserves_the_inner_fold() {
        let mut timeout =
            ReceiveTimeout::new(Count(0), TimerId(0), Duration::from_secs(1), elapsed);
        crate::calculus::initialize(&mut timeout).unwrap();
        timeout.timer = TimerLease::idle(TimerGeneration(u64::MAX));

        let actions = crate::calculus::delegate_transition(
            &mut timeout,
            ReceiveTimeoutEvent::Behavior(User::user(MailAddr(1), ())),
        )
        .unwrap();

        assert!(actions.sends.schedules.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(crate::BehaviorBase::base(&timeout).0, 1);
        assert_eq!(timeout.timer.live(), None);
    }
}
