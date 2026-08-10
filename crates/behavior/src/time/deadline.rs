//! Pure one-shot time composition. Scheduling is a request to the emitting
//! actor's local clock service.

use tokio::time::Instant;

use super::domain::OneShotSchedule;
use super::event::TimedEvent;
use crate::Step;
use crate::behavior::{
    Actions, Address, Become, Behavior, BirthMode, SendAlgebra, SendProduct, ServiceSends,
};
use crate::protocol::{ScheduleAt, TimeEvent, TimerId};

pub type AtEvent<E> = TimedEvent<E>;

pub type AtReaction<B> =
    fn(&mut B) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

pub type AtSends<B> = SendProduct<<B as Behavior>::Sends, ServiceSends<ScheduleAt>>;

pub type AtActions<B> =
    Actions<<B as Behavior>::Addr, <B as Behavior>::Ph, AtSends<B>, <B as Behavior>::Birth>;

pub struct At<B: Behavior> {
    inner: B,
    schedule: OneShotSchedule,
    on_reached: AtReaction<B>,
}

impl<B: Behavior> At<B> {
    #[must_use]
    pub fn new(inner: B, id: TimerId, at: Option<Instant>, on_reached: AtReaction<B>) -> Self {
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

impl<B, A, Ph, Sends, Br> Behavior for At<B>
where
    A: Address + Send,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br> + Send,
    B::Event: TimeEvent + Send,
    B::Msg: Send,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = AtEvent<B::Event>;
    type Sends = SendProduct<Sends, ServiceSends<ScheduleAt>>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Br;

    async fn init(&mut self) -> Result<AtActions<B>, B::Error> {
        let actions = self.inner.init().await?;
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

    async fn step(&mut self, event: Self::Event) -> Result<AtActions<B>, B::Error> {
        match event {
            AtEvent::Elapsed(event) if self.schedule.accept(event.id, event.generation) => {
                let become_ = match (self.on_reached)(&mut self.inner)? {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions::just(become_))
            }
            AtEvent::Elapsed(event) => match B::Event::time_reached(event) {
                Some(inner) => {
                    let actions = self.inner.step(inner).await?;
                    if matches!(actions.become_, Step::Stop(_)) {
                        self.schedule.cancel();
                    }
                    Ok(Self::wrap(actions, ServiceSends::empty()))
                }
                None => Ok(Actions::cont()),
            },
            AtEvent::Inner(event) => {
                let actions = self.inner.step(event).await?;
                if matches!(actions.become_, Step::Stop(_)) {
                    self.schedule.cancel();
                }
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
        }
    }
}

impl<B: Behavior> At<B> {
    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ScheduleAt>,
    ) -> AtActions<B> {
        actions.map_sends(|inner| SendProduct::new(inner, own))
    }
}
