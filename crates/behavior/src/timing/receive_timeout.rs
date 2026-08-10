//! Pure receive-inactivity composition. Relative scheduling is a request to
//! the emitting actor's interpreter and never observes a clock here.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::Step;
use crate::behavior::{
    Actions, Address, Behavior, BirthMode, SendAlgebra, ServiceSends, UserEvent,
};
use crate::protocol::{ScheduleAfter, TimeEvent, TimerId};

pub type ReceiveTimeoutEvent<E> = TimedEvent<E>;

/// A controlled receive-timeout failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveTimeoutError<E> {
    /// The inner fold or timeout reaction failed.
    Inner(E),
    /// Advancing the timer generation would make a stale delivery live again.
    ///
    /// This is detected after the successful continuing inner user fold: the
    /// inner state mutation has occurred, but its returned sends and creations
    /// are not emitted because the composed transition fails. Bombay behavior
    /// folds are not transactional and wrappers cannot roll back inner state.
    GenerationExhausted,
}

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

pub type ReceiveTimeoutActions<B> = Actions<
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
    pub fn new(
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

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }

    fn schedule(&mut self) -> Result<ServiceSends<ScheduleAfter>, ReceiveTimeoutError<B::Error>> {
        let generation = self
            .timer
            .arm()
            .map_err(|_| ReceiveTimeoutError::GenerationExhausted)?;
        Ok(ServiceSends::one(ScheduleAfter::new(
            self.id, generation, self.after,
        )))
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

impl<B, A, Ph, Sends, Br> Behavior for ReceiveTimeout<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: TimeEvent,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = ReceiveTimeoutEvent<B::Event>;
    type Sends = ReceiveTimeoutSends<Sends>;
    type Ph = Ph;
    type Error = ReceiveTimeoutError<B::Error>;
    type Birth = Br;

    fn init(&mut self) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        let actions = self.inner.init().map_err(ReceiveTimeoutError::Inner)?;
        let own = if Self::terminal(&actions) {
            self.timer.disarm();
            ServiceSends::empty()
        } else {
            self.schedule()?
        };
        Ok(Self::wrap(actions, own))
    }

    fn transition(&mut self, event: Self::Event) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        match event {
            ReceiveTimeoutEvent::Elapsed(elapsed)
                if elapsed.id == self.id && self.timer.accept(elapsed.generation) =>
            {
                let actions =
                    (self.on_elapsed)(&mut self.inner).map_err(ReceiveTimeoutError::Inner)?;
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            ReceiveTimeoutEvent::Elapsed(elapsed) if elapsed.id == self.id => Ok(Actions::cont()),
            ReceiveTimeoutEvent::Elapsed(elapsed) => {
                let Some(inner) = B::Event::time_reached(elapsed) else {
                    return Ok(Actions::cont());
                };
                let actions = self
                    .inner
                    .transition(inner)
                    .map_err(ReceiveTimeoutError::Inner)?;
                if Self::terminal(&actions) {
                    self.timer.disarm();
                }
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            ReceiveTimeoutEvent::Inner(event) => match event.into_user() {
                Ok(user) => {
                    let event = B::Event::user(user.from, user.message);
                    let actions = self
                        .inner
                        .transition(event)
                        .map_err(ReceiveTimeoutError::Inner)?;
                    let own = if Self::terminal(&actions) {
                        self.timer.disarm();
                        ServiceSends::empty()
                    } else {
                        self.schedule()?
                    };
                    Ok(Self::wrap(actions, own))
                }
                Err(service) => {
                    let actions = self
                        .inner
                        .transition(service)
                        .map_err(ReceiveTimeoutError::Inner)?;
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
    use crate::{Acted, Delivery, Handler, MailAddr, Never, NoBirths, Pure, TimerGeneration, User};

    struct Count(u8);

    impl Handler for Count {
        type Addr = MailAddr;
        type Msg = ();

        fn receive(
            &mut self,
            _from: MailAddr,
            (): (),
        ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
            self.0 += 1;
            Ok(Actions::cont())
        }
    }

    type Inner = Pure<Count>;

    fn elapsed(
        _inner: &mut Inner,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        Ok(Actions::cont())
    }

    #[tokio::test]
    async fn exhaustion_follows_the_successful_inner_fold_without_emitting_its_actions() {
        let mut timeout = ReceiveTimeout::new(
            Pure::new(Count(0)),
            TimerId(0),
            Duration::from_secs(1),
            elapsed,
        );
        timeout.init().unwrap();
        timeout.timer = TimerLease::idle(TimerGeneration(u64::MAX));

        let result = timeout.transition(ReceiveTimeoutEvent::Inner(User::user(MailAddr(1), ())));

        assert!(matches!(
            result,
            Err(ReceiveTimeoutError::GenerationExhausted)
        ));
        assert_eq!(timeout.inner().state().0, 1);
        assert_eq!(timeout.timer.live(), None);
    }
}
