//! Pure receive-inactivity composition. Relative scheduling is a request to
//! the emitting actor's interpreter and never observes a clock here.

use std::time::Duration;

use super::domain::TimerLease;
use super::event::TimedEvent;
use crate::Step;
use crate::protocol::{ScheduleAfter, TimerId};
use behavior::{
    Actions, Address, Behavior, BirthMode, EventLayer, InterpreterRequests, SendEffects, SendLayer,
    UserEvent,
};

pub type ReceiveTimeoutEvent<E> = TimedEvent<E>;

/// Infallible fold invoked for the accepted inactivity generation.
///
/// ```compile_fail,E0308
/// # use std::time::Duration;
/// # use behavior::{Actions, Behavior, MailAddr, Never, NoBirths, User};
/// # use behavior_actors::{ReceiveTimeout, TimerId};
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = MailAddr; type Msg = (); }
/// # impl Behavior for App {
/// #   type Protocol = Self; type Event = User<MailAddr, ()>; type Sends = Vec<Never>;
/// #   type Ph = Never; type Error = Never; type Birth = NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// fn fallible(_: &mut App) -> behavior::BehaviorActed<App> { Ok(Actions::cont()) }
/// let _ = ReceiveTimeout::new(App, TimerId(1), Duration::from_secs(1), fallible);
/// ```
pub type ReceiveTimeoutReaction<B> = fn(
    &mut B,
) -> Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

pub(crate) type ReceiveTimeoutActions<B> = Actions<
    crate::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<InterpreterRequests<ScheduleAfter>, <B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

/// A pure one-notification-per-idle-period receive timeout.
///
/// Only successful user communications are activity. Timer, peer, child,
/// worker, and shutdown interpreter events compose through this wrapper but never
/// rearm it. A matching timeout invokes the infallible reaction and consumes
/// the live generation. If that reaction continues, the timeout remains
/// unarmed until another successful continuing user communication. Ordinary
/// delegated transitions retain the wrapped error type.
pub struct ReceiveTimeout<B: Behavior> {
    inner: B,
    id: TimerId,
    after: Duration,
    timer: TimerLease,
    on_elapsed: ReceiveTimeoutReaction<B>,
}

impl<B: Behavior> ReceiveTimeout<B> {
    /// Wrap `inner` with a relative inactivity timeout.
    ///
    /// Initialization and each successful continuing user fold stage a fresh
    /// timer generation. Interpreter events do not reset inactivity.
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

    fn schedule(&mut self) -> InterpreterRequests<ScheduleAfter> {
        self.timer
            .arm()
            .map_or_else(InterpreterRequests::empty, |generation| {
                InterpreterRequests::one(ScheduleAfter::new(self.id, generation, self.after))
            })
    }

    fn wrap(
        actions: Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        own: InterpreterRequests<ScheduleAfter>,
    ) -> ReceiveTimeoutActions<B> {
        actions.map_sends(|inner| SendLayer::new(own, inner))
    }

    fn terminal(actions: &Actions<crate::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>) -> bool {
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
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = ReceiveTimeoutEvent<B::Event>;
    type Sends = SendLayer<InterpreterRequests<ScheduleAfter>, Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(
        &mut self,
        _: crate::InitializationTurn,
    ) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        let actions = behavior::initialize(&mut self.inner)?;
        let own = if Self::terminal(&actions) {
            self.timer.disarm();
            InterpreterRequests::empty()
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
            EventLayer::Owned(elapsed)
                if elapsed.id == self.id && self.timer.accept(elapsed.generation) =>
            {
                let actions = (self.on_elapsed)(&mut self.inner);
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
            EventLayer::Owned(_) => Ok(Actions::cont()),
            EventLayer::Inner(event) => match event.into_user() {
                Ok(user) => {
                    let event = B::Event::user(user.from, user.message);
                    let actions = behavior::delegate_transition(&mut self.inner, event)?;
                    let own = if Self::terminal(&actions) {
                        self.timer.disarm();
                        InterpreterRequests::empty()
                    } else {
                        self.schedule()
                    };
                    Ok(Self::wrap(actions, own))
                }
                Err(service) => {
                    let actions = behavior::delegate_transition(&mut self.inner, service)?;
                    if Self::terminal(&actions) {
                        self.timer.disarm();
                    }
                    Ok(Self::wrap(actions, InterpreterRequests::empty()))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MailAddr, Never, NoBirths, TimerGeneration, User};

    struct Count(u8);

    impl behavior::Protocol for Count {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Count {
        type Protocol = Self;
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

    fn elapsed(_inner: &mut CountBehavior) -> Actions<MailAddr, Never, Vec<Never>, NoBirths> {
        Actions::cont()
    }

    #[tokio::test]
    async fn exhaustion_retires_only_the_timer_and_preserves_the_inner_fold() {
        let mut timeout =
            ReceiveTimeout::new(Count(0), TimerId(0), Duration::from_secs(1), elapsed);
        behavior::initialize(&mut timeout).unwrap();
        timeout.timer = TimerLease::idle(TimerGeneration(u64::MAX));

        let actions = behavior::delegate_transition(
            &mut timeout,
            EventLayer::Inner(User::user(MailAddr(1), ())),
        )
        .unwrap();

        assert!(actions.sends.owned.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(crate::BehaviorBase::base(&timeout).0, 1);
        assert_eq!(timeout.timer.live(), None);
    }
}
