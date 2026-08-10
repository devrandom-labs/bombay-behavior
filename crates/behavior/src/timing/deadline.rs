//! Pure one-shot time composition. Scheduling is a request to the emitting
//! actor's local clock service.

use tokio::time::Instant;

use super::domain::OneShotSchedule;
use super::event::TimedEvent;
use crate::Step;
use crate::behavior::{Actions, Address, Become, Behavior, BirthMode, SendAlgebra, ServiceSends};
use crate::protocol::{ScheduleAt, TimeEvent, TimerId};
use crate::{Inner, Own, SendInput};

pub type DeadlineEvent<E> = TimedEvent<E>;

pub type DeadlineReaction<B> =
    fn(&mut B) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

/// Named effect lanes added by [`Deadline`].
pub struct DeadlineSends<Sends> {
    pub behavior: Sends,
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

impl<Sends, Input, Path> SendInput<Input, Inner<Path>> for DeadlineSends<Sends>
where
    Sends: SendInput<Input, Path>,
{
    fn emit(&mut self, input: Input) {
        <Sends as SendInput<Input, Path>>::emit(&mut self.behavior, input);
    }
}

pub type DeadlineActions<B> = Actions<
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
    pub fn new(
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

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B, A, Ph, Sends, Br> Behavior for Deadline<B>
where
    A: Address,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br>,
    B::Event: TimeEvent,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = DeadlineEvent<B::Event>;
    type Sends = DeadlineSends<Sends>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    fn init(&mut self) -> Result<DeadlineActions<B>, B::Error> {
        let actions = self.inner.init()?;
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

    fn transition(&mut self, event: Self::Event) -> Result<DeadlineActions<B>, B::Error> {
        match event {
            DeadlineEvent::Elapsed(event) if self.schedule.accept(event.id, event.generation) => {
                let become_ = match (self.on_reached)(&mut self.inner)? {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::just(become_))
            }
            DeadlineEvent::Elapsed(event) => match B::Event::time_reached(event) {
                Some(inner) => {
                    let actions = self.inner.transition(inner)?;
                    if matches!(actions.become_, Step::Stop(_)) {
                        self.schedule.cancel();
                    }
                    Ok(Self::wrap(actions, ServiceSends::empty()))
                }
                None => Ok(Actions::cont()),
            },
            DeadlineEvent::Inner(event) => {
                let actions = self.inner.transition(event)?;
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
