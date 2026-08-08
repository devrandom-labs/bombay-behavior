//! Pure one-shot time composition. Scheduling is a request to the emitting
//! actor's local clock service.

use tokio::time::Instant;

use crate::Step;
use crate::behavior::{
    Actions, Address, Become, Behavior, BirthMode, SendAlgebra, SendProduct, ServiceSends, User,
    UserEvent,
};
use crate::protocol::{
    AtGeneration, AtId, ChildEvent, ChildStopped, PeerEvent, PeerStopped, ScheduleAt,
    ShutdownEvent, ShutdownRequested, TimeEvent, TimeReached, WorkerEvent, WorkerStopped,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtEvent<E> {
    Inner(E),
    Reached(TimeReached),
}

impl<E> TimeEvent for AtEvent<E> {
    fn time_reached(event: TimeReached) -> Option<Self> {
        Some(Self::Reached(event))
    }
}

impl<E: UserEvent> UserEvent for AtEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            reached @ Self::Reached(_) => Err(reached),
        }
    }
}

impl<E: PeerEvent<A>, A: Address> PeerEvent<A> for AtEvent<E> {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self> {
        E::peer_stopped(event).map(Self::Inner)
    }
}

impl<E: ChildEvent<A>, A: Address> ChildEvent<A> for AtEvent<E> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        E::child_stopped(event).map(Self::Inner)
    }
}

impl<E: WorkerEvent<A>, A: Address> WorkerEvent<A> for AtEvent<E> {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self> {
        E::worker_stopped(event).map(Self::Inner)
    }
}

impl<E: ShutdownEvent> ShutdownEvent for AtEvent<E> {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self> {
        E::shutdown_requested(event).map(Self::Inner)
    }
}

pub type AtReaction<B> =
    fn(&mut B) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

pub type AtSends<B> = SendProduct<<B as Behavior>::Sends, ServiceSends<ScheduleAt>>;

pub type AtActions<B> =
    Actions<<B as Behavior>::Addr, <B as Behavior>::Ph, AtSends<B>, <B as Behavior>::Birth>;

pub struct At<B: Behavior> {
    inner: B,
    id: AtId,
    scheduled: Option<(AtGeneration, Instant)>,
    on_reached: AtReaction<B>,
}

impl<B: Behavior> At<B> {
    #[must_use]
    pub fn new(inner: B, id: AtId, at: Option<Instant>, on_reached: AtReaction<B>) -> Self {
        Self {
            inner,
            id,
            scheduled: at.map(|at| (AtGeneration(0), at)),
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
        let own = self
            .scheduled
            .map_or_else(ServiceSends::empty, |(generation, at)| {
                ServiceSends::one(ScheduleAt {
                    id: self.id,
                    generation,
                    at,
                })
            });
        Ok(Self::wrap(actions, own))
    }

    async fn step(&mut self, event: Self::Event) -> Result<AtActions<B>, B::Error> {
        match event {
            AtEvent::Reached(event)
                if event.id == self.id && self.scheduled == Some((event.generation, event.at)) =>
            {
                self.scheduled = None;
                let become_ = match (self.on_reached)(&mut self.inner)? {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                };
                Ok(Actions {
                    sends: SendProduct {
                        inner: B::Sends::empty(),
                        own: ServiceSends::empty(),
                    },
                    creates: Vec::new(),
                    become_,
                })
            }
            AtEvent::Reached(event) => match B::Event::time_reached(event) {
                Some(inner) => self
                    .inner
                    .step(inner)
                    .await
                    .map(|actions| Self::wrap(actions, ServiceSends::empty())),
                None => Ok(Actions::cont()),
            },
            AtEvent::Inner(event) => self
                .inner
                .step(event)
                .await
                .map(|actions| Self::wrap(actions, ServiceSends::empty())),
        }
    }
}

impl<B: Behavior> At<B> {
    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ScheduleAt>,
    ) -> AtActions<B> {
        Actions {
            sends: SendProduct {
                inner: actions.sends,
                own,
            },
            creates: actions.creates,
            become_: actions.become_,
        }
    }
}
