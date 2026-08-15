//! Pure one-shot time composition. Scheduling is a request to the emitting
//! actor's local clock service.

use std::time::Instant;

use super::domain::OneShotSchedule;
use super::event::TimedEvent;
use crate::Step;
use crate::protocol::{ScheduleAt, TimerElapsed, TimerId};
use crate::{Own, RouteInput, SendInput};
use behavior::{Actions, Address, Become, Behavior, BirthMode, SendAlgebra, ServiceSends};

pub type DeadlineEvent<E> = TimedEvent<E>;

pub type DeadlineReaction<B> = fn(&mut B) -> Result<Become, <B as Behavior>::Error>;

/// Named effect lanes added by [`Deadline`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineSends<Sends> {
    /// Sends emitted by the wrapped behavior or its deadline reaction.
    pub behavior: Sends,
    /// Absolute scheduling requests interpreted by the local timer capability.
    pub schedules: ServiceSends<ScheduleAt>,
}

impl<Sends: SendAlgebra> SendAlgebra for DeadlineSends<Sends> {
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

impl<Sends> SendInput<ScheduleAt, Own> for DeadlineSends<Sends> {
    fn emit(&mut self, input: ScheduleAt) {
        self.schedules.send(input);
    }
}

pub(crate) type DeadlineActions<B> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    DeadlineSends<<B as Behavior>::Sends>,
    <B as Behavior>::Birth,
>;

pub struct Deadline<B: Behavior> {
    inner: B,
    schedule: OneShotSchedule,
    on_reached: DeadlineReaction<B>,
}

impl<B: Behavior> Deadline<B> {
    #[must_use]
    pub(crate) fn new(
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
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: crate::RouteInput<TimerElapsed>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = DeadlineEvent<B::Event>;
    type Sends = DeadlineSends<Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self, _: crate::InitializationTurn) -> Result<DeadlineActions<B>, B::Error> {
        let actions = behavior::initialize(&mut self.inner)?;
        let own = if matches!(actions.become_, Step::Stop(_)) {
            self.schedule.cancel();
            ServiceSends::empty()
        } else {
            self.schedule
                .request()
                .map_or_else(ServiceSends::empty, |(id, generation, at)| {
                    ServiceSends::one(ScheduleAt::new(id, generation, at))
                })
        };
        Ok(Self::wrap(actions, own))
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> Result<DeadlineActions<B>, B::Error> {
        match event {
            DeadlineEvent::Elapsed(event) if self.schedule.accept(event.id, event.generation) => {
                let become_ = match (self.on_reached)(&mut self.inner)? {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::just(become_))
            }
            DeadlineEvent::Elapsed(event) => match B::Event::route(event) {
                Ok(inner) => {
                    let actions = behavior::delegate_transition(&mut self.inner, inner)?;
                    if matches!(actions.become_, Step::Stop(_)) {
                        self.schedule.cancel();
                    }
                    Ok(Self::wrap(actions, ServiceSends::empty()))
                }
                Err(_) => Ok(Actions::cont()),
            },
            DeadlineEvent::Behavior(event) => {
                let actions = behavior::delegate_transition(&mut self.inner, event)?;
                if matches!(actions.become_, Step::Stop(_)) {
                    self.schedule.cancel();
                }
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
        }
    }
}

impl<B: Behavior> Deadline<B> {
    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ScheduleAt>,
    ) -> DeadlineActions<B> {
        actions.map_sends(|behavior| DeadlineSends {
            behavior,
            schedules: own,
        })
    }
}
